//! Shared enums, numbered like the game's. Stable: add variants, never renumber.
//!
//! Data-driven ids whose value set can grow with game updates (skill ability/usage/target types, rarity,
//! ground type, course param type, ...) stay plain `i32` on the structs, with the constants here.

#[repr(i32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Phase {
    #[default]
    Start = 0,
    MiddleRun = 1,
    End = 2,
    LastSpurt = 3,
    Finished = 4,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RunningStyle {
    #[default]
    None = 0,
    Nige = 1,
    Senko = 2,
    Sashi = 3,
    Oikomi = 4,
}

impl RunningStyle {
    /// Enum order, as used by the per-style arrays.
    pub const ALL: [RunningStyle; 5] = [Self::None, Self::Nige, Self::Senko, Self::Sashi, Self::Oikomi];

    pub fn from_i32(v: i32) -> Self {
        match v {
            1 => Self::Nige,
            2 => Self::Senko,
            3 => Self::Sashi,
            4 => Self::Oikomi,
            _ => Self::None,
        }
    }
}

/// Extended running style (Oonige, the runaway Nige).
#[repr(i32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RunningStyleEx {
    #[default]
    None = 0,
    Oonige = 1,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SlopeType {
    #[default]
    Null = 0,
    Up = 1,
    Down = 2,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LaneDirection {
    #[default]
    In = 0,
    Out = 1,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PositionKeepMode {
    #[default]
    Null = 0,
    SpeedUp = 1,
    Overtake = 2,
    PaceUp = 3,
    PaceDown = 4,
    PaceUpEx = 5,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SkillModifierParam {
    Speed = 0,
    Stamina = 1,
    Power = 2,
    Guts = 3,
    Wiz = 4,
    CurrentSpeed = 5,
    TargetSpeed = 6,
    LaneMoveSpeed = 7,
    TemptationPer = 8,
    PushPer = 9,
    Accel = 10,
    TargetLane = 11,
    Hp = 12,
    HpDecRate = 13,
    TemptationEndTime = 14,
    StartDelayScale = 15,
    ForceOvertakeIn = 16,
    ForceOvertakeOut = 17,
    DebuffCancel = 18,
    VisibleDistance = 19,
    RunningStyleExOonige = 20,
    TargetSpeedOnMoveLane = 21,
    StartDelayFix = 22,
    CurrentSpeedWithNaturalDeceleration = 23,
    ActivateRandomNormalAndRareSkill = 24,
    ActivateRandomRareSkill = 25,
}

/// Last-spurt calculation result (flags; stored as i32).
pub mod last_spurt_calc_result {
    pub const TRUE_EXCEED_NEED_MAX_HP: i32 = 1;
    pub const TRUE: i32 = 2;
    pub const FALSE_BELOW_NEED_MIN_HP: i32 = 4;
    pub const FALSE: i32 = 8;
    pub const TRUE_FLAGS: i32 = 3;
    pub const FALSE_FLAGS: i32 = 12;
}

pub mod ground_type {
    pub const TURF: i32 = 1;
    pub const DIRT: i32 = 2;
}

pub mod straight_front_type {
    pub const NULL: i32 = 0;
    pub const FRONT: i32 = 1;
    pub const ACROSS_FRONT: i32 = 2;
}

/// The values the sim branches on.
pub mod race_type {
    pub const STORY: i32 = 3;
    pub const CHALLENGE_MATCH: i32 = 13;
}

/// `race_track` column.
pub mod initial_lane_type {
    pub const EXTRA_SPACE_AFTER_9: i32 = 1;
    pub const EQUIDISTANT: i32 = 2;
    pub const EXTRA_SPACE_AFTER_14: i32 = 3;
    pub const EXTRA_SPACE_AFTER_8: i32 = 4;
}
