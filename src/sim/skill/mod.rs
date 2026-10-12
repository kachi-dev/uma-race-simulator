//! Skill object model and lifecycle: creation, wiz lot, gate check, per-frame update, activation,
//! duration/cooldown and reserved random activation.
//!
//! Addressing: a detail is `(horse i, skill s, detail d)` = `race.horses[i].skill_manager.skills[s].details[d]`.
//! Collections that must run with `&mut Race` (triggers, abilities) can be taken out with `std::mem::take`,
//! updated, and put back — but then code running meanwhile sees the taken slot empty; only do it where
//! nothing else can read that slot.

pub mod ability;
pub mod modifier;
pub mod target;
pub mod trigger;
pub mod value;

use crate::sim::race::Race;
use crate::sim::skill::ability::{SkillAbility};
use crate::sim::skill::trigger::{SkillTrigger, SkillTriggerCollection, TriggerSlot};
use crate::sim::skill::value::{AbilityTime};
use crate::sim::record;
use crate::data::{SkillAbilityData, SkillData};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SkillDetailRef {
    pub horse: usize,
    pub skill: usize,
    pub detail: usize,
}

#[derive(Clone, Debug, Default)]
pub struct SkillManager {
    /// In input order.
    pub skills: Vec<SkillBase>,
    pub used_skill_ids: Vec<i32>,
    pub prev_activated_details: Vec<SkillDetailRef>,
    pub activate_skills: Vec<SkillDetailRef>,
    pub reserved_skill_ids: Vec<i32>,
    /// Arena holding the triggers of every `SkillTriggerCollection` of this horse's details, contiguous in creation
    /// order (perf: the per-frame trigger walk reads one array per horse).
    pub triggers: Vec<SkillTrigger>,
    /// Set once construction is done. Before that `find_skill` finds nothing, so duplicate ids are not dropped
    /// while skills are added.
    pub has_skill_array: bool,
}

#[derive(Clone, Debug, Default)]
pub struct SkillBase {
    pub is_activate_enable: bool,
    /// Detail 0 = the `*_1` master columns, detail 1 = `*_2`.
    pub details: [SkillDetail; 2],
    /// Era-effective master row.
    pub skill_master: Box<SkillData>,
    /// Hot copies of `skill_master.id` / `.priority` (perf: the per-frame loops read only these).
    pub id: i32,
    pub priority: i32,
    pub level: i32,
    pub cool_down_time: f32,
}

/// One condition/effect half of a skill.
#[derive(Clone, Debug, Default)]
pub struct SkillDetail {
    pub detail_index: i32,
    pub trigger: SkillTriggerCollection,
    pub pre_trigger: SkillTriggerCollection,
    pub is_need_pre_trigger_check: bool,
    pub default_cool_down_time: f32,
    pub cool_down_time: f32,
    pub is_use_skill_detail_cool_down_time: bool,
    pub is_not_activate_skill: bool,
    pub is_time_infinite: bool,
    pub left_time: f32,
    pub time: AbilityTime,
    pub skill_level: i32,
    pub is_debuff: bool,
    /// Speed 0, Heal 1, Accel 2, none -1.
    pub category: i32,
    /// `None` = an ability type without effect (6, 501..503, unknown).
    pub abilities: Vec<Option<SkillAbility>>,
    pub is_activated: bool,
}

pub fn detail<'r>(race: &'r Race, r: SkillDetailRef) -> &'r SkillDetail {
    &race.horses[r.horse].skill_manager.skills[r.skill].details[r.detail]
}

pub fn detail_mut<'r>(race: &'r mut Race, r: SkillDetailRef) -> &'r mut SkillDetail {
    &mut race.horses[r.horse].skill_manager.skills[r.skill].details[r.detail]
}

pub fn skill_id(race: &Race, r: SkillDetailRef) -> i32 {
    race.horses[r.horse].skill_manager.skills[r.skill].id
}

