//! Run the sim on a saved race and diff against its server replay.
//!   cargo run --release --bin compare -- <saved race.json> [data_dir]
//!   cargo run --release --bin compare -- --corpus <dir> [data_dir]     (all *.json under dir, recursively)

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use uma_race_sim::data::{DataEra, GameData, corpus};
use uma_race_sim::replay::{self, Divergence};

const DEFAULT_DATA_DIR: &str = uma_race_sim::data::DATA_DIR;

enum Outcome {
    /// Replay frame count.
    Match(usize, Vec<SpurtCmp>),
    Diverged(Divergence, ResultCmp, Vec<SpurtCmp>),
    Panicked(String),
    LoadError(String),
}

/// End-of-race outcome vs the replay, independent of where the frames first diverge.
struct ResultCmp {
    /// Every horse's finish order equal.
    order: bool,
    /// Same horse finished first.
    winner: bool,
    /// Max |sim - replay| finish_time_raw over horses (s).
    max_dt: f32,
    /// Replay frame count.
    frames: usize,
}

fn cmp_results(sim: &uma_race_sim::api::SimOutput, rep: &replay::Replay) -> ResultCmp {
    let pairs = || sim.results.iter().zip(&rep.results);
    ResultCmp {
        order: sim.results.len() == rep.results.len() && pairs().all(|(s, r)| s.finish_order == r.finish_order),
        winner: sim.results.iter().position(|r| r.finish_order == 0) == rep.results.iter().position(|r| r.finish_order == 0),
        max_dt: pairs().map(|(s, r)| (s.finish_time_raw - r.finish_time_raw).abs()).fold(0.0, f32::max),
        frames: rep.frames.len(),
    }
}

/// Last-spurt / HP summary of one horse, inferred the same way from sim (quantized) and replay frames + result records.
#[derive(Clone, Copy, Debug)]
struct SpurtHp {
    /// Recorded `LastSpurtStartDistance` (-1: no spurt).
    start: f32,
    /// Full spurt: max-speed spurt from the start of the final leg (see `spurt_hp`).
    full: bool,
    /// Integer HP in the first frame at/after the goal (HP stops changing at the finish).
    hp_goal: u16,
    /// Some frame up to the finish shows HP 0 (integer, so < 1 HP counts).
    hp_out: bool,
}

/// Per horse: sim and replay inference, plus the sim's internal truth (last-spurt calc result was "HP covers the max
/// spurt", HP ran out during the race).
#[derive(Clone, Copy, Debug)]
struct SpurtCmp {
    sim: SpurtHp,
    rep: SpurtHp,
    truth_full: bool,
    truth_out: bool,
}

/// Neither side records the spurt calc result or speed, so `full` redoes the calc's first test (HP >= the HP needed
/// at max spurt speed over the remaining distance) on the frames: the spurt
/// must start on entering the final leg (the calc's distance; within 2 steps of the phase-End distance), and the
/// HP there, extrapolated from the two frames before it (+0.5 for the integer truncation), must cover
/// `need_per_m` (input-derived, `need_hp_per_m`) times the remaining distance less the 60 m goal buffer.
/// The final-leg speed is no use: 1 s frames, skills, slopes and blocking hide the 0.1 m/s speed steps.
fn spurt_hp(frames: &[replay::ReplayFrame], res: &replay::ReplayResult, h: usize, goal: f32, end_start: f32, need_per_m: f32) -> SpurtHp {
    let start = res.last_spurt_start_distance;
    let before = |d: f32| frames.iter().map(|f| f.horses[h]).filter(move |x| x.distance < d);
    let hp_at_start = match before(start).collect::<Vec<_>>()[..] {
        [.., a, b] => b.hp as f32 + (b.hp as f32 - a.hp as f32) / (b.distance - a.distance) * (start - b.distance),
        [.., b] => b.hp as f32,
        [] => 0.0,
    };
    let finish = frames.iter().map(|f| f.horses[h]).find(|x| x.distance >= goal).unwrap_or(frames[frames.len() - 1].horses[h]);
    SpurtHp {
        start,
        // A 1/15 s step at < 26 m/s moves < 1.75 m.
        full: start >= end_start && start < end_start + 3.5 && hp_at_start + 0.5 >= need_per_m * (goal - start - 60.0).max(0.0),
        hp_goal: finish.hp,
        hp_out: before(goal).any(|x| x.hp == 0) || finish.hp == 0,
    }
}

