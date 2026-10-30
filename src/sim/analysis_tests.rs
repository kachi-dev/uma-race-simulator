use crate::{HorseInput, RaceInput, SkillInput};
use super::{race, tick, test_util};

fn fixture(course: i32, seed: i32) -> RaceInput {
    RaceInput {
        random_seed: seed, course_set_id: course, ground_condition: 2, weather: 2,
        season: 4, time: 2, race_type: 5,
        horses: (0..9).map(|i| HorseInput {
            speed: 1600, stamina: 800 + i * 30, pow: 1200, guts: 600, wiz: 100 + i * 100,
            running_style: [1, 1, 2, 2, 2, 3, 3, 4, 4][i as usize], motivation: 5,
            proper_distance: [7; 4], proper_running_style: [7; 4], proper_ground: [7; 2],
            popularity: i + 1, frame_order: i + 1, rarity: 3, viewer_id: 1,
            skills: vec![SkillInput {skill_id: 200351, level: 1}, SkillInput {skill_id: 200331, level: 1}],
            ..Default::default()
        }).collect(), ..Default::default()
    }
}

// Hash every output field and the final RNG state. Floats are hashed by bits.
fn fingerprint(input: &RaceInput) -> u64 {
    use std::hash::{Hash, Hasher, DefaultHasher};
    let mut race = race::init(input, test_util::data());
    tick::run(&mut race);
    let rng = format!("{:?}", race.rng);
    let output = super::record::into_output(race);
    let mut h = DefaultHasher::new();
    rng.hash(&mut h);
    for f in output.frames {
        f.time.to_bits().hash(&mut h);
        for x in f.horses {
            [x.distance, x.lane_position, x.speed, x.hp, x.skill_speed, x.skill_accel].map(f32::to_bits).hash(&mut h);
            (x.temptation_mode, x.block_front_horse_index).hash(&mut h);
        }
    }
    for r in output.results {
        [r.finish_time_raw, r.finish_time_scaled, r.finish_diff_time, r.start_delay_time, r.last_spurt_start_distance].map(f32::to_bits).hash(&mut h);
        (r.finish_order, r.guts_order, r.wiz_order, r.running_style, r.defeat).hash(&mut h);
    }
    for e in output.events { (e.time.to_bits(), e.kind, e.params).hash(&mut h); }
    for s in output.spans { (s.horse, s.kind, s.start.to_bits(), s.end.to_bits()).hash(&mut h); }
    format!("{:?}", output.skill_spans).hash(&mut h);
    h.finish()
}

#[test]
fn default_options_match_explicit_normal_behavior() {
    for course in [10101, 11103] {
        for seed in [1, 17, 104729, 81077] {
            let default = fixture(course, seed);
            let mut explicit = default.clone();
            explicit.analysis_options = crate::AnalysisOptions {
                skill_wiz_check: crate::SkillWizCheck::Normal,
                rushed: true, spot_struggle: true, dueling: true, lane_movement: true,
            };
            assert_eq!(fingerprint(&default), fingerprint(&explicit));
        }
    }
}

fn options(race: &mut race::Race<'static>, change: impl FnOnce(&mut crate::AnalysisOptions)) {
    let mut input = race.input.clone();
    change(&mut input.analysis_options);
    race.input = Box::leak(Box::new(input));
}

#[test]
fn skill_lottery_override_preserves_draws() {
    use super::skill;
    let normal = test_util::input(1);
    let mut forced = normal.clone();
    forced.analysis_options.skill_wiz_check = crate::SkillWizCheck::AlwaysPass;
    let mut failures = 0;
    for seed in 0..32 {
        let mut a = test_util::skill_race(&normal, seed);
        let mut b = test_util::skill_race(&forced, seed);
        for r in [&mut a, &mut b] {
            r.horses[0].horse_data.race_param.base_wiz = 1.0;
            r.horses[0].skill_manager.skills[0].skill_master.activate_lot = 1;
            skill::lot_activate_skill(r, 0);
        }
        failures += usize::from(!a.horses[0].skill_manager.skills[0].is_activate_enable);
        assert!(b.horses[0].skill_manager.skills[0].is_activate_enable);
        assert_eq!(a.rng.draws(), 1);
        assert_eq!(a.rng.random_float(100.0).to_bits(), b.rng.random_float(100.0).to_bits());
    }
    assert!(failures > 0);
}

