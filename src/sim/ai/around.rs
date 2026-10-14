//! The around-horse scan (vision, near, blocking, surround, compete-fight candidates), the block and near-horse
//! timers, and the queries on what the scan found.

use crate::data::ParamDefine;
use crate::sim::defs::SkillModifierParam;
use crate::sim::field::horse_by_order;
use crate::sim::horse::Horse;
use crate::sim::race::Race;
use crate::sim::util::max;

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

/// What the around-horse scan reads of each horse, packed (perf: the scan reads every other horse, every horse,
/// every frame).
#[derive(Clone, Copy, Debug, Default)]
pub struct ScanView {
    pub distance: f32,
    pub lane_distance: f32,
    pub last_speed: f32,
    pub is_straight_last: bool,
}

/// Fills `field.scan_view` from every horse. The fields are only written while horses move or by course events, so
/// one copy taken before the pre-update loop is exact for every scan in it.
pub fn snapshot_scan_view(race: &mut Race) {
    let mut v = std::mem::take(&mut race.field.scan_view);
    v.clear();
    v.extend(race.horses.iter().map(|h| ScanView {
        distance: h.distance,
        lane_distance: h.lane_distance,
        last_speed: h.last_speed,
        is_straight_last: is_straight_last(h),
    }));
    race.field.scan_view = v;
}

/// Scans every other horse: gaps, near / visible lists, front and side blockers, lane space,
/// compete-fight candidates and the surround flags. Runs before any horse moves, so it sees start-of-frame positions
/// (read from `field.scan_view`, see [`snapshot_scan_view`]).
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
    let s = &p.Surrounded;
    let view = &race.field.scan_view;
    for k in 0..around_horses.len() {
        let o = &view[around_horses[k].info];
        let a = &mut around_horses[k];
        a.distance_gap = o.distance - my_dist;
        a.distance_gap_abs = abs_keep_neg_zero(a.distance_gap);
        a.lane_gap = o.lane_distance - my_lane;
        a.lane_gap_abs = abs_keep_neg_zero(a.lane_gap);
        a.speed_gap = my_speed - o.last_speed;
        a.speed_gap_abs = abs_keep_neg_zero(a.speed_gap);
        // Surround: the game stops testing a flag once it is set; the tests are pure, so a branch-free OR is the same.
        scan.surround_out |= (s.DistanceGapAbs1 >= a.distance_gap_abs) & (a.lane_gap >= 0.0) & (s.LaneGap1 >= a.lane_gap);
        scan.surround_ahead |= (a.distance_gap > 0.0) & (s.DistanceGap2 >= a.distance_gap) & (s.LaneGapAbs2 >= a.lane_gap_abs);
        scan.surround_behind |= (0.0 > a.distance_gap) & (a.distance_gap >= -s.DistanceGap2) & (s.LaneGapAbs2 >= a.lane_gap_abs);
        // Near: branch-free store (count <= k < len, and entries past the count are never read).
        near_horses[scan.near_horse_count as usize] = k;
        scan.near_horse_count += (!(a.distance_gap_abs > p.Near.DistanceThreshold) & !(a.lane_gap_abs > p.Near.LaneDistanceThreshold)) as i32;
        // Compete-fight candidates: close, side by side, and either horse on the final straight.
        let lane_ok = p.CompeteFight.LaneGap >= a.lane_gap_abs;
        let dist_far = p.CompeteFight.DistanceGap < a.distance_gap_abs;
        let straight = self_straight_last || o.is_straight_last;
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
    (h.ai.is_surround_out, h.ai.is_surround_ahead, h.ai.is_surround_behind) = (scan.surround_out, scan.surround_ahead, scan.surround_behind);
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
    surround_out: bool,
    surround_ahead: bool,
    surround_behind: bool,
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

/// Front / side block timers, from the start-of-frame scan.
pub fn update_block_time(race: &mut Race, i: usize, dt: f32) {
    let front = block_horse_front(&race.horses[i]);
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
    let f = &race.horses[horse_by_order(race, cur - 1)];
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
        let b = &race.horses[horse_by_order(race, cur + 1)];
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
        let b = &race.horses[horse_by_order(race, cur + 1)];
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
pub fn block_horse_front(h: &Horse) -> Option<usize> {
    let ai = &h.ai;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::test_util::ai_race;

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
        let mut race = ai_race(6);
        place(&mut race, 0, 100.0, 0.30);
        place(&mut race, 1, 101.0, 0.30); // front, 1 m: front block
        place(&mut race, 2, 101.0, 0.31); // front, same gap: loses the tie to horse 1 (strict <)
        place(&mut race, 3, 100.5, 0.25); // inside, 0.5 m: side block in, in-space 0.05
        place(&mut race, 4, 99.5, 0.20); // inside, -0.5 m: later side-block-in match wins; space stays 0.05
        place(&mut race, 5, 98.0, 0.30); // behind beyond 1.05 m: isBack, ignored except near
        snapshot_scan_view(&mut race);
        update_around_horses_param(&mut race, 0, 0.0666);
        let ai = &race.horses[0].ai;
        assert_eq!(block_horse_front(&race.horses[0]), Some(1));
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
