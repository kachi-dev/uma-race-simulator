//! Distance windows of the random-position triggers (random slope, straight, corner, phase, ...) and the course
//! spans they are drawn from.

use crate::sim::race::Race;
use crate::sim::util::intro_sort;

/// Distance window of a random-position trigger: active while `distance_min <= distance <= distance_max`.
#[derive(Clone, Copy, Debug, Default)]
pub struct DistanceWindow {
    pub distance_min: f32,
    pub distance_max: f32,
    pub is_initialized: bool,
    pub has_distance: bool,
}

/// How a random-position trigger picks its window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DistanceInit {
    UpSlopeRandom,
    DownSlopeRandom,
    FurlongRandom,
    AllPhaseRandom,
    DistanceRateAfterRandom,
    StraightRandom,
    WhileInStraightRandom,
    LastStraightRandom,
    CornerRandom,
    FinalCornerRandom,
    PhaseRandom,
    PhaseLaterHalfRandom,
    PhaseFirstHalfRandom,
}

impl DistanceInit {
    /// Windows that may fail at construction and are retried after each update until they succeed.
    pub(super) fn retries_in_update(self) -> bool {
        use DistanceInit::*;
        matches!(self, StraightRandom | WhileInStraightRandom | LastStraightRandom | CornerRandom | FinalCornerRandom)
    }
}

/// Length of a random-position window, m.
const DISTANCE_RANGE: f32 = 10.0;

/// (start, end) of corner number `corner`: the last matching corner event.
pub fn calc_corner_distance(race: &Race, corner: i32) -> Option<(f32, f32)> {
    if corner <= 0 {
        return None;
    }
    for ev in race.course.event_param.course_event_corner.iter().rev() {
        let v = &ev.param._values;
        if v.len() > 1 && v[0] == corner {
            let start = ev.param._distance;
            return Some((start, start + v[1] as f32));
        }
    }
    None
}

/// (start, end) of the final corner: the last corner numbered >= 4 starting within 1000 m of the goal.
pub fn calc_final_corner_distance(race: &Race) -> Option<(f32, f32)> {
    let d = race.info.race_course_set.distance as f32;
    for ev in race.course.event_param.course_event_corner.iter().rev() {
        let v = &ev.param._values;
        if v.len() > 1 && d - ev.param._distance <= 1000.0 && v[0] > 3 {
            let start = ev.param._distance;
            return Some((start, start + v[1] as f32));
        }
    }
    None
}

fn phase_window(race: &Race, phase: i32) -> (f32, f32) {
    let pc = &race.info.phase_distances;
    match phase {
        0 => (0.0, pc.phase_middle_start_distance),
        1 => (pc.phase_middle_start_distance, pc.phase_end_start_distance),
        2 => (pc.phase_end_start_distance, pc.phase_last_start_distance),
        3 => (pc.phase_last_start_distance, pc.course_distance),
        4 => (pc.course_distance, f32::MAX),
        _ => (0.0, 0.0),
    }
}

/// Draws a `DISTANCE_RANGE` window starting in `[min, max - range]` (one float draw).
fn lot_window(race: &mut Race, w: &mut DistanceWindow, min: f32, max: f32) {
    let hi = max - DISTANCE_RANGE;
    let hi = if min > hi { min } else { hi };
    let s = race.rng.random_float_range(min, hi);
    w.distance_min = s;
    w.distance_max = s + DISTANCE_RANGE;
}

