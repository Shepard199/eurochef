use serde::Serialize;

use super::hit_reaction::RobotsCommonAiHitConfig;

/// Exact concrete XItemHandler selected by native `0x0047EA70` from
/// MonsterDatabase runtime sheet (5..12) and row/config index.
///
/// This is engine-neutral identity. Behavior modules key from this enum; GUI and
/// future UE5.8 hosts must not infer a brain from the character EDB filename.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum RobotsAiHitReactionKind {
    Common,
    RollerBot,
    Ef01Mine,
    TurretFamily,
    Dodgem,
    TurretBot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum RobotsAiHandlerClass {
    MonsterBase,
    Monster2Rockets,
    ConstructionBot,
    DogBot,
    Eb07MineBot,
    Eb10RollerBot,
    Eb11MagnaBot,
    Eb12EvilBot,
    Eb13KnightBot,
    Eb14Minion,
    Eb15Launcher,
    Eb16KnuckleBot,
    Ef01Mine,
    Ef03EvilBot,
    Em07PiranhaBot,
    Ep02Turret,
    Ep04Turret,
    Ep05Turret,
    Ep06Turret,
    Eq02MineBot,
    Eq03Spider,
    Eq04Mine,
    Ew07Dodgem,
    Ew08Flambe,
    Ew08FlambeLarge,
    Ew09Armoured,
    Ew10Minion,
    Ew11FatBot,
    GuardBot,
    JailBotLarge,
    JailBotNormal,
    MalfBot,
    SawBot,
    SecurityBot,
    ShieldBot,
    ShuntBot,
    ShuntBotBoss,
    SpikeBot,
    SpinTop,
    Sweeper,
    TestAnimBot,
    ThiefBot,
    TurretBot,
    Npc,
    NpcFender,
}

pub const ROBOTS_MONSTER_ATTACK_COOLDOWN_SECONDS: f32 = 1.5;
pub const ROBOTS_MONSTER_ATTACK_PLAYER_REACTION_EPSILON: f32 = 0.001;
pub const ROBOTS_MONSTER_ATTACK_BYPASS_FLAG: u32 = 0x0001_0000;

/// Common Monster state0 tail `0x00451BF0` player-proximity constants.
pub const ROBOTS_MONSTER_PLAYER_PROXIMITY_HORIZONTAL_RADIUS: f32 = 5.0;
pub const ROBOTS_MONSTER_PLAYER_PROXIMITY_VERTICAL_TOLERANCE: f32 = 2.75;
pub const ROBOTS_MONSTER_PLAYER_PROXIMITY_SMOOTHING: f32 = 0.5;
pub const ROBOTS_MONSTER_PLAYER_PROXIMITY_DISABLE_EPSILON: f32 = 0.001;
pub const ROBOTS_MONSTER_PLAYER_PROXIMITY_I16_SCALE: f32 = 32767.0;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsMonsterPlayerProximityRuntimeState {
    /// Handler+0x5F4. Common Monster ctor `0x004514F0` seeds this to 1.0.
    pub factor: f32,
    /// XItem service mask bit4 toggled through `0x004441B0`.
    pub xitem_service_bit4_enabled: bool,
    /// Owner XItem+0x68 written only while factor is >=0.001.
    pub owner_scale_i16: i16,
    /// Owner XItem+0x65 bit1 is ORed on every enabled write and never cleared by
    /// this helper.
    pub owner_flag_65_bit1: bool,
}