/// Truth per horse: (full spurt, HP out, need HP per m at max spurt speed).
fn spurt_cmp(q: &replay::Replay, rep: &replay::Replay, truth: &[(bool, bool, f32)], goal: f32, end_start: f32) -> Vec<SpurtCmp> {
    if q.results.len() != rep.results.len() || q.horse_num != rep.horse_num {
        return Vec::new();
    }
    (0..q.results.len())
        .map(|h| SpurtCmp {
            sim: spurt_hp(&q.frames, &q.results[h], h, goal, end_start, truth[h].2),
            rep: spurt_hp(&rep.frames, &rep.results[h], h, goal, end_start, truth[h].2),
            truth_full: truth[h].0,
            truth_out: truth[h].1,
        })
        .collect()
}

/// Final-leg HP per metre at the horse's max last-spurt speed: the last-spurt calc on its phase-End params with
/// unlimited HP (the "HP covers it" path, no RNG draw) gives the max speed, then `calc_need_hp(v, 1 m)`.
/// Run on the finished race; the stats are the race-long ones.
fn need_hp_per_m(r: &mut uma_race_sim::sim::race::Race, i: usize) -> f32 {
    use uma_race_sim::sim::{defs::Phase, physics::hp, physics::last_spurt, physics::speed, physics::stats};
    let h = &r.horses[i];
    let p = last_spurt::LastSpurtCalcParam {
        cur_distance: r.info.phase_distances.phase_end_start_distance,
        hp: f32::MAX,
        phase: Phase::End,
        speed: stats::speed(h),
        guts: stats::guts(h),
        wiz: stats::wiz(h),
        base_target_speed: speed::base_target_speed_without_random(r, i, Phase::End),
        ground_modifier_multi_hp_sub: h.ground_modifier_param.multi_hp_sub,
        proper_distance_coef: h.proper_distance_coef_speed,
        course_distance: r.info.race_course_set.distance,
        race_base_speed: r.info.base_speed,
    };
    let mut calc = last_spurt::new_last_spurt(r, r.info.base_speed);
    calc.calc_param = p;
    last_spurt::calc_last_spurt(r, &mut calc);
    hp::calc_need_hp(r, p.race_base_speed, p.ground_modifier_multi_hp_sub, Phase::End, p.guts, calc.last_spurt_target_speed, 1.0)
}

