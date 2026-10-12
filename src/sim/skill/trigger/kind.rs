//! Trigger keywords: what each one checks ([`SkillTriggerKind`]) and how a keyword builds its trigger. Building
//! also picks the window of the random-position triggers, so construction-time RNG draws happen in the game's order.

use crate::sim::defs::SkillTriggerOperator;
use crate::sim::field;
use crate::sim::race::Race;
use crate::sim::skill::trigger::window::{all_corner_random_init, init_distance, DistanceInit, DistanceWindow};
use crate::sim::skill::trigger::{rescale_rate, SkillTrigger};
use crate::sim::skill::SkillDetailRef;

/// Unit variants are stateless: `value_lh() op value_rh` every frame.
#[derive(Clone, Debug, Default)]
pub enum SkillTriggerKind {
    /// Unknown keyword or unparsable term.
    #[default]
    Null,
    CourseCornerCount,
    Grade,
    RaceId,
    Time,
    CourseDistance,
    IsDirtGrade,
    RaceAccumulateTime,
    Slope,
    Corner,
    Furlong,
    Order,
    /// order_rate, order_rate_lastspurt, order_rate_final_corner_end, gate_number_rate, post_number_rate,
    /// popularity_rate and distance_rate only rescale `value_rh` at construction; they reuse these lh variants.
    OrderLastSpurt,
    OrderFinalCornerEnd,
    GateNumber,
    PostNumber,
    HorseDistanceRate,
    HorseRemainDistance,
    HorseDistanceDiffRate,
    OtherHorseBehind,
    BeBlocked,
    BeBlockedFront,
    BeBlockedSide,
    BeBlockedAllContinueTime,
    BeBlockedFrontContinueTime,
    BeBlockedSideContinueTime,
    BeBlockedFrontHpEmpty,
    BeBlockedFrontHpEmptyContinueTime,
    TemptationCount,
    CompeteFightCount,
    GoodStart,
    BadStart,
    BaseSpeed,
    BaseStamina,
    BasePower,
    BaseGuts,
    BaseWiz,
    HpPer,
    HpEmpty,
    CurrentSpeed,
    SpeedOrder,
    VisibleHorse,
    ChangeOrder,
    ChangeOrderOneTime,
    LaneType,
    LanePer,
    MoveLane,
    Overtake,
    OvertakeTargetContinueTime,
    OvertakeTargetHaveNoOrderUpContinueTime,
    OvertakeTargetHaveNoOrderDownContinueTime,
    Surrounded,
    NearHorseCount,
    NearAroundAccumulateTime,
    DistanceDiffTop,
    DistanceDiffInfront,
    DistanceDiffBehind,
    NearFrontHorseCount,
    NearBehindHorseCount,
    HorseBehindIn,
    HorseBehindOut,
    HorseInfrontNearTime,
    HorseBehindNearTime,
    HorseInfrontNearLaneTime,
    HorseBehindNearLaneTime,
    ActivateHealSkillCount,
    ActivateCount,
    ActivateCountLaterHalf,
    ActivateCountAllTeam,
    ChangeOrderUpPhaseMiddle,
    ChangeOrderUpPhaseEndAfter,
    ChangeOrderUpCorner,
    ChangeOrderUpLastSpurt,
    ChangeOrderUpFinalCornerAfter,
    ChangeOrderDownPhaseMiddle,
    ChangeOrderDownPhaseEndAfter,
    ChangeOrderDownCorner,
    ChangeOrderDownLastSpurt,
    ChangeOrderDownFinalCornerAfter,
    GroundType,
    GroundCondition,
    CourseRotation,
    CourseDistanceType,
    Weather,
    Season,
    RaceTrackId,
    StraightFrontType,
    FinalCorner,
    RunningStyle,
    Motivation,
    Popularity,
    Phase,
    Temptation,
    IsLastSpurt,
    LastSpurt,
    Always,
    // lh computed once at construction:
    /// month, running_style_count_*, running_style_is_max, final_grade_*_count, is_basis_distance,
    /// running_style_count_same(_rate), running_style_equal_popularity_one, popularity_topicon_num /
    /// popularity_icon_num.
    Latched(i32),
    /// is_exist_chara_id; only == / != can activate.
    ExistCharaId(bool),
    // Float lh:
    Hp,
    BashinDiffInfront,
    BashinDiffBehind,
    /// The float rh, `value_rh / 10`.
    DistanceDiffTopFloat(f32),
    // Own state:
    RandomLot(bool),
    RandomLotOtherActivate { is_lot_ok: bool, lot_interval: f32, lot_interval_max: f32 },
    /// The drawn start second.
    RaceAccumulateTimeRandom(i32),
    /// Index 0..8 into the 10..90 % order thresholds.
    OrderRateInContinue(usize),
    OrderRateOutContinue(usize),
    RunningStyleTemptationCount { style: i32, is_check_only_opponent: bool },
    TemptationBehind { is_check_only_opponent: bool },
    TemptationInfront { is_check_only_opponent: bool },
    HpEmptyOneTime { prev_hp: f32, is_initialized: bool },
    /// Horses with viewer_id != 0.
    HorseViewerIdRemainDistance(Vec<usize>),
    HorseBehindNearLaneTimeParamSet(usize),
    SameSkillHorseCount { same_skill_horse_count: i32, is_initialized: bool },
    UsedParticularSkillId(i32),
    OtherCharacterActivateAdvantageSkill(i32),
    ActivateOtherSkillDetail(bool),
    ActivateCountPhase(Vec<usize>),
    IsActivateAnySkill { prev_activate_skill_count: i32, activate_this_skill_count: i32 },
    ActivateSkillTagGroup(Vec<i32>),
    LastStraightOneTime(bool),
    FinalCornerLaterHalf { is_initialized: bool, is_exist_final_corner: bool, distance_min: f32, distance_max: f32 },
    /// Random-position triggers with one window.
    Distance(DistanceInit, DistanceWindow),
    /// all_corner_random: up to 4 (min, max) windows, ascending.
    AllCornerRandom { distance_set_list: Vec<(f32, f32)>, is_initialized: bool },
}

