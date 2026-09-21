use serde::Serialize;

use super::{
    ai_character::{RobotsAiHandlerClass, RobotsAiPatrolConfig},
    circle_target::RobotsCircleTargetConfig,
    ef01_mine::{
        ef01_mine_patrol_config, ROBOTS_EF01_ELECTRO_HIT_ANIM_MODE,
        ROBOTS_EF01_PERIODIC_IDLE_ANIM_MODES, ROBOTS_EF01_PERIODIC_IDLE_BASE_DELAY_TICKS,
        ROBOTS_EF01_PERMANENT_SOUND_NATIVE_PARAMETER, ROBOTS_EF01_PERMANENT_SOUND_UID,
    },
    ef03_evilbot::{
        ROBOTS_EF03_FLY_LOOP_NATIVE_PARAMETER, ROBOTS_EF03_FLY_LOOP_SOUND_UID,
    },
    ew09_armoured::{ew09_attack_config, ROBOTS_EW09_PATROL_SCALAR},
    flee_navmesh::RobotsFleeNavMeshConfig,
    generic_attack::RobotsGenericAttackConfig,
    hit_reaction::RobotsCommonAiHitConfig,
    locomotion::RobotsAiTurnRateInput,
    pursue_navmesh::{
        ROBOTS_MALFBOT_PURSUE_NAV_PRELUDE_ANIM_MODE, ROBOTS_PURSUE_NAV_NO_STOP_DISTANCE,
    },
    shunt_attack::RobotsShuntAttackConfig,
    spike_attack::RobotsSpikeAttackConfig,
    stalk_navmesh::RobotsStalkNavMeshConfig,
    three_phase_attack::RobotsThreePhaseAttackConfig,
    turn_then_attack::RobotsTurnThenAttackConfig,
};

pub const ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_ANIM_MODES: [u32; 2] = [0x0900_0006, 0x0900_0007];
pub const ROBOTS_SAWBOT_PERIODIC_IDLE_ANIM_MODES: [u32; 1] = [0x0900_0006];
pub const ROBOTS_JAILBOT_PERIODIC_IDLE_ANIM_MODES: [u32; 4] =
    [0x0900_0006, 0x0900_0007, 0x0900_0008, 0x0900_0009];