fn run(path: &Path, data_dir: &Path, cache: &mut HashMap<DataEra, GameData>, verbose: bool) -> Outcome {
    let (input, rep, date) = match corpus::load_race(path) {
        Ok(x) => x,
        Err(e) => return Outcome::LoadError(e),
    };
    let era = match DataEra::for_date(&date) {
        Ok(e) => e,
        Err(e) => return Outcome::LoadError(e),
    };
    if !cache.contains_key(&era) {
        match GameData::load(data_dir, era.clone()) {
            Ok(g) => {
                cache.insert(era.clone(), g);
            }
            Err(e) => return Outcome::LoadError(e),
        }
    }
    let data = &cache[&era];
    let mut rep = rep;
    if verbose {
        println!(
            "{}\n  date {date} era {:?} seed {} course {} horses {} replay frames {} events {}",
            path.display(), era, input.random_seed, input.course_set_id, input.horses.len(), rep.frames.len(), rep.events.len()
        );
    }
    // = sim::simulate, keeping the internals the spurt/HP report validates against.
    let run_sim = || {
        use uma_race_sim::sim::{defs::last_spurt_calc_result, race, record, tick};
        let mut r = race::init(&input, data);
        tick::run(&mut r);
        let truth: Vec<_> = (0..r.horses.len())
            .map(|i| {
                let need = need_hp_per_m(&mut r, i);
                let h = &r.horses[i];
                (h.ai.last_spurt_calc_result == last_spurt_calc_result::TRUE_EXCEED_NEED_MAX_HP, h.is_hp_empty_on_race, need)
            })
            .collect();
        let pc = r.info.phase_distances;
        (record::into_output(r), truth, pc.course_distance, pc.phase_end_start_distance)
    };
    let (sim, truth, goal, end_start) = match catch_unwind(AssertUnwindSafe(run_sim)) {
        Ok(s) => s,
        Err(p) => {
            let msg = p.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| p.downcast_ref::<String>().cloned());
            return Outcome::Panicked(msg.unwrap_or_else(|| "panic".into()));
        }
    };
    if verbose {
        println!("  sim frames {} events {}", sim.frames.len(), sim.events.len());
    }
    // COMPARE_DUMP=a..b (or a): print sim vs replay frames a..b per horse (replay value only where it differs).
    if let (true, Ok(r)) = (verbose, std::env::var("COMPARE_DUMP")) {
        let (a, b) = r.split_once("..").map_or_else(|| (r.clone(), r.clone()), |(a, b)| (a.into(), b.into()));
        let q = replay::quantize(&sim);
        for k in a.parse().unwrap_or(0)..=b.parse().unwrap_or(0) {
            let (Some(x), Some(y)) = (q.frames.get(k), rep.frames.get(k)) else { break };
            println!("frame {k} t={}", y.time);
            for (h, (s, r)) in x.horses.iter().zip(&y.horses).enumerate() {
                let mark = if s == r { "" } else { "  <-- replay " };
                let raw = &sim.frames[k].horses[h]; // unquantized speed / HP (e.g. how far an HP point is from flipping)
                println!("  {h} {:?} {:?} {:?} {:?} {:?} {:?}{mark}{}   raw v {} hp {}", s.distance, s.lane_position, s.speed, s.hp, s.temptation_mode, s.block_front_horse_index,
                    if s == r { String::new() } else { format!("{:?} {:?} {:?} {:?} {:?} {:?}", r.distance, r.lane_position, r.speed, r.hp, r.temptation_mode, r.block_front_horse_index) },
                    raw.speed, raw.hp);
            }
        }
    }
    // COMPARE_EVENTS=1: print sim and replay events (time, kind, params); e.g. `| sort -k2 -n` to interleave.
    if verbose && std::env::var("COMPARE_EVENTS").is_ok() {
        sim.events.iter().for_each(|e| println!("sim    {:.4} k{} {:?}", e.time, e.kind, e.params));
        rep.events.iter().for_each(|e| println!("replay {:.4} k{} {:?}", e.time, e.kind, e.params));
    }
    // COMPARE_RESULTS=1: print sim vs replay result records.
    if verbose && std::env::var("COMPARE_RESULTS").is_ok() {
        let q = replay::quantize(&sim);
        for (h, (s, r)) in q.results.iter().zip(&rep.results).enumerate() {
            println!("result {h} sim    {s:?}\n         replay {r:?}");
        }
    }
    // COMPARE_EV=k: print the sim and replay events of kind k ("time:horse"), also in corpus mode.
    if let Some(k) = std::env::var("COMPARE_EV").ok().and_then(|k| k.parse::<u8>().ok()) {
        let ev = |es: &[uma_race_sim::api::Event]| {
            es.iter().filter(|e| e.kind == k).map(|e| format!("{}:{}", e.time, e.params.iter().map(|p| p.to_string()).collect::<Vec<_>>().join("/"))).collect::<Vec<_>>().join(" ")
        };
        let (s, r) = (ev(&sim.events), ev(&rep.events));
        if !s.is_empty() || !r.is_empty() {
            println!("EV{k} {} {}\n  sim {s}\n  rep {r}", if s == r { "SAME" } else { "DIFF" }, path.display());
        }
    }
    let mut q = replay::quantize(&sim);
    let spurt = spurt_cmp(&q, &rep, &truth, goal, end_start);
    // COMPARE_NO_HP=1: ignore the (truncated) HP column, to see the first position divergence.
    if std::env::var("COMPARE_NO_HP").is_ok() {
        q.frames.iter_mut().chain(rep.frames.iter_mut()).flat_map(|f| f.horses.iter_mut()).for_each(|h| h.hp = 0);
    }
    match replay::diff_quantized(&q, &rep) {
        None => Outcome::Match(rep.frames.len(), spurt),
        Some(d) => Outcome::Diverged(d, cmp_results(&sim, &rep), spurt),
    }
}

