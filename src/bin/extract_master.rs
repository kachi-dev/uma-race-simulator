//! Extract the master.mdb tables the sim reads into data/master_sim.db (the game's full master.mdb is not distributed).
//!   cargo run --release --bin extract_master -- <path to the game's master.mdb> [out]

use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(src) = args.first() else {
        eprintln!("usage: extract_master <master.mdb> [out, default {}/master_sim.db]", uma_race_sim::data::DATA_DIR);
        std::process::exit(1);
    };
    let out = args.get(1).map_or_else(|| Path::new(uma_race_sim::data::DATA_DIR).join("master_sim.db"), PathBuf::from);
    if let Err(e) = uma_race_sim::data::master::extract(Path::new(src), &out) {
        eprintln!("extract failed: {e}");
        std::process::exit(1);
    }
    println!("wrote {}", out.display());
}
