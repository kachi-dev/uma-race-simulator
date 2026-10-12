//! Horse AI: its state, creation, the around-horse scan (vision, near, blocking, surround, compete-fight
//! candidates), and the block and near-horse timers. The AI's per-frame order is in [`crate::sim::tick`].

pub mod compete_fight;
pub mod compete_top;
pub mod lane;
pub mod order;
pub mod overtake;
pub mod pacemaker;
pub mod position_keep;
pub mod temptation;

use crate::sim::defs::{last_spurt_calc_result, SkillModifierParam};
use crate::sim::ai::lane::{ActiveLanePlanner, FixedLane, LanePlanner};
use crate::sim::ai::overtake::OvertakeTargets;
use crate::sim::ai::position_keep::{PositionKeep};
use crate::sim::race::Race;
use crate::sim::physics::conserve_power::{self, ConservePower};
use crate::sim::physics::speed::{self, BaseTargetSpeeds, TargetSpeed};
use crate::sim::ai::temptation::{Temptation};
use crate::sim::horse::Horse;
use crate::sim::field::horse_at_order;
use crate::sim::util::max;
use crate::data::ParamDefine;

/// What one horse sees of another, from the start-of-frame scan. One per (observer, other) pair, in other-horse
/// index order, self skipped.
#[derive(Clone, Copy, Debug, Default)]
pub struct AroundHorse {
    /// Horse index.
    pub info: usize,
    pub distance_gap: f32,
    pub distance_gap_abs: f32,
    pub lane_gap: f32,
    pub lane_gap_abs: f32,
    pub is_back: bool,
    /// Self - other (sign reversed vs the other gaps).
    pub speed_gap: f32,
    pub speed_gap_abs: f32,
    pub compete_fight_candidate_continue_time: f32,
}

/// The horses inside the vision cone.
#[derive(Clone, Debug, Default)]
pub struct Visibility {
    /// Indices into `around_horses`; the first `visible_horse_count` are valid.
    pub visible_horses: Vec<usize>,
    pub visible_horse_count: i32,
}

/// Per-horse AI state. References to other horses (`block_horse`, `near_horses`, the overtake list, ...) are
/// indices into `around_horses`.
#[derive(Clone, Debug, Default)]
pub struct Ai {
    pub target_speed: f32,
    pub target_lane: f32,
    pub after_goal_time: f32,
    pub around_horses: Vec<AroundHorse>,
    pub block_horse: Option<usize>,
    pub block_horse_in: Option<usize>,
    pub block_horse_out: Option<usize>,
    pub is_around_horse_initialized: bool,
    pub in_lane_space: f32,
    pub out_lane_space: f32,
    pub overtake_cool_down_time: f32,
    pub is_down_slope_accel_mode: bool,
    pub down_slope_accel_check_sec: f32,
    pub position_keep: PositionKeep,
    pub temptation: Temptation,
    pub overtake_targets: OvertakeTargets,
    pub active_lane_planner: ActiveLanePlanner,
    /// Null when the course has no first move-lane point.
    pub lane_planner: LanePlanner,
    pub after_goal_lane: FixedLane,
    pub base_target_speeds: BaseTargetSpeeds,
    pub target_speed_calc: TargetSpeed,
    pub visibility: Visibility,
    pub conserve_power: ConservePower,
    pub is_force_speed_down_by_lane: bool,
    pub is_req_calc_last_spurt: bool,
    pub is_force_in_move_enable: bool,
    pub last_spurt_calc_count: i32,
    /// A `defs::last_spurt_calc_result` value.
    pub last_spurt_calc_result: i32,
    pub last_spurt_target_speed: f32,
    pub last_spurt_start_distance: f32,
    pub is_last_spurt_by_distance_requested: bool,
    pub is_surround_out: bool,
    pub is_surround_ahead: bool,
    pub is_surround_behind: bool,
    /// Indices into `around_horses`; the first `near_horse_count` are valid.
    pub near_horses: Vec<usize>,
    pub near_horse_count: i32,
}