pub const ROBOTS_STANDARD_MONSTER_MAX_ATTACKS: usize = 3;
pub const ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_BASE_DELAY_TICKS: i32 = 180;
pub const ROBOTS_EW10_EB14_ATTACK_YAW_TOLERANCE_RADIANS: f32 = f32::from_bits(0x3e86_0a92);
pub const ROBOTS_JAILBOT_ATTACK1_YAW_TOLERANCE_RADIANS: f32 = f32::from_bits(0x3e86_0a92);
pub const ROBOTS_JAILBOT_ATTACK2_YAW_TOLERANCE_RADIANS: f32 = f32::from_bits(0x3dfa_35dd);
pub const ROBOTS_EW10_ATTACK_OUTER_RADIUS: f32 = 1.5;
pub const ROBOTS_EB14_ATTACK_OUTER_RADIUS: f32 = 2.0;
pub const ROBOTS_EW10_EB14_ATTACK1_ANIM_MODE: u32 = 0x0900_0025;
pub const ROBOTS_EW10_EB14_ATTACK2_ANIM_MODE: u32 = 0x0900_0027;
pub const ROBOTS_EW10_EB14_ELECTRO_HIT_ANIM_MODE: u32 = 0x0900_00b1;
pub const ROBOTS_MALFBOT_ELECTRO_HIT_ANIM_MODE: u32 = 0x0900_007d;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsPermanentSoundConfig {
    pub sound_uid: u32,
    /// Third explicit argument passed by the native class to
    /// `XSoundTag::CreatePermanentSound`. Its higher-level meaning is not yet
    /// named; preserving the exact value is sufficient for the runtime seam.
    pub native_parameter: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsStandardMonsterBehaviorConfig {
    pub handler_class: RobotsAiHandlerClass,
    pub patrol: RobotsAiPatrolConfig,
    pub periodic_idle_base_delay_ticks: i32,
    pub periodic_idle_anim_modes: &'static [u32],
    pub pursue_enabled: bool,
    pub pursue_prelude_anim_mode: Option<u32>,
    pub pursue_stop_distance: f32,
    /// Native class first-update transform scale. Most Monster handlers keep the
    /// XItem ctor scale of 1.0; JailBotLarge and EW08 FlambeLarge share the proven
    /// `0x00462BE0 -> 0x00455E60(2.0)` one-shot uniform transform scale.
    pub first_update_uniform_scale: f32,
    pub stalk: Option<RobotsStalkNavMeshConfig>,
    pub attacks: [Option<RobotsGenericAttackConfig>; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
    pub common_hit: Option<RobotsCommonAiHitConfig>,
    pub shunt_attack: Option<RobotsShuntAttackConfig>,
    pub spike_attack: Option<RobotsSpikeAttackConfig>,
    pub turn_then_attacks:
        [Option<RobotsTurnThenAttackConfig>; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
    pub secondary_three_phase_attack: Option<RobotsThreePhaseAttackConfig>,
    pub has_status_hit_nodes: bool,
    pub uses_common_monster_attack_gate: bool,
    pub electro_hit_anim_mode: u32,
}

pub const fn standard_monster_physics_locomotion_speed_range(
    handler_class: RobotsAiHandlerClass,
) -> Option<[f32; 2]> {
    match handler_class {
        // 0x00462730: Handler+0x5F8=1, +0x5DC/+0x5E0 = 5/5.
        RobotsAiHandlerClass::Ew08Flambe | RobotsAiHandlerClass::Ew08FlambeLarge => {
            Some([5.0, 5.0])
        }
        // 0x00462F90: physical locomotion range 2..8.
        RobotsAiHandlerClass::Ew09Armoured => Some([2.0, 8.0]),
        // 0x00463620: +0x609=0 and fixed physical speed 2.
        RobotsAiHandlerClass::Ef01Mine => Some([2.0, 2.0]),
        // 0x00463BD0: +0x609=0 and physical range 1..5.
        RobotsAiHandlerClass::Ew10Minion => Some([1.0, 5.0]),
        _ => None,
    }
}

impl RobotsStandardMonsterBehaviorConfig {
    pub const fn for_handler_class(handler_class: RobotsAiHandlerClass) -> Option<Self> {
        match handler_class {
            RobotsAiHandlerClass::Monster2Rockets => Some(Self::monster_2rockets()),
            RobotsAiHandlerClass::MalfBot => Some(Self::malfbot()),
            RobotsAiHandlerClass::Ef01Mine => Some(Self::ef01_mine()),
            RobotsAiHandlerClass::ConstructionBot => Some(Self::constructionbot()),
            RobotsAiHandlerClass::ThiefBot => Some(Self::thiefbot()),
            RobotsAiHandlerClass::SecurityBot => Some(Self::securitybot()),
            RobotsAiHandlerClass::GuardBot => Some(Self::guardbot()),
            RobotsAiHandlerClass::ShieldBot => Some(Self::shieldbot()),
            RobotsAiHandlerClass::Eb07MineBot => Some(Self::eb07_minebot()),
            RobotsAiHandlerClass::Ew09Armoured => Some(Self::ew09_armoured()),
            RobotsAiHandlerClass::Eb11MagnaBot => Some(Self::magnabot()),
            RobotsAiHandlerClass::Eb12EvilBot => Some(Self::eb12_evilbot()),
            RobotsAiHandlerClass::Eb13KnightBot => Some(Self::eb13_knightbot()),
            RobotsAiHandlerClass::Eb15Launcher => Some(Self::eb15_launcher()),
            RobotsAiHandlerClass::Eb16KnuckleBot => Some(Self::eb16_knucklebot()),
            RobotsAiHandlerClass::Eq03Spider => Some(Self::eq03_spider()),
            RobotsAiHandlerClass::Ew08Flambe => Some(Self::flambe()),
            RobotsAiHandlerClass::Ew08FlambeLarge => Some(Self::flambe_large()),
            RobotsAiHandlerClass::Ew10Minion => Some(Self::ew10_minion()),
            RobotsAiHandlerClass::Ew11FatBot => Some(Self::ew11_fatbot()),
            RobotsAiHandlerClass::Ef03EvilBot => Some(Self::ef03_evilbot()),
            RobotsAiHandlerClass::Eb14Minion => Some(Self::eb14_minion()),
            RobotsAiHandlerClass::SawBot => Some(Self::sawbot()),
            RobotsAiHandlerClass::SpikeBot => Some(Self::spikebot()),
            RobotsAiHandlerClass::JailBotNormal => Some(Self::jailbot_normal()),
            RobotsAiHandlerClass::JailBotLarge => Some(Self::jailbot_large()),
            RobotsAiHandlerClass::ShuntBot => Some(Self::shuntbot()),
            RobotsAiHandlerClass::ShuntBotBoss => Some(Self::shuntbot_boss()),
            RobotsAiHandlerClass::Sweeper => Some(Self::sweeper()),
            _ => None,
        }
    }

    pub const fn physics_locomotion_speed_range(self) -> Option<[f32; 2]> {
        standard_monster_physics_locomotion_speed_range(self.handler_class)
    }

    /// Sweeper builder `0x00460380` starts with PeriodicIdle and never installs
    /// the ordinary `AI_Patrol` node. Every previously hosted standard Monster does.
    pub const fn has_patrol_node(self) -> bool {
        !matches!(self.handler_class, RobotsAiHandlerClass::Sweeper)
    }

    pub const fn has_magnetic_hit_node(self) -> bool {
        self.has_status_hit_nodes
            && !matches!(self.handler_class, RobotsAiHandlerClass::Eb11MagnaBot)
    }

    pub const fn circle_target(self) -> Option<RobotsCircleTargetConfig> {
        match self.handler_class {
            RobotsAiHandlerClass::GuardBot => Some(RobotsCircleTargetConfig::guardbot()),
            RobotsAiHandlerClass::Ew08Flambe | RobotsAiHandlerClass::Ew08FlambeLarge => {
                Some(RobotsCircleTargetConfig::flambe())
            }
            _ => None,
        }
    }

    pub const fn direct_turn_then_attack(self) -> Option<RobotsTurnThenAttackConfig> {
        match self.handler_class {
            RobotsAiHandlerClass::Ew08Flambe | RobotsAiHandlerClass::Ew08FlambeLarge => {
                Some(RobotsTurnThenAttackConfig::flambe())
            }
            _ => None,
        }
    }

    pub const fn flee_navmesh(self) -> Option<RobotsFleeNavMeshConfig> {
        match self.handler_class {
            RobotsAiHandlerClass::Eb07MineBot => Some(RobotsFleeNavMeshConfig::eb07_minebot()),
            _ => None,
        }
    }

    /// EB07 installs its reusable three-phase node as AttackGroup child1. Keep
    /// this distinct from EB13's independent Handler+0x4D8 secondary host.
    pub const fn attack_group_three_phase(self) -> Option<RobotsThreePhaseAttackConfig> {
        match self.handler_class {
            RobotsAiHandlerClass::Eb07MineBot => Some(RobotsThreePhaseAttackConfig::eb07_minebot()),
            _ => None,
        }
    }

    pub const fn attack_group_three_phase_index(self) -> Option<usize> {
        match self.handler_class {
            RobotsAiHandlerClass::Eb07MineBot => Some(1),
            _ => None,
        }
    }

    /// Handler+0x618 is the signed-byte retained HitCheck-record capacity consumed
    /// by common script-event handler `0x00453160`. Common Monster service keeps 1;
    /// Monster_2Rockets vslot +0x08 `0x0045B6E0` raises it to exactly 2 after
    /// delegating to `0x004514F0`.
    pub const fn hit_query_capacity(self) -> usize {
        match self.handler_class {
            RobotsAiHandlerClass::Monster2Rockets => 2,
            _ => 1,
        }
    }

    /// Class first-update side effect that lives outside the behavior-builder list.
    /// Native EF03 `+0x100 = 0x00467540` first runs the common Monster update and
    /// then performs `Find(0x1AF00155) -> CreatePermanentSound` once.
    pub const fn first_update_permanent_sound(self) -> Option<RobotsPermanentSoundConfig> {
        match self.handler_class {
            RobotsAiHandlerClass::Ef01Mine => Some(RobotsPermanentSoundConfig {
                sound_uid: ROBOTS_EF01_PERMANENT_SOUND_UID,
                native_parameter: ROBOTS_EF01_PERMANENT_SOUND_NATIVE_PARAMETER,
            }),
            RobotsAiHandlerClass::Ef03EvilBot => Some(RobotsPermanentSoundConfig {
                sound_uid: ROBOTS_EF03_FLY_LOOP_SOUND_UID,
                native_parameter: ROBOTS_EF03_FLY_LOOP_NATIVE_PARAMETER,
            }),
            _ => None,
        }
    }

    /// EF01 builder `0x00463680`. The conditional route node is appended later by
    /// first-update `0x00463970`, so `pursue_enabled` stays false here and the GUI/UE
    /// host selects FollowFlyingPath vs PursueNavMesh from the creator XPath.
    pub const fn ef01_mine() -> Self {
        Self {
            handler_class: RobotsAiHandlerClass::Ef01Mine,
            patrol: ef01_mine_patrol_config(),
            periodic_idle_base_delay_ticks: ROBOTS_EF01_PERIODIC_IDLE_BASE_DELAY_TICKS,
            periodic_idle_anim_modes: &ROBOTS_EF01_PERIODIC_IDLE_ANIM_MODES,
            pursue_enabled: false,
            pursue_prelude_anim_mode: None,
            pursue_stop_distance: ROBOTS_PURSUE_NAV_NO_STOP_DISTANCE,
            first_update_uniform_scale: 1.0,
            stalk: None,
            attacks: [None; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
            common_hit: None,
            shunt_attack: None,
            spike_attack: None,
            turn_then_attacks: [None; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
            secondary_three_phase_attack: None,
            has_status_hit_nodes: true,
            uses_common_monster_attack_gate: false,
            electro_hit_anim_mode: ROBOTS_EF01_ELECTRO_HIT_ANIM_MODE,
        }
    }

    /// Monster_2Rockets builder `0x0045B700`. Both AttackGroup children are
    /// the already-shared TurnThenAttack family rather than plain AI_Attack nodes.
    pub const fn monster_2rockets() -> Self {
        Self {
            handler_class: RobotsAiHandlerClass::Monster2Rockets,
            patrol: RobotsAiPatrolConfig {
                base_yaw_radians: 0.0,
                interval_seconds: 5,
                target_locomotion_scalar: 0.0,
                turn_rate: RobotsAiTurnRateInput::Default,
            },
            periodic_idle_base_delay_ticks: 360,
            periodic_idle_anim_modes: &ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_ANIM_MODES,
            pursue_enabled: true,
            pursue_prelude_anim_mode: None,
            pursue_stop_distance: 5.0,
            first_update_uniform_scale: 1.0,
            stalk: Some(RobotsStalkNavMeshConfig::monster_2rockets()),
            attacks: [None; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
            common_hit: None,
            shunt_attack: None,
            spike_attack: None,
            turn_then_attacks: [
                Some(RobotsTurnThenAttackConfig::monster_2rockets_primary()),
                Some(RobotsTurnThenAttackConfig::monster_2rockets_secondary()),
                None,
            ],
            secondary_three_phase_attack: None,
            has_status_hit_nodes: true,
            uses_common_monster_attack_gate: true,
            electro_hit_anim_mode: ROBOTS_MALFBOT_ELECTRO_HIT_ANIM_MODE,
        }
    }

    pub const fn malfbot() -> Self {
        Self {
            handler_class: RobotsAiHandlerClass::MalfBot,
            patrol: RobotsAiPatrolConfig {
                base_yaw_radians: 0.0,
                interval_seconds: 5,
                target_locomotion_scalar: 0.0,
                turn_rate: RobotsAiTurnRateInput::Default,
            },
            periodic_idle_base_delay_ticks: ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_BASE_DELAY_TICKS,
            periodic_idle_anim_modes: &ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_ANIM_MODES,
            pursue_enabled: true,
            pursue_prelude_anim_mode: Some(ROBOTS_MALFBOT_PURSUE_NAV_PRELUDE_ANIM_MODE),
            pursue_stop_distance: ROBOTS_PURSUE_NAV_NO_STOP_DISTANCE,
            first_update_uniform_scale: 1.0,
            stalk: None,
            attacks: [
                Some(RobotsGenericAttackConfig::malfbot_attack1()),
                Some(RobotsGenericAttackConfig::malfbot_attack2()),
                None,
            ],
            common_hit: None,
            shunt_attack: None,
            spike_attack: None,
            turn_then_attacks: [None; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
            secondary_three_phase_attack: None,
            has_status_hit_nodes: true,
            uses_common_monster_attack_gate: false,
            electro_hit_anim_mode: ROBOTS_MALFBOT_ELECTRO_HIT_ANIM_MODE,
        }
    }

    /// EB04 ConstructionBot builder `0x0045D7F0`. The handler uses the common
    /// Monster vtable through the locomotion/action slots; only `+0x108` installs
    /// this data-driven node composition.
    pub const fn constructionbot() -> Self {
        Self {
            handler_class: RobotsAiHandlerClass::ConstructionBot,
            patrol: RobotsAiPatrolConfig {
                base_yaw_radians: 0.0,
                interval_seconds: 5,
                target_locomotion_scalar: 0.0,
                turn_rate: RobotsAiTurnRateInput::Default,
            },
            periodic_idle_base_delay_ticks: ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_BASE_DELAY_TICKS,
            periodic_idle_anim_modes: &ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_ANIM_MODES,
            pursue_enabled: true,
            pursue_prelude_anim_mode: None,
            pursue_stop_distance: 5.0,
            first_update_uniform_scale: 1.0,
            stalk: Some(RobotsStalkNavMeshConfig::eb14_minion()),
            attacks: [
                Some(
                    RobotsGenericAttackConfig::standard_monster_attack_with_reentry(
                        0x0900_0025,
                        1.5,
                        ROBOTS_EW10_EB14_ATTACK_YAW_TOLERANCE_RADIANS,
                        0,
                    ),
                ),
                Some(
                    RobotsGenericAttackConfig::standard_monster_attack_window_with_reentry(
                        0x0900_0027,
                        1.0,
                        3.0,
                        ROBOTS_EW10_EB14_ATTACK_YAW_TOLERANCE_RADIANS,
                        120,
                    ),
                ),
                None,
            ],
            common_hit: Some(RobotsCommonAiHitConfig::fast_front_back()),
            shunt_attack: None,
            spike_attack: None,
            turn_then_attacks: [None; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
            secondary_three_phase_attack: None,
            has_status_hit_nodes: true,
            uses_common_monster_attack_gate: true,
            electro_hit_anim_mode: ROBOTS_MALFBOT_ELECTRO_HIT_ANIM_MODE,
        }
    }

    /// Sweeper builder `0x00460380`. Unlike the ordinary standard-Monster
    /// family it has no `AI_Patrol` child: insertion begins at PeriodicIdle, then
    /// PursueNavMesh -> StalkNavMesh -> PatrolNavMesh2 -> CommonHit -> Fatal ->
    /// status hits -> AttackGroup(25,27). Fatal remains owned by the common fatal host.
    pub const fn sweeper() -> Self {
        Self {
            handler_class: RobotsAiHandlerClass::Sweeper,
            // The runtime state keeps a structurally valid Patrol reducer, but
            // `has_patrol_node()` is false and the selector never exposes it.
            patrol: RobotsAiPatrolConfig {
                base_yaw_radians: 0.0,
                interval_seconds: 5,
                target_locomotion_scalar: 0.0,
                turn_rate: RobotsAiTurnRateInput::Default,
            },
            periodic_idle_base_delay_ticks: ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_BASE_DELAY_TICKS,
            periodic_idle_anim_modes: &ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_ANIM_MODES,
            pursue_enabled: true,
            pursue_prelude_anim_mode: None,
            pursue_stop_distance: 1.5,
            first_update_uniform_scale: 1.0,
            stalk: Some(RobotsStalkNavMeshConfig::ew10_minion()),
            attacks: [
                Some(RobotsGenericAttackConfig::standard_monster_attack_with_reentry(
                    0x0900_0025,
                    2.0,
                    ROBOTS_EW10_EB14_ATTACK_YAW_TOLERANCE_RADIANS,
                    0,
                )),
                Some(RobotsGenericAttackConfig::standard_monster_attack_window_with_reentry(
                    0x0900_0027,
                    2.0,
                    4.0,
                    ROBOTS_EW10_EB14_ATTACK_YAW_TOLERANCE_RADIANS,
                    120,
                )),
                None,
            ],
            common_hit: Some(RobotsCommonAiHitConfig::fast_front_back()),
            shunt_attack: None,
            spike_attack: None,
            turn_then_attacks: [None; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
            secondary_three_phase_attack: None,
            has_status_hit_nodes: true,
            uses_common_monster_attack_gate: true,
            electro_hit_anim_mode: ROBOTS_MALFBOT_ELECTRO_HIT_ANIM_MODE,
        }
    }

    /// EW05 ThiefBot builder `0x0045F240`. The brain reuses only common
    /// behavior primitives, but its insertion order is class-specific and its
    /// post-common +0x34 override owns the separate blades attachment service.
    pub const fn thiefbot() -> Self {
        Self {
            handler_class: RobotsAiHandlerClass::ThiefBot,
            patrol: RobotsAiPatrolConfig {
                base_yaw_radians: 0.0,
                interval_seconds: 5,
                target_locomotion_scalar: 0.0,
                turn_rate: RobotsAiTurnRateInput::Default,
            },
            periodic_idle_base_delay_ticks: ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_BASE_DELAY_TICKS,
            periodic_idle_anim_modes: &ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_ANIM_MODES,
            pursue_enabled: true,
            pursue_prelude_anim_mode: None,
            pursue_stop_distance: 1.5,
            first_update_uniform_scale: 1.0,
            stalk: Some(RobotsStalkNavMeshConfig::ew10_minion()),
            attacks: [
                Some(
                    RobotsGenericAttackConfig::standard_monster_attack_with_reentry(
                        0x0900_0025,
                        2.0,
                        ROBOTS_EW10_EB14_ATTACK_YAW_TOLERANCE_RADIANS,
                        0,
                    ),
                ),
                Some(
                    RobotsGenericAttackConfig::standard_monster_attack_window_with_reentry(
                        0x0900_0027,
                        1.0,
                        3.0,
                        ROBOTS_EW10_EB14_ATTACK_YAW_TOLERANCE_RADIANS,
                        120,
                    ),
                ),
                None,
            ],
            common_hit: Some(RobotsCommonAiHitConfig::fast_front_back()),
            shunt_attack: None,
            spike_attack: None,
            turn_then_attacks: [None; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
            secondary_three_phase_attack: None,
            has_status_hit_nodes: true,
            uses_common_monster_attack_gate: true,
            electro_hit_anim_mode: ROBOTS_MALFBOT_ELECTRO_HIT_ANIM_MODE,
        }
    }

    /// EW04 SecurityBot builder `0x0045E7A0`. Like ConstructionBot, the class
    /// keeps the common Monster runtime vslots and differs only by data-driven
    /// behavior composition in `+0x108`.
    pub const fn securitybot() -> Self {
        Self {
            handler_class: RobotsAiHandlerClass::SecurityBot,
            patrol: RobotsAiPatrolConfig {
                base_yaw_radians: 0.0,
                interval_seconds: 5,
                target_locomotion_scalar: 0.0,
                turn_rate: RobotsAiTurnRateInput::Default,
            },
            periodic_idle_base_delay_ticks: ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_BASE_DELAY_TICKS,
            periodic_idle_anim_modes: &ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_ANIM_MODES,
            pursue_enabled: true,
            pursue_prelude_anim_mode: None,
            pursue_stop_distance: 5.0,
            first_update_uniform_scale: 1.0,
            stalk: Some(RobotsStalkNavMeshConfig::eb14_minion()),
            attacks: [
                Some(
                    RobotsGenericAttackConfig::standard_monster_attack_with_reentry(
                        0x0900_0025,
                        2.0,
                        f32::from_bits(0x3f06_0a92),
                        0,
                    ),
                ),
                Some(
                    RobotsGenericAttackConfig::standard_monster_attack_window_with_reentry(
                        0x0900_0027,
                        3.0,
                        4.5,
                        f32::from_bits(0x3f06_0a92),
                        120,
                    ),
                ),
                None,
            ],
            common_hit: Some(RobotsCommonAiHitConfig::fast_front_back()),
            shunt_attack: None,
            spike_attack: None,
            turn_then_attacks: [None; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
            secondary_three_phase_attack: None,
            has_status_hit_nodes: true,
            uses_common_monster_attack_gate: true,
            electro_hit_anim_mode: ROBOTS_MALFBOT_ELECTRO_HIT_ANIM_MODE,
        }
    }

    /// EW08 Flambe/FlambeLarge shared builder `0x00462760`. Both selectors
    /// use the same brain; Large differs only by first-update XItem scale x2.
    const fn flambe_with_class(
        handler_class: RobotsAiHandlerClass,
        first_update_uniform_scale: f32,
    ) -> Self {
        Self {
            handler_class,
            patrol: RobotsAiPatrolConfig {
                base_yaw_radians: 0.0,
                interval_seconds: 3,
                target_locomotion_scalar: 0.0,
                turn_rate: RobotsAiTurnRateInput::Default,
            },
            periodic_idle_base_delay_ticks: 0,
            periodic_idle_anim_modes: &[],
            pursue_enabled: true,
            pursue_prelude_anim_mode: None,
            pursue_stop_distance: 1.5,
            first_update_uniform_scale,
            stalk: None,
            attacks: [None; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
            common_hit: Some(RobotsCommonAiHitConfig::common_front_back()),
            shunt_attack: None,
            spike_attack: None,
            turn_then_attacks: [None; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
            secondary_three_phase_attack: None,
            has_status_hit_nodes: true,
            uses_common_monster_attack_gate: true,
            electro_hit_anim_mode: ROBOTS_MALFBOT_ELECTRO_HIT_ANIM_MODE,
        }
    }

    pub const fn flambe() -> Self {
        Self::flambe_with_class(RobotsAiHandlerClass::Ew08Flambe, 1.0)
    }

    pub const fn flambe_large() -> Self {
        Self::flambe_with_class(RobotsAiHandlerClass::Ew08FlambeLarge, 2.0)
    }

    /// EB05 GuardBot builder `0x0045DD50`. Its only class-unique behavior is the
    /// final `AI_CircleTarget`; every preceding node reuses common Monster reducers.
    pub const fn guardbot() -> Self {
        Self {
            handler_class: RobotsAiHandlerClass::GuardBot,
            patrol: RobotsAiPatrolConfig {
                base_yaw_radians: 0.0,
                interval_seconds: 5,
                target_locomotion_scalar: 0.0,
                turn_rate: RobotsAiTurnRateInput::Default,
            },
            periodic_idle_base_delay_ticks: 0,
            periodic_idle_anim_modes: &[],
            pursue_enabled: true,
            pursue_prelude_anim_mode: None,
            pursue_stop_distance: ROBOTS_PURSUE_NAV_NO_STOP_DISTANCE,
            first_update_uniform_scale: 1.0,
            stalk: Some(RobotsStalkNavMeshConfig::ew10_minion()),
            attacks: [None; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
            common_hit: Some(RobotsCommonAiHitConfig::fast_front_back()),
            shunt_attack: None,
            spike_attack: None,
            turn_then_attacks: [Some(RobotsTurnThenAttackConfig::guardbot()), None, None],
            secondary_three_phase_attack: None,
            has_status_hit_nodes: true,
            uses_common_monster_attack_gate: true,
            electro_hit_anim_mode: ROBOTS_MALFBOT_ELECTRO_HIT_ANIM_MODE,
        }
    }

    /// ShieldBot builder `0x0045E240`. All behavior node classes and retained
    /// HitCheck capacity use the common Monster contract.
    pub const fn shieldbot() -> Self {
        Self {
            handler_class: RobotsAiHandlerClass::ShieldBot,
            patrol: RobotsAiPatrolConfig {
                base_yaw_radians: 0.0,
                interval_seconds: 5,
                target_locomotion_scalar: 0.0,
                turn_rate: RobotsAiTurnRateInput::Default,
            },
            periodic_idle_base_delay_ticks: ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_BASE_DELAY_TICKS,
            periodic_idle_anim_modes: &ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_ANIM_MODES,
            pursue_enabled: true,
            pursue_prelude_anim_mode: None,
            pursue_stop_distance: 5.0,
            first_update_uniform_scale: 1.0,
            stalk: Some(RobotsStalkNavMeshConfig::eb14_minion()),
            attacks: [
                Some(
                    RobotsGenericAttackConfig::standard_monster_attack_with_reentry(
                        0x0900_0025,
                        2.0,
                        f32::from_bits(0x3db2_b8c2),
                        0,
                    ),
                ),
                None,
                None,
            ],
            common_hit: Some(RobotsCommonAiHitConfig::fast_front_back()),
            shunt_attack: None,
            spike_attack: None,
            turn_then_attacks: [None, Some(RobotsTurnThenAttackConfig::shieldbot()), None],
            secondary_three_phase_attack: None,
            has_status_hit_nodes: true,
            uses_common_monster_attack_gate: true,
            electro_hit_anim_mode: ROBOTS_MALFBOT_ELECTRO_HIT_ANIM_MODE,
        }
    }

    /// EB07 MineBot builder `0x0045ECF0`. Despite the historical name this is not
    /// EQ02 MineBot: it is a standard Monster composition with AI_FleeNavMesh and
    /// a two-child AttackGroup (ordinary Attack25 + reusable three-phase 3E/3F/40).
    pub const fn eb07_minebot() -> Self {
        Self {
            handler_class: RobotsAiHandlerClass::Eb07MineBot,
            patrol: RobotsAiPatrolConfig {
                base_yaw_radians: 0.0,
                interval_seconds: 5,
                target_locomotion_scalar: 0.0,
                turn_rate: RobotsAiTurnRateInput::Default,
            },
            periodic_idle_base_delay_ticks: ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_BASE_DELAY_TICKS,
            periodic_idle_anim_modes: &ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_ANIM_MODES,
            pursue_enabled: false,
            pursue_prelude_anim_mode: None,
            pursue_stop_distance: ROBOTS_PURSUE_NAV_NO_STOP_DISTANCE,
            first_update_uniform_scale: 1.0,
            stalk: None,
            attacks: [
                Some(RobotsGenericAttackConfig::eb07_minebot_primary()),
                None,
                None,
            ],
            common_hit: Some(RobotsCommonAiHitConfig::fast_front_back()),
            shunt_attack: None,
            spike_attack: None,
            turn_then_attacks: [None; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
            secondary_three_phase_attack: None,
            has_status_hit_nodes: true,
            uses_common_monster_attack_gate: true,
            electro_hit_anim_mode: ROBOTS_MALFBOT_ELECTRO_HIT_ANIM_MODE,
        }
    }

    /// EW09 Armoured builder `0x00463010`. The class has only Patrol plus one
    /// derived generic Attack inside AttackGroup. FollowNetworkPath is appended by
    /// first-update `0x00463230`, so it is modeled by the EW09-specific selector.
    pub const fn ew09_armoured() -> Self {
        Self {
            handler_class: RobotsAiHandlerClass::Ew09Armoured,
            patrol: RobotsAiPatrolConfig {
                base_yaw_radians: 0.0,
                interval_seconds: 6,
                target_locomotion_scalar: ROBOTS_EW09_PATROL_SCALAR,
                turn_rate: RobotsAiTurnRateInput::Explicit(std::f32::consts::PI),
            },
            periodic_idle_base_delay_ticks: 0,
            periodic_idle_anim_modes: &[],
            pursue_enabled: false,
            pursue_prelude_anim_mode: None,
            pursue_stop_distance: ROBOTS_PURSUE_NAV_NO_STOP_DISTANCE,
            first_update_uniform_scale: 1.0,
            stalk: None,
            attacks: [Some(ew09_attack_config()), None, None],
            common_hit: None,
            shunt_attack: None,
            spike_attack: None,
            turn_then_attacks: [None; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
            secondary_three_phase_attack: None,
            has_status_hit_nodes: false,
            uses_common_monster_attack_gate: true,
            electro_hit_anim_mode: 0,
        }
    }

    /// EB11 MagnaBot builder `0x00464810`. PeriodicIdle and Magnetic are absent
    /// from the native node list; an empty periodic mode slice keeps the common
    /// host from consuming RNG for a node that does not exist.
    pub const fn magnabot() -> Self {
        Self {
            handler_class: RobotsAiHandlerClass::Eb11MagnaBot,
            patrol: RobotsAiPatrolConfig {
                base_yaw_radians: 0.0,
                interval_seconds: 9,
                target_locomotion_scalar: f32::from_bits(0x3ecc_cccd),
                turn_rate: RobotsAiTurnRateInput::Explicit(std::f32::consts::PI),
            },
            periodic_idle_base_delay_ticks: 0,
            periodic_idle_anim_modes: &[],
            pursue_enabled: true,
            pursue_prelude_anim_mode: None,
            pursue_stop_distance: 5.0,
            first_update_uniform_scale: 1.0,
            stalk: Some(RobotsStalkNavMeshConfig::eb14_minion()),
            attacks: [
                Some(
                    RobotsGenericAttackConfig::standard_monster_attack_with_reentry(
                        0x0900_0025,
                        9.0,
                        f32::from_bits(0x3c9d_466e),
                        0,
                    ),
                ),
                None,
                None,
            ],
            common_hit: Some(RobotsCommonAiHitConfig::magnabot()),
            shunt_attack: None,
            spike_attack: None,
            turn_then_attacks: [None; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
            secondary_three_phase_attack: None,
            has_status_hit_nodes: true,
            uses_common_monster_attack_gate: true,
            electro_hit_anim_mode: ROBOTS_MALFBOT_ELECTRO_HIT_ANIM_MODE,
        }
    }

    /// EB12 EvilBot builder `0x00465260`. The builder has no PeriodicIdle node;
    /// its `0x00450CB0` allocation is the already-shared AI_AttackGroup container.
    pub const fn eb12_evilbot() -> Self {
        Self {
            handler_class: RobotsAiHandlerClass::Eb12EvilBot,
            patrol: RobotsAiPatrolConfig {
                base_yaw_radians: 0.0,
                interval_seconds: 3,
                target_locomotion_scalar: 0.0,
                turn_rate: RobotsAiTurnRateInput::Default,
            },
            periodic_idle_base_delay_ticks: 0,
            periodic_idle_anim_modes: &[],
            pursue_enabled: true,
            pursue_prelude_anim_mode: None,
            pursue_stop_distance: 5.0,
            first_update_uniform_scale: 1.0,
            stalk: Some(RobotsStalkNavMeshConfig::eb14_minion()),
            attacks: [
                Some(
                    RobotsGenericAttackConfig::standard_monster_attack_with_reentry(
                        0x0900_0025,
                        1.5,
                        ROBOTS_EW10_EB14_ATTACK_YAW_TOLERANCE_RADIANS,
                        0,
                    ),
                ),
                Some(
                    RobotsGenericAttackConfig::standard_monster_attack_window_with_reentry(
                        0x0900_0027,
                        f32::from_bits(0x3fa6_6666),
                        2.5,
                        ROBOTS_EW10_EB14_ATTACK_YAW_TOLERANCE_RADIANS,
                        0x78,
                    ),
                ),
                None,
            ],
            common_hit: Some(RobotsCommonAiHitConfig::evilbot()),
            shunt_attack: None,
            spike_attack: None,
            turn_then_attacks: [None; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
            secondary_three_phase_attack: None,
            has_status_hit_nodes: true,
            uses_common_monster_attack_gate: true,
            electro_hit_anim_mode: ROBOTS_MALFBOT_ELECTRO_HIT_ANIM_MODE,
        }
    }

    /// EB13 KnightBot builder `0x00465770`. The main behavior host owns Patrol,
    /// Pursue/Stalk/Nav, PeriodicIdle and an AttackGroup with children in exact
    /// builder order `[Attack25, TurnThenAttack27, Attack37]`. The reusable
    /// `0x004508A0` three-phase node lives in the independent secondary host
    /// `Handler+0x4D8` and is serviced by the GUI/runtime adapter separately.
    pub const fn eb13_knightbot() -> Self {
        Self {
            handler_class: RobotsAiHandlerClass::Eb13KnightBot,
            patrol: RobotsAiPatrolConfig {
                base_yaw_radians: 0.0,
                interval_seconds: 7,
                target_locomotion_scalar: 0.0,
                turn_rate: RobotsAiTurnRateInput::Default,
            },
            periodic_idle_base_delay_ticks: ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_BASE_DELAY_TICKS,
            periodic_idle_anim_modes: &ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_ANIM_MODES,
            pursue_enabled: true,
            pursue_prelude_anim_mode: None,
            pursue_stop_distance: 5.0,
            first_update_uniform_scale: 1.0,
            stalk: Some(RobotsStalkNavMeshConfig::eb14_minion()),
            attacks: [
                Some(
                    RobotsGenericAttackConfig::standard_monster_attack_with_reentry(
                        0x0900_0025,
                        2.0,
                        ROBOTS_EW10_EB14_ATTACK_YAW_TOLERANCE_RADIANS,
                        0,
                    ),
                ),
                None,
                Some(
                    RobotsGenericAttackConfig::standard_monster_attack_window_with_reentry(
                        0x0900_0037,
                        1.0,
                        3.0,
                        ROBOTS_EW10_EB14_ATTACK_YAW_TOLERANCE_RADIANS,
                        0x3c,
                    ),
                ),
            ],
            common_hit: Some(RobotsCommonAiHitConfig::evilbot()),
            shunt_attack: None,
            spike_attack: None,
            turn_then_attacks: [
                None,
                Some(RobotsTurnThenAttackConfig::eb13_knightbot()),
                None,
            ],
            secondary_three_phase_attack: Some(
                RobotsThreePhaseAttackConfig::eb13_knightbot_secondary(),
            ),
            has_status_hit_nodes: true,
            uses_common_monster_attack_gate: true,
            electro_hit_anim_mode: ROBOTS_MALFBOT_ELECTRO_HIT_ANIM_MODE,
        }
    }

    /// EB15 Launcher builder `0x00465FF0`. This remains a common Monster host;
    /// its AttackGroup children are `[generic Attack27, TurnThenAttack25]`.
    pub const fn eb15_launcher() -> Self {
        Self {
            handler_class: RobotsAiHandlerClass::Eb15Launcher,
            patrol: RobotsAiPatrolConfig {
                base_yaw_radians: 0.0,
                interval_seconds: 7,
                target_locomotion_scalar: 0.0,
                turn_rate: RobotsAiTurnRateInput::Default,
            },
            periodic_idle_base_delay_ticks: ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_BASE_DELAY_TICKS,
            periodic_idle_anim_modes: &ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_ANIM_MODES,
            pursue_enabled: true,
            pursue_prelude_anim_mode: None,
            pursue_stop_distance: ROBOTS_PURSUE_NAV_NO_STOP_DISTANCE,
            first_update_uniform_scale: 1.0,
            stalk: Some(RobotsStalkNavMeshConfig::eb15_launcher()),
            attacks: [
                Some(
                    RobotsGenericAttackConfig::standard_monster_attack_with_reentry(
                        0x0900_0027,
                        2.0,
                        ROBOTS_EW10_EB14_ATTACK_YAW_TOLERANCE_RADIANS,
                        0,
                    ),
                ),
                None,
                None,
            ],
            common_hit: Some(RobotsCommonAiHitConfig::evilbot()),
            shunt_attack: None,
            spike_attack: None,
            turn_then_attacks: [
                None,
                Some(RobotsTurnThenAttackConfig::eb15_launcher()),
                None,
            ],
            secondary_three_phase_attack: None,
            has_status_hit_nodes: true,
            uses_common_monster_attack_gate: true,
            electro_hit_anim_mode: ROBOTS_MALFBOT_ELECTRO_HIT_ANIM_MODE,
        }
    }

    /// EB16 KnuckleBot builder `0x00466560`. No class-specific behavior node is
    /// installed: the AttackGroup owns two generic attacks in native order 25,27.
    pub const fn eb16_knucklebot() -> Self {
        Self {
            handler_class: RobotsAiHandlerClass::Eb16KnuckleBot,
            patrol: RobotsAiPatrolConfig {
                base_yaw_radians: 0.0,
                interval_seconds: 7,
                target_locomotion_scalar: 0.0,
                turn_rate: RobotsAiTurnRateInput::Default,
            },
            periodic_idle_base_delay_ticks: ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_BASE_DELAY_TICKS,
            periodic_idle_anim_modes: &ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_ANIM_MODES,
            pursue_enabled: true,
            pursue_prelude_anim_mode: None,
            pursue_stop_distance: 5.0,
            first_update_uniform_scale: 1.0,
            stalk: Some(RobotsStalkNavMeshConfig::eb14_minion()),
            attacks: [
                Some(
                    RobotsGenericAttackConfig::standard_monster_attack_with_reentry(
                        0x0900_0025,
                        1.5,
                        ROBOTS_EW10_EB14_ATTACK_YAW_TOLERANCE_RADIANS,
                        120,
                    ),
                ),
                Some(
                    RobotsGenericAttackConfig::standard_monster_attack_with_reentry(
                        0x0900_0027,
                        2.5,
                        ROBOTS_EW10_EB14_ATTACK_YAW_TOLERANCE_RADIANS,
                        120,
                    ),
                ),
                None,
            ],
            common_hit: Some(RobotsCommonAiHitConfig::evilbot()),
            shunt_attack: None,
            spike_attack: None,
            turn_then_attacks: [None; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
            secondary_three_phase_attack: None,
            has_status_hit_nodes: true,
            uses_common_monster_attack_gate: true,
            electro_hit_anim_mode: ROBOTS_MALFBOT_ELECTRO_HIT_ANIM_MODE,
        }
    }

    /// EQ03 Spider builder `0x00464F70`. Native installs only Patrol,
    /// ScrambledHit, one-child AttackGroup, ElectroHit and MagneticHit in that
    /// exact order. There is no PeriodicIdle/Pursue/Stalk/CommonHit node.
    pub const fn eq03_spider() -> Self {
        Self {
            handler_class: RobotsAiHandlerClass::Eq03Spider,
            patrol: RobotsAiPatrolConfig {
                base_yaw_radians: 0.0,
                interval_seconds: 7,
                target_locomotion_scalar: 0.0,
                turn_rate: RobotsAiTurnRateInput::Default,
            },
            periodic_idle_base_delay_ticks: 0,
            periodic_idle_anim_modes: &[],
            pursue_enabled: false,
            pursue_prelude_anim_mode: None,
            pursue_stop_distance: ROBOTS_PURSUE_NAV_NO_STOP_DISTANCE,
            first_update_uniform_scale: 1.0,
            stalk: None,
            attacks: [None; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
            common_hit: None,
            shunt_attack: None,
            spike_attack: None,
            turn_then_attacks: [Some(RobotsTurnThenAttackConfig::eq03_spider()), None, None],
            secondary_three_phase_attack: None,
            has_status_hit_nodes: true,
            uses_common_monster_attack_gate: true,
            electro_hit_anim_mode: ROBOTS_MALFBOT_ELECTRO_HIT_ANIM_MODE,
        }
    }

    /// EW11 FatBot builder `0x00466A80`. The handler is a normal Monster subclass
    /// at vtable `0x005E6770`; all behavior nodes are shared common-family nodes.
    pub const fn ew11_fatbot() -> Self {
        Self {
            handler_class: RobotsAiHandlerClass::Ew11FatBot,
            patrol: RobotsAiPatrolConfig {
                base_yaw_radians: 0.0,
                interval_seconds: 7,
                target_locomotion_scalar: 0.0,
                turn_rate: RobotsAiTurnRateInput::Default,
            },
            periodic_idle_base_delay_ticks: ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_BASE_DELAY_TICKS,
            periodic_idle_anim_modes: &ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_ANIM_MODES,
            pursue_enabled: true,
            pursue_prelude_anim_mode: None,
            pursue_stop_distance: ROBOTS_PURSUE_NAV_NO_STOP_DISTANCE,
            first_update_uniform_scale: 1.0,
            stalk: Some(RobotsStalkNavMeshConfig::ew10_minion()),
            attacks: [
                Some(
                    RobotsGenericAttackConfig::standard_monster_attack_with_reentry(
                        0x0900_0025,
                        2.0,
                        ROBOTS_EW10_EB14_ATTACK_YAW_TOLERANCE_RADIANS,
                        60,
                    ),
                ),
                None,
                None,
            ],
            common_hit: Some(RobotsCommonAiHitConfig::common_front_back()),
            shunt_attack: None,
            spike_attack: None,
            turn_then_attacks: [None; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
            secondary_three_phase_attack: None,
            has_status_hit_nodes: true,
            uses_common_monster_attack_gate: true,
            electro_hit_anim_mode: ROBOTS_MALFBOT_ELECTRO_HIT_ANIM_MODE,
        }
    }

    /// EF03 EvilBot builder `0x00466FB0`. The behavior composition remains entirely
    /// common-family; the class-local `+0x100` override only adds the permanent-sound
    /// first-update effect exposed by `first_update_permanent_sound()`.
    pub const fn ef03_evilbot() -> Self {
        Self {
            handler_class: RobotsAiHandlerClass::Ef03EvilBot,
            patrol: RobotsAiPatrolConfig {
                base_yaw_radians: 0.0,
                interval_seconds: 7,
                target_locomotion_scalar: 0.0,
                turn_rate: RobotsAiTurnRateInput::Default,
            },
            periodic_idle_base_delay_ticks: ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_BASE_DELAY_TICKS,
            periodic_idle_anim_modes: &ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_ANIM_MODES,
            pursue_enabled: true,
            pursue_prelude_anim_mode: None,
            pursue_stop_distance: ROBOTS_PURSUE_NAV_NO_STOP_DISTANCE,
            first_update_uniform_scale: 1.0,
            stalk: Some(RobotsStalkNavMeshConfig::ew10_minion()),
            attacks: [
                Some(
                    RobotsGenericAttackConfig::standard_monster_attack_with_reentry(
                        0x0900_0025,
                        1.0,
                        ROBOTS_EW10_EB14_ATTACK_YAW_TOLERANCE_RADIANS,
                        0,
                    ),
                ),
                Some(
                    RobotsGenericAttackConfig::standard_monster_attack_window_with_reentry(
                        0x0900_0027,
                        1.0,
                        1.5,
                        ROBOTS_EW10_EB14_ATTACK_YAW_TOLERANCE_RADIANS,
                        60,
                    ),
                ),
                None,
            ],
            common_hit: Some(RobotsCommonAiHitConfig::common_front_back()),
            shunt_attack: None,
            spike_attack: None,
            turn_then_attacks: [None; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
            secondary_three_phase_attack: None,
            has_status_hit_nodes: true,
            uses_common_monster_attack_gate: true,
            electro_hit_anim_mode: ROBOTS_MALFBOT_ELECTRO_HIT_ANIM_MODE,
        }
    }

    pub const fn ew10_minion() -> Self {
        Self {
            handler_class: RobotsAiHandlerClass::Ew10Minion,
            patrol: RobotsAiPatrolConfig {
                base_yaw_radians: 0.0,
                interval_seconds: 3,
                target_locomotion_scalar: 0.0,
                turn_rate: RobotsAiTurnRateInput::Default,
            },
            periodic_idle_base_delay_ticks: ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_BASE_DELAY_TICKS,
            periodic_idle_anim_modes: &ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_ANIM_MODES,
            pursue_enabled: true,
            pursue_prelude_anim_mode: None,
            pursue_stop_distance: ROBOTS_PURSUE_NAV_NO_STOP_DISTANCE,
            first_update_uniform_scale: 1.0,
            stalk: Some(RobotsStalkNavMeshConfig::ew10_minion()),
            attacks: [
                Some(RobotsGenericAttackConfig::standard_monster_attack(
                    ROBOTS_EW10_EB14_ATTACK1_ANIM_MODE,
                    ROBOTS_EW10_ATTACK_OUTER_RADIUS,
                    ROBOTS_EW10_EB14_ATTACK_YAW_TOLERANCE_RADIANS,
                )),
                Some(RobotsGenericAttackConfig::standard_monster_attack(
                    ROBOTS_EW10_EB14_ATTACK2_ANIM_MODE,
                    ROBOTS_EW10_ATTACK_OUTER_RADIUS,
                    ROBOTS_EW10_EB14_ATTACK_YAW_TOLERANCE_RADIANS,
                )),
                None,
            ],
            common_hit: None,
            shunt_attack: None,
            spike_attack: None,
            turn_then_attacks: [None; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
            secondary_three_phase_attack: None,
            has_status_hit_nodes: true,
            uses_common_monster_attack_gate: true,
            electro_hit_anim_mode: ROBOTS_EW10_EB14_ELECTRO_HIT_ANIM_MODE,
        }
    }

    pub const fn eb14_minion() -> Self {
        Self {
            handler_class: RobotsAiHandlerClass::Eb14Minion,
            patrol: RobotsAiPatrolConfig {
                base_yaw_radians: 0.0,
                interval_seconds: 3,
                target_locomotion_scalar: 0.0,
                turn_rate: RobotsAiTurnRateInput::Default,
            },
            periodic_idle_base_delay_ticks: ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_BASE_DELAY_TICKS,
            periodic_idle_anim_modes: &ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_ANIM_MODES,
            pursue_enabled: true,
            pursue_prelude_anim_mode: None,
            pursue_stop_distance: 5.0,
            first_update_uniform_scale: 1.0,
            stalk: Some(RobotsStalkNavMeshConfig::eb14_minion()),
            attacks: [
                Some(RobotsGenericAttackConfig::standard_monster_attack(
                    ROBOTS_EW10_EB14_ATTACK1_ANIM_MODE,
                    ROBOTS_EB14_ATTACK_OUTER_RADIUS,
                    ROBOTS_EW10_EB14_ATTACK_YAW_TOLERANCE_RADIANS,
                )),
                Some(RobotsGenericAttackConfig::standard_monster_attack(
                    ROBOTS_EW10_EB14_ATTACK2_ANIM_MODE,
                    ROBOTS_EB14_ATTACK_OUTER_RADIUS,
                    ROBOTS_EW10_EB14_ATTACK_YAW_TOLERANCE_RADIANS,
                )),
                None,
            ],
            common_hit: None,
            shunt_attack: None,
            spike_attack: None,
            turn_then_attacks: [None; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
            secondary_three_phase_attack: None,
            has_status_hit_nodes: true,
            uses_common_monster_attack_gate: true,
            electro_hit_anim_mode: ROBOTS_EW10_EB14_ELECTRO_HIT_ANIM_MODE,
        }
    }

    pub const fn sawbot() -> Self {
        Self {
            handler_class: RobotsAiHandlerClass::SawBot,
            patrol: RobotsAiPatrolConfig {
                base_yaw_radians: 0.0,
                interval_seconds: 5,
                target_locomotion_scalar: 0.0,
                turn_rate: RobotsAiTurnRateInput::Default,
            },
            periodic_idle_base_delay_ticks: 120,
            periodic_idle_anim_modes: &ROBOTS_SAWBOT_PERIODIC_IDLE_ANIM_MODES,
            pursue_enabled: true,
            pursue_prelude_anim_mode: None,
            pursue_stop_distance: ROBOTS_PURSUE_NAV_NO_STOP_DISTANCE,
            first_update_uniform_scale: 1.0,
            stalk: Some(RobotsStalkNavMeshConfig::ew10_minion()),
            attacks: [
                Some(
                    RobotsGenericAttackConfig::standard_monster_attack_with_reentry(
                        0x0900_0025,
                        1.5,
                        ROBOTS_EW10_EB14_ATTACK_YAW_TOLERANCE_RADIANS,
                        0,
                    ),
                ),
                Some(
                    RobotsGenericAttackConfig::standard_monster_attack_with_reentry(
                        0x0900_0027,
                        1.5,
                        ROBOTS_EW10_EB14_ATTACK_YAW_TOLERANCE_RADIANS,
                        0,
                    ),
                ),
                Some(
                    RobotsGenericAttackConfig::standard_monster_attack_with_reentry(
                        0x0900_0037,
                        2.0,
                        ROBOTS_EW10_EB14_ATTACK_YAW_TOLERANCE_RADIANS,
                        60,
                    ),
                ),
            ],
            common_hit: Some(RobotsCommonAiHitConfig::sawbot()),
            shunt_attack: None,
            spike_attack: None,
            turn_then_attacks: [None; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
            secondary_three_phase_attack: None,
            has_status_hit_nodes: true,
            uses_common_monster_attack_gate: true,
            electro_hit_anim_mode: ROBOTS_MALFBOT_ELECTRO_HIT_ANIM_MODE,
        }
    }

    pub const fn spikebot() -> Self {
        Self {
            handler_class: RobotsAiHandlerClass::SpikeBot,
            patrol: RobotsAiPatrolConfig {
                base_yaw_radians: 0.0,
                interval_seconds: 5,
                target_locomotion_scalar: 0.0,
                turn_rate: RobotsAiTurnRateInput::Default,
            },
            periodic_idle_base_delay_ticks: 120,
            periodic_idle_anim_modes: &ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_ANIM_MODES,
            pursue_enabled: false,
            pursue_prelude_anim_mode: None,
            pursue_stop_distance: ROBOTS_PURSUE_NAV_NO_STOP_DISTANCE,
            first_update_uniform_scale: 1.0,
            stalk: None,
            attacks: [None; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
            common_hit: Some(RobotsCommonAiHitConfig::spikebot()),
            shunt_attack: None,
            spike_attack: Some(RobotsSpikeAttackConfig::spikebot()),
            turn_then_attacks: [None; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
            secondary_three_phase_attack: None,
            has_status_hit_nodes: true,
            uses_common_monster_attack_gate: false,
            electro_hit_anim_mode: ROBOTS_MALFBOT_ELECTRO_HIT_ANIM_MODE,
        }
    }

    pub const fn jailbot_normal() -> Self {
        Self {
            handler_class: RobotsAiHandlerClass::JailBotNormal,
            patrol: RobotsAiPatrolConfig {
                base_yaw_radians: 0.0,
                interval_seconds: 5,
                target_locomotion_scalar: 0.0,
                turn_rate: RobotsAiTurnRateInput::Default,
            },
            periodic_idle_base_delay_ticks: ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_BASE_DELAY_TICKS,
            periodic_idle_anim_modes: &ROBOTS_JAILBOT_PERIODIC_IDLE_ANIM_MODES,
            pursue_enabled: true,
            pursue_prelude_anim_mode: None,
            pursue_stop_distance: 5.0,
            first_update_uniform_scale: 1.0,
            stalk: Some(RobotsStalkNavMeshConfig::eb14_minion()),
            attacks: [
                Some(
                    RobotsGenericAttackConfig::standard_monster_attack_window_with_reentry(
                        0x0900_0025,
                        0.0,
                        2.0,
                        ROBOTS_JAILBOT_ATTACK1_YAW_TOLERANCE_RADIANS,
                        0,
                    ),
                ),
                Some(
                    RobotsGenericAttackConfig::standard_monster_attack_window_with_reentry(
                        0x0900_0027,
                        2.0,
                        4.0,
                        ROBOTS_JAILBOT_ATTACK2_YAW_TOLERANCE_RADIANS,
                        30,
                    ),
                ),
                None,
            ],
            common_hit: Some(RobotsCommonAiHitConfig::fast_front_back()),
            shunt_attack: None,
            spike_attack: None,
            turn_then_attacks: [None; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
            secondary_three_phase_attack: None,
            has_status_hit_nodes: true,
            uses_common_monster_attack_gate: true,
            electro_hit_anim_mode: ROBOTS_MALFBOT_ELECTRO_HIT_ANIM_MODE,
        }
    }

    pub const fn jailbot_large() -> Self {
        Self {
            handler_class: RobotsAiHandlerClass::JailBotLarge,
            patrol: RobotsAiPatrolConfig {
                base_yaw_radians: 0.0,
                interval_seconds: 5,
                target_locomotion_scalar: 0.0,
                turn_rate: RobotsAiTurnRateInput::Default,
            },
            periodic_idle_base_delay_ticks: ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_BASE_DELAY_TICKS,
            periodic_idle_anim_modes: &ROBOTS_JAILBOT_PERIODIC_IDLE_ANIM_MODES,
            pursue_enabled: true,
            pursue_prelude_anim_mode: None,
            pursue_stop_distance: 5.0,
            first_update_uniform_scale: 2.0,
            stalk: Some(RobotsStalkNavMeshConfig::eb14_minion()),
            attacks: [
                Some(
                    RobotsGenericAttackConfig::standard_monster_attack_window_with_reentry(
                        0x0900_0025,
                        0.0,
                        3.0,
                        ROBOTS_JAILBOT_ATTACK1_YAW_TOLERANCE_RADIANS,
                        0,
                    ),
                ),
                None,
                None,
            ],
            common_hit: Some(RobotsCommonAiHitConfig::fast_front_back()),
            shunt_attack: None,
            spike_attack: None,
            turn_then_attacks: [None; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
            secondary_three_phase_attack: None,
            has_status_hit_nodes: true,
            uses_common_monster_attack_gate: true,
            electro_hit_anim_mode: ROBOTS_MALFBOT_ELECTRO_HIT_ANIM_MODE,
        }
    }

    pub const fn shuntbot() -> Self {
        Self {
            handler_class: RobotsAiHandlerClass::ShuntBot,
            patrol: RobotsAiPatrolConfig {
                base_yaw_radians: 0.0,
                interval_seconds: 5,
                target_locomotion_scalar: 0.0,
                turn_rate: RobotsAiTurnRateInput::Default,
            },
            periodic_idle_base_delay_ticks: 120,
            periodic_idle_anim_modes: &ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_ANIM_MODES,
            pursue_enabled: true,
            pursue_prelude_anim_mode: None,
            pursue_stop_distance: ROBOTS_PURSUE_NAV_NO_STOP_DISTANCE,
            first_update_uniform_scale: 1.0,
            stalk: Some(RobotsStalkNavMeshConfig::shuntbot()),
            attacks: [None; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
            common_hit: Some(RobotsCommonAiHitConfig::shuntbot()),
            shunt_attack: Some(RobotsShuntAttackConfig::normal()),
            spike_attack: None,
            turn_then_attacks: [None; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
            secondary_three_phase_attack: None,
            has_status_hit_nodes: true,
            uses_common_monster_attack_gate: true,
            electro_hit_anim_mode: ROBOTS_MALFBOT_ELECTRO_HIT_ANIM_MODE,
        }
    }

    pub const fn shuntbot_boss() -> Self {
        Self {
            handler_class: RobotsAiHandlerClass::ShuntBotBoss,
            patrol: RobotsAiPatrolConfig {
                base_yaw_radians: 0.0,
                interval_seconds: 5,
                target_locomotion_scalar: 0.0,
                turn_rate: RobotsAiTurnRateInput::Default,
            },
            periodic_idle_base_delay_ticks: 120,
            periodic_idle_anim_modes: &ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_ANIM_MODES,
            pursue_enabled: true,
            pursue_prelude_anim_mode: None,
            pursue_stop_distance: ROBOTS_PURSUE_NAV_NO_STOP_DISTANCE,
            first_update_uniform_scale: 1.0,
            stalk: Some(RobotsStalkNavMeshConfig::ew10_minion()),
            attacks: [None; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
            common_hit: None,
            shunt_attack: Some(RobotsShuntAttackConfig::boss()),
            spike_attack: None,
            turn_then_attacks: [None; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
            secondary_three_phase_attack: None,
            has_status_hit_nodes: false,
            uses_common_monster_attack_gate: true,
            electro_hit_anim_mode: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RobotsStandardMonsterBehaviorWinner {
    Patrol,
    FollowFlyingPath,
    FollowNetworkPath,
    ProximityIdleAttack,
    TargetFacingTurn,
    PeriodicIdle,
    PatrolNavMesh2,
    PursueNavMesh,
    StalkNavMesh,
    FleeNavMesh,
    CommonHit,
    ScrambledHit,
    ElectroHit,
    MagneticHit,
    DirectTurnThenAttack,
    AttackGroup,
    ShuntAttack,
    SpikeAttack,
    CircleTarget,
}

/// Native `0x00457140` strict-greater-than selector in builder insertion order.
/// Keeping the order here matters when two nodes return the same priority.
#[allow(clippy::too_many_arguments)]
pub fn standard_monster_behavior_winner(
    attack_group_priority: u8,
    shunt_attack_priority: u8,
    spike_attack_priority: u8,
    pursue_ready: bool,
    stalk_priority: u8,
    periodic_idle_priority: u8,
    patrol_navmesh2_priority: u8,
    patrol_priority: u8,
    common_hit_priority: u8,
    scrambled_hit_priority: u8,
    electro_hit_priority: u8,
    magnetic_hit_priority: u8,
) -> Option<RobotsStandardMonsterBehaviorWinner> {
    let candidates = [
        (patrol_priority, RobotsStandardMonsterBehaviorWinner::Patrol),
        (
            periodic_idle_priority,
            RobotsStandardMonsterBehaviorWinner::PeriodicIdle,
        ),
        (
            patrol_navmesh2_priority,
            RobotsStandardMonsterBehaviorWinner::PatrolNavMesh2,
        ),
        (
            if pursue_ready { 0x1f } else { 1 },
            RobotsStandardMonsterBehaviorWinner::PursueNavMesh,
        ),
        (
            stalk_priority,
            RobotsStandardMonsterBehaviorWinner::StalkNavMesh,
        ),
        (
            common_hit_priority,
            RobotsStandardMonsterBehaviorWinner::CommonHit,
        ),
        (
            scrambled_hit_priority,
            RobotsStandardMonsterBehaviorWinner::ScrambledHit,
        ),
        (
            electro_hit_priority,
            RobotsStandardMonsterBehaviorWinner::ElectroHit,
        ),
        (
            magnetic_hit_priority,
            RobotsStandardMonsterBehaviorWinner::MagneticHit,
        ),
        (
            attack_group_priority,
            RobotsStandardMonsterBehaviorWinner::AttackGroup,
        ),
        (
            shunt_attack_priority,
            RobotsStandardMonsterBehaviorWinner::ShuntAttack,
        ),
        (
            spike_attack_priority,
            RobotsStandardMonsterBehaviorWinner::SpikeAttack,
        ),
    ];
    let mut best_priority = 1u8;
    let mut selected = None;
    for (priority, winner) in candidates {
        if best_priority < priority {
            best_priority = priority;
            selected = Some(winner);
        }
    }
    selected
}

/// Sweeper builder `0x00460380` insertion order. There is deliberately no
/// ordinary Patrol node. Fatal is serviced by the common fatal host before this
/// selector, so the remaining order is PeriodicIdle -> Pursue -> Stalk ->
/// PatrolNavMesh2 -> CommonHit -> Scrambled -> Electro -> Magnetic -> AttackGroup.
#[allow(clippy::too_many_arguments)]
pub fn sweeper_behavior_winner(
    periodic_idle_priority: u8,
    pursue_ready: bool,
    stalk_priority: u8,
    patrol_navmesh2_priority: u8,
    common_hit_priority: u8,
    scrambled_hit_priority: u8,
    electro_hit_priority: u8,
    magnetic_hit_priority: u8,
    attack_group_priority: u8,
) -> Option<RobotsStandardMonsterBehaviorWinner> {
    let candidates = [
        (periodic_idle_priority, RobotsStandardMonsterBehaviorWinner::PeriodicIdle),
        (if pursue_ready { 0x1f } else { 1 }, RobotsStandardMonsterBehaviorWinner::PursueNavMesh),
        (stalk_priority, RobotsStandardMonsterBehaviorWinner::StalkNavMesh),
        (patrol_navmesh2_priority, RobotsStandardMonsterBehaviorWinner::PatrolNavMesh2),
        (common_hit_priority, RobotsStandardMonsterBehaviorWinner::CommonHit),
        (scrambled_hit_priority, RobotsStandardMonsterBehaviorWinner::ScrambledHit),
        (electro_hit_priority, RobotsStandardMonsterBehaviorWinner::ElectroHit),
        (magnetic_hit_priority, RobotsStandardMonsterBehaviorWinner::MagneticHit),
        (attack_group_priority, RobotsStandardMonsterBehaviorWinner::AttackGroup),
    ];
    let mut best_priority = 1u8;
    let mut selected = None;
    for (priority, winner) in candidates {
        if best_priority < priority {
            best_priority = priority;
            selected = Some(winner);
        }
    }
    selected
}

/// EW05 ThiefBot builder `0x0045F240` insertion order. Fatal is serviced
/// by the common fatal host before this selector. StalkNavMesh is appended last,
/// after AttackGroup and its two children, so keep this separate from the generic
/// standard-Monster ordering.
#[allow(clippy::too_many_arguments)]
pub fn thiefbot_behavior_winner(
    patrol_priority: u8,
    pursue_ready: bool,
    patrol_navmesh2_priority: u8,
    periodic_idle_priority: u8,
    common_hit_priority: u8,
    scrambled_hit_priority: u8,
    electro_hit_priority: u8,
    magnetic_hit_priority: u8,
    attack_group_priority: u8,
    stalk_priority: u8,
) -> Option<RobotsStandardMonsterBehaviorWinner> {
    let candidates = [
        (patrol_priority, RobotsStandardMonsterBehaviorWinner::Patrol),
        (
            if pursue_ready { 0x1f } else { 1 },
            RobotsStandardMonsterBehaviorWinner::PursueNavMesh,
        ),
        (
            patrol_navmesh2_priority,
            RobotsStandardMonsterBehaviorWinner::PatrolNavMesh2,
        ),
        (
            periodic_idle_priority,
            RobotsStandardMonsterBehaviorWinner::PeriodicIdle,
        ),
        (
            common_hit_priority,
            RobotsStandardMonsterBehaviorWinner::CommonHit,
        ),
        (
            scrambled_hit_priority,
            RobotsStandardMonsterBehaviorWinner::ScrambledHit,
        ),
        (
            electro_hit_priority,
            RobotsStandardMonsterBehaviorWinner::ElectroHit,
        ),
        (
            magnetic_hit_priority,
            RobotsStandardMonsterBehaviorWinner::MagneticHit,
        ),
        (
            attack_group_priority,
            RobotsStandardMonsterBehaviorWinner::AttackGroup,
        ),
        (
            stalk_priority,
            RobotsStandardMonsterBehaviorWinner::StalkNavMesh,
        ),
    ];
    let mut best_priority = 1u8;
    let mut selected = None;
    for (priority, winner) in candidates {
        if best_priority < priority {
            best_priority = priority;
            selected = Some(winner);
        }
    }
    selected
}

/// EQ03 Spider builder `0x00464F70` insertion order. The class does not install
/// PeriodicIdle/Pursue/Stalk/CommonHit/Fatal nodes, so keep its compact selector
/// separate instead of feeding nonexistent nodes through the generic ordering.
pub fn eq03_spider_behavior_winner(
    patrol_priority: u8,
    scrambled_hit_priority: u8,
    attack_group_priority: u8,
    electro_hit_priority: u8,
    magnetic_hit_priority: u8,
) -> Option<RobotsStandardMonsterBehaviorWinner> {
    let candidates = [
        (patrol_priority, RobotsStandardMonsterBehaviorWinner::Patrol),
        (
            scrambled_hit_priority,
            RobotsStandardMonsterBehaviorWinner::ScrambledHit,
        ),
        (
            attack_group_priority,
            RobotsStandardMonsterBehaviorWinner::AttackGroup,
        ),
        (
            electro_hit_priority,
            RobotsStandardMonsterBehaviorWinner::ElectroHit,
        ),
        (
            magnetic_hit_priority,
            RobotsStandardMonsterBehaviorWinner::MagneticHit,
        ),
    ];
    let mut best_priority = 1u8;
    let mut selected = None;
    for (priority, winner) in candidates {
        if best_priority < priority {
            best_priority = priority;
            selected = Some(winner);
        }
    }
    selected
}

/// EB07 MineBot native builder order from `0x0045ECF0`. Fatal is serviced by the
/// common fatal host before this selector. Flee is inserted after status-hit nodes
/// and before AttackGroup; strict `>` keeps original insertion order on ties.
#[allow(clippy::too_many_arguments)]
pub fn eb07_minebot_behavior_winner(
    patrol_priority: u8,
    patrol_navmesh2_priority: u8,
    periodic_idle_priority: u8,
    common_hit_priority: u8,
    scrambled_hit_priority: u8,
    electro_hit_priority: u8,
    magnetic_hit_priority: u8,
    flee_navmesh_priority: u8,
    attack_group_priority: u8,
) -> Option<RobotsStandardMonsterBehaviorWinner> {
    let candidates = [
        (patrol_priority, RobotsStandardMonsterBehaviorWinner::Patrol),
        (
            patrol_navmesh2_priority,
            RobotsStandardMonsterBehaviorWinner::PatrolNavMesh2,
        ),
        (
            periodic_idle_priority,
            RobotsStandardMonsterBehaviorWinner::PeriodicIdle,
        ),
        (
            common_hit_priority,
            RobotsStandardMonsterBehaviorWinner::CommonHit,
        ),
        (
            scrambled_hit_priority,
            RobotsStandardMonsterBehaviorWinner::ScrambledHit,
        ),
        (
            electro_hit_priority,
            RobotsStandardMonsterBehaviorWinner::ElectroHit,
        ),
        (
            magnetic_hit_priority,
            RobotsStandardMonsterBehaviorWinner::MagneticHit,
        ),
        (
            flee_navmesh_priority,
            RobotsStandardMonsterBehaviorWinner::FleeNavMesh,
        ),
        (
            attack_group_priority,
            RobotsStandardMonsterBehaviorWinner::AttackGroup,
        ),
    ];
    let mut best_priority = 1u8;
    let mut selected = None;
    for (priority, winner) in candidates {
        if best_priority < priority {
            best_priority = priority;
            selected = Some(winner);
        }
    }
    selected
}

/// EB05 GuardBot native builder order from `0x0045DD50`. `AI_CircleTarget` is
/// appended after AttackGroup. The common selector uses strict `>`, so Stalk and
/// CircleTarget ties at priority 0x28 are intentionally won by Stalk.
#[allow(clippy::too_many_arguments)]
pub fn guardbot_behavior_winner(
    patrol_priority: u8,
    pursue_ready: bool,
    stalk_priority: u8,
    patrol_navmesh2_priority: u8,
    common_hit_priority: u8,
    scrambled_hit_priority: u8,
    electro_hit_priority: u8,
    magnetic_hit_priority: u8,
    attack_group_priority: u8,
    circle_target_priority: u8,
) -> Option<RobotsStandardMonsterBehaviorWinner> {
    let candidates = [
        (patrol_priority, RobotsStandardMonsterBehaviorWinner::Patrol),
        (
            if pursue_ready { 0x1f } else { 1 },
            RobotsStandardMonsterBehaviorWinner::PursueNavMesh,
        ),
        (
            stalk_priority,
            RobotsStandardMonsterBehaviorWinner::StalkNavMesh,
        ),
        (
            patrol_navmesh2_priority,
            RobotsStandardMonsterBehaviorWinner::PatrolNavMesh2,
        ),
        (
            common_hit_priority,
            RobotsStandardMonsterBehaviorWinner::CommonHit,
        ),
        (
            scrambled_hit_priority,
            RobotsStandardMonsterBehaviorWinner::ScrambledHit,
        ),
        (
            electro_hit_priority,
            RobotsStandardMonsterBehaviorWinner::ElectroHit,
        ),
        (
            magnetic_hit_priority,
            RobotsStandardMonsterBehaviorWinner::MagneticHit,
        ),
        (
            attack_group_priority,
            RobotsStandardMonsterBehaviorWinner::AttackGroup,
        ),
        (
            circle_target_priority,
            RobotsStandardMonsterBehaviorWinner::CircleTarget,
        ),
    ];
    let mut best_priority = 1u8;
    let mut selected = None;
    for (priority, winner) in candidates {
        if best_priority < priority {
            best_priority = priority;
            selected = Some(winner);
        }
    }
    selected
}

/// EW08 Flambe/FlambeLarge shared builder order from `0x00462760`.
/// Fatal is serviced by the common fatal host before this selector. The direct
/// TurnThenAttack is intentionally distinct from AttackGroup because native inserts
/// it straight into Handler+0x4C4 and therefore consumes no AttackGroup RNG.
#[allow(clippy::too_many_arguments)]
pub fn flambe_behavior_winner(
    patrol_priority: u8,
    pursue_ready: bool,
    patrol_navmesh2_priority: u8,
    direct_turn_attack_priority: u8,
    circle_target_priority: u8,
    common_hit_priority: u8,
    scrambled_hit_priority: u8,
    electro_hit_priority: u8,
    magnetic_hit_priority: u8,
) -> Option<RobotsStandardMonsterBehaviorWinner> {
    let candidates = [
        (patrol_priority, RobotsStandardMonsterBehaviorWinner::Patrol),
        (
            if pursue_ready { 0x1f } else { 1 },
            RobotsStandardMonsterBehaviorWinner::PursueNavMesh,
        ),
        (
            patrol_navmesh2_priority,
            RobotsStandardMonsterBehaviorWinner::PatrolNavMesh2,
        ),
        (
            direct_turn_attack_priority,
            RobotsStandardMonsterBehaviorWinner::DirectTurnThenAttack,
        ),
        (
            circle_target_priority,
            RobotsStandardMonsterBehaviorWinner::CircleTarget,
        ),
        (
            common_hit_priority,
            RobotsStandardMonsterBehaviorWinner::CommonHit,
        ),
        (
            scrambled_hit_priority,
            RobotsStandardMonsterBehaviorWinner::ScrambledHit,
        ),
        (
            electro_hit_priority,
            RobotsStandardMonsterBehaviorWinner::ElectroHit,
        ),
        (
            magnetic_hit_priority,
            RobotsStandardMonsterBehaviorWinner::MagneticHit,
        ),
    ];
    let mut best_priority = 1u8;
    let mut selected = None;
    for (priority, winner) in candidates {
        if best_priority < priority {
            best_priority = priority;
            selected = Some(winner);
        }
    }
    selected
}

/// ShieldBot native builder order from `0x0045E240`. Fatal is serviced by the
/// common fatal host before this selector, matching the other standard monsters.
#[allow(clippy::too_many_arguments)]
pub fn shieldbot_behavior_winner(
    patrol_priority: u8,
    pursue_ready: bool,
    stalk_priority: u8,
    patrol_navmesh2_priority: u8,
    periodic_idle_priority: u8,
    common_hit_priority: u8,
    scrambled_hit_priority: u8,
    electro_hit_priority: u8,
    magnetic_hit_priority: u8,
    attack_group_priority: u8,
) -> Option<RobotsStandardMonsterBehaviorWinner> {
    let candidates = [
        (patrol_priority, RobotsStandardMonsterBehaviorWinner::Patrol),
        (
            if pursue_ready { 0x1f } else { 1 },
            RobotsStandardMonsterBehaviorWinner::PursueNavMesh,
        ),
        (
            stalk_priority,
            RobotsStandardMonsterBehaviorWinner::StalkNavMesh,
        ),
        (
            patrol_navmesh2_priority,
            RobotsStandardMonsterBehaviorWinner::PatrolNavMesh2,
        ),
        (
            periodic_idle_priority,
            RobotsStandardMonsterBehaviorWinner::PeriodicIdle,
        ),
        (
            common_hit_priority,
            RobotsStandardMonsterBehaviorWinner::CommonHit,
        ),
        (
            scrambled_hit_priority,
            RobotsStandardMonsterBehaviorWinner::ScrambledHit,
        ),
        (
            electro_hit_priority,
            RobotsStandardMonsterBehaviorWinner::ElectroHit,
        ),
        (
            magnetic_hit_priority,
            RobotsStandardMonsterBehaviorWinner::MagneticHit,
        ),
        (
            attack_group_priority,
            RobotsStandardMonsterBehaviorWinner::AttackGroup,
        ),
    ];
    let mut best_priority = 1u8;
    let mut selected = None;
    for (priority, winner) in candidates {
        if best_priority < priority {
            best_priority = priority;
            selected = Some(winner);
        }
    }
    selected
}

/// EW09 Armoured native order: builder `0x00463010` installs Patrol then
/// AttackGroup, and first-update `0x00463230` appends FollowNetworkPath when the
/// creator exposes a valid 0x0B path UID. Priorities are distinct in shipped data,
/// but preserve insertion order anyway because the common selector uses strict `>`.
pub fn ew09_armoured_behavior_winner(
    patrol_priority: u8,
    attack_group_priority: u8,
    follow_network_path_priority: u8,
) -> Option<RobotsStandardMonsterBehaviorWinner> {
    let candidates = [
        (patrol_priority, RobotsStandardMonsterBehaviorWinner::Patrol),
        (
            attack_group_priority,
            RobotsStandardMonsterBehaviorWinner::AttackGroup,
        ),
        (
            follow_network_path_priority,
            RobotsStandardMonsterBehaviorWinner::FollowNetworkPath,
        ),
    ];
    let mut best_priority = 1u8;
    let mut selected = None;
    for (priority, winner) in candidates {
        if best_priority < priority {
            best_priority = priority;
            selected = Some(winner);
        }
    }
    selected
}

/// EB11 MagnaBot native builder order from `0x00464810`. The common selector
/// uses strict `>` and therefore preserves the first node on equal priority.
#[allow(clippy::too_many_arguments)]
pub fn magnabot_behavior_winner(
    patrol_priority: u8,
    proximity_idle_attack_priority: u8,
    target_facing_turn_priority: u8,
    pursue_ready: bool,
    stalk_priority: u8,
    patrol_navmesh2_priority: u8,
    attack_group_priority: u8,
    common_hit_priority: u8,
    scrambled_hit_priority: u8,
    electro_hit_priority: u8,
) -> Option<RobotsStandardMonsterBehaviorWinner> {
    let candidates = [
        (patrol_priority, RobotsStandardMonsterBehaviorWinner::Patrol),
        (
            proximity_idle_attack_priority,
            RobotsStandardMonsterBehaviorWinner::ProximityIdleAttack,
        ),
        (
            target_facing_turn_priority,
            RobotsStandardMonsterBehaviorWinner::TargetFacingTurn,
        ),
        (
            if pursue_ready { 0x1f } else { 1 },
            RobotsStandardMonsterBehaviorWinner::PursueNavMesh,
        ),
        (
            stalk_priority,
            RobotsStandardMonsterBehaviorWinner::StalkNavMesh,
        ),
        (
            patrol_navmesh2_priority,
            RobotsStandardMonsterBehaviorWinner::PatrolNavMesh2,
        ),
        (
            attack_group_priority,
            RobotsStandardMonsterBehaviorWinner::AttackGroup,
        ),
        (
            common_hit_priority,
            RobotsStandardMonsterBehaviorWinner::CommonHit,
        ),
        (
            scrambled_hit_priority,
            RobotsStandardMonsterBehaviorWinner::ScrambledHit,
        ),
        (
            electro_hit_priority,
            RobotsStandardMonsterBehaviorWinner::ElectroHit,
        ),
    ];
    let mut best_priority = 1u8;
    let mut selected = None;
    for (priority, winner) in candidates {
        if best_priority < priority {
            best_priority = priority;
            selected = Some(winner);
        }
    }
    selected
}

/// EB13 KnightBot native builder order from `0x00465770`. `AI_HitFatal` is
/// serviced by the common fatal host before this selector, so it is intentionally
/// absent here even though the builder installs the node between CommonHit and
/// ScrambledHit.
#[allow(clippy::too_many_arguments)]
pub fn knightbot_behavior_winner(
    patrol_priority: u8,
    pursue_ready: bool,
    stalk_priority: u8,
    patrol_navmesh2_priority: u8,
    periodic_idle_priority: u8,
    attack_group_priority: u8,
    common_hit_priority: u8,
    scrambled_hit_priority: u8,
    electro_hit_priority: u8,
    magnetic_hit_priority: u8,
) -> Option<RobotsStandardMonsterBehaviorWinner> {
    let candidates = [
        (patrol_priority, RobotsStandardMonsterBehaviorWinner::Patrol),
        (
            if pursue_ready { 0x1f } else { 1 },
            RobotsStandardMonsterBehaviorWinner::PursueNavMesh,
        ),
        (
            stalk_priority,
            RobotsStandardMonsterBehaviorWinner::StalkNavMesh,
        ),
        (
            patrol_navmesh2_priority,
            RobotsStandardMonsterBehaviorWinner::PatrolNavMesh2,
        ),
        (
            periodic_idle_priority,
            RobotsStandardMonsterBehaviorWinner::PeriodicIdle,
        ),
        (
            attack_group_priority,
            RobotsStandardMonsterBehaviorWinner::AttackGroup,
        ),
        (
            common_hit_priority,
            RobotsStandardMonsterBehaviorWinner::CommonHit,
        ),
        (
            scrambled_hit_priority,
            RobotsStandardMonsterBehaviorWinner::ScrambledHit,
        ),
        (
            electro_hit_priority,
            RobotsStandardMonsterBehaviorWinner::ElectroHit,
        ),
        (
            magnetic_hit_priority,
            RobotsStandardMonsterBehaviorWinner::MagneticHit,
        ),
    ];
    let mut best_priority = 1u8;
    let mut selected = None;
    for (priority, winner) in candidates {
        if best_priority < priority {
            best_priority = priority;
            selected = Some(winner);
        }
    }
    selected
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::robots_runtime::stalk_navmesh::ROBOTS_STALK_NAVMESH_PRIORITY;

    #[test]
    fn monster_2rockets_config_matches_native_builder_and_dual_turn_attack_order() {
        let config = RobotsStandardMonsterBehaviorConfig::monster_2rockets();
        assert_eq!(config.handler_class, RobotsAiHandlerClass::Monster2Rockets);
        assert_eq!(config.patrol.interval_seconds, 5);
        assert_eq!(config.periodic_idle_base_delay_ticks, 360);
        assert_eq!(
            config.periodic_idle_anim_modes,
            &ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_ANIM_MODES
        );
        assert!(config.pursue_enabled);
        assert_eq!(config.pursue_stop_distance, 5.0);
        assert_eq!(
            config.stalk,
            Some(RobotsStalkNavMeshConfig::monster_2rockets())
        );
        assert!(config.attacks.iter().all(Option::is_none));
        assert_eq!(config.hit_query_capacity(), 2);

        let primary = config.turn_then_attacks[0].expect("2Rockets primary TurnThenAttack");
        assert_eq!(primary.turn_anim_mode, 0x0900_0026);
        assert_eq!(primary.attack_anim_mode, 0x0900_0025);
        assert_eq!(primary.inner_radius, 1.9);
        assert_eq!(primary.outer_radius, 15.0);
        assert_eq!(
            primary.yaw_completion_tolerance_radians.to_bits(),
            0x3c8e_fa35
        );
        assert_eq!(primary.turn_rate_radians_per_second, std::f32::consts::PI);
        assert_eq!(primary.vertical_limit, 1000.0);
        assert_eq!(primary.reentry_delay_ticks, 120);

        let secondary = config.turn_then_attacks[1].expect("2Rockets secondary TurnThenAttack");
        assert_eq!(secondary.turn_anim_mode, 0x0900_0026);
        assert_eq!(secondary.attack_anim_mode, 0x0900_0027);
        assert_eq!(secondary.inner_radius, 0.0);
        assert_eq!(secondary.outer_radius, 2.0);
        assert_eq!(secondary.reentry_delay_ticks, 30);
        assert!(config.turn_then_attacks[2].is_none());
        assert_eq!(
            RobotsStandardMonsterBehaviorConfig::for_handler_class(
                RobotsAiHandlerClass::Monster2Rockets
            ),
            Some(config)
        );
    }

    #[test]
    fn eq03_spider_config_and_selector_match_native_builder_order() {
        let config = RobotsStandardMonsterBehaviorConfig::eq03_spider();
        assert_eq!(config.handler_class, RobotsAiHandlerClass::Eq03Spider);
        assert_eq!(config.patrol.interval_seconds, 7);
        assert_eq!(config.patrol.target_locomotion_scalar, 0.0);
        assert_eq!(config.patrol.turn_rate, RobotsAiTurnRateInput::Default);
        assert!(config.periodic_idle_anim_modes.is_empty());
        assert!(!config.pursue_enabled);
        assert!(config.stalk.is_none());
        assert!(config.attacks.iter().all(Option::is_none));
        assert!(config.common_hit.is_none());
        assert!(config.has_status_hit_nodes);
        assert!(config.has_magnetic_hit_node());
        assert!(config.uses_common_monster_attack_gate);
        assert_eq!(config.electro_hit_anim_mode, 0x0900_007d);

        let attack = config.turn_then_attacks[0].expect("EQ03 Spider TurnThenAttack");
        assert_eq!(attack.turn_anim_mode, 0);
        assert_eq!(attack.attack_anim_mode, 0x0900_0037);
        assert_eq!(attack.inner_radius, 5.0);
        assert_eq!(attack.outer_radius, 15.0);
        assert_eq!(
            attack.yaw_completion_tolerance_radians.to_bits(),
            0x3e86_0a92
        );
        assert_eq!(attack.turn_rate_radians_per_second, 0.0);
        assert_eq!(attack.vertical_limit, 1000.0);
        assert_eq!(attack.reentry_delay_ticks, 600);
        assert!(config.turn_then_attacks[1].is_none());
        assert!(config.turn_then_attacks[2].is_none());
        assert_eq!(
            RobotsStandardMonsterBehaviorConfig::for_handler_class(
                RobotsAiHandlerClass::Eq03Spider
            ),
            Some(config)
        );

        // Native insertion order is Patrol -> Scrambled -> AttackGroup -> Electro -> Magnetic.
        // Strict greater-than keeps AttackGroup when it ties the later Electro node.
        assert_eq!(
            eq03_spider_behavior_winner(1, 1, 0x55, 0x55, 1),
            Some(RobotsStandardMonsterBehaviorWinner::AttackGroup)
        );
        assert_eq!(
            eq03_spider_behavior_winner(0x55, 0x55, 0x55, 0x55, 0x55),
            Some(RobotsStandardMonsterBehaviorWinner::Patrol)
        );
    }

    #[test]
    fn root_motion_disabled_standard_monsters_keep_native_physics_speed_ranges() {
        assert_eq!(
            standard_monster_physics_locomotion_speed_range(RobotsAiHandlerClass::Ew08Flambe),
            Some([5.0, 5.0])
        );
        assert_eq!(
            standard_monster_physics_locomotion_speed_range(RobotsAiHandlerClass::Ew08FlambeLarge),
            Some([5.0, 5.0])
        );
        assert_eq!(
            standard_monster_physics_locomotion_speed_range(RobotsAiHandlerClass::Ew09Armoured),
            Some([2.0, 8.0])
        );
        assert_eq!(
            standard_monster_physics_locomotion_speed_range(RobotsAiHandlerClass::Ef01Mine),
            Some([2.0, 2.0])
        );
        assert_eq!(
            standard_monster_physics_locomotion_speed_range(RobotsAiHandlerClass::Ew10Minion),
            Some([1.0, 5.0])
        );
        assert_eq!(
            standard_monster_physics_locomotion_speed_range(RobotsAiHandlerClass::MalfBot),
            None
        );
    }

    #[test]
    fn shipped_malf_ew10_eb14_saw_shunt_family_configs_match_recovered_builder_constants() {
        let malf = RobotsStandardMonsterBehaviorConfig::malfbot();
        let malf_attacks = malf.attacks;
        assert_eq!(malf.patrol.interval_seconds, 5);
        assert_eq!(malf.pursue_prelude_anim_mode, Some(0x0900_0027));
        assert_eq!(
            malf.pursue_stop_distance,
            ROBOTS_PURSUE_NAV_NO_STOP_DISTANCE
        );
        assert_eq!(malf_attacks[0].unwrap().outer_radius, 4.5);
        assert_eq!(malf_attacks[1].unwrap().primary_anim_mode, 0x0900_0037);
        assert!(malf_attacks[2].is_none());
        assert!(!malf.uses_common_monster_attack_gate);

        let ew10 = RobotsStandardMonsterBehaviorConfig::ew10_minion();
        let ew10_attacks = ew10.attacks;
        assert_eq!(ew10.patrol.interval_seconds, 3);
        assert_eq!(ew10.pursue_prelude_anim_mode, None);
        assert_eq!(ew10_attacks[0].unwrap().outer_radius, 1.5);
        assert_eq!(ew10_attacks[1].unwrap().primary_anim_mode, 0x0900_0027);
        assert!(ew10_attacks[2].is_none());
        assert_eq!(ew10.stalk, Some(RobotsStalkNavMeshConfig::ew10_minion()));
        assert!(ew10.uses_common_monster_attack_gate);
        assert_eq!(ew10.electro_hit_anim_mode, 0x0900_00b1);

        let eb14 = RobotsStandardMonsterBehaviorConfig::eb14_minion();
        assert_eq!(eb14.pursue_stop_distance, 5.0);
        assert_eq!(eb14.attacks[0].unwrap().outer_radius, 2.0);
        assert_eq!(eb14.stalk, Some(RobotsStalkNavMeshConfig::eb14_minion()));

        let saw = RobotsStandardMonsterBehaviorConfig::sawbot();
        assert_eq!(saw.patrol.interval_seconds, 5);
        assert_eq!(saw.periodic_idle_base_delay_ticks, 120);
        assert_eq!(
            saw.periodic_idle_anim_modes,
            &ROBOTS_SAWBOT_PERIODIC_IDLE_ANIM_MODES
        );
        assert_eq!(saw.stalk, Some(RobotsStalkNavMeshConfig::ew10_minion()));
        assert_eq!(saw.common_hit, Some(RobotsCommonAiHitConfig::sawbot()));
        assert_eq!(saw.attacks[0].unwrap().primary_anim_mode, 0x0900_0025);
        assert_eq!(saw.attacks[0].unwrap().reentry_delay_ticks, 0);
        assert_eq!(saw.attacks[1].unwrap().primary_anim_mode, 0x0900_0027);
        assert_eq!(saw.attacks[1].unwrap().reentry_delay_ticks, 0);
        assert_eq!(saw.attacks[2].unwrap().primary_anim_mode, 0x0900_0037);
        assert_eq!(saw.attacks[2].unwrap().outer_radius, 2.0);
        assert_eq!(saw.attacks[2].unwrap().reentry_delay_ticks, 60);

        let spike = RobotsStandardMonsterBehaviorConfig::spikebot();
        assert_eq!(spike.patrol.interval_seconds, 5);
        assert_eq!(spike.periodic_idle_base_delay_ticks, 120);
        assert!(!spike.pursue_enabled);
        assert!(spike.stalk.is_none());
        assert!(spike.attacks.iter().all(Option::is_none));
        assert_eq!(spike.common_hit, Some(RobotsCommonAiHitConfig::spikebot()));
        assert_eq!(
            spike.spike_attack,
            Some(RobotsSpikeAttackConfig::spikebot())
        );
        assert!(spike.has_status_hit_nodes);

        let jail = RobotsStandardMonsterBehaviorConfig::jailbot_normal();
        assert_eq!(jail.patrol.interval_seconds, 5);
        assert_eq!(jail.periodic_idle_base_delay_ticks, 180);
        assert_eq!(
            jail.periodic_idle_anim_modes,
            &ROBOTS_JAILBOT_PERIODIC_IDLE_ANIM_MODES
        );
        assert!(jail.pursue_enabled);
        assert_eq!(jail.pursue_prelude_anim_mode, None);
        assert_eq!(jail.pursue_stop_distance, 5.0);
        assert_eq!(jail.stalk, Some(RobotsStalkNavMeshConfig::eb14_minion()));
        let jail_attack1 = jail.attacks[0].unwrap();
        assert_eq!(jail_attack1.primary_anim_mode, 0x0900_0025);
        assert_eq!(jail_attack1.inner_radius, 0.0);
        assert_eq!(jail_attack1.outer_radius, 2.0);
        assert_eq!(jail_attack1.yaw_tolerance_radians.to_bits(), 0x3e86_0a92);
        assert_eq!(jail_attack1.reentry_delay_ticks, 0);
        let jail_attack2 = jail.attacks[1].unwrap();
        assert_eq!(jail_attack2.primary_anim_mode, 0x0900_0027);
        assert_eq!(jail_attack2.inner_radius, 2.0);
        assert_eq!(jail_attack2.outer_radius, 4.0);
        assert_eq!(jail_attack2.yaw_tolerance_radians.to_bits(), 0x3dfa_35dd);
        assert_eq!(jail_attack2.reentry_delay_ticks, 30);
        assert!(jail.attacks[2].is_none());
        assert_eq!(
            jail.common_hit,
            Some(RobotsCommonAiHitConfig::fast_front_back())
        );
        assert_eq!(jail.first_update_uniform_scale, 1.0);
        assert!(jail.has_status_hit_nodes);
        assert!(jail.uses_common_monster_attack_gate);

        let jail_large = RobotsStandardMonsterBehaviorConfig::jailbot_large();
        assert_eq!(jail_large.patrol.interval_seconds, 5);
        assert_eq!(
            jail_large.periodic_idle_anim_modes,
            &ROBOTS_JAILBOT_PERIODIC_IDLE_ANIM_MODES
        );
        assert_eq!(jail_large.pursue_stop_distance, 5.0);
        assert_eq!(jail_large.first_update_uniform_scale, 2.0);
        assert_eq!(
            jail_large.stalk,
            Some(RobotsStalkNavMeshConfig::eb14_minion())
        );
        let jail_large_attack = jail_large.attacks[0].unwrap();
        assert_eq!(jail_large_attack.primary_anim_mode, 0x0900_0025);
        assert_eq!(jail_large_attack.inner_radius, 0.0);
        assert_eq!(jail_large_attack.outer_radius, 3.0);
        assert_eq!(
            jail_large_attack.yaw_tolerance_radians.to_bits(),
            0x3e86_0a92
        );
        assert_eq!(jail_large_attack.reentry_delay_ticks, 0);
        assert!(jail_large.attacks[1].is_none());
        assert!(jail_large.attacks[2].is_none());
        assert_eq!(
            jail_large.common_hit,
            Some(RobotsCommonAiHitConfig::fast_front_back())
        );
        assert!(jail_large.has_status_hit_nodes);
        assert!(jail_large.uses_common_monster_attack_gate);

        let shunt = RobotsStandardMonsterBehaviorConfig::shuntbot();
        assert_eq!(shunt.patrol.interval_seconds, 5);
        assert_eq!(shunt.periodic_idle_base_delay_ticks, 120);
        assert_eq!(shunt.pursue_prelude_anim_mode, None);
        assert_eq!(shunt.stalk, Some(RobotsStalkNavMeshConfig::shuntbot()));
        assert!(shunt.attacks.iter().all(Option::is_none));
        assert_eq!(shunt.common_hit, Some(RobotsCommonAiHitConfig::shuntbot()));
        assert_eq!(shunt.shunt_attack, Some(RobotsShuntAttackConfig::normal()));
        assert!(shunt.has_status_hit_nodes);
        assert!(shunt.uses_common_monster_attack_gate);
        assert_eq!(shunt.electro_hit_anim_mode, 0x0900_007d);

        let boss = RobotsStandardMonsterBehaviorConfig::shuntbot_boss();
        assert_eq!(boss.stalk, Some(RobotsStalkNavMeshConfig::ew10_minion()));
        assert!(boss.common_hit.is_none());
        assert_eq!(boss.shunt_attack, Some(RobotsShuntAttackConfig::boss()));
        assert!(!boss.has_status_hit_nodes);
    }

    #[test]
    fn magnabot_builder_config_matches_00464810_without_invented_nodes() {
        let magna = RobotsStandardMonsterBehaviorConfig::magnabot();
        assert_eq!(magna.handler_class, RobotsAiHandlerClass::Eb11MagnaBot);
        assert_eq!(magna.patrol.interval_seconds, 9);
        assert_eq!(magna.patrol.target_locomotion_scalar.to_bits(), 0x3ecc_cccd);
        assert_eq!(
            magna.patrol.turn_rate,
            RobotsAiTurnRateInput::Explicit(std::f32::consts::PI)
        );
        assert!(magna.periodic_idle_anim_modes.is_empty());
        assert!(magna.pursue_enabled);
        assert_eq!(magna.pursue_prelude_anim_mode, None);
        assert_eq!(magna.pursue_stop_distance, 5.0);
        assert_eq!(magna.stalk, Some(RobotsStalkNavMeshConfig::eb14_minion()));
        let attack = magna.attacks[0].expect("Magna generic attack25");
        assert_eq!(attack.primary_anim_mode, 0x0900_0025);
        assert_eq!(attack.inner_radius, 0.0);
        assert_eq!(attack.outer_radius, 9.0);
        assert_eq!(attack.yaw_tolerance_radians.to_bits(), 0x3c9d_466e);
        assert_eq!(attack.reentry_delay_ticks, 0);
        assert!(magna.attacks[1].is_none());
        assert!(magna.attacks[2].is_none());
        assert_eq!(magna.common_hit, Some(RobotsCommonAiHitConfig::magnabot()));
        assert!(magna.has_status_hit_nodes);
        assert!(!magna.has_magnetic_hit_node());
        assert!(magna.uses_common_monster_attack_gate);
    }

    #[test]
    fn magnabot_selector_preserves_native_builder_order_and_priorities() {
        assert_eq!(
            magnabot_behavior_winner(10, 20, 25, false, 1, 11, 1, 1, 1, 1),
            Some(RobotsStandardMonsterBehaviorWinner::TargetFacingTurn)
        );
        assert_eq!(
            magnabot_behavior_winner(10, 20, 25, true, 1, 11, 1, 1, 1, 1),
            Some(RobotsStandardMonsterBehaviorWinner::PursueNavMesh)
        );
        assert_eq!(
            magnabot_behavior_winner(10, 20, 25, true, 40, 11, 1, 1, 1, 1),
            Some(RobotsStandardMonsterBehaviorWinner::StalkNavMesh)
        );
        assert_eq!(
            magnabot_behavior_winner(10, 20, 25, true, 40, 11, 50, 1, 1, 1),
            Some(RobotsStandardMonsterBehaviorWinner::AttackGroup)
        );
        assert_eq!(
            magnabot_behavior_winner(10, 20, 25, true, 40, 11, 50, 100, 100, 1),
            Some(RobotsStandardMonsterBehaviorWinner::CommonHit),
            "strict greater-than must preserve earlier CommonHit on equal priority"
        );
        assert_eq!(
            magnabot_behavior_winner(10, 20, 25, true, 40, 11, 50, 100, 100, 200),
            Some(RobotsStandardMonsterBehaviorWinner::ElectroHit)
        );
    }

    #[test]
    fn eb12_evilbot_builder_config_matches_00465260() {
        let evil = RobotsStandardMonsterBehaviorConfig::eb12_evilbot();
        assert_eq!(evil.handler_class, RobotsAiHandlerClass::Eb12EvilBot);
        assert_eq!(evil.patrol.interval_seconds, 3);
        assert_eq!(evil.patrol.target_locomotion_scalar, 0.0);
        assert_eq!(evil.patrol.turn_rate, RobotsAiTurnRateInput::Default);
        assert!(evil.periodic_idle_anim_modes.is_empty());
        assert!(evil.pursue_enabled);
        assert_eq!(evil.pursue_prelude_anim_mode, None);
        assert_eq!(evil.pursue_stop_distance, 5.0);
        assert_eq!(evil.stalk, Some(RobotsStalkNavMeshConfig::eb14_minion()));

        let attack1 = evil.attacks[0].expect("EB12 attack25");
        assert_eq!(attack1.primary_anim_mode, 0x0900_0025);
        assert_eq!(attack1.inner_radius, 0.0);
        assert_eq!(attack1.outer_radius, 1.5);
        assert_eq!(attack1.yaw_tolerance_radians.to_bits(), 0x3e86_0a92);
        assert_eq!(attack1.reentry_delay_ticks, 0);

        let attack2 = evil.attacks[1].expect("EB12 attack27");
        assert_eq!(attack2.primary_anim_mode, 0x0900_0027);
        assert_eq!(attack2.inner_radius.to_bits(), 0x3fa6_6666);
        assert_eq!(attack2.outer_radius, 2.5);
        assert_eq!(attack2.yaw_tolerance_radians.to_bits(), 0x3e86_0a92);
        assert_eq!(attack2.reentry_delay_ticks, 0x78);
        assert!(evil.attacks[2].is_none());

        assert_eq!(evil.common_hit, Some(RobotsCommonAiHitConfig::evilbot()));
        assert_eq!(
            evil.common_hit.unwrap().turn_rate_radians_per_second,
            std::f32::consts::TAU
        );
        assert!(evil.has_status_hit_nodes);
        assert!(evil.has_magnetic_hit_node());
        assert!(evil.uses_common_monster_attack_gate);
        assert_eq!(evil.electro_hit_anim_mode, 0x0900_007d);
        assert_eq!(
            RobotsAiHandlerClass::Eb12EvilBot.common_fatal_hit_config(),
            Some(RobotsCommonAiHitConfig::evilbot_fatal())
        );
    }

    #[test]
    fn eb15_launcher_builder_config_matches_00465ff0() {
        let launcher = RobotsStandardMonsterBehaviorConfig::eb15_launcher();
        assert_eq!(launcher.handler_class, RobotsAiHandlerClass::Eb15Launcher);
        assert_eq!(launcher.patrol.interval_seconds, 7);
        assert_eq!(launcher.patrol.target_locomotion_scalar, 0.0);
        assert_eq!(launcher.patrol.turn_rate, RobotsAiTurnRateInput::Default);
        assert_eq!(launcher.periodic_idle_base_delay_ticks, 180);
        assert_eq!(
            launcher.periodic_idle_anim_modes,
            &ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_ANIM_MODES
        );
        assert!(launcher.pursue_enabled);
        assert_eq!(launcher.pursue_prelude_anim_mode, None);
        assert_eq!(
            launcher.pursue_stop_distance,
            ROBOTS_PURSUE_NAV_NO_STOP_DISTANCE
        );
        assert_eq!(
            launcher.stalk,
            Some(RobotsStalkNavMeshConfig::eb15_launcher())
        );

        let generic = launcher.attacks[0].expect("EB15 generic attack27");
        assert_eq!(generic.primary_anim_mode, 0x0900_0027);
        assert_eq!(generic.inner_radius, 0.0);
        assert_eq!(generic.outer_radius, 2.0);
        assert_eq!(generic.yaw_tolerance_radians.to_bits(), 0x3e86_0a92);
        assert_eq!(generic.reentry_delay_ticks, 0);
        assert!(launcher.attacks[1].is_none());
        assert!(launcher.attacks[2].is_none());

        assert!(launcher.turn_then_attacks[0].is_none());
        let special = launcher.turn_then_attacks[1].expect("EB15 turn-then-attack child");
        assert!(launcher.turn_then_attacks[2].is_none());
        assert_eq!(special.turn_anim_mode, 0);
        assert_eq!(special.attack_anim_mode, 0x0900_0025);
        assert_eq!(special.inner_radius, 2.0);
        assert_eq!(special.outer_radius, 15.0);
        assert_eq!(
            special.yaw_completion_tolerance_radians.to_bits(),
            0x3c8e_fa35
        );
        assert_eq!(special.turn_rate_radians_per_second, std::f32::consts::TAU);
        assert_eq!(special.vertical_limit, 1000.0);
        assert_eq!(special.reentry_delay_ticks, 120);
        assert!(launcher.secondary_three_phase_attack.is_none());

        assert_eq!(
            launcher.common_hit,
            Some(RobotsCommonAiHitConfig::evilbot())
        );
        assert!(launcher.has_status_hit_nodes);
        assert!(launcher.has_magnetic_hit_node());
        assert!(launcher.uses_common_monster_attack_gate);
        assert_eq!(launcher.electro_hit_anim_mode, 0x0900_007d);
        assert_eq!(
            RobotsAiHandlerClass::Eb15Launcher.common_fatal_hit_config(),
            Some(RobotsCommonAiHitConfig::evilbot_fatal())
        );
        assert_eq!(
            RobotsStandardMonsterBehaviorConfig::for_handler_class(
                RobotsAiHandlerClass::Eb15Launcher
            ),
            Some(launcher)
        );
    }

    #[test]
    fn eb16_knucklebot_builder_config_matches_00466560() {
        let knuckle = RobotsStandardMonsterBehaviorConfig::eb16_knucklebot();
        assert_eq!(knuckle.handler_class, RobotsAiHandlerClass::Eb16KnuckleBot);
        assert_eq!(knuckle.patrol.interval_seconds, 7);
        assert_eq!(knuckle.patrol.target_locomotion_scalar, 0.0);
        assert_eq!(knuckle.patrol.turn_rate, RobotsAiTurnRateInput::Default);
        assert_eq!(knuckle.periodic_idle_base_delay_ticks, 180);
        assert_eq!(
            knuckle.periodic_idle_anim_modes,
            &ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_ANIM_MODES
        );
        assert!(knuckle.pursue_enabled);
        assert_eq!(knuckle.pursue_stop_distance, 5.0);
        assert_eq!(knuckle.stalk, Some(RobotsStalkNavMeshConfig::eb14_minion()));

        let attack25 = knuckle.attacks[0].expect("EB16 attack25");
        assert_eq!(attack25.primary_anim_mode, 0x0900_0025);
        assert_eq!(attack25.inner_radius, 0.0);
        assert_eq!(attack25.outer_radius, 1.5);
        assert_eq!(attack25.yaw_tolerance_radians.to_bits(), 0x3e86_0a92);
        assert_eq!(attack25.reentry_delay_ticks, 120);
        let attack27 = knuckle.attacks[1].expect("EB16 attack27");
        assert_eq!(attack27.primary_anim_mode, 0x0900_0027);
        assert_eq!(attack27.inner_radius, 0.0);
        assert_eq!(attack27.outer_radius, 2.5);
        assert_eq!(attack27.yaw_tolerance_radians.to_bits(), 0x3e86_0a92);
        assert_eq!(attack27.reentry_delay_ticks, 120);
        assert!(knuckle.attacks[2].is_none());
        assert!(knuckle.turn_then_attacks.iter().all(Option::is_none));
        assert!(knuckle.secondary_three_phase_attack.is_none());
        assert_eq!(knuckle.common_hit, Some(RobotsCommonAiHitConfig::evilbot()));
        assert!(knuckle.has_status_hit_nodes);
        assert!(knuckle.has_magnetic_hit_node());
        assert!(knuckle.uses_common_monster_attack_gate);
        assert_eq!(
            RobotsAiHandlerClass::Eb16KnuckleBot.common_fatal_hit_config(),
            Some(RobotsCommonAiHitConfig::evilbot_fatal())
        );
    }

    #[test]
    fn ew08_flambe_shared_builder_config_matches_00462760() {
        let normal = RobotsStandardMonsterBehaviorConfig::flambe();
        let large = RobotsStandardMonsterBehaviorConfig::flambe_large();

        assert_eq!(normal.handler_class, RobotsAiHandlerClass::Ew08Flambe);
        assert_eq!(large.handler_class, RobotsAiHandlerClass::Ew08FlambeLarge);
        assert_eq!(normal.patrol.interval_seconds, 3);
        assert_eq!(normal.pursue_stop_distance, 1.5);
        assert!(normal.stalk.is_none());
        assert!(normal.attacks.iter().all(Option::is_none));
        assert!(normal.turn_then_attacks.iter().all(Option::is_none));
        assert_eq!(
            normal.common_hit,
            Some(RobotsCommonAiHitConfig::common_front_back())
        );
        assert!(normal.has_status_hit_nodes);
        assert!(normal.uses_common_monster_attack_gate);
        assert_eq!(normal.first_update_uniform_scale, 1.0);
        assert_eq!(large.first_update_uniform_scale, 2.0);
        assert_eq!(
            normal.circle_target(),
            Some(RobotsCircleTargetConfig::flambe())
        );

        let direct = normal
            .direct_turn_then_attack()
            .expect("Flambe direct TurnThenAttack");
        assert_eq!(direct.turn_anim_mode, 0x0900_0003);
        assert_eq!(direct.attack_anim_mode, 0x0900_0027);
        assert_eq!(direct.inner_radius, 0.0);
        assert_eq!(direct.outer_radius, 12.0);
        assert_eq!(
            direct.yaw_completion_tolerance_radians.to_bits(),
            0x3c8e_fa35
        );
        assert_eq!(direct.turn_rate_radians_per_second, std::f32::consts::TAU);
        assert_eq!(direct.vertical_limit, 2.0);
        assert_eq!(direct.reentry_delay_ticks, 60);

        assert_eq!(
            RobotsAiHandlerClass::Ew08Flambe.common_fatal_hit_config(),
            Some(RobotsCommonAiHitConfig::common_fatal())
        );
        assert_eq!(
            RobotsAiHandlerClass::Ew08FlambeLarge.common_fatal_hit_config(),
            Some(RobotsCommonAiHitConfig::common_fatal())
        );
        assert_eq!(
            RobotsStandardMonsterBehaviorConfig::for_handler_class(
                RobotsAiHandlerClass::Ew08Flambe
            ),
            Some(normal)
        );
        assert_eq!(
            RobotsStandardMonsterBehaviorConfig::for_handler_class(
                RobotsAiHandlerClass::Ew08FlambeLarge
            ),
            Some(large)
        );
    }

    #[test]
    fn ew08_flambe_selector_preserves_direct_node_and_native_tie_order() {
        assert_eq!(
            flambe_behavior_winner(10, true, 11, 0x32, 0x28, 1, 1, 1, 1),
            Some(RobotsStandardMonsterBehaviorWinner::DirectTurnThenAttack)
        );
        assert_eq!(
            flambe_behavior_winner(10, true, 11, 1, 0x28, 1, 1, 1, 1),
            Some(RobotsStandardMonsterBehaviorWinner::CircleTarget)
        );
        assert_eq!(
            flambe_behavior_winner(10, false, 11, 0x28, 0x28, 1, 1, 1, 1),
            Some(RobotsStandardMonsterBehaviorWinner::DirectTurnThenAttack)
        );
        assert_eq!(
            flambe_behavior_winner(10, false, 11, 1, 1, 0x55, 0x55, 1, 1),
            Some(RobotsStandardMonsterBehaviorWinner::CommonHit)
        );
    }

    #[test]
    fn ew11_fatbot_builder_config_matches_00466a80_and_real_vtable() {
        let fat = RobotsStandardMonsterBehaviorConfig::ew11_fatbot();
        assert_eq!(fat.handler_class, RobotsAiHandlerClass::Ew11FatBot);
        assert_eq!(
            RobotsAiHandlerClass::Ew11FatBot.vtable_address(),
            0x005E_6770
        );
        assert_eq!(fat.patrol.interval_seconds, 7);
        assert_eq!(fat.patrol.target_locomotion_scalar, 0.0);
        assert_eq!(fat.patrol.turn_rate, RobotsAiTurnRateInput::Default);
        assert_eq!(fat.periodic_idle_base_delay_ticks, 180);
        assert_eq!(
            fat.periodic_idle_anim_modes,
            &ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_ANIM_MODES
        );
        assert!(fat.pursue_enabled);
        assert_eq!(fat.pursue_prelude_anim_mode, None);
        assert_eq!(fat.pursue_stop_distance, ROBOTS_PURSUE_NAV_NO_STOP_DISTANCE);
        assert_eq!(fat.stalk, Some(RobotsStalkNavMeshConfig::ew10_minion()));

        let attack25 = fat.attacks[0].expect("EW11 attack25");
        assert_eq!(attack25.primary_anim_mode, 0x0900_0025);
        assert_eq!(attack25.inner_radius, 0.0);
        assert_eq!(attack25.outer_radius, 2.0);
        assert_eq!(attack25.yaw_tolerance_radians.to_bits(), 0x3e86_0a92);
        assert_eq!(attack25.reentry_delay_ticks, 60);
        assert!(fat.attacks[1].is_none());
        assert!(fat.attacks[2].is_none());
        assert_eq!(
            fat.common_hit,
            Some(RobotsCommonAiHitConfig::common_front_back())
        );
        assert_eq!(
            fat.common_hit.unwrap().turn_rate_radians_per_second,
            std::f32::consts::PI
        );
        assert_eq!(
            RobotsAiHandlerClass::Ew11FatBot.common_fatal_hit_config(),
            Some(RobotsCommonAiHitConfig::common_fatal())
        );
        assert!(fat.has_status_hit_nodes);
        assert!(fat.has_magnetic_hit_node());
        assert!(fat.uses_common_monster_attack_gate);
        assert_eq!(
            RobotsStandardMonsterBehaviorConfig::for_handler_class(
                RobotsAiHandlerClass::Ew11FatBot
            ),
            Some(fat)
        );
    }

    #[test]
    fn guardbot_builder_config_matches_0045dd50_and_child_order() {
        let guard = RobotsStandardMonsterBehaviorConfig::guardbot();
        assert_eq!(guard.handler_class, RobotsAiHandlerClass::GuardBot);
        assert_eq!(guard.patrol.interval_seconds, 5);
        assert_eq!(guard.patrol.target_locomotion_scalar, 0.0);
        assert_eq!(guard.patrol.turn_rate, RobotsAiTurnRateInput::Default);
        assert!(guard.periodic_idle_anim_modes.is_empty());
        assert!(guard.pursue_enabled);
        assert_eq!(
            guard.pursue_stop_distance,
            ROBOTS_PURSUE_NAV_NO_STOP_DISTANCE
        );
        assert_eq!(guard.stalk, Some(RobotsStalkNavMeshConfig::ew10_minion()));
        assert!(guard.attacks.iter().all(Option::is_none));
        assert_eq!(
            guard.common_hit,
            Some(RobotsCommonAiHitConfig::fast_front_back())
        );
        assert_eq!(
            RobotsAiHandlerClass::GuardBot.common_fatal_hit_config(),
            Some(RobotsCommonAiHitConfig::fast_front_back_fatal())
        );
        assert!(guard.has_status_hit_nodes);
        assert!(guard.has_magnetic_hit_node());
        assert!(guard.uses_common_monster_attack_gate);
        let turn_attack = guard.turn_then_attacks[0].expect("GuardBot AI_TurnAttack");
        assert!(guard.turn_then_attacks[1..].iter().all(Option::is_none));
        assert_eq!(turn_attack.turn_anim_mode, 0x0900_0003);
        assert_eq!(turn_attack.attack_anim_mode, 0x0900_0025);
        assert_eq!(turn_attack.inner_radius, 0.0);
        assert_eq!(turn_attack.outer_radius, 12.0);
        assert_eq!(
            turn_attack.yaw_completion_tolerance_radians.to_bits(),
            0x3c8e_fa35
        );
        assert_eq!(
            turn_attack.turn_rate_radians_per_second,
            std::f32::consts::TAU
        );
        assert_eq!(turn_attack.vertical_limit, 1000.0);
        assert_eq!(turn_attack.reentry_delay_ticks, 60);
        let circle = guard.circle_target().expect("GuardBot AI_CircleTarget");
        assert_eq!(circle.enter_radius, 15.0);
        assert_eq!(circle.retain_radius, 18.0);
        assert_eq!(circle.orbit_radius, 10.0);
        assert_eq!(circle.direction_mode, 2);
        assert_eq!(
            RobotsStandardMonsterBehaviorConfig::for_handler_class(RobotsAiHandlerClass::GuardBot),
            Some(guard)
        );
    }

    #[test]
    fn guardbot_selector_preserves_stalk_tie_before_circle_target() {
        assert_eq!(
            guardbot_behavior_winner(10, true, 40, 40, 1, 1, 1, 1, 1, 40),
            Some(RobotsStandardMonsterBehaviorWinner::StalkNavMesh),
            "strict greater-than keeps the earlier Stalk node on the 0x28 tie"
        );
        assert_eq!(
            guardbot_behavior_winner(10, true, 40, 40, 1, 1, 1, 1, 50, 40),
            Some(RobotsStandardMonsterBehaviorWinner::AttackGroup)
        );
        assert_eq!(
            guardbot_behavior_winner(10, true, 40, 40, 1, 1, 1, 1, 1, 41),
            Some(RobotsStandardMonsterBehaviorWinner::CircleTarget)
        );
    }

    #[test]
    fn eb07_minebot_builder_config_matches_0045ecf0() {
        let mine = RobotsStandardMonsterBehaviorConfig::eb07_minebot();
        assert_eq!(mine.handler_class, RobotsAiHandlerClass::Eb07MineBot);
        assert_eq!(
            RobotsAiHandlerClass::Eb07MineBot.descriptor_address(),
            0x005E_270C
        );
        assert_eq!(
            RobotsAiHandlerClass::Eb07MineBot.vtable_address(),
            0x005E_3FA0
        );
        assert_eq!(mine.patrol.interval_seconds, 5);
        assert!(!mine.pursue_enabled);
        assert!(mine.stalk.is_none());
        assert_eq!(mine.periodic_idle_base_delay_ticks, 180);
        assert_eq!(
            mine.periodic_idle_anim_modes,
            &ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_ANIM_MODES
        );
        assert_eq!(
            mine.common_hit,
            Some(RobotsCommonAiHitConfig::fast_front_back())
        );
        assert_eq!(
            RobotsAiHandlerClass::Eb07MineBot.common_fatal_hit_config(),
            Some(RobotsCommonAiHitConfig::fast_front_back_fatal())
        );
        assert_eq!(
            mine.flee_navmesh(),
            Some(RobotsFleeNavMeshConfig::eb07_minebot())
        );
        assert_eq!(mine.attack_group_three_phase_index(), Some(1));
        assert_eq!(
            mine.attack_group_three_phase(),
            Some(RobotsThreePhaseAttackConfig::eb07_minebot())
        );
        let attack25 = mine.attacks[0].expect("EB07 primary AI_Attack25");
        assert_eq!(attack25.primary_anim_mode, 0x0900_0025);
        assert_eq!(attack25.outer_radius, 15.0);
        assert_eq!(attack25.yaw_tolerance_radians, std::f32::consts::PI);
        assert_eq!(attack25.priority, 0x51);
        assert_eq!(attack25.reentry_delay_ticks, 120);
        assert_eq!(
            RobotsStandardMonsterBehaviorConfig::for_handler_class(
                RobotsAiHandlerClass::Eb07MineBot
            ),
            Some(mine)
        );
    }

    #[test]
    fn eb07_selector_preserves_flee_before_attack_group_on_tie() {
        assert_eq!(
            eb07_minebot_behavior_winner(1, 1, 1, 1, 1, 1, 1, 0x50, 1),
            Some(RobotsStandardMonsterBehaviorWinner::FleeNavMesh)
        );
        assert_eq!(
            eb07_minebot_behavior_winner(1, 1, 1, 1, 1, 1, 1, 0x50, 0x50),
            Some(RobotsStandardMonsterBehaviorWinner::FleeNavMesh),
            "strict greater-than keeps the earlier Flee node on an equal-priority tie"
        );
        assert_eq!(
            eb07_minebot_behavior_winner(1, 1, 1, 1, 1, 1, 1, 0x50, 0x51),
            Some(RobotsStandardMonsterBehaviorWinner::AttackGroup)
        );
    }

    #[test]
    fn shieldbot_builder_config_matches_0045e240_and_hitcheck_capacity() {
        let shield = RobotsStandardMonsterBehaviorConfig::shieldbot();
        assert_eq!(shield.handler_class, RobotsAiHandlerClass::ShieldBot);
        assert_eq!(
            RobotsAiHandlerClass::ShieldBot.descriptor_address(),
            0x005E_26EC
        );
        assert_eq!(
            RobotsAiHandlerClass::ShieldBot.vtable_address(),
            0x005E_3CD0
        );
        assert_eq!(shield.patrol.interval_seconds, 5);
        assert_eq!(shield.pursue_stop_distance, 5.0);
        assert_eq!(shield.stalk, Some(RobotsStalkNavMeshConfig::eb14_minion()));
        assert_eq!(shield.periodic_idle_base_delay_ticks, 180);
        assert_eq!(
            shield.periodic_idle_anim_modes,
            &ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_ANIM_MODES
        );
        assert_eq!(
            shield.common_hit,
            Some(RobotsCommonAiHitConfig::fast_front_back())
        );
        assert_eq!(
            RobotsAiHandlerClass::ShieldBot.common_fatal_hit_config(),
            Some(RobotsCommonAiHitConfig::fast_front_back_fatal())
        );
        assert_eq!(shield.hit_query_capacity(), 1);

        let attack25 = shield.attacks[0].expect("ShieldBot AI_Attack25");
        assert_eq!(attack25.primary_anim_mode, 0x0900_0025);
        assert_eq!(attack25.inner_radius, 0.0);
        assert_eq!(attack25.outer_radius, 2.0);
        assert_eq!(attack25.yaw_tolerance_radians.to_bits(), 0x3db2_b8c2);
        assert_eq!(attack25.vertical_limit, 1000.0);
        assert_eq!(attack25.reentry_delay_ticks, 0);
        assert!(shield.attacks[1].is_none());
        assert!(shield.attacks[2].is_none());

        assert!(shield.turn_then_attacks[0].is_none());
        let turn_attack = shield.turn_then_attacks[1].expect("ShieldBot AI_TurnAttack");
        assert!(shield.turn_then_attacks[2].is_none());
        assert_eq!(turn_attack.turn_anim_mode, 0x0900_0003);
        assert_eq!(turn_attack.attack_anim_mode, 0x0900_0027);
        assert_eq!(turn_attack.inner_radius, 2.0);
        assert_eq!(turn_attack.outer_radius, 15.0);
        assert_eq!(
            turn_attack.yaw_completion_tolerance_radians.to_bits(),
            0x3c8e_fa35
        );
        assert_eq!(
            turn_attack.turn_rate_radians_per_second,
            std::f32::consts::TAU
        );
        assert_eq!(turn_attack.vertical_limit, 1.0);
        assert_eq!(turn_attack.reentry_delay_ticks, 30);
        assert_eq!(
            RobotsStandardMonsterBehaviorConfig::for_handler_class(RobotsAiHandlerClass::ShieldBot),
            Some(shield)
        );
    }

    #[test]
    fn shieldbot_selector_preserves_native_builder_ties() {
        assert_eq!(
            shieldbot_behavior_winner(10, true, 40, 40, 40, 1, 1, 1, 1, 40),
            Some(RobotsStandardMonsterBehaviorWinner::StalkNavMesh),
            "Stalk is inserted before PatrolNavMesh2/PeriodicIdle/AttackGroup on equal priority"
        );
        assert_eq!(
            shieldbot_behavior_winner(10, true, 1, 1, 40, 40, 1, 1, 1, 40),
            Some(RobotsStandardMonsterBehaviorWinner::PeriodicIdle),
            "PeriodicIdle is inserted before CommonHit/AttackGroup on equal priority"
        );
        assert_eq!(
            shieldbot_behavior_winner(10, true, 1, 1, 1, 50, 1, 1, 1, 50),
            Some(RobotsStandardMonsterBehaviorWinner::CommonHit),
            "CommonHit keeps the tie because AttackGroup is appended last"
        );
    }

    #[test]
    fn securitybot_builder_config_matches_0045e7a0() {
        let security = RobotsStandardMonsterBehaviorConfig::securitybot();
        assert_eq!(security.handler_class, RobotsAiHandlerClass::SecurityBot);
        assert_eq!(security.patrol.interval_seconds, 5);
        assert_eq!(security.pursue_stop_distance, 5.0);
        assert_eq!(
            security.stalk,
            Some(RobotsStalkNavMeshConfig::eb14_minion())
        );
        assert_eq!(security.periodic_idle_base_delay_ticks, 180);
        assert_eq!(
            security.periodic_idle_anim_modes,
            &ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_ANIM_MODES
        );
        let attack25 = security.attacks[0].expect("SecurityBot attack25");
        assert_eq!(attack25.primary_anim_mode, 0x0900_0025);
        assert_eq!(attack25.inner_radius, 0.0);
        assert_eq!(attack25.outer_radius, 2.0);
        assert_eq!(attack25.yaw_tolerance_radians.to_bits(), 0x3f06_0a92);
        assert_eq!(attack25.reentry_delay_ticks, 0);
        let attack27 = security.attacks[1].expect("SecurityBot attack27");
        assert_eq!(attack27.primary_anim_mode, 0x0900_0027);
        assert_eq!(attack27.inner_radius, 3.0);
        assert_eq!(attack27.outer_radius, 4.5);
        assert_eq!(attack27.yaw_tolerance_radians.to_bits(), 0x3f06_0a92);
        assert_eq!(attack27.reentry_delay_ticks, 120);
        assert!(security.attacks[2].is_none());
        assert_eq!(
            security.common_hit,
            Some(RobotsCommonAiHitConfig::fast_front_back())
        );
        assert_eq!(
            RobotsAiHandlerClass::SecurityBot.common_fatal_hit_config(),
            Some(RobotsCommonAiHitConfig::fast_front_back_fatal())
        );
        assert!(security.has_status_hit_nodes);
        assert!(security.has_magnetic_hit_node());
        assert_eq!(
            RobotsStandardMonsterBehaviorConfig::for_handler_class(
                RobotsAiHandlerClass::SecurityBot
            ),
            Some(security)
        );
    }

    #[test]
    fn constructionbot_builder_config_matches_0045d7f0() {
        let construction = RobotsStandardMonsterBehaviorConfig::constructionbot();
        assert_eq!(
            construction.handler_class,
            RobotsAiHandlerClass::ConstructionBot
        );
        assert_eq!(construction.patrol.interval_seconds, 5);
        assert_eq!(construction.pursue_stop_distance, 5.0);
        assert_eq!(
            construction.stalk,
            Some(RobotsStalkNavMeshConfig::eb14_minion())
        );
        assert_eq!(construction.periodic_idle_base_delay_ticks, 180);
        assert_eq!(
            construction.periodic_idle_anim_modes,
            &ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_ANIM_MODES
        );
        let attack25 = construction.attacks[0].expect("ConstructionBot attack25");
        assert_eq!(attack25.primary_anim_mode, 0x0900_0025);
        assert_eq!(attack25.inner_radius, 0.0);
        assert_eq!(attack25.outer_radius, 1.5);
        assert_eq!(attack25.reentry_delay_ticks, 0);
        let attack27 = construction.attacks[1].expect("ConstructionBot attack27");
        assert_eq!(attack27.primary_anim_mode, 0x0900_0027);
        assert_eq!(attack27.inner_radius, 1.0);
        assert_eq!(attack27.outer_radius, 3.0);
        assert_eq!(attack27.reentry_delay_ticks, 120);
        assert_eq!(
            construction.common_hit,
            Some(RobotsCommonAiHitConfig::fast_front_back())
        );
        assert_eq!(
            RobotsAiHandlerClass::ConstructionBot.common_fatal_hit_config(),
            Some(RobotsCommonAiHitConfig::fast_front_back_fatal())
        );
        assert!(construction.has_status_hit_nodes);
        assert!(construction.has_magnetic_hit_node());
        assert_eq!(
            RobotsStandardMonsterBehaviorConfig::for_handler_class(
                RobotsAiHandlerClass::ConstructionBot
            ),
            Some(construction)
        );
    }

    #[test]
    fn thiefbot_builder_config_and_selector_match_0045f240_order() {
        let thief = RobotsStandardMonsterBehaviorConfig::thiefbot();
        assert_eq!(thief.handler_class, RobotsAiHandlerClass::ThiefBot);
        assert_eq!(thief.patrol.interval_seconds, 5);
        assert!(thief.pursue_enabled);
        assert_eq!(thief.pursue_stop_distance, 1.5);
        assert_eq!(thief.stalk, Some(RobotsStalkNavMeshConfig::ew10_minion()));
        assert_eq!(thief.periodic_idle_base_delay_ticks, 180);
        assert_eq!(
            thief.periodic_idle_anim_modes,
            &ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_ANIM_MODES
        );

        let attack25 = thief.attacks[0].expect("ThiefBot attack25");
        assert_eq!(attack25.primary_anim_mode, 0x0900_0025);
        assert_eq!(attack25.inner_radius, 0.0);
        assert_eq!(attack25.outer_radius, 2.0);
        assert_eq!(
            attack25.yaw_tolerance_radians,
            ROBOTS_EW10_EB14_ATTACK_YAW_TOLERANCE_RADIANS
        );
        assert_eq!(attack25.reentry_delay_ticks, 0);

        let attack27 = thief.attacks[1].expect("ThiefBot attack27");
        assert_eq!(attack27.primary_anim_mode, 0x0900_0027);
        assert_eq!(attack27.inner_radius, 1.0);
        assert_eq!(attack27.outer_radius, 3.0);
        assert_eq!(attack27.reentry_delay_ticks, 120);
        assert_eq!(
            thief.common_hit,
            Some(RobotsCommonAiHitConfig::fast_front_back())
        );
        assert!(thief.has_status_hit_nodes);
        assert!(thief.has_magnetic_hit_node());
        assert_eq!(
            RobotsStandardMonsterBehaviorConfig::for_handler_class(RobotsAiHandlerClass::ThiefBot),
            Some(thief)
        );

        // AttackGroup is inserted before the final Stalk node. Strict '>'
        // selection therefore keeps AttackGroup on an artificial priority tie.
        assert_eq!(
            thiefbot_behavior_winner(1, false, 1, 1, 1, 1, 1, 1, 0x28, 0x28),
            Some(RobotsStandardMonsterBehaviorWinner::AttackGroup)
        );
    }

    #[test]
    fn ew09_armoured_builder_and_first_update_path_match_native_contract() {
        let armoured = RobotsStandardMonsterBehaviorConfig::ew09_armoured();
        assert_eq!(armoured.handler_class, RobotsAiHandlerClass::Ew09Armoured);
        assert_eq!(
            RobotsAiHandlerClass::Ew09Armoured.vtable_address(),
            0x005E_50E0
        );
        assert_eq!(armoured.patrol.interval_seconds, 6);
        assert_eq!(armoured.patrol.target_locomotion_scalar, 0.4);
        assert_eq!(
            armoured.patrol.turn_rate,
            RobotsAiTurnRateInput::Explicit(std::f32::consts::PI)
        );
        assert!(armoured.periodic_idle_anim_modes.is_empty());
        assert!(!armoured.pursue_enabled);
        assert!(armoured.stalk.is_none());
        assert!(armoured.common_hit.is_none());
        assert!(!armoured.has_status_hit_nodes);
        assert!(armoured.uses_common_monster_attack_gate);
        assert_eq!(armoured.attacks[0], Some(ew09_attack_config()));
        assert!(armoured.attacks[1].is_none());
        assert!(armoured.attacks[2].is_none());
        assert_eq!(
            RobotsStandardMonsterBehaviorConfig::for_handler_class(
                RobotsAiHandlerClass::Ew09Armoured
            ),
            Some(armoured)
        );

        assert_eq!(
            ew09_armoured_behavior_winner(0x0A, 1, 0x10),
            Some(RobotsStandardMonsterBehaviorWinner::FollowNetworkPath)
        );
        assert_eq!(
            ew09_armoured_behavior_winner(0x0A, 0x32, 0x10),
            Some(RobotsStandardMonsterBehaviorWinner::AttackGroup)
        );
    }

    #[test]
    fn ef03_evilbot_builder_and_first_update_sound_match_native_contract() {
        let evil = RobotsStandardMonsterBehaviorConfig::ef03_evilbot();
        assert_eq!(evil.handler_class, RobotsAiHandlerClass::Ef03EvilBot);
        assert_eq!(
            RobotsAiHandlerClass::Ef03EvilBot.vtable_address(),
            0x005E_68D8
        );
        assert_eq!(evil.patrol.interval_seconds, 7);
        assert_eq!(
            evil.pursue_stop_distance,
            ROBOTS_PURSUE_NAV_NO_STOP_DISTANCE
        );
        assert_eq!(evil.stalk, Some(RobotsStalkNavMeshConfig::ew10_minion()));
        assert_eq!(evil.periodic_idle_base_delay_ticks, 180);
        assert_eq!(
            evil.periodic_idle_anim_modes,
            &ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_ANIM_MODES
        );

        let attack25 = evil.attacks[0].expect("EF03 attack25");
        assert_eq!(attack25.primary_anim_mode, 0x0900_0025);
        assert_eq!(attack25.inner_radius, 0.0);
        assert_eq!(attack25.outer_radius, 1.0);
        assert_eq!(attack25.yaw_tolerance_radians.to_bits(), 0x3e86_0a92);
        assert_eq!(attack25.reentry_delay_ticks, 0);
        let attack27 = evil.attacks[1].expect("EF03 attack27");
        assert_eq!(attack27.primary_anim_mode, 0x0900_0027);
        assert_eq!(attack27.inner_radius, 1.0);
        assert_eq!(attack27.outer_radius, 1.5);
        assert_eq!(attack27.yaw_tolerance_radians.to_bits(), 0x3e86_0a92);
        assert_eq!(attack27.reentry_delay_ticks, 60);
        assert!(evil.attacks[2].is_none());

        assert_eq!(
            evil.common_hit,
            Some(RobotsCommonAiHitConfig::common_front_back())
        );
        assert_eq!(
            evil.common_hit.unwrap().turn_rate_radians_per_second,
            std::f32::consts::PI
        );
        assert_eq!(
            RobotsAiHandlerClass::Ef03EvilBot.common_fatal_hit_config(),
            Some(RobotsCommonAiHitConfig::common_fatal())
        );
        assert!(evil.has_status_hit_nodes);
        assert!(evil.has_magnetic_hit_node());
        assert_eq!(
            evil.first_update_permanent_sound(),
            Some(RobotsPermanentSoundConfig {
                sound_uid: 0x1AF0_0155,
                native_parameter: 100,
            })
        );
        assert_eq!(
            RobotsStandardMonsterBehaviorConfig::for_handler_class(
                RobotsAiHandlerClass::Ef03EvilBot
            ),
            Some(evil)
        );
    }

    #[test]
    fn sweeper_builder_config_and_order_match_00460380() {
        let sweeper = RobotsStandardMonsterBehaviorConfig::sweeper();
        assert_eq!(sweeper.handler_class, RobotsAiHandlerClass::Sweeper);
        assert!(!sweeper.has_patrol_node());
        assert_eq!(sweeper.periodic_idle_base_delay_ticks, 180);
        assert_eq!(
            sweeper.periodic_idle_anim_modes,
            &ROBOTS_STANDARD_MONSTER_PERIODIC_IDLE_ANIM_MODES
        );
        assert!(sweeper.pursue_enabled);
        assert_eq!(sweeper.pursue_stop_distance.to_bits(), 1.5f32.to_bits());
        assert_eq!(sweeper.stalk, Some(RobotsStalkNavMeshConfig::ew10_minion()));
        assert_eq!(sweeper.common_hit, Some(RobotsCommonAiHitConfig::fast_front_back()));
        assert!(sweeper.has_status_hit_nodes);
        assert!(sweeper.has_magnetic_hit_node());

        let attack25 = sweeper.attacks[0].expect("Sweeper attack25");
        assert_eq!(attack25.primary_anim_mode, 0x0900_0025);
        assert_eq!(attack25.inner_radius.to_bits(), 0.0f32.to_bits());
        assert_eq!(attack25.outer_radius.to_bits(), 2.0f32.to_bits());
        assert_eq!(attack25.yaw_tolerance_radians.to_bits(), 0x3e86_0a92);
        assert_eq!(attack25.reentry_delay_ticks, 0);

        let attack27 = sweeper.attacks[1].expect("Sweeper attack27");
        assert_eq!(attack27.primary_anim_mode, 0x0900_0027);
        assert_eq!(attack27.inner_radius.to_bits(), 2.0f32.to_bits());
        assert_eq!(attack27.outer_radius.to_bits(), 4.0f32.to_bits());
        assert_eq!(attack27.yaw_tolerance_radians.to_bits(), 0x3e86_0a92);
        assert_eq!(attack27.reentry_delay_ticks, 120);
        assert!(sweeper.attacks[2].is_none());
        assert_eq!(
            RobotsAiHandlerClass::Sweeper.common_fatal_hit_config(),
            Some(RobotsCommonAiHitConfig::fast_front_back_fatal())
        );
        assert_eq!(
            RobotsStandardMonsterBehaviorConfig::for_handler_class(RobotsAiHandlerClass::Sweeper),
            Some(sweeper)
        );

        // Strict-greater selector preserves native insertion order. Stalk outranks
        // Pursue (0x28 vs 0x1F) and, because it precedes PatrolNavMesh2, also
        // retains an artificial equal-priority tie against that later node.
        assert_eq!(
            sweeper_behavior_winner(1, true, 0x28, 0x28, 1, 1, 1, 1, 1),
            Some(RobotsStandardMonsterBehaviorWinner::StalkNavMesh)
        );
        assert_eq!(
            sweeper_behavior_winner(1, false, 0x28, 0x28, 1, 1, 1, 1, 0x32),
            Some(RobotsStandardMonsterBehaviorWinner::AttackGroup)
        );
    }

    #[test]
    fn common_hit_keeps_native_insertion_precedence_over_equal_status_priorities() {
        assert_eq!(
            standard_monster_behavior_winner(
                1, 0x32, 1, true, 0x28, 0x16, 0x0b, 0x0a, 100, 100, 100, 100,
            ),
            Some(RobotsStandardMonsterBehaviorWinner::CommonHit)
        );
    }

    #[test]
    fn stalk_priority_sits_between_pursue_and_generic_attack() {
        assert_eq!(ROBOTS_STALK_NAVMESH_PRIORITY, 0x28);
        assert_eq!(
            standard_monster_behavior_winner(1, 1, 1, true, 0x28, 0x16, 0x0b, 0x0a, 1, 1, 1, 1),
            Some(RobotsStandardMonsterBehaviorWinner::StalkNavMesh)
        );
        assert_eq!(
            standard_monster_behavior_winner(0x32, 1, 1, true, 0x28, 0x16, 0x0b, 0x0a, 1, 1, 1, 1),
            Some(RobotsStandardMonsterBehaviorWinner::AttackGroup)
        );
    }
}
