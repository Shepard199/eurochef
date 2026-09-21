use serde::Serialize;

use crate::script::{UXGeoScriptCommand, UXGeoScriptCommandData};

use super::hit_query::RobotsHitQueryInitPlan;

pub mod event_type {
    pub const SETUP_IDLE: u32 = 0x1600_0001;
    pub const CHANGE_ANIM_MODE: u32 = 0x1600_0004;
    pub const FORCE_IDLE_MODE: u32 = 0x1600_0006;
    pub const SET_SCRIPT_VALUE: u32 = 0x1600_0007;
    pub const HIT_CHECK: u32 = 0x1600_000A;
    pub const WAIT: u32 = 0x1600_000B;
    pub const WAIT_FOR_MESSAGE: u32 = 0x1600_000C;
    pub const STATE_MARKER: u32 = 0x1600_000D;
    pub const SET_ALTERNATE_STATE: u32 = 0x1600_000E;
    pub const OPEN: u32 = 0x1600_0011;
    pub const CLOSE: u32 = 0x1600_0012;
    pub const OPENED: u32 = 0x1600_0013;
    pub const CLOSED: u32 = 0x1600_0014;
    pub const WAIT_FOR_ITEM_LAND_ON: u32 = 0x1600_000F;
    pub const FILE_LOAD: u32 = 0x1600_0015;
    pub const FILE_DELOAD: u32 = 0x1600_0016;
    pub const MESSAGE_RELAY: u32 = 0x1600_0018;
    pub const WAIT_FOR_ITEM_OFF: u32 = 0x1600_0019;
    pub const MESSAGE_RELAY_SPECIFIC: u32 = 0x1600_001A;
    pub const CLEAR_STATE_MARKER: u32 = 0x1600_001B;
    pub const WAIT_FOR_HIT: u32 = 0x1600_001D;
    pub const SET_WEAPON_VALUE: u32 = 0x1600_001E;
    pub const CREATE_PROJECTILE: u32 = 0x1600_001F;
    pub const GENERATE_PICKUP: u32 = 0x1600_0020;
    pub const ATTACH_SWOOSH: u32 = 0x1600_0021;
    pub const ATTACH_PARTICLE: u32 = 0x1600_0022;
    pub const ATTACH_PARTICLES_TO_SKELETON: u32 = 0x1600_0023;
    pub const SWAP_CHARACTER: u32 = 0x1600_0024;
    pub const ATTACH_SPECIAL_EFFECT: u32 = 0x1600_0025;
    pub const CREATE_EXPLOSION: u32 = 0x1600_0026;
    pub const SHOW_MESSAGE: u32 = 0x1600_0027;
    pub const PLAYER_ATTACK_BRANCH: u32 = 0x1600_0028;
    pub const WAIT_FOR_TEXT: u32 = 0x1600_0029;
    pub const EXIT_SCRIPT: u32 = 0x1600_002A;
    pub const POSITION_CHARACTER: u32 = 0x1600_002B;
    pub const WAIT_FOR_MESSAGE_SPECIFIC: u32 = 0x1600_002C;
    pub const INVENTORY_ADD: u32 = 0x1600_002E;
    pub const INVENTORY_REMOVE: u32 = 0x1600_002F;
    pub const MISSION_UPDATE: u32 = 0x1600_0030;
    pub const CHANGE_SCRIPT: u32 = 0x1600_0031;
    pub const INVENTORY_CHECK: u32 = 0x1600_0032;
    pub const TEXTURE_CONTROL: u32 = 0x1600_0033;
    pub const WAIT_FOR_STATE: u32 = 0x1600_0034;
    pub const FOOTSTEP: u32 = 0x1600_0035;
    pub const SET_PROPERTIES: u32 = 0x1600_0037;
    pub const SHAKE_CAMERA: u32 = 0x1600_0038;
    pub const MONSTER_EXPLOSION: u32 = 0x1600_0039;
    pub const SET_PROPERTIES_CUTSCENE: u32 = 0x1600_003A;
    pub const SOUND_STOP: u32 = 0x1600_003B;
    pub const MISSION_CHECK: u32 = 0x1600_003C;
    pub const ANIMATION_CONTROL: u32 = 0x1600_003D;
    pub const PLAY_TO_STATE_MARKER: u32 = 0x1600_003E;
    pub const DETTACH_SWOOSH: u32 = 0x1600_003F;
    pub const SET_PROPERTIES_PLAYER: u32 = 0x1600_0040;
    pub const ATTACH_BEAM: u32 = 0x1600_0043;
    pub const DETACH_BEAM: u32 = 0x1600_0044;
    pub const PERFORM_ACTION_CUTSCENE: u32 = 0x1600_0045;
    pub const PARTICLE_HITCHECK: u32 = 0x1600_0046;
    pub const WAIT_FOR_HIT_MISSILES: u32 = 0x1600_0047;
    pub const FILE_LOAD_SUBFILE: u32 = 0x1600_0048;
    pub const FILE_DELOAD_SUBFILE: u32 = 0x1600_0049;
    pub const STATE_MARKER_END: u32 = 0x1600_004A;
    pub const INTERNAL_PROJECTILE_SENTINEL: u32 = 0xFFFF_FFFF;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsHandlerEventFamily {
    Base,
    Projectile,
    AiCharacter,
    Camera,
    MagnaBot,
    WatchBot,
    Player,
    FirstPersonPlayer,
    BossExecBomb,
    BossSewer,
    SweeperBoss,
    SweeperRatchet,
    FinalBossHealth,
}

impl RobotsHandlerEventFamily {
    pub const fn from_native_target(target: u32) -> Option<Self> {
        Some(match target {
            0x0040_2ED0 => Self::Base,
            0x0041_5460 => Self::Projectile,
            0x0045_3160 => Self::AiCharacter,
            0x0045_6EF0 => Self::Camera,
            0x0046_4EA0 => Self::MagnaBot,
            0x0049_0B40 => Self::WatchBot,
            0x004A_F9C0 => Self::Player,
            0x004B_6F50 => Self::FirstPersonPlayer,
            0x004C_AA30 => Self::BossExecBomb,
            0x004C_BCE0 => Self::BossSewer,
            0x004C_E940 => Self::SweeperBoss,
            0x004C_FB00 => Self::SweeperRatchet,
            0x004D_0480 => Self::FinalBossHealth,
            _ => return None,
        })
    }

