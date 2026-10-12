//! Position keep: the early-race speed-up / overtake / pace-up / pace-down modes, how they are chosen and ended.
//!
//! RNG: SpeedUp / Overtake / PaceUp starts draw a wiz roll `GetRandom(100f)` only when not rushed; PaceUp draws
//! `GetRandom(min, max)` on success; PaceDown draws `GetRandom(min, max')` when it starts.

use crate::sim::defs::{Phase, PositionKeepMode, RunningStyle, RunningStyleEx, SkillModifierParam, TemptationMode};
use crate::sim::field::{horse_at_order, running_style_horses};
use crate::sim::race::Race;
use crate::sim::physics::stats;
use crate::sim::util::clamp01;
use crate::data::param::PositionKeep as PositionKeepParams;

/// Chooses which mode to try.
#[derive(Clone, Copy, Debug)]
pub enum KeepRule {
    /// Front runners (and rushed horses that run as one).
    Nige { is_force_speed_up: bool },
    /// Everyone else: keep a distance band behind the pacemaker.
    PaceAdjust { course_distance: i32, running_style: RunningStyle },
}

impl Default for KeepRule {
    fn default() -> Self {
        KeepRule::Nige { is_force_speed_up: false }
    }
}

/// Shared state of the pace modes (PaceUp / PaceDown / PaceUpEx).
#[derive(Clone, Copy, Debug, Default)]
pub struct PaceState {
    pub course_distance: i32,
    pub distance_diff_min: f32,
    pub distance_diff_max: f32,
    pub pace_adjust_target_distance_diff: f32,
    pub running_style: RunningStyle,
}

#[derive(Clone, Copy, Debug, Default)]
pub enum PositionKeepStrategyKind {
    #[default]
    Null,
    SpeedUp,
    Overtake,
    PaceUp(PaceState),
    PaceDown(PaceState),
    PaceUpEx(PaceState),
}

/// One position-keep mode: how long it lasts and its target-speed multiplier.
#[derive(Clone, Copy, Debug, Default)]
pub struct PositionKeepStrategy {
    pub continue_distance: f32,
    pub end_distance: f32,
    pub base_target_speed_multiply: f32,
    pub kind: PositionKeepStrategyKind,
}

