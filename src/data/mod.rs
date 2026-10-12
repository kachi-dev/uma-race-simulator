//! Game data loaders.

pub mod corpus;
pub mod course;
pub mod master;
pub mod param;

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

pub use course::{CourseEvents, CourseLaneAnim, CourseParam, JsonFiles};
pub use master::{Master, SkillAbilityData, SkillData, SkillDetailData};
pub use param::ParamDefine;

/// Which data version applies to a race (skill data and courses changed over time). `Default` = current data.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct DataEra {
    /// Snapshot file in data/skill_history/ overlaid on master.mdb, or None = master.mdb as-is.
    pub skill_snapshot: Option<String>,
    /// Use data/courseeventparam_pre_2026-07 + data/coursepath_pre_2026-07.
    pub pre_2026_07_courses: bool,
}

/// Mirrors data/skill_history/eras.json (checked by a test): (from, to inclusive, snapshot).
/// The first era uses the 06-18 snapshot, not 02-19: the 03-13 team-total rework (2100xx skills) was already live in
/// February.
const SKILL_ERAS: &[(Option<&str>, Option<&str>, Option<&str>)] = &[
    (None, Some("20260218"), Some("2026-06-18-581abe20.json")),
    (Some("20260314"), Some("20260630"), Some("2026-06-18-581abe20.json")),
    (Some("20260703"), None, None),
];

/// The course event param / course path update shipped with the 06-30..07-03 skill rework.
const COURSE_UPDATE_DATE: &str = "20260703";

impl DataEra {
    /// `date` = "YYYYMMDD". Errors for dates between eras (no skill data for them).
    pub fn for_date(date: &str) -> Result<DataEra, String> {
        if date.len() != 8 || !date.bytes().all(|b| b.is_ascii_digit()) {
            return Err(format!("bad date {date:?}, want YYYYMMDD"));
        }
        let era = SKILL_ERAS
            .iter()
            .find(|(from, to, _)| from.is_none_or(|f| date >= f) && to.is_none_or(|t| date <= t))
            .ok_or_else(|| format!("date {date} falls between skill eras (eras.json); no verified skill data"))?;
        Ok(DataEra { skill_snapshot: era.2.map(str::to_string), pre_2026_07_courses: date < COURSE_UPDATE_DATE })
    }

    /// Current data (master.mdb skills, current courses).
    pub fn current() -> DataEra {
        DataEra { skill_snapshot: None, pre_2026_07_courses: false }
    }
}

/// Everything the sim needs that isn't per-race input (constants, master data, courses, paths, skills).
#[derive(Debug, Default)]
pub struct GameData {
    pub param: ParamDefine,
    pub master: Master,
    /// Era-effective skill table (master.mdb with the era's snapshot overlaid). Use this, not `master.skill_data`.
    pub skills: HashMap<i32, SkillData>,
    /// Key: race_course_set.id as a string. Loaded on first use.
    pub course_event_params: JsonFiles<CourseEvents>,
    /// Key: file stem, see `course::lane_path_name` / `course::round_lane_path_name`. Loaded on first use.
    pub course_paths: JsonFiles<CourseLaneAnim>,
    pub era: DataEra,
}

/// Game data dir (`race_paramdefine.json`, `master_sim.db`, `course*/`, `skill_history/`): `UMA_DATA_DIR` at build
/// time (e.g. `[env]` in `.cargo/config.toml`), else `<crate>/data`.
pub const DATA_DIR: &str = match option_env!("UMA_DATA_DIR") {
    Some(d) => d,
    None => concat!(env!("CARGO_MANIFEST_DIR"), "/data"),
};

