//! Course events and geometry: the sorted event list, corner/straight/slope lists, per-frame event
//! dispatch and the event handlers.

pub mod lanes;

use crate::sim::defs::{straight_front_type, SlopeType};
use crate::sim::race::{Race, RaceInfo};
use crate::sim::util::intro_sort;
use crate::data::{CourseParam, GameData};

pub mod course_param_type {
    pub const CORNER: i32 = 0;
    pub const GROUND_CHANGE: i32 = 1;
    pub const STRAIGHT: i32 = 2;
    pub const LANE_MAX_CHANGE: i32 = 3;
    pub const MOVE_LANE_POINT: i32 = 7;
    pub const SLOPE: i32 = 11;
}

/// `param._paramType` is a [`course_param_type`].
#[derive(Clone, Debug, Default)]
pub struct CourseEvent {
    pub param: CourseParam,
}

#[derive(Clone, Debug, Default)]
pub struct CourseEvents {
    /// All events, sorted by distance.
    pub course_event_list: Vec<CourseEvent>,
    pub course_event_corner: Vec<CourseEvent>,
    pub course_event_straight: Vec<CourseEvent>,
    pub course_event_slope: Vec<CourseEvent>,
    pub first_move_lane_point_distance: f32,
    pub is_empty_in_move: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CourseCorner {
    pub start_distance: f32,
    pub end_distance: f32,
    pub corner_number: i32,
    pub is_final_corner: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CourseSlope {
    pub start_distance: f32,
    pub end_distance: f32,
    pub slope_type: SlopeType,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CourseStraight {
    pub start_distance: f32,
    pub end_distance: f32,
    pub range: f32,
    /// A `defs::straight_front_type`.
    pub front_type: i32,
    /// 1-based number among the Front straights, or among all the others (they share one counter).
    pub front_type_number: i32,
}

#[derive(Clone, Debug, Default)]
pub struct CourseState {
    pub event_param: CourseEvents,
    pub first_corner_distance: f32,
    pub straights: Vec<CourseStraight>,
    /// [Front(1), AcrossFront(2)] → straights of that front type (index = type - 1).
    pub straights_by_front_type: [Vec<CourseStraight>; 2],
    pub corners: Vec<CourseCorner>,
    pub slopes: Vec<CourseSlope>,
}

/// Per-horse progress through the course event list.
#[derive(Clone, Debug, Default)]
pub struct RaceEventManager {
    /// Last event executed for each horse, -1 initially.
    pub last_event_by_horse: Vec<i32>,
}

/// The course's event params (data/courseeventparam/<course_set_id>.json).
pub fn load_course_params(data: &GameData, info: &RaceInfo) -> Vec<CourseParam> {
    let id = info.race_course_set.id;
    data.course_event_params.get(&id.to_string()).unwrap_or_else(|| panic!("courseeventparam {id} not loaded")).courseParams.clone()
}

pub fn create_course_event(params: &[CourseParam]) -> CourseState {
    use course_param_type as t;
    let mut ep = CourseEvents::default();
    ep.course_event_list = params.iter().map(|p| CourseEvent { param: p.clone() }).collect();
    // Unstable IntroSort with a three-way distance comparator.
    intro_sort(&mut ep.course_event_list, |a, b| {
        let (ad, bd) = (a.param._distance, b.param._distance);
        if bd < ad || bd == ad {
            (bd < ad) as i32
        } else {
            -1
        }
    });
    let of_type = |ty: i32| ep.course_event_list.iter().filter(|e| e.param._paramType == ty).cloned().collect::<Vec<_>>();
    ep.course_event_corner = of_type(t::CORNER);
    ep.course_event_straight = of_type(t::STRAIGHT);
    ep.course_event_slope = of_type(t::SLOPE);
    // The first lane-move point: an explicit event, else the first corner.
    match ep.course_event_list.iter().find(|e| e.param._paramType == t::MOVE_LANE_POINT) {
        Some(e) if e.param._values.len() == 1 => {
            ep.first_move_lane_point_distance = e.param._distance;
            ep.is_empty_in_move = e.param._values[0] == 1;
        }
        _ => {
            let d = ep.course_event_corner.first().map_or(0.0, |c| c.param._distance);
            ep.first_move_lane_point_distance = if d > 0.0 { d } else { 0.0 };
            ep.is_empty_in_move = d > 0.0;
        }
    }
    let first_corner_distance = ep.course_event_corner.first().map_or(0.0, |c| c.param._distance);
    CourseState { event_param: ep, first_corner_distance, ..Default::default() }
}

/// Pairs straight start/end events.
pub fn cache_straights(course: &mut CourseState) {
    course.straights.clear();
    let (mut n_front, mut n_other) = (1, 1);
    let mut cur: Option<CourseStraight> = None;
    for ev in &course.event_param.course_event_straight {
        let v = &ev.param._values;
        // A malformed event aborts the whole method (`straights_by_front_type` stays empty).
        if v.is_empty() {
            return;
        }
        if v[0] == 1 {
            if v.len() < 2 {
                return;
            }
            let front_type = v[1];
            let number = if front_type == straight_front_type::FRONT {
                n_front += 1;
                n_front - 1
            } else {
                n_other += 1;
                n_other - 1
            };
            // A still-open previous start is dropped.
            cur = Some(CourseStraight { start_distance: ev.param._distance, end_distance: -1.0, range: 0.0, front_type, front_type_number: number });
        } else if let Some(mut c) = cur.take() {
            c.end_distance = ev.param._distance;
            c.range = c.end_distance - c.start_distance;
            course.straights.push(c);
        }
    }
    let by = |ft: i32| course.straights.iter().filter(|s| s.front_type == ft).copied().collect::<Vec<_>>();
    course.straights_by_front_type = [by(straight_front_type::FRONT), by(straight_front_type::ACROSS_FRONT)];
}

/// Corners from their events (length defaults to 200 m). Corner 4+ ending within the last 1000 m is final.
pub fn calc_corner_list(corner_events: &[CourseEvent], course_distance: i32) -> Vec<CourseCorner> {
    corner_events
        .iter()
        .map(|ev| {
            let v = &ev.param._values;
            let no = v[0];
            let len = if v.len() > 1 { v[1] } else { 200 };
            let start = ev.param._distance;
            let end = len as f32 + start;
            let is_final = !(course_distance as f32 - end > 1000.0) && no >= 4;
            CourseCorner { start_distance: start, end_distance: end, corner_number: no, is_final_corner: is_final }
        })
        .collect()
}

fn slope_type_of(per: f32) -> SlopeType {
    if per > 0.0 {
        SlopeType::Up
    } else if per < 0.0 {
        SlopeType::Down
    } else {
        SlopeType::Null
    }
}

/// Slopes from their events (`[per * 10000, length]`; others are skipped).
pub fn calc_slope_list(slope_events: &[CourseEvent]) -> Vec<CourseSlope> {
    slope_events
        .iter()
        .filter(|ev| ev.param._values.len() == 2)
        .map(|ev| {
            let v = &ev.param._values;
            let per = v[0] as f32 / 10000.0;
            CourseSlope { start_distance: ev.param._distance, end_distance: v[1] as f32 + ev.param._distance, slope_type: slope_type_of(per) }
        })
        .collect()
}

pub fn corner(course: &CourseState, corner_no: i32, distance: f32) -> Option<CourseCorner> {
    course.corners.iter().find(|c| c.corner_number == corner_no && c.start_distance <= distance && distance <= c.end_distance).copied()
}

pub fn slope(course: &CourseState, distance: f32, slope_type: SlopeType) -> Option<CourseSlope> {
    course.slopes.iter().find(|s| s.slope_type == slope_type && s.start_distance <= distance && distance <= s.end_distance).copied()
}

pub fn init_event_manager(race: &mut Race) {
    race.event_manager.last_event_by_horse = vec![-1; race.horses.len()];
}

/// Fires course events reached this frame: events in sorted order, then horses in index order.
/// Runs once per frame after the horse update.
pub fn event_manager_update(race: &mut Race) {
    let event_count = race.course.event_param.course_event_list.len();
    // Skipping events no horse can execute this frame is equivalent: below the lowest `called_event_no` none passes
    // the `called < event_no` test, and the list is sorted by distance (handlers don't move horses), so past the
    // first event beyond every horse's distance none passes `dist >= ev_distance`.
    let first = race.event_manager.last_event_by_horse.iter().min().map_or(0, |&m| (m + 1).max(0) as usize);
    let max_dist = race.horses.iter().fold(f32::MIN, |m, h| if h.distance > m { h.distance } else { m });
    for event_no in first..event_count {
        if race.course.event_param.course_event_list[event_no].param._distance > max_dist {
            break;
        }
        for horse_index in 0..race.horses.len() {
            let dist = race.horses[horse_index].distance;
            exec_event(race, event_no, horse_index, dist);
        }
    }
}

/// Runs every handler on the event once the horse reaches it; each handler filters on the param type.
fn exec_event(race: &mut Race, event_no: usize, i: usize, dist: f32) {
    let ev_distance = race.course.event_param.course_event_list[event_no].param._distance;
    if race.event_manager.last_event_by_horse[i] < event_no as i32 && dist >= ev_distance {
        race.event_manager.last_event_by_horse[i] = event_no as i32;
        // Copied out so the handlers can mutate the race.
        let p = race.course.event_param.course_event_list[event_no].param.clone();
        event_ground_type_change(race, i, p._paramType, &p._values);
        event_straight_change(race, i, p._paramType, &p._values);
        event_corner_change(race, i, p._paramType, &p._values);
        event_lane_max_change(race, i, p._paramType, &p._values);
        event_slope_change(race, i, p._paramType, &p._values);
    }
}

pub fn event_ground_type_change(race: &mut Race, i: usize, param_type: i32, values: &[i32]) {
    if param_type != course_param_type::GROUND_CHANGE {
        return;
    }
    let h = &mut race.horses[i];
    if h.ground_type != values[0] {
        h.ground_type = values[0]; // the rest is view
    }
}

/// Straight start (`[1, front_type]`) or end (`[0]`).
pub fn event_straight_change(race: &mut Race, i: usize, param_type: i32, values: &[i32]) {
    if param_type != course_param_type::STRAIGHT || values.is_empty() {
        return;
    }
    let is_start = values[0] == 1;
    if is_start && values.len() < 2 {
        return;
    }
    let ft = if is_start { values[1] } else { 0 };
    let dist = race.horses[i].distance;
    let found = race.course.straights.iter().find(|s| s.front_type == ft && !(s.start_distance > dist) && !(dist > s.end_distance)).copied();
    let h = &mut race.horses[i];
    let was_straight = h.is_straight;
    h.is_straight = is_start;
    h.cur_straight_front_type = ft;
    h.cur_straight_type_number = -1;
    if is_start {
        if let Some(s) = found {
            h.cur_straight_type_number = s.front_type_number;
        }
        h.cur_corner = 0;
        // Entering the last straight (after the final corner) records the order, once.
        let is_straight_last = h.is_straight && h.is_final_corner;
        if !was_straight && is_straight_last && h.last_straight_order == -1 {
            h.last_straight_order = h.cur_order;
        }
    }
}

/// Corner start (`[corner_no, length]`).
pub fn event_corner_change(race: &mut Race, i: usize, param_type: i32, values: &[i32]) {
    if param_type != course_param_type::CORNER || values.len() < 2 {
        return;
    }
    let no = values[0];
    let dist = race.horses[i].distance;
    let c = corner(&race.course, no, dist);
    let h = &mut race.horses[i];
    h.is_final_corner = c.is_some_and(|c| c.is_final_corner);
    h.is_straight = false;
    h.cur_corner = no;
    h.corner_end_distance = match c {
        Some(c) => c.end_distance,
        None => dist + 200.0,
    };
}

/// New maximum lane distance (ignored in Story races).
pub fn event_lane_max_change(race: &mut Race, i: usize, param_type: i32, values: &[i32]) {
    if param_type != course_param_type::LANE_MAX_CHANGE || race.info.race_type == crate::sim::defs::race_type::STORY {
        return;
    }
    race.horses[i].lane_distance_max = values[0] as f32 / 10000.0;
}

/// Slope start (`[per * 10000, length]`).
pub fn event_slope_change(race: &mut Race, i: usize, param_type: i32, values: &[i32]) {
    if param_type != course_param_type::SLOPE || values.len() != 2 {
        return;
    }
    let per = values[0] as f32 / 10000.0;
    let ty = slope_type_of(per);
    let dist = race.horses[i].distance;
    let s = slope(&race.course, dist, ty);
    let h = &mut race.horses[i];
    match s {
        Some(s) => {
            h.slope_type = ty;
            h.slope_per = per;
            h.slope_end_distance = s.end_distance;
        }
        None => {
            h.slope_type = SlopeType::Null;
            h.slope_per = 0.0;
        }
    }
}