    pub const fn native_target(self) -> u32 {
        match self {
            Self::Base => 0x0040_2ED0,
            Self::Projectile => 0x0041_5460,
            Self::AiCharacter => 0x0045_3160,
            Self::Camera => 0x0045_6EF0,
            Self::MagnaBot => 0x0046_4EA0,
            Self::WatchBot => 0x0049_0B40,
            Self::Player => 0x004A_F9C0,
            Self::FirstPersonPlayer => 0x004B_6F50,
            Self::BossExecBomb => 0x004C_AA30,
            Self::BossSewer => 0x004C_BCE0,
            Self::SweeperBoss => 0x004C_E940,
            Self::SweeperRatchet => 0x004C_FB00,
            Self::FinalBossHealth => 0x004D_0480,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsHandlerScriptCommandFamily {
    Generic,
    Cutscene,
    Door,
    MagneticScript,
    ArcadeShop,
    Hazard,
    Interactive,
    Light,
    Pickup,
    ScriptLifecycle,
    BossExec,
    BossSewerCanon,
}

impl RobotsHandlerScriptCommandFamily {
    pub const fn from_native_target(target: u32) -> Option<Self> {
        Some(match target {
            0x0040_2EF0 => Self::Generic,
            0x0040_7E40 => Self::Cutscene,
            0x0040_CE20 => Self::Door,
            0x0040_ED80 => Self::MagneticScript,
            0x0041_09F0 => Self::ArcadeShop,
            0x0041_13A0 => Self::Hazard,
            0x0041_25C0 => Self::Interactive,
            0x0041_3880 => Self::Light,
            0x0041_3B80 => Self::Pickup,
            0x0041_6D10 => Self::ScriptLifecycle,
            0x004C_9440 => Self::BossExec,
            0x004C_C510 => Self::BossSewerCanon,
            _ => return None,
        })
    }

    pub const fn native_target(self) -> u32 {
        match self {
            Self::Generic => 0x0040_2EF0,
            Self::Cutscene => 0x0040_7E40,
            Self::Door => 0x0040_CE20,
            Self::MagneticScript => 0x0040_ED80,
            Self::ArcadeShop => 0x0041_09F0,
            Self::Hazard => 0x0041_13A0,
            Self::Interactive => 0x0041_25C0,
            Self::Light => 0x0041_3880,
            Self::Pickup => 0x0041_3B80,
            Self::ScriptLifecycle => 0x0041_6D10,
            Self::BossExec => 0x004C_9440,
            Self::BossSewerCanon => 0x004C_C510,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RobotsScriptEventView<'a> {
    pub event_type: u32,
    pub data: &'a [u8],
    pub start: Option<i16>,
    pub length: Option<u16>,
}

impl<'a> RobotsScriptEventView<'a> {
    pub fn from_script_data(data: &'a UXGeoScriptCommandData) -> Option<Self> {
        match data {
            UXGeoScriptCommandData::Event { event_type, data } => Some(Self {
                event_type: *event_type,
                data,
                start: None,
                length: None,
            }),
            _ => None,
        }
    }

    pub fn from_command(command: &'a UXGeoScriptCommand) -> Option<Self> {
        match &command.data {
            UXGeoScriptCommandData::Event { event_type, data } => Some(Self {
                event_type: *event_type,
                data,
                start: Some(command.start),
                length: Some(command.length),
            }),
            _ => None,
        }
    }

    pub fn word(self, index: usize) -> Option<u32> {
        let offset = index.checked_mul(4)?;
        let bytes: [u8; 4] = self.data.get(offset..offset + 4)?.try_into().ok()?;
        Some(u32::from_le_bytes(bytes))
    }

    pub fn float(self, index: usize) -> Option<f32> {
        self.word(index).map(f32::from_bits)
    }

    pub fn words<const N: usize>(self) -> [Option<u32>; N] {
        std::array::from_fn(|index| self.word(index))
    }

    pub fn data_byte(self, offset: usize) -> Option<u8> {
        self.data.get(offset).copied()
    }

    /// First native Handler Event argument is command+0x14. `Event.data`
    /// starts at command+0x10 because command+0x0C (event_type) is stripped by
    /// `UXGeoScriptCommandData::Event`, so native arg0 is raw Event word 1.
    pub fn native_arg_word(self, index: usize) -> Option<u32> {
        self.word(index.checked_add(1)?)
    }

    pub fn native_arg_float(self, index: usize) -> Option<f32> {
        self.native_arg_word(index).map(f32::from_bits)
    }

    pub fn native_args<const N: usize>(self) -> [Option<u32>; N] {
        std::array::from_fn(|index| self.native_arg_word(index))
    }

    /// Byte offset relative to native command+0x14 (arg0), not relative to the
    /// start of `Event.data`.
    pub fn native_arg_byte(self, offset: usize) -> Option<u8> {
        self.data_byte(offset.checked_add(4)?)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsHandlerEventPostDispatch {
    None,
    AiEmbeddedChannels,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RobotsHandlerEventSemantic {
    DelegateCommandHandler,
    NoOp,
    Unhandled,
    HandledNoEffect,
    DelegateFamily {
        family: RobotsHandlerEventFamily,
    },
    ForwardCurrentWatchBotComponent,
    ProjectileOwnerScriptSentinel,
    SetupIdle,
    ChangeAnimMode {
        anim_mode: Option<u32>,
    },
    ForceIdleMode,
    SetScriptValue {
        value: Option<f32>,
    },
    HitCheck {
        query: Option<RobotsHitQueryInitPlan>,
    },
    SetWeaponValue,
    CreateProjectile {
        /// Native Event arguments at command+0x14..+0x20. Keeping the raw words
        /// here lets UE/GUI projectile hosts resolve class-specific missile tables
        /// without reparsing serialized command bytes.
        args: [Option<u32>; 4],
    },
    AttachSwoosh {
        arg0: Option<u32>,
        arg1: Option<u32>,
    },
    AttachParticle {
        arg0: Option<u32>,
        arg1: Option<u32>,
    },
    AttachParticlesToSkeleton {
        args: [Option<u32>; 4],
    },
    CreateExplosion,
    PlayerAttackBranch {
        threshold: Option<f32>,
        anim_mode: Option<u32>,
    },
    WaitForState {
        state: Option<u32>,
    },
    WaitForHit,
    InventoryAdd,
    Footstep {
        arg0: Option<u32>,
    },
    DettachSwoosh {
        arg0: Option<u32>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsHandlerEventReturnPolicy {
    Zero,
    One,
    Delegate,
    HostResult,
    StateDependent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsInventoryComparison {
    GreaterOrEqual,
    Greater,
    Equal,
    Less,
    LessOrEqual,
}

impl RobotsInventoryComparison {
    pub const fn from_native_mode(mode: u8) -> Self {
        match mode {
            1 => Self::Greater,
            2 => Self::Equal,
            3 => Self::Less,
            4 => Self::LessOrEqual,
            _ => Self::GreaterOrEqual,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsCutsceneCommandKind {
    StateMarker,
    SwapCharacter,
    ShowMessage,
    WaitForText,
    ExitScript,
    PositionCharacter,
    SetProperties,
    SetPropertiesCutscene,
    MissionCheck,
    AnimationControl,
    SetPropertiesPlayer,
    PerformActionCutscene,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsDoorCommandKind {
    Open,
    Close,
    Opened,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsSpecializedScriptCommandKind {
    MagneticStateMarker,
    ArcadeWaitForState,
    HazardHitCheck,
    HazardSetAlternateState,
    HazardWaitForHit,
    LightSetAlternateState,
    LightClearStateMarker,
    PickupExitScript,
    BossExecSetupIdle,
    BossExecCreateProjectile,
    BossExecWaitForState,
    BossSewerCanonCreateProjectile,
    BossSewerCanonWaitForState,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RobotsHandlerScriptCommandSemantic {
    DelegateFamily {
        family: RobotsHandlerScriptCommandFamily,
    },
    CutsceneCommand {
        command: RobotsCutsceneCommandKind,
        args: [Option<u32>; 8],
        start: Option<i16>,
        length: Option<u16>,
    },
    DoorCommand {
        command: RobotsDoorCommandKind,
    },
    SpecializedCommand {
        command: RobotsSpecializedScriptCommandKind,
        args: [Option<u32>; 4],
        start: Option<i16>,
        hit_query: Option<RobotsHitQueryInitPlan>,
    },
    StateMarker {
        marker: Option<u32>,
        start: Option<i16>,
    },
    SetAlternateState,
    ClearStateMarker,
    WaitForHit {
        mask: Option<u32>,
    },
    ChangeScript {
        script: Option<u32>,
        owner_selector: Option<u32>,
    },
    InventoryCheck {
        owner_selector: Option<u32>,
        item: Option<u32>,
        quantity: Option<f32>,
        fallback_item: Option<u32>,
        on_true: [Option<u32>; 2],
        on_false: [Option<u32>; 2],
        comparison: Option<RobotsInventoryComparison>,
    },
    TextureControl {
        mode: Option<u32>,
        target: Option<u32>,
    },
    SoundStop {
        sound: Option<u32>,
    },
    PlayToStateMarker {
        owner_selector: Option<u32>,
        marker: Option<u32>,
    },
    WaitForHitMissiles {
        mask: Option<u32>,
    },
    StateMarkerEnd,
    Wait,
    WaitForMessage,
    WaitForItemLandOn,
    WaitForItemOff,
    WaitForMessageSpecific,
    FileLoad {
        resources: [Option<u32>; 8],
    },
    FileDeload {
        resources: [Option<u32>; 8],
    },
    MessageRelay,
    MessageRelaySpecific {
        arg0: Option<u32>,
        arg1: Option<u32>,
    },
    GeneratePickup {
        pickup: Option<u32>,
        quantity: Option<f32>,
    },
    AttachSwoosh {
        arg0: Option<u32>,
        arg1: Option<u32>,
    },
    AttachParticle {
        arg0: Option<u32>,
        arg1: Option<u32>,
    },
    AttachParticlesToSkeleton {
        args: [Option<u32>; 4],
    },
    AttachSpecialEffect {
        effect: Option<u32>,
    },
    CreateExplosion {
        arg0: Option<u32>,
    },
    ShowMessage {
        message: Option<u32>,
        value: Option<f32>,
    },
    ExitScript,
    InventoryAdd {
        owner_selector: Option<u32>,
        item: Option<u32>,
        quantity: Option<f32>,
        fallback_item: Option<u32>,
        mode: Option<u32>,
    },
    InventoryRemove {
        owner_selector: Option<u32>,
        item: Option<u32>,
        quantity: Option<f32>,
    },
    MissionUpdate {
        mission: Option<u32>,
        mode: Option<u32>,
        flags: Option<u32>,
    },
    ShakeCamera {
        arg0: Option<f32>,
        arg1: Option<f32>,
        flags: Option<u32>,
    },
    DettachSwoosh {
        arg0: Option<u32>,
    },
    FileLoadSubfile {
        entries: [Option<u32>; 8],
    },
    FileDeloadSubfile {
        entries: [Option<u32>; 8],
    },
    Unhandled,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RobotsHandlerScriptCommandPlan {
    pub semantic: RobotsHandlerScriptCommandSemantic,
    pub return_policy: RobotsHandlerEventReturnPolicy,
}

pub fn classify_generic_handler_script_command(
    event: RobotsScriptEventView<'_>,
) -> RobotsHandlerScriptCommandPlan {
    use event_type::*;
    use RobotsHandlerEventReturnPolicy as Return;
    use RobotsHandlerScriptCommandSemantic as Semantic;

    let semantic = match event.event_type {
        WAIT => Semantic::Wait,
        WAIT_FOR_MESSAGE => Semantic::WaitForMessage,
        WAIT_FOR_ITEM_LAND_ON => Semantic::WaitForItemLandOn,
        WAIT_FOR_ITEM_OFF => Semantic::WaitForItemOff,
        WAIT_FOR_MESSAGE_SPECIFIC => Semantic::WaitForMessageSpecific,
        FILE_LOAD => Semantic::FileLoad {
            resources: event.native_args(),
        },
        FILE_DELOAD => Semantic::FileDeload {
            resources: event.native_args(),
        },
        MESSAGE_RELAY => Semantic::MessageRelay,
        MESSAGE_RELAY_SPECIFIC => Semantic::MessageRelaySpecific {
            arg0: event.native_arg_word(0),
            arg1: event.native_arg_word(1),
        },
        GENERATE_PICKUP => Semantic::GeneratePickup {
            pickup: event.native_arg_word(0),
            quantity: event.native_arg_float(1),
        },
        ATTACH_SWOOSH => Semantic::AttachSwoosh {
            arg0: event.native_arg_word(0),
            arg1: event.native_arg_word(1),
        },
        ATTACH_PARTICLE => Semantic::AttachParticle {
            arg0: event.native_arg_word(0),
            arg1: event.native_arg_word(1),
        },
        ATTACH_PARTICLES_TO_SKELETON => Semantic::AttachParticlesToSkeleton {
            args: event.native_args(),
        },
        ATTACH_SPECIAL_EFFECT => Semantic::AttachSpecialEffect {
            effect: event.native_arg_word(0),
        },
        CREATE_EXPLOSION => Semantic::CreateExplosion {
            arg0: event.native_arg_word(0),
        },
        SHOW_MESSAGE => Semantic::ShowMessage {
            message: event.native_arg_word(0),
            value: event.native_arg_float(1),
        },
        EXIT_SCRIPT => Semantic::ExitScript,
        INVENTORY_ADD => Semantic::InventoryAdd {
            owner_selector: event.native_arg_word(0),
            item: event.native_arg_word(1),
            quantity: event.native_arg_float(2),
            fallback_item: event.native_arg_word(3),
            mode: event.native_arg_word(4),
        },
        INVENTORY_REMOVE => Semantic::InventoryRemove {
            owner_selector: event.native_arg_word(0),
            item: event.native_arg_word(1),
            quantity: event.native_arg_float(2),
        },
        MISSION_UPDATE => Semantic::MissionUpdate {
            mission: event.native_arg_word(0),
            mode: event.native_arg_word(1),
            flags: event.native_arg_word(2),
        },
        SHAKE_CAMERA => Semantic::ShakeCamera {
            arg0: event.native_arg_float(0),
            arg1: event.native_arg_float(1),
            flags: event.native_arg_word(2),
        },
        DETTACH_SWOOSH => Semantic::DettachSwoosh {
            arg0: event.native_arg_word(0),
        },
        FILE_LOAD_SUBFILE => Semantic::FileLoadSubfile {
            entries: event.native_args(),
        },
        FILE_DELOAD_SUBFILE => Semantic::FileDeloadSubfile {
            entries: event.native_args(),
        },
        _ => Semantic::Unhandled,
    };

    let return_policy = match event.event_type {
        WAIT
        | WAIT_FOR_MESSAGE
        | WAIT_FOR_ITEM_LAND_ON
        | WAIT_FOR_ITEM_OFF
        | WAIT_FOR_MESSAGE_SPECIFIC => Return::HostResult,
        EXIT_SCRIPT => Return::One,
        _ => Return::Zero,
    };

    RobotsHandlerScriptCommandPlan {
        semantic,
        return_policy,
    }
}

pub fn classify_door_handler_script_command(
    event: RobotsScriptEventView<'_>,
) -> RobotsHandlerScriptCommandPlan {
    use event_type::*;
    use RobotsDoorCommandKind as Command;
    use RobotsHandlerEventReturnPolicy as Return;
    use RobotsHandlerScriptCommandFamily as Family;
    use RobotsHandlerScriptCommandSemantic as Semantic;

    let command = match event.event_type {
        OPEN => Some(Command::Open),
        CLOSE => Some(Command::Close),
        OPENED => Some(Command::Opened),
        CLOSED => Some(Command::Closed),
        _ => None,
    };
    let semantic = match command {
        Some(command) => Semantic::DoorCommand { command },
        None => Semantic::DelegateFamily {
            family: Family::ScriptLifecycle,
        },
    };
    let return_policy = match event.event_type {
        OPEN | CLOSE => Return::StateDependent,
        OPENED | CLOSED => Return::Zero,
        _ => Return::Delegate,
    };
    RobotsHandlerScriptCommandPlan {
        semantic,
        return_policy,
    }
}

pub fn classify_cutscene_handler_script_command(
    event: RobotsScriptEventView<'_>,
) -> RobotsHandlerScriptCommandPlan {
    use event_type::*;
    use RobotsCutsceneCommandKind as Command;
    use RobotsHandlerEventReturnPolicy as Return;
    use RobotsHandlerScriptCommandFamily as Family;
    use RobotsHandlerScriptCommandSemantic as Semantic;

    let command = match event.event_type {
        STATE_MARKER => Some(Command::StateMarker),
        SWAP_CHARACTER => Some(Command::SwapCharacter),
        SHOW_MESSAGE => Some(Command::ShowMessage),
        WAIT_FOR_TEXT => Some(Command::WaitForText),
        EXIT_SCRIPT | u32::MAX => Some(Command::ExitScript),
        POSITION_CHARACTER => Some(Command::PositionCharacter),
        SET_PROPERTIES => Some(Command::SetProperties),
        SET_PROPERTIES_CUTSCENE => Some(Command::SetPropertiesCutscene),
        MISSION_CHECK => Some(Command::MissionCheck),
        ANIMATION_CONTROL => Some(Command::AnimationControl),
        SET_PROPERTIES_PLAYER => Some(Command::SetPropertiesPlayer),
        PERFORM_ACTION_CUTSCENE => Some(Command::PerformActionCutscene),
        _ => None,
    };

    let semantic = match command {
        Some(command) => Semantic::CutsceneCommand {
            command,
            args: event.native_args(),
            start: event.start,
            length: event.length,
        },
        None => Semantic::DelegateFamily {
            family: Family::ScriptLifecycle,
        },
    };
    let return_policy = match event.event_type {
        STATE_MARKER => Return::StateDependent,
        EXIT_SCRIPT | u32::MAX => Return::One,
        _ if command.is_some() => Return::Zero,
        _ => Return::Delegate,
    };

    RobotsHandlerScriptCommandPlan {
        semantic,
        return_policy,
    }
}

fn specialized_plan(
    command: RobotsSpecializedScriptCommandKind,
    event: RobotsScriptEventView<'_>,
    return_policy: RobotsHandlerEventReturnPolicy,
) -> RobotsHandlerScriptCommandPlan {
    RobotsHandlerScriptCommandPlan {
        semantic: RobotsHandlerScriptCommandSemantic::SpecializedCommand {
            command,
            args: event.native_args(),
            start: event.start,
            hit_query: (command == RobotsSpecializedScriptCommandKind::HazardHitCheck)
                .then(|| RobotsHitQueryInitPlan::from_event(event))
                .flatten(),
        },
        return_policy,
    }
}

fn delegate_script_command_family(
    family: RobotsHandlerScriptCommandFamily,
) -> RobotsHandlerScriptCommandPlan {
    RobotsHandlerScriptCommandPlan {
        semantic: RobotsHandlerScriptCommandSemantic::DelegateFamily { family },
        return_policy: RobotsHandlerEventReturnPolicy::Delegate,
    }
}

pub fn classify_magnetic_script_handler_script_command(
    event: RobotsScriptEventView<'_>,
) -> RobotsHandlerScriptCommandPlan {
    use RobotsHandlerEventReturnPolicy as Return;
    use RobotsHandlerScriptCommandFamily as Family;
    use RobotsSpecializedScriptCommandKind as Command;

    if event.event_type == event_type::STATE_MARKER {
        specialized_plan(Command::MagneticStateMarker, event, Return::Zero)
    } else {
        delegate_script_command_family(Family::ScriptLifecycle)
    }
}

pub fn classify_arcade_shop_handler_script_command(
    event: RobotsScriptEventView<'_>,
) -> RobotsHandlerScriptCommandPlan {
    use RobotsHandlerEventReturnPolicy as Return;
    use RobotsHandlerScriptCommandFamily as Family;
    use RobotsSpecializedScriptCommandKind as Command;

    if event.event_type == event_type::WAIT_FOR_STATE
        && matches!(event.native_arg_word(0), Some(3 | 4))
    {
        specialized_plan(Command::ArcadeWaitForState, event, Return::StateDependent)
    } else {
        delegate_script_command_family(Family::Interactive)
    }
}

pub fn classify_hazard_handler_script_command(
    event: RobotsScriptEventView<'_>,
) -> RobotsHandlerScriptCommandPlan {
    use RobotsHandlerEventReturnPolicy as Return;
    use RobotsHandlerScriptCommandFamily as Family;
    use RobotsSpecializedScriptCommandKind as Command;

    match event.event_type {
        event_type::HIT_CHECK => specialized_plan(Command::HazardHitCheck, event, Return::Zero),
        event_type::SET_ALTERNATE_STATE => {
            specialized_plan(Command::HazardSetAlternateState, event, Return::Zero)
        }
        // Hazard performs its own pre-hook before ScriptLifecycle WaitForHit.
        event_type::WAIT_FOR_HIT => {
            specialized_plan(Command::HazardWaitForHit, event, Return::StateDependent)
        }
        _ => delegate_script_command_family(Family::ScriptLifecycle),
    }
}

pub fn classify_light_handler_script_command(
    event: RobotsScriptEventView<'_>,
) -> RobotsHandlerScriptCommandPlan {
    use RobotsHandlerEventReturnPolicy as Return;
    use RobotsHandlerScriptCommandFamily as Family;
    use RobotsSpecializedScriptCommandKind as Command;

    match event.event_type {
        event_type::SET_ALTERNATE_STATE => {
            specialized_plan(Command::LightSetAlternateState, event, Return::Zero)
        }
        event_type::CLEAR_STATE_MARKER => {
            specialized_plan(Command::LightClearStateMarker, event, Return::Zero)
        }
        _ => delegate_script_command_family(Family::ScriptLifecycle),
    }
}

pub fn classify_pickup_handler_script_command(
    event: RobotsScriptEventView<'_>,
) -> RobotsHandlerScriptCommandPlan {
    use RobotsHandlerEventReturnPolicy as Return;
    use RobotsHandlerScriptCommandSemantic as Semantic;
    use RobotsSpecializedScriptCommandKind as Command;

    if event.event_type == u32::MAX {
        specialized_plan(Command::PickupExitScript, event, Return::One)
    } else {
        // 0x00413B80 deliberately swallows every non-FFFFFFFF command.
        RobotsHandlerScriptCommandPlan {
            semantic: Semantic::Unhandled,
            return_policy: Return::Zero,
        }
    }
}

pub fn classify_boss_exec_handler_script_command(
    event: RobotsScriptEventView<'_>,
) -> RobotsHandlerScriptCommandPlan {
    use RobotsHandlerEventReturnPolicy as Return;
    use RobotsHandlerScriptCommandFamily as Family;
    use RobotsSpecializedScriptCommandKind as Command;

    match event.event_type {
        event_type::SETUP_IDLE => specialized_plan(Command::BossExecSetupIdle, event, Return::Zero),
        event_type::CREATE_PROJECTILE => {
            specialized_plan(Command::BossExecCreateProjectile, event, Return::Zero)
        }
        event_type::WAIT_FOR_STATE => {
            specialized_plan(Command::BossExecWaitForState, event, Return::StateDependent)
        }
        _ => delegate_script_command_family(Family::ScriptLifecycle),
    }
}

pub fn classify_boss_sewer_canon_handler_script_command(
    event: RobotsScriptEventView<'_>,
) -> RobotsHandlerScriptCommandPlan {
    use RobotsHandlerEventReturnPolicy as Return;
    use RobotsHandlerScriptCommandFamily as Family;
    use RobotsSpecializedScriptCommandKind as Command;

    if event.event_type == event_type::CREATE_PROJECTILE {
        return specialized_plan(Command::BossSewerCanonCreateProjectile, event, Return::Zero);
    }
    if event.event_type == event_type::WAIT_FOR_STATE
        && matches!(event.native_arg_word(0), Some(7 | 8 | 9))
    {
        return specialized_plan(
            Command::BossSewerCanonWaitForState,
            event,
            Return::StateDependent,
        );
    }
    delegate_script_command_family(Family::Generic)
}

pub fn classify_recovered_handler_script_command(
    family: RobotsHandlerScriptCommandFamily,
    event: RobotsScriptEventView<'_>,
) -> Option<RobotsHandlerScriptCommandPlan> {
    use RobotsHandlerScriptCommandFamily as Family;

    Some(match family {
        Family::Generic => classify_generic_handler_script_command(event),
        Family::Cutscene => classify_cutscene_handler_script_command(event),
        Family::Door => classify_door_handler_script_command(event),
        Family::MagneticScript => classify_magnetic_script_handler_script_command(event),
        Family::ArcadeShop => classify_arcade_shop_handler_script_command(event),
        Family::Hazard => classify_hazard_handler_script_command(event),
        Family::Interactive => classify_interactive_handler_script_command(event),
        Family::Light => classify_light_handler_script_command(event),
        Family::Pickup => classify_pickup_handler_script_command(event),
        Family::ScriptLifecycle => classify_script_lifecycle_handler_script_command(event),
        Family::BossExec => classify_boss_exec_handler_script_command(event),
        Family::BossSewerCanon => classify_boss_sewer_canon_handler_script_command(event),
    })
}

pub fn resolve_recovered_handler_script_command(
    family: RobotsHandlerScriptCommandFamily,
    event: RobotsScriptEventView<'_>,
) -> Option<RobotsHandlerScriptCommandPlan> {
    use RobotsHandlerScriptCommandSemantic as Semantic;

    let mut family = family;
    for _ in 0..4 {
        let plan = classify_recovered_handler_script_command(family, event)?;
        match &plan.semantic {
            Semantic::DelegateFamily { family: next } => family = *next,
            _ => return Some(plan),
        }
    }
    None
}

pub fn classify_script_lifecycle_handler_script_command(
    event: RobotsScriptEventView<'_>,
) -> RobotsHandlerScriptCommandPlan {
    use event_type::*;
    use RobotsHandlerEventReturnPolicy as Return;
    use RobotsHandlerScriptCommandFamily as Family;
    use RobotsHandlerScriptCommandSemantic as Semantic;

    let semantic = match event.event_type {
        // Native 0x00416D10 compares command+0x14 against the marker-table UID.
        // UXGeoScriptCommandData::Event strips event_type at command+0x0C, so
        // command+0x14 is Event data word 1, not word 0.
        STATE_MARKER => Semantic::StateMarker {
            marker: event.native_arg_word(0),
            start: event.start,
        },
        SET_ALTERNATE_STATE => Semantic::SetAlternateState,
        CLEAR_STATE_MARKER => Semantic::ClearStateMarker,
        WAIT_FOR_HIT => Semantic::WaitForHit {
            mask: event.native_arg_word(0),
        },
        CHANGE_SCRIPT => Semantic::ChangeScript {
            script: event.native_arg_word(0),
            owner_selector: event.native_arg_word(1),
        },
        INVENTORY_CHECK => Semantic::InventoryCheck {
            owner_selector: event.native_arg_word(0),
            item: event.native_arg_word(1),
            quantity: event.native_arg_float(2),
            fallback_item: event.native_arg_word(3),
            on_true: [event.native_arg_word(4), event.native_arg_word(5)],
            on_false: [event.native_arg_word(6), event.native_arg_word(7)],
            comparison: event
                .native_arg_byte(0x20)
                .map(RobotsInventoryComparison::from_native_mode),
        },
        TEXTURE_CONTROL => Semantic::TextureControl {
            mode: event.native_arg_word(0),
            target: event.native_arg_word(1),
        },
        SOUND_STOP => Semantic::SoundStop {
            sound: event.native_arg_word(0),
        },
        PLAY_TO_STATE_MARKER => Semantic::PlayToStateMarker {
            owner_selector: event.native_arg_word(0),
            marker: event.native_arg_word(1),
        },
        WAIT_FOR_HIT_MISSILES => Semantic::WaitForHitMissiles {
            mask: event.native_arg_word(0),
        },
        STATE_MARKER_END => Semantic::StateMarkerEnd,
        _ => Semantic::DelegateFamily {
            family: Family::Generic,
        },
    };

    let return_policy = match event.event_type {
        WAIT_FOR_HIT | WAIT_FOR_HIT_MISSILES => Return::StateDependent,
        STATE_MARKER | SET_ALTERNATE_STATE | CLEAR_STATE_MARKER | CHANGE_SCRIPT
        | INVENTORY_CHECK | TEXTURE_CONTROL | SOUND_STOP | PLAY_TO_STATE_MARKER
        | STATE_MARKER_END => Return::Zero,
        _ => Return::Delegate,
    };

    RobotsHandlerScriptCommandPlan {
        semantic,
        return_policy,
    }
}

pub fn classify_interactive_handler_script_command(
    event: RobotsScriptEventView<'_>,
) -> RobotsHandlerScriptCommandPlan {
    use event_type::*;
    use RobotsHandlerEventReturnPolicy as Return;
    use RobotsHandlerScriptCommandFamily as Family;
    use RobotsHandlerScriptCommandSemantic as Semantic;

    let semantic = match event.event_type {
        SET_ALTERNATE_STATE => Semantic::SetAlternateState,
        CLEAR_STATE_MARKER => Semantic::ClearStateMarker,
        _ => Semantic::DelegateFamily {
            family: Family::ScriptLifecycle,
        },
    };
    let return_policy = match event.event_type {
        SET_ALTERNATE_STATE | CLEAR_STATE_MARKER => Return::Zero,
        _ => Return::Delegate,
    };

    RobotsHandlerScriptCommandPlan {
        semantic,
        return_policy,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RobotsHandlerEventPlan {
    pub semantic: RobotsHandlerEventSemantic,
    pub return_policy: RobotsHandlerEventReturnPolicy,
    pub post_dispatch: RobotsHandlerEventPostDispatch,
}

impl RobotsHandlerEventPlan {
    const fn new(
        semantic: RobotsHandlerEventSemantic,
        return_policy: RobotsHandlerEventReturnPolicy,
        post_dispatch: RobotsHandlerEventPostDispatch,
    ) -> Self {
        Self {
            semantic,
            return_policy,
            post_dispatch,
        }
    }
}

pub fn classify_handler_event(
    family: RobotsHandlerEventFamily,
    event: RobotsScriptEventView<'_>,
) -> RobotsHandlerEventPlan {
    use event_type::*;
    use RobotsHandlerEventFamily as Family;
    use RobotsHandlerEventPostDispatch as Post;
    use RobotsHandlerEventReturnPolicy as Return;
    use RobotsHandlerEventSemantic as Semantic;

    let semantic = match family {
        Family::Base => Semantic::DelegateCommandHandler,
        Family::Camera => Semantic::NoOp,
        Family::Projectile => {
            if event.event_type == INTERNAL_PROJECTILE_SENTINEL {
                Semantic::ProjectileOwnerScriptSentinel
            } else {
                Semantic::Unhandled
            }
        }
        Family::AiCharacter => match event.event_type {
            SETUP_IDLE => Semantic::SetupIdle,
            SET_SCRIPT_VALUE => Semantic::SetScriptValue {
                value: event.native_arg_float(0),
            },
            HIT_CHECK => Semantic::HitCheck {
                query: RobotsHitQueryInitPlan::from_event(event),
            },
            ATTACH_SWOOSH => Semantic::AttachSwoosh {
                arg0: event.native_arg_word(0),
                arg1: event.native_arg_word(1),
            },
            ATTACH_PARTICLE => Semantic::AttachParticle {
                arg0: event.native_arg_word(0),
                arg1: event.native_arg_word(1),
            },
            CREATE_EXPLOSION => Semantic::CreateExplosion,
            FOOTSTEP => Semantic::Footstep {
                arg0: event.native_arg_word(0),
            },
            DETTACH_SWOOSH => Semantic::DettachSwoosh {
                arg0: event.native_arg_word(0),
            },
            _ => Semantic::Unhandled,
        },
        Family::MagnaBot => match event.event_type {
            SETUP_IDLE => Semantic::SetupIdle,
            CHANGE_ANIM_MODE => Semantic::ChangeAnimMode {
                anim_mode: event.native_arg_word(0),
            },
            _ => Semantic::DelegateFamily {
                family: Family::AiCharacter,
            },
        },
        Family::WatchBot => Semantic::ForwardCurrentWatchBotComponent,
        Family::Player => match event.event_type {
            SETUP_IDLE => Semantic::SetupIdle,
            CHANGE_ANIM_MODE => Semantic::ChangeAnimMode {
                anim_mode: event.native_arg_word(0),
            },
            FORCE_IDLE_MODE => Semantic::ForceIdleMode,
            SET_SCRIPT_VALUE => Semantic::SetScriptValue {
                value: event.native_arg_float(0),
            },
            HIT_CHECK => Semantic::HitCheck {
                query: RobotsHitQueryInitPlan::from_event(event),
            },
            SET_WEAPON_VALUE => Semantic::SetWeaponValue,
            CREATE_PROJECTILE => Semantic::CreateProjectile {
                args: event.native_args::<4>(),
            },
            ATTACH_SWOOSH => Semantic::AttachSwoosh {
                arg0: event.native_arg_word(0),
                arg1: event.native_arg_word(1),
            },
            ATTACH_PARTICLE => Semantic::AttachParticle {
                arg0: event.native_arg_word(0),
                arg1: event.native_arg_word(1),
            },
            ATTACH_PARTICLES_TO_SKELETON => Semantic::AttachParticlesToSkeleton {
                args: [
                    event.native_arg_word(0),
                    event.native_arg_word(1),
                    event.native_arg_word(2),
                    event.native_arg_word(3),
                ],
            },
            PLAYER_ATTACK_BRANCH => Semantic::PlayerAttackBranch {
                threshold: event.native_arg_float(0),
                anim_mode: event.native_arg_word(1),
            },
            FOOTSTEP => Semantic::Footstep {
                arg0: event.native_arg_word(0),
            },
            DETTACH_SWOOSH => Semantic::DettachSwoosh {
                arg0: event.native_arg_word(0),
            },
            _ => Semantic::Unhandled,
        },
        Family::FirstPersonPlayer => match event.event_type {
            SETUP_IDLE => Semantic::SetupIdle,
            SET_WEAPON_VALUE => Semantic::SetWeaponValue,
            _ => Semantic::Unhandled,
        },
        Family::BossExecBomb => match event.event_type {
            WAIT_FOR_STATE if event.native_arg_word(0) == Some(12) => {
                Semantic::WaitForState { state: Some(12) }
            }
            _ => Semantic::DelegateCommandHandler,
        },
        Family::BossSewer => match event.event_type {
            SETUP_IDLE => Semantic::SetupIdle,
            HIT_CHECK => Semantic::HitCheck {
                query: RobotsHitQueryInitPlan::from_event(event),
            },
            _ => Semantic::DelegateFamily {
                family: Family::AiCharacter,
            },
        },
        Family::SweeperBoss => match event.event_type {
            SET_SCRIPT_VALUE => Semantic::SetScriptValue {
                value: event.native_arg_float(0),
            },
            _ => Semantic::Unhandled,
        },
        Family::SweeperRatchet => match event.event_type {
            SET_SCRIPT_VALUE => Semantic::SetScriptValue {
                value: event.native_arg_float(0),
            },
            CREATE_PROJECTILE => Semantic::CreateProjectile {
                args: event.native_args::<4>(),
            },
            _ => Semantic::HandledNoEffect,
        },
        Family::FinalBossHealth => match event.event_type {
            WAIT_FOR_HIT => Semantic::WaitForHit,
            INVENTORY_ADD => Semantic::InventoryAdd,
            _ => Semantic::DelegateCommandHandler,
        },
    };

    let return_policy = match &semantic {
        Semantic::DelegateCommandHandler => Return::Delegate,
        Semantic::DelegateFamily { .. } => Return::Delegate,
        Semantic::WaitForState { .. } => Return::StateDependent,
        Semantic::WaitForHit | Semantic::InventoryAdd if family == Family::FinalBossHealth => {
            Return::StateDependent
        }
        Semantic::SetScriptValue { .. } if family == Family::SweeperRatchet => Return::One,
        Semantic::HandledNoEffect if family == Family::SweeperRatchet => Return::One,
        _ => Return::Zero,
    };

    RobotsHandlerEventPlan::new(
        semantic,
        return_policy,
        if family == Family::AiCharacter {
            Post::AiEmbeddedChannels
        } else {
            Post::None
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw_event(event_type: u32, words: &[u32]) -> RobotsScriptEventView<'static> {
        let bytes = words
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        RobotsScriptEventView {
            event_type,
            data: Box::leak(bytes),
            start: None,
            length: None,
        }
    }

    /// Test helper takes native Handler arguments, not raw serialized Event.data.
    /// The ignored/provenance dword at command+0x10 is inserted automatically.
    fn event(event_type: u32, args: &[u32]) -> RobotsScriptEventView<'static> {
        let mut words = Vec::with_capacity(args.len() + 1);
        words.push(0);
        words.extend_from_slice(args);
        raw_event(event_type, &words)
    }

    fn event_with_start(
        event_type: u32,
        start: i16,
        args: &[u32],
    ) -> RobotsScriptEventView<'static> {
        let mut event = event(event_type, args);
        event.start = Some(start);
        event
    }

    #[test]
    fn native_argument_view_skips_only_the_raw_command_plus_0x10_dword() {
        let event = raw_event(0x1600_0001, &[0xAAAA_AAAA, 0xBBBB_BBBB, 0xCCCC_CCCC]);
        assert_eq!(event.word(0), Some(0xAAAA_AAAA));
        assert_eq!(event.native_arg_word(0), Some(0xBBBB_BBBB));
        assert_eq!(event.native_arg_word(1), Some(0xCCCC_CCCC));
        assert_eq!(event.native_arg_word(2), None);
    }

    #[test]
    fn native_handler_targets_round_trip() {
        let families = [
            RobotsHandlerEventFamily::Base,
            RobotsHandlerEventFamily::Projectile,
            RobotsHandlerEventFamily::AiCharacter,
            RobotsHandlerEventFamily::Camera,
            RobotsHandlerEventFamily::MagnaBot,
            RobotsHandlerEventFamily::WatchBot,
            RobotsHandlerEventFamily::Player,
            RobotsHandlerEventFamily::FirstPersonPlayer,
            RobotsHandlerEventFamily::BossExecBomb,
            RobotsHandlerEventFamily::BossSewer,
            RobotsHandlerEventFamily::SweeperBoss,
            RobotsHandlerEventFamily::SweeperRatchet,
            RobotsHandlerEventFamily::FinalBossHealth,
        ];
        for family in families {
            assert_eq!(
                RobotsHandlerEventFamily::from_native_target(family.native_target()),
                Some(family)
            );
        }
        assert_eq!(RobotsHandlerEventFamily::from_native_target(0), None);
    }

    #[test]
    fn native_script_command_targets_round_trip() {
        let families = [
            RobotsHandlerScriptCommandFamily::Generic,
            RobotsHandlerScriptCommandFamily::Cutscene,
            RobotsHandlerScriptCommandFamily::Door,
            RobotsHandlerScriptCommandFamily::MagneticScript,
            RobotsHandlerScriptCommandFamily::ArcadeShop,
            RobotsHandlerScriptCommandFamily::Hazard,
            RobotsHandlerScriptCommandFamily::Interactive,
            RobotsHandlerScriptCommandFamily::Light,
            RobotsHandlerScriptCommandFamily::Pickup,
            RobotsHandlerScriptCommandFamily::ScriptLifecycle,
            RobotsHandlerScriptCommandFamily::BossExec,
            RobotsHandlerScriptCommandFamily::BossSewerCanon,
        ];
        for family in families {
            assert_eq!(
                RobotsHandlerScriptCommandFamily::from_native_target(family.native_target()),
                Some(family)
            );
        }
        assert_eq!(
            RobotsHandlerScriptCommandFamily::from_native_target(0),
            None
        );
    }

    #[test]
    fn camera_event_family_is_a_true_no_op() {
        let plan = classify_handler_event(
            RobotsHandlerEventFamily::Camera,
            event(event_type::SET_SCRIPT_VALUE, &[1.0f32.to_bits()]),
        );
        assert_eq!(plan.semantic, RobotsHandlerEventSemantic::NoOp);
        assert_eq!(plan.post_dispatch, RobotsHandlerEventPostDispatch::None);
    }

    #[test]
    fn ai_script_value_keeps_post_channel_forwarding() {
        let plan = classify_handler_event(
            RobotsHandlerEventFamily::AiCharacter,
            event(event_type::SET_SCRIPT_VALUE, &[1.0f32.to_bits()]),
        );
        assert_eq!(
            plan.semantic,
            RobotsHandlerEventSemantic::SetScriptValue { value: Some(1.0) }
        );
        assert_eq!(
            plan.post_dispatch,
            RobotsHandlerEventPostDispatch::AiEmbeddedChannels
        );
        assert_eq!(plan.return_policy, RobotsHandlerEventReturnPolicy::Zero);
    }

    #[test]
    fn ai_and_player_hit_check_share_exact_native_query_init() {
        for family in [
            RobotsHandlerEventFamily::AiCharacter,
            RobotsHandlerEventFamily::Player,
        ] {
            let plan = classify_handler_event(
                family,
                event(event_type::HIT_CHECK, &[0x1000_0010, 0.75f32.to_bits()]),
            );
            assert_eq!(
                plan.semantic,
                RobotsHandlerEventSemantic::HitCheck {
                    query: Some(RobotsHitQueryInitPlan {
                        selector: 0x1000_0010,
                        remaining_budget: 1.5,
                        initial_flags: 0,
                    }),
                }
            );
            assert_eq!(plan.return_policy, RobotsHandlerEventReturnPolicy::Zero);
        }
    }

    #[test]
    fn magnabot_unknown_event_delegates_to_shared_ai_family() {
        let plan =
            classify_handler_event(RobotsHandlerEventFamily::MagnaBot, event(0x1600_004A, &[]));
        assert_eq!(
            plan.semantic,
            RobotsHandlerEventSemantic::DelegateFamily {
                family: RobotsHandlerEventFamily::AiCharacter
            }
        );
    }

    #[test]
    fn player_attack_branch_decodes_threshold_and_anim_mode() {
        let plan = classify_handler_event(
            RobotsHandlerEventFamily::Player,
            event(
                event_type::PLAYER_ATTACK_BRANCH,
                &[0.75f32.to_bits(), 0x0900_0025],
            ),
        );
        assert_eq!(
            plan.semantic,
            RobotsHandlerEventSemantic::PlayerAttackBranch {
                threshold: Some(0.75),
                anim_mode: Some(0x0900_0025),
            }
        );
    }

    #[test]
    fn projectile_family_only_promotes_the_internal_sentinel() {
        let sentinel = classify_handler_event(
            RobotsHandlerEventFamily::Projectile,
            event(event_type::INTERNAL_PROJECTILE_SENTINEL, &[]),
        );
        assert_eq!(
            sentinel.semantic,
            RobotsHandlerEventSemantic::ProjectileOwnerScriptSentinel
        );
        let normal = classify_handler_event(
            RobotsHandlerEventFamily::Projectile,
            event(event_type::SETUP_IDLE, &[]),
        );
        assert_eq!(normal.semantic, RobotsHandlerEventSemantic::Unhandled);
    }

    #[test]
    fn boss_exec_bomb_only_specializes_wait_for_state_12() {
        let specialized = classify_handler_event(
            RobotsHandlerEventFamily::BossExecBomb,
            event(event_type::WAIT_FOR_STATE, &[12]),
        );
        assert_eq!(
            specialized.semantic,
            RobotsHandlerEventSemantic::WaitForState { state: Some(12) }
        );
        let fallback = classify_handler_event(
            RobotsHandlerEventFamily::BossExecBomb,
            event(event_type::WAIT_FOR_STATE, &[11]),
        );
        assert_eq!(
            fallback.semantic,
            RobotsHandlerEventSemantic::DelegateCommandHandler
        );
        assert_eq!(
            fallback.return_policy,
            RobotsHandlerEventReturnPolicy::Delegate
        );
    }

    #[test]
    fn ratchet_unknown_event_is_handled_without_effect() {
        let plan = classify_handler_event(
            RobotsHandlerEventFamily::SweeperRatchet,
            event(event_type::FOOTSTEP, &[0]),
        );
        assert_eq!(plan.semantic, RobotsHandlerEventSemantic::HandledNoEffect);
        assert_eq!(plan.return_policy, RobotsHandlerEventReturnPolicy::One);
    }

    #[test]
    fn final_boss_health_routes_only_its_two_special_events() {
        let wait = classify_handler_event(
            RobotsHandlerEventFamily::FinalBossHealth,
            event(event_type::WAIT_FOR_HIT, &[]),
        );
        assert_eq!(wait.semantic, RobotsHandlerEventSemantic::WaitForHit);
        assert_eq!(
            wait.return_policy,
            RobotsHandlerEventReturnPolicy::StateDependent
        );
        let inventory = classify_handler_event(
            RobotsHandlerEventFamily::FinalBossHealth,
            event(event_type::INVENTORY_ADD, &[1]),
        );
        assert_eq!(inventory.semantic, RobotsHandlerEventSemantic::InventoryAdd);
    }

    #[test]
    fn generic_command_switch_covers_all_recovered_native_cases() {
        let event_types = [
            event_type::WAIT,
            event_type::WAIT_FOR_MESSAGE,
            event_type::WAIT_FOR_ITEM_LAND_ON,
            event_type::WAIT_FOR_ITEM_OFF,
            event_type::WAIT_FOR_MESSAGE_SPECIFIC,
            event_type::FILE_LOAD,
            event_type::FILE_DELOAD,
            event_type::MESSAGE_RELAY,
            event_type::MESSAGE_RELAY_SPECIFIC,
            event_type::GENERATE_PICKUP,
            event_type::ATTACH_SWOOSH,
            event_type::ATTACH_PARTICLE,
            event_type::ATTACH_PARTICLES_TO_SKELETON,
            event_type::ATTACH_SPECIAL_EFFECT,
            event_type::CREATE_EXPLOSION,
            event_type::SHOW_MESSAGE,
            event_type::EXIT_SCRIPT,
            event_type::INVENTORY_ADD,
            event_type::INVENTORY_REMOVE,
            event_type::MISSION_UPDATE,
            event_type::SHAKE_CAMERA,
            event_type::DETTACH_SWOOSH,
            event_type::FILE_LOAD_SUBFILE,
            event_type::FILE_DELOAD_SUBFILE,
        ];
        assert_eq!(event_types.len(), 24);
        for event_type in event_types {
            let plan = classify_generic_handler_script_command(event(event_type, &[]));
            assert_ne!(plan.semantic, RobotsHandlerScriptCommandSemantic::Unhandled);
        }
    }

    #[test]
    fn generic_wait_family_preserves_native_host_result() {
        for event_type in [
            event_type::WAIT,
            event_type::WAIT_FOR_MESSAGE,
            event_type::WAIT_FOR_ITEM_LAND_ON,
            event_type::WAIT_FOR_ITEM_OFF,
            event_type::WAIT_FOR_MESSAGE_SPECIFIC,
        ] {
            let plan = classify_generic_handler_script_command(event(event_type, &[]));
            assert_eq!(
                plan.return_policy,
                RobotsHandlerEventReturnPolicy::HostResult
            );
        }
    }

    #[test]
    fn generic_exit_script_is_the_only_recovered_one_return() {
        let exit = classify_generic_handler_script_command(event(event_type::EXIT_SCRIPT, &[]));
        assert_eq!(
            exit.semantic,
            RobotsHandlerScriptCommandSemantic::ExitScript
        );
        assert_eq!(exit.return_policy, RobotsHandlerEventReturnPolicy::One);

        let unknown = classify_generic_handler_script_command(event(0x1600_004A, &[]));
        assert_eq!(
            unknown.semantic,
            RobotsHandlerScriptCommandSemantic::Unhandled
        );
        assert_eq!(unknown.return_policy, RobotsHandlerEventReturnPolicy::Zero);
    }

    #[test]
    fn generic_command_payloads_decode_without_handler_internals() {
        let pickup = classify_generic_handler_script_command(event(
            event_type::GENERATE_PICKUP,
            &[0x4700_0023, 3.0f32.to_bits()],
        ));
        assert_eq!(
            pickup.semantic,
            RobotsHandlerScriptCommandSemantic::GeneratePickup {
                pickup: Some(0x4700_0023),
                quantity: Some(3.0),
            }
        );

        let inventory = classify_generic_handler_script_command(event(
            event_type::INVENTORY_ADD,
            &[0, 0x4700_0023, 1.0f32.to_bits(), 0x4700_0042, 2],
        ));
        assert_eq!(
            inventory.semantic,
            RobotsHandlerScriptCommandSemantic::InventoryAdd {
                owner_selector: Some(0),
                item: Some(0x4700_0023),
                quantity: Some(1.0),
                fallback_item: Some(0x4700_0042),
                mode: Some(2),
            }
        );

        let subfile = classify_generic_handler_script_command(event(
            event_type::FILE_LOAD_SUBFILE,
            &[1, 2, 3, 4, 5, 6, 7, 8],
        ));
        assert_eq!(
            subfile.semantic,
            RobotsHandlerScriptCommandSemantic::FileLoadSubfile {
                entries: [
                    Some(1),
                    Some(2),
                    Some(3),
                    Some(4),
                    Some(5),
                    Some(6),
                    Some(7),
                    Some(8),
                ],
            }
        );
    }

    #[test]
    fn script_lifecycle_keeps_local_cases_and_generic_fallback_separate() {
        let marker = classify_script_lifecycle_handler_script_command(event_with_start(
            event_type::STATE_MARKER,
            37,
            &[0x5600_0012],
        ));
        assert_eq!(
            marker.semantic,
            RobotsHandlerScriptCommandSemantic::StateMarker {
                marker: Some(0x5600_0012),
                start: Some(37),
            }
        );
        assert_eq!(marker.return_policy, RobotsHandlerEventReturnPolicy::Zero);

        let wait = classify_script_lifecycle_handler_script_command(event(
            event_type::WAIT_FOR_HIT,
            &[0x15A],
        ));
        assert_eq!(
            wait.semantic,
            RobotsHandlerScriptCommandSemantic::WaitForHit { mask: Some(0x15A) }
        );
        assert_eq!(
            wait.return_policy,
            RobotsHandlerEventReturnPolicy::StateDependent
        );

        let fallback =
            classify_script_lifecycle_handler_script_command(event(event_type::EXIT_SCRIPT, &[]));
        assert_eq!(
            fallback.semantic,
            RobotsHandlerScriptCommandSemantic::DelegateFamily {
                family: RobotsHandlerScriptCommandFamily::Generic,
            }
        );
        assert_eq!(
            fallback.return_policy,
            RobotsHandlerEventReturnPolicy::Delegate
        );
    }

    #[test]
    fn script_lifecycle_inventory_check_decodes_native_comparison_and_branches() {
        let plan = classify_script_lifecycle_handler_script_command(event(
            event_type::INVENTORY_CHECK,
            &[
                0,
                0x4700_0023,
                3.0f32.to_bits(),
                0x4700_0042,
                0x5600_0001,
                0x5600_0002,
                0x5600_0003,
                0x5600_0004,
                2,
            ],
        ));
        assert_eq!(
            plan.semantic,
            RobotsHandlerScriptCommandSemantic::InventoryCheck {
                owner_selector: Some(0),
                item: Some(0x4700_0023),
                quantity: Some(3.0),
                fallback_item: Some(0x4700_0042),
                on_true: [Some(0x5600_0001), Some(0x5600_0002)],
                on_false: [Some(0x5600_0003), Some(0x5600_0004)],
                comparison: Some(RobotsInventoryComparison::Equal),
            }
        );
    }

    #[test]
    fn interactive_specializes_two_events_then_delegates_to_script_lifecycle() {
        for event_type in [
            event_type::SET_ALTERNATE_STATE,
            event_type::CLEAR_STATE_MARKER,
        ] {
            let plan = classify_interactive_handler_script_command(event(event_type, &[]));
            assert_eq!(plan.return_policy, RobotsHandlerEventReturnPolicy::Zero);
            assert!(!matches!(
                plan.semantic,
                RobotsHandlerScriptCommandSemantic::DelegateFamily { .. }
            ));
        }

        let fallback = classify_interactive_handler_script_command(event(
            event_type::STATE_MARKER,
            &[0x5600_0012],
        ));
        assert_eq!(
            fallback.semantic,
            RobotsHandlerScriptCommandSemantic::DelegateFamily {
                family: RobotsHandlerScriptCommandFamily::ScriptLifecycle,
            }
        );
        assert_eq!(
            fallback.return_policy,
            RobotsHandlerEventReturnPolicy::Delegate
        );
    }

    #[test]
    fn cutscene_keeps_native_local_cases_out_of_generic_fallback() {
        let plan = classify_cutscene_handler_script_command(event_with_start(
            event_type::POSITION_CHARACTER,
            73,
            &[0, 0x0200_0017, 0x1234_5678, 0x9ABC_DEF0],
        ));
        assert_eq!(
            plan.semantic,
            RobotsHandlerScriptCommandSemantic::CutsceneCommand {
                command: RobotsCutsceneCommandKind::PositionCharacter,
                args: [
                    Some(0),
                    Some(0x0200_0017),
                    Some(0x1234_5678),
                    Some(0x9ABC_DEF0),
                    None,
                    None,
                    None,
                    None,
                ],
                start: Some(73),
                length: None,
            }
        );
        assert_eq!(plan.return_policy, RobotsHandlerEventReturnPolicy::Zero);
    }

    #[test]
    fn cutscene_inventory_add_resolves_through_script_lifecycle_to_generic() {
        let payload = &[4, 0x4700_0015, 12.0f32.to_bits()];
        let local =
            classify_cutscene_handler_script_command(event(event_type::INVENTORY_ADD, payload));
        assert_eq!(
            local.semantic,
            RobotsHandlerScriptCommandSemantic::DelegateFamily {
                family: RobotsHandlerScriptCommandFamily::ScriptLifecycle,
            }
        );

        let resolved = resolve_recovered_handler_script_command(
            RobotsHandlerScriptCommandFamily::Cutscene,
            event(event_type::INVENTORY_ADD, payload),
        )
        .expect("Cutscene -> ScriptLifecycle -> Generic is fully recovered");
        assert_eq!(
            resolved.semantic,
            RobotsHandlerScriptCommandSemantic::InventoryAdd {
                owner_selector: Some(4),
                item: Some(0x4700_0015),
                quantity: Some(12.0),
                fallback_item: None,
                mode: None,
            }
        );
        assert_eq!(resolved.return_policy, RobotsHandlerEventReturnPolicy::Zero);
    }

    #[test]
    fn door_local_cases_preserve_state_dependent_returns_and_fallback() {
        let open = classify_door_handler_script_command(event(event_type::OPEN, &[]));
        assert_eq!(
            open.semantic,
            RobotsHandlerScriptCommandSemantic::DoorCommand {
                command: RobotsDoorCommandKind::Open,
            }
        );
        assert_eq!(
            open.return_policy,
            RobotsHandlerEventReturnPolicy::StateDependent
        );

        let closed = classify_door_handler_script_command(event(event_type::CLOSED, &[]));
        assert_eq!(
            closed.semantic,
            RobotsHandlerScriptCommandSemantic::DoorCommand {
                command: RobotsDoorCommandKind::Closed,
            }
        );
        assert_eq!(closed.return_policy, RobotsHandlerEventReturnPolicy::Zero);

        let fallback = classify_door_handler_script_command(event(event_type::INVENTORY_ADD, &[]));
        assert_eq!(
            fallback.semantic,
            RobotsHandlerScriptCommandSemantic::DelegateFamily {
                family: RobotsHandlerScriptCommandFamily::ScriptLifecycle,
            }
        );
        assert_eq!(
            fallback.return_policy,
            RobotsHandlerEventReturnPolicy::Delegate
        );
    }

    #[test]
    fn remaining_specialized_families_preserve_local_cases_and_native_fallbacks() {
        let magnetic = classify_magnetic_script_handler_script_command(event_with_start(
            event_type::STATE_MARKER,
            19,
            &[0x5600_0001],
        ));
        assert!(matches!(
            magnetic.semantic,
            RobotsHandlerScriptCommandSemantic::SpecializedCommand {
                command: RobotsSpecializedScriptCommandKind::MagneticStateMarker,
                start: Some(19),
                ..
            }
        ));

        let arcade =
            classify_arcade_shop_handler_script_command(event(event_type::WAIT_FOR_STATE, &[3]));
        assert_eq!(
            arcade.return_policy,
            RobotsHandlerEventReturnPolicy::StateDependent
        );
        let arcade_fallback =
            classify_arcade_shop_handler_script_command(event(event_type::WAIT_FOR_STATE, &[2]));
        assert_eq!(
            arcade_fallback.semantic,
            RobotsHandlerScriptCommandSemantic::DelegateFamily {
                family: RobotsHandlerScriptCommandFamily::Interactive,
            }
        );

        let hazard =
            classify_hazard_handler_script_command(event(event_type::WAIT_FOR_HIT, &[0x15A]));
        assert!(matches!(
            hazard.semantic,
            RobotsHandlerScriptCommandSemantic::SpecializedCommand {
                command: RobotsSpecializedScriptCommandKind::HazardWaitForHit,
                ..
            }
        ));
        assert_eq!(
            hazard.return_policy,
            RobotsHandlerEventReturnPolicy::StateDependent
        );

        let hazard_hit = classify_hazard_handler_script_command(event(
            event_type::HIT_CHECK,
            &[0x1000_0010, 0.75f32.to_bits()],
        ));
        assert_eq!(
            hazard_hit.semantic,
            RobotsHandlerScriptCommandSemantic::SpecializedCommand {
                command: RobotsSpecializedScriptCommandKind::HazardHitCheck,
                args: [Some(0x1000_0010), Some(0.75f32.to_bits()), None, None,],
                start: None,
                hit_query: Some(RobotsHitQueryInitPlan {
                    selector: 0x1000_0010,
                    remaining_budget: 1.5,
                    initial_flags: 0,
                }),
            }
        );
        assert_eq!(
            hazard_hit.return_policy,
            RobotsHandlerEventReturnPolicy::Zero
        );

        let light =
            classify_light_handler_script_command(event(event_type::CLEAR_STATE_MARKER, &[]));
        assert!(matches!(
            light.semantic,
            RobotsHandlerScriptCommandSemantic::SpecializedCommand {
                command: RobotsSpecializedScriptCommandKind::LightClearStateMarker,
                ..
            }
        ));

        let pickup_exit = classify_pickup_handler_script_command(event(u32::MAX, &[]));
        assert_eq!(
            pickup_exit.return_policy,
            RobotsHandlerEventReturnPolicy::One
        );
        let pickup_swallow =
            classify_pickup_handler_script_command(event(event_type::INVENTORY_ADD, &[]));
        assert_eq!(
            pickup_swallow.semantic,
            RobotsHandlerScriptCommandSemantic::Unhandled
        );
        assert_eq!(
            pickup_swallow.return_policy,
            RobotsHandlerEventReturnPolicy::Zero
        );

        let boss_exec =
            classify_boss_exec_handler_script_command(event(event_type::WAIT_FOR_STATE, &[6]));
        assert_eq!(
            boss_exec.return_policy,
            RobotsHandlerEventReturnPolicy::StateDependent
        );

        let sewer = classify_boss_sewer_canon_handler_script_command(event(
            event_type::WAIT_FOR_STATE,
            &[8],
        ));
        assert_eq!(
            sewer.return_policy,
            RobotsHandlerEventReturnPolicy::StateDependent
        );
        let sewer_fallback = classify_boss_sewer_canon_handler_script_command(event(
            event_type::WAIT_FOR_STATE,
            &[6],
        ));
        assert_eq!(
            sewer_fallback.semantic,
            RobotsHandlerScriptCommandSemantic::DelegateFamily {
                family: RobotsHandlerScriptCommandFamily::Generic,
            }
        );
    }

    #[test]
    fn every_native_script_command_family_is_recovered() {
        let families = [
            RobotsHandlerScriptCommandFamily::Generic,
            RobotsHandlerScriptCommandFamily::Cutscene,
            RobotsHandlerScriptCommandFamily::Door,
            RobotsHandlerScriptCommandFamily::MagneticScript,
            RobotsHandlerScriptCommandFamily::ArcadeShop,
            RobotsHandlerScriptCommandFamily::Hazard,
            RobotsHandlerScriptCommandFamily::Interactive,
            RobotsHandlerScriptCommandFamily::Light,
            RobotsHandlerScriptCommandFamily::Pickup,
            RobotsHandlerScriptCommandFamily::ScriptLifecycle,
            RobotsHandlerScriptCommandFamily::BossExec,
            RobotsHandlerScriptCommandFamily::BossSewerCanon,
        ];
        for family in families {
            assert!(classify_recovered_handler_script_command(
                family,
                event(
                    event_type::INVENTORY_ADD,
                    &[0, 0x4700_0001, 1.0f32.to_bits()]
                ),
            )
            .is_some());
        }
    }
}