impl Default for RobotsMonsterPlayerProximityRuntimeState {
    fn default() -> Self {
        Self {
            factor: 1.0,
            xitem_service_bit4_enabled: false,
            owner_scale_i16: 0,
            owner_flag_65_bit1: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsMonsterPlayerProximityStep {
    pub factor: f32,
    pub xitem_service_bit4_enabled: Option<bool>,
    pub owner_scale_i16: Option<i16>,
    pub set_owner_flag_65_bit1: bool,
}

/// Engine-neutral reducer for common Monster state0 tail `0x00451BF0`.
/// `player_position_xyz=None` models native `DAT_007B3220 == 0`, which is a
/// complete no-op. Float-to-short uses the exact `0x00582D2C` x87 truncation
/// contract after multiplying by 32767.0.
pub fn step_monster_player_proximity(
    state: &mut RobotsMonsterPlayerProximityRuntimeState,
    owner_position_xyz: [f32; 3],
    player_position_xyz: Option<[f32; 3]>,
) -> RobotsMonsterPlayerProximityStep {
    let Some(player) = player_position_xyz else {
        return RobotsMonsterPlayerProximityStep {
            factor: state.factor,
            xitem_service_bit4_enabled: None,
            owner_scale_i16: None,
            set_owner_flag_65_bit1: false,
        };
    };

    let dx = owner_position_xyz[0] - player[0];
    let dz = owner_position_xyz[2] - player[2];
    let horizontal_distance = (dx * dx + dz * dz).max(0.0).sqrt();
    let inside = horizontal_distance <= ROBOTS_MONSTER_PLAYER_PROXIMITY_HORIZONTAL_RADIUS
        && (owner_position_xyz[1] - player[1]).abs()
            <= ROBOTS_MONSTER_PLAYER_PROXIMITY_VERTICAL_TOLERANCE;
    let target = if inside { 1.0 } else { 0.0 };
    state.factor += (target - state.factor) * ROBOTS_MONSTER_PLAYER_PROXIMITY_SMOOTHING;

    if state.factor < ROBOTS_MONSTER_PLAYER_PROXIMITY_DISABLE_EPSILON {
        state.xitem_service_bit4_enabled = false;
        return RobotsMonsterPlayerProximityStep {
            factor: state.factor,
            xitem_service_bit4_enabled: Some(false),
            owner_scale_i16: None,
            set_owner_flag_65_bit1: false,
        };
    }

    let scale = (state.factor * ROBOTS_MONSTER_PLAYER_PROXIMITY_I16_SCALE).trunc() as i16;
    state.xitem_service_bit4_enabled = true;
    state.owner_scale_i16 = scale;
    state.owner_flag_65_bit1 = true;
    RobotsMonsterPlayerProximityStep {
        factor: state.factor,
        xitem_service_bit4_enabled: Some(true),
        owner_scale_i16: Some(scale),
        set_owner_flag_65_bit1: true,
    }
}

/// Process-wide monster attack lockout represented by native
/// `DAT_007B2A18 - DAT_007B2A3C`. Attack-node enter paths store the current
/// global clock in `DAT_007B2A3C`; class gates reject a new attack for 1.5s.
/// Keeping elapsed time instead of a renderer/wall clock makes this portable to UE
/// and lets the host replace the clock adapter later without changing class brains.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct RobotsMonsterAttackCooldownRuntime {
    elapsed_since_attack_seconds: Option<f32>,
}

impl RobotsMonsterAttackCooldownRuntime {
    pub fn advance(&mut self, delta_seconds: f32) {
        if let Some(elapsed) = &mut self.elapsed_since_attack_seconds {
            *elapsed += delta_seconds.max(0.0);
        }
    }

    pub fn record_attack(&mut self) {
        self.elapsed_since_attack_seconds = Some(0.0);
    }

    pub fn elapsed_since_attack_seconds(self) -> Option<f32> {
        self.elapsed_since_attack_seconds
    }

    pub fn attack_allowed(self) -> bool {
        self.elapsed_since_attack_seconds
            .is_none_or(|elapsed| elapsed >= ROBOTS_MONSTER_ATTACK_COOLDOWN_SECONDS)
    }
}

/// Common monster vslot `+0x158 = 0x00455150` attack admission gate.
///
/// The host supplies the current game-state/player-state view because those live
/// outside an AI Handler. The process-wide cooldown is shared by all monsters.
pub fn robots_player_reaction_gate_active(
    top_game_state: Option<u32>,
    player_state_6de: u8,
    target_is_player_category: bool,
    player_reaction_window_6e4: f32,
) -> bool {
    let top_game_state = top_game_state.unwrap_or(0);
    let game_state_bypasses_reaction = matches!(top_game_state, 1 | 2 | 3 | 0x0f);
    let player_state_bypasses_reaction = matches!(player_state_6de, 1 | 0x1d | 0x2f | 0x3e);
    !game_state_bypasses_reaction
        && !player_state_bypasses_reaction
        && target_is_player_category
        && player_reaction_window_6e4 > ROBOTS_MONSTER_ATTACK_PLAYER_REACTION_EPSILON
}

pub fn robots_common_monster_attack_allowed(
    handler_flags_628: u32,
    top_game_state: Option<u32>,
    player_state_6de: u8,
    target_is_player_category: bool,
    player_reaction_window_6e4: f32,
    cooldown: RobotsMonsterAttackCooldownRuntime,
) -> bool {
    if handler_flags_628 & ROBOTS_MONSTER_ATTACK_BYPASS_FLAG != 0 {
        return true;
    }

    if robots_player_reaction_gate_active(
        top_game_state,
        player_state_6de,
        target_is_player_category,
        player_reaction_window_6e4,
    ) {
        return false;
    }

    cooldown.attack_allowed()
}

pub const ROBOTS_AI_PATROL_PRIORITY: u8 = 10;
pub const ROBOTS_AI_PATROL_BLOCKED_PRIORITY: u8 = 1;
pub const ROBOTS_AI_PATROL_HALF_TURN_RADIANS: f32 = std::f32::consts::PI;
pub const ROBOTS_AI_PATROL_TICKS_PER_SECOND: i32 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsAiPatrolConfig {
    /// Node+0x2C written by `0x0046BCF0`.
    pub base_yaw_radians: f32,
    /// Builder seconds, multiplied by 60 into node+0x24/+0x28.
    pub interval_seconds: i32,
    /// First argument to handler vslot +0x10C.
    pub target_locomotion_scalar: f32,
    /// Second argument to handler vslot +0x10C. Native `-1` means default rate.
    pub turn_rate: crate::robots_runtime::locomotion::RobotsAiTurnRateInput,
}

impl RobotsAiPatrolConfig {
    pub fn interval_ticks(self) -> i32 {
        self.interval_seconds
            .saturating_mul(ROBOTS_AI_PATROL_TICKS_PER_SECOND)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsAiPatrolRuntimeState {
    /// Node+0x28.
    pub countdown_ticks: i32,
    /// Node+0x38.
    pub alternate_heading: bool,
    /// Handler+0x5D8. Base AI ctor seeds this lane to zero.
    pub steering_target_yaw_radians: f32,
}

impl RobotsAiPatrolRuntimeState {
    pub fn configured(config: RobotsAiPatrolConfig) -> Self {
        Self {
            countdown_ticks: config.interval_ticks(),
            alternate_heading: false,
            steering_target_yaw_radians: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsAiPatrolStep {
    pub steering_target_yaw_radians: f32,
    pub target_locomotion_scalar: f32,
    pub turn_rate: crate::robots_runtime::locomotion::RobotsAiTurnRateInput,
}

/// Native `AI_Patrol` priority callback `0x0046BD80`.
pub fn ai_patrol_priority(top_game_state: Option<u32>) -> u8 {
    if matches!(top_game_state, Some(1 | 2 | 3 | 0x0f)) {
        ROBOTS_AI_PATROL_BLOCKED_PRIORITY
    } else {
        ROBOTS_AI_PATROL_PRIORITY
    }
}

/// Native `AI_Patrol` execute callback `0x0046BD30`.
pub fn step_ai_patrol(
    state: &mut RobotsAiPatrolRuntimeState,
    config: RobotsAiPatrolConfig,
) -> RobotsAiPatrolStep {
    if state.countdown_ticks == 0 {
        state.alternate_heading = !state.alternate_heading;
        state.steering_target_yaw_radians = if state.alternate_heading {
            config.base_yaw_radians + ROBOTS_AI_PATROL_HALF_TURN_RADIANS
        } else {
            config.base_yaw_radians
        };
        state.countdown_ticks = config.interval_ticks();
    }
    state.countdown_ticks = state.countdown_ticks.saturating_sub(1);

    RobotsAiPatrolStep {
        steering_target_yaw_radians: state.steering_target_yaw_radians,
        target_locomotion_scalar: config.target_locomotion_scalar,
        turn_rate: config.turn_rate,
    }
}

impl RobotsAiHandlerClass {
    /// Concrete handlers whose behavior builders have been proven to install the
    /// common `AI_HitFatal` node (`0x00457E30 / vtable 0x005E240C`). Keep this
    /// explicit as the reverse frontier expands; sharing +0xC8 alone does not prove
    /// that a class uses the same fatal selector.
    pub const fn common_fatal_hit_config(self) -> Option<RobotsCommonAiHitConfig> {
        match self {
            Self::DogBot | Self::Eq02MineBot | Self::MalfBot | Self::Eb10RollerBot => {
                Some(RobotsCommonAiHitConfig::common_fatal())
            }
            Self::Eb11MagnaBot => Some(RobotsCommonAiHitConfig::magnabot_fatal()),
            Self::Eb12EvilBot | Self::Eb13KnightBot | Self::Eb15Launcher | Self::Eb16KnuckleBot => {
                Some(RobotsCommonAiHitConfig::evilbot_fatal())
            }
            Self::Ew11FatBot | Self::Ef03EvilBot => Some(RobotsCommonAiHitConfig::common_fatal()),
            Self::ConstructionBot
            | Self::SecurityBot
            | Self::GuardBot
            | Self::ShieldBot
            | Self::Eb07MineBot => Some(RobotsCommonAiHitConfig::fast_front_back_fatal()),
            Self::Ep02Turret | Self::Ep04Turret | Self::Ep05Turret | Self::Ep06Turret => {
                Some(RobotsCommonAiHitConfig::turret_fatal())
            }
            Self::Ew07Dodgem => Some(RobotsCommonAiHitConfig::dodgem_fatal()),
            Self::SpinTop | Self::TestAnimBot | Self::Sweeper => {
                Some(RobotsCommonAiHitConfig::fast_front_back_fatal())
            }
            Self::Ew08Flambe | Self::Ew08FlambeLarge => {
                Some(RobotsCommonAiHitConfig::common_fatal())
            }
            Self::SawBot => Some(RobotsCommonAiHitConfig::sawbot_fatal()),
            Self::SpikeBot => Some(RobotsCommonAiHitConfig::spikebot_fatal()),
            Self::JailBotNormal | Self::JailBotLarge => {
                Some(RobotsCommonAiHitConfig::fast_front_back_fatal())
            }
            Self::ShuntBot | Self::ShuntBotBoss => Some(RobotsCommonAiHitConfig::shunt_fatal()),
            Self::TurretBot => Some(RobotsCommonAiHitConfig::turretbot_fatal()),
            _ => None,
        }
    }

    pub const fn has_proven_common_fatal_node(self) -> bool {
        self.common_fatal_hit_config().is_some()
    }

    pub const fn hit_reaction_kind(self) -> RobotsAiHitReactionKind {
        match self {
            Self::Eb10RollerBot => RobotsAiHitReactionKind::RollerBot,
            Self::Ef01Mine => RobotsAiHitReactionKind::Ef01Mine,
            Self::Ep02Turret | Self::Ep04Turret | Self::Ep05Turret | Self::Ep06Turret => {
                RobotsAiHitReactionKind::TurretFamily
            }
            Self::Ew07Dodgem => RobotsAiHitReactionKind::Dodgem,
            Self::TurretBot => RobotsAiHitReactionKind::TurretBot,
            _ => RobotsAiHitReactionKind::Common,
        }
    }

    pub const fn native_name(self) -> &'static str {
        match self {
            Self::MonsterBase => "XItemHandler_Monster",
            Self::Monster2Rockets => "XItemHandler_Monster_2Rockets",
            Self::ConstructionBot => "XItemHandler_Monster_ConstructionBot",
            Self::DogBot => "XItemHandler_Monster_Dogbot",
            Self::Eb07MineBot => "XItemHandler_Monster_EB07_MineBot",
            Self::Eb10RollerBot => "XItemHandler_Monster_EB10_RollerBot",
            Self::Eb11MagnaBot => "XItemHandler_Monster_EB11_MagnaBot",
            Self::Eb12EvilBot => "XItemHandler_Monster_EB12_EvilBot",
            Self::Eb13KnightBot => "XItemHandler_Monster_EB13_KnightBot",
            Self::Eb14Minion => "XItemHandler_Monster_EB14_Minion",
            Self::Eb15Launcher => "XItemHandler_Monster_EB15_Launcher",
            Self::Eb16KnuckleBot => "XItemHandler_Monster_EB16_KnuckleBot",
            Self::Ef01Mine => "XItemHandler_Monster_EF01_Mine",
            Self::Ef03EvilBot => "XItemHandler_Monster_EF03_EvilBot",
            Self::Em07PiranhaBot => "XItemHandler_Monster_EM07_PiranhaBot",
            Self::Ep02Turret => "XItemHandler_Monster_EP02_Turret",
            Self::Ep04Turret => "XItemHandler_Monster_EP04_Turret",
            Self::Ep05Turret => "XItemHandler_Monster_EP05_Turret",
            Self::Ep06Turret => "XItemHandler_Monster_EP06_Turret",
            Self::Eq02MineBot => "XItemHandler_Monster_EQ02_MineBot",
            Self::Eq03Spider => "XItemHandler_Monster_EQ03_Spider",
            Self::Eq04Mine => "XItemHandler_Monster_EQ04_Mine",
            Self::Ew07Dodgem => "XItemHandler_Monster_EW07_Dodgem",
            Self::Ew08Flambe => "XItemHandler_Monster_EW08_Flambe",
            Self::Ew08FlambeLarge => "XItemHandler_Monster_EW08_FlambeLarge",
            Self::Ew09Armoured => "XItemHandler_Monster_EW09_Armoured",
            Self::Ew10Minion => "XItemHandler_Monster_EW10_Minion",
            Self::Ew11FatBot => "XItemHandler_Monster_EW11_FatBot",
            Self::GuardBot => "XItemHandler_Monster_GuardBot",
            Self::JailBotLarge => "XItemHandler_Monster_JailBotLarge",
            Self::JailBotNormal => "XItemHandler_Monster_JailBotNormal",
            Self::MalfBot => "XItemHandler_Monster_MalfBot",
            Self::SawBot => "XItemHandler_Monster_SawBot",
            Self::SecurityBot => "XItemHandler_Monster_SecurityBot",
            Self::ShieldBot => "XItemHandler_Monster_ShieldBot",
            Self::ShuntBot => "XItemHandler_Monster_ShuntBot",
            Self::ShuntBotBoss => "XItemHandler_Monster_ShuntBot_Boss",
            Self::SpikeBot => "XItemHandler_Monster_SpikeBot",
            Self::SpinTop => "XItemHandler_Monster_SpinTop",
            Self::Sweeper => "XItemHandler_Monster_Sweeper",
            Self::TestAnimBot => "XItemHandler_Monster_TestAnimBot",
            Self::ThiefBot => "XItemHandler_Monster_ThiefBot",
            Self::TurretBot => "XItemHandler_Monster_TurretBot",
            Self::Npc => "XItemHandler_Npc",
            Self::NpcFender => "XItemHandler_Npc_Fender",
        }
    }

    pub const fn descriptor_address(self) -> u32 {
        match self {
            Self::MonsterBase => 0x005E_260C,
            Self::Monster2Rockets => 0x005E_263C,
            Self::ConstructionBot => 0x005E_26CC,
            Self::DogBot => 0x005E_262C,
            Self::Eb07MineBot => 0x005E_270C,
            Self::Eb10RollerBot => 0x005E_27FC,
            Self::Eb11MagnaBot => 0x005E_27EC,
            Self::Eb12EvilBot => 0x005E_286C,
            Self::Eb13KnightBot => 0x005E_289C,
            Self::Eb14Minion => 0x005E_288C,
            Self::Eb15Launcher => 0x005E_28AC,
            Self::Eb16KnuckleBot => 0x005E_28BC,
            Self::Ef01Mine => 0x005E_283C,
            Self::Ef03EvilBot => 0x005E_28DC,
            Self::Em07PiranhaBot => 0x005E_28EC,
            Self::Ep02Turret => 0x005E_276C,
            Self::Ep04Turret => 0x005E_277C,
            Self::Ep05Turret => 0x005E_278C,
            Self::Ep06Turret => 0x005E_279C,
            Self::Eq02MineBot => 0x005E_273C,
            Self::Eq03Spider => 0x005E_285C,
            Self::Eq04Mine => 0x005E_282C,
            Self::Ew07Dodgem => 0x005E_27BC,
            Self::Ew08Flambe => 0x005E_280C,
            Self::Ew08FlambeLarge => 0x005E_281C,
            Self::Ew09Armoured => 0x005E_27CC,
            Self::Ew10Minion => 0x005E_287C,
            Self::Ew11FatBot => 0x005E_28CC,
            Self::GuardBot => 0x005E_26DC,
            Self::JailBotLarge => 0x005E_26BC,
            Self::JailBotNormal => 0x005E_26AC,
            Self::MalfBot => 0x005E_27AC,
            Self::SawBot => 0x005E_265C,
            Self::SecurityBot => 0x005E_26FC,
            Self::ShieldBot => 0x005E_26EC,
            Self::ShuntBot => 0x005E_266C,
            Self::ShuntBotBoss => 0x005E_267C,
            Self::SpikeBot => 0x005E_264C,
            Self::SpinTop => 0x005E_272C,
            Self::Sweeper => 0x005E_274C,
            Self::TestAnimBot => 0x005E_28FC,
            Self::ThiefBot => 0x005E_271C,
            Self::TurretBot => 0x005E_268C,
            Self::Npc => 0x005E_7024,
            Self::NpcFender => 0x005E_7034,
        }
    }

    pub const fn vtable_address(self) -> u32 {
        match self {
            Self::MonsterBase => 0x005E_2920,
            Self::Em07PiranhaBot => 0x005E_6A40,
            Self::TestAnimBot => 0x005E_6BA8,
            Self::Ef03EvilBot => 0x005E_68D8,
            Self::Ew11FatBot => 0x005E_6770,
            Self::Monster2Rockets => 0x005E_2D58,
            Self::ConstructionBot => 0x005E_3A00,
            Self::DogBot => 0x005E_2BF0,
            Self::Eb07MineBot => 0x005E_3FA0,
            Self::Eb10RollerBot => 0x005E_5520,
            Self::Eb11MagnaBot => 0x005E_53B8,
            Self::Eb12EvilBot => 0x005E_5F00,
            Self::Eb13KnightBot => 0x005E_6338,
            Self::Eb14Minion => 0x005E_61D0,
            Self::Eb15Launcher => 0x005E_64A0,
            Self::Eb16KnuckleBot => 0x005E_6608,
            Self::Ef01Mine => 0x005E_5AC8,
            Self::Ep02Turret => 0x005E_4828,
            Self::Ep04Turret => 0x005E_49A0,
            Self::Ep05Turret => 0x005E_4B18,
            Self::Ep06Turret => 0x005E_4C90,
            Self::Eq02MineBot => 0x005E_43E0,
            Self::Eq03Spider => 0x005E_5D98,
            Self::Eq04Mine => 0x005E_5960,
            Self::Ew07Dodgem => 0x005E_4F70,
            Self::Ew08Flambe => 0x005E_5690,
            Self::Ew08FlambeLarge => 0x005E_57F8,
            Self::Ew09Armoured => 0x005E_50E0,
            Self::Ew10Minion => 0x005E_6068,
            Self::GuardBot => 0x005E_3B68,
            Self::JailBotLarge => 0x005E_3898,
            Self::JailBotNormal => 0x005E_3730,
            Self::MalfBot => 0x005E_4E08,
            Self::SawBot => 0x005E_3028,
            Self::SecurityBot => 0x005E_3E38,
            Self::ShieldBot => 0x005E_3CD0,
            Self::ShuntBot => 0x005E_3190,
            Self::ShuntBotBoss => 0x005E_32F8,
            Self::SpikeBot => 0x005E_2EC0,
            Self::SpinTop => 0x005E_4270,
            Self::Sweeper => 0x005E_4548,
            Self::ThiefBot => 0x005E_4108,
            Self::TurretBot => 0x005E_3460,
            Self::Npc => 0x005E_7048,
            Self::NpcFender => 0x005E_71B8,
        }
    }
}

/// Native XTrigger_AI_Character serialized trigger-type bridge. The eight shipped
/// EXGeoTriggerType families map one-to-one onto MonsterDatabase runtime sheets 5..12.
/// Keep this engine-neutral: GUI, CLI corpus tools and the future UE adapter must not
/// maintain parallel copies of the same native table.
pub const fn ai_runtime_type_for_serialized_trigger_type(serialized_type: u32) -> Option<u32> {
    match serialized_type {
        10 => Some(5),
        11 => Some(6),
        18 => Some(7),
        33 => Some(8),
        74 => Some(9),
        3 => Some(10),
        48 => Some(11),
        70 => Some(12),
        _ => None,
    }
}

/// Native `0x0047EA70`. Valid runtime sheets are exactly 5..12. Unknown runtime
/// families fail closed here instead of reproducing the native infinite-loop assert.
pub const fn ai_handler_class_for_runtime_selector(
    runtime_type: u32,
    config_index: u32,
) -> Option<RobotsAiHandlerClass> {
    use RobotsAiHandlerClass as C;
    let class = match runtime_type {
        5 => match config_index {
            0 | 21 | 22 => C::DogBot,
            1 => C::SawBot,
            2 => C::ShuntBot,
            3 => C::JailBotNormal,
            4 => C::ConstructionBot,
            5 => C::SecurityBot,
            6 => C::Eq02MineBot,
            7 => C::MalfBot,
            8 => C::Ew09Armoured,
            9 => C::Eb10RollerBot,
            10 => C::Eq04Mine,
            11 => C::Ef01Mine,
            12 => C::Eb12EvilBot,
            13 => C::Ew10Minion,
            14 => C::Eb14Minion,
            15 => C::Eb13KnightBot,
            16 => C::Eb15Launcher,
            17 => C::Eb16KnuckleBot,
            18 => C::Ew11FatBot,
            19 => C::Ef03EvilBot,
            20 => C::ShuntBotBoss,
            _ => C::MonsterBase,
        },
        6 => match config_index {
            0 => C::Monster2Rockets,
            1 => C::TurretBot,
            2 => C::GuardBot,
            3 => C::ShieldBot,
            4 => C::Eb07MineBot,
            5 => C::Ep02Turret,
            6 => C::Ep04Turret,
            7 => C::Ep05Turret,
            8 => C::Ep06Turret,
            9 => C::Eb11MagnaBot,
            10 => C::Ew08Flambe,
            11 => C::Eq03Spider,
            _ => C::MonsterBase,
        },
        7 => match config_index {
            0 | 1 => C::SpikeBot,
            2 => C::SpinTop,
            3 => C::Ew07Dodgem,
            _ => C::MonsterBase,
        },
        8 => match config_index {
            0 => C::ThiefBot,
            1 => C::Sweeper,
            _ => C::MonsterBase,
        },
        9 => match config_index {
            0 => C::JailBotLarge,
            1 => C::Ef01Mine,
            2 => C::Ew08FlambeLarge,
            _ => C::MonsterBase,
        },
        10 => C::TestAnimBot,
        11 => {
            if config_index == 11 {
                C::NpcFender
            } else {
                C::Npc
            }
        }
        12 => {
            if config_index == 0 {
                C::Em07PiranhaBot
            } else {
                C::MonsterBase
            }
        }
        _ => return None,
    };
    Some(class)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_monster_player_proximity_matches_451bf0_threshold_smoothing_and_short_scale() {
        assert_eq!(
            ROBOTS_MONSTER_PLAYER_PROXIMITY_HORIZONTAL_RADIUS.to_bits(),
            5.0f32.to_bits()
        );
        assert_eq!(
            ROBOTS_MONSTER_PLAYER_PROXIMITY_VERTICAL_TOLERANCE.to_bits(),
            2.75f32.to_bits()
        );
        assert_eq!(
            ROBOTS_MONSTER_PLAYER_PROXIMITY_SMOOTHING.to_bits(),
            0.5f32.to_bits()
        );
        assert_eq!(
            ROBOTS_MONSTER_PLAYER_PROXIMITY_DISABLE_EPSILON.to_bits(),
            0.001f32.to_bits()
        );
        assert_eq!(
            ROBOTS_MONSTER_PLAYER_PROXIMITY_I16_SCALE.to_bits(),
            32767.0f32.to_bits()
        );

        let owner = [0.0, 0.0, 0.0];
        let mut state = RobotsMonsterPlayerProximityRuntimeState::default();
        let no_player = step_monster_player_proximity(&mut state, owner, None);
        assert_eq!(no_player.xitem_service_bit4_enabled, None);
        assert_eq!(state.factor, 1.0);

        let boundary = step_monster_player_proximity(&mut state, owner, Some([5.0, 2.75, 0.0]));
        assert_eq!(boundary.factor, 1.0);
        assert_eq!(boundary.owner_scale_i16, Some(32767));
        assert_eq!(boundary.xitem_service_bit4_enabled, Some(true));
        assert!(boundary.set_owner_flag_65_bit1);

        let far = [6.0, 3.0, 0.0];
        let first_far = step_monster_player_proximity(&mut state, owner, Some(far));
        assert_eq!(first_far.factor, 0.5);
        assert_eq!(first_far.owner_scale_i16, Some(16383));
        for _ in 0..8 {
            let step = step_monster_player_proximity(&mut state, owner, Some(far));
            assert_eq!(step.xitem_service_bit4_enabled, Some(true));
        }
        let disabled = step_monster_player_proximity(&mut state, owner, Some(far));
        assert_eq!(disabled.factor, 0.0009765625);
        assert_eq!(disabled.xitem_service_bit4_enabled, Some(false));
        assert_eq!(disabled.owner_scale_i16, None);
        assert!(!disabled.set_owner_flag_65_bit1);
        assert_eq!(state.owner_scale_i16, 63);
        assert!(state.owner_flag_65_bit1);
    }

    #[test]
    fn common_monster_attack_gate_matches_native_cooldown_and_player_reaction_veto() {
        let mut cooldown = RobotsMonsterAttackCooldownRuntime::default();
        assert!(robots_common_monster_attack_allowed(
            0, None, 2, true, 0.0, cooldown
        ));

        cooldown.record_attack();
        assert!(!robots_common_monster_attack_allowed(
            0, None, 2, true, 0.0, cooldown
        ));
        cooldown.advance(ROBOTS_MONSTER_ATTACK_COOLDOWN_SECONDS);
        assert!(robots_common_monster_attack_allowed(
            0, None, 2, true, 0.0, cooldown
        ));
        assert!(!robots_common_monster_attack_allowed(
            0, None, 2, true, 5.0, cooldown
        ));
        assert!(robots_player_reaction_gate_active(None, 2, true, 5.0));
        assert!(!robots_player_reaction_gate_active(Some(1), 2, true, 5.0));
        assert!(!robots_player_reaction_gate_active(None, 0x1d, true, 5.0));
        assert!(robots_common_monster_attack_allowed(
            0,
            Some(1),
            2,
            true,
            5.0,
            cooldown
        ));
        assert!(robots_common_monster_attack_allowed(
            0, None, 0x1d, true, 5.0, cooldown
        ));
        assert!(robots_common_monster_attack_allowed(
            ROBOTS_MONSTER_ATTACK_BYPASS_FLAG,
            None,
            2,
            true,
            5.0,
            RobotsMonsterAttackCooldownRuntime::default(),
        ));
    }

    #[test]
    fn ai_patrol_malfbot_config_matches_native_five_second_half_turn_cycle() {
        let config = RobotsAiPatrolConfig {
            base_yaw_radians: 0.0,
            interval_seconds: 5,
            target_locomotion_scalar: 0.0,
            turn_rate: crate::robots_runtime::locomotion::RobotsAiTurnRateInput::Default,
        };
        let mut state = RobotsAiPatrolRuntimeState::configured(config);
        assert_eq!(config.interval_ticks(), 300);
        assert_eq!(ai_patrol_priority(None), ROBOTS_AI_PATROL_PRIORITY);
        assert_eq!(
            ai_patrol_priority(Some(1)),
            ROBOTS_AI_PATROL_BLOCKED_PRIORITY
        );
        assert_eq!(
            ai_patrol_priority(Some(0x0f)),
            ROBOTS_AI_PATROL_BLOCKED_PRIORITY
        );

        for expected in (0..300).rev() {
            let step = step_ai_patrol(&mut state, config);
            assert_eq!(step.steering_target_yaw_radians, 0.0);
            assert_eq!(state.countdown_ticks, expected);
        }
        let step = step_ai_patrol(&mut state, config);
        assert!((step.steering_target_yaw_radians - std::f32::consts::PI).abs() < 1.0e-6);
        assert_eq!(state.countdown_ticks, 299);
        assert!(state.alternate_heading);
    }

    #[test]
    fn serialized_ai_trigger_types_map_exactly_to_native_runtime_sheets() {
        assert_eq!(ai_runtime_type_for_serialized_trigger_type(10), Some(5));
        assert_eq!(ai_runtime_type_for_serialized_trigger_type(11), Some(6));
        assert_eq!(ai_runtime_type_for_serialized_trigger_type(18), Some(7));
        assert_eq!(ai_runtime_type_for_serialized_trigger_type(33), Some(8));
        assert_eq!(ai_runtime_type_for_serialized_trigger_type(74), Some(9));
        assert_eq!(ai_runtime_type_for_serialized_trigger_type(3), Some(10));
        assert_eq!(ai_runtime_type_for_serialized_trigger_type(48), Some(11));
        assert_eq!(ai_runtime_type_for_serialized_trigger_type(70), Some(12));
        assert_eq!(ai_runtime_type_for_serialized_trigger_type(47), None);
        assert_eq!(ai_runtime_type_for_serialized_trigger_type(75), None);
    }

    #[test]
    fn native_ai_handler_dispatch_covers_all_shipped_special_cases() {
        use RobotsAiHandlerClass as C;
        assert_eq!(ai_handler_class_for_runtime_selector(5, 1), Some(C::SawBot));
        assert_eq!(
            ai_handler_class_for_runtime_selector(5, 6),
            Some(C::Eq02MineBot)
        );
        assert_eq!(
            ai_handler_class_for_runtime_selector(5, 9),
            Some(C::Eb10RollerBot)
        );
        assert_eq!(
            ai_handler_class_for_runtime_selector(6, 4),
            Some(C::Eb07MineBot)
        );
        assert_eq!(
            ai_handler_class_for_runtime_selector(7, 2),
            Some(C::SpinTop)
        );
        assert_eq!(
            ai_handler_class_for_runtime_selector(8, 1),
            Some(C::Sweeper)
        );
        assert_eq!(
            ai_handler_class_for_runtime_selector(9, 2),
            Some(C::Ew08FlambeLarge)
        );
        assert_eq!(
            ai_handler_class_for_runtime_selector(10, 999),
            Some(C::TestAnimBot)
        );
        assert_eq!(
            ai_handler_class_for_runtime_selector(11, 11),
            Some(C::NpcFender)
        );
        assert_eq!(ai_handler_class_for_runtime_selector(11, 10), Some(C::Npc));
        assert_eq!(
            ai_handler_class_for_runtime_selector(12, 0),
            Some(C::Em07PiranhaBot)
        );
        assert_eq!(
            ai_handler_class_for_runtime_selector(12, 1),
            Some(C::MonsterBase)
        );
        assert_eq!(ai_handler_class_for_runtime_selector(4, 0), None);
        assert_eq!(C::Eq02MineBot.descriptor_address(), 0x005E_273C);
        assert_eq!(C::Eq02MineBot.vtable_address(), 0x005E_43E0);
        assert_eq!(
            C::DogBot.hit_reaction_kind(),
            RobotsAiHitReactionKind::Common
        );
        assert_eq!(
            C::MalfBot.hit_reaction_kind(),
            RobotsAiHitReactionKind::Common
        );
        assert_eq!(
            C::SawBot.common_fatal_hit_config(),
            Some(RobotsCommonAiHitConfig::sawbot_fatal())
        );
        assert_eq!(
            C::JailBotNormal.common_fatal_hit_config(),
            Some(RobotsCommonAiHitConfig::fast_front_back_fatal())
        );
        assert_eq!(
            C::JailBotLarge.common_fatal_hit_config(),
            Some(RobotsCommonAiHitConfig::fast_front_back_fatal())
        );
        assert_eq!(
            C::ShuntBot.common_fatal_hit_config(),
            Some(RobotsCommonAiHitConfig::shunt_fatal())
        );
        assert_eq!(
            C::ShuntBotBoss.common_fatal_hit_config(),
            Some(RobotsCommonAiHitConfig::shunt_fatal())
        );
        assert_eq!(
            C::Eb10RollerBot.hit_reaction_kind(),
            RobotsAiHitReactionKind::RollerBot
        );
        assert_eq!(
            C::Ef01Mine.hit_reaction_kind(),
            RobotsAiHitReactionKind::Ef01Mine
        );
        assert_eq!(
            C::Ep04Turret.hit_reaction_kind(),
            RobotsAiHitReactionKind::TurretFamily
        );
        assert_eq!(
            C::Ew07Dodgem.hit_reaction_kind(),
            RobotsAiHitReactionKind::Dodgem
        );
        assert_eq!(
            C::Ew07Dodgem.common_fatal_hit_config(),
            Some(RobotsCommonAiHitConfig::dodgem_fatal())
        );
        assert_eq!(
            C::TurretBot.hit_reaction_kind(),
            RobotsAiHitReactionKind::TurretBot
        );
    }
}