/// Never finds anything during construction, see `has_skill_array`.
pub fn find_skill(sm: &SkillManager, id: i32) -> Option<usize> {
    if !sm.has_skill_array {
        return None;
    }
    sm.skills.iter().position(|s| s.id == id)
}

pub fn tag_ids(m: &SkillData) -> Vec<i32> {
    m.tag_id.split('/').filter_map(|t| t.trim().parse().ok()).collect()
}

/// Whether an ability with this type and value is a debuff.
pub fn is_skill_demerit(ability_type: i32, v: f32) -> bool {
    match ability_type {
        1..=5 | 8 | 9 | 21 | 22 | 27 | 28 | 30..=32 => v < 0.0,
        10 => v > 1.0,
        13 | 14 => true,
        29 => !(v < 0.0),
        _ => false,
    }
}

/// Speed 0, Heal 1, Accel 2, none -1.
pub fn skill_category(ability_type: i32) -> i32 {
    match ability_type {
        7 | 9 | 13 => 1,
        21 | 22 | 27 => 0,
        31 => 2,
        _ => -1,
    }
}

/// Era-effective `skill_data` row; `None` for an unknown id (the skill is skipped).
pub fn lookup_skill_master(race: &Race, skill_id: i32) -> Option<SkillData> {
    race.data.skills.get(&skill_id).cloned()
}

/// Trigger construction may draw RNG, in order: detail 1 pre, cond, abilities 1..3, then detail 2.
pub fn create_skill(race: &mut Race, i: usize) {
    race.horses[i].skill_manager = SkillManager::default();
    let list = race.horses[i].horse_data.skills.clone();
    for s in list {
        let Some(m) = lookup_skill_master(race, s.skill_id) else { continue };
        // Duplicate check (never true here, see `has_skill_array`).
        if find_skill(&race.horses[i].skill_manager, m.id).is_some() {
            continue;
        }
        let sm = &mut race.horses[i].skill_manager;
        sm.skills.push(SkillBase { id: m.id, priority: m.priority, skill_master: Box::new(m), level: s.level, ..Default::default() });
        let si = sm.skills.len() - 1;
        setup_detail(race, i, si);
    }
    race.horses[i].skill_manager.has_skill_array = true;
    race.horses[i].is_skill_initialized = true;
}

fn setup_detail(race: &mut Race, i: usize, si: usize) {
    let course_distance = race.info.race_course_set.distance;
    let (div_time, div_cd) = (race.data.param.Skill.AbilityTimeDivideDistance, race.data.param.Skill.CoolDownTimeDivideDistance);
    for d in 0..2 {
        let base = &race.horses[i].skill_manager.skills[si];
        let level = base.level;
        let dd = base.skill_master.details[d].clone();
        let r = SkillDetailRef { horse: i, skill: si, detail: d };
        let ability_time = dd.float_ability_time as f32 * 0.0001;
        let cool_down = dd.float_cooldown_time as f32 * 0.0001;
        let dist_rate_time = value::calc_ability_time_by_distance_rate(course_distance, div_time);
        let ability_time = dist_rate_time * ability_time;
        let tc = value::new_ability_time(dd.ability_time_usage, ability_time, dist_rate_time);
        race.horses[i].skill_manager.skills[si].details[d] = SkillDetail {
            detail_index: d as i32,
            time: tc,
            cool_down_time: 0.0,
            is_time_infinite: ability_time < 0.0,
            default_cool_down_time: value::calc_cool_down_time_by_distance(cool_down, course_distance, div_cd),
            skill_level: level,
            ..Default::default()
        };
        // Trigger construction may draw RNG.
        let pre = trigger::create_trigger_collection(race, r, &dd.precondition);
        let cond = trigger::create_trigger_collection(race, r, &dd.condition);
        let arena = &race.horses[i].skill_manager.triggers;
        let other_detail = trigger::collection_has_activate_other_skill_detail(arena, &pre)
            || trigger::collection_has_activate_other_skill_detail(arena, &cond);
        let det = detail_mut(race, r);
        det.is_need_pre_trigger_check = !matches!(pre, SkillTriggerCollection::Null);
        det.is_use_skill_detail_cool_down_time = other_detail;
        det.is_not_activate_skill = other_detail;
        det.pre_trigger = pre;
        det.trigger = cond;
        let a1 = &dd.abilities[0];
        det.is_debuff = a1.ability_type != 0 && is_skill_demerit(a1.ability_type, a1.float_ability_value as f32 * 0.0001);
        det.category = skill_category(a1.ability_type);
        for ab in &dd.abilities {
            if ab.ability_type != 0 {
                if let Some(a) = create_ability(race, r, level, ab) {
                    detail_mut(race, r).abilities.push(a);
                }
            }
        }
    }
}