/// Corpus spurt/HP accuracy lines (see README). COMPARE_SPURT=n also lists the yes/no disagreements and the n worst
/// spurt-start and HP-at-goal errors.
fn spurt_summary(spurts: &[(usize, usize, bool, SpurtCmp)], files: &[PathBuf]) {
    let pc = |a: usize, n: usize| 100.0 * a as f64 / n.max(1) as f64;
    let q = |v: &[f32], p: f32| v.get(((v.len() as f32 - 1.0) * p) as usize).copied().unwrap_or(0.0);
    let all: Vec<&SpurtCmp> = spurts.iter().map(|s| &s.3).collect();
    let n = all.len();
    let full_ok = all.iter().filter(|c| c.sim.full == c.truth_full).count();
    let out_ok = all.iter().filter(|c| c.sim.hp_out == c.truth_out).count();
    println!(
        "spurt/HP inference on sim frames vs sim internals, {n} horses: full spurt {full_ok} ({:.2}%), HP out {out_ok} ({:.2}%)",
        pc(full_ok, n), pc(out_ok, n)
    );
    for (label, sel) in [("all", None), ("diverged races", Some(true))] {
        let cs: Vec<&SpurtCmp> = spurts.iter().filter(|s| sel.is_none_or(|d| s.2 == d)).map(|s| &s.3).collect();
        let n = cs.len();
        let cnt = |f: &dyn Fn(&SpurtHp) -> bool| (cs.iter().filter(|c| f(&c.sim)).count(), cs.iter().filter(|c| f(&c.rep)).count());
        let (fs, fr) = cnt(&|x| x.full);
        let (os, or) = cnt(&|x| x.hp_out);
        let full_same = cs.iter().filter(|c| c.sim.full == c.rep.full).count();
        let out_same = cs.iter().filter(|c| c.sim.hp_out == c.rep.hp_out).count();
        let both: Vec<_> = cs.iter().filter(|c| c.sim.start > 0.0 && c.rep.start > 0.0).collect();
        let no_spurt_diff = cs.iter().filter(|c| (c.sim.start > 0.0) != (c.rep.start > 0.0)).count();
        let mut ds: Vec<f32> = both.iter().map(|c| (c.sim.start - c.rep.start).abs()).collect();
        let mut dh: Vec<f32> = cs.iter().map(|c| (c.sim.hp_goal as f32 - c.rep.hp_goal as f32).abs()).collect();
        ds.sort_by(f32::total_cmp);
        dh.sort_by(f32::total_cmp);
        println!(
            "spurt/HP sim vs replay, {label} ({n} horses): full spurt same {full_same} ({:.2}%; sim {fs} / replay {fr} full), HP out same {out_same} ({:.2}%; sim {os} / replay {or} out)",
            pc(full_same, n), pc(out_same, n)
        );
        println!(
            "  spurt start |err| m p50 {:.3} p90 {:.3} max {:.3} (exact {} of {}; spurt vs none {no_spurt_diff}), HP at goal |err| p50 {} p90 {} max {} (exact {} of {n})",
            q(&ds, 0.5), q(&ds, 0.9), q(&ds, 1.0), ds.iter().filter(|&&d| d == 0.0).count(), ds.len(),
            q(&dh, 0.5), q(&dh, 0.9), q(&dh, 1.0), dh.iter().filter(|&&d| d == 0.0).count()
        );
    }
    let Some(top) = std::env::var("COMPARE_SPURT").ok().and_then(|v| v.parse::<usize>().ok()) else { return };
    let line = |(i, h, d, c): &(usize, usize, bool, SpurtCmp)| {
        let x = |s: &SpurtHp| format!("full {} start {:.3} hp_goal {} out {}", s.full as u8, s.start, s.hp_goal, s.hp_out as u8);
        format!("  {} h{h}{}: sim {} | replay {} | truth full {} out {}", files[*i].display(), if *d { "" } else { " (match)" }, x(&c.sim), x(&c.rep), c.truth_full as u8, c.truth_out as u8)
    };
    println!("full-spurt / HP-out disagreements (sim vs replay, or inference vs truth):");
    spurts.iter()
        .filter(|s| s.3.sim.full != s.3.rep.full || s.3.sim.hp_out != s.3.rep.hp_out || s.3.sim.full != s.3.truth_full || s.3.sim.hp_out != s.3.truth_out)
        .for_each(|s| println!("{}", line(s)));
    let mut v: Vec<_> = spurts.iter().filter(|s| s.3.sim.start > 0.0 && s.3.rep.start > 0.0).collect();
    v.sort_by(|a, b| (b.3.sim.start - b.3.rep.start).abs().total_cmp(&(a.3.sim.start - a.3.rep.start).abs()));
    println!("worst spurt start errors:");
    v.iter().take(top).for_each(|s| println!("{}", line(s)));
    let mut v: Vec<_> = spurts.iter().collect();
    v.sort_by_key(|s| std::cmp::Reverse(s.3.sim.hp_goal.abs_diff(s.3.rep.hp_goal)));
    println!("worst HP-at-goal errors:");
    v.iter().take(top).for_each(|s| println!("{}", line(s)));
}