/// Creation order is load-bearing (the target-speed calculator draws RNG).
pub fn create_ai(race: &mut Race, i: usize) {
    race.horses[i].ai = Ai {
        last_spurt_calc_result: last_spurt_calc_result::FALSE,
        last_spurt_start_distance: -1.0,
        ..Default::default()
    };
    let around: Vec<AroundHorse> = (0..race.horses.len()).filter(|&j| j != i).map(|j| AroundHorse { info: j, ..Default::default() }).collect();
    race.horses[i].ai.around_horses = around;
    let c = position_keep::new_position_keep(race, i);
    race.horses[i].ai.position_keep = c;
    let c = temptation::new_temptation();
    race.horses[i].ai.temptation = c;
    let c = lane::new_lane_planner(race, i);
    race.horses[i].ai.lane_planner = c;
    let c = speed::new_target_speed(race, i); // RNG
    race.horses[i].ai.target_speed_calc = c;
    let n_around = race.horses[i].ai.around_horses.len();
    race.horses[i].ai.visibility = Visibility { visible_horses: vec![0; n_around], visible_horse_count: 0 };
    let c = speed::new_base_target_speeds(race);
    race.horses[i].ai.base_target_speeds = c;
    let h = &mut race.horses[i];
    h.ai.target_speed = h.last_speed;
    h.ai.target_lane = h.lane_distance;
    h.ai.active_lane_planner = ActiveLanePlanner::Race;
    h.ai.near_horses = vec![0; n_around];
}

/// Scans every other horse: gaps, near / visible lists, front and side blockers, lane space,
/// compete-fight candidates and the surround flags. Runs before any horse moves, so it sees start-of-frame positions.
pub fn update_around_horses_param(race: &mut Race, i: usize, dt: f32) {
    let data = race.data;
    let p = &data.param;
    // Only horse i's lists and scan outputs are written, and nothing reads them back through `race.horses[i]`, so
    // they are taken out (the scalars accumulate in `scan`) and stored at the end.
    let mut around_horses = std::mem::take(&mut race.horses[i].ai.around_horses);
    let mut near_horses = std::mem::take(&mut race.horses[i].ai.near_horses);
    let mut visible_horses = std::mem::take(&mut race.horses[i].ai.visibility.visible_horses);
    let mut cf_list = std::mem::take(&mut race.horses[i].compete_fight_near);
    let mut scan = AroundScan::default();
    let h = &race.horses[i];
    let my_speed = h.last_speed;
    let my_dist = h.distance;
    let my_lane = h.lane_distance;
    scan.in_lane_space = my_lane;
    scan.out_lane_space = h.lane_distance_max - my_lane;
    // The near / visible lists are not cleared: only their first `count` entries are ever read.
    let mut min_front = f32::MAX;
    let vis_dist = visible_distance(h);
    let self_straight_last = is_straight_last(h);
    let block = &p.Block;
    for k in 0..around_horses.len() {
        let o = &race.horses[around_horses[k].info];
        let a = &mut around_horses[k];
        a.distance_gap = o.distance - my_dist;
        a.distance_gap_abs = abs_keep_neg_zero(a.distance_gap);
        a.lane_gap = o.lane_distance - my_lane;
        a.lane_gap_abs = abs_keep_neg_zero(a.lane_gap);
        a.speed_gap = my_speed - o.last_speed;
        a.speed_gap_abs = abs_keep_neg_zero(a.speed_gap);
        // Near: branch-free store (count <= k < len, and entries past the count are never read).
        near_horses[scan.near_horse_count as usize] = k;
        scan.near_horse_count += (!(a.distance_gap_abs > p.Near.DistanceThreshold) & !(a.lane_gap_abs > p.Near.LaneDistanceThreshold)) as i32;
        // Compete-fight candidates: close, side by side, and either horse on the final straight.
        let lane_ok = p.CompeteFight.LaneGap >= a.lane_gap_abs;
        let dist_far = p.CompeteFight.DistanceGap < a.distance_gap_abs;
        let straight = self_straight_last || is_straight_last(o);
        if dist_far || !lane_ok || !straight {
            if let Some(pos) = cf_list.iter().position(|&x| x == k) {
                cf_list.remove(pos);
            }
            a.compete_fight_candidate_continue_time = 0.0;
        } else if cf_list.contains(&k) {
            a.compete_fight_candidate_continue_time = dt + a.compete_fight_candidate_continue_time;
        } else {
            cf_list.push(k);
        }
        if -block.SideBlockDistanceGap > a.distance_gap {
            a.is_back = true;
            continue;
        }
        a.is_back = false;
        if check_front_block(p, a.distance_gap, a.lane_gap_abs) && min_front > a.distance_gap {
            min_front = a.distance_gap;
            scan.block_horse = Some(k);
        }
        if check_side_block(p, a.distance_gap_abs, a.lane_gap_abs) {
            if a.lane_gap > 0.0 {
                scan.block_horse_out = Some(k);
            } else {
                scan.block_horse_in = Some(k);
            }
        }
        if block.SideBlockDistanceGap > a.distance_gap_abs {
            if 0.0 > a.lane_gap {
                if scan.in_lane_space > a.lane_gap_abs {
                    scan.in_lane_space = a.lane_gap_abs;
                }
            } else if a.lane_gap > 0.0 && scan.out_lane_space > a.lane_gap_abs {
                scan.out_lane_space = a.lane_gap_abs;
            }
        }
        // Visible: branch-free store, as for `near_horses`.
        visible_horses[scan.visible_horse_count as usize] = k;
        scan.visible_horse_count += is_visible(vis_dist, a) as i32;
    }
    let h = &mut race.horses[i];
    h.ai.is_around_horse_initialized = true;
    (h.ai.block_horse, h.ai.block_horse_in, h.ai.block_horse_out) = (scan.block_horse, scan.block_horse_in, scan.block_horse_out);
    (h.ai.in_lane_space, h.ai.out_lane_space) = (scan.in_lane_space, scan.out_lane_space);
    (h.ai.near_horse_count, h.ai.visibility.visible_horse_count) = (scan.near_horse_count, scan.visible_horse_count);
    h.ai.around_horses = around_horses;
    h.ai.near_horses = near_horses;
    h.ai.visibility.visible_horses = visible_horses;
    h.compete_fight_near = cf_list;
    update_surrounded(p, &mut h.ai);
}

