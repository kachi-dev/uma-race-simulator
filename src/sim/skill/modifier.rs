//! Skill modifiers (timed or one-time Add/Set on a parameter) and the per-horse receiver that holds them.
//! Everything reads skill effects through `SkillModifierReceiver::apply_modifier` and friends.

use crate::sim::defs::SkillModifierParam;
use crate::sim::util::approximately;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SkillParamModifierKind {
    #[default]
    Add,
    Set,
}

/// `id` identifies it across the receiver and the ability that pushed it.
#[derive(Clone, Copy, Debug, Default)]
pub struct SkillParamModifier {
    pub id: u64,
    pub kind: SkillParamModifierKind,
    pub time: f32,
    pub is_one_time: bool,
    pub is_time_infinite: bool,
    pub value: f32,
}

impl SkillParamModifier {
    /// Lasts `time` seconds; a negative time means forever.
    pub fn new_timed(id: u64, kind: SkillParamModifierKind, value: f32, time: f32) -> Self {
        SkillParamModifier { id, kind, time, is_one_time: false, is_time_infinite: 0.0 > time, value }
    }

    /// Expires on the next update.
    pub fn new_one_time(id: u64, kind: SkillParamModifierKind, value: f32) -> Self {
        SkillParamModifier { id, kind, time: 0.0, is_one_time: true, is_time_infinite: false, value }
    }

    /// Advances the timer; true = expired.
    pub fn update(&mut self, dt: f32) -> bool {
        if self.is_time_infinite {
            return false;
        }
        if self.is_one_time {
            return true;
        }
        self.time = self.time - dt;
        if 0.0 > self.time {
            self.time = 0.0;
        }
        0.0 >= self.time
    }

    pub fn add_ability_time(&mut self, additional_ability_time: f32) {
        if !self.is_time_infinite && !self.is_one_time {
            self.time = additional_ability_time + self.time;
        }
    }

    pub fn apply(&self, x: f32) -> f32 {
        match self.kind {
            SkillParamModifierKind::Add => x + self.value,
            SkillParamModifierKind::Set => self.value,
        }
    }

    /// Add passes the bool through; Set gives `value != 0` (approximately).
    pub fn apply_bool(&self, b: bool) -> bool {
        match self.kind {
            SkillParamModifierKind::Add => b,
            SkillParamModifierKind::Set => !approximately(self.value, 0.0),
        }
    }
}

/// Modifier lists per parameter, in the order each key was first added (keys are never removed).
pub type ModifierDict = Vec<(SkillModifierParam, Vec<SkillParamModifier>)>;

fn list(d: &ModifierDict, t: SkillModifierParam) -> Option<&Vec<SkillParamModifier>> {
    d.iter().find(|(k, _)| *k == t).map(|(_, l)| l)
}

fn list_mut(d: &mut ModifierDict, t: SkillModifierParam) -> Option<&mut Vec<SkillParamModifier>> {
    d.iter_mut().find(|(k, _)| *k == t).map(|(_, l)| l)
}

fn add(d: &mut ModifierDict, t: SkillModifierParam, m: SkillParamModifier) {
    match list_mut(d, t) {
        Some(l) => l.push(m),
        None => d.push((t, vec![m])),
    }
}

#[derive(Clone, Debug, Default)]
pub struct SkillModifierReceiver {
    pub modifier: ModifierDict,
    /// Expired natural-deceleration speed modifiers, handed back once by `apply_one_time_skill_modifier`.
    pub one_time_skill_modifier: ModifierDict,
}

impl SkillModifierReceiver {
    /// Ticks every modifier and drops the expired ones (keys in insertion order, each list backwards). Expired
    /// `CurrentSpeedWithNaturalDeceleration` modifiers move to `one_time_skill_modifier`.
    pub fn update_modifiers(&mut self, dt: f32) {
        for k in 0..self.modifier.len() {
            let t = self.modifier[k].0;
            let mut i = self.modifier[k].1.len();
            while i > 0 {
                i -= 1;
                if self.modifier[k].1[i].update(dt) {
                    let m = self.modifier[k].1.remove(i);
                    if t == SkillModifierParam::CurrentSpeedWithNaturalDeceleration {
                        add(&mut self.one_time_skill_modifier, t, m);
                    }
                }
            }
        }
    }

    pub fn add_modifier(&mut self, t: SkillModifierParam, m: SkillParamModifier) {
        add(&mut self.modifier, t, m);
    }

    pub fn has_modifier(&self, t: SkillModifierParam) -> bool {
        list(&self.modifier, t).is_some_and(|l| !l.is_empty())
    }

    /// `value` with every modifier of `t` applied in insertion order.
    pub fn apply_modifier(&self, t: SkillModifierParam, value: f32) -> f32 {
        list(&self.modifier, t).map_or(value, |l| l.iter().fold(value, |x, m| m.apply(x)))
    }

    pub fn apply_modifier_bool(&self, t: SkillModifierParam, value: bool) -> bool {
        list(&self.modifier, t).map_or(value, |l| l.iter().fold(value, |b, m| m.apply_bool(b)))
    }

