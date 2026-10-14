//! Rushed ("temptation"): the lottery before the start, the rush itself and its end rolls.

use crate::sim::defs::{RunningStyle, SkillModifierParam};
use crate::sim::horse::{self, Horse};
use crate::sim::race::Race;
use crate::sim::physics::stats;

/// Rushed (temptation) mode; recorded per frame as i8 (`HorseFrame::temptation_mode`).
#[repr(i32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TemptationMode {
    #[default]
    Null = 0,
    PositionSashi = 1,
    PositionSenko = 2,
    PositionNige = 3,
    Boost = 4,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Temptation {
    pub temptation_start_section: i32,
    pub temptation_end_check_sec: f32,
    pub temptation_force_end_sec: f32,
    pub is_force_end_time_by_skill_enable: bool,
    pub force_end_time_by_skill: f32,
    pub is_temptation: bool,
    pub mode: TemptationMode,
    pub temptation_count: i32,
    pub is_temptation_start_enable: bool,
}

impl Temptation {
    /// A fresh calculator (no rush drawn yet). No RNG calls.
    pub fn new() -> Temptation {
        Temptation { temptation_start_section: -1, ..Default::default() }
    }
}

/// The pre-race rush lottery (run once before the start): whether the horse will rush, and in which section.
/// RNG: always one `GetRandom(100)` (int); one `GetRandom(min, max + 1)` (int) on success.
pub fn lot_temptation(race: &mut Race, i: usize) {
    let data = race.data;
    let tp = &data.param.Temptation;
    let h = &race.horses[i];
    // The Pow argument is one f64 expression; Pow / Log10 are f64 calls rounded to f32.
    let x = crate::sim::util::mathf_pow((stats::wiz_for(h, crate::api::wiz_mechanic::RUSHED) as f64 * 0.1f32 as f64 + 1.0) as f32, 2.0);
    let per = crate::sim::util::mathf_pow(tp.TemptationStartPerVal1 / crate::sim::util::mathf_log10(x), 2.0);
    let per = h.skill_modifier_receiver.apply_modifier(SkillModifierParam::TemptationPer, per);
    let r = race.rng.random_int(100);
    if per > r as f32 {
        race.horses[i].ai.temptation.is_temptation_start_enable = true;
        let s = race.rng.random_int_range(tp.TemptationLotSectionMin, tp.TemptationLotSectionMax + 1);
        race.horses[i].ai.temptation.temptation_start_section = s;
    }
}

fn tick(t: &mut f32, dt: f32) {
    if *t > 0.0 {
        *t -= dt;
        if 0.0 > *t {
            *t = 0.0;
        }
    }
}

/// Starts the rush at its section, then ends it on the force-end timer, a skill's end timer, or a periodic end roll.
pub fn update(race: &mut Race, i: usize, dt: f32) {
    let data = race.data;
    let tp = &data.param.Temptation;
    let calc = &race.horses[i].ai.temptation;
    if !calc.is_temptation {
        if !calc.is_temptation_start_enable {
            return;
        }
        if horse::calc_section(race, i) < calc.temptation_start_section {
            return;
        }
        race.horses[i].ai.temptation.is_temptation = true;
        let mode = calc_temptation_mode(race, race.horses[i].horse_data.running_style);
        let c = &mut race.horses[i].ai.temptation;
        c.mode = mode;
        c.temptation_end_check_sec = tp.TemptationEndCheckTime;
        c.temptation_count += 1;
        c.temptation_force_end_sec = tp.TemptationForceEndTime;
        c.is_force_end_time_by_skill_enable = false;
        c.force_end_time_by_skill = 0.0;
        c.is_temptation_start_enable = false;
        return;
    }
    let c = &mut race.horses[i].ai.temptation;
    if !c.is_force_end_time_by_skill_enable {
        tick(&mut c.temptation_force_end_sec, dt);
        if !(0.0 < c.temptation_force_end_sec) {
            end_temptation(c);
            return;
        }
        tick(&mut c.temptation_end_check_sec, dt);
        if 0.0 < c.temptation_end_check_sec {
            return;
        }
        c.temptation_end_check_sec = tp.TemptationEndCheckTime;
        let r = race.rng.random_int(100);
        if tp.TemptationEndPer > r as f32 {
            end_temptation(&mut race.horses[i].ai.temptation);
        }
    } else {
        tick(&mut c.force_end_time_by_skill, dt);
        if !(0.0 < c.force_end_time_by_skill) {
            end_temptation(c);
        }
    }
}

/// Ends the rush (the count is kept).
fn end_temptation(c: &mut Temptation) {
    c.is_temptation = false;
    c.mode = TemptationMode::Null;
}