fn collect_json(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_json(&p, out);
        } else if p.extension().is_some_and(|x| x == "json") {
            out.push(p);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut cache = HashMap::new();
    match args.as_slice() {
        [flag, dir, rest @ ..] if flag == "--corpus" => {
            let data_dir = PathBuf::from(rest.first().map_or(DEFAULT_DATA_DIR, |s| s.as_str()));
            let mut files = Vec::new();
            collect_json(Path::new(dir), &mut files);
            files.sort();
            // COMPARE_THREADS=n (default: all cores). Each thread keeps its own GameData cache.
            let threads = std::env::var("COMPARE_THREADS").ok().and_then(|t| t.parse().ok())
                .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()));
            let next = std::sync::atomic::AtomicUsize::new(0);
            let mut outcomes: Vec<(usize, Outcome)> = std::thread::scope(|sc| {
                let workers: Vec<_> = (0..threads).map(|_| sc.spawn(|| {
                    let mut cache = HashMap::new();
                    let mut out = Vec::new();
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(f) = files.get(i) else { break };
                        out.push((i, run(f, &data_dir, &mut cache, false)));
                    }
                    out
                })).collect();
                workers.into_iter().flat_map(|w| w.join().unwrap()).collect()
            });
            outcomes.sort_by_key(|o| o.0);
            let (mut ok, mut div, mut panics, mut skipped) = (0, 0, 0, 0);
            let (mut good, mut total) = (0usize, 0usize);
            let (mut same_order, mut same_winner, mut dts) = (0, 0, Vec::new());
            let mut first_frames = Vec::new();
            // (race file index, horse, in a diverged race, comparison)
            let mut spurts: Vec<(usize, usize, bool, SpurtCmp)> = Vec::new();
            for (i, o) in outcomes {
                let f = &files[i];
                match o {
                    Outcome::Match(n, sp) => {
                        spurts.extend(sp.into_iter().enumerate().map(|(h, c)| (i, h, false, c)));
                        ok += 1;
                        (good, total) = (good + n, total + n);
                        same_order += 1;
                        same_winner += 1;
                        dts.push(0.0);
                    }
                    Outcome::Diverged(d, r, sp) => {
                        spurts.extend(sp.into_iter().enumerate().map(|(h, c)| (i, h, true, c)));
                        div += 1;
                        println!("{}: {d}  [results: order {} winner {} max_dt {:.4}]", f.display(), r.order, r.winner, r.max_dt);
                        first_frames.push(d.index);
                        // Frames matched: up to the first frame divergence, or all of them if only results/events differ.
                        good += if d.section == replay::Section::Frames { d.index } else { r.frames };
                        total += r.frames;
                        same_order += r.order as usize;
                        same_winner += r.winner as usize;
                        dts.push(r.max_dt);
                    }
                    Outcome::Panicked(m) => {
                        panics += 1;
                        if panics <= 3 {
                            println!("{}: PANIC {m}", f.display());
                        }
                    }
                    Outcome::LoadError(_) => skipped += 1,
                }
            }
            first_frames.sort();
            let median = first_frames.get(first_frames.len() / 2).copied();
            let sum: usize = first_frames.iter().sum();
            dts.sort_by(f32::total_cmp);
            let pct = |q: f32| dts.get(((dts.len() as f32 - 1.0) * q) as usize).copied().unwrap_or(0.0);
            let n = ok + div;
            println!(
                "
{} files: {ok} match, {div} diverge (median first index {median:?}, sum {sum}), {panics} panic, {skipped} skipped (no replay / load error)",
                files.len()
            );
            println!("frames matched before first divergence: {good} of {total} ({:.1}%)", 100.0 * good as f64 / total.max(1) as f64);
            println!(
                "results over {n} simulated: same finish order {same_order}, same winner {same_winner}, finish_time_raw max|dt| per race p50 {:.4} p90 {:.4} max {:.4}",
                pct(0.5), pct(0.9), pct(1.0)
            );
            spurt_summary(&spurts, &files);
        }
        [race, rest @ ..] => {
            let data_dir = PathBuf::from(rest.first().map_or(DEFAULT_DATA_DIR, |s| s.as_str()));
            match run(Path::new(race), &data_dir, &mut cache, true) {
                Outcome::Match(..) => println!("MATCH: frames, results and events identical"),
                Outcome::Diverged(d, r, _) => {
                    println!("FIRST DIVERGENCE: {d}");
                    println!("results: same finish order {}, same winner {}, finish_time_raw max|dt| {:.4}", r.order, r.winner, r.max_dt);
                }
                Outcome::Panicked(m) => {
                    println!("sim panicked: {m}");
                    std::process::exit(2);
                }
                Outcome::LoadError(e) => {
                    eprintln!("load error: {e}");
                    std::process::exit(1);
                }
            }
        }
        [] => {
            eprintln!("usage: compare <saved race.json> [data_dir] | compare --corpus <dir> [data_dir]");
            std::process::exit(1);
        }
    }
}
