//! Slopes: leaving a slope after the distance step, downhill accel mode.
//! Entering a slope is the course event `course::event_slope_change`.

use crate::sim::defs::SlopeType;
use crate::sim::race::Race;
use crate::sim::physics::stats;
use crate::sim::util::min;

/// Leaves the slope once past its end (called right after the distance step). The cosmetic slope angle is not
/// modelled.
pub fn update_slope(race: &mut Race, i: usize) {
    let h = &mut race.horses[i];
    if h.slope_type != SlopeType::Null && h.slope_end_distance < h.distance {
        h.slope_type = SlopeType::Null;
        h.slope_per = 0.0;
    }
}

/// Enters / leaves downhill accel mode, checked once per second while on a downhill (AI update step 8, also when
/// finished). One `GetRandom(0f, 100f)` per check.
pub fn update_down_slope_accel(race: &mut Race, i: usize, dt: f32) {
    let data = race.data;
    let p = &data.param.Slope;
    let h = &mut race.horses[i];
    let on_down = h.slope_type == SlopeType::Down;
    if !h.ai.is_down_slope_accel_mode {
        if !on_down {
            return; // timer NOT reset
        }
        if h.ai.down_slope_accel_check_sec > 0.0 {
            h.ai.down_slope_accel_check_sec = h.ai.down_slope_accel_check_sec - dt;
            return;
        }
        h.ai.down_slope_accel_check_sec = 1.0;
        let per = min(100.0, stats::wiz_for(h, crate::api::wiz_mechanic::DOWNHILL) * p.DownSlopeAddSpeedStartWizRate);
        let r = race.rng.random_float_range(0.0, 100.0);
        if r < per {
            let ai = &mut race.horses[i].ai;
            ai.is_down_slope_accel_mode = true;
            ai.down_slope_accel_check_sec = 1.0;
        }
    } else {
        if !on_down {
            h.ai.is_down_slope_accel_mode = false;
            h.ai.down_slope_accel_check_sec = 1.0;
            return;
        }
        if h.ai.down_slope_accel_check_sec > 0.0 {
            h.ai.down_slope_accel_check_sec = h.ai.down_slope_accel_check_sec - dt;
            return;
        }
        h.ai.down_slope_accel_check_sec = 1.0;
        let r = race.rng.random_float_range(0.0, 100.0);
        if r < p.DownSlopeAddSpeedEndPer {
            let ai = &mut race.horses[i].ai;
            ai.is_down_slope_accel_mode = false;
            ai.down_slope_accel_check_sec = 1.0;
        }
    }
}
