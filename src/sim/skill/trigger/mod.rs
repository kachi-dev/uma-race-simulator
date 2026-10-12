//! Skill conditions: a condition string parses into a collection of triggers (OR-groups of AND-terms) that
//! updates every frame; plus the per-horse order-in/out-continue flags the order-rate triggers read.
//!
//! Every trigger is a [`SkillTrigger`] (shared fields) + a [`SkillTriggerKind`] (the keyword's own state).

pub mod kind;
pub mod read;
pub mod window;

use crate::sim::ai;
use crate::sim::defs::SkillTriggerOperator;
use crate::sim::race::Race;
use crate::sim::skill::trigger::kind::{create_trigger, SkillTriggerKind};
use crate::sim::skill::trigger::read::{activate_skill_count, first_horse, horse_by_order, last_horse_index, value_lh};
use crate::sim::skill::trigger::window::{all_corner_random_init, calc_final_corner_distance, init_distance};
use crate::sim::skill::{self, SkillDetailRef};
use crate::sim::util::{approximately, f32_to_i32};

/// A parsed condition string; `Null` for an empty one.
#[derive(Clone, Debug, Default)]
pub enum SkillTriggerCollection {
    #[default]
    Null,
    /// OR-groups (split by `@`) of AND-terms (split by `&`), left to right.
    /// Flattened (perf): the triggers live contiguously in the owner's [`skill::SkillManager::triggers`] arena at
    /// `start..start + len`, in the same left-to-right order, each tagged with its group index
    /// ([`SkillTrigger::group`]); an empty group (`"a@"`) has no triggers, so it is kept as `any_empty_group`
    /// (an empty AND-group counts as activated).
    Collection { start: u32, len: u32, any_empty_group: bool },
}

impl SkillTriggerCollection {
    /// This collection's triggers in the owner's `SkillManager::triggers` arena.
    pub fn range(&self) -> std::ops::Range<usize> {
        match *self {
            SkillTriggerCollection::Null => 0..0,
            SkillTriggerCollection::Collection { start, len, .. } => start as usize..(start + len) as usize,
        }
    }
}

