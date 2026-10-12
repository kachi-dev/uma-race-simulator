//! Race recording: frames, events and results → `api::SimOutput`.

use crate::sim::race::Race;
use crate::sim::ai::{self, temptation};
use crate::api::{event_kind, span_kind, Event, Frame, HorseFrame, HorseResult, SimOutput, Span};

/// Recorded race data, un-quantized.
#[derive(Clone, Debug, Default)]
pub struct Recorder {
    /// Accumulates DT on its own (same values as `Race::accumulate_time_since_start`).
    pub cur_time: f32,
    pub frames: Vec<Frame>,
    pub events: Vec<Event>,
    pub results: Vec<HorseResult>,
    pub distance_diff_max: f32,
    pub spans: Vec<Span>,
    /// Per horse and `span_kind`: start time of the open span.
    pub open_spans: Vec<[Option<f32>; span_kind::COUNT]>,
}

/// Opens / closes `spans` from each horse's state after a step. Read-only on the race.
pub fn record_spans(race: &mut Race) {
    let t = race.recorder.cur_time;
    let n = race.horses.len();
    race.recorder.open_spans.resize(n, [None; span_kind::COUNT]);
    for i in 0..n {
        let h = &race.horses[i];
        let on = !h.is_finished;
        let state = [
            on && h.is_compete_fight,
            on && h.is_compete_top,
            on && h.ai.temptation.is_temptation,
            on && h.ai.block_horse.is_some(),
            on && h.ai.conserve_power.accel_value > 0.0,
        ];
        for (kind, &s) in state.iter().enumerate() {
            match (race.recorder.open_spans[i][kind], s) {
                (None, true) => race.recorder.open_spans[i][kind] = Some(t),
                (Some(start), false) => {
                    race.recorder.spans.push(Span { horse: i, kind: kind as u8, start, end: t });
                    race.recorder.open_spans[i][kind] = None;
                }
                _ => {}
            }
        }
    }
}

pub fn record(race: &mut Race) {
    let horses = (0..race.horses.len())
        .map(|i| {
            let h = &race.horses[i];
            HorseFrame {
                distance: h.distance,
                lane_position: h.lane_distance,
                speed: h.last_speed,
                hp: h.hp,
                temptation_mode: temptation::temptation_mode(race, i) as i8,
                block_front_horse_index: ai::block_horse_front(race, i).map_or(-1, |j| j as i8),
            }
        })
        .collect();
    race.recorder.frames.push(Frame { time: race.recorder.cur_time, horses });
}

pub fn record_race_result(race: &mut Race) {
    race.recorder.results = race
        .horses
        .iter()
        .map(|h| HorseResult {
            finish_order: h.finish_order,
            finish_time_raw: h.finish_time_raw,
            finish_time_scaled: h.finish_time_scaled,
            finish_diff_time: h.finish_time_diff_from_prev_horse,
            start_delay_time: h.delay_time_saved,
            last_spurt_start_distance: h.ai.last_spurt_start_distance,
            guts_order: h.guts_order,
            wiz_order: h.wiz_order,
            // The entry style, not a rushed-changed one.
            running_style: h.horse_data.running_style as i32,
            defeat: h.defeat,
        })
        .collect();
    race.recorder.distance_diff_max = race.distance_diff_max;
}

/// Skill activation event, params `[horseIndex, skillId, continueTimeInt, detailIndex, targetFlags, activateType]`.
pub fn add_skill_event(race: &mut Race, horse_index: usize, skill_id: i32, detail_index: i32, target_flags: i32, cur_time: f32, continue_time_int: i32, activate_type: i32) {
    let mut params = vec![horse_index as i32, skill_id, continue_time_int, detail_index, target_flags, activate_type];
    // Before the July 2026 update, skill events carry no activateType.
    if race.data.era.pre_2026_07_courses {
        params.pop();
    }
    race.recorder.events.push(Event { time: cur_time, kind: event_kind::SKILL, params });
}

/// Spot struggle, dueling or conserve-power release event, params `[horseIndex]`.
pub fn add_horse_event(race: &mut Race, kind: u8, horse_index: usize, cur_time: f32) {
    race.recorder.events.push(Event { time: cur_time, kind, params: vec![horse_index as i32] });
}

pub fn into_output(race: Race) -> SimOutput {
    SimOutput { frames: race.recorder.frames, results: race.recorder.results, events: race.recorder.events, spans: race.recorder.spans }
}
