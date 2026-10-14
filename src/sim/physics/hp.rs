//! HP: skill heal / drain, consumption, HP needed to reach the goal.

use crate::sim::defs::{Phase, PositionKeepMode, RunningStyleEx, SkillModifierParam};
use crate::sim::race::Race;
use crate::sim::util::{approximately, clamp, mathf_pow};
use crate::sim::ai::position_keep;
use crate::sim::physics::stats;
use crate::data::param::Hp;

/// Applies this frame's skill heal / drain. A heal from the End phase on requests a last-spurt recompute.
pub fn update_hp_by_skill(race: &mut Race, i: usize) {
    let h = &mut race.horses[i];
    let v = h.skill_modifier_receiver.apply_modifier(SkillModifierParam::Hp, 0.0);
    if !approximately(v, 0.0) {
        h.hp = clamp(v + h.hp, 0.0, h.max_hp);
        if v > 0.0 && h.phase >= Phase::End {
            h.ai.is_req_calc_last_spurt = true;
        }
    }
}

/// HP consumption of one frame at `self_speed` (this frame's new speed). The caller skips finished horses.
pub fn consume_hp_in_update_ai(race: &mut Race, i: usize, dt: f32, self_speed: f32) {
    let h = &race.horses[i];
    if h.hp > 0.0 {
        let rbs = if h.is_start_dash { self_speed } else { race.info.base_speed };
        let dec = calc_dec_hp_per_sec(race, i, rbs, h.ground_modifier_param.multi_hp_sub, h.phase, stats::guts(h), self_speed);
        let h = &mut race.horses[i];
        // `dec * dt` is rounded to f32 before the add, like the speed update's `accel * dt`.
        h.hp = clamp(-(dec * dt) + h.hp, 0.0, h.max_hp);
    }
}

/// HP/s at speed `v` with the given rate; shared by consumption, need-HP and the last-spurt distance search.
pub fn hp_per_sec(p: &Hp, rbs: f32, rate: f32, multi_hp_sub: f32, phase: Phase, guts: f32, v: f32) -> f32 {
    // Three temporaries, each rounded to f32:
    //   gap = Pow(v - rbs + SpeedGapParam1, 2) / SpeedGapParam1Pow      (f32 op)
    //   x   = gap * HpDecBase * rate * multiHpSub                       (one f64 expression)
    //   x * (HpGutsCoef / Sqrt(guts * HpGutsCoefSqrt) + 1)              (the factor one f64 expression)
    let f = |x: f32| x as f64;
    let gap = mathf_pow((v - rbs) + p.SpeedGapParam1, 2.0) / p.SpeedGapParam1Pow;
    let mut x = (f(gap) * f(p.HpDecBase) * f(rate) * f(multi_hp_sub)) as f32;
    if phase >= Phase::End {
        x = x * (f(p.HpGutsCoef) / f((guts * p.HpGutsCoefSqrt).sqrt()) + 1.0) as f32;
    }
    x
}

pub fn calc_dec_hp_per_sec(race: &Race, i: usize, race_base_speed: f32, ground_modifier_multi_hp_sub: f32, phase: Phase, guts: f32, cur_speed: f32) -> f32 {
    let p = &race.data.param.Hp;
    hp_per_sec(p, race_base_speed, calc_hp_dec_rate(race, i), ground_modifier_multi_hp_sub, phase, guts, cur_speed)
}

/// HP consumption rate of the horse's state (spot struggle, rushed, pace down, downhill accel mode).
pub fn calc_hp_dec_rate(race: &Race, i: usize) -> f32 {
    let p = &race.data.param.Hp;
    let h = &race.horses[i];
    let oonige = h.horse_data.running_style_ex == RunningStyleEx::Oonige;
    let tempt = h.ai.temptation.is_temptation;
    let mut rate = p.HpDecRateBaseNormal;
    let s = h.skill_modifier_receiver.apply_modifier(SkillModifierParam::HpDecRate, 0.0);
    if s > 0.0 {
        rate = s; // a positive HpDecRate modifier replaces the state rate
    } else if tempt && h.is_compete_top {
        rate = if oonige { p.HpDecRateBaseTemptationAndCompeteTopOonige } else { p.HpDecRateBaseTemptationAndCompeteTopNige };
    } else if h.is_compete_top {
        rate = if oonige { p.HpDecRateBaseCompeteTopOonige } else { p.HpDecRateBaseCompeteTopNige };
    } else if tempt {
        rate = p.HpDecRateBaseTemptation;
    } else if position_keep::is_position_keep(&race.horses[i]) && position_keep::position_keep_mode(&race.horses[i]) == PositionKeepMode::PaceDown {
        rate = p.HpDecRateBasePositionKeepPaceDown;
    }
    if h.ai.is_down_slope_accel_mode {
        rate = rate * p.HpDecRateMultiplyDownSlopeAccelMode;
    }
    rate
}

/// HP needed to run `distance` at `target_speed` at the normal rate: `(distance / v) * hpPerSec`.
pub fn calc_need_hp(race: &Race, race_base_speed: f32, ground_modifier_multi_hp_sub: f32, phase: Phase, guts: f32, target_speed: f32, distance: f32) -> f32 {
    let p = &race.data.param.Hp;
    (distance / target_speed) * hp_per_sec(p, race_base_speed, p.HpDecRateBaseNormal, ground_modifier_multi_hp_sub, phase, guts, target_speed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hp_per_sec_hand_computed() {
        let p = Hp { SpeedGapParam1: 12.0, SpeedGapParam1Pow: 144.0, HpDecBase: 20.0, HpGutsCoef: 200.0, HpGutsCoefSqrt: 600.0, ..Default::default() };
        // v == rbs: gap term 1.0 → 20 * rate * multi.
        assert_eq!(hp_per_sec(&p, 20.0, 1.0, 1.0, Phase::MiddleRun, 1000.0, 20.0), 20.0);
        // Late race guts term: 200 / sqrt(600*600) + 1 = 1.3333334.
        let x = hp_per_sec(&p, 20.0, 1.0, 1.0, Phase::End, 600.0, 20.0);
        assert_eq!(x, 20.0 * (200.0f64 / 600.0 + 1.0) as f32);
        // (13 + 12)^2 / 144 rounded, then * 20 * 1.6f * 1.02f as one f64 expression.
        let e = ((625.0f32 / 144.0) as f64 * 20.0 * 1.6f32 as f64 * 1.02f32 as f64) as f32;
        assert_eq!(hp_per_sec(&p, 0.0, 1.6, 1.02, Phase::Start, 1.0, 13.0), e);
        assert_eq!(25.0f32.powf(2.0), 625.0); // powf(x, 2) == x*x here
    }
}