/// `None` = not added; `Some(None)` = an ability without effect. The target is always created.
fn create_ability(race: &mut Race, r: SkillDetailRef, level: i32, ab: &SkillAbilityData) -> Option<Option<SkillAbility>> {
    let t = ab.ability_type;
    let value = ab.float_ability_value as f32 * 0.0001;
    let is_demerit = t != 0 && is_skill_demerit(t, value);
    let target = target::new_skill_target(r.horse, ab.target_type, ab.target_value, is_demerit);
    if t < 1 {
        return None;
    }
    match t {
        1..=5 | 7..=14 | 21 | 22 | 27..=32 | 35..=38 => Some(Some(ability::new_skill_ability(race, r, level, ab, target))),
        _ => Some(None),
    }
}

/// Pre-race wiz lot: decides which `activate_lot` skills may activate this race. `base_wiz` is the halved wiz.
pub fn lot_activate_skill(race: &mut Race, i: usize) {
    let p = &race.data.param.Skill;
    let (v1, v2, min) = (p.LotActivatePerVal1, p.LotActivatePerVal2, p.ActivatePerMin as f32);
    for s in 0..race.horses[i].skill_manager.skills.len() {
        let enable = if race.horses[i].skill_manager.skills[s].skill_master.activate_lot == 1 {
            let wiz = race.horses[i].horse_data.race_param.base_wiz;
            // One f64 expression, rounded at the store.
            let thr = (v1 as f64 - v2 as f64 / wiz as f64) as f32;
            let thr = if min > thr { min } else { thr };
            let r = race.rng.random_float(100.0);
            r < thr
        } else {
            true
        };
        race.horses[i].skill_manager.skills[s].is_activate_enable = enable;
    }
}

pub fn check_skill_trigger_and_activate_first_group(race: &mut Race, i: usize) {
    for s in 0..race.horses[i].skill_manager.skills.len() {
        if race.horses[i].skill_manager.skills[s].priority == 1 {
            skill_base_check_trigger_and_activate(race, i, s);
        }
    }
}

pub fn check_skill_trigger_and_activate_second_group(race: &mut Race, i: usize) {
    for s in 0..race.horses[i].skill_manager.skills.len() {
        if race.horses[i].skill_manager.skills[s].priority == 0 {
            skill_base_check_trigger_and_activate(race, i, s);
        }
    }
}

/// Gate check of one skill (dt = 0): no cooldown, no break, already-activated details are re-checked.
pub fn skill_base_check_trigger_and_activate(race: &mut Race, i: usize, s: usize) {
    let enable = race.horses[i].skill_manager.skills[s].is_activate_enable;
    for d in 0..race.horses[i].skill_manager.skills[s].details.len() {
        let r = SkillDetailRef { horse: i, skill: s, detail: d };
        if (enable || detail_is_reserved_random_activate(race, r)) && detail_update_trigger_and_activate(race, r, 0.0) {
            add_prev_activate_skill_detail(race, r);
        }
    }
}

