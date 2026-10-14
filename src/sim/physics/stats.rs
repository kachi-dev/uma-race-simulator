//! Stat pipeline: base stats, aptitude coefs, ground modifier, course-set bonus, adjusted / live stats, max HP,
//! guts/wiz order.

use crate::sim::defs::{RunningStyle, RunningStyleEx, SkillModifierParam};
use crate::sim::horse::Horse;
use crate::sim::race::Race;
use crate::sim::util::{approximately, clamp, intro_sort, max};
use crate::sim::physics::speed;
use crate::api::HorseInput;
use crate::data::master::Master;

/// Raw stats above 1200 are halved (`1200 + (raw - 1200) / 2`), in one of two forms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatHalving {
    /// `1200f + (float)(raw - 1200) / 2f` (keeps the .5).
    Float,
    /// `1200 + (raw - 1200) / 2` in int (truncates).
    Int,
}

pub const STAT_HALVING: StatHalving = StatHalving::Float;

/// The [1, 2000] stat cap.
/// `false` keeps only the lower clamp of 1.
pub const STAT_CAP_2000: bool = true;

/// Clamps a stat to [1, 2000] (see [`STAT_CAP_2000`]).
#[inline]
pub fn clamp_stat(v: f32) -> f32 {
    if STAT_CAP_2000 {
        clamp(v, 1.0, 2000.0)
    } else if v < 1.0 {
        1.0
    } else {
        v
    }
}

pub fn halved_raw(raw: i32, mode: StatHalving) -> f32 {
    if raw <= 1200 {
        return raw as f32;
    }
    match mode {
        StatHalving::Float => 1200.0 + (raw - 1200) as f32 / 2.0,
        StatHalving::Int => (1200 + (raw - 1200) / 2) as f32,
    }
}

/// A horse's stats as entered: raw (un-halved) and base (halved, plus team bonus, times motivation).
#[derive(Clone, Copy, Debug, Default)]
pub struct RaceParameter {
    pub raw_speed: i32,
    pub raw_stamina: i32,
    pub raw_pow: i32,
    pub raw_guts: i32,
    pub raw_wiz: i32,
    pub base_speed: f32,
    pub base_stamina: f32,
    pub base_pow: f32,
    pub base_guts: f32,
    pub base_wiz: f32,
    /// `HorseInput::wiz_alt`: its base Wit (else `base_wiz`) and mechanics mask (else 0).
    pub base_wiz_alt: f32,
    pub wiz_alt_mechanics: u8,
    pub motivation: i32,
    pub motivation_coef: f32,
}

impl RaceParameter {
    /// Builds the stats with the >1200 halving. No RNG.
    /// The raw fields keep the un-halved values.
    /// The team-rank bonus is added after halving.
    pub fn from_input(master: &Master, input: &HorseInput, halving: StatHalving) -> RaceParameter {
        let bonus = if input.team_rank == 0 { 0 } else { team_status_bonus(master, input.team_rank) };
        let coef = master.race_motivation_rate.get(&input.motivation).map_or(1.0, |&r| r as f32 * 0.0001);
        let base = |raw: i32| clamp_stat((bonus as f32 + halved_raw(raw, halving)) * coef);
        RaceParameter {
            raw_speed: input.speed,
            raw_stamina: input.stamina,
            raw_pow: input.pow,
            raw_guts: input.guts,
            raw_wiz: input.wiz,
            base_speed: base(input.speed),
            base_stamina: base(input.stamina),
            base_pow: base(input.pow),
            base_guts: base(input.guts),
            base_wiz: base(input.wiz),
            base_wiz_alt: base(input.wiz_alt.map_or(input.wiz, |a| a.wiz)),
            wiz_alt_mechanics: input.wiz_alt.map_or(0, |a| a.mechanics),
            motivation: input.motivation,
            motivation_coef: coef,
        }
    }
}