/// What the rush does for this running style. One `GetRandom(100)` (int) for Senko/Sashi/Oikomi, none for Nige.
/// The thresholds are cumulative f32 sums.
fn calc_temptation_mode(race: &mut Race, style: RunningStyle) -> TemptationMode {
    let data = race.data;
    let tp = &data.param.Temptation;
    let pick = |thresholds: &[(f32, TemptationMode)], r: f32| {
        let mut acc = 0.0f32;
        for (k, &(per, mode)) in thresholds.iter().enumerate() {
            acc = if k == 0 { per } else { acc + per };
            if acc > r {
                return mode;
            }
        }
        TemptationMode::Null
    };
    match style {
        RunningStyle::Nige => TemptationMode::Boost,
        RunningStyle::Senko => {
            let r = race.rng.random_int(100) as f32;
            pick(&[(tp.SenkoToNigePer, TemptationMode::PositionNige)], r)
        }
        RunningStyle::Sashi => {
            let r = race.rng.random_int(100) as f32;
            pick(&[(tp.SashiToSenkoPer, TemptationMode::PositionSenko), (tp.SashiToNigePer, TemptationMode::PositionNige)], r)
        }
        RunningStyle::Oikomi => {
            let r = race.rng.random_int(100) as f32;
            pick(
                &[
                    (tp.OikomiToSashiPer, TemptationMode::PositionSashi),
                    (tp.OikomiToSenkoPer, TemptationMode::PositionSenko),
                    (tp.OikomiToNigePer, TemptationMode::PositionNige),
                ],
                r,
            )
        }
        RunningStyle::None => TemptationMode::Null,
    }
}

/// A skill ends the rush after `time` seconds (replacing the end rolls).
pub fn set_force_end_time_by_skill(calc: &mut Temptation, time: f32) {
    calc.force_end_time_by_skill = time;
    calc.is_force_end_time_by_skill_enable = true;
}

pub fn temptation_mode(h: &Horse) -> TemptationMode {
    h.ai.temptation.mode
}

/// Currently rushed.
pub fn is_temptation(h: &Horse) -> bool {
    h.ai.temptation.is_temptation
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::test_util::ai_race;

    #[test]
    fn lottery_draw_counts() {
        let (mut fails, mut wins) = (0, 0);
        for seed in 0..200 {
            let mut race = ai_race(1);
            race.rng = crate::rng::RaceRandom::new(seed);
            race.horses[0].base_wiz_adjusted = 600.0;
            race.horses[0].ai.temptation = Temptation::new();
            lot_temptation(&mut race, 0);
            let c = race.horses[0].ai.temptation;
            let draws = race.rng.draws();
            if c.is_temptation_start_enable {
                wins += 1;
                assert_eq!(draws, 2);
                assert!((2..=9).contains(&c.temptation_start_section));
            } else {
                fails += 1;
                assert_eq!(draws, 1);
                assert_eq!(c.temptation_start_section, -1);
            }
        }
        assert!(fails > 0 && wins > 0);
    }

    #[test]
    fn mode_and_end_draws() {
        let mut race = ai_race(1);
        race.info.course_section_distance = 100.0;
        for (style, draws, mode) in [
            (RunningStyle::Nige, 0, TemptationMode::Boost),
            (RunningStyle::Senko, 1, TemptationMode::PositionNige),
        ] {
            race.horses[0].horse_data.running_style = style;
            race.horses[0].distance = 150.0; // section 2
            race.horses[0].ai.temptation = Temptation { is_temptation_start_enable: true, temptation_start_section: 2, ..Default::default() };
            let before = race.rng.draws();
            update(&mut race, 0, 0.0666);
            let c = race.horses[0].ai.temptation;
            assert!(c.is_temptation && !c.is_temptation_start_enable && c.temptation_count == 1);
            assert_eq!(c.mode, mode);
            assert_eq!(race.rng.draws() - before, draws);
        }
        // Running: one int draw per 3 s check; the 12 s force end wins on its frame (no draw).
        let mut checks = 0;
        let mut frames = 0;
        while race.horses[0].ai.temptation.is_temptation {
            let before = race.rng.draws();
            update(&mut race, 0, 0.0666);
            checks += race.rng.draws() - before;
            frames += 1;
        }
        assert!(checks >= 1 && checks <= 3 && frames <= 181, "checks {checks} frames {frames}");
        // Skill end timer: no rolls at all.
        race.horses[0].ai.temptation = Temptation { is_temptation: true, temptation_force_end_sec: 12.0, temptation_end_check_sec: 0.01, ..Default::default() };
        set_force_end_time_by_skill(&mut race.horses[0].ai.temptation, 0.1);
        let before = race.rng.draws();
        update(&mut race, 0, 0.0666);
        update(&mut race, 0, 0.0666);
        assert!(!race.horses[0].ai.temptation.is_temptation);
        assert_eq!(race.rng.draws(), before);
    }
}