impl GameData {
    pub fn load(data_dir: &Path, era: DataEra) -> Result<GameData, String> {
        let param_path = data_dir.join("race_paramdefine.json");
        let s = std::fs::read_to_string(&param_path).map_err(|e| format!("{}: {e}", param_path.display()))?;
        let param: ParamDefine = serde_json::from_str(&s).map_err(|e| format!("{}: {e}", param_path.display()))?;

        let master = Master::load(&data_dir.join("master_sim.db"))?;
        let skills = match &era.skill_snapshot {
            None => master.skill_data.clone(),
            Some(f) => {
                let p = data_dir.join("skill_history").join(f);
                let s = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
                let snap: HashMap<String, SnapSkill> = serde_json::from_str(&s).map_err(|e| format!("{}: {e}", p.display()))?;
                overlay_skills(&master.skill_data, &snap)?
            }
        };

        // Pre-July: current files first, then the pre versions on top. Courses missing from the old extraction
        // (added after Nov 2025) fall back to the current files.
        let dirs = |name: &str| {
            let mut d = vec![data_dir.join(name)];
            if era.pre_2026_07_courses {
                d.push(data_dir.join(format!("{name}_pre_2026-07")));
            }
            d
        };
        let course_event_params = JsonFiles::new(dirs("courseeventparam"))?;
        let course_paths = JsonFiles::new(dirs("coursepath"))?;
        Ok(GameData { param, master, skills, course_event_params, course_paths, era })
    }
}