#[test]
fn rushed_off_keeps_initial_lottery_but_blocks_admission() {
    use super::ai::temptation;
    let mut successes = 0;
    for seed in 0..64 {
        let mut a = test_util::ai_race(1);
        a.rng = crate::rng::RaceRandom::new(seed);
        a.horses[0].horse_data.race_param.base_wiz = 1.0;
        let mut b = a.clone();
        options(&mut b, |o| o.rushed = false);
        temptation::lot_temptation(&mut a, 0);
        temptation::lot_temptation(&mut b, 0);
        successes += usize::from(a.horses[0].ai.temptation.is_temptation_start_enable);
        assert!(!b.horses[0].ai.temptation.is_temptation_start_enable);
        assert_eq!(a.rng.draws(), b.rng.draws());
        assert_eq!(a.rng.random_int(100), b.rng.random_int(100));
        b.horses[0].ai.temptation.is_temptation_start_enable = true;
        b.horses[0].ai.temptation.temptation_start_section = 0;
        temptation::update(&mut b, 0, race::DT);
        assert!(!b.horses[0].ai.temptation.is_temptation);
    }
    assert!(successes > 0);
}

#[test]
fn spot_struggle_off_blocks_flags_groups_and_events() {
    use super::{ai::compete_top, defs::RunningStyle, field};
    let mut a = test_util::ai_race(2);
    for h in &mut a.horses {
        h.horse_data.running_style = RunningStyle::Nige;
        h.distance = a.data.param.CompeteTop.CheckStartDistance;
        h.horse_data.race_param.base_guts = 600.0;
        h.proper_running_style_coef = 1.0;
    }
    a.info.course_section_distance = 100.0;
    field::cache_same_running_style_horses(&mut a);
    let mut b = a.clone();
    options(&mut b, |o| o.spot_struggle = false);
    compete_top::update(&mut a, race::DT);
    compete_top::update(&mut b, race::DT);
    assert!(a.horses.iter().all(|h| h.is_compete_top));
    assert!(b.horses.iter().all(|h| !h.is_compete_top && h.compete_top_count == 0));
    assert_eq!(b.field.compete_top.start_count, [0, 0]);
    assert!(b.recorder.events.is_empty());
}

#[test]
fn dueling_off_blocks_flags_groups_and_events() {
    use super::ai::compete_fight;
    let mut a = test_util::ai_race(2);
    for h in &mut a.horses {
        h.hp = 2000.0; h.max_hp = 2000.0;
        h.compete_fight_near = vec![0];
        h.ai.around_horses[0].compete_fight_candidate_continue_time = a.data.param.CompeteFight.TargetContinueTime;
    }
    let mut b = a.clone();
    options(&mut b, |o| o.dueling = false);
    compete_fight::update(&mut a);
    compete_fight::update(&mut b);
    assert!(a.horses.iter().all(|h| h.is_compete_fight));
    assert!(b.horses.iter().all(|h| !h.is_compete_fight && h.compete_fight_count == 0));
    assert!(b.recorder.events.is_empty());
}

#[test]
fn lane_off_keeps_overlap_separation() {
    use super::{ai::lane, field};
    let mut a = test_util::ai_race(3);
    options(&mut a, |o| o.lane_movement = false);
    for (i, h) in a.horses.iter_mut().enumerate() {
        h.distance = 10.0 + i as f32 * 0.1;
        h.lane_distance = 0.5 + i as f32 * 0.01;
        h.ai.target_lane = 1.0;
        h.ai.is_force_in_move_enable = true;
    }
    for i in 0..3 {
        let before = a.horses[i].lane_distance;
        let draws = a.rng.draws();
        lane::update_move_lane(&mut a, i);
        lane::update_force_in_move(&mut a, i);
        lane::move_lane(&mut a, i, race::DT);
        assert_eq!(a.horses[i].lane_distance.to_bits(), before.to_bits());
        assert_eq!(a.rng.draws(), draws);
        assert!(!a.horses[i].ai.is_force_in_move_enable);
        assert_eq!(a.horses[i].lane_move_speed, 0.0);
    }
    field::update_overlap(&mut a);
    assert_eq!(a.horses[1].lane_distance, 0.5f32 + 0.022222223);
    assert_eq!(a.horses[2].lane_distance, (0.5f32 + 0.022222223) + 0.022222223);
}
