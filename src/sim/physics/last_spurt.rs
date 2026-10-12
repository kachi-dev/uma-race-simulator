//! Last spurt: where to start it and at what speed, from the HP left. The persistent state
//! (`last_spurt_calc_count`, `last_spurt_calc_result`, ...) lives on `ai::Ai`.

use crate::sim::defs::{last_spurt_calc_result as res, Phase};
use crate::sim::physics::hp;
use crate::sim::race::Race;
use crate::sim::util::{f32_to_i32, intro_sort, mathf_pow};
use crate::sim::physics::{speed, stats};

/// Snapshot of the horse the last-spurt calculation works on.
#[derive(Clone, Copy, Debug, Default)]
pub struct LastSpurtCalcParam {
    pub cur_distance: f32,
    pub phase: Phase,
    pub hp: f32,
    pub speed: f32,
    pub guts: f32,
    pub wiz: f32,
    pub base_target_speed: f32,
    pub ground_modifier_multi_hp_sub: f32,
    pub proper_distance_coef: f32,
    pub course_distance: i32,
    pub race_base_speed: f32,
}

/// One last-spurt calculation (built fresh for every recompute).
#[derive(Clone, Copy, Debug, Default)]
pub struct LastSpurt {
    pub calc_param: LastSpurtCalcParam,
    pub last_spurt_base_target_speed_add: f32,
    pub last_spurt_distance_goal_buffer: i32,
    pub last_spurt_start_distance: f32,
    pub last_spurt_target_speed: f32,
    /// A `defs::last_spurt_calc_result` flag.
    pub calc_result: i32,
}

#[derive(Clone, Copy, Debug)]
struct LastSpurtCandidate {
    distance: f32,
    speed: f32,
    need_time: f32,
}

/// A calculator with no result yet (start distance -1, target speed = race base speed).
pub fn new_last_spurt(race: &Race, race_base_speed: f32) -> LastSpurt {
    let p = &race.data.param;
    LastSpurt {
        calc_param: LastSpurtCalcParam::default(),
        last_spurt_base_target_speed_add: race_base_speed * p.lastSpurtBaseTargetSpeedAddCoef,
        last_spurt_distance_goal_buffer: p.LastSpurtDistanceGoalBuffer,
        last_spurt_start_distance: -1.0,
        last_spurt_target_speed: race_base_speed,
        calc_result: res::FALSE,
    }
}

impl LastSpurt {
    /// Distance left to the goal minus the goal buffer, at least 0.
    fn remain_distance(&self) -> f32 {
        let p = &self.calc_param;
        let r = (p.course_distance as f32 - p.cur_distance) - self.last_spurt_distance_goal_buffer as f32;
        if r > 0.0 { r } else { 0.0 }
    }

    /// Normal-rate HP/s at `v`.
    fn hp_per_sec(&self, race: &Race, v: f32) -> f32 {
        let (h, p) = (&race.data.param.Hp, &self.calc_param);
        hp::hp_per_sec(h, p.race_base_speed, h.HpDecRateBaseNormal, p.ground_modifier_multi_hp_sub, p.phase, p.guts, v)
    }

    fn need_hp(&self, race: &Race, v: f32, distance: f32) -> f32 {
        let p = &self.calc_param;
        hp::calc_need_hp(race, p.race_base_speed, p.ground_modifier_multi_hp_sub, p.phase, p.guts, v, distance)
    }

    /// Longest spurt at `vs` (the rest at `vn`) the HP allows, searched in 30 m steps and starting before
    /// `start_max`. Returns (start distance, total time).
    fn calc_spurt_distance(&self, race: &Race, vs: f32, vn: f32, start_max: f32) -> Option<(f32, f32)> {
        let p = &self.calc_param;
        let remain = self.remain_distance();
        let hs = self.hp_per_sec(race, vs);
        let hn = self.hp_per_sec(race, vn);
        let mut d = remain;
        while d > 0.0 {
            let t1 = d / vs;
            let t2 = (remain - d) / vn;
            // The comparison sees the f64 sum.
            if p.hp as f64 >= hs as f64 * t1 as f64 + t2 as f64 * hn as f64 {
                let s = (p.course_distance - self.last_spurt_distance_goal_buffer) as f32 - d;
                if start_max > s {
                    return Some((s, t2 + t1));
                }
            }
            d = d + -30.0;
        }
        None
    }

