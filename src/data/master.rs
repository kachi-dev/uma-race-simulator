//! master.mdb tables the sim reads.
//! Values are the raw master integers; the sim does the `(float)v * 0.0001f` conversions itself.

use std::collections::HashMap;
use std::path::Path;

use rusqlite::{Connection, OpenFlags, Row};

#[derive(Clone, Debug, Default)]
pub struct RaceCourseSet {
    pub id: i32,
    pub race_track_id: i32,
    pub distance: i32,
    /// 1 Turf, 2 Dirt.
    pub ground: i32,
    pub inout: i32,
    /// 1 Right, 2 Left, 3 StraightRight, 4 StraightLeft.
    pub turn: i32,
    pub fence_set: i32,
    pub float_lane_max: i32,
    pub course_set_status_id: i32,
    pub finish_time_min: i32,
    pub finish_time_min_random_range: i32,
    pub finish_time_max: i32,
    pub finish_time_max_random_range: i32,
}

#[derive(Clone, Debug, Default)]
pub struct RaceTrack {
    pub id: i32,
    pub initial_lane_type: i32,
    pub enable_half_gate: i32,
    pub horse_num_gate_variation: i32,
    pub turf_vision_type: i32,
    pub board_condition_type: i32,
}

/// `race` row, sim-relevant columns only (grade / race id skill conditions).
#[derive(Clone, Debug, Default)]
pub struct Race {
    pub id: i32,
    pub group: i32,
    pub grade: i32,
    pub course_set: i32,
    pub goal_gate: i32,
    pub goal_flower: i32,
    pub entry_num: i32,
    pub is_dirtgrade: i32,
}

#[derive(Clone, Debug, Default)]
pub struct RaceInstance {
    pub id: i32,
    pub race_id: i32,
    pub npc_group_id: i32,
    pub date: i32,
    /// Time of day, as `RaceInput::time`.
    pub time: i32,
    pub clock_time: i32,
    pub race_number: i32,
}

/// `race_proper_distance_rate` row.
#[derive(Clone, Copy, Debug, Default)]
pub struct ProperDistanceRate {
    pub proper_rate_speed: i32,
    pub proper_rate_power: i32,
}

/// `race_course_set_status` row.
#[derive(Clone, Copy, Debug, Default)]
pub struct CourseSetStatus {
    pub target_status_1: i32,
    pub target_status_2: i32,
}

/// `race_single_mode_team_status` row (team stat bonus). Kept in id order.
#[derive(Clone, Copy, Debug, Default)]
pub struct TeamStatus {
    pub id: i32,
    pub team_rank_threshold: i32,
    pub add_status: i32,
}

/// `skill_level_value` row. Kept in id order.
#[derive(Clone, Copy, Debug, Default)]
pub struct SkillLevelValue {
    pub id: i32,
    pub ability_type: i32,
    pub level: i32,
    pub float_ability_value_coef: i32,
}

/// One `ability_*_N_M` slot of a skill detail. `ability_type == 0` = unused slot.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SkillAbilityData {
    pub ability_type: i32,
    pub ability_value_usage: i32,
    pub additional_activate_type: i32,
    pub ability_value_level_usage: i32,
    pub float_ability_value: i32,
    pub target_type: i32,
    pub target_value: i32,
}

/// One detail (`*_1` / `*_2` columns) of a skill.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SkillDetailData {
    pub precondition: String,
    pub condition: String,
    pub float_ability_time: i32,
    pub ability_time_usage: i32,
    pub float_cooldown_time: i32,
    pub abilities: [SkillAbilityData; 3],
}

/// `skill_data` row, the columns race code reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SkillData {
    pub id: i32,
    pub rarity: i32,
    pub group_id: i32,
    pub group_rate: i32,
    pub skill_category: i32,
    /// '/'-separated tag ids, e.g. "401/403".
    pub tag_id: String,
    pub activate_lot: i32,
    pub priority: i32,
    pub is_general_skill: i32,
    pub details: [SkillDetailData; 2],
}

#[derive(Clone, Debug, Default)]
pub struct Master {
    pub race_course_set: HashMap<i32, RaceCourseSet>,
    pub race_track: HashMap<i32, RaceTrack>,
    pub race: HashMap<i32, Race>,
    pub race_instance: HashMap<i32, RaceInstance>,
    /// Key: proficiency grade 0..=8.
    pub race_proper_distance_rate: HashMap<i32, ProperDistanceRate>,
    pub race_proper_runningstyle_rate: HashMap<i32, i32>,
    pub race_proper_ground_rate: HashMap<i32, i32>,
    /// Key: motivation 1..=5 → motivation_rate.
    pub race_motivation_rate: HashMap<i32, i32>,
    /// Key: course_set_status_id.
    pub race_course_set_status: HashMap<i32, CourseSetStatus>,
    pub race_single_mode_team_status: Vec<TeamStatus>,
    pub skill_level_value: Vec<SkillLevelValue>,
    /// master.mdb skill_data (current era); `GameData::skills` holds the era-effective table.
    pub skill_data: HashMap<i32, SkillData>,
}