/// The scalar outputs of `update_around_horses_param`, accumulated locally (see there).
#[derive(Default)]
struct AroundScan {
    block_horse: Option<usize>,
    block_horse_in: Option<usize>,
    block_horse_out: Option<usize>,
    in_lane_space: f32,
    out_lane_space: f32,
    near_horse_count: i32,
    visible_horse_count: i32,
}

/// `|x|`, but -0.0 stays -0.0 (unlike `f32::abs`), as in the game.
#[inline]
fn abs_keep_neg_zero(x: f32) -> f32 {
    if 0.0 > x {
        -x
    } else {
        x
    }
}

/// Whether a horse `distance_gap` ahead blocks the front. The lane tolerance narrows with distance.
pub fn check_front_block(p: &ParamDefine, distance_gap: f32, lane_gap_abs: f32) -> bool {
    let b = &p.Block;
    if 0.0 >= distance_gap || distance_gap >= b.FrontBlockDistanceGap {
        return false;
    }
    // Evaluated in f64 as one expression.
    let tol = ((1.0 - (distance_gap as f64 / b.FrontBlockDistanceGap as f64) * 0.6f32 as f64) * b.FrontBlockLaneGap as f64) as f32;
    !(lane_gap_abs > tol)
}

/// Whether a horse alongside blocks a side (one in exactly the same lane does not).
pub fn check_side_block(p: &ParamDefine, distance_gap_abs: f32, lane_gap_abs: f32) -> bool {
    let b = &p.Block;
    if distance_gap_abs >= b.SideBlockDistanceGap || lane_gap_abs >= b.SideBlockLaneGap {
        return false;
    }
    0.0 > lane_gap_abs || lane_gap_abs > 0.0
}

/// Whether `a` is inside the vision cone of depth `v`: half-width 1/18 lane at the nose, widening to 0.375 at `v`.
pub fn is_visible(v: f32, a: &AroundHorse) -> bool {
    if 0.0 >= v || 0.0 > a.distance_gap || a.distance_gap > v {
        return false;
    }
    // Evaluated in f64 as one expression.
    let w = (((a.distance_gap as f64 / v as f64) * 0.638888896f32 as f64 + 0.111111112f32 as f64) * 0.5) as f32;
    !(a.lane_gap_abs > w)
}