impl PositionKeepStrategy {
    pub fn mode(&self) -> PositionKeepMode {
        match self.kind {
            PositionKeepStrategyKind::Null => PositionKeepMode::Null,
            PositionKeepStrategyKind::SpeedUp => PositionKeepMode::SpeedUp,
            PositionKeepStrategyKind::Overtake => PositionKeepMode::Overtake,
            PositionKeepStrategyKind::PaceUp(_) => PositionKeepMode::PaceUp,
            PositionKeepStrategyKind::PaceDown(_) => PositionKeepMode::PaceDown,
            PositionKeepStrategyKind::PaceUpEx(_) => PositionKeepMode::PaceUpEx,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct PositionKeep {
    pub check_start_distance: f32,
    pub check_end_distance: f32,
    pub course_distance: i32,
    pub keep_rule: KeepRule,
    pub is_temptation_prev: bool,
    /// The current mode, as an index into `strategies`.
    pub cur_strategy: usize,
    /// Indexed by `PositionKeepMode`.
    pub strategies: Vec<PositionKeepStrategy>,
    pub cool_down_sec: f32,
    pub check_start_interval_sec: f32,
}

/// Builds horse `i`'s calculator: the check window and every mode's duration and multiplier. No RNG.
pub fn new_position_keep(race: &mut Race, i: usize) -> PositionKeep {
    let data = race.data;
    let kp = &data.param.PositionKeep;
    let h = &race.horses[i];
    let course_distance = race.info.race_course_set.distance;
    let sec_len = course_distance / 24;
    let ex = h.horse_data.running_style_ex;
    let style = h.horse_data.running_style;
    let sections = if ex == RunningStyleEx::None {
        kp.PositionKeepContinueSection
    } else {
        kp.PositionKeepContinueSectionExArray.get(ex as usize - 1).copied().unwrap_or(0)
    };
    let continue_distance = (sec_len.wrapping_mul(sections)) as f32;
    let oonige = ex == RunningStyleEx::Oonige;
    let pace = PaceState { course_distance, running_style: style, ..Default::default() };
    let strategy = |mult: f32, kind| PositionKeepStrategy { continue_distance, end_distance: 0.0, base_target_speed_multiply: mult, kind };
    let strategies = vec![
        // Null: its multiplier is never read.
        PositionKeepStrategy::default(),
        strategy(
            if oonige { kp.PositionKeepSpeedUpBaseTargetSpeedMultiplyOonige } else { kp.PositionKeepSpeedUpBaseTargetSpeedMultiply },
            PositionKeepStrategyKind::SpeedUp,
        ),
        strategy(
            if oonige { kp.PositionKeepOvertakeBaseTargetSpeedMultiplyOonige } else { kp.PositionKeepOvertakeBaseTargetSpeedMultiply },
            PositionKeepStrategyKind::Overtake,
        ),
        strategy(kp.PositionKeepPaceUpBaseTargetSpeedMultiply, PositionKeepStrategyKind::PaceUp(pace)),
        strategy(
            if h.phase == Phase::MiddleRun {
                kp.PositionKeepPaceDownBaseTargetSpeedMultiplyPhaseMiddle
            } else {
                kp.PositionKeepPaceDownBaseTargetSpeedMultiply
            },
            PositionKeepStrategyKind::PaceDown(pace),
        ),
        strategy(kp.PositionKeepPaceUpExBaseTargetSpeedMultiply, PositionKeepStrategyKind::PaceUpEx(pace)),
    ];
    PositionKeep {
        check_start_distance: (kp.PositionKeepStartSection - 1) as f32 * sec_len as f32,
        check_end_distance: kp.PositionKeepEndSection as f32 * sec_len as f32,
        course_distance,
        keep_rule: new_keep_rule(style, false, course_distance),
        is_temptation_prev: false,
        cur_strategy: 0,
        strategies,
        cool_down_sec: 0.0,
        check_start_interval_sec: 0.0,
    }
}

/// The mode calculator for a running style; a horse rushed into running as a Nige is forced into SpeedUp.
fn new_keep_rule(style: RunningStyle, is_temptation: bool, course_distance: i32) -> KeepRule {
    if style == RunningStyle::Nige {
        KeepRule::Nige { is_force_speed_up: is_temptation }
    } else {
        KeepRule::PaceAdjust { course_distance, running_style: style }
    }
}

/// Reacts to a rush starting or ending, then either tries to start a mode or checks whether the current one ends.
pub fn update(race: &mut Race, i: usize, dt: f32) {
    check_temptation(race, i);
    let pk = &race.horses[i].ai.position_keep;
    if pk.strategies[pk.cur_strategy].mode() == PositionKeepMode::Null {
        check_start(race, i, dt);
        return;
    }
    let idx = pk.cur_strategy;
    if check_end(race, i, idx) {
        clear(&mut race.horses[i].ai.position_keep, race.data.param.PositionKeep.PositionKeepCoolDownSec);
    }
}

/// Back to no mode, with a cooldown before the next start.
fn clear(pk: &mut PositionKeep, cool_down_sec: i32) {
    pk.cur_strategy = 0;
    pk.cool_down_sec = cool_down_sec as f32;
}

/// When the rush starts or ends: reset the timers and switch to the running style the rush imposes (or back).
fn check_temptation(race: &mut Race, i: usize) {
    let data = race.data;
    let kp = &data.param.PositionKeep;
    let h = &mut race.horses[i];
    let t = h.ai.temptation.is_temptation;
    let pk = &mut h.ai.position_keep;
    if t == pk.is_temptation_prev {
        return;
    }
    pk.is_temptation_prev = t;
    clear(pk, kp.PositionKeepCoolDownSec);
    pk.cool_down_sec = 0.0;
    pk.check_start_interval_sec = 0.0;
    let style = if !t {
        h.horse_data.running_style
    } else {
        match h.ai.temptation.mode {
            TemptationMode::PositionSashi => RunningStyle::Sashi,
            TemptationMode::PositionSenko => RunningStyle::Senko,
            _ => RunningStyle::Nige,
        }
    };
    let pk = &mut h.ai.position_keep;
    pk.keep_rule = new_keep_rule(style, t, pk.course_distance);
    // Only the pace modes store the style.
    for s in &mut pk.strategies {
        if let PositionKeepStrategyKind::PaceUp(ps) | PositionKeepStrategyKind::PaceDown(ps) | PositionKeepStrategyKind::PaceUpEx(ps) = &mut s.kind {
            ps.running_style = style;
        }
    }
}

/// Inside the check window, after the cooldown and check interval: pick a mode and try to start it.
fn check_start(race: &mut Race, i: usize, dt: f32) {
    let interval = race.data.param.PositionKeep.PositionKeepCheckIntervalSec;
    let h = &mut race.horses[i];
    let d = h.distance;
    let pk = &mut h.ai.position_keep;
    if d < pk.check_start_distance || pk.check_end_distance <= d {
        return;
    }
    if pk.cool_down_sec > 0.0 {
        pk.cool_down_sec -= dt;
    }
    if pk.cool_down_sec > 0.0 {
        return;
    }
    if pk.check_start_interval_sec > 0.0 {
        pk.check_start_interval_sec -= dt;
    }
    if pk.check_start_interval_sec > 0.0 {
        return;
    }
    pk.check_start_interval_sec = interval as f32;
    let mode = calc_keep_mode(race, i) as usize;
    let idx = if mode < 6 { mode } else { 0 };
    let with_wiz_roll = !race.horses[i].ai.temptation.is_temptation;
    if try_start(race, i, idx, with_wiz_roll) {
        race.horses[i].ai.position_keep.cur_strategy = idx;
        race.horses[i].position_keep_count += 1;
    }
}

fn pacemaker(race: &mut Race) -> usize {
    race.field.pacemaker.current()
}

/// Whether any horse ahead of `cur_order` has a more backward running style than `style`.
fn is_exist_backward_running_style(race: &Race, cur_order: i32, style: RunningStyle) -> bool {
    (0..cur_order).rev().any(|o| {
        let h = horse_at_order(race, o).expect("a horse at every order ahead");
        race.horses[h].horse_data.running_style > style
    })
}

/// The (min, max) distance band behind the pacemaker for a style, widened on long courses.
fn calc_offset_distance(kp: &PositionKeepParams, course_distance: i32, style: RunningStyle) -> (f32, f32) {
    // Evaluated in f64 as one expression.
    let mut coef = ((course_distance - kp.PositionKeepPaceBaseDistance) as f32 as f64 * kp.PositionKeepPaceBaseDistanceCoef as f64 + 1.0) as f32;
    if 1.0 > coef {
        coef = 1.0;
    }
    match style {
        RunningStyle::Senko => (kp.PositionKeepPaceUpDownDistanceDiffMinSenko, coef * kp.PositionKeepPaceUpDownDistanceDiffMaxSenko),
        RunningStyle::Sashi => (coef * kp.PositionKeepPaceUpDownDistanceDiffMinSashi, coef * kp.PositionKeepPaceUpDownDistanceDiffMaxSashi),
        RunningStyle::Oikomi => (coef * kp.PositionKeepPaceUpDownDistanceDiffMinOikomi, coef * kp.PositionKeepPaceUpDownDistanceDiffMaxOikomi),
        _ => (0.0, 0.0),
    }
}

fn calc_keep_mode(race: &mut Race, i: usize) -> PositionKeepMode {
    match race.horses[i].ai.position_keep.keep_rule {
        KeepRule::Nige { is_force_speed_up } => {
            if is_force_speed_up {
                return PositionKeepMode::SpeedUp;
            }
            let h = &race.horses[i];
            // No PaceUpEx before the July 2026 update.
            if !race.data.era.pre_2026_07_courses && is_exist_backward_running_style(race, h.cur_order, h.horse_data.running_style) {
                return PositionKeepMode::PaceUpEx;
            }
            // SpeedUp when first overall or the first Nige.
            if h.cur_order == 0 || race.field.running_style_top_order_horses[RunningStyle::Nige as usize] == Some(i) {
                PositionKeepMode::SpeedUp
            } else {
                PositionKeepMode::Overtake
            }
        }
        KeepRule::PaceAdjust { course_distance, running_style } => {
            let pm = pacemaker(race);
            let (me, pm_horse) = (&race.horses[i], &race.horses[pm]);
            // No PaceUpEx before the July 2026 update (as in the Nige branch).
            if !race.data.era.pre_2026_07_courses && pm_horse.cur_order < me.cur_order && me.horse_data.running_style < pm_horse.horse_data.running_style {
                return PositionKeepMode::PaceUpEx;
            }
            let (min, max) = calc_offset_distance(&race.data.param.PositionKeep, course_distance, running_style);
            let gap = pm_horse.distance - me.distance;
            if gap < min {
                if pm != i {
                    PositionKeepMode::PaceDown
                } else {
                    PositionKeepMode::Null
                }
            } else if !(gap > max) {
                PositionKeepMode::Null
            } else {
                PositionKeepMode::PaceUp
            }
        }
    }
}

/// Wiz roll: one `GetRandom(100f)`, success iff `r < log10(Wiz * 0.1) * val1`.
fn wiz_roll(race: &mut Race, i: usize, val1: f32) -> bool {
    let r = race.rng.random_float(100.0);
    let per = crate::sim::util::mathf_log10(stats::wiz(&race.horses[i]) * 0.1) * val1;
    !(per <= r)
}

/// How far ahead of the horse behind a SpeedUp ends.
fn speed_up_end_distance_diff(race: &mut Race, i: usize) -> f32 {
    let data = race.data;
    let kp = &data.param.PositionKeep;
    if race.horses[i].horse_data.running_style_ex == RunningStyleEx::Oonige {
        return kp.PositionKeepSpeedUpEndDistanceDiffOonige;
    }
    if pacemaker(race) == i && race.horses[i].horse_data.running_style != RunningStyle::Nige {
        return kp.PositionKeepSpeedUpEndDistanceDiff;
    }
    let n = running_style_horses(race, RunningStyle::Nige)
        .iter()
        .filter(|&&h| race.horses[h].horse_data.running_style_ex == RunningStyleEx::None)
        .count();
    // The single-Nige distance only from the July 2026 update on.
    if n == 1 && !data.era.pre_2026_07_courses {
        kp.PositionKeepSpeedUpEndDistanceDiffOne
    } else {
        kp.PositionKeepSpeedUpEndDistanceDiff
    }
}

/// Whether the horse is already far enough ahead of the horse behind it.
fn check_distance_diff_behind(race: &mut Race, i: usize) -> bool {
    // False when there is no horse behind, so the last horse can still speed up.
    let Some(b) = horse_at_order(race, race.horses[i].cur_order + 1) else {
        return false;
    };
    let gap = race.horses[i].distance - race.horses[b].distance;
    speed_up_end_distance_diff(race, i) <= gap
}

/// How far the pacemaker is ahead.
fn pacemaker_distance_diff(race: &mut Race, i: usize) -> f32 {
    let pm = pacemaker(race);
    race.horses[pm].distance - race.horses[i].distance
}

fn has_speed_up_skill_value(race: &Race, i: usize) -> bool {
    let m = &race.horses[i].skill_modifier_receiver;
    m.apply_modifier(SkillModifierParam::CurrentSpeed, 0.0) > 0.0 || m.apply_modifier(SkillModifierParam::TargetSpeed, 0.0) > 0.0
}

/// Tries to start mode `idx`, with the wiz roll only when not rushed. All of position keep's RNG happens here.
fn try_start(race: &mut Race, i: usize, idx: usize, with_wiz_roll: bool) -> bool {
    let data = race.data;
    let kp = &data.param.PositionKeep;
    let dist = race.horses[i].distance;
    let oonige = race.horses[i].horse_data.running_style_ex == RunningStyleEx::Oonige;
    let s = race.horses[i].ai.position_keep.strategies[idx];
    let end = dist + s.continue_distance;
    let mut new = s;
    match s.kind {
        PositionKeepStrategyKind::Null => return false,
        PositionKeepStrategyKind::SpeedUp => {
            if check_distance_diff_behind(race, i) {
                return false;
            }
            let val1 = if oonige { kp.PositionKeepSpeedUpStartPerVal1Oonige } else { kp.PositionKeepSpeedUpStartPerVal1 };
            if with_wiz_roll && !wiz_roll(race, i, val1) {
                return false;
            }
            new.end_distance = end;
        }
        PositionKeepStrategyKind::Overtake => {
            let val1 = if oonige { kp.PositionKeepOvertakeStartPerVal1Oonige } else { kp.PositionKeepOvertakeStartPerVal1 };
            if with_wiz_roll && !wiz_roll(race, i, val1) {
                return false;
            }
            new.end_distance = end;
        }
        PositionKeepStrategyKind::PaceUp(mut ps) => {
            if with_wiz_roll && !wiz_roll(race, i, kp.PositionKeepPaceUpStartPerVal1) {
                return false;
            }
            new.end_distance = end;
            let (min, max) = calc_offset_distance(kp, ps.course_distance, ps.running_style);
            ps.distance_diff_min = min;
            ps.distance_diff_max = max;
            ps.pace_adjust_target_distance_diff = race.rng.random_float_range(min, max);
            new.kind = PositionKeepStrategyKind::PaceUp(ps);
        }
        PositionKeepStrategyKind::PaceDown(mut ps) => {
            if has_speed_up_skill_value(race, i) {
                return false;
            }
            new.end_distance = end;
            let mid = race.horses[i].phase == Phase::MiddleRun;
            let (min, mut max) = calc_offset_distance(kp, ps.course_distance, ps.running_style);
            // The middle-phase band and multiplier only from the July 2026 update on.
            let post_july = !data.era.pre_2026_07_courses;
            if mid && post_july {
                // Evaluated in f64 as one expression.
                max = ((max as f64 - min as f64) * clamp01(kp.PositionKeepPaceUpDownDistanceDiffMaxCoef) as f64 + min as f64) as f32;
            }
            ps.distance_diff_min = min;
            ps.distance_diff_max = max;
            ps.pace_adjust_target_distance_diff = race.rng.random_float_range(min, max);
            new.base_target_speed_multiply = if mid && post_july {
                kp.PositionKeepPaceDownBaseTargetSpeedMultiplyPhaseMiddle
            } else {
                kp.PositionKeepPaceDownBaseTargetSpeedMultiply
            };
            new.kind = PositionKeepStrategyKind::PaceDown(ps);
        }
        PositionKeepStrategyKind::PaceUpEx(_) => new.end_distance = end,
    }
    race.horses[i].ai.position_keep.strategies[idx] = new;
    true
}

fn check_end(race: &mut Race, i: usize, idx: usize) -> bool {
    let s = race.horses[i].ai.position_keep.strategies[idx];
    let data = race.data;
    let kp = &data.param.PositionKeep;
    let dist = race.horses[i].distance;
    match s.kind {
        PositionKeepStrategyKind::Null => true,
        PositionKeepStrategyKind::SpeedUp => check_distance_diff_behind(race, i) || dist >= s.end_distance,
        PositionKeepStrategyKind::Overtake => {
            if s.end_distance <= dist {
                return true;
            }
            // Also ends once first overall and the end distance clear of second place.
            if race.horses[i].cur_order > 0 {
                return false;
            }
            let Some(second) = horse_at_order(race, 1) else {
                return true;
            };
            let oonige = race.horses[i].horse_data.running_style_ex == RunningStyleEx::Oonige;
            let diff = if oonige {
                kp.PositionKeepOvertakeEndDistanceDiffOonige
            } else {
                kp.PositionKeepOvertakeEndDistanceDiff
            };
            !(diff > dist - race.horses[second].distance)
        }
        PositionKeepStrategyKind::PaceUp(ps) => {
            if dist >= s.end_distance {
                return true;
            }
            pacemaker_distance_diff(race, i) <= ps.pace_adjust_target_distance_diff
        }
        PositionKeepStrategyKind::PaceDown(ps) => {
            // Before the July 2026 update there is no exit when this horse becomes the pacemaker.
            if !race.data.era.pre_2026_07_courses && race.field.pacemaker.horse == Some(i) {
                return true;
            }
            if has_speed_up_skill_value(race, i) || s.end_distance <= dist {
                return true;
            }
            pacemaker_distance_diff(race, i) >= ps.pace_adjust_target_distance_diff
        }
        PositionKeepStrategyKind::PaceUpEx(_) => {
            if s.end_distance <= dist {
                return true;
            }
            let h = &race.horses[i];
            !is_exist_backward_running_style(race, h.cur_order, h.horse_data.running_style)
        }
    }
}

pub fn is_position_keep(race: &Race, i: usize) -> bool {
    position_keep_mode(race, i) != PositionKeepMode::Null
}

pub fn position_keep_mode(race: &Race, i: usize) -> PositionKeepMode {
    let pk = &race.horses[i].ai.position_keep;
    pk.strategies.get(pk.cur_strategy).map_or(PositionKeepMode::Null, |s| s.mode())
}

pub fn position_keep_base_target_speed_multiply(race: &Race, i: usize) -> f32 {
    let pk = &race.horses[i].ai.position_keep;
    pk.strategies[pk.cur_strategy].base_target_speed_multiply
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::ai::test_race;

    /// Horse 0 = pacemaker (Nige) at 300 m, horse 1 = Sashi `gap` metres behind; 2000 m course.
    fn setup(gap: f32, seed: i32) -> Race<'static> {
        let mut race = test_race(2);
        race.rng = crate::rng::RaceRandom::new(seed);
        race.info.race_course_set.distance = 2000;
        race.horses[0].horse_data.running_style = RunningStyle::Nige;
        race.horses[1].horse_data.running_style = RunningStyle::Sashi;
        race.horses[0].distance = 300.0;
        race.horses[1].distance = 300.0 - gap;
        race.horses[1].cur_order = 1;
        for i in 0..2 {
            race.horses[i].base_wiz_adjusted = 400.0;
            race.horses[i].ai.position_keep = new_position_keep(&mut race, i);
        }
        race
    }

    #[test]
    fn ctor_and_offsets() {
        let race = setup(20.0, 1);
        let pk = &race.horses[1].ai.position_keep;
        assert_eq!(pk.check_start_distance, 0.0);
        assert_eq!(pk.check_end_distance, 830.0); // 10 * (2000 / 24 = 83)
        assert_eq!(pk.strategies[3].continue_distance, 83.0);
        let kp = &race.data.param.PositionKeep;
        let (min, max) = calc_offset_distance(kp, 2000, RunningStyle::Sashi);
        assert_eq!((min, max), (1.8 * kp.PositionKeepPaceUpDownDistanceDiffMinSashi, 1.8 * kp.PositionKeepPaceUpDownDistanceDiffMaxSashi));
        assert_eq!(calc_offset_distance(kp, 1000, RunningStyle::Senko).0, kp.PositionKeepPaceUpDownDistanceDiffMinSenko);
    }

    #[test]
    fn pace_up_draws() {
        let (mut ok, mut ng) = (0, 0);
        for seed in 0..100 {
            let mut race = setup(20.0, seed);
            assert_eq!(calc_keep_mode(&mut race, 1), PositionKeepMode::PaceUp);
            let started = try_start(&mut race, 1, PositionKeepMode::PaceUp as usize, true);
            assert_eq!(race.rng.draws(), if started { 2 } else { 1 });
            if started { ok += 1 } else { ng += 1 }
        }
        assert!(ok > 0 && ng > 0);
        // Rushed (no lot): no wiz roll, always starts, one range draw.
        let mut race = setup(20.0, 7);
        assert!(try_start(&mut race, 1, PositionKeepMode::PaceUp as usize, false));
        assert_eq!(race.rng.draws(), 1);
        let PositionKeepStrategyKind::PaceUp(ps) = race.horses[1].ai.position_keep.strategies[3].kind else { panic!() };
        assert!(ps.pace_adjust_target_distance_diff >= ps.distance_diff_min && ps.pace_adjust_target_distance_diff <= ps.distance_diff_max);
    }

    #[test]
    fn first_nige_behind_a_leader_speeds_up() {
        // Before the July 2026 update: a Nige behind a Sashi leader is still the first Nige → SpeedUp, not Overtake.
        let mut race = setup(2.0, 3);
        let s = std::fs::read_to_string(format!("{}/race_paramdefine.json", crate::data::DATA_DIR)).unwrap();
        let era = crate::data::DataEra { skill_snapshot: None, pre_2026_07_courses: true };
        race.data = Box::leak(Box::new(crate::data::GameData { param: serde_json::from_str(&s).unwrap(), era, ..Default::default() }));
        race.horses[0].horse_data.running_style = RunningStyle::Sashi;
        race.horses[1].horse_data.running_style = RunningStyle::Nige;
        for i in 0..2 {
            race.horses[i].ai.position_keep = new_position_keep(&mut race, i);
        }
        crate::sim::field::cache_same_running_style_horses(&mut race);
        crate::sim::field::update_running_style_first_and_last_order_horses(&mut race);
        assert_eq!(calc_keep_mode(&mut race, 1), PositionKeepMode::SpeedUp);
        race.horses.push(race.horses[1].clone()); // a second Nige, 3rd overall → Overtake
        race.horses[2].cur_order = 2;
        crate::sim::field::cache_same_running_style_horses(&mut race);
        crate::sim::field::update_running_style_first_and_last_order_horses(&mut race);
        assert_eq!(calc_keep_mode(&mut race, 2), PositionKeepMode::Overtake);
    }

    #[test]
    fn speed_up_no_draw_when_gap_large() {
        let mut race = setup(20.0, 3);
        update(&mut race, 0, 0.0666); // 20 m >= 4.5 m: already far enough ahead → no draw
        assert_eq!(race.rng.draws(), 0);
        assert_eq!(race.horses[0].ai.position_keep.check_start_interval_sec, 2.0);
    }

    #[test]
    fn modes_and_check_start() {
        // Too close → PaceDown (one range draw, no wiz roll); the pacemaker itself never paces down.
        let mut race = setup(2.0, 3);
        assert_eq!(calc_keep_mode(&mut race, 1), PositionKeepMode::PaceDown);
        // Nige leader → SpeedUp; horse 1 is 2 m behind (< 4.5 m) → the wiz roll is drawn.
        assert_eq!(calc_keep_mode(&mut race, 0), PositionKeepMode::SpeedUp);
        update(&mut race, 0, 0.0666);
        assert_eq!(race.rng.draws(), 1);
        assert_eq!(race.horses[0].ai.position_keep.check_start_interval_sec, 2.0);
        update(&mut race, 1, 0.0666);
        assert_eq!(race.rng.draws(), 2);
        assert_eq!(position_keep_mode(&race, 1), PositionKeepMode::PaceDown);
        assert_eq!(race.horses[1].position_keep_count, 1);
        // Gap reaches the target → mode ends, 1 s cooldown.
        race.horses[1].distance = 250.0;
        update(&mut race, 1, 0.0666);
        assert_eq!(position_keep_mode(&race, 1), PositionKeepMode::Null);
        assert_eq!(race.horses[1].ai.position_keep.cool_down_sec, 1.0);
    }
    #[test]
    fn temptation_switches_calculator() {
        let mut race = setup(20.0, 5);
        let pk = &mut race.horses[1].ai.position_keep;
        pk.cool_down_sec = 0.5;
        pk.check_start_interval_sec = 1.0;
        // Rushed Sashi mapped to Nige: forced SpeedUp, timers zeroed → checked this frame, no wiz roll.
        race.horses[1].ai.temptation.is_temptation = true;
        race.horses[1].ai.temptation.mode = TemptationMode::PositionNige;
        check_temptation(&mut race, 1);
        let pk = &race.horses[1].ai.position_keep;
        assert!(matches!(pk.keep_rule, KeepRule::Nige { is_force_speed_up: true }));
        assert_eq!((pk.cool_down_sec, pk.check_start_interval_sec), (0.0, 0.0));
        let PositionKeepStrategyKind::PaceUp(ps) = pk.strategies[3].kind else { panic!() };
        assert_eq!(ps.running_style, RunningStyle::Nige);
        assert_eq!(calc_keep_mode(&mut race, 1), PositionKeepMode::SpeedUp);
        // Rush ends: back to the real style's PaceAdjust calculator.
        race.horses[1].ai.temptation.is_temptation = false;
        check_temptation(&mut race, 1);
        let pk = &race.horses[1].ai.position_keep;
        assert!(matches!(pk.keep_rule, KeepRule::PaceAdjust { running_style: RunningStyle::Sashi, .. }));
        assert_eq!(race.rng.draws(), 0);
    }
}