    /// Lowers the speed from `v` until some spurt distance fits (at most 100 steps). No RNG.
    /// Returns (result, distance, speed).
    fn speed_down(&self, race: &Race, mut v: f32, start_max: f32) -> (i32, f32, f32) {
        let p = &self.calc_param;
        let (dist, speed) = (p.cur_distance, p.base_target_speed);
        let remain = self.remain_distance();
        if self.need_hp(race, p.base_target_speed, remain) > p.hp {
            return (res::FALSE_BELOW_NEED_MIN_HP, dist, speed);
        }
        if !(p.hp < self.need_hp(race, v, remain)) {
            return (res::TRUE_EXCEED_NEED_MAX_HP, dist, speed);
        }
        let lo = if p.base_target_speed > v { v } else { p.base_target_speed };
        if v < lo {
            return (res::FALSE, dist, speed);
        }
        let mut k = 0;
        loop {
            if let Some((d, _)) = self.calc_spurt_distance(race, v, p.base_target_speed, start_max) {
                return (res::TRUE, d, v);
            }
            v = v - race.data.param.lastSpurtSpeedDelta;
            if lo >= v {
                return (res::FALSE, dist, speed);
            }
            k += 1;
            if k > 100 || !(v >= lo) {
                return (res::FALSE, dist, speed);
            }
        }
    }

    /// Second pass from `v` down: collects every feasible speed, sorts by need time (.NET introsort with the game's
    /// truncating, non-transitive comparator), then picks one by Wit: one `Ri(100)` per candidate tried, the last
    /// candidate if none passes. Returns (distance, speed).
    fn speed_down_next(&self, race: &mut Race, mut v: f32, start_max: f32) -> (f32, f32) {
        let p = self.calc_param;
        let v_in = v;
        let lo = if p.base_target_speed > v { v } else { p.base_target_speed };
        let mut list = Vec::new();
        let mut k = 0;
        while v >= lo {
            if let Some((distance, need_time)) = self.calc_spurt_distance(race, v, p.base_target_speed, start_max) {
                list.push(LastSpurtCandidate { distance, speed: v, need_time });
            }
            v = v - race.data.param.lastSpurtStep2SpeedDelta;
            if lo >= v {
                break;
            }
            k += 1;
            if k > 100 {
                break;
            }
        }
        if list.is_empty() {
            return (p.cur_distance, v_in);
        }
        intro_sort(&mut list, |a, b| f32_to_i32((a.need_time - b.need_time) * 10000.0));
        // Evaluated in f64 as one expression.
        let per = (p.wiz as f64 * race.data.param.lastSpurtStep2PerVal2 as f64 + race.data.param.lastSpurtStep2PerVal1 as f64) as f32;
        let mut chosen = None;
        for c in &list {
            let r = race.rng.random_int(100);
            if per > r as f32 {
                chosen = Some(*c);
                break;
            }
        }
        let c = chosen.unwrap_or(list[list.len() - 1]);
        (c.distance, c.speed)
    }
}