fn query<T>(c: &Connection, sql: &str, f: impl Fn(&Row) -> rusqlite::Result<T>) -> Result<Vec<T>, String> {
    let mut st = c.prepare(sql).map_err(|e| format!("{sql}: {e}"))?;
    let rows = st.query_map([], |r| f(r)).map_err(|e| format!("{sql}: {e}"))?;
    rows.collect::<rusqlite::Result<Vec<T>>>().map_err(|e| format!("{sql}: {e}"))
}

/// `skill_data` columns in the order `skill_row` reads them (positional reads: a by-name `Row::get` scans all ~100
/// columns per lookup, which made this query ~25 ms).
fn skill_columns() -> String {
    let mut c: Vec<String> = ["id", "rarity", "group_id", "group_rate", "skill_category", "tag_id", "activate_lot", "priority", "is_general_skill"]
        .map(String::from)
        .into();
    for n in 1..=2 {
        for f in ["precondition", "condition", "float_ability_time", "ability_time_usage", "float_cooldown_time"] {
            c.push(format!("{f}_{n}"));
        }
        for m in 1..=3 {
            for f in ["ability_type", "ability_value_usage", "additional_activate_type", "ability_value_level_usage", "float_ability_value", "target_type", "target_value"] {
                c.push(format!("{f}_{n}_{m}"));
            }
        }
    }
    c.join(", ")
}

fn skill_row(r: &Row) -> rusqlite::Result<SkillData> {
    // Struct fields are evaluated in source order, which is the `skill_columns` order.
    let k = std::cell::Cell::new(0);
    let next = || k.replace(k.get() + 1);
    let g = || r.get::<_, i32>(next());
    let s = || r.get::<_, String>(next());
    let detail = || -> rusqlite::Result<SkillDetailData> {
        let ab = || -> rusqlite::Result<SkillAbilityData> {
            Ok(SkillAbilityData {
                ability_type: g()?,
                ability_value_usage: g()?,
                additional_activate_type: g()?,
                ability_value_level_usage: g()?,
                float_ability_value: g()?,
                target_type: g()?,
                target_value: g()?,
            })
        };
        Ok(SkillDetailData {
            precondition: s()?,
            condition: s()?,
            float_ability_time: g()?,
            ability_time_usage: g()?,
            float_cooldown_time: g()?,
            abilities: [ab()?, ab()?, ab()?],
        })
    };
    Ok(SkillData {
        id: g()?,
        rarity: g()?,
        group_id: g()?,
        group_rate: g()?,
        skill_category: g()?,
        tag_id: s()?,
        activate_lot: g()?,
        priority: g()?,
        is_general_skill: g()?,
        details: [detail()?, detail()?],
    })
}

/// The master.mdb tables and columns the sim reads. `extract` copies exactly these into `data/master_sim.db` (the
/// game's full master.mdb is not distributed).
const TABLES: [(&str, &str); 12] = [
    ("race_course_set", "id, race_track_id, distance, ground, inout, turn, fence_set, float_lane_max, course_set_status_id, finish_time_min, finish_time_min_random_range, finish_time_max, finish_time_max_random_range"),
    ("race_track", "id, initial_lane_type, enable_half_gate, horse_num_gate_variation, turf_vision_type, board_condition_type"),
    ("race", "id, \"group\", grade, course_set, goal_gate, goal_flower, entry_num, is_dirtgrade"),
    ("race_instance", "id, race_id, npc_group_id, date, time, clock_time, race_number"),
    ("race_proper_distance_rate", "id, proper_rate_speed, proper_rate_power"),
    ("race_proper_runningstyle_rate", "id, proper_rate"),
    ("race_proper_ground_rate", "id, proper_rate"),
    ("race_motivation_rate", "id, motivation_rate"),
    ("race_course_set_status", "course_set_status_id, target_status_1, target_status_2"),
    ("race_single_mode_team_status", "id, team_rank_threshold, add_status"),
    ("skill_level_value", "id, ability_type, level, float_ability_value_coef"),
    ("skill_data", ""), // skill_columns()
];

fn cols(table: &str) -> String {
    if table == "skill_data" { skill_columns() } else { TABLES.iter().find(|t| t.0 == table).unwrap().1.to_string() }
}

