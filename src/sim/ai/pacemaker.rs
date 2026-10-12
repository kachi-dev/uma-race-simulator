//! Pacemaker selection: the horse that position keep measures everyone's distance against.

use crate::sim::race::Race;

#[derive(Clone, Copy, Debug, Default)]
pub struct Pacemaker {
    pub horse: Option<usize>,
    /// From the July 2026 update on ([`update`]): the candidate's section at the last re-pick, the section pick, and the
    /// number of consecutive re-picks that saw an overall leader not of the most-forward style.
    pub section: i32,
    pub section_pick: Option<usize>,
    pub not_most_forward_count: i32,
    /// Under the leader override: the leader pacemaker's section when it was last re-checked.
    pub leader_section: i32,
}

impl Pacemaker {
    /// The pacemaker; horse 0 only before the first `update`.
    pub fn current(&mut self) -> usize {
        *self.horse.get_or_insert(0)
    }
}

/// Before the 2026-07 update: the top-order horse of the most-forward running style (as cached at the end of the
/// previous horse update), re-picked every frame.
///
/// From the 2026-07 update on:
/// - The pick is re-taken only when that candidate enters a new section, from the gate frame (section 1) up to
///   `PositionKeepEndSection`; in between the pacemaker is kept.
/// - Each re-pick that sees an overall leader not of the most-forward style counts +1 (else the count resets) and
///   keeps the old section pick. While the count is >= `PacemakerChangeTopHorseNotMostForwardStyleCount` and such a
///   horse leads, the leader is the pacemaker; once a most-forward-style horse leads again (checked every frame), the
///   section pick is. Between re-picks a leader pacemaker hands over to a new leader only when the pacemaker enters a
///   new section (fitted on one race, where the new leader entered it on the same frame, so tracking the leader's
///   section instead fits equally).
///
/// The `PacemakerChange*Range` params are not used.
pub fn update(race: &mut Race) {
    let most = race.field.most_forward_running_style;
    let Some(cand) = race.field.running_style_top_order_horses[most as usize] else { return };
    if race.data.era.pre_2026_07_courses {
        race.field.pacemaker.horse = Some(cand);
        return;
    }
    let lead = crate::sim::field::horse_at_order(race, 0);
    let not_most = lead.is_some_and(|l| race.horses[l].horse_data.running_style != most);
    let sec = crate::sim::horse::calc_section(race, cand);
    let kp = &race.data.param.PositionKeep;
    let count = kp.PacemakerChangeTopHorseNotMostForwardStyleCount;
    let pmc = &mut race.field.pacemaker;
    let repick = pmc.horse.is_none() || sec > pmc.section && sec <= kp.PositionKeepEndSection;
    if repick {
        pmc.section = sec;
        pmc.not_most_forward_count = if not_most { pmc.not_most_forward_count + 1 } else { 0 };
        if pmc.section_pick.is_none() || !not_most {
            pmc.section_pick = Some(cand);
        }
    }
    let leader_override = not_most && pmc.not_most_forward_count as f32 >= count;
    let prev = pmc.horse;
    pmc.horse = if leader_override { lead } else { pmc.section_pick };
    // Between re-picks, a leader pacemaker hands over to the current leader only on entering a new section.
    if let (true, false, Some(p)) = (leader_override, repick, prev) {
        if prev != race.field.pacemaker.section_pick {
            let sec = crate::sim::horse::calc_section(race, p);
            let pmc = &mut race.field.pacemaker;
            if sec > pmc.leader_section {
                pmc.leader_section = sec;
            } else {
                pmc.horse = prev;
            }
        }
    }
    if repick && race.field.pacemaker.horse != prev {
        if let Some(h) = race.field.pacemaker.horse {
            race.field.pacemaker.leader_section = crate::sim::horse::calc_section(race, h);
        }
    }
}
