//! Spot struggle (CompeteTop): front runners that start close together race each other for the lead.

use crate::api::event_kind;
use crate::sim::race::Race;
use crate::sim::physics::{conserve_power, stats};

#[derive(Clone, Copy, Debug, Default)]
pub struct CompeteTop {
    /// Groups started so far, per style: [Nige, Oonige] (limited by `NigeCount` / `OonigeCount`).
    pub start_count: [i32; 2],
}

/// Spot struggle update (after the overlap update): starts new groups, then counts down and ends running ones.
pub fn update(race: &mut Race, dt: f32) {
    let p = &race.data.param.CompeteTop;
    let n = race.horses.len();
    if race.horses.iter().any(|h| h.distance >= p.CheckStartDistance) {
        start_groups(race);
    }
    // A horse leaves early when no other horse of its group is within DistanceGap2 / LaneGap2. Membership counts,
    // not the active flag: members that already left still keep a partner in.
    let near = |race: &Race, i: usize| {
        let h = &race.horses[i];
        (0..n).any(|j| {
            let o = &race.horses[j];
            j != i && o.compete_top_group == h.compete_top_group
                && (o.distance - h.distance).abs() <= p.DistanceGap2 && (o.lane_distance - h.lane_distance).abs() <= p.LaneGap2
        })
    };
    // The section end is per group: all members end once any member active at the start of this update is in
    // section >= EndSection.
    let ended: Vec<i32> = (0..n)
        .filter(|&i| race.horses[i].is_compete_top && crate::sim::horse::calc_section(race, i) >= p.EndSection)
        .map(|i| race.horses[i].compete_top_group)
        .collect();
    for i in 0..n {
        if !race.horses[i].is_compete_top {
            continue;
        }
        if !near(race, i) {
            race.horses[i].is_compete_top = false;
            continue;
        }
        // The countdown also runs on the start frame.
        let h = &mut race.horses[i];
        h.compete_top_remain_time -= dt;
        if h.compete_top_remain_time <= 0.0 || ended.contains(&h.compete_top_group) {
            h.is_compete_top = false;
        }
    }
}

/// Starts spot struggles once some horse has passed `CheckStartDistance`. Per style group (Oonige, then Nige, so a
/// shared start frame records the Oonige events first; at most `NigeCount` / `OonigeCount` groups): the members are
/// the group's top and every other group horse within `DistanceGap1` / `LaneGap1` of it; it starts when there are
/// >= 2 and one of them is in a section <= `CheckEndSection`. Records event 4 per member.
fn start_groups(race: &mut Race) {
    use crate::sim::defs::{RunningStyle, RunningStyleEx};
    let p = &race.data.param.CompeteTop;
    let n = race.horses.len();
    for (k, ex) in [(1, RunningStyleEx::Oonige), (0, RunningStyleEx::None)] {
        let limit = if k == 0 { p.NigeCount } else { p.OonigeCount };
        if race.field.compete_top.start_count[k] >= limit {
            continue;
        }
        // The top is the frontmost by last frame's order; its event comes first, then the others' by index.
        let in_group = |i: usize| {
            let h = &race.horses[i];
            h.horse_data.running_style == RunningStyle::Nige && h.horse_data.running_style_ex == ex && !h.is_finished
        };
        let Some(top) = crate::sim::field::running_style_horses(race, RunningStyle::Nige).iter().copied().find(|&i| in_group(i))
        else {
            continue;
        };
        // No start while the top is passed by a horse of a later style (Senko+; for Oonige also a plain Nige).
        let to = race.horses[top].cur_order;
        if race.horses.iter().any(|h| {
            h.cur_order < to && (h.horse_data.running_style > RunningStyle::Nige || (k == 1 && h.horse_data.running_style_ex != RunningStyleEx::Oonige))
        }) {
            continue;
        }
        let (td, tl) = (race.horses[top].distance, race.horses[top].lane_distance);
        let mut members = vec![top];
        members.extend((0..n).filter(|&i| {
            i != top && in_group(i) && (race.horses[i].distance - td).abs() < p.DistanceGap1 && (race.horses[i].lane_distance - tl).abs() < p.LaneGap1
        }));
        if members.len() < 2 || !members.iter().any(|&i| crate::sim::horse::calc_section(race, i) <= p.CheckEndSection) {
            continue;
        }
        race.field.compete_top.start_count[k] += 1;
        let t = race.recorder.cur_time;
        for &i in &members {
            let h = &mut race.horses[i];
            // Duration `(TimeCoef1 * Guts)^TimeCoef2 * TimeCoef3 * style aptitude`, with the live Guts.
            let guts = crate::sim::physics::stats::guts(h);
            h.is_compete_top = true;
            h.compete_top_count += 1;
            h.compete_top_group = k as i32;
            h.compete_top_remain_time = crate::sim::util::mathf_pow(p.TimeCoef1 * guts, p.TimeCoef2) * p.TimeCoef3 * h.proper_running_style_coef;
            crate::sim::record::add_horse_event(race, event_kind::COMPETE_TOP, i, t);
            conserve_power::on_compete_top_start(race, i);
        }
    }
}

/// Target-speed add during a spot struggle, `(AddParam1Coef1 * Guts)^Coef2 * Coef3` with the live Guts.
/// `is_compete_top` is set late in the frame, so this applies from the next frame on.
pub fn add_target_speed(race: &Race, i: usize) -> f32 {
    let h = &race.horses[i];
    if !h.is_compete_top {
        return 0.0;
    }
    let p = &race.data.param.CompeteTop;
    crate::sim::util::mathf_pow(p.AddParam1Coef1 * stats::guts(h), p.AddParam1Coef2) * p.AddParam1Coef3
}