pub fn update_first_group(race: &mut Race, i: usize, dt: f32) {
    for s in 0..race.horses[i].skill_manager.skills.len() {
        if race.horses[i].skill_manager.skills[s].priority == 1 {
            skill_base_update(race, i, s, dt);
        }
    }
}

pub fn update_second_group(race: &mut Race, i: usize, dt: f32) {
    for s in 0..race.horses[i].skill_manager.skills.len() {
        if race.horses[i].skill_manager.skills[s].priority == 0 {
            skill_base_update(race, i, s, dt);
        }
    }
    update_random_activate_skill_id_list(race, i);
}

/// The skill list itself is kept; finished horses no longer update skills.
pub fn clear_skills(race: &mut Race, i: usize) {
    for s in 0..race.horses[i].skill_manager.skills.len() {
        for d in 0..race.horses[i].skill_manager.skills[s].details.len() {
            detail_stop(race, SkillDetailRef { horse: i, skill: s, detail: d });
        }
    }
}

fn add_prev_activate_skill_detail(race: &mut Race, r: SkillDetailRef) {
    let l = &mut race.horses[r.horse].skill_manager.prev_activated_details;
    if !l.contains(&r) {
        l.push(r);
    }
}

/// Per-frame update of one skill: active details run; otherwise the skill cooldown ticks and the first detail whose
/// trigger fires activates. Details with their own cooldown are handled separately at the end.
pub fn skill_base_update(race: &mut Race, i: usize, s: usize, dt: f32) {
    let nd = race.horses[i].skill_manager.skills[s].details.len();
    let rf = |d: usize| SkillDetailRef { horse: i, skill: s, detail: d };
    if race.horses[i].is_complete_first_update {
        for d in 0..nd {
            let l = &mut race.horses[i].skill_manager.prev_activated_details;
            if let Some(k) = l.iter().position(|x| *x == rf(d)) {
                l.remove(k);
            }
        }
    }
    let any_normal_active = (0..nd).any(|d| {
        let det = detail(race, rf(d));
        !det.is_use_skill_detail_cool_down_time && det.is_activated
    });
    if any_normal_active {
        for d in 0..nd {
            let det = detail(race, rf(d));
            if det.is_activated && !det.is_use_skill_detail_cool_down_time && detail_update_skill(race, rf(d), dt) {
                let cd = detail(race, rf(d)).default_cool_down_time;
                race.horses[i].skill_manager.skills[s].cool_down_time = cd;
            }
        }
    } else {
        let base = &mut race.horses[i].skill_manager.skills[s];
        let enable = base.is_activate_enable;
        if 0.0 < base.cool_down_time {
            base.cool_down_time -= dt;
            if base.cool_down_time < 0.0 {
                base.cool_down_time = 0.0;
            }
        }
        if base.cool_down_time <= 0.0 {
            for d in 0..nd {
                if !(enable || detail_is_reserved_random_activate(race, rf(d))) {
                    continue;
                }
                let det = detail(race, rf(d));
                if !det.is_activated && !det.is_use_skill_detail_cool_down_time && detail_update_trigger_and_activate(race, rf(d), dt) {
                    add_prev_activate_skill_detail(race, rf(d));
                    break;
                }
            }
        }
    }
    for d in 0..nd {
        let det = detail(race, rf(d));
        if !det.is_use_skill_detail_cool_down_time {
            continue;
        }
        if !det.is_activated {
            if detail_check_cool_down(race, rf(d), dt)
                && detail_update_trigger_and_activate(race, rf(d), dt)
                && !detail(race, rf(d)).is_not_activate_skill
            {
                add_prev_activate_skill_detail(race, rf(d));
            }
        } else {
            detail_update_skill(race, rf(d), dt);
        }
    }
}