/// Team-rank stat bonus: the first row by threshold (stable sort, id order on ties) with `team_rank < threshold`, else 0.
fn team_status_bonus(master: &Master, team_rank: i32) -> i32 {
    let mut rows: Vec<_> = master.race_single_mode_team_status.iter().collect();
    rows.sort_by_key(|r| r.team_rank_threshold);
    rows.iter().find(|r| team_rank < r.team_rank_threshold).map_or(0, |r| r.add_status)
}

/// Ground-condition modifiers: speed / power add and the HP consumption multiplier.
#[derive(Clone, Copy, Debug, Default)]
pub struct GroundModifierParam {
    pub add_speed: f32,
    pub add_power: f32,
    pub multi_hp_sub: f32,
}

/// `(float)rate * 0.0001f`, or 1.0 when the master row is missing.
fn rate(r: Option<&i32>) -> f32 {
    r.map_or(1.0, |&r| r as f32 * 0.0001)
}

/// Distance aptitude coefs (speed, power).
pub fn init_proper_distance(race: &mut Race, i: usize) {
    let data = race.data;
    let h = &mut race.horses[i];
    let row = data.master.race_proper_distance_rate.get(&h.horse_data.active_proper_distance);
    h.proper_distance_coef_speed = rate(row.map(|r| &r.proper_rate_speed));
    h.proper_distance_coef_pow = rate(row.map(|r| &r.proper_rate_power));
}

/// Running-style aptitude coef, keyed by the grade of the horse's style (grade 0 without a style; Oonige uses the
/// Nige grade).
pub fn init_proper_running_style(race: &mut Race, i: usize) {
    let data = race.data;
    let h = &mut race.horses[i];
    let grade = match h.horse_data.running_style {
        RunningStyle::None => 0,
        s => h.horse_data.proper_running_style[s as usize - 1],
    };
    h.proper_running_style_coef = rate(data.master.race_proper_runningstyle_rate.get(&grade));
}

pub fn init_proper_ground_type(race: &mut Race, i: usize) {
    let data = race.data;
    let h = &mut race.horses[i];
    h.proper_ground_type_coef = rate(data.master.race_proper_ground_rate.get(&h.horse_data.active_proper_ground_type));
}

/// Ground-condition modifiers for turf / dirt. Out of range or another ground: left unchanged.
pub fn init_ground_condition_param(race: &mut Race, i: usize) {
    let p = &race.data.param;
    let k = race.info.ground_condition - 1;
    let gm = match race.info.race_course_set.ground {
        1 => usize::try_from(k).ok().and_then(|k| p._groundModifierParamTurf.get(k)).map(|g| (g.addSpeed, g.addPower, g.multiHpSub)),
        2 => usize::try_from(k).ok().and_then(|k| p._groundModifierParamDirt.get(k)).map(|g| (g.addSpeed, g.addPower, g.multiHpSub)),
        _ => None,
    };
    if let Some((add_speed, add_power, multi_hp_sub)) = gm {
        let h = &mut race.horses[i];
        h.ground_modifier_param = GroundModifierParam { add_speed, add_power, multi_hp_sub };
    }
}