    /// Applies and consumes the handed-back modifiers of `t`.
    pub fn apply_one_time_skill_modifier(&mut self, t: SkillModifierParam, value: f32) -> f32 {
        match list_mut(&mut self.one_time_skill_modifier, t) {
            Some(l) if !l.is_empty() => {
                let x = l.iter().fold(value, |x, m| m.apply(x));
                l.clear();
                x
            }
            _ => value,
        }
    }

    pub fn remove_modifier(&mut self, t: SkillModifierParam) {
        if let Some(l) = list_mut(&mut self.modifier, t) {
            l.clear();
        }
    }

    /// Extends modifier `id`. One that already left `modifier` (expired, cleared or handed back) no longer affects
    /// anything, so not finding it is a no-op.
    pub fn add_ability_time(&mut self, t: SkillModifierParam, id: u64, additional_ability_time: f32) {
        if let Some(m) = list_mut(&mut self.modifier, t).and_then(|l| l.iter_mut().find(|m| m.id == id)) {
            m.add_ability_time(additional_ability_time);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SkillModifierParam as P;
    use super::*;

    fn add_m(id: u64, v: f32, t: f32) -> SkillParamModifier {
        SkillParamModifier::new_timed(id, SkillParamModifierKind::Add, v, t)
    }

    #[test]
    fn add_set_fold_in_insertion_order() {
        let mut r = SkillModifierReceiver::default();
        assert_eq!(r.apply_modifier(P::Speed, 7.0), 7.0);
        assert!(!r.has_modifier(P::Speed));
        r.add_modifier(P::Speed, add_m(1, 0.1, 1.0));
        r.add_modifier(P::Speed, SkillParamModifier::new_timed(2, SkillParamModifierKind::Set, 3.0, 1.0));
        r.add_modifier(P::Speed, add_m(3, 0.2, 1.0));
        // ((x + 0.1) → 3.0) + 0.2
        assert_eq!(r.apply_modifier(P::Speed, 7.0), 3.0f32 + 0.2f32);
        // f32 sum in order: 0.1 + 0.2 starting from 0
        r.add_modifier(P::Accel, add_m(4, 0.1, 1.0));
        r.add_modifier(P::Accel, add_m(5, 0.2, 1.0));
        assert_eq!(r.apply_modifier(P::Accel, 0.0).to_bits(), (0.1f32 + 0.2f32).to_bits());
        r.remove_modifier(P::Speed);
        assert!(!r.has_modifier(P::Speed));
        assert_eq!(r.modifier[0].0, P::Speed); // key stays
    }

    #[test]
    fn bool_semantics() {
        let mut r = SkillModifierReceiver::default();
        assert!(!r.apply_modifier_bool(P::DebuffCancel, false));
        r.add_modifier(P::DebuffCancel, SkillParamModifier::new_timed(1, SkillParamModifierKind::Set, 1.0, 2.0));
        assert!(r.apply_modifier_bool(P::DebuffCancel, false));
        r.add_modifier(P::DebuffCancel, add_m(2, 5.0, 2.0)); // Add passes the bool through
        assert!(r.apply_modifier_bool(P::DebuffCancel, false));
    }

    #[test]
    fn expiry_and_one_time() {
        let dt = 0.0666f32;
        let mut r = SkillModifierReceiver::default();
        r.add_modifier(P::TargetSpeed, add_m(1, 0.35, 0.1));
        r.add_modifier(P::TargetSpeed, SkillParamModifier::new_timed(2, SkillParamModifierKind::Add, 1.0, -1.0));
        r.add_modifier(P::Hp, SkillParamModifier::new_one_time(3, SkillParamModifierKind::Add, 50.0));
        r.update_modifiers(dt);
        // one-time removed at the first update; 0.1 - 0.0666 > 0 still alive; infinite stays.
        assert!(!r.has_modifier(P::Hp));
        assert_eq!(r.apply_modifier(P::TargetSpeed, 0.0), 0.35f32 + 1.0f32);
        r.add_ability_time(P::TargetSpeed, 1, 0.05);
        r.update_modifiers(dt);
        assert_eq!(r.apply_modifier(P::TargetSpeed, 0.0), 0.35f32 + 1.0f32); // 0.1 + 0.05 - 2dt > 0
        r.update_modifiers(dt);
        assert_eq!(r.apply_modifier(P::TargetSpeed, 0.0), 1.0f32);
        // time 0 expires on the first update
        let mut m = add_m(9, 1.0, 0.0);
        assert!(m.update(dt));
    }

    #[test]
    fn natural_deceleration_hand_back() {
        let mut r = SkillModifierReceiver::default();
        let p = P::CurrentSpeedWithNaturalDeceleration;
        r.add_modifier(p, add_m(1, 0.15, 0.05));
        assert_eq!(r.apply_one_time_skill_modifier(p, 0.0), 0.0);
        r.update_modifiers(0.0666);
        assert!(!r.has_modifier(p));
        assert_eq!(r.apply_one_time_skill_modifier(p, 0.0), 0.15);
        assert_eq!(r.apply_one_time_skill_modifier(p, 0.0), 0.0); // consumed
    }
}
