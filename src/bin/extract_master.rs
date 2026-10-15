//! Extract the master.mdb tables the sim reads into data/master_sim.db (the game's full master.mdb is not distributed),
//! and the display text Ulala needs into data/names.json next to it.
//!   cargo run --release --bin extract_master -- <path to the game's master.mdb> [out]

use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(src) = args.first() else {
        eprintln!("usage: extract_master <master.mdb> [out, default {}/master_sim.db]", uma_race_sim::data::DATA_DIR);
        std::process::exit(1);
    };
    let out = args.get(1).map_or_else(|| Path::new(uma_race_sim::data::DATA_DIR).join("master_sim.db"), PathBuf::from);
    let names = out.with_file_name("names.json");
    if let Err(e) = uma_race_sim::data::master::extract(Path::new(src), &out).and_then(|_| write_names(Path::new(src), &names)) {
        eprintln!("extract failed: {e}");
        std::process::exit(1);
    }
    println!("wrote {} and {}", out.display(), names.display());
}

/// names.json, keys in field order. Maps are by id (BTreeMap: numeric order).
#[derive(Serialize)]
struct Names {
    /// By card_id: [title, chara_id, unique_skill_id, running_style, 10 max-rarity aptitudes].
    card: BTreeMap<i64, Value>,
    /// By chara_id.
    chara: BTreeMap<i64, String>,
    /// Champions Meetings, newest first: [number, name, start date, course_set, race_instance, ground, weather, season, time].
    cm: Vec<Value>,
    /// By race_instance id.
    race: BTreeMap<i64, String>,
    /// By skill id: [name, desc, rarity, icon_id, disp_order].
    skill: BTreeMap<i64, Value>,
    /// By skill id: SP to learn it with no hints, its group's lower tiers included (a gold also pays for the white it
    /// replaces, ◎ for ○). Skills that can't be bought (uniques) are absent.
    skill_sp: BTreeMap<i64, i64>,
    /// By race_track_id.
    track: BTreeMap<i64, String>,
}

const APT: [&str; 10] = [
    "distance_short", "distance_mile", "distance_middle", "distance_long", "running_style_nige", "running_style_senko",
    "running_style_sashi", "running_style_oikomi", "ground_turf", "ground_dirt",
];

fn write_names(src: &Path, dst: &Path) -> Result<(), String> {
    let e = |e: rusqlite::Error| e.to_string();
    let c = Connection::open_with_flags(src, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(e)?;
    let text = |cat: i32| -> Result<BTreeMap<i64, String>, String> {
        let mut st = c.prepare("SELECT `index`, text FROM text_data WHERE category = ?1").map_err(e)?;
        st.query_map([cat], |r| Ok((r.get(0)?, r.get(1)?))).map_err(e)?.collect::<Result<_, _>>().map_err(e)
    };
    let (chara, title, desc) = (text(6)?, text(5)?, text(48)?);

    let mut st = c.prepare("SELECT id, rarity, icon_id, disp_order FROM skill_data").map_err(e)?;
    let sd: BTreeMap<i64, [i64; 3]> =
        st.query_map([], |r| Ok((r.get(0)?, [r.get(1)?, r.get(2)?, r.get(3)?]))).map_err(e)?.collect::<Result<_, _>>().map_err(e)?;
    let skill = text(47)?
        .into_iter()
        .map(|(i, n)| {
            let [rarity, icon, order] = sd.get(&i).copied().unwrap_or_default();
            (i, json!([n, desc.get(&i).map_or("", |d| d.as_str()), rarity, icon, order]))
        })
        .collect();

    let mut st = c
        .prepare(
            "SELECT s.id, n.need_skill_point + COALESCE((SELECT SUM(n2.need_skill_point) FROM skill_data s2
                JOIN single_mode_skill_need_point n2 ON n2.id = s2.id
                WHERE s2.group_id = s.group_id AND s2.group_rate > 0 AND s2.group_rate < s.group_rate), 0)
            FROM skill_data s JOIN single_mode_skill_need_point n ON n.id = s.id",
        )
        .map_err(e)?;
    let skill_sp = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).map_err(e)?.collect::<Result<_, _>>().map_err(e)?;

    let apt = APT.map(|a| format!("proper_{a}")).join(", ");
    let mut top = c.prepare(&format!("SELECT skill_set, {apt} FROM card_rarity_data WHERE card_id = ?1 ORDER BY rarity DESC LIMIT 1")).map_err(e)?;
    let mut set = c.prepare("SELECT * FROM skill_set WHERE id = ?1").map_err(e)?;
    let mut st = c.prepare("SELECT id, chara_id, running_style FROM card_data ORDER BY id").map_err(e)?;
    let cards: Vec<(i64, i64, i64)> = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).map_err(e)?.collect::<Result<_, _>>().map_err(e)?;
    let mut card = BTreeMap::new();
    for (cid, chid, style) in cards {
        // Skips cards with no rarity rows (e.g. 9100101).
        let Ok(t) = top.query_row([cid], |r| (0..11).map(|i| r.get::<_, i64>(i)).collect::<Result<Vec<_>, _>>()) else { continue };
        if !chara.contains_key(&chid) {
            continue;
        }
        // skill_set is id, skill_id1, skill_level1, skill_id2, ...; unique skills are 1xxxxx.
        let ids: Vec<i64> = set
            .query_row([t[0]], |r| (1..r.as_ref().column_count()).step_by(2).map(|i| r.get(i)).collect())
            .unwrap_or_default();
        let unique = ids.into_iter().find(|s| (100000..200000).contains(s)).unwrap_or(0);
        let mut row = vec![json!(title.get(&cid).map_or("", |t| t.as_str())), json!(chid), json!(unique), json!(style)];
        row.extend(t[1..].iter().map(|&a| json!(a)));
        card.insert(cid, Value::Array(row));
    }

    let mut st = c
        .prepare(
            "SELECT s.id, (SELECT text FROM text_data WHERE category = 206 AND `index` = s.id), date(s.start_date, 'unixepoch'),
                r.course_set, ri.id, rc.ground, rc.weather, rc.season, ri.time FROM champions_schedule s
            JOIN champions_race_condition cr ON cr.champions_id = s.id AND cr.round_id = 0
            JOIN race_instance ri ON ri.id = cr.race_instance_id JOIN race r ON r.id = ri.race_id
            JOIN race_condition rc ON rc.id = cr.race_condition_id ORDER BY s.start_date DESC",
        )
        .map_err(e)?;
    let cm = st
        .query_map([], |r| {
            let i = |n| r.get::<_, i64>(n);
            Ok(json!([i(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<String>>(2)?, i(3)?, i(4)?, i(5)?, i(6)?, i(7)?, i(8)?]))
        })
        .map_err(e)?
        .collect::<Result<_, _>>()
        .map_err(e)?;

    let names = Names { card, chara, cm, race: text(28)?, skill, skill_sp, track: text(35)? };
    std::fs::write(dst, serde_json::to_string(&names).map_err(|e| e.to_string())?).map_err(|e| format!("{}: {e}", dst.display()))
}
