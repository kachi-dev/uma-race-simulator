//! The race simulator: a race, frame by frame, bit-exact with the game.
//!
//! # Modules
//!
//! ```text
//! tick.rs        the race loop and one frame, in the game's order: read this first
//! race.rs        race state and setup
//! field.rs       all horses together: overlap, finish order, end-of-frame caches, order queries
//! horse.rs       one horse's state, placement, phase, finish
//! record.rs      output frames, results and events
//! course/        corners, straights, slopes, course events; lanes.rs: lane geometry, world positions
//! physics/       stats → race params, target speed and accel, HP, last spurt, slopes, conserve power
//! ai/            surrounding horses, blocking, order, overtake, lane choice, position keep, pacemaker, rushed
//!                (temptation), spot struggle (compete_top), dueling (compete_fight)
//! skill/         lifecycle; trigger (conditions), target, ability (effects), modifier, value
//! defs.rs        shared enums
//! util.rs        .NET float, rounding and sort semantics
//! ```
//!
//! # Exactness rules
//!
//! * **No snapshots.** Horses update one at a time in index order, and later horses see earlier horses'
//!   already-updated state within the same frame. So there is one [`race::Race`] (game data, the single RNG, all
//!   horses) and functions take `(race: &mut Race, i: usize)` and read other horses through `race.horses[j]`.
//! * **Call order is the contract.** The order of steps in the frame loop ([`tick`]) and of RNG draws defines the result.
//!   Never reorder reads/writes (or draws) to please the borrow checker; copy values out first instead.
//! * **Float semantics.** f32 in the game's expression order; .NET rounding/clamp/sort semantics only via [`util`].
//!   The game evaluates float expressions in double and rounds to f32 on stores, arguments and returns, so
//!   multi-op expressions are computed in f64 where noted.

pub mod ai;
pub mod course;
mod debug;
pub mod defs;
pub mod field;
pub mod horse;
pub mod physics;
pub mod race;
pub mod record;
pub mod skill;
pub mod tick;
#[cfg(test)]
mod test_util;
pub mod util;

use crate::api::{RaceInput, SimOutput};
use crate::data::GameData;

pub fn simulate(input: &RaceInput, data: &GameData) -> SimOutput {
    let mut race = race::init(input, data);
    tick::run(&mut race);
    record::into_output(race)
}