/// Ticks the detail's own cooldown; true once it is over.
fn detail_check_cool_down(race: &mut Race, r: SkillDetailRef, dt: f32) -> bool {
    let det = detail_mut(race, r);
    if 0.0 < det.cool_down_time {
        det.cool_down_time -= dt;
        if det.cool_down_time < 0.0 {
            det.cool_down_time = 0.0;
        }
    }
    det.cool_down_time <= 0.0
}

/// Checks the triggers and activates the detail. The precondition latches: once met it is not checked again.
pub fn detail_update_trigger_and_activate(race: &mut Race, r: SkillDetailRef, dt: f32) -> bool {
    let id = skill_id(race, r);
    let skip_pre = r.detail == 0 && race.horses[r.horse].skill_manager.reserved_skill_ids.contains(&id);
    if !skip_pre && detail(race, r).is_need_pre_trigger_check {
        if !trigger::collection_update(race, r, TriggerSlot::Pre, dt) {
            return false;
        }
        detail_mut(race, r).is_need_pre_trigger_check = false;
    }
    if !trigger::collection_update(race, r, TriggerSlot::Main, dt) {
        return false;
    }
    detail_activate(race, r);
    true
}

/// Bit per target horse index of the first ability (none for an ability without effect).
fn calc_target_flags(race: &Race, r: SkillDetailRef) -> i32 {
    let Some(Some(ab)) = detail(race, r).abilities.first() else { return 0 };
    ab.target.targets.iter().fold(0u32, |f, &h| f | 1u32 << (race.horses[h].horse_data.horse_index & 31)) as i32
}

pub fn detail_activate(race: &mut Race, r: SkillDetailRef) {
    detail_mut(race, r).is_activated = true;
    let t = value::ability_time(race, r);
    detail_mut(race, r).left_time = t;
    for k in 0..detail(race, r).abilities.len() {
        if detail(race, r).abilities[k].is_some() {
            ability::activate(race, r, k, false);
        }
    }
    record_skill_event(race, r);
    let det = detail(race, r);
    if det.is_not_activate_skill {
        return;
    }
    let i = r.horse;
    let m = &race.horses[i].skill_manager.skills[r.skill].skill_master;
    let id = m.id;
    let tags = tag_ids(m);
    let is_heal = det.abilities.iter().flatten().any(|ab| ab.ability_type == 9 && ab.ability_value_on_activate > 0.0);
    let course_distance = race.info.race_course_set.distance;
    let tag_group = &race.data.param.Skill.AbilityValueUsageTagGroup1Array;
    let specific = !tag_group.is_empty() && tags.iter().any(|t| tag_group.contains(t));
    let h = &mut race.horses[i];
    if !h.skill_manager.activate_skills.contains(&r) {
        h.skill_manager.activate_skills.push(r);
    }
    if !h.skill_manager.used_skill_ids.contains(&id) {
        h.skill_manager.used_skill_ids.push(id);
    }
    // Counted only for Start..LastSpurt.
    let phase = h.phase as usize;
    if phase < h.activate_skill_count_by_phase.len() {
        h.activate_skill_count_by_phase[phase] += 1;
    }
    let second_half = course_distance as f32 * 0.5 <= h.distance;
    h.activate_skill_count_by_distance[second_half as usize] += 1;
    if is_heal {
        h.activate_heal_skill_count += 1;
    }
    if specific {
        h.activate_specific_tag_group_skill_count += 1;
    }
}

/// At the gate (time <= 0) the target flags are 0 and the continue time is -1.
fn record_skill_event(race: &mut Race, r: SkillDetailRef) {
    let time = race.accumulate_time_since_start;
    let det = detail(race, r);
    let (flags, continue_time) = if 0.0 < time {
        // The continue time is computed in f64.
        (calc_target_flags(race, r), crate::sim::util::f64_to_i32(det.left_time as f64 * 10000.0))
    } else {
        (0, -1)
    };
    let (detail_index, activate_type) = (det.detail_index, det.is_not_activate_skill as i32);
    let id = skill_id(race, r);
    let horse_index = race.horses[r.horse].horse_data.horse_index as usize;
    record::add_skill_event(race, horse_index, id, detail_index, flags, time, continue_time, activate_type);
}