/// `None` = unknown keyword.
pub fn create_trigger(race: &mut Race, r: SkillDetailRef, kw: &str, op: SkillTriggerOperator, value_rh: i32) -> Option<SkillTrigger> {
    use SkillTriggerKind as K;
    let owner = r.horse;
    let n_horses = race.info.num_race_horses;
    let mut rh = value_rh;
    let kind = match kw {
        // One int draw.
        "random_lot" => K::RandomLot(race.rng.random_int(100) < value_rh),
        "random_lot_other_activate" => K::RandomLotOtherActivate {
            is_lot_ok: false,
            lot_interval: 0.0,
            lot_interval_max: race.data.param.Skill.RandomLotOtherActivateLotInterval,
        },
        "corner_count" => K::CourseCornerCount,
        "month" => K::Latched(race.data.master.race_instance.get(&race.input.race_instance_id).map_or(0, |x| x.date) / 100),
        "grade" => K::Grade,
        "race_id" => K::RaceId,
        "time" => K::Time,
        "course_distance" => K::CourseDistance,
        "is_dirtgrade" => K::IsDirtGrade,
        "accumulatetime" => K::RaceAccumulateTime,
        // One int draw.
        "accumulatetime_random" => K::RaceAccumulateTimeRandom(race.rng.random_int(value_rh)),
        "slope" => K::Slope,
        "up_slope_random" => K::Distance(DistanceInit::UpSlopeRandom, DistanceWindow::default()),
        "down_slope_random" => K::Distance(DistanceInit::DownSlopeRandom, DistanceWindow::default()),
        "corner" => K::Corner,
        "furlong" => K::Furlong,
        "furlong_random" => K::Distance(DistanceInit::FurlongRandom, DistanceWindow::default()),
        "order" => K::Order,
        "order_rate" => {
            rh = rescale_rate(value_rh, n_horses);
            K::Order
        }
        "order_lastspurt" => K::OrderLastSpurt,
        "order_rate_lastspurt" => {
            rh = rescale_rate(value_rh, n_horses);
            K::OrderLastSpurt
        }
        "order_final_corner_end" => K::OrderFinalCornerEnd,
        "order_rate_final_corner_end" => {
            rh = rescale_rate(value_rh, n_horses);
            K::OrderFinalCornerEnd
        }
        "gate_number" => K::GateNumber,
        "gate_number_rate" => {
            rh = rescale_rate(value_rh, n_horses);
            K::GateNumber
        }
        "post_number" => K::PostNumber,
        "post_number_rate" => {
            rh = rescale_rate(value_rh, post_number_max(race));
            K::PostNumber
        }
        "distance_rate" => {
            rh = rescale_rate(value_rh, race.info.race_course_set.distance);
            K::HorseDistanceRate
        }
        "remain_distance" => K::HorseRemainDistance,
        "remain_distance_viewer_id" => {
            K::HorseViewerIdRemainDistance((0..race.horses.len()).filter(|&j| race.horses[j].horse_data.viewer_id != 0).collect())
        }
        "distance_diff_rate" => K::HorseDistanceDiffRate,
        "all_phase_random" => K::Distance(DistanceInit::AllPhaseRandom, DistanceWindow::default()),
        "distance_rate_after_random" => K::Distance(DistanceInit::DistanceRateAfterRandom, DistanceWindow::default()),
        "other_behind" => K::OtherHorseBehind,
        "blocked_all" => K::BeBlocked,
        "blocked_front" => K::BeBlockedFront,
        "blocked_side" => K::BeBlockedSide,
        "blocked_all_continuetime" => K::BeBlockedAllContinueTime,
        "blocked_front_continuetime" => K::BeBlockedFrontContinueTime,
        "blocked_side_continuetime" => K::BeBlockedSideContinueTime,
        "blocked_front_hp_empty" => K::BeBlockedFrontHpEmpty,
        "blocked_front_hp_empty_continuetime" => K::BeBlockedFrontHpEmptyContinueTime,
        "running_style_is_max" => K::Latched(is_running_style_count_max(race, owner) as i32),
        "temptation_count_behind" => K::TemptationBehind { is_check_only_opponent: false },
        "temptation_opponent_count_behind" => K::TemptationBehind { is_check_only_opponent: true },
        "temptation_count_infront" => K::TemptationInfront { is_check_only_opponent: false },
        "temptation_opponent_count_infront" => K::TemptationInfront { is_check_only_opponent: true },
        "temptation_count" => K::TemptationCount,
        "compete_fight_count" => K::CompeteFightCount,
        "is_goodstart" => K::GoodStart,
        "is_badstart" => K::BadStart,
        "base_speed" => K::BaseSpeed,
        "base_stamina" => K::BaseStamina,
        "base_power" => K::BasePower,
        "base_guts" => K::BaseGuts,
        "base_wiz" => K::BaseWiz,
        "hp" => K::Hp,
        "hp_per" => K::HpPer,
        "is_hp_empty" => K::HpEmpty,
        "is_hp_empty_onetime" => K::HpEmptyOneTime { prev_hp: 0.0, is_initialized: false },
        "current_speed" => K::CurrentSpeed,
        "speed_order" => K::SpeedOrder,
        "visiblehorse" => K::VisibleHorse,
        "change_order" => K::ChangeOrder,
        "change_order_onetime" => K::ChangeOrderOneTime,
        "lane_type" => K::LaneType,
        "lane_per" => K::LanePer,
        "is_move_lane" => K::MoveLane,
        "is_overtake" => K::Overtake,
        "overtake_target_time" => K::OvertakeTargetContinueTime,
        "overtake_target_no_order_up_time" => K::OvertakeTargetHaveNoOrderUpContinueTime,
        "overtake_target_no_order_down_time" => K::OvertakeTargetHaveNoOrderDownContinueTime,
        "final_grade_higher_count" => {
            let g = race.horses[owner].horse_data.final_grade;
            K::Latched(race.horses.iter().filter(|h| h.horse_data.final_grade > g).count() as i32)
        }
        "final_grade_lower_count" => {
            let g = race.horses[owner].horse_data.final_grade;
            K::Latched(race.horses.iter().filter(|h| h.horse_data.final_grade < g).count() as i32)
        }
        "is_surrounded" => K::Surrounded,
        "near_count" => K::NearHorseCount,
        "near_around_accumulatetime" => K::NearAroundAccumulateTime,
        "distance_diff_top" => K::DistanceDiffTop,
        "distance_diff_top_float" => K::DistanceDiffTopFloat(value_rh as f32 / 10.0),
        "distance_diff_infront" => K::DistanceDiffInfront,
        "distance_diff_behind" => K::DistanceDiffBehind,
        "bashin_diff_infront" => K::BashinDiffInfront,
        "bashin_diff_behind" => K::BashinDiffBehind,
        "near_infront_count" => K::NearFrontHorseCount,
        "near_behind_count" => K::NearBehindHorseCount,
        "same_skill_horse_count" => K::SameSkillHorseCount { same_skill_horse_count: 0, is_initialized: false },
        "is_used_skill_id" => K::UsedParticularSkillId(value_rh),
        "is_behind_in" => K::HorseBehindIn,
        "is_behind_out" => K::HorseBehindOut,
        "infront_near_time" => K::HorseInfrontNearTime,
        "behind_near_time" => K::HorseBehindNearTime,
        "infront_near_lane_time" => K::HorseInfrontNearLaneTime,
        "behind_near_lane_time" => K::HorseBehindNearLaneTime,
        "behind_near_lane_time_set1" => K::HorseBehindNearLaneTimeParamSet(0),
        "is_exist_chara_id" => K::ExistCharaId(race.horses.iter().any(|h| h.horse_data.chara_id == value_rh)),
        "is_other_character_activate_advantage_skill" => K::OtherCharacterActivateAdvantageSkill(value_rh),
        "is_activate_other_skill_detail" => K::ActivateOtherSkillDetail(false),
        "activate_count_heal" => K::ActivateHealSkillCount,
        "activate_count_start" => K::ActivateCountPhase(vec![0]),
        "activate_count_middle" => K::ActivateCountPhase(vec![1]),
        "activate_count_end_after" => K::ActivateCountPhase(vec![2, 3]),
        "activate_count_all" => K::ActivateCount,
        "activate_count_later_half" => K::ActivateCountLaterHalf,
        "activate_count_all_team" => K::ActivateCountAllTeam,
        "is_activate_any_skill" => K::IsActivateAnySkill { prev_activate_skill_count: 0, activate_this_skill_count: 0 },
        "activate_skill_tag_group" => K::ActivateSkillTagGroup(
            race.data.param.Skill.SkillActivateTagGroupArray.get(value_rh as usize).expect("SkillActivateTagGroupArray index").TagArray.clone(),
        ),
        "change_order_up_middle" => K::ChangeOrderUpPhaseMiddle,
        "change_order_up_end_after" => K::ChangeOrderUpPhaseEndAfter,
        "change_order_up_corner" => K::ChangeOrderUpCorner,
        "change_order_up_lastspurt" => K::ChangeOrderUpLastSpurt,
        "change_order_up_finalcorner_after" => K::ChangeOrderUpFinalCornerAfter,
        "change_order_down_middle" => K::ChangeOrderDownPhaseMiddle,
        "change_order_down_end_after" => K::ChangeOrderDownPhaseEndAfter,
        "change_order_down_corner" => K::ChangeOrderDownCorner,
        "change_order_down_lastspurt" => K::ChangeOrderDownLastSpurt,
        "change_order_down_finalcorner_after" => K::ChangeOrderDownFinalCornerAfter,
        "ground_type" => K::GroundType,
        "always" => K::Always,
        "ground_condition" => K::GroundCondition,
        "rotation" => K::CourseRotation,
        "distance_type" => K::CourseDistanceType,
        "is_basis_distance" => K::Latched((race.info.race_course_set.distance % 400 < 100) as i32),
        "weather" => K::Weather,
        "season" => K::Season,
        "track_id" => K::RaceTrackId,
        "straight_random" => K::Distance(DistanceInit::StraightRandom, DistanceWindow::default()),
        "while_in_straight_random" => K::Distance(DistanceInit::WhileInStraightRandom, DistanceWindow::default()),
        "last_straight_random" => K::Distance(DistanceInit::LastStraightRandom, DistanceWindow::default()),
        "is_last_straight_onetime" => K::LastStraightOneTime(false),
        "straight_front_type" => K::StraightFrontType,
        "corner_random" => K::Distance(DistanceInit::CornerRandom, DistanceWindow::default()),
        "all_corner_random" => K::AllCornerRandom { distance_set_list: Vec::new(), is_initialized: false },
        "is_finalcorner" => K::FinalCorner,
        "is_finalcorner_random" => K::Distance(DistanceInit::FinalCornerRandom, DistanceWindow::default()),
        "is_finalcorner_laterhalf" => {
            K::FinalCornerLaterHalf { is_initialized: false, is_exist_final_corner: false, distance_min: 0.0, distance_max: 0.0 }
        }
        "running_style" => K::RunningStyle,
        "motivation" => K::Motivation,
        "popularity" => K::Popularity,
        "popularity_rate" => {
            rh = rescale_rate(value_rh, n_horses);
            K::Popularity
        }
        "popularity_topicon_num" => K::Latched(popularity_ranks(race, owner).iter().filter(|&&x| x == 0).count() as i32),
        "popularity_icon_num" => K::Latched(popularity_ranks(race, owner).iter().filter(|&&x| x < 5).count() as i32),
        "phase" => K::Phase,
        "phase_random" => K::Distance(DistanceInit::PhaseRandom, DistanceWindow::default()),
        "phase_laterhalf_random" => K::Distance(DistanceInit::PhaseLaterHalfRandom, DistanceWindow::default()),
        "phase_firsthalf_random" => K::Distance(DistanceInit::PhaseFirstHalfRandom, DistanceWindow::default()),
        "running_style_count_same" => {
            let s = race.horses[owner].horse_data.running_style;
            K::Latched(field::running_style_count(race, s))
        }
        "running_style_count_same_rate" => {
            rh = rescale_rate(value_rh, n_horses);
            let s = race.horses[owner].horse_data.running_style;
            K::Latched(field::running_style_count(race, s))
        }
        "is_temptation" => K::Temptation,
        "is_lastspurt" => K::IsLastSpurt,
        "running_style_equal_popularity_one" => {
            let fav = race.info.horse_index_by_popularity[0] as usize;
            K::Latched((race.horses[fav].horse_data.running_style == race.horses[owner].horse_data.running_style) as i32)
        }
        "lastspurt" => K::LastSpurt,
        _ => {
            if let Some(n) = kw.strip_prefix("order_rate_in").and_then(|s| s.strip_suffix("_continue")) {
                K::OrderRateInContinue(order_type(n)?)
            } else if let Some(n) = kw.strip_prefix("order_rate_out").and_then(|s| s.strip_suffix("_continue")) {
                K::OrderRateOutContinue(order_type(n)?)
            } else if let Some(s) = kw.strip_prefix("running_style_temptation_opponent_count_") {
                K::RunningStyleTemptationCount { style: style_name(s)?, is_check_only_opponent: true }
            } else if let Some(s) = kw.strip_prefix("running_style_temptation_count_") {
                K::RunningStyleTemptationCount { style: style_name(s)?, is_check_only_opponent: false }
            } else if let Some(s) = kw.strip_prefix("running_style_count_").and_then(|s| s.strip_suffix("_otherself")) {
                let style = style_name(s)?;
                let mut c = running_style_count(race, style);
                if race.horses[owner].horse_data.running_style as i32 == style {
                    c -= 1;
                }
                K::Latched(c)
            } else if let Some(s) = kw.strip_prefix("running_style_count_") {
                let style = style_name(s)?;
                K::Latched(running_style_count(race, style))
            } else {
                return None;
            }
        }
    };
    let mut t = SkillTrigger { op, value_rh: rh, is_activated: false, group: 0, kind };
    // Random-position triggers pick their window now.
    match &mut t.kind {
        K::Distance(init, w) => {
            w.has_distance = true;
            init_distance(race, *init, t.value_rh, w);
        }
        K::AllCornerRandom { distance_set_list, is_initialized } => {
            all_corner_random_init(race, distance_set_list);
            *is_initialized = true;
        }
        _ => {}
    }
    Some(t)
}