pub(super) fn init_distance(race: &mut Race, init: DistanceInit, value_rh: i32, w: &mut DistanceWindow) {
    use DistanceInit::*;
    match init {
        // A random slope of that type.
        UpSlopeRandom | DownSlopeRandom => {
            w.is_initialized = true;
            let st = if init == UpSlopeRandom { crate::sim::defs::SlopeType::Up } else { crate::sim::defs::SlopeType::Down };
            let arr: Vec<(f32, f32)> =
                race.course.slopes.iter().filter(|s| s.slope_type == st).map(|s| (s.start_distance, s.end_distance)).collect();
            w.has_distance = !arr.is_empty();
            if arr.is_empty() {
                return;
            }
            let i = race.rng.random_int(arr.len() as i32) as usize;
            lot_window(race, w, arr[i].0, arr[i].1);
        }
        // Within furlong `value_rh`.
        FurlongRandom => {
            lot_window(race, w, value_rh as f32 * 200.0, (value_rh + 1) as f32 * 200.0);
            w.is_initialized = true;
        }
        // Within a random one of the phases Start, MiddleRun, End.
        AllPhaseRandom => {
            let i = race.rng.random_int(3);
            let (lo, hi) = phase_window(race, i);
            lot_window(race, w, lo, hi);
            w.is_initialized = true;
        }
        // After `value_rh` % of the course.
        DistanceRateAfterRandom => {
            let d = race.info.race_course_set.distance as f32;
            lot_window(race, w, value_rh as f32 / 100.0 * d, d);
            w.is_initialized = true;
        }
        // Within a random straight.
        StraightRandom => {
            let n = race.course.straights.len();
            if n > 0 {
                let i = race.rng.random_int(n as i32) as usize;
                let s = race.course.straights[i];
                lot_window(race, w, s.start_distance, s.end_distance);
            }
            w.is_initialized = true;
        }
        // The whole of a random straight (no window draw).
        WhileInStraightRandom => {
            let n = race.course.straights.len();
            if n > 0 {
                let i = race.rng.random_int(n as i32) as usize;
                let s = race.course.straights[i];
                w.distance_min = s.start_distance;
                w.distance_max = s.end_distance;
            }
            w.is_initialized = true;
        }
        // Within the last straight starting at or after the final corner.
        LastStraightRandom => {
            w.is_initialized = true;
            w.has_distance = false;
            let Some((fc_start, _)) = calc_final_corner_distance(race) else { return };
            if race.course.straights.is_empty() {
                return;
            }
            let found = race.course.straights.iter().rev().find(|s| s.start_distance >= fc_start).copied();
            if let Some(s) = found {
                lot_window(race, w, s.start_distance, s.end_distance);
            }
            // Set even without a qualifying straight (the window then stays [0, 0]).
            w.has_distance = true;
        }
        // Within corner number `value_rh`.
        CornerRandom => {
            if let Some((s, e)) = calc_corner_distance(race, value_rh) {
                lot_window(race, w, s, e);
                w.is_initialized = true;
            }
        }
        // Within the final corner.
        FinalCornerRandom => {
            if let Some((s, e)) = calc_final_corner_distance(race) {
                lot_window(race, w, s, e);
                w.is_initialized = true;
            }
        }
        // Within phase `value_rh`.
        PhaseRandom => {
            let (lo, hi) = phase_window(race, value_rh);
            lot_window(race, w, lo, hi);
            w.is_initialized = true;
        }
        // Within the second half of phase `value_rh`.
        PhaseLaterHalfRandom => {
            let (lo, hi) = phase_window(race, value_rh);
            let mid = (hi - lo) * 0.5 + lo;
            let max_start = hi - DISTANCE_RANGE;
            let r = race.rng.random_float_range(mid, if mid > max_start { mid } else { max_start });
            w.distance_min = r;
            w.distance_max = r + DISTANCE_RANGE;
            w.is_initialized = true;
        }
        // Within the first half of phase `value_rh`.
        PhaseFirstHalfRandom => {
            let (lo, hi) = phase_window(race, value_rh);
            let max_start = ((hi - lo) * 0.5 + lo) - DISTANCE_RANGE;
            let r = race.rng.random_float_range(lo, if lo > max_start { lo } else { max_start });
            w.distance_min = r;
            w.distance_max = r + DISTANCE_RANGE;
            w.is_initialized = true;
        }
    }
}

/// all_corner_random windows: up to 4 picks among the course corners, each a random corner then a window in it. A
/// corner with room left after the window stays a candidate (from the window's end); corners before it drop out.
pub(super) fn all_corner_random_init(race: &mut Race, list: &mut Vec<(f32, f32)>) {
    let mut l: Vec<(f32, f32)> = race.course.corners.iter().map(|c| (c.start_distance, c.end_distance)).collect();
    for _ in 0..4 {
        if l.is_empty() {
            break;
        }
        let i = race.rng.random_int_range(0, l.len() as i32) as usize;
        let (c_start, c_end) = l[i];
        let hi = if c_end - DISTANCE_RANGE < c_start { c_start } else { c_end - DISTANCE_RANGE };
        let s = race.rng.random_float_range(c_start, hi);
        list.push((s, s + DISTANCE_RANGE));
        intro_sort(list, |a, b| if a.0 > b.0 { 1 } else if b.0 > a.0 { -1 } else { 0 });
        let ret_max = s + DISTANCE_RANGE;
        let c_start_now = if c_end - ret_max >= DISTANCE_RANGE {
            l[i].0 = ret_max;
            ret_max
        } else {
            l.remove(i);
            c_start
        };
        l.retain(|x| !(c_start_now > x.0));
    }
}