/// Returns true when the detail ended this frame.
pub fn detail_update_skill(race: &mut Race, r: SkillDetailRef, dt: f32) -> bool {
    if detail(race, r).is_activated {
        for k in 0..detail(race, r).abilities.len() {
            if detail(race, r).abilities[k].is_none() {
                continue;
            }
            let n = ability::calc_additional_activate_count(race, r, k);
            for _ in 0..n {
                ability::activate(race, r, k, true);
            }
        }
        if !detail(race, r).is_time_infinite {
            value::update_ability_time(race, r);
            let add = value::additional_ability_time(race, r);
            if 0.0 < add {
                let det = detail_mut(race, r);
                det.left_time = add + det.left_time;
                for k in 0..detail(race, r).abilities.len() {
                    if detail(race, r).abilities[k].is_some() {
                        ability::add_ability_time(race, r, k, add);
                    }
                }
            }
        }
    }
    let det = detail_mut(race, r);
    if det.is_time_infinite {
        return false;
    }
    det.left_time -= dt;
    if 0.0 < det.left_time {
        return false;
    }
    detail_stop(race, r);
    true
}

pub fn detail_stop(race: &mut Race, r: SkillDetailRef) {
    if !detail(race, r).is_activated {
        return;
    }
    detail_mut(race, r).is_activated = false;
    for k in 0..detail(race, r).abilities.len() {
        if detail(race, r).abilities[k].is_some() {
            ability::stop(race, r, k);
        }
    }
    value::time_calculator_init(race, r);
    let l = &mut race.horses[r.horse].skill_manager.activate_skills;
    if let Some(k) = l.iter().position(|x| *x == r) {
        l.remove(k);
    }
    let det = detail_mut(race, r);
    det.cool_down_time = det.default_cool_down_time;
}

/// Whether this is detail 0 of a skill reserved for random activation.
pub fn detail_is_reserved_random_activate(race: &Race, r: SkillDetailRef) -> bool {
    r.detail == 0 && race.horses[r.horse].skill_manager.reserved_skill_ids.contains(&skill_id(race, r))
}

/// Re-draws the skills reserved for random activation (ability types 36/37, unused in current data): per modifier,
/// up to n eligible skills drawn from the race RNG.
pub fn update_random_activate_skill_id_list(race: &mut Race, i: usize) {
    use crate::sim::defs::SkillModifierParam as P;
    race.horses[i].skill_manager.reserved_skill_ids.clear();
    for (param, rarities) in [(P::ActivateRandomRareSkill, &[2, 6][..]), (P::ActivateRandomNormalAndRareSkill, &[1, 2, 6][..])] {
        let n = crate::sim::util::f32_to_i32(race.horses[i].skill_modifier_receiver.apply_modifier(param, 0.0));
        if n <= 0 {
            continue;
        }
        let sm = &race.horses[i].skill_manager;
        let mut cand: Vec<i32> = sm
            .skills
            .iter()
            .filter(|s| {
                let m = &s.skill_master;
                rarities.contains(&m.rarity)
                    && m.details[0].float_ability_time != -1
                    && s.cool_down_time <= 0.0
                    && m.is_general_skill == 1
                    && sm.activate_skills.iter().all(|d| race.horses[d.horse].skill_manager.skills[d.skill].skill_master.id != m.id)
                    && !sm.reserved_skill_ids.contains(&m.id)
            })
            .map(|s| s.skill_master.id)
            .collect();
        for _ in 0..n.min(cand.len() as i32) {
            let k = race.rng.random.next_range(0, cand.len() as i32) as usize;
            race.horses[i].skill_manager.reserved_skill_ids.push(cand.remove(k));
        }
    }
}
