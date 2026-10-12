//! Race constants (data/race_paramdefine.json).
//! GENERATED from the JSON: field names = JSON keys verbatim, nested objects / array elements = structs named
//! after their key, floats f32 (JSON holds exact f32 values), ints i32. Skipped: Camera, Sound, Jikkyo (presentation).
#![allow(non_snake_case, non_camel_case_types)]

use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ParamDefine {
    pub paramDefineVersion: String,
    pub Global: Global,
    pub raceBaseSpeedBaseSpeed: f32,
    pub raceBaseSpeedBaseDistance: f32,
    pub raceBaseSpeedDistanceRate: f32,
    pub accelPowCoef: f32,
    pub accelPowCoefUpSlope: f32,
    pub AccelPowCoefSqrt: f32,
    pub declBase: f32,
    pub declRateStart: f32,
    pub declRateMiddle: f32,
    pub declRateEnd: f32,
    pub declRateHpZero: f32,
    #[serde(rename = "DeclRatePositionKeepPaseDown")]
    pub DeclRatePositionKeepPaceDown: f32,
    pub forceInMoveLaneThreshold: f32,
    pub forceInMoveTargetSpeedAddRunningOut: f32,
    pub forceInMoveTargetSpeedAddProceding: f32,
    pub forceInMoveTargetSpeedAddInsertion: f32,
    pub forceInMoveTargetSpeedAddChasing: f32,
    pub forceInMoveTargetSpeedAddRandomRange: f32,
    pub addSpeedParamCoef: f32,
    pub baseTargetSpeedCoef: f32,
    pub LastSpurtTargetSpeedCoefSqrt: f32,
    pub LastSpurtDistanceGoalBuffer: i32,
    pub lastSpurtSpeedDelta: f32,
    pub lastSpurtDistanceMaxFromGoal: f32,
    pub lastSpurtStep2SpeedDelta: f32,
    pub lastSpurtStep2PerVal1: f32,
    pub lastSpurtStep2PerVal2: f32,
    pub LastSpurtTargetSpeedGutsCoef1: f32,
    pub LastSpurtTargetSpeedGutsPow: f32,
    pub LastSpurtTargetSpeedGutsCoef2: f32,
    pub StartSpeed: f32,
    pub StartAccelAdd: f32,
    pub Speed: Speed,
    pub CourseSetAdjust: CourseSetAdjust,
    pub Slope: Slope,
    pub lastSpurtBaseTargetSpeedAddCoef: f32,
    pub _groundModifierParamTurf: Vec<_groundModifierParamTurf>,
    pub _groundModifierParamDirt: Vec<_groundModifierParamDirt>,
    pub BasetTargetSpeed: BasetTargetSpeed,
    pub laneMoveSpeedBase: f32,
    pub laneMoveSpeedAdd: f32,
    pub laneMoveSpeedPowCoef: f32,
    pub laneMoveSpeedOutCoef: f32,
    pub CongestionLaneGapAbs: f32,
    pub CongestionHorseCntThreshold: i32,
    pub laneMoveAccelMoveSpeedBaseCoef: f32,
    pub lastMoveOutLaneNormalizeVal: f32,
    pub lastMoveOutLaneCoef: f32,
    pub lastMoveOutLaneAddRandom: f32,
    pub LastMoveOutStartFinalCornerRate: f32,
    pub Block: Block,
    pub Near: Near,
    pub Hp: Hp,
    pub PositionKeep: PositionKeep,
    pub startDelayMax: f32,
    pub ConservePower: ConservePower,
    pub Temptation: Temptation,
    pub CompeteTop: CompeteTop,
    pub CompeteFight: CompeteFight,
    pub Surrounded: Surrounded,
    pub visibleDistance: f32,
    pub overTakeDistPerSpeed: f32,
    pub overTakeCoolDownTime: f32,
    pub overTakeCountCoolDownTime: f32,
    pub overTakeInLaneCoefEnd: f32,
    pub overTakeOutLaneCoefEnd: f32,
    pub Skill: Skill,
    pub Popularity: Popularity,
    pub Defeat: Defeat,
    pub runMotionSpeedCoef: f32,
    pub runMotionSpeedMin: f32,
    pub runMotionSpeedMax: f32,
    pub runMotionSpeedBaseHorseSpeed: f32,
    pub runMotionSpeedHorseSpeedRate: f32,
    pub runMotionBlendRateMinSpeedOffset: f32,
    pub runMotionBlendRateMaxSpeedOffset: f32,
    pub laneMoveRotYThresholdPerSec: f32,
    pub laneMoveRotYThresholdRotZ: f32,
    pub laneMoveRotYPerMovedLane: f32,
    pub laneMoveRotYMax: f32,
    pub laneMoveRotYRotatePerSec: f32,
    pub laneMoveRotYRotatePerSecToZero: f32,
    pub laneMoveRotZThresholdPerSec: f32,
    pub laneMoveRotZThresholdRotZ: f32,
    pub laneMoveRotZPerMovedLane: f32,
    pub laneMoveRotZMax: f32,
    pub laneMoveRotZRotatePerSec: f32,
    pub laneMoveRotZRotatePerSecToZero: f32,
    pub RunMotionSpeedCoefByRunningType: Vec<f32>,
    pub footDirtBaseEnergy: Vec<f32>,
    pub lowerDirtBaseEnergy: Vec<f32>,
    pub upperDirtBaseEnergy: Vec<f32>,
    pub dirtDistance: f32,
    pub dirtAngle: f32,
    pub footDirtStep: Vec<f32>,
    pub lowerDirtStep: Vec<f32>,
    pub upperDirtStep: Vec<f32>,
    pub EventCameraIntervalSec: f32,
    pub EventCameraUnPlayableDistanceFromGoal: i32,
    pub SkillCutInEventCameraUnPlayableDistanceFromGoalTeamStadium: i32,
    pub BehindPosCheckDistanceThresholdTop: f32,
    pub BehindPosCheckDistanceThreshold: f32,
    pub BehindPosCheckLaneDistanceMeterThreshold: f32,
    pub WaitStartDashSec: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Defeat {
    pub DefeatSameRunningStylePer: i32,
    pub DefeatGutsOrderPer: i32,
    pub DefeatTemptaionCount: i32,
    pub DefeatFrontBlockAccumulateTime: f32,
    pub DefeatLoseFinishOrder: i32,
    pub DefeatLoseBashinDiff: i32,
    pub DefeatProperDistance: i32,
    pub DefeatProperGround: i32,
    pub DefeatMotivation: i32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Popularity {
    pub Z1Add: f32,
    pub Z1Coef: f32,
    pub Z2Add: f32,
    pub Z2Coef: f32,
    pub RankCenterZ1Pow: f32,
    pub RankCenterZ2Pow: f32,
    pub DeviationBase: i32,
    pub DeviationCoef: i32,
    pub DeviationThreshold: i32,
    pub DeviationMinus: i32,
    pub DeviationFix: i32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Skill {
    pub AbilityTimeDivideDistance: i32,
    pub CoolDownTimeDivideDistance: i32,
    pub RandomLotOtherActivateLotInterval: f32,
    pub LotActivatePerVal1: f32,
    pub LotActivatePerVal2: f32,
    pub InfrontHorseNearDistance: f32,
    pub BehindHorseNearDistance: f32,
    pub InfrontHorseNearLaneDistance: f32,
    pub BehindHorseNearLaneDistance: f32,
    pub ActivatePerMin: i32,
    pub TeamTotalStatusCoefAscArray: Vec<TeamTotalStatusCoefAscArray>,
    pub AbilityTimeDiffDistanceTopCoefPerDistance: f32,
    pub AbilityTimeDiffDistanceTopCoefMin: f32,
    pub AbilityTimeDiffDistanceTopCoefMax: f32,
    pub AbiiltyValueUsageRandom1Array: Vec<AbiiltyValueUsageRandom1Array>,
    pub AbiiltyValueUsageRandom2Array: Vec<AbiiltyValueUsageRandom2Array>,
    pub BehindNearParamArray: Vec<BehindNearParamArray>,
    pub LaneMoveAddParam1: f32,
    pub LaneMoveAddParam2: f32,
    pub TimeRemainHpCoefAscArray: Vec<TimeRemainHpCoefAscArray>,
    pub TimeRemainHpCoefAscArray2: Vec<TimeRemainHpCoefAscArray2>,
    pub RaceWinCountCoefAscArray: Vec<RaceWinCountCoefAscArray>,
    pub OrderUpCornerPhaseEndAfterCoefAscArray: Vec<OrderUpCornerPhaseEndAfterCoefAscArray>,
    pub BaseWizCoefAscArray: Vec<BaseWizCoefAscArray>,
    pub MaximumRawStatusCoefAscArray: Vec<MaximumRawStatusCoefAscArray>,
    pub OrderUpAddAbilityTime: f32,
    pub OrderUpAddAbilityTimeMaxCount: i32,
    pub SkillActivateTagGroupArray: Vec<SkillActivateTagGroupArray>,
    pub AbilityValueUsageTagGroup1Array: Vec<i32>,
    pub ActivateSpecificTagGroupSkillCountCoefAscArray: Vec<ActivateSpecificTagGroupSkillCountCoefAscArray>,
    pub FanCountCoefAscArray: Vec<FanCountCoefAscArray>,
    pub AdditionalActivateAbilityMaxCountArray: Vec<AdditionalActivateAbilityMaxCountArray>,
    pub DistanceDiffTopCoefAscArray: Vec<DistanceDiffTopCoefAscArray>,
    pub BlockedSideMaxContinueTimePhaseMiddleRunAbilityValueCoefAscArray1: Vec<BlockedSideMaxContinueTimePhaseMiddleRunAbilityValueCoefAscArray1>,
    pub BlockedSideMaxContinueTimePhaseMiddleRunAbilityValueCoefAscArray2: Vec<BlockedSideMaxContinueTimePhaseMiddleRunAbilityValueCoefAscArray1>,
    pub BlockedSideMaxContinueTimePhaseMiddleRunAbilityTimeCoefAscArray1: Vec<BlockedSideMaxContinueTimePhaseMiddleRunAbilityTimeCoefAscArray1>,
    pub BlockedSideMaxContinueTimePhaseMiddleRunAbilityTimeCoefAscArray2: Vec<BlockedSideMaxContinueTimePhaseMiddleRunAbilityTimeCoefAscArray1>,
    pub SpeedAbilityValueCoefAscArray1: Vec<SpeedAbilityValueCoefAscArray1>,
    pub SpeedAbilityValueCoefAscArray2: Vec<SpeedAbilityValueCoefAscArray2>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct SpeedAbilityValueCoefAscArray2 {
    pub SpeedThreshold: i32,
    pub Multiply: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct SpeedAbilityValueCoefAscArray1 {
    pub SpeedThreshold: i32,
    pub Multiply: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct BlockedSideMaxContinueTimePhaseMiddleRunAbilityTimeCoefAscArray1 {
    pub BlockedSideMaxContinueTimeThreshold: i32,
    pub Multiply: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct BlockedSideMaxContinueTimePhaseMiddleRunAbilityValueCoefAscArray1 {
    pub BlockedSideMaxContinueTimeThreshold: i32,
    pub Multiply: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct DistanceDiffTopCoefAscArray {
    pub DistanceDiffTopThreshold: i32,
    pub AddValue: i32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct AdditionalActivateAbilityMaxCountArray {
    pub AdditionalActivateType: i32,
    pub AdditionalActivateMaxCount: i32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct FanCountCoefAscArray {
    pub FanCountThreshold: i32,
    pub Multiply: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ActivateSpecificTagGroupSkillCountCoefAscArray {
    pub ActivateSpecificTagGroupSkillCountThreshold: i32,
    pub Multiply: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct SkillActivateTagGroupArray {
    pub TagArray: Vec<i32>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct MaximumRawStatusCoefAscArray {
    pub BaseStatusThreshold: f32,
    pub Multiply: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct BaseWizCoefAscArray {
    pub BaseStatusThreshold: f32,
    pub Multiply: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct OrderUpCornerPhaseEndAfterCoefAscArray {
    pub OrderChangeCountThreshold: i32,
    pub Multiply: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct RaceWinCountCoefAscArray {
    pub WinCountThreshold: i32,
    pub Multiply: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct TimeRemainHpCoefAscArray2 {
    pub HpThreshold: f32,
    pub Multiply: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct TimeRemainHpCoefAscArray {
    pub HpThreshold: f32,
    pub Multiply: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct BehindNearParamArray {
    pub Distance: f32,
    pub LaneDistance: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct AbiiltyValueUsageRandom2Array {
    pub AbilityValueCoef: f32,
    pub Per: i32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct AbiiltyValueUsageRandom1Array {
    pub AbilityValueCoef: f32,
    pub Per: i32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct TeamTotalStatusCoefAscArray {
    pub StatusThreshold: f32,
    pub Multiply: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Surrounded {
    pub DistanceGapAbs1: f32,
    pub LaneGap1: f32,
    pub DistanceGap2: f32,
    pub LaneGapAbs2: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct CompeteFight {
    pub DistanceGap: f32,
    pub LaneGap: f32,
    pub TargetContinueTime: f32,
    pub SpeedGap: f32,
    pub TargetOrderPer: i32,
    pub TargetContinueDistance: f32,
    pub HpPer: i32,
    pub HpPer2: i32,
    pub AddParam1Coef1: f32,
    pub AddParam1Coef2: f32,
    pub AddParam1Coef3: f32,
    pub AddParam2Coef1: f32,
    pub AddParam2Coef2: f32,
    pub AddParam2Coef3: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct CompeteTop {
    pub CheckStartDistance: f32,
    pub CheckEndSection: i32,
    pub EndSection: i32,
    pub NigeCount: i32,
    pub OonigeCount: i32,
    pub DistanceGap1: f32,
    pub LaneGap1: f32,
    pub DistanceGap2: f32,
    pub LaneGap2: f32,
    pub TimeCoef1: f32,
    pub TimeCoef2: f32,
    pub TimeCoef3: f32,
    pub AddParam1Coef1: f32,
    pub AddParam1Coef2: f32,
    pub AddParam1Coef3: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Temptation {
    pub TemptationLotSectionMin: i32,
    pub TemptationLotSectionMax: i32,
    pub TemptationStartPerVal1: f32,
    pub TemptationForceEndTime: f32,
    pub TemptationEndCheckTime: f32,
    pub TemptationEndPer: f32,
    pub OikomiToSashiPer: f32,
    pub OikomiToSenkoPer: f32,
    pub OikomiToNigePer: f32,
    pub SashiToSenkoPer: f32,
    pub SashiToNigePer: f32,
    pub SenkoToNigePer: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ConservePower {
    pub ReleaseConservePowerAccelCoef: f32,
    pub ReleaseConservePowerDeclCoef: f32,
    pub ReleaseConservePowerAccelRunningStyleAndDistanceTypeCoefArray: Vec<ReleaseConservePowerAccelRunningStyleAndDistanceTypeCoefArray>,
    pub ConservePowerDecreaseAccelCoefArray: Vec<ConservePowerDecreaseAccelCoefArray>,
    pub ReleaseConservePowerActivityTimeCoef: f32,
    pub ReleaseConservePowerActivityTimeDistanceTypeCoefArray: Vec<ReleaseConservePowerActivityTimeDistanceTypeCoefArray>,
    pub ConservePowerDefaultCoolDownTime: f32,
    pub ConservePowerIncreaseCoefArray: Vec<ConservePowerIncreaseCoefArray>,
    pub ConservePowerRunningStyleCoefArray: Vec<ConservePowerRunningStyleCoefArray>,
    pub ConservePowerDecreaseCoefArray: Vec<ConservePowerDecreaseCoefArray>,
    pub ConservePowerReleasableActivityTimeThreshold: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ConservePowerDecreaseCoefArray {
    pub Mode: i32,
    pub Coef: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ConservePowerRunningStyleCoefArray {
    pub RunningStyle: i32,
    pub Coef: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ConservePowerIncreaseCoefArray {
    pub Mode: i32,
    pub Coef: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ReleaseConservePowerActivityTimeDistanceTypeCoefArray {
    pub DistanceType: i32,
    pub Coef: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ConservePowerDecreaseAccelCoefArray {
    pub Mode: i32,
    pub Coef: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ReleaseConservePowerAccelRunningStyleAndDistanceTypeCoefArray {
    pub RunningStyle: i32,
    pub DistanceType: i32,
    pub Coef: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct PositionKeep {
    pub PositionKeepCheckIntervalSec: i32,
    pub PositionKeepCoolDownSec: i32,
    pub PositionKeepStartSection: i32,
    pub PositionKeepEndSection: i32,
    pub PositionKeepContinueSection: i32,
    pub PositionKeepContinueSectionExArray: Vec<i32>,
    pub PositionKeepSpeedUpStartPerVal1: f32,
    pub PositionKeepSpeedUpBaseTargetSpeedMultiply: f32,
    pub PositionKeepSpeedUpEndDistanceDiff: f32,
    pub PositionKeepSpeedUpEndDistanceDiffOne: f32,
    pub PositionKeepOvertakeStartPerVal1: f32,
    pub PositionKeepOvertakeEndDistanceDiff: f32,
    pub PositionKeepOvertakeBaseTargetSpeedMultiply: f32,
    pub PositionKeepSpeedUpStartPerVal1Oonige: f32,
    pub PositionKeepSpeedUpBaseTargetSpeedMultiplyOonige: f32,
    pub PositionKeepSpeedUpEndDistanceDiffOonige: f32,
    pub PositionKeepOvertakeStartPerVal1Oonige: f32,
    pub PositionKeepOvertakeEndDistanceDiffOonige: f32,
    pub PositionKeepOvertakeBaseTargetSpeedMultiplyOonige: f32,
    #[serde(rename = "PositionKeepPaseUpDownDistanceDiffMaxCoef")]
    pub PositionKeepPaceUpDownDistanceDiffMaxCoef: f32,
    #[serde(rename = "PositionKeepPaseUpDownDistanceDiffMinSenko")]
    pub PositionKeepPaceUpDownDistanceDiffMinSenko: f32,
    #[serde(rename = "PositionKeepPaseUpDownDistanceDiffMaxSenko")]
    pub PositionKeepPaceUpDownDistanceDiffMaxSenko: f32,
    #[serde(rename = "PositionKeepPaseUpDownDistanceDiffMinSashi")]
    pub PositionKeepPaceUpDownDistanceDiffMinSashi: f32,
    #[serde(rename = "PositionKeepPaseUpDownDistanceDiffMaxSashi")]
    pub PositionKeepPaceUpDownDistanceDiffMaxSashi: f32,
    #[serde(rename = "PositionKeepPaseUpDownDistanceDiffMinOikomi")]
    pub PositionKeepPaceUpDownDistanceDiffMinOikomi: f32,
    #[serde(rename = "PositionKeepPaseUpDownDistanceDiffMaxOikomi")]
    pub PositionKeepPaceUpDownDistanceDiffMaxOikomi: f32,
    #[serde(rename = "PositionKeepPaseBaseDistance")]
    pub PositionKeepPaceBaseDistance: i32,
    #[serde(rename = "PositionKeepPaseBaseDistanceCoef")]
    pub PositionKeepPaceBaseDistanceCoef: f32,
    #[serde(rename = "PositionKeepPaseDownBaseTargetSpeedMultiply")]
    pub PositionKeepPaceDownBaseTargetSpeedMultiply: f32,
    #[serde(rename = "PositionKeepPaseDownBaseTargetSpeedMultiplyPhaseMiddle")]
    pub PositionKeepPaceDownBaseTargetSpeedMultiplyPhaseMiddle: f32,
    #[serde(rename = "PositionKeepPaseDownTargetLane")]
    pub PositionKeepPaceDownTargetLane: f32,
    #[serde(rename = "PositionKeepPaseUpStartPerVal1")]
    pub PositionKeepPaceUpStartPerVal1: f32,
    #[serde(rename = "PositionKeepPaseUpBaseTargetSpeedMultiply")]
    pub PositionKeepPaceUpBaseTargetSpeedMultiply: f32,
    #[serde(rename = "PositionKeepPaseUpExBaseTargetSpeedMultiply")]
    pub PositionKeepPaceUpExBaseTargetSpeedMultiply: f32,
    #[serde(rename = "PaseMakerChangeForwardRunningStyleRange")]
    pub PacemakerChangeForwardRunningStyleRange: f32,
    #[serde(rename = "PaseMakerChangeMostForwardStyleRange")]
    pub PacemakerChangeMostForwardStyleRange: f32,
    #[serde(rename = "PaseMakerChangeTopHorseNotMostForwardStyleCount")]
    pub PacemakerChangeTopHorseNotMostForwardStyleCount: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Hp {
    pub HpInitialVal1: f32,
    pub HpDecBase: f32,
    pub HpMaxCoefOoNige: f32,
    pub HpMaxCoefNige: f32,
    pub HpMaxCoefSenko: f32,
    pub HpMaxCoefSashi: f32,
    pub HpMaxCoefOikomi: f32,
    pub SpeedGapParam1: f32,
    pub SpeedGapParam1Pow: f32,
    pub HpDecRateBaseNormal: f32,
    pub HpDecRateBaseTemptation: f32,
    #[serde(rename = "HpDecRateBasePositionKeepPaseDown")]
    pub HpDecRateBasePositionKeepPaceDown: f32,
    pub HpDecRateBaseCompeteTopNige: f32,
    pub HpDecRateBaseTemptationAndCompeteTopNige: f32,
    pub HpDecRateBaseCompeteTopOonige: f32,
    pub HpDecRateBaseTemptationAndCompeteTopOonige: f32,
    pub HpDecRateMultiplyDownSlopeAccelMode: f32,
    pub HpGutsCoef: f32,
    pub HpGutsCoefSqrt: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Near {
    pub DistanceThreshold: f32,
    pub LaneDistanceThreshold: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Block {
    pub FrontBlockDistanceGap: f32,
    pub FrontBlockLaneGap: f32,
    pub SideBlockDistanceGap: f32,
    pub SideBlockLaneGap: f32,
    pub FrontBlockMaxSpeedRateMin: f32,
    pub FrontBlockMaxSpeedRateMax: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct BasetTargetSpeed {
    pub PhaseBaseTargetSpeedPerArray: Vec<PhaseBaseTargetSpeedPerArray>,
    pub PhaseBaseTargetSpeedPerExArray: Vec<PhaseBaseTargetSpeedPerExArray>,
    pub BaseTargetSpeedRandomMinusVal1: f32,
    pub BaseTargetSpeedRandomMinusVal2: f32,
    pub BaseTargetSpeedRandomPlusVal1: f32,
    pub PhaseEndBaseTargetSpeedCoef: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct PhaseBaseTargetSpeedPerExArray {
    pub Start: f32,
    pub Middle: f32,
    pub End: f32,
    pub Last: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct PhaseBaseTargetSpeedPerArray {
    pub Start: f32,
    pub Middle: f32,
    pub End: f32,
    pub Last: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct _groundModifierParamDirt {
    pub addSpeed: f32,
    pub addPower: f32,
    pub multiHpSub: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct _groundModifierParamTurf {
    pub addSpeed: f32,
    pub addPower: f32,
    pub multiHpSub: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Slope {
    pub SlopePerThreshold: f32,
    pub UpSlopeAddSpeedVal1: f32,
    pub DownSlopeAddSpeedVal1: f32,
    pub DownSlopeAddSpeedVal2: f32,
    pub DownSlopeAddSpeedStartWizRate: f32,
    pub DownSlopeAddSpeedEndPer: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct CourseSetAdjust {
    pub CoefAndThresholdArray: Vec<CoefAndThresholdArray>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct CoefAndThresholdArray {
    pub Coef: f32,
    pub StatusThreshold: i32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Speed {
    pub MinSpeedRate: f32,
    pub MinSpeedGutsCoefSqrt: f32,
    pub MinSpeedGutsCoef: f32,
    pub TargetSpeedMin: f32,
    pub ExpectLastSpurtSpeed: f32,
    pub PhaseAccelCoefArray: Vec<PhaseAccelCoefArray>,
    pub PhaseAccelCoefExArray: Vec<PhaseAccelCoefExArray>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct PhaseAccelCoefExArray {
    pub Start: f32,
    pub Middle: f32,
    pub End: f32,
    pub Last: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct PhaseAccelCoefArray {
    pub Start: f32,
    pub Middle: f32,
    pub End: f32,
    pub Last: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Global {
    pub FinishTimeCoef: f32,
    pub SingleModeAddSpeed: f32,
    pub SingleModeAddStamina: f32,
    pub SingleModeAddPow: f32,
    pub SingleModeAddGuts: f32,
    pub SingleModeAddWiz: f32,
}