/// Course-set speed bonus coef: 1 + the mean threshold coef of the course's target stats.
pub fn init_course_set_status(race: &mut Race, i: usize) {
    let data = race.data;
    let mut row = data.master.race_course_set_status.get(&race.info.race_course_set.course_set_status_id);
    // Era: before the 2026-07 update the Ooi courses (track 10101) have no course-set bonus (their master rows
    // arrived with it).
    if data.era.pre_2026_07_courses && race.info.race_course_set.race_track_id == 10101 {
        row = None;
    }
    let h = &mut race.horses[i];
    h.base_speed_coef = match row {
        None => 1.0,
        Some(row) => {
            let targets = [row.target_status_1, row.target_status_2];
            let n = targets.iter().filter(|&&t| t != 0).count() as i32;
            let mut coef = 1.0f32;
            if n > 0 {
                let rp = &h.horse_data.race_param;
                for t in targets {
                    if t != 0 {
                        // The motivated base stat (not the adjusted one); first threshold >= s.
                        let s = match t {
                            1 => rp.base_speed,
                            2 => rp.base_stamina,
                            3 => rp.base_pow,
                            4 => rp.base_guts,
                            5 => base_wiz_for(h, crate::api::wiz_mechanic::COURSE_BONUS),
                            _ => f32::NAN, // unknown target stat: coef 0
                        };
                        let c = data.param.CourseSetAdjust.CoefAndThresholdArray.iter().find(|e| e.StatusThreshold as f32 >= s).map_or(0.0, |e| e.Coef);
                        coef += c / n as f32;
                    }
                }
            }
            coef
        }
    };
}

/// Adjusted stats: course-set bonus, ground modifier, style aptitude and the Career stat add.
pub fn init_base_status_adjust(race: &mut Race, i: usize) {
    init_course_set_status(race, i);
    init_ground_condition_param(race, i);
    let g = &race.data.param.Global;
    // Career races (race types 6, 7) add a flat bonus.
    let (sp, st, po, gu, wi) = if race.info.race_type == 6 || race.info.race_type == 7 {
        (0.0 + g.SingleModeAddSpeed, 0.0 + g.SingleModeAddStamina, 0.0 + g.SingleModeAddPow, 0.0 + g.SingleModeAddGuts, 0.0 + g.SingleModeAddWiz)
    } else {
        (0.0, 0.0, 0.0, 0.0, 0.0)
    };
    let h = &mut race.horses[i];
    let rp = h.horse_data.race_param;
    let gm = h.ground_modifier_param;
    // Each multi-op sum is one f64 expression, rounded once at the clamp argument.
    let f = |x: f32| x as f64;
    h.base_speed_adjusted = clamp_stat((f(rp.base_speed) * f(h.base_speed_coef) + f(gm.add_speed) + f(sp)) as f32);
    h.base_stamina_adjusted = clamp_stat(rp.base_stamina + st);
    h.base_pow_adjusted = clamp_stat((f(rp.base_pow) + f(gm.add_power) + f(po)) as f32);
    h.base_guts_adjusted = clamp_stat(rp.base_guts + gu);
    h.base_wiz_adjusted = clamp_stat((f(rp.base_wiz) * f(h.proper_running_style_coef) + f(wi)) as f32);
    h.base_wiz_adjusted_alt = clamp_stat((f(rp.base_wiz_alt) * f(h.proper_running_style_coef) + f(wi)) as f32);
}

/// Live Speed: the adjusted stat with skill modifiers, clamped.
pub fn speed(h: &Horse) -> f32 {
    clamp_stat(h.skill_modifier_receiver.apply_modifier(SkillModifierParam::Speed, h.base_speed_adjusted))
}

pub fn stamina(h: &Horse) -> f32 {
    clamp_stat(h.skill_modifier_receiver.apply_modifier(SkillModifierParam::Stamina, h.base_stamina_adjusted))
}

pub fn pow(h: &Horse) -> f32 {
    clamp_stat(h.skill_modifier_receiver.apply_modifier(SkillModifierParam::Power, h.base_pow_adjusted))
}

pub fn guts(h: &Horse) -> f32 {
    clamp_stat(h.skill_modifier_receiver.apply_modifier(SkillModifierParam::Guts, h.base_guts_adjusted))
}

pub fn wiz(h: &Horse) -> f32 {
    clamp_stat(h.skill_modifier_receiver.apply_modifier(SkillModifierParam::Wiz, h.base_wiz_adjusted))
}

fn uses_alt(h: &Horse, m: u8) -> bool {
    h.horse_data.race_param.wiz_alt_mechanics & m != 0
}