fn select(table: &str) -> String {
    format!("SELECT {} FROM {table}", cols(table))
}

/// Copies the tables the sim reads from the game's master.mdb (`src`) into a new SQLite file (`dst`).
pub fn extract(src: &Path, dst: &Path) -> Result<(), String> {
    let err = |e: rusqlite::Error| e.to_string();
    let _ = std::fs::remove_file(dst);
    let c = Connection::open(dst).map_err(err)?;
    c.execute("ATTACH DATABASE ?1 AS src", [src.to_string_lossy()]).map_err(err)?;
    for (t, _) in TABLES {
        c.execute_batch(&format!("CREATE TABLE {t} AS SELECT {} FROM src.{t}", cols(t))).map_err(err)?;
    }
    c.execute_batch("DETACH DATABASE src; VACUUM;").map_err(err)
}

impl Master {
    /// Opens `master_sim.db` (see `extract`; master.mdb itself also works) read-only and loads every table above.
    pub fn load(path: &Path) -> Result<Master, String> {
        let c = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let map = |v: Vec<(i32, i32)>| v.into_iter().collect::<HashMap<_, _>>();
        Ok(Master {
            race_course_set: query(&c, &select("race_course_set"), |r| {
                Ok(RaceCourseSet {
                    id: r.get(0)?, race_track_id: r.get(1)?, distance: r.get(2)?, ground: r.get(3)?, inout: r.get(4)?,
                    turn: r.get(5)?, fence_set: r.get(6)?, float_lane_max: r.get(7)?, course_set_status_id: r.get(8)?,
                    finish_time_min: r.get(9)?, finish_time_min_random_range: r.get(10)?, finish_time_max: r.get(11)?,
                    finish_time_max_random_range: r.get(12)?,
                })
            })?.into_iter().map(|x| (x.id, x)).collect(),
            race_track: query(&c, &select("race_track"), |r| {
                Ok(RaceTrack {
                    id: r.get(0)?, initial_lane_type: r.get(1)?, enable_half_gate: r.get(2)?,
                    horse_num_gate_variation: r.get(3)?, turf_vision_type: r.get(4)?, board_condition_type: r.get(5)?,
                })
            })?.into_iter().map(|x| (x.id, x)).collect(),
            race: query(&c, &select("race"), |r| {
                Ok(Race {
                    id: r.get(0)?, group: r.get(1)?, grade: r.get(2)?, course_set: r.get(3)?, goal_gate: r.get(4)?,
                    goal_flower: r.get(5)?, entry_num: r.get(6)?, is_dirtgrade: r.get(7)?,
                })
            })?.into_iter().map(|x| (x.id, x)).collect(),
            race_instance: query(&c, &select("race_instance"), |r| {
                Ok(RaceInstance {
                    id: r.get(0)?, race_id: r.get(1)?, npc_group_id: r.get(2)?, date: r.get(3)?, time: r.get(4)?,
                    clock_time: r.get(5)?, race_number: r.get(6)?,
                })
            })?.into_iter().map(|x| (x.id, x)).collect(),
            race_proper_distance_rate: query(&c, &select("race_proper_distance_rate"), |r| {
                Ok((r.get(0)?, ProperDistanceRate { proper_rate_speed: r.get(1)?, proper_rate_power: r.get(2)? }))
            })?.into_iter().collect(),
            race_proper_runningstyle_rate: map(query(&c, &select("race_proper_runningstyle_rate"), |r| Ok((r.get(0)?, r.get(1)?)))?),
            race_proper_ground_rate: map(query(&c, &select("race_proper_ground_rate"), |r| Ok((r.get(0)?, r.get(1)?)))?),
            race_motivation_rate: map(query(&c, &select("race_motivation_rate"), |r| Ok((r.get(0)?, r.get(1)?)))?),
            race_course_set_status: query(&c, &select("race_course_set_status"), |r| {
                Ok((r.get(0)?, CourseSetStatus { target_status_1: r.get(1)?, target_status_2: r.get(2)? }))
            })?.into_iter().collect(),
            race_single_mode_team_status: query(&c, &format!("{} ORDER BY id", select("race_single_mode_team_status")), |r| {
                Ok(TeamStatus { id: r.get(0)?, team_rank_threshold: r.get(1)?, add_status: r.get(2)? })
            })?,
            skill_level_value: query(&c, &format!("{} ORDER BY id", select("skill_level_value")), |r| {
                Ok(SkillLevelValue { id: r.get(0)?, ability_type: r.get(1)?, level: r.get(2)?, float_ability_value_coef: r.get(3)? })
            })?,
            skill_data: query(&c, &select("skill_data"), skill_row)?.into_iter().map(|x| (x.id, x)).collect(),
        })
    }
}
