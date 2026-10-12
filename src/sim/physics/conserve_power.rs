//! Conserve power: power stored while running relaxed in the early race, released as an accel boost at the start
//! of the last spurt.

use crate::api::event_kind;
use crate::sim::ai::position_keep;
use crate::sim::defs::{PositionKeepMode, SkillModifierParam};
use crate::sim::physics::last_spurt;
use crate::sim::race::Race;
use crate::sim::record;

#[derive(Clone, Copy, Debug, Default)]
pub struct ConservePower {
    /// `PowerStat > 1200` at creation; otherwise the calculator stays inert.
    pub is_active: bool,
    pub power_stat: f32,
    pub conserved_power: f32,
    pub cool_down: f32,
    /// Time towards the next whole second of accrual.
    pub bank_timer: f32,
    pub is_temptation_prev: bool,
    /// Release activity coefficient (shrunk by spot struggles and rushing).
    pub activity_coef: f32,
    pub is_released: bool,
    /// Accel add while the boost lasts (read by `speed::calc_accel_per_sec`).
    pub accel_value: f32,
    pub remain_time: f32,
}

/// Mode/style-keyed lookup in the conserve-power tables (1.0 when no row matches).
fn coef_by<T>(a: &[T], f: impl Fn(&T) -> bool, c: impl Fn(&T) -> f32) -> f32 {
    a.iter().find(|x| f(x)).map_or(1.0, c)
}

/// Entering a spot struggle shrinks the horse's conserved power and its release activity coefficient.
pub fn on_compete_top_start(race: &mut Race, i: usize) {
    let cp = &race.data.param.ConservePower;
    let c = &mut race.horses[i].ai.conserve_power;
    c.conserved_power *= coef_by(&cp.ConservePowerDecreaseCoefArray, |x| x.Mode == 3, |x| x.Coef);
    c.activity_coef *= coef_by(&cp.ConservePowerDecreaseAccelCoefArray, |x| x.Mode == 3, |x| x.Coef);
}

/// Creates the calculator after the gate skills. `PowerStat = base Power + 0.5 * max(raw Power - 1200, 0) + skill
/// Power`; only horses above 1200 conserve power.
pub fn new(race: &Race, i: usize) -> ConservePower {
    let h = &race.horses[i];
    let rp = h.horse_data.race_param;
    let skill_add = h.skill_modifier_receiver.apply_modifier(SkillModifierParam::Power, 0.0);
    let power_stat = rp.base_pow + 0.5 * (rp.raw_pow - 1200).max(0) as f32 + skill_add;
    // From the July 2026 update on.
    let is_active = power_stat > 1200.0 && !race.data.era.pre_2026_07_courses;
    ConservePower { is_active, power_stat, activity_coef: 1.0, ..Default::default() }
}

/// Conserve power update (AI update step 9, not when finished).
/// - Accrues in whole seconds: every eligible frame before the position-keep end distance adds `dt` to a timer; each
///   time it reaches 1 s, 1 s is taken off (the rest carries over) and `IncreaseCoef[PaceDown ? 1 : 2] *
///   RunningStyleCoef` is added, by the mode on that frame.
/// - A frame in another position-keep mode, rushed or in a spot struggle is blocked: it clears the timer and restarts
///   a 1.5 s cooldown, which counts down on the following unblocked frames; the frame where it runs out is eligible.
///   Entering rushed shrinks the power and the activity coefficient.
/// - Releases on the first last-spurt frame (event 6): `Duration = Power * sqrt(CourseDistance) * DistanceTypeCoef /
///   1450` (none if < 0.2 s), `Accel = sqrt((PowerStat - 1200) * 130) * 0.001 * StyleDistanceCoef * Activity`. The
///   accel applies from this frame and lasts one frame longer than the duration.
pub fn update(race: &mut Race, i: usize, dt: f32) {
    let cp = &race.data.param.ConservePower;
    let mut c = race.horses[i].ai.conserve_power;
    if !c.is_active {
        return;
    }
    if !c.is_released {
        if last_spurt::is_last_spurt(race, i) {
            c.is_released = true;
            let dtype = race.info.course_distance_type;
            let dcoef = coef_by(&cp.ReleaseConservePowerActivityTimeDistanceTypeCoefArray, |x| x.DistanceType == dtype, |x| x.Coef);
            let dist = race.info.race_course_set.distance as f32;
            let duration = c.conserved_power * dist.sqrt() * dcoef / cp.ReleaseConservePowerActivityTimeCoef;
            if duration >= cp.ConservePowerReleasableActivityTimeThreshold {
                let style = race.horses[i].horse_data.running_style as i32;
                let scoef = coef_by(&cp.ReleaseConservePowerAccelRunningStyleAndDistanceTypeCoefArray,
                    |x| x.RunningStyle == style && x.DistanceType == dtype, |x| x.Coef);
                c.accel_value = ((c.power_stat - 1200.0) * cp.ReleaseConservePowerDeclCoef).sqrt() * cp.ReleaseConservePowerAccelCoef * scoef * c.activity_coef;
                c.remain_time = duration + dt;
                let t = race.recorder.cur_time;
                record::add_horse_event(race, event_kind::CONSERVE_POWER_RELEASE, race.horses[i].horse_data.horse_index as usize, t);
            }
        } else {
            let mode = position_keep::position_keep_mode(race, i);
            let tempt = race.horses[i].ai.temptation.is_temptation;
            // Entering rushed (decrease mode 4; the spot struggle's mode 3 is `on_compete_top_start`).
            if tempt && !c.is_temptation_prev {
                c.conserved_power *= coef_by(&cp.ConservePowerDecreaseCoefArray, |x| x.Mode == 4, |x| x.Coef);
                c.activity_coef *= coef_by(&cp.ConservePowerDecreaseAccelCoefArray, |x| x.Mode == 4, |x| x.Coef);
            }
            c.is_temptation_prev = tempt;
            let blocked = tempt || race.horses[i].is_compete_top || !matches!(mode, PositionKeepMode::Null | PositionKeepMode::PaceDown);
            if blocked {
                c.cool_down = cp.ConservePowerDefaultCoolDownTime;
                c.bank_timer = 0.0;
            } else {
                if c.cool_down > 0.0 {
                    c.cool_down -= dt;
                }
                if c.cool_down > 0.0 || race.horses[i].distance >= race.horses[i].ai.position_keep.check_end_distance {
                    race.horses[i].ai.conserve_power = c;
                    return;
                }
                c.bank_timer += dt;
                if c.bank_timer >= 1.0 {
                    c.bank_timer -= 1.0;
                    // Increase mode: 1 = pace down, 2 = no position keep.
                    let m = if mode == PositionKeepMode::PaceDown { 1 } else { 2 };
                    let style = race.horses[i].horse_data.running_style as i32;
                    let k = coef_by(&cp.ConservePowerIncreaseCoefArray, |x| x.Mode == m, |x| x.Coef);
                    let s = coef_by(&cp.ConservePowerRunningStyleCoefArray, |x| x.RunningStyle == style, |x| x.Coef);
                    c.conserved_power += k * s;
                }
            }
        }
    }
    if c.remain_time > 0.0 {
        c.remain_time -= dt;
        if c.remain_time <= 0.0 {
            c.accel_value = 0.0;
        }
    }
    race.horses[i].ai.conserve_power = c;
}