/// Live Wit as mechanic `m` (a [`crate::api::wiz_mechanic`] bit) reads it: [`wiz`], or from `HorseInput::wiz_alt`.
pub fn wiz_for(h: &Horse, m: u8) -> f32 {
    let base = if uses_alt(h, m) { h.base_wiz_adjusted_alt } else { h.base_wiz_adjusted };
    clamp_stat(h.skill_modifier_receiver.apply_modifier(SkillModifierParam::Wiz, base))
}

/// `race_param.base_wiz` as mechanic `m` reads it (see [`wiz_for`]).
pub fn base_wiz_for(h: &Horse, m: u8) -> f32 {
    let rp = &h.horse_data.race_param;
    if uses_alt(h, m) { rp.base_wiz_alt } else { rp.base_wiz }
}

/// Max HP: `stamina * HpInitialVal1 * style coef + course distance`.
pub fn calc_max_hp(p: &crate::data::param::Hp, stamina: f32, style: RunningStyle, style_ex: RunningStyleEx, course_distance: i32) -> f32 {
    let coef = if style_ex == RunningStyleEx::Oonige {
        p.HpMaxCoefOoNige
    } else {
        match style {
            RunningStyle::Nige => p.HpMaxCoefNige,
            RunningStyle::Senko => p.HpMaxCoefSenko,
            RunningStyle::Sashi => p.HpMaxCoefSashi,
            RunningStyle::Oikomi => p.HpMaxCoefOikomi,
            RunningStyle::None => 1.0,
        }
    };
    // One f64 expression, rounded once.
    ((stamina as f64 * p.HpInitialVal1 as f64) * coef as f64 + course_distance as f64) as f32
}

/// Recomputes the adjusted stats, max HP (HP refilled), min / start speed and the base target speeds (24 RNG draws).
pub fn recalc_params(race: &mut Race, i: usize) {
    init_base_status_adjust(race, i);
    let p = &race.data.param;
    let d = race.info.race_course_set.distance;
    let h = &mut race.horses[i];
    let st = stamina(h);
    h.max_hp = calc_max_hp(&p.Hp, st, h.horse_data.running_style, h.horse_data.running_style_ex, d);
    h.hp = h.max_hp;
    speed::update_min_speed(race, i);
    let h = &mut race.horses[i];
    h.last_speed = max(p.StartSpeed, h.min_speed);
    speed::calc_base_target_speed(race, i);
    race.horses[i].is_param_initialized = true;
}

/// Horse indices sorted by `getter`, descending (.NET introsort; approximately equal values by horse index).
fn apply_order(race: &Race, getter: fn(&Horse) -> f32) -> Vec<usize> {
    let vals: Vec<f32> = race.horses.iter().map(getter).collect();
    let mut list: Vec<usize> = (0..race.horses.len()).collect();
    intro_sort(&mut list, |&x, &y| {
        let (a, b) = (vals[x], vals[y]);
        if approximately(a, b) {
            race.horses[x].horse_data.horse_index - race.horses[y].horse_data.horse_index
        } else if a > b {
            -1
        } else {
            1
        }
    });
    list
}

/// Ranks every horse by live Guts (0 = highest).
pub fn calc_guts_order(race: &mut Race) {
    for (rank, j) in apply_order(race, guts).into_iter().enumerate() {
        race.horses[j].guts_order = rank as i32;
    }
}

/// Ranks every horse by live Wit (0 = highest).
pub fn calc_wiz_order(race: &mut Race) {
    for (rank, j) in apply_order(race, wiz).into_iter().enumerate() {
        race.horses[j].wiz_order = rank as i32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halving() {
        assert_eq!(halved_raw(1200, StatHalving::Float), 1200.0);
        assert_eq!(halved_raw(1261, StatHalving::Float), 1230.5);
        assert_eq!(halved_raw(1261, StatHalving::Int), 1230.0);
        assert_eq!(halved_raw(1707, StatHalving::Int), 1453.0);
    }
}