/// Runs the calculation: max spurt speed, then the speed-down passes. Draws `Ri(100)` per candidate tried.
pub fn calc_last_spurt(race: &mut Race, calc: &mut LastSpurt) {
    let pd = &race.data.param;
    let p = calc.calc_param;
    // Max spurt speed `g + (s + b)`, the whole sum in f64 with one rounding.
    let f = |x: f32| x as f64;
    let g = f(mathf_pow(p.guts * pd.LastSpurtTargetSpeedGutsCoef1, pd.LastSpurtTargetSpeedGutsPow)) * f(pd.LastSpurtTargetSpeedGutsCoef2);
    let s = f((pd.LastSpurtTargetSpeedCoefSqrt * p.speed).sqrt()) * f(p.proper_distance_coef) * f(pd.addSpeedParamCoef);
    // The `add + base target speed` sum is a float argument in the game, so it is rounded to f32 first.
    let b = f(calc.last_spurt_base_target_speed_add + p.base_target_speed) * f(pd.baseTargetSpeedCoef);
    let max_v = (g + (s + b)) as f32;
    let start_max = p.course_distance as f32 - pd.lastSpurtDistanceMaxFromGoal;
    let (r, _, out_v) = calc.speed_down(race, max_v, start_max);
    calc.calc_result = r;
    if r & res::TRUE_FLAGS == 0 {
        calc.last_spurt_start_distance = -1.0;
    } else {
        if r == res::TRUE {
            let (d2, v2) = calc.speed_down_next(race, out_v, start_max);
            calc.last_spurt_start_distance = d2;
            calc.last_spurt_target_speed = v2;
        } else if r == res::TRUE_EXCEED_NEED_MAX_HP {
            calc.last_spurt_start_distance = p.cur_distance;
            calc.last_spurt_target_speed = max_v;
        }
        if p.cur_distance > calc.last_spurt_start_distance {
            calc.last_spurt_start_distance = p.cur_distance;
        }
    }
}

/// Recomputes the last spurt when requested (AI update step 7) and stores the result on the AI.
pub fn update_last_spurt(race: &mut Race, i: usize) {
    if !race.horses[i].ai.is_req_calc_last_spurt {
        return;
    }
    let h = &race.horses[i];
    let p = LastSpurtCalcParam {
        cur_distance: h.distance,
        hp: h.hp,
        phase: h.phase,
        speed: stats::speed(h),
        guts: stats::guts(h),
        wiz: stats::wiz(h),
        base_target_speed: speed::base_target_speed_without_random(race, i, h.phase),
        ground_modifier_multi_hp_sub: h.ground_modifier_param.multi_hp_sub,
        proper_distance_coef: h.proper_distance_coef_speed,
        course_distance: race.info.race_course_set.distance,
        race_base_speed: race.info.base_speed,
    };
    let mut calc = new_last_spurt(race, race.info.base_speed);
    calc.calc_param = p;
    calc_last_spurt(race, &mut calc);
    let ai = &mut race.horses[i].ai;
    ai.last_spurt_calc_count += 1;
    if ai.last_spurt_calc_count <= 1 {
        ai.last_spurt_calc_result = calc.calc_result;
        ai.last_spurt_target_speed = calc.last_spurt_target_speed;
        ai.last_spurt_start_distance = calc.last_spurt_start_distance;
    } else if calc.calc_result & res::TRUE_FLAGS != 0 {
        // A recompute that yields False keeps the old values; an existing start point is kept.
        ai.last_spurt_calc_result = calc.calc_result;
        ai.last_spurt_target_speed = calc.last_spurt_target_speed;
        if 0.0 > ai.last_spurt_start_distance {
            ai.last_spurt_start_distance = calc.last_spurt_start_distance;
        }
    }
    ai.is_req_calc_last_spurt = false;
}

/// Is the horse past its (successful) last-spurt start point?
pub fn is_last_spurt(race: &Race, i: usize) -> bool {
    let h = &race.horses[i];
    let ai = &h.ai;
    ai.last_spurt_start_distance > 0.0 && ai.last_spurt_calc_result & res::FALSE_FLAGS == 0 && h.distance >= ai.last_spurt_start_distance
}

pub fn last_spurt_target_speed(race: &Race, i: usize) -> f32 {
    race.horses[i].ai.last_spurt_target_speed
}