/// Sets the three surround flags: a horse alongside on the outside, one ahead, one behind.
fn update_surrounded(p: &ParamDefine, ai: &mut Ai) {
    let s = &p.Surrounded;
    // The game stops testing a flag once it is set; the tests are pure, so a branch-free OR is the same and cheaper.
    let (mut s1, mut s2, mut s3) = (false, false, false);
    for a in &ai.around_horses {
        s1 |= (s.DistanceGapAbs1 >= a.distance_gap_abs) & (a.lane_gap >= 0.0) & (s.LaneGap1 >= a.lane_gap);
        s2 |= (a.distance_gap > 0.0) & (s.DistanceGap2 >= a.distance_gap) & (s.LaneGapAbs2 >= a.lane_gap_abs);
        s3 |= (0.0 > a.distance_gap) & (a.distance_gap >= -s.DistanceGap2) & (s.LaneGapAbs2 >= a.lane_gap_abs);
    }
    (ai.is_surround_out, ai.is_surround_ahead, ai.is_surround_behind) = (s1, s2, s3);
}

/// Time since the goal; switches `active_lane_planner` to OverRun after 1 s.
pub fn update_after_goal_time(race: &mut Race, i: usize, dt: f32) {
    let h = &mut race.horses[i];
    if h.is_finished {
        let old = h.ai.after_goal_time;
        h.ai.after_goal_time = dt + old;
        if 1.0 > old && h.ai.after_goal_time >= 1.0 {
            h.ai.active_lane_planner = ActiveLanePlanner::AfterGoal;
        }
    }
}

/// Re-creates the conserve-power calculator.
pub fn update_activate_status_conserve_power(race: &mut Race, i: usize) {
    race.horses[i].ai.conserve_power = conserve_power::new(race, i);
}

/// Front / side block timers, from the start-of-frame scan.
pub fn update_block_time(race: &mut Race, i: usize, dt: f32) {
    let front = block_horse_front(race, i);
    let front_hp_empty = front.is_some_and(|j| race.horses[j].hp <= 0.0);
    let h = &mut race.horses[i];
    let side = h.ai.block_horse_in.is_some() || h.ai.block_horse_out.is_some();
    if front.is_some() {
        h.block_front_accumulate_time += dt;
    }
    h.block_front_continue_time = if front.is_some() { h.block_front_continue_time + dt } else { 0.0 };
    h.block_front_hp_empty_continue_time = if front_hp_empty { h.block_front_hp_empty_continue_time + dt } else { 0.0 };
    if side {
        h.block_side_continue_time += dt;
        // Per-phase maximum; Finished has no entry.
        if let Some(stored) = h.block_side_max_continue_time_by_phase.get_mut(h.phase as usize) {
            if *stored < h.block_side_continue_time {
                *stored = h.block_side_continue_time;
            }
        }
    } else {
        h.block_side_continue_time = 0.0;
    }
}

/// How long the horse one place ahead has stayed near (and near in lane), while the order is unchanged.
pub fn update_in_front_horse_near_time(race: &mut Race, i: usize, dt: f32) {
    let p = &race.data.param.Skill;
    let (cur, prev) = (race.horses[i].cur_order, race.horses[i].prev_order);
    if cur == 0 || cur != prev {
        let h = &mut race.horses[i];
        h.in_front_horse_near_continue_time = 0.0;
        h.in_front_horse_near_lane_continue_time = 0.0;
        return;
    }
    let f = &race.horses[horse_at_order(race, cur - 1).expect("a horse one place ahead")];
    let (f_dist, f_lane) = (f.distance, f.lane_distance);
    let (near_d, near_l) = (p.InfrontHorseNearDistance, p.InfrontHorseNearLaneDistance);
    let h = &mut race.horses[i];
    if f_dist - h.distance > near_d {
        h.in_front_horse_near_continue_time = 0.0;
        h.in_front_horse_near_lane_continue_time = 0.0;
    } else {
        h.in_front_horse_near_continue_time += dt;
        if (h.lane_distance - f_lane).abs() <= near_l {
            h.in_front_horse_near_lane_continue_time += dt;
        } else {
            h.in_front_horse_near_lane_continue_time = 0.0;
        }
    }
}

/// How long the horse one place behind has stayed near (and near in lane), while the order is unchanged.
pub fn update_behind_horse_near_time(race: &mut Race, i: usize, dt: f32) {
    let p = &race.data.param.Skill;
    let (cur, prev) = (race.horses[i].cur_order, race.horses[i].prev_order);
    if cur != race.field.last_order && cur == prev {
        let b = &race.horses[horse_at_order(race, cur + 1).expect("a horse one place behind")];
        let (b_dist, b_lane) = (b.distance, b.lane_distance);
        let (near_d, near_l) = (p.BehindHorseNearDistance, p.BehindHorseNearLaneDistance);
        let h = &mut race.horses[i];
        if h.distance - b_dist <= near_d {
            h.behind_horse_near_continue_time += dt;
            if (h.lane_distance - b_lane).abs() <= near_l {
                h.behind_horse_near_lane_continue_time += dt;
            } else {
                h.behind_horse_near_lane_continue_time = 0.0;
            }
            return;
        }
    }
    let h = &mut race.horses[i];
    h.behind_horse_near_continue_time = 0.0;
    h.behind_horse_near_lane_continue_time = 0.0;
}