fn style_name(s: &str) -> Option<i32> {
    Some(match s {
        "nige" => 1,
        "senko" => 2,
        "sashi" => 3,
        "oikomi" => 4,
        _ => return None,
    })
}

/// `order_rate_in{N}_continue` → OrderInType N/10 - 1.
pub(super) fn order_type(n: &str) -> Option<usize> {
    let n: usize = n.parse().ok()?;
    (n % 10 == 0 && (10..=90).contains(&n)).then(|| n / 10 - 1)
}

fn running_style_count(race: &Race, style: i32) -> i32 {
    if style < 1 {
        return 0;
    }
    field::running_style_count(race, crate::sim::defs::RunningStyle::from_i32(style))
}

fn post_number_max(race: &Race) -> i32 {
    race.horses.iter().map(|h| h.horse_data.post_number).max().unwrap_or(0)
}

/// The three popularity mark ranks, 0-based.
fn popularity_ranks(race: &Race, i: usize) -> [i32; 3] {
    let a = &race.input.horses[i].popularity_mark_rank;
    let g = |k: usize| a.get(k).map_or(0, |v| v - 1);
    [g(0), g(1), g(2)]
}

/// Strictly more horses with the owner's style than with every other style (styles 1..=4).
fn is_running_style_count_max(race: &Race, owner: usize) -> bool {
    let own_style = race.horses[owner].horse_data.running_style as i32;
    let own = running_style_count(race, own_style);
    for s in 1..=4 {
        if s != own_style && running_style_count(race, s) >= own {
            return false;
        }
    }
    true
}