/// alpha123/uma-tools skill_data.json entry. Field mapping in eras.json `_doc`.
#[derive(Deserialize)]
#[allow(non_snake_case)]
struct SnapSkill {
    alternatives: Vec<SnapAlt>,
    rarity: i32,
    /// Absent in old snapshots (≤ 2026-02) → master value kept.
    tags: Option<Vec<i32>>,
    wisdomCheck: i32,
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct SnapAlt {
    precondition: String,
    condition: String,
    baseDuration: i32,
    /// Absent in old snapshots → master value kept.
    durationScaling: Option<i32>,
    effects: Vec<SnapEffect>,
}

#[derive(Deserialize)]
struct SnapEffect {
    #[serde(rename = "type")]
    kind: i32,
    modifier: i32,
    /// Absent in old snapshots → master value kept.
    scaling: Option<i32>,
    target: i32,
}

/// Marks a slot unused (type 0) the way master.mdb does; usage columns are left as they are (unread for type 0).
fn clear_ability(ab: &mut SkillAbilityData) {
    ab.ability_type = 0;
    ab.float_ability_value = 0;
    ab.target_type = 0;
}

/// master.mdb rows with the snapshot's tracked fields overlaid. Untracked fields (cooldown, target_value,
/// level usage, additional activate, priority, group) stay from master, by slot position. Details / ability slots
/// the snapshot doesn't have are cleared (`clear_ability`). Master skills absent from the snapshot are kept unchanged.
fn overlay_skills(master: &HashMap<i32, SkillData>, snap: &HashMap<String, SnapSkill>) -> Result<HashMap<i32, SkillData>, String> {
    let mut out = master.clone();
    for (id, s) in snap {
        let id: i32 = id.parse().map_err(|_| format!("bad skill id {id:?}"))?;
        if s.alternatives.len() > 2 || s.alternatives.iter().any(|a| a.effects.len() > 3) {
            return Err(format!("skill {id}: more details/effects than master has slots"));
        }
        let row = out.entry(id).or_insert_with(|| SkillData { id, ..Default::default() });
        row.rarity = s.rarity;
        if let Some(tags) = &s.tags {
            row.tag_id = tags.iter().map(|t| t.to_string()).collect::<Vec<_>>().join("/");
        }
        row.activate_lot = s.wisdomCheck;
        for (d, detail) in row.details.iter_mut().enumerate() {
            let Some(a) = s.alternatives.get(d) else {
                detail.precondition.clear();
                detail.condition.clear();
                detail.float_ability_time = 0;
                detail.abilities.iter_mut().for_each(clear_ability);
                continue;
            };
            detail.precondition = a.precondition.clone();
            detail.condition = a.condition.clone();
            detail.float_ability_time = a.baseDuration;
            if let Some(x) = a.durationScaling {
                detail.ability_time_usage = x;
            }
            for (j, ab) in detail.abilities.iter_mut().enumerate() {
                match a.effects.get(j) {
                    Some(e) => {
                        ab.ability_type = e.kind;
                        ab.float_ability_value = e.modifier;
                        if let Some(x) = e.scaling {
                            ab.ability_value_usage = x;
                        } else {
                            // The old snapshot format (2026-02-19, no longer used by any era) has no `scaling`: a
                            // missing one is direct (1), not master's. A missing `durationScaling` keeps master's.
                            ab.ability_value_usage = 1;
                        }
                        ab.target_type = e.target;
                    }
                    None => clear_ability(ab),
                }
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn era_for_date() {
        assert_eq!(DataEra::for_date("20260217").unwrap().skill_snapshot.as_deref(), Some("2026-06-18-581abe20.json"));
        assert_eq!(DataEra::for_date("20260630").unwrap(), DataEra { skill_snapshot: Some("2026-06-18-581abe20.json".into()), pre_2026_07_courses: true });
        assert_eq!(DataEra::for_date("20260814").unwrap(), DataEra::current());
        assert!(DataEra::for_date("20260301").is_err());
    }

    #[test]
    fn eras_table_matches_eras_json() {
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(format!("{DATA_DIR}/skill_history/eras.json")).unwrap()).unwrap();
        let eras = v["eras"].as_array().unwrap();
        assert_eq!(eras.len(), SKILL_ERAS.len());
        let d = |x: &serde_json::Value| x.as_str().map(|s| s.replace('-', ""));
        for (e, (from, to, snap)) in eras.iter().zip(SKILL_ERAS) {
            assert_eq!(d(&e["from"]), from.map(str::to_string));
            assert_eq!(d(&e["to"]), to.map(str::to_string));
            let s = e["skills"].as_str().unwrap();
            assert_eq!(if s == "master.mdb" { None } else { Some(s) }, *snap);
        }
    }

    #[test]
    fn load_and_overlay() {
        let dir = Path::new(DATA_DIR);
        let g = GameData::load(dir, DataEra::current()).unwrap();
        assert_eq!(g.param.Global.FinishTimeCoef, 1.18f32);
        assert_eq!(g.param.startDelayMax, 0.1f32);
        assert_eq!(g.master.race_motivation_rate[&5], 10400);
        assert_eq!(g.master.race_course_set[&11103].distance, 2000);
        assert_eq!(g.master.skill_data.len(), 732);
        assert_eq!(g.course_event_params.get("10101").unwrap().courseParams[1]._values, vec![4, 259]);
        let cs = &g.master.race_course_set[&11103];
        let name = course::lane_path_name(cs.race_track_id, cs.distance, cs.ground, cs.inout, 0);
        assert_eq!(g.course_paths.get(&name).unwrap().Distance, 2000);

        // The 2026-10-08 snapshot equals master.mdb (eras.json), so overlaying it must be the identity.
        let s = std::fs::read_to_string(dir.join("skill_history/2026-10-08-c330bbb7.json")).unwrap();
        let snap: HashMap<String, SnapSkill> = serde_json::from_str(&s).unwrap();
        let o = overlay_skills(&g.master.skill_data, &snap).unwrap();
        for (id, row) in &g.master.skill_data {
            assert_eq!(&o[id], row, "skill {id}");
        }

        // Old era: the pre-rework corner length (10101 2nd corner 275 m) and an old-snapshot skill.
        let old = GameData::load(dir, DataEra::for_date("20260625").unwrap()).unwrap();
        assert_eq!(old.course_event_params.get("10101").unwrap().courseParams[1]._values, vec![4, 275]);
        assert_eq!(old.skills[&100011].tag_id, "401/403");
    }
}