/// As `update_behind_horse_near_time`, once per (distance, lane) threshold pair of the skill params.
pub fn update_behind_horse_near_time_param_set(race: &mut Race, i: usize, dt: f32) {
    let params = &race.data.param.Skill.BehindNearParamArray;
    let (cur, prev) = (race.horses[i].cur_order, race.horses[i].prev_order);
    if cur != race.field.last_order && cur == prev {
        let b = &race.horses[horse_at_order(race, cur + 1).expect("a horse one place behind")];
        let (b_dist, b_lane) = (b.distance, b.lane_distance);
        let h = &mut race.horses[i];
        let d_gap = h.distance - b_dist;
        let l_gap = (h.lane_distance - b_lane).abs();
        for (t, p) in h.behind_near_lane_times.iter_mut().zip(params) {
            if p.Distance < d_gap || p.LaneDistance < l_gap {
                *t = 0.0;
            } else {
                *t += dt;
            }
        }
    } else {
        race.horses[i].behind_near_lane_times.iter_mut().for_each(|t| *t = 0.0);
    }
}

/// The front-block horse index.
pub fn block_horse_front(race: &Race, i: usize) -> Option<usize> {
    let ai = &race.horses[i].ai;
    ai.block_horse.map(|k| ai.around_horses[k].info)
}

/// On the final straight.
pub fn is_straight_last(h: &Horse) -> bool {
    h.is_straight && h.is_final_corner
}

/// Vision depth after skills, at least 0 (NaN → 0).
pub fn visible_distance(h: &Horse) -> f32 {
    max(h.skill_modifier_receiver.apply_modifier(SkillModifierParam::VisibleDistance, h.base_visible_distance), 0.0)
}

/// Free to move inward: no inside blocker and not at the inner limit.
pub fn is_enable_in_move(h: &Horse) -> bool {
    h.ai.block_horse_in.is_none() && !(h.lane_distance_min + 0.0001 >= h.lane_distance)
}

/// Free to move outward: no outside blocker and not at the outer limit.
pub fn is_enable_out_move(h: &Horse) -> bool {
    h.ai.block_horse_out.is_none() && !(h.lane_distance >= h.lane_distance_max)
}

/// Whether there is room for a lane change of `lane_gap_range` (negative = inward).
pub fn check_lane_space(h: &Horse, lane_gap_range: f32) -> bool {
    if 0.0 > lane_gap_range {
        h.ai.in_lane_space >= -lane_gap_range
    } else {
        h.ai.out_lane_space >= lane_gap_range
    }
}

pub fn is_block_front(h: &Horse) -> bool {
    h.ai.block_horse.is_some()
}

pub fn is_block_side(h: &Horse) -> bool {
    h.ai.block_horse_in.is_some() || h.ai.block_horse_out.is_some()
}

/// Blocked in front and unable to move to either side.
pub fn is_block_all(h: &Horse) -> bool {
    is_block_front(h) && !is_enable_in_move(h) && !is_enable_out_move(h)
}

/// All three surround flags set.
pub fn is_surrounded(h: &Horse) -> bool {
    h.ai.is_surround_out && h.ai.is_surround_ahead && h.ai.is_surround_behind
}

/// Has an overtake target.
pub fn is_overtake(h: &Horse) -> bool {
    !h.ai.overtake_targets.targets.is_empty()
}

/// Has an overtake target, or the overtake cooldown is still running.
pub fn is_overtake_or_cool_down(h: &Horse) -> bool {
    is_overtake(h) || h.ai.overtake_cool_down_time > 0.0
}

/// At least 1 s past the goal.
pub fn is_over_run_speed_down_mode(h: &Horse) -> bool {
    h.ai.after_goal_time >= 1.0
}

/// Currently rushed.
pub fn is_temptation(h: &Horse) -> bool {
    h.ai.temptation.is_temptation
}