/// One condition term: `keyword op value_rh`. `kind == Null` (unknown keyword) is never activated and never updates.
#[derive(Clone, Debug, Default)]
pub struct SkillTrigger {
    pub op: SkillTriggerOperator,
    pub value_rh: i32,
    pub is_activated: bool,
    /// Index of the OR-group in the collection (see [`SkillTriggerCollection`]).
    pub group: u16,
    pub kind: SkillTriggerKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriggerSlot {
    /// precondition_N.
    Pre,
    /// condition_N.
    Main,
}

/// Splits a term into (keyword, operator, value). Exactly one operator; the value is an integer with optional
/// surrounding white space and sign. Two-char operators are tried first.
pub fn parse_term(term: &str) -> Option<(&str, SkillTriggerOperator, i32)> {
    const OPS: [(&str, SkillTriggerOperator); 6] = [
        ("==", SkillTriggerOperator::Equal),
        ("!=", SkillTriggerOperator::NotEqual),
        (">=", SkillTriggerOperator::GreaterEqual),
        ("<=", SkillTriggerOperator::LesserEqual),
        (">", SkillTriggerOperator::Greater),
        ("<", SkillTriggerOperator::Lesser),
    ];
    let mut found = None;
    let mut i = 0;
    let b = term.as_bytes();
    while i < b.len() {
        if let Some(&(s, op)) = OPS.iter().find(|(s, _)| term[i..].starts_with(s)) {
            if found.is_some() {
                return None;
            }
            found = Some((i, s.len(), op));
            i += s.len();
        } else {
            i += 1;
        }
    }
    let (pos, len, op) = found?;
    let v = term[pos + len..].trim_matches(|c: char| matches!(c, ' ' | '\t' | '\n' | '\x0B' | '\x0C' | '\r'));
    let value: i32 = v.parse().ok()?;
    Some((&term[..pos], op, value))
}

/// Some triggers draw RNG at construction (left to right).
pub fn create_trigger_collection(race: &mut Race, r: SkillDetailRef, text: &str) -> SkillTriggerCollection {
    if text.is_empty() {
        return SkillTriggerCollection::Null;
    }
    let start = race.horses[r.horse].skill_manager.triggers.len();
    let mut any_empty_group = false;
    for (g, part) in text.split('@').enumerate() {
        if part.is_empty() {
            any_empty_group = true;
            continue;
        }
        for term in part.split('&') {
            let t = parse_term(term).and_then(|(kw, op, v)| create_trigger(race, r, kw, op, v));
            race.horses[r.horse].skill_manager.triggers.push(SkillTrigger { group: g as u16, ..t.unwrap_or_default() });
        }
    }
    let len = race.horses[r.horse].skill_manager.triggers.len() - start;
    SkillTriggerCollection::Collection { start: start as u32, len: len as u32, any_empty_group }
}

fn check_value(lh: i32, op: SkillTriggerOperator, rh: i32) -> bool {
    use SkillTriggerOperator::*;
    match op {
        Equal => lh == rh,
        NotEqual => lh != rh,
        Greater => lh > rh,
        GreaterEqual => lh >= rh,
        Lesser => lh < rh,
        LesserEqual => lh <= rh,
    }
}

/// `==` is `approximately`.
fn check_value_float(lh: f32, op: SkillTriggerOperator, rh: i32) -> bool {
    use SkillTriggerOperator::*;
    let rh = rh as f32;
    match op {
        Equal => approximately(lh, rh),
        NotEqual => !approximately(lh, rh),
        Greater => rh < lh,
        GreaterEqual => rh <= lh,
        Lesser => lh < rh,
        LesserEqual => lh <= rh,
    }
}

/// `==` is `approximately`.
fn check_value_input_float(lh: f32, op: SkillTriggerOperator, rhf: f32) -> bool {
    use SkillTriggerOperator::*;
    match op {
        Equal => approximately(lh, rhf),
        NotEqual => !approximately(lh, rhf),
        Greater => lh > rhf,
        GreaterEqual => lh >= rhf,
        Lesser => lh < rhf,
        LesserEqual => lh <= rhf,
    }
}

/// Whether the trigger is met this frame; updates the trigger's own state.
/// `others_activated`: every other term of the AND-group is activated (used by random_lot_other_activate only).
fn check_activate(race: &mut Race, r: SkillDetailRef, t: &mut SkillTrigger, others_activated: bool) -> bool {
    use SkillTriggerKind as K;
    let owner = r.horse;
    let (op, rh, was_activated) = (t.op, t.value_rh, t.is_activated);
    match &mut t.kind {
        K::Null => false,
        K::Always => true,
        K::RandomLot(ok) => *ok,
        // Draws a wiz lot when the rest of the group is met and the interval is over; holds while the group stays met.
        K::RandomLotOtherActivate { is_lot_ok, lot_interval, lot_interval_max } => {
            if *is_lot_ok && others_activated {
                return true;
            }
            *is_lot_ok = false;
            if *lot_interval <= 0.0 && others_activated {
                let rnd = race.rng.random_float(100.0);
                let wiz = crate::sim::physics::stats::wiz(&race.horses[owner]);
                *lot_interval = *lot_interval_max;
                let p = &race.data.param.Skill;
                // Evaluated in f64 as one expression, rounded at the store.
                let mut per = (p.LotActivatePerVal1 as f64 - p.LotActivatePerVal2 as f64 / wiz as f64) as f32;
                if !(p.ActivatePerMin as f32 <= per) {
                    per = p.ActivatePerMin as f32;
                }
                *is_lot_ok = rnd < per;
                return *is_lot_ok;
            }
            false
        }
        // Truncating cast.
        K::RaceAccumulateTimeRandom(start) => *start == f32_to_i32(race.accumulate_time_since_start),
        K::OrderRateInContinue(ty) => check_value(race.horses[owner].order_in_continue[*ty] as i32, op, rh),
        K::OrderRateOutContinue(ty) => check_value(race.horses[owner].order_out_continue[*ty] as i32, op, rh),
        K::OrderLastSpurt => race.horses[owner].last_spurt_order >= 0 && check_value(value_lh(race, owner, &t.kind), op, rh),
        K::OrderFinalCornerEnd => {
            race.horses[owner].final_corner_end_order != -1 && check_value(value_lh(race, owner, &t.kind), op, rh)
        }
        K::HpPer | K::HpEmpty => race.horses[owner].is_param_initialized && check_value(value_lh(race, owner, &t.kind), op, rh),
        K::Hp => race.horses[owner].is_param_initialized && check_value_float(race.horses[owner].hp, op, rh),
        // True on the frame HP reaches 0.
        K::HpEmptyOneTime { prev_hp, is_initialized } => {
            let h = &race.horses[owner];
            if !*is_initialized {
                if !h.is_param_initialized {
                    return false;
                }
                *prev_hp = h.hp;
                *is_initialized = true;
            }
            if *prev_hp <= 0.0 {
                return false;
            }
            *prev_hp = h.hp;
            *prev_hp <= 0.0
        }
        K::ChangeOrder => was_activated || check_value(value_lh(race, owner, &t.kind), op, rh),
        K::DistanceDiffInfront => race.horses[owner].cur_order != 0 && check_value(value_lh(race, owner, &t.kind), op, rh),
        K::DistanceDiffBehind | K::HorseBehindIn | K::HorseBehindOut => {
            race.horses[owner].horse_data.horse_index != last_horse_index(race) && check_value(value_lh(race, owner, &t.kind), op, rh)
        }
        // Gap in horse lengths (2.5 m).
        K::BashinDiffInfront => {
            let h = &race.horses[owner];
            if h.cur_order == 0 {
                return false;
            }
            let d = race.horses[horse_by_order(race, h.cur_order - 1)].distance - h.distance;
            check_value_float(if d >= 0.0 { d / 2.5 } else { 0.0 }, op, rh)
        }
        K::BashinDiffBehind => {
            let h = &race.horses[owner];
            if h.horse_data.horse_index == last_horse_index(race) {
                return false;
            }
            let d = h.distance - race.horses[horse_by_order(race, h.cur_order + 1)].distance;
            check_value_float(if d >= 0.0 { d / 2.5 } else { 0.0 }, op, rh)
        }
        // Floored to 0.1 m in f32.
        K::DistanceDiffTopFloat(rhf) => {
            let d = race.horses[first_horse(race)].distance - race.horses[owner].distance;
            check_value_input_float((d * 10.0).floor() / 10.0, op, *rhf)
        }
        K::LastSpurt => race.horses[owner].ai.last_spurt_calc_count > 0 && check_value(value_lh(race, owner, &t.kind), op, rh),
        K::ExistCharaId(e) => match op {
            SkillTriggerOperator::Equal => *e,
            SkillTriggerOperator::NotEqual => !*e,
            _ => false,
        },
        // Counted on first use (the count is not reset if it bails out on an uninitialized horse).
        K::SameSkillHorseCount { same_skill_horse_count, is_initialized } => {
            if !*is_initialized {
                let id = race.horses[owner].skill_manager.skills[r.skill].skill_master.id;
                for h in &race.horses {
                    if !h.is_skill_initialized {
                        return false;
                    }
                    if skill::find_skill(&h.skill_manager, id).is_some() {
                        *same_skill_horse_count += 1;
                    }
                }
                *is_initialized = true;
            }
            check_value(*same_skill_horse_count, op, rh)
        }
        K::UsedParticularSkillId(id) => race.horses[owner].skill_manager.used_skill_ids.contains(id),
        // A non-teammate's previous-frame activation has a buff of ability type `target_type`.
        K::OtherCharacterActivateAdvantageSkill(target_type) => {
            let b = race.horses[owner].horse_data.team_id;
            for (hi, details) in &race.field.prev_activated_skills {
                if *hi == owner {
                    continue;
                }
                let a = race.horses[*hi].horse_data.team_id;
                if a != 0 && b != 0 && a == b {
                    continue;
                }
                for d in details {
                    let det = skill::detail(race, *d);
                    for ab in &det.abilities {
                        // An ability without effect reads as type 0, value 0.
                        let (ty, v) = ab.as_ref().map_or((0, 0.0), |x| (x.ability_type, x.ability_value_on_activate));
                        if ty != *target_type {
                            continue;
                        }
                        if ty == 9 {
                            if v > 0.0 {
                                return true;
                            }
                        } else if !skill::is_skill_demerit(ty, v) {
                            return true;
                        }
                    }
                }
            }
            false
        }
        // Latches once a detail of this skill is in the previous-frame activation list.
        K::ActivateOtherSkillDetail(enabled) => {
            if *enabled {
                return true;
            }
            let id = race.horses[owner].skill_manager.skills[r.skill].skill_master.id;
            let sm = &race.horses[owner].skill_manager;
            if sm.prev_activated_details.iter().any(|d| race.horses[d.horse].skill_manager.skills[d.skill].skill_master.id == id) {
                *enabled = true;
            }
            *enabled
        }
        // 1 when the owner activated another skill since the last check.
        K::IsActivateAnySkill { prev_activate_skill_count, activate_this_skill_count } => {
            let cur = activate_skill_count(race, owner) - *activate_this_skill_count;
            let prev = *prev_activate_skill_count;
            *prev_activate_skill_count = cur;
            if prev < cur {
                *activate_this_skill_count += 1;
            }
            check_value((prev < cur) as i32, op, rh)
        }
        K::ActivateSkillTagGroup(tags) => race.horses[owner]
            .skill_manager
            .prev_activated_details
            .iter()
            .any(|d| skill::tag_ids(&race.horses[d.horse].skill_manager.skills[d.skill].skill_master).iter().any(|x| tags.contains(x))),
        // True once, on the first check in the last straight.
        K::LastStraightOneTime(prev) => {
            if !*prev {
                *prev = ai::is_straight_last(&race.horses[owner]);
                return *prev;
            }
            false
        }
        K::FinalCornerLaterHalf { is_initialized, is_exist_final_corner, distance_min, distance_max } => {
            if !(*is_initialized && *is_exist_final_corner) {
                return false;
            }
            let d = race.horses[owner].distance;
            check_value((*distance_min <= d && d <= *distance_max) as i32, op, rh)
        }
        K::Distance(_, w) => {
            let d = race.horses[owner].distance;
            w.is_initialized && w.has_distance && w.distance_min <= d && d <= w.distance_max
        }
        K::AllCornerRandom { distance_set_list, is_initialized } => {
            if !*is_initialized || distance_set_list.is_empty() {
                return false;
            }
            let d = race.horses[owner].distance;
            for &(lo, hi) in distance_set_list.iter() {
                if d < lo {
                    return false;
                }
                if d <= hi {
                    return true;
                }
            }
            false
        }
        _ => check_value(value_lh(race, owner, &t.kind), op, rh),
    }
}

/// Reserved details always activate.
fn trigger_update(race: &mut Race, r: SkillDetailRef, t: &mut SkillTrigger, dt: f32, reserved: bool, others_activated: bool) {
    use SkillTriggerKind as K;
    if matches!(t.kind, K::Null) {
        return;
    }
    t.is_activated = check_activate(race, r, t, others_activated);
    if reserved {
        t.is_activated = true;
    }
    let value_rh = t.value_rh;
    match &mut t.kind {
        // Retry a failed window pick after the evaluation.
        K::Distance(init, w) if init.retries_in_update() && !w.is_initialized => init_distance(race, *init, value_rh, w),
        K::AllCornerRandom { distance_set_list, is_initialized } if !*is_initialized => {
            all_corner_random_init(race, distance_set_list);
            *is_initialized = true;
        }
        K::RandomLotOtherActivate { lot_interval, .. } => {
            if *lot_interval > 0.0 {
                *lot_interval -= dt;
            }
        }
        // One-time window init after the first evaluation.
        K::FinalCornerLaterHalf { is_initialized, is_exist_final_corner, distance_min, distance_max } if !*is_initialized => {
            *is_initialized = true;
            match calc_final_corner_distance(race) {
                Some((s, e)) => {
                    *is_exist_final_corner = true;
                    *distance_min = (e - s) * 0.5 + s;
                    *distance_max = e;
                }
                None => *is_exist_final_corner = false,
            }
        }
        _ => {}
    }
}

/// Updates every trigger of every group (no short-circuit), then returns whether some group is fully activated.
/// The owner's trigger arena is taken out while the triggers run (only this function reads triggers).
pub fn collection_update(race: &mut Race, r: SkillDetailRef, slot: TriggerSlot, dt: f32) -> bool {
    let d = skill::detail(race, r);
    let c = match slot {
        TriggerSlot::Pre => &d.pre_trigger,
        TriggerSlot::Main => &d.trigger,
    };
    let SkillTriggerCollection::Collection { any_empty_group, .. } = *c else { return false };
    let range = c.range();
    let mut arena = std::mem::take(&mut race.horses[r.horse].skill_manager.triggers);
    let triggers = &mut arena[range];
    let reserved = skill::detail_is_reserved_random_activate(race, r);
    // Groups in order, then triggers in order = the flat list in order.
    let mut start = 0; // first trigger of the current group
    for k in 0..triggers.len() {
        let g = triggers[k].group;
        if triggers[start].group != g {
            start = k;
        }
        // The other group members' current `is_activated`.
        let others = matches!(triggers[k].kind, SkillTriggerKind::RandomLotOtherActivate { .. })
            && triggers[start..].iter().take_while(|t| t.group == g).enumerate().all(|(j, t)| start + j == k || t.is_activated);
        trigger_update(race, r, &mut triggers[k], dt, reserved, others);
    }
    let any = collection_is_activated_any(triggers, any_empty_group);
    race.horses[r.horse].skill_manager.triggers = arena;
    any
}

fn collection_is_activated_any(triggers: &[SkillTrigger], any_empty_group: bool) -> bool {
    any_empty_group || triggers.chunk_by(|a, b| a.group == b.group).any(|g| g.iter().all(|t| t.is_activated))
}

/// A detail with an is_activate_other_skill_detail term runs its own cooldown, and its activation does not count as
/// a skill activation.
pub fn collection_has_activate_other_skill_detail(arena: &[SkillTrigger], c: &SkillTriggerCollection) -> bool {
    arena[c.range()].iter().any(|t| matches!(t.kind, SkillTriggerKind::ActivateOtherSkillDetail(_)))
}

/// Order-in/out thresholds in %.
const ORDER_IN_OUT_PER: [i32; 9] = [10, 20, 30, 40, 50, 60, 70, 80, 90];

/// `v` % of `n`, rounded half up (f32, truncating cast).
fn rescale_rate(v: i32, n: i32) -> i32 {
    f32_to_i32(v as f32 / 100.0 * n as f32 + 0.5)
}

/// `rescale_rate(ORDER_IN_OUT_PER[k], n)` for n < 32, precomputed (the order-in/out updates run per horse per frame).
/// Same f32 ops at compile time; the values are small, so `as i32` equals `f32_to_i32`.
const ORDER_IN_OUT_TH: [[i32; 9]; 32] = {
    let mut t = [[0; 9]; 32];
    let mut n = 0;
    while n < 32 {
        let mut k = 0;
        while k < 9 {
            t[n][k] = (ORDER_IN_OUT_PER[k] as f32 / 100.0 * n as f32 + 0.5) as i32;
            k += 1;
        }
        n += 1;
    }
    t
};

fn order_in_out_th(k: usize, n: i32) -> i32 {
    match ORDER_IN_OUT_TH.get(n as usize) {
        Some(t) if n >= 0 => t[k],
        _ => rescale_rate(ORDER_IN_OUT_PER[k], n),
    }
}

/// Clears each order-in flag once the horse falls behind that threshold (from 5 s into the race).
pub fn update_order_in_continue(race: &mut Race, i: usize) {
    if race.accumulate_time_since_start < 5.0 {
        return;
    }
    let n = race.info.num_race_horses;
    let h = &mut race.horses[i];
    for k in 0..ORDER_IN_OUT_PER.len() {
        if h.order_in_continue[k] && order_in_out_th(k, n) < h.cur_order + 1 {
            h.order_in_continue[k] = false;
        }
    }
}

/// Clears each order-out flag once the horse gets ahead of that threshold (from 5 s into the race).
pub fn update_order_out_continue(race: &mut Race, i: usize) {
    if race.accumulate_time_since_start < 5.0 {
        return;
    }
    let n = race.info.num_race_horses;
    let h = &mut race.horses[i];
    for k in 0..ORDER_IN_OUT_PER.len() {
        if h.order_out_continue[k] && h.cur_order + 1 < order_in_out_th(k, n) {
            h.order_out_continue[k] = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::skill::trigger::kind::order_type;
    use crate::data::{DataEra, GameData};
    use std::path::Path;

    #[test]
    fn order_in_out_table() {
        for n in -1..40 {
            for k in 0..9 {
                assert_eq!(order_in_out_th(k, n), rescale_rate(ORDER_IN_OUT_PER[k], n), "n {n} k {k}");
            }
        }
    }

    #[test]
    fn parse_terms() {
        assert_eq!(parse_term("distance_rate>=50"), Some(("distance_rate", SkillTriggerOperator::GreaterEqual, 50)));
        assert_eq!(parse_term("change_order_onetime<0"), Some(("change_order_onetime", SkillTriggerOperator::Lesser, 0)));
        assert_eq!(parse_term("order!=-1"), Some(("order", SkillTriggerOperator::NotEqual, -1)));
        assert_eq!(parse_term("order==1 "), Some(("order", SkillTriggerOperator::Equal, 1)));
        assert_eq!(parse_term("order"), None);
        assert_eq!(parse_term("order==x"), None);
        assert_eq!(parse_term("a==1==2"), None);
        assert_eq!(order_type("40"), Some(3));
        assert_eq!(rescale_rate(50, 9), 5); // 4.5 + 0.5 → 5
        assert_eq!(rescale_rate(40, 9), 4); // 3.6 + 0.5 = 4.1 → 4
    }

    /// Every condition term of the current master (and of every era snapshot) must parse and build a trigger.
    #[test]
    fn all_master_conditions_parse() {
        let dir = Path::new(crate::data::DATA_DIR);
        if !dir.exists() {
            return;
        }
        let mut race = crate::sim::ai::test_race(2);
        race.horses[0].skill_manager.skills.push(Default::default());
        race.info.horse_index_by_popularity = vec![0, 1];
        let r = SkillDetailRef { horse: 0, skill: 0, detail: 0 };
        let (mut bad, mut checked) = (Vec::new(), 0);
        let mut eras = vec![DataEra::current()];
        for d in ["20260210", "20260620"] {
            eras.push(DataEra::for_date(d).unwrap());
        }
        for era in eras {
            let g = GameData::load(dir, era).unwrap();
            for (id, s) in &g.skills {
                for d in &s.details {
                    for text in [&d.precondition, &d.condition] {
                        for term in text.split(['@', '&']).filter(|t| !t.is_empty()) {
                            checked += 1;
                            let built = parse_term(term).and_then(|(kw, op, rh)| create_trigger(&mut race, r, kw, op, rh));
                            if built.is_none() {
                                bad.push(format!("{id}: {term:?} in {text:?}"));
                            }
                        }
                    }
                }
            }
        }
        assert!(checked > 1000, "only {checked} terms checked");
        assert!(bad.is_empty(), "unknown/unparsable condition terms:\n{}", bad.join("\n"));
    }
}