/// Test fixture shared by the AI modules: a `Race` with `n` default horses, real `race_paramdefine.json`,
/// course max lane 1.5, AI state built with the no-RNG parts only (around horses, visible/near arrays).
#[cfg(test)]
pub(crate) fn test_race(n: usize) -> Race<'static> {
    use crate::api::RaceInput;
    use crate::data::GameData;
    let s = std::fs::read_to_string(format!("{}/race_paramdefine.json", crate::data::DATA_DIR)).unwrap();
    let data: &'static GameData = Box::leak(Box::new(GameData { param: serde_json::from_str(&s).unwrap(), ..Default::default() }));
    let input: &'static RaceInput = Box::leak(Box::new(RaceInput::default()));
    let mut race = Race {
        data,
        input,
        info: Default::default(),
        rng: crate::rng::RaceRandom::new(1),
        horses: vec![Horse::default(); n],
        field: Default::default(),
        course: Default::default(),
        course_lane: Default::default(),
        event_manager: Default::default(),
        accumulate_time_since_start: 0.0,
        behind_lane_threshold: 0.0,
        world_per_metre: 0.0,
        distance_diff_max: 0.0,
        is_simulate_end: false,
        recorder: Default::default(),
        next_modifier_id: 0,
    };
    race.info.lane_distance_max = 1.5;
    race.info.num_race_horses = n as i32;
    race.field.last_order = n as i32 - 1;
    race.field.standings = crate::sim::ai::order::Standings::init(n);
    for i in 0..n {
        let h = &mut race.horses[i];
        h.lane_distance_max = 1.5;
        h.base_visible_distance = data.param.visibleDistance;
        h.ai.around_horses = (0..n).filter(|&j| j != i).map(|j| AroundHorse { info: j, ..Default::default() }).collect();
        h.ai.visibility.visible_horses = vec![0; n - 1];
        h.ai.near_horses = vec![0; n - 1];
    }
    race
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(race: &mut Race, i: usize, dist: f32, lane: f32) {
        race.horses[i].distance = dist;
        race.horses[i].lane_distance = lane;
    }

    #[test]
    fn constants_bits() {
        assert_eq!(0.638888896f32.to_bits(), 0x3f238e39);
        assert_eq!(0.111111112f32.to_bits(), 0x3de38e39);
        assert_eq!(0.0001f32.to_bits(), 0x38d1b717);
        assert_eq!(0.6f32.to_bits(), 0x3f19999a);
    }

    #[test]
    fn scan_blocks_spaces_vision() {
        let mut race = test_race(6);
        place(&mut race, 0, 100.0, 0.30);
        place(&mut race, 1, 101.0, 0.30); // front, 1 m: front block
        place(&mut race, 2, 101.0, 0.31); // front, same gap: loses the tie to horse 1 (strict <)
        place(&mut race, 3, 100.5, 0.25); // inside, 0.5 m: side block in, in-space 0.05
        place(&mut race, 4, 99.5, 0.20); // inside, -0.5 m: later side-block-in match wins; space stays 0.05
        place(&mut race, 5, 98.0, 0.30); // behind beyond 1.05 m: isBack, ignored except near
        update_around_horses_param(&mut race, 0, 0.0666);
        let ai = &race.horses[0].ai;
        assert_eq!(block_horse_front(&race, 0), Some(1));
        assert_eq!(ai.block_horse_in.map(|k| ai.around_horses[k].info), Some(4));
        assert_eq!(ai.block_horse_out.map(|k| ai.around_horses[k].info), Some(2)); // 1.0 m < 1.05 m, lane +0.01
        assert!((ai.in_lane_space - 0.05).abs() < 1e-6);
        assert!((ai.out_lane_space - 0.01).abs() < 1e-6);
        assert!(ai.around_horses[4].is_back); // horse 5 → around index 4
        let vis: Vec<usize> = ai.visibility.visible_horses[..ai.visibility.visible_horse_count as usize]
            .iter()
            .map(|&k| ai.around_horses[k].info)
            .collect();
        assert_eq!(vis, [1, 2, 3]); // cone half-width at 0.5 m ≈ 0.0635 ≥ 0.05
        assert_eq!(ai.near_horse_count, 5);
        assert!(!is_enable_in_move(&race.horses[0]));
        assert!(!is_enable_out_move(&race.horses[0]));
        assert!(is_block_all(&race.horses[0]));
    }
}
