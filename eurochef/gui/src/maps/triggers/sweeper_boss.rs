use std::{
    collections::BTreeMap,
    io::{Seek, SeekFrom},
};

use anyhow::{bail, Context};
use eurochef_edb::{binrw::BinReaderExt, edb::EdbFile};
use eurochef_shared::{
    script::{UXGeoScript, UXGeoScriptCommandData},
    spreadsheets::UXGeoSpreadsheet,
};

pub const EYE_TYPE: u32 = 86;
pub const CONTROLLER_TYPE: u32 = 87;
pub const PATTERN_FILE: u32 = 0x0100_00BC;
pub const PATTERN_SHEET: u32 = 0x1400_0010;
pub const PATTERN_ROW_COUNT: usize = 70;

pub const CONTROLLER_SAVE_SIZE: usize = 0x18;
pub const EYE_COUNT: usize = 5;
pub const INITIAL_DIFFICULTY: u8 = 5;
pub const INITIAL_EYE_STATUS: u8 = 1;
pub const CONTROLLER_RUNTIME_CLASS_CODE: u32 = 19;
pub const CONTROLLER_SERVICE_RADIUS: f32 = 10.0;
pub const EYE_PATTERN_STAGGER_UPDATES: f32 = 20.0;
pub const EYE_PRESSURE_THRESHOLD: u32 = 5;
pub const EYE_OPEN_SECONDS: f32 = 4.0;
pub const EYE_INITIAL_HIT_POINTS: u8 = 1;
pub const MAX_DIFFICULTY: u8 = 15;

pub const EYE_STATE_CLOSED: i32 = 1;
pub const EYE_STATE_OPENING: i32 = 2;
pub const EYE_STATE_OPEN: i32 = 3;
pub const EYE_STATE_CLOSING: i32 = 4;
pub const EYE_STATE_HIT: i32 = 5;
pub const EYE_STATE_DESTROYED: i32 = 7;
pub const EYE_OPEN_REQUEST: i32 = 3;

pub const EYE_SCRIPT_CLOSED: u32 = 0x0400_0258;
pub const EYE_SCRIPT_OPENING: u32 = 0x0400_0259;
pub const EYE_SCRIPT_OPEN: u32 = 0x0400_025A;
pub const EYE_SCRIPT_CLOSING: u32 = 0x0400_025B;
pub const EYE_SCRIPT_HIT: u32 = 0x0400_025C;
pub const EYE_SCRIPT_DESTROYED: u32 = 0x0400_025D;

pub const HEALTH_PICKUP_SCRIPT: u32 = 0x0400_0022;
pub const HEALTH_PICKUP_ITEM: u32 = 0x4700_0023;
pub const HEALTH_PICKUP_WAIT_FOR_HIT_EVENT: u32 = 0x1600_001D;
pub const HEALTH_PICKUP_INVENTORY_ADD_EVENT: u32 = 0x1600_002E;
pub const HEALTH_PICKUP_TIMER_INITIAL_SECONDS: f32 = 5.0;
pub const HEALTH_PICKUP_TIMER_JITTER_SECONDS: f32 = 10.0;
pub const HEALTH_PICKUP_LIFETIME_SECONDS: f32 = 10.0;
pub const HEALTH_PICKUP_ROTATION_PER_UPDATE: f32 = 0.034_906_585;
pub const HEALTH_PICKUP_REGISTRATION_MASK: u32 = 0x800;
// HT_Script_Pickup_Health_EnergyCell creates bo5_fin entity0x82000008.
// Its searchable HitArea0x10000010 is a radius0.75 sphere centered 0.49632353 above owner.
const HEALTH_PICKUP_HIT_AREA_OWNER_RADIUS_BOUND: f32 = 1.246_324;

/// Conservative owner-space miss proof for the FinalBossHealth WaitForHit contact.
/// Rodney's full animated HitArea is enclosed by `MAX_OWNER_OFFSET + radius`; the
/// pickup's shipped HitArea is enclosed by `HEALTH_PICKUP_HIT_AREA_OWNER_RADIUS_BOUND`.
/// Returning true proves the two native HitArea sets cannot overlap. Returning false
/// means only "possible/unknown contact" and must not be promoted to a hit.
pub(crate) fn sweeper_health_pickup_player_contact_guaranteed_miss(
    player_position: [f32; 4],
    pickup_owner_position: [f32; 4],
) -> bool {
    if player_position.iter().any(|value| !value.is_finite())
        || pickup_owner_position.iter().any(|value| !value.is_finite())
    {
        return false;
    }

    let dx = player_position[0] - pickup_owner_position[0];
    let dy = player_position[1] - pickup_owner_position[1];
    let dz = player_position[2] - pickup_owner_position[2];
    let clear_radius = RODNEY_HIT_AREA_MAX_OWNER_OFFSET
        + RODNEY_HIT_AREA_RADIUS
        + HEALTH_PICKUP_HIT_AREA_OWNER_RADIUS_BOUND;
    dx * dx + dy * dy + dz * dz > clear_radius * clear_radius
}
pub const HEALTH_PICKUP_SPAWN_X_SCALE: f32 = 60.0;
pub const HEALTH_PICKUP_SPAWN_X_BIAS: f32 = 30.0;
pub const HEALTH_PICKUP_SPAWN_Z: f32 = 5.0;
const HEALTH_PICKUP_WAIT_FOR_HIT_FRAME: u8 = 17;
const HEALTH_PICKUP_INVENTORY_ADD_FRAME: u8 = 19;
const HEALTH_PICKUP_SCRIPT_LENGTH: u8 = 26;

pub const RAT_RESOURCE_FILE: u32 = 0x0100_0051;
pub const RAT_BASE_SCRIPT: u32 = 0x0400_0250;
pub const RAT_POSITION_DATUM: u32 = 0x1000_0036;
pub const RAT_ENTITY: u32 = 0x0200_01AF;
pub const JUMP_LEFT_ANIM_MODE: u32 = 0x0900_0104;
pub const JUMP_RIGHT_ANIM_MODE: u32 = 0x0900_0105;
pub const APPEAR_ANIM_MODE: u32 = 0x0900_0107;
pub const RAT_JUMP_LEFT_YAW: f32 = -1.570_796_4;
pub const RAT_JUMP_RIGHT_YAW: f32 = 1.570_796_4;
pub const RAT_ARRIVAL_YAW: f32 = std::f32::consts::PI;
pub const RAT_FACE_PLAYER_ALPHA: f32 = 0.1;
pub const BOSS_ANIM_MODE: u32 = 0x0900_0106;
pub const ATTACK_ANIM_MODE: u32 = 0x0900_0025;
// Ratchet combat reducer constants recovered from the native boss handler.
pub const ATTACK2_ANIM_MODE: u32 = 0x0900_0027;
pub const IDLE_ATTACK_ANIM_MODE: u32 = 0x0900_0004;

pub const IDLE_COMBAT1_ANIM_MODE: u32 = 0x0900_002D;

pub const IDLE_COMBAT2_ANIM_MODE: u32 = 0x0900_007E;

pub const IDLE_COMBAT3_ANIM_MODE: u32 = 0x0900_0109;

pub const DISAPPEAR_ANIM_MODE: u32 = 0x0900_0108;
pub const ATTACK_ANIM_SET: u32 = 0x8A00_0012;
pub const APPEAR_SCRIPT: u32 = 0x8400_0006;
pub const ATTACK2_SCRIPT: u32 = 0x8400_0007;
pub const DISAPPEAR_SCRIPT: u32 = 0x8400_0008;
pub const IDLE_ATTACK_SCRIPT: u32 = 0x8400_000B;
pub const JUMP_LEFT_SCRIPT: u32 = 0x8400_000C;
pub const JUMP_RIGHT_SCRIPT: u32 = 0x8400_000D;
pub const IDLE_COMBAT1_SCRIPT: u32 = 0x8400_000F;
pub const IDLE_COMBAT2_SCRIPT: u32 = 0x8400_0010;
pub const IDLE_COMBAT3_SCRIPT: u32 = 0x8400_0011;
pub const ATTACK_SCRIPT: u32 = 0x8400_0012;
pub const ATTACK_ANIMATION: u32 = 0x8300_0009;
pub const RAT_SCRIPT_FIXED_HZ: f32 = 60.0;
pub const RAT_PROJECTILE_GRAVITY: f32 = 9.8;
pub const RAT_PROJECTILE_APEX_EXTRA_HEIGHT: f32 = 4.0;
pub const RAT_PROJECTILE_ATTACK_POINT_RADIUS: f32 = 0.7;
// ef02_dro entity 0x02000039 searchable HitArea 0x10000010 is a mode3 capsule:
// half-segment=2.5, radius=2.0, local center=(0,-0.13006775,0.38355651).
// Bounding the rotated center offset by its L1 norm keeps this owner-centered sphere conservative.
const TRANSPORTER_HIT_AREA_OWNER_RADIUS_BOUND: f32 = 5.013_625;
pub const RODNEY_HIT_AREA_RADIUS: f32 = 0.35;
pub const RODNEY_HIT_AREA_MAX_OWNER_OFFSET: f32 = 1.940_176_5;
pub const MALFBOT_HIT_AREA_MAX_OWNER_ENCLOSING_RADIUS: f32 = 3.529_991_2;
pub const ROLLERBOT_HIT_AREA_MAX_OWNER_ENCLOSING_RADIUS: f32 = 3.569_321_2;
pub const RAT_SCRIPT_VALUE_EVENT: u32 = 0x1600_0007;
pub const RAT_SCRIPT_VALUE_SIGNAL_1: u8 = 1;
pub const RAT_SCRIPT_VALUE_SIGNAL_2: u8 = 2;
pub const RAT_INITIAL_HIT_POINTS: u8 = 5;
pub const RAT_DAMAGE_PHASE_DIFFICULTY: u8 = 10;
pub const RAT_HIT_FORWARD_THRESHOLD: u8 = 70;
pub const RAT_HIT_FORWARD_ANIM_MODE: u32 = 0x0900_0029;
pub const RAT_HIT_BACK_ANIM_MODE: u32 = 0x0900_002A;
pub const RAT_DEATH_ANIM_MODE: u32 = 0x0900_0065;
pub const RAT_HIT_FORWARD_ANIM_SET: u32 = 0x8A00_000A;
pub const RAT_HIT_BACK_ANIM_SET: u32 = 0x8A00_000B;
pub const RAT_DEATH_ANIM_SET: u32 = 0x8A00_0008;
pub const RAT_HIT_FORWARD_SCRIPT: u32 = 0x8400_0009;
pub const RAT_HIT_BACK_SCRIPT: u32 = 0x8400_000A;
pub const RAT_DEATH_SCRIPT: u32 = 0x8400_000E;
pub const RAT_HIT_FORWARD_SIGNAL_FRAME: u16 = 80;
pub const RAT_HIT_BACK_SIGNAL_FRAME: u16 = 27;
pub const RAT_DEATH_SIGNAL_FRAME: u16 = 165;
pub const MISSILE_EVENT: u32 = 0x1600_001F;
pub const MISSILE_LIVE_BONE_FRAME: f32 = 16.5;
pub const MISSILE_RESOURCE_FILE: u32 = 0x0100_00BC;
pub const MISSILE_SCRIPT: u32 = 0x0400_02B0;
pub const MISSILE_LAUNCH_BONE: u32 = 0x0E00_0015;

pub const MONSTER_RUNTIME_TYPE: u32 = 5;
const MONSTER_SELECTORS: [u8; 6] = [0, 7, 9, 13, 14, 20];

pub const MONSTER_CREATE_RESOURCE: u32 = 0x0300_000E;
pub const MONSTER_PHYSICS_DESCRIPTOR: u32 = 0x005D_FC50;
/// `0x0047E6F0 -> 0x004E9A1C`: update-manager membership mask.
pub const MONSTER_UPDATE_REGISTRATION_MASK: u32 = 0x05;
pub const MONSTER_UPDATE_REGISTRATION_PRIORITY: u32 = 0x32;
/// `0x0047E6F0 -> 0x00444240`: native XItem registration flags for ordinary
/// runtime_type=5 Sweeper-spawned monsters.
pub const MONSTER_XITEM_REGISTRATION_FLAGS: u32 = 0x0000_9C02;

pub(crate) fn sweeper_ratchet_script_for_anim_mode(anim_mode: u32) -> Option<u32> {
    match anim_mode {
        APPEAR_ANIM_MODE => Some(APPEAR_SCRIPT),
        ATTACK2_ANIM_MODE => Some(ATTACK2_SCRIPT),
        DISAPPEAR_ANIM_MODE => Some(DISAPPEAR_SCRIPT),
        RAT_HIT_FORWARD_ANIM_MODE => Some(RAT_HIT_FORWARD_SCRIPT),
        RAT_HIT_BACK_ANIM_MODE => Some(RAT_HIT_BACK_SCRIPT),
        IDLE_ATTACK_ANIM_MODE => Some(IDLE_ATTACK_SCRIPT),
        JUMP_LEFT_ANIM_MODE => Some(JUMP_LEFT_SCRIPT),
        JUMP_RIGHT_ANIM_MODE => Some(JUMP_RIGHT_SCRIPT),
        RAT_DEATH_ANIM_MODE => Some(RAT_DEATH_SCRIPT),
        IDLE_COMBAT1_ANIM_MODE => Some(IDLE_COMBAT1_SCRIPT),
        IDLE_COMBAT2_ANIM_MODE => Some(IDLE_COMBAT2_SCRIPT),
        IDLE_COMBAT3_ANIM_MODE => Some(IDLE_COMBAT3_SCRIPT),
        ATTACK_ANIM_MODE => Some(ATTACK_SCRIPT),
        _ => None,
    }
}

const RAT_RUNTIME_SCRIPTS: [u32; 13] = [
    APPEAR_SCRIPT,
    ATTACK2_SCRIPT,
    DISAPPEAR_SCRIPT,
    RAT_HIT_FORWARD_SCRIPT,
    RAT_HIT_BACK_SCRIPT,
    IDLE_ATTACK_SCRIPT,
    JUMP_LEFT_SCRIPT,
    JUMP_RIGHT_SCRIPT,
    RAT_DEATH_SCRIPT,
    IDLE_COMBAT1_SCRIPT,
    IDLE_COMBAT2_SCRIPT,
    IDLE_COMBAT3_SCRIPT,
    ATTACK_SCRIPT,
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum NativeSweeperRatchetScriptEventKind {
    ScriptValue(f32),
    Missile,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeSweeperRatchetScriptEvent {
    pub start_frame: i16,
    pub length: u16,
    pub kind: NativeSweeperRatchetScriptEventKind,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeSweeperRatchetScriptProfile {
    pub script: u32,
    pub script_frame_rate: f32,
    pub length: u32,
    pub animation: u32,
    pub animation_frame_count: u16,
    pub animation_frame_rate: f32,
    pub events: Vec<NativeSweeperRatchetScriptEvent>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct RobotsSweeperRatchetScripts {
    profiles: BTreeMap<u32, NativeSweeperRatchetScriptProfile>,
}

impl RobotsSweeperRatchetScripts {
    pub(crate) fn read(edb: &mut EdbFile) -> anyhow::Result<Self> {
        let endian = edb.endian;
        let scripts = UXGeoScript::read_all(edb)?;
        let mut profiles = BTreeMap::new();
        for script in scripts {
            if !RAT_RUNTIME_SCRIPTS.contains(&script.hashcode) {
                continue;
            }
            let animation_hashcode = script
                .commands
                .iter()
                .find_map(|command| match command.data {
                    UXGeoScriptCommandData::Animation { anim_hashcode, .. } => Some(anim_hashcode),
                    _ => None,
                })
                .with_context(|| {
                    format!(
                        "Ratchet Script 0x{:08X} has no Animation command",
                        script.hashcode
                    )
                })?;
            let animation = edb
                .header
                .anim_list
                .iter()
                .find(|animation| animation.common.hashcode == animation_hashcode)
                .cloned()
                .with_context(|| {
                    format!(
                        "Ratchet Script 0x{:08X} Animation 0x{animation_hashcode:08X} is missing",
                        script.hashcode
                    )
                })?;
            let serialized_animation = animation.read_robots_v248_exgeoanim(edb, endian)?;

            let mut events = Vec::new();
            for command in &script.commands {
                let UXGeoScriptCommandData::Event { event_type, data } = &command.data else {
                    continue;
                };
                let kind = if *event_type == RAT_SCRIPT_VALUE_EVENT {
                    let Some(bytes) = data.get(4..8) else {
                        continue;
                    };
                    let bytes: [u8; 4] = bytes.try_into().expect("four-byte ScriptValue payload");
                    let value = match endian {
                        eurochef_edb::binrw::Endian::Little => f32::from_le_bytes(bytes),
                        eurochef_edb::binrw::Endian::Big => f32::from_be_bytes(bytes),
                    };
                    NativeSweeperRatchetScriptEventKind::ScriptValue(value)
                } else if *event_type == MISSILE_EVENT {
                    NativeSweeperRatchetScriptEventKind::Missile
                } else {
                    continue;
                };
                events.push(NativeSweeperRatchetScriptEvent {
                    start_frame: command.start,
                    length: command.length,
                    kind,
                });
            }
            profiles.insert(
                script.hashcode,
                NativeSweeperRatchetScriptProfile {
                    script: script.hashcode,
                    script_frame_rate: script.timeline_framerate(),
                    length: script.length,
                    animation: animation_hashcode,
                    animation_frame_count: serialized_animation.frame_count,
                    animation_frame_rate: f32::from(serialized_animation.raw_byte_0c),
                    events,
                },
            );
        }
        Ok(Self { profiles })
    }

    pub(crate) fn profile(&self, script: u32) -> Option<&NativeSweeperRatchetScriptProfile> {
        self.profiles.get(&script)
    }

    #[cfg(test)]
    pub(crate) fn profile_for_anim_mode(
        &self,
        anim_mode: u32,
    ) -> Option<&NativeSweeperRatchetScriptProfile> {
        self.profile(sweeper_ratchet_script_for_anim_mode(anim_mode)?)
    }

    pub(crate) fn len(&self) -> usize {
        self.profiles.len()
    }

    pub(crate) fn diagnostic_summary(&self) -> String {
        let event_count = self
            .profiles
            .values()
            .map(|profile| profile.events.len())
            .sum::<usize>();
        let idle_attack_span = self
            .profile(IDLE_ATTACK_SCRIPT)
            .map(|profile| {
                format!(
                    "IdleAttack Script{} / Animation{}",
                    profile.length, profile.animation_frame_count
                )
            })
            .unwrap_or_else(|| "IdleAttack unresolved".to_string());
        format!(
            "{} real nb11_rat family-4 profiles / {} relevant Events; Animation fps drives normalized phase at {:.0}Hz; previous<start<=current crossing, callback return ignored; {}",
            self.len(), event_count, RAT_SCRIPT_FIXED_HZ, idle_attack_span
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct NativeSweeperRatchetScriptStep {
    pub crossing_from: f32,
    pub crossing_to: f32,
    pub frame_after_advance: f32,
    pub script_value_status: Option<u8>,
    pub unsupported_script_value: Option<f32>,
    pub missile_event: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct NativeSweeperRatchetScriptRuntime {
    pub active_script: Option<u32>,
    pub previous_frame: f32,
    pub current_frame: f32,
    pub next_event_index: usize,
}

impl NativeSweeperRatchetScriptRuntime {
    pub(crate) fn restart(&mut self, script: u32) {
        self.active_script = Some(script);
        self.previous_frame = 0.0;
        self.current_frame = 0.0;
        self.next_event_index = 0;
    }

    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }

    /// Exact normal-forward family-4 Ratchet Event crossing for the shipped
    /// single-animation Script profiles.
    ///
    /// `HT_Script_*` AnimMode contributions use the family-4 wrapper vtable
    /// `0x005F4238`. Parent `EXItemAnimator_Anim::+0x10` reaches wrapper `+0x20
    /// = 0x0050E554`; `0x0050E57D -> 0x0050E8DD` dispatches opcode-11 Events
    /// against the *previous/current* normalized Script phase before
    /// `0x0050EF4C` advances that phase for the next update. The callback return
    /// is ignored on this path, unlike standalone `EXItemAnimator_Script`.
    ///
    /// For the shipped Ratchet corpus `0x0050CB79` selects one Animation
    /// controller, so `+0x70 / current_span` cancels. Native phase advance is
    /// therefore `animation_fps / 60 / (script_length - 1)`, while Event command
    /// coordinates are `phase * script_length`.
    pub(crate) fn step(
        &mut self,
        catalog: &RobotsSweeperRatchetScripts,
    ) -> NativeSweeperRatchetScriptStep {
        let from = self.previous_frame;
        let to = self.current_frame;
        let Some(script) = self.active_script else {
            return NativeSweeperRatchetScriptStep {
                crossing_from: from,
                crossing_to: to,
                frame_after_advance: to,
                ..Default::default()
            };
        };
        let Some(profile) = catalog.profile(script) else {
            return NativeSweeperRatchetScriptStep {
                crossing_from: from,
                crossing_to: to,
                frame_after_advance: to,
                ..Default::default()
            };
        };

        let mut step = NativeSweeperRatchetScriptStep {
            crossing_from: from,
            crossing_to: to,
            frame_after_advance: to,
            ..Default::default()
        };

        while let Some(event) = profile.events.get(self.next_event_index).copied() {
            let event_frame = f32::from(event.start_frame);
            if event_frame > to {
                break;
            }
            self.next_event_index += 1;
            if event_frame <= from {
                continue;
            }
            match event.kind {
                NativeSweeperRatchetScriptEventKind::Missile => step.missile_event = true,
                NativeSweeperRatchetScriptEventKind::ScriptValue(value) => {
                    if value == f32::from(RAT_SCRIPT_VALUE_SIGNAL_1) {
                        step.script_value_status = Some(RAT_SCRIPT_VALUE_SIGNAL_1);
                    } else if value == f32::from(RAT_SCRIPT_VALUE_SIGNAL_2) {
                        step.script_value_status = Some(RAT_SCRIPT_VALUE_SIGNAL_2);
                    } else {
                        step.unsupported_script_value = Some(value);
                    }
                }
            }
        }

        self.previous_frame = to;
        let length = profile.length as f32;
        if profile.length > 1 {
            let delta =
                (profile.animation_frame_rate / RAT_SCRIPT_FIXED_HZ) * (length / (length - 1.0));
            self.current_frame = (to + delta).min(length);
        }
        step.frame_after_advance = self.current_frame;
        step
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeSweeperBossSpawnSelection {
    NoSpawn,
    ActivateMonsterTransporter,
    Monster {
        config_index: u8,
        random_draws_used: u8,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeSweeperBossSpawnTransform {
    pub position: [f32; 4],
    pub yaw: f32,
}

pub(crate) fn sweeper_boss_spawn_selection(
    monster_id: i8,
    variant_random_mod100: u8,
) -> NativeSweeperBossSpawnSelection {
    if monster_id < 0 {
        return NativeSweeperBossSpawnSelection::NoSpawn;
    }
    let (config_index, random_draws_used) = match monster_id {
        0 => return NativeSweeperBossSpawnSelection::NoSpawn,
        1 => (0, 0),
        2 => {
            let selector = if variant_random_mod100 % 100 < 50 {
                13
            } else {
                14
            };
            (selector, 1)
        }
        3 => (9, 0),
        4 => (20, 0),
        5 => (7, 0),
        6 => return NativeSweeperBossSpawnSelection::ActivateMonsterTransporter,
        // The native helper's switch initializes the selector to zero and uses the
        // common Monster path for out-of-range positive values. Shipped Bo5_Final
        // pattern cells currently use only 0..6, but preserve the executable fallback.
        _ => (0, 0),
    };
    NativeSweeperBossSpawnSelection::Monster {
        config_index,
        random_draws_used,
    }
}

pub(crate) fn sweeper_boss_spawn_transform(
    eye_x: f32,
    eye_w: f32,
    eye_yaw: f32,
    spawn_ordinal: u32,
) -> NativeSweeperBossSpawnTransform {
    NativeSweeperBossSpawnTransform {
        position: [eye_x, 2.4, 35.0 + spawn_ordinal as f32 * 3.0, eye_w],
        yaw: eye_yaw,
    }
}

const SWEEPER_MALFBOT_FILE: u32 = 0x0100_0039;
const SWEEPER_ROLLERBOT_FILE: u32 = 0x0100_0046;
const SWEEPER_EW10_MINION_FILE: u32 = 0x0100_004f;
const SWEEPER_EB14_MINION_FILE: u32 = 0x0100_0056;
const SWEEPER_SHUNTBOT_BOSS_FILE: u32 = 0x0100_0021;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeSweeperBossSpawnOrigin {
    Eye {
        eye_ordinal: usize,
        spawn_ordinal: u32,
    },
    Transporter {
        target_trigger_index: usize,
        attempt_ordinal: u16,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeSweeperBossSpawnFactoryRngStep {
    MalfBot {
        origin: NativeSweeperBossSpawnOrigin,
        config_index: u8,
        random_mod90: u8,
    },
    RollerBot {
        origin: NativeSweeperBossSpawnOrigin,
        config_index: u8,
        setup_random_mod3: u8,
        config_random_mod1: u8,
    },
    StandardMonster {
        origin: NativeSweeperBossSpawnOrigin,
        config_index: u8,
        periodic_idle_offset: u8,
    },
}

impl NativeSweeperBossSpawnFactoryRngStep {
    fn random_draws_used(self) -> u32 {
        match self {
            Self::MalfBot { .. } | Self::StandardMonster { .. } => 1,
            Self::RollerBot { .. } => 2,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct NativeSweeperBossPreRatchetRngStep {
    pub variant_random_mod100_draws: Vec<u8>,
    pub factory_steps: Vec<NativeSweeperBossSpawnFactoryRngStep>,
    pub random_draws_used: u32,
}

fn sweeper_boss_consume_spawn_factory_rng(
    origin: NativeSweeperBossSpawnOrigin,
    config_index: u8,
    patterns: &RobotsSweeperBossPatterns,
    rng: &mut crate::map_runtime::RuntimeRobotsGlobalRngState,
) -> Option<NativeSweeperBossSpawnFactoryRngStep> {
    match patterns.monster_file(config_index) {
        Some(SWEEPER_MALFBOT_FILE) => Some(NativeSweeperBossSpawnFactoryRngStep::MalfBot {
            origin,
            config_index,
            random_mod90: (rng.next_u32()? % 90) as u8,
        }),
        Some(SWEEPER_ROLLERBOT_FILE) => Some(NativeSweeperBossSpawnFactoryRngStep::RollerBot {
            origin,
            config_index,
            setup_random_mod3: (rng.next_u32()? % 3) as u8,
            config_random_mod1: (rng.next_u32()? % 1) as u8,
        }),
        Some(SWEEPER_EW10_MINION_FILE | SWEEPER_EB14_MINION_FILE) => {
            Some(NativeSweeperBossSpawnFactoryRngStep::StandardMonster {
                origin,
                config_index,
                // AI_PeriodicIdle setup 0x004591C0 uses base180, therefore
                // exactly one constructor draw and stores 135 + draw%90.
                periodic_idle_offset: (rng.next_u32()? % 90) as u8,
            })
        }
        Some(SWEEPER_SHUNTBOT_BOSS_FILE) => {
            Some(NativeSweeperBossSpawnFactoryRngStep::StandardMonster {
                origin,
                config_index,
                // Shunt builder 0x0045CAF0 passes base120 into the same
                // AI_PeriodicIdle setup, which stores 90 + draw%60.
                periodic_idle_offset: (rng.next_u32()? % 60) as u8,
            })
        }
        Some(_) | None => None,
    }
}

/// Consume only the RNG that native Eye command execution uses before Ratchet is serviced.
/// Eligibility comes from owned command clocks, never an absolute tick. Each ready Eye executes
/// in ordinal order; each spawn consumes its optional variant selector first and then constructor
/// setup RNG for the concrete MonsterDatabase file. Unknown constructor RNG semantics fail closed.

pub(crate) fn sweeper_boss_consume_ready_eye_pre_ratchet_rng(
    eye_commands: &[NativeSweeperBossEyeCommandRuntime; EYE_COUNT],
    patterns: &RobotsSweeperBossPatterns,
    rng: &mut crate::map_runtime::RuntimeRobotsGlobalRngState,
) -> Option<NativeSweeperBossPreRatchetRngStep> {
    let mut next_rng = *rng;
    let mut step = NativeSweeperBossPreRatchetRngStep::default();

    for (eye_ordinal, runtime) in eye_commands.iter().enumerate() {
        let Some(command) = runtime.pending else {
            continue;
        };
        if !runtime
            .clock
            .is_some_and(|clock| clock.remaining_updates <= 0.0)
        {
            continue;
        }
        for spawn_ordinal in 0..command.spawn_count.max(0) as u32 {
            let variant_random_mod100 = if command.monster_id == 2 {
                let value = (next_rng.next_u32()? % 100) as u8;
                step.variant_random_mod100_draws.push(value);
                step.random_draws_used = step.random_draws_used.wrapping_add(1);
                value
            } else {
                0
            };
            let NativeSweeperBossSpawnSelection::Monster { config_index, .. } =
                sweeper_boss_spawn_selection(command.monster_id, variant_random_mod100)
            else {
                continue;
            };
            let factory = sweeper_boss_consume_spawn_factory_rng(
                NativeSweeperBossSpawnOrigin::Eye {
                    eye_ordinal,
                    spawn_ordinal,
                },
                config_index,
                patterns,
                &mut next_rng,
            )?;
            step.random_draws_used = step
                .random_draws_used
                .wrapping_add(factory.random_draws_used());
            step.factory_steps.push(factory);
        }
    }

    *rng = next_rng;
    Some(step)
}

const ROLLERBOT_PROXIMITY_RADIUS: f32 = 60.0;
const ROLLERBOT_STATE0_DAMPING: f32 = f32::from_bits(0x3F7C_28F6);
const ROLLERBOT_SPEED: f32 = 10.0;
const ROLLERBOT_TURN_SPEED: f32 = f32::from_bits(0x4116_CBE4);
const ROLLERBOT_PATH_ARRIVAL_RADIUS: f32 = 1.0;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeSweeperRollerbotPathGraph {
    pub node_positions: Vec<[f32; 3]>,
    pub links: Vec<(usize, usize)>,
}

impl NativeSweeperRollerbotPathGraph {
    pub(crate) fn new(node_positions: &[[f32; 3]], links: &[(usize, usize)]) -> Option<Self> {
        if node_positions.is_empty()
            || links
                .iter()
                .any(|&(a, b)| a >= node_positions.len() || b >= node_positions.len() || a == b)
        {
            return None;
        }
        Some(Self {
            node_positions: node_positions.to_vec(),
            links: links.to_vec(),
        })
    }
}

/// Generic native EB10 fallback-path state. The path graph is data; this runtime only owns
/// the behavior state that `0x0046BFA0/0x0046C490` advances every fixed update.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeSweeperRollerbotPathRuntime {
    pub owner_position: [f32; 3],
    pub yaw: f32,
    pub accumulator_x: f32,
    pub accumulator_z: f32,
    pub current_node: usize,
    pub arrivals: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeSweeperRollerbotPathStep {
    pub committed: bool,
    pub arrived: bool,
    pub needs_neighbor_rng: bool,
    pub neighbor_count: usize,
    pub reached_node: Option<usize>,
    pub selected_next_node: Option<usize>,
    pub distance_to_target: f32,
    pub random_draws_used: u8,
}

fn sweeper_rollerbot_path_neighbor_count(node: usize, links: &[(usize, usize)]) -> usize {
    links
        .iter()
        .filter(|&&(a, b)| a == node || b == node)
        .count()
}

fn sweeper_rollerbot_path_neighbor_at(
    node: usize,
    links: &[(usize, usize)],
    ordinal: usize,
) -> Option<usize> {
    links
        .iter()
        .filter_map(|&(a, b)| {
            if a == node {
                Some(b)
            } else if b == node {
                Some(a)
            } else {
                None
            }
        })
        .nth(ordinal)
}

impl NativeSweeperRollerbotPathRuntime {
    /// Advance one native fixed update. If this update reaches a node and that node has
    /// neighbors, `None` returns an uncommitted preview. The caller then consumes exactly one
    /// process-global RNG draw and retries with `Some(draw)`. Arrival timing is therefore a
    /// consequence of motion state, not an absolute-tick oracle.
    pub(crate) fn step(
        &mut self,
        node_positions: &[[f32; 3]],
        links: &[(usize, usize)],
        neighbor_random_draw: Option<u32>,
    ) -> Option<NativeSweeperRollerbotPathStep> {
        let target = *node_positions.get(self.current_node)?;
        let desired_yaw =
            (target[0] - self.owner_position[0]).atan2(target[2] - self.owner_position[2]);
        let mut delta = desired_yaw - self.yaw;
        while delta > std::f32::consts::PI {
            delta -= std::f32::consts::TAU;
        }
        while delta < -std::f32::consts::PI {
            delta += std::f32::consts::TAU;
        }

        let max_yaw_step = ROLLERBOT_TURN_SPEED / RAT_SCRIPT_FIXED_HZ;
        let speed_step = ROLLERBOT_SPEED / RAT_SCRIPT_FIXED_HZ;
        let yaw = self.yaw + delta.clamp(-max_yaw_step, max_yaw_step);
        let accumulator_x =
            ROLLERBOT_STATE0_DAMPING * (self.accumulator_x + yaw.sin() * speed_step);
        let accumulator_z =
            ROLLERBOT_STATE0_DAMPING * (self.accumulator_z + yaw.cos() * speed_step);
        let owner_position = [
            self.owner_position[0] + accumulator_x / RAT_SCRIPT_FIXED_HZ,
            self.owner_position[1],
            self.owner_position[2] + accumulator_z / RAT_SCRIPT_FIXED_HZ,
        ];
        let dx = target[0] - owner_position[0];
        let dy = target[1] - owner_position[1];
        let dz = target[2] - owner_position[2];
        let distance_to_target = (dx * dx + dy * dy + dz * dz).sqrt();
        let arrived = distance_to_target < ROLLERBOT_PATH_ARRIVAL_RADIUS;
        let neighbor_count = if arrived {
            sweeper_rollerbot_path_neighbor_count(self.current_node, links)
        } else {
            0
        };

        if arrived && neighbor_count != 0 && neighbor_random_draw.is_none() {
            return Some(NativeSweeperRollerbotPathStep {
                committed: false,
                arrived: true,
                needs_neighbor_rng: true,
                neighbor_count,
                reached_node: Some(self.current_node),
                selected_next_node: None,
                distance_to_target,
                random_draws_used: 0,
            });
        }

        let reached_node = arrived.then_some(self.current_node);
        let selected_next_node = if arrived && neighbor_count != 0 {
            let ordinal = (neighbor_random_draw? as usize) % neighbor_count;
            Some(sweeper_rollerbot_path_neighbor_at(
                self.current_node,
                links,
                ordinal,
            )?)
        } else {
            None
        };

        self.owner_position = owner_position;
        self.yaw = yaw;
        self.accumulator_x = accumulator_x;
        self.accumulator_z = accumulator_z;
        if let Some(next) = selected_next_node {
            self.current_node = next;
            self.arrivals = self.arrivals.wrapping_add(1);
        }

        Some(NativeSweeperRollerbotPathStep {
            committed: true,
            arrived,
            needs_neighbor_rng: false,
            neighbor_count,
            reached_node,
            selected_next_node,
            distance_to_target,
            random_draws_used: u8::from(selected_next_node.is_some()),
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct NativeSweeperBossPatternCell {
    pub monster_id: i8,
    pub spawn_count: i8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeSweeperBossEyeCommand {
    pub group_index: usize,
    pub eye_ordinal: usize,
    pub monster_id: i8,
    pub spawn_count: i8,
    pub countdown_seed: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct RobotsSweeperBossPatterns {
    pub rows: Vec<[NativeSweeperBossPatternCell; EYE_COUNT]>,
    monster_files_by_selector: BTreeMap<u8, u32>,
    monster_pickup_drop_counts_by_selector: BTreeMap<u8, u8>,
}

impl RobotsSweeperBossPatterns {
    pub(crate) fn read(edb: &mut EdbFile) -> anyhow::Result<Self> {
        let spreadsheets = UXGeoSpreadsheet::read_all(edb)?;
        let (_, spreadsheet) = spreadsheets
            .into_iter()
            .find(|(hashcode, _)| *hashcode == PATTERN_SHEET)
            .context("HT_SpreadSheet_SweeperBossPatterns is missing")?;
        let UXGeoSpreadsheet::Data(sheets) = spreadsheet else {
            bail!("HT_SpreadSheet_SweeperBossPatterns is not a data spreadsheet");
        };
        if sheets.len() != 1 {
            bail!(
                "HT_SpreadSheet_SweeperBossPatterns expected 1 sheet, got {}",
                sheets.len()
            );
        }
        let sheet = &sheets[0];
        if sheet.row_count as usize != PATTERN_ROW_COUNT {
            bail!(
                "HT_SpreadSheet_SweeperBossPatterns expected {} rows, got {}",
                PATTERN_ROW_COUNT,
                sheet.row_count
            );
        }

        let endian = edb.endian;
        let mut rows = Vec::with_capacity(PATTERN_ROW_COUNT);
        edb.seek(SeekFrom::Start(sheet.address as u64))?;
        for _ in 0..PATTERN_ROW_COUNT {
            let mut row = [NativeSweeperBossPatternCell::default(); EYE_COUNT];
            for cell in &mut row {
                cell.monster_id = edb.read_type::<i8>(endian)?;
                cell.spawn_count = edb.read_type::<i8>(endian)?;
            }
            rows.push(row);
        }
        Ok(Self {
            rows,
            monster_files_by_selector: BTreeMap::new(),
            monster_pickup_drop_counts_by_selector: BTreeMap::new(),
        })
    }

    pub(crate) fn set_monster_file(&mut self, config_index: u8, file: u32) {
        self.monster_files_by_selector.insert(config_index, file);
    }

    pub(crate) fn set_monster_pickup_drop_count(&mut self, config_index: u8, count: u8) {
        self.monster_pickup_drop_counts_by_selector
            .insert(config_index, count);
    }

    pub(crate) fn monster_file(&self, config_index: u8) -> Option<u32> {
        self.monster_files_by_selector.get(&config_index).copied()
    }

    pub(crate) fn monster_pickup_drop_count(&self, config_index: u8) -> Option<u8> {
        self.monster_pickup_drop_counts_by_selector
            .get(&config_index)
            .copied()
    }

    pub(crate) fn monster_selectors() -> &'static [u8] {
        &MONSTER_SELECTORS
    }

    pub(crate) fn group_index(difficulty: u8, random_mod5: u8) -> usize {
        (usize::from(difficulty) * 5 + usize::from(random_mod5 % 5)).min(PATTERN_ROW_COUNT - 1)
    }

    pub(crate) fn command_for_eye(
        &self,
        difficulty: u8,
        random_mod5: u8,
        eye_ordinal: usize,
    ) -> Option<NativeSweeperBossEyeCommand> {
        if eye_ordinal >= EYE_COUNT {
            return None;
        }
        let group_index = Self::group_index(difficulty, random_mod5);
        let cell = *self.rows.get(group_index)?.get(eye_ordinal)?;
        if cell.monster_id < 0 || cell.spawn_count <= 0 {
            return None;
        }
        Some(NativeSweeperBossEyeCommand {
            group_index,
            eye_ordinal,
            monster_id: cell.monster_id,
            spawn_count: cell.spawn_count,
            countdown_seed: eye_ordinal as f32 * EYE_PATTERN_STAGGER_UPDATES,
        })
    }
}

// Boss pattern scheduler state mirrors the native Eye/XItem-manager gate.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct NativeSweeperBossPatternEyeState {
    pub main_state: i32,
    pub command_state: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeSweeperBossPatternSchedulerState {
    pub gate_counter: f32,
}

impl Default for NativeSweeperBossPatternSchedulerState {
    fn default() -> Self {
        Self { gate_counter: 0.0 }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativeSweeperBossPatternSchedulerInput {
    pub difficulty: u8,
    pub random_mod5: u8,
    pub live_ai_character_count: u32,
    pub transporter_active: bool,
    pub eyes: [NativeSweeperBossPatternEyeState; EYE_COUNT],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeSweeperBossPatternSchedulerStep {
    pub selected_group: Option<usize>,
    pub commands: [Option<NativeSweeperBossEyeCommand>; EYE_COUNT],
    pub random_draws_used: u8,
}

impl NativeSweeperBossPatternSchedulerState {
    pub(crate) fn step(
        &mut self,
        patterns: &RobotsSweeperBossPatterns,
        input: NativeSweeperBossPatternSchedulerInput,
    ) -> NativeSweeperBossPatternSchedulerStep {
        let empty = || NativeSweeperBossPatternSchedulerStep {
            selected_group: None,
            commands: [None; EYE_COUNT],
            random_draws_used: 0,
        };

        if input
            .eyes
            .iter()
            .any(|eye| eye.main_state != 1 || eye.command_state == 3)
        {
            return empty();
        }

        let combined_live_count = input
            .live_ai_character_count
            .wrapping_add(u32::from(input.transporter_active));
        if combined_live_count > 1 {
            return empty();
        }

        self.gate_counter += 1.0;
        if self.gate_counter <= 0.0 {
            return empty();
        }

        let group = RobotsSweeperBossPatterns::group_index(input.difficulty, input.random_mod5);
        let mut commands = [None; EYE_COUNT];
        for (eye_ordinal, command) in commands.iter_mut().enumerate() {
            *command = patterns.command_for_eye(input.difficulty, input.random_mod5, eye_ordinal);
        }
        self.gate_counter = 0.0;
        NativeSweeperBossPatternSchedulerStep {
            selected_group: Some(group),
            commands,
            random_draws_used: 1,
        }
    }
}

// Exact raw per-eye pending-command countdown before 0x004CF1D0 dispatch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeSweeperBossEyeCommandClock {
    pub remaining_updates: f32,
}

impl NativeSweeperBossEyeCommandClock {
    pub(crate) fn step(&mut self) -> bool {
        if self.remaining_updates > 0.0 {
            self.remaining_updates -= 1.0;
            false
        } else {
            true
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct NativeSweeperBossEyeSpawnSource {
    pub owner_position: [f32; 4],
    pub owner_yaw: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeSweeperBossEyeCommandRuntime {
    pub command_state: i32,
    pub pending: Option<NativeSweeperBossEyeCommand>,
    pub clock: Option<NativeSweeperBossEyeCommandClock>,
}

impl Default for NativeSweeperBossEyeCommandRuntime {
    fn default() -> Self {
        Self {
            // XItemHandler_Sweeper_Boss_Eye ctor 0x004CD6D0 writes +0x3F4 = 1.
            // 0x004CEE10 later clears this lane to zero when a command actually executes.
            command_state: 1,
            pending: None,
            clock: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct NativeSweeperBossEyeCommandStep {
    pub command_completed: bool,
    pub spawn_requests: u32,
    pub spawned_ai_count: u32,
    pub transporter_activation_requests: u32,
    pub random_draws_used: u8,
    pub needs_more_spawn_rng: bool,
}

impl NativeSweeperBossEyeCommandRuntime {
    pub(crate) fn assign(&mut self, command: NativeSweeperBossEyeCommand) {
        self.command_state = 3;
        self.clock = Some(NativeSweeperBossEyeCommandClock {
            remaining_updates: command.countdown_seed,
        });
        self.pending = Some(command);
    }

    pub(crate) fn step(
        &mut self,
        patterns: &RobotsSweeperBossPatterns,
        live_ai: &mut NativeSweeperBossLiveAiRegistry,
        source: NativeSweeperBossEyeSpawnSource,
        variant_random_mod100_draws: &[u8],
    ) -> NativeSweeperBossEyeCommandStep {
        let Some(command) = self.pending else {
            return NativeSweeperBossEyeCommandStep::default();
        };
        let Some(clock) = self.clock.as_mut() else {
            return NativeSweeperBossEyeCommandStep::default();
        };
        if !clock.step() {
            return NativeSweeperBossEyeCommandStep::default();
        }

        let spawn_count = command.spawn_count.max(0) as u32;
        let required_random_draws = if command.monster_id == 2 {
            spawn_count as usize
        } else {
            0
        };
        if variant_random_mod100_draws.len() < required_random_draws {
            return NativeSweeperBossEyeCommandStep {
                needs_more_spawn_rng: true,
                ..Default::default()
            };
        }

        // 0x004CEE10 clears +0x3F4 before the repeated 0x004CF1D0 calls.
        self.command_state = 0;
        self.pending = None;
        self.clock = None;

        let mut step = NativeSweeperBossEyeCommandStep {
            command_completed: true,
            ..Default::default()
        };
        let mut random_index = 0usize;
        for spawn_ordinal in 0..spawn_count {
            // Native increments controller+0x110 once before every requested
            // 0x004CF1D0 call, including NoSpawn/Transporter selectors.
            step.spawn_requests = step.spawn_requests.wrapping_add(1);
            let random = if command.monster_id == 2 {
                let value = variant_random_mod100_draws[random_index];
                random_index += 1;
                value
            } else {
                0
            };
            match sweeper_boss_spawn_selection(command.monster_id, random) {
                NativeSweeperBossSpawnSelection::NoSpawn => {}
                NativeSweeperBossSpawnSelection::ActivateMonsterTransporter => {
                    step.transporter_activation_requests =
                        step.transporter_activation_requests.wrapping_add(1);
                }
                NativeSweeperBossSpawnSelection::Monster {
                    config_index,
                    random_draws_used,
                } => {
                    step.random_draws_used = step.random_draws_used.wrapping_add(random_draws_used);
                    let transform = sweeper_boss_spawn_transform(
                        source.owner_position[0],
                        source.owner_position[3],
                        source.owner_yaw,
                        spawn_ordinal,
                    );
                    live_ai.register_spawn(patterns, config_index, transform);
                    step.spawned_ai_count = step.spawned_ai_count.wrapping_add(1);
                }
            }
        }
        step
    }
}

// Eye-pressure reducer tracks native spawn pressure and open-eye selection.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct NativeSweeperBossEyePressureState {
    pub spawn_request_count: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativeSweeperBossEyePressureStep {
    pub selected_eye: Option<usize>,
    pub random_draws_used: u8,
    pub counter_reset: bool,
}

impl NativeSweeperBossEyePressureState {
    pub(crate) fn step(
        &mut self,
        eye_states: [i32; EYE_COUNT],
        random_eye_index: u32,
    ) -> NativeSweeperBossEyePressureStep {
        let idle = || NativeSweeperBossEyePressureStep {
            selected_eye: None,
            random_draws_used: 0,
            counter_reset: false,
        };
        if self.spawn_request_count <= EYE_PRESSURE_THRESHOLD
            || eye_states
                .iter()
                .any(|state| *state != EYE_STATE_CLOSED && *state != EYE_STATE_DESTROYED)
        {
            return idle();
        }

        self.spawn_request_count = 0;
        let selected = random_eye_index as usize % EYE_COUNT;
        let selected_eye = if eye_states[selected] != EYE_STATE_DESTROYED {
            Some(selected)
        } else {
            (1..=EYE_COUNT)
                .map(|offset| (selected + offset) % EYE_COUNT)
                .find(|index| eye_states[*index] != EYE_STATE_DESTROYED)
        };
        NativeSweeperBossEyePressureStep {
            selected_eye,
            random_draws_used: 1,
            counter_reset: true,
        }
    }
}

// Script completion signal is external until the eye Script XItem is replayed live.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeSweeperBossEyeLifecycle {
    pub state: i32,
    pub request: i32,
    pub script_signal: i32,
    pub timer_seconds: f32,
    pub hit_points: u8,
}

impl Default for NativeSweeperBossEyeLifecycle {
    fn default() -> Self {
        Self {
            state: EYE_STATE_CLOSED,
            request: 1,
            script_signal: 0,
            timer_seconds: 0.0,
            hit_points: EYE_INITIAL_HIT_POINTS,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct NativeSweeperBossEyeLifecycleStep {
    pub script: Option<u32>,
}

impl NativeSweeperBossEyeLifecycle {
    pub(crate) fn request_open(&mut self) {
        self.request = EYE_OPEN_REQUEST;
        self.timer_seconds = EYE_OPEN_SECONDS;
    }

    pub(crate) fn step(&mut self) -> NativeSweeperBossEyeLifecycleStep {
        let mut script = None;
        match self.state {
            EYE_STATE_CLOSED => {
                if self.request == EYE_OPEN_REQUEST {
                    self.state = EYE_STATE_OPENING;
                    self.script_signal = 0;
                    script = Some(EYE_SCRIPT_OPENING);
                }
            }
            EYE_STATE_OPENING => {
                if self.script_signal == 1 {
                    self.state = EYE_STATE_OPEN;
                    self.request = 0;
                    self.script_signal = 0;
                    script = Some(EYE_SCRIPT_OPEN);
                }
            }
            EYE_STATE_OPEN => {
                if self.timer_seconds > 0.0 {
                    self.timer_seconds -= 1.0 / 60.0;
                } else {
                    self.state = EYE_STATE_CLOSING;
                    self.script_signal = 0;
                    script = Some(EYE_SCRIPT_CLOSING);
                }
            }
            EYE_STATE_CLOSING => {
                if self.script_signal == 1 {
                    self.state = EYE_STATE_CLOSED;
                    self.script_signal = 0;
                    script = Some(EYE_SCRIPT_CLOSED);
                }
            }
            EYE_STATE_HIT => {
                if self.script_signal == 1 {
                    self.state = EYE_STATE_CLOSING;
                    self.script_signal = 0;
                    script = Some(EYE_SCRIPT_CLOSING);
                }
            }
            _ => {}
        }
        NativeSweeperBossEyeLifecycleStep { script }
    }
}

// Health scheduler mirrors the native pickup timer and player-health gate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeSweeperBossHealthSchedulerState {
    pub timer_seconds: f32,
}

impl Default for NativeSweeperBossHealthSchedulerState {
    fn default() -> Self {
        Self {
            timer_seconds: HEALTH_PICKUP_TIMER_INITIAL_SECONDS,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeSweeperBossHealthSchedulerInput {
    pub pickup_active: bool,
    pub player_handler_present: bool,
    pub player_current_health: f32,
    pub player_max_health: f32,
    /// Native FUN_00509C6E() result used only when an expired timer is re-armed.
    pub timer_random_unit: f32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct NativeSweeperBossHealthSchedulerStep {
    pub timer_reset: bool,
    pub spawn_requested: bool,
    pub random_draws_used: u8,
}

impl NativeSweeperBossHealthSchedulerState {
    pub(crate) fn step(
        &mut self,
        input: NativeSweeperBossHealthSchedulerInput,
    ) -> NativeSweeperBossHealthSchedulerStep {
        self.timer_seconds -= 1.0 / 60.0;
        if self.timer_seconds >= 0.0 || input.pickup_active {
            return NativeSweeperBossHealthSchedulerStep::default();
        }

        self.timer_seconds = HEALTH_PICKUP_TIMER_INITIAL_SECONDS
            + input.timer_random_unit * HEALTH_PICKUP_TIMER_JITTER_SECONDS;
        NativeSweeperBossHealthSchedulerStep {
            timer_reset: true,
            spawn_requested: input.player_handler_present
                && input.player_current_health < input.player_max_health,
            random_draws_used: 1,
        }
    }
}

// This plan starts only after 0x004CE690 successfully creates the health XItem.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeSweeperBossHealthPickupSpawnPlan {
    pub script: u32,
    pub item: u32,
    pub registration_mask: u32,
    pub position: [f32; 4],
    pub random_draws_used: u8,
}

pub(crate) fn sweeper_boss_health_pickup_spawn_plan(
    spawn_random_unit: f32,
) -> NativeSweeperBossHealthPickupSpawnPlan {
    NativeSweeperBossHealthPickupSpawnPlan {
        script: HEALTH_PICKUP_SCRIPT,
        item: HEALTH_PICKUP_ITEM,
        registration_mask: HEALTH_PICKUP_REGISTRATION_MASK,
        position: [
            spawn_random_unit * HEALTH_PICKUP_SPAWN_X_SCALE - HEALTH_PICKUP_SPAWN_X_BIAS,
            0.0,
            HEALTH_PICKUP_SPAWN_Z,
            0.0,
        ],
        random_draws_used: 1,
    }
}

// Valid-hit input remains external until the native collision/contact list is live.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeSweeperBossHealthPickupRuntime {
    pub remaining_seconds: f32,
}

impl Default for NativeSweeperBossHealthPickupRuntime {
    fn default() -> Self {
        Self {
            remaining_seconds: HEALTH_PICKUP_LIFETIME_SECONDS,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeSweeperBossHealthPickupUpdateStep {
    pub rotation_y_delta: f32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct NativeSweeperBossHealthPickupInventoryStep {
    pub clear_controller_backref: bool,
    pub add_health_replenish: bool,
    pub item: u32,
    pub quantity: u32,
}

impl NativeSweeperBossHealthPickupRuntime {
    pub(crate) fn step_update(&mut self) -> NativeSweeperBossHealthPickupUpdateStep {
        self.remaining_seconds -= 1.0 / 60.0;
        NativeSweeperBossHealthPickupUpdateStep {
            rotation_y_delta: HEALTH_PICKUP_ROTATION_PER_UPDATE,
        }
    }

    /// True means the Script Event must remain paused on HT_ScriptEvents_WaitForHit.
    pub(crate) fn wait_for_hit_holds_timeline(&self, valid_hit: bool) -> bool {
        self.remaining_seconds >= 0.0 && !valid_hit
    }

    /// InventoryAdd always releases controller ownership. The actual inventory add is
    /// suppressed after the ten-second gate expires.
    pub(crate) fn inventory_add_step(&self) -> NativeSweeperBossHealthPickupInventoryStep {
        NativeSweeperBossHealthPickupInventoryStep {
            clear_controller_backref: true,
            add_health_replenish: self.remaining_seconds >= 0.0,
            item: HEALTH_PICKUP_ITEM,
            quantity: 1,
        }
    }
}

// Exact Player inventory callback; live InventoryAdd status is not fabricated by Maps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeSweeperBossOwnedHealthPickupRuntime {
    pub spawn_plan: NativeSweeperBossHealthPickupSpawnPlan,
    pub handler: NativeSweeperBossHealthPickupRuntime,
    pub script_frame: u8,
    pub controller_owned: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct NativeSweeperBossOwnedHealthPickupStep {
    pub script_frame_before: u8,
    pub script_frame_after: u8,
    pub remaining_seconds: f32,
    pub wait_for_hit_held: bool,
    pub inventory_add_requested: bool,
    pub controller_ownership_cleared: bool,
    pub completed: bool,
}

impl NativeSweeperBossOwnedHealthPickupRuntime {
    pub(crate) fn new(spawn_plan: NativeSweeperBossHealthPickupSpawnPlan) -> Self {
        Self {
            spawn_plan,
            handler: NativeSweeperBossHealthPickupRuntime::default(),
            script_frame: 0,
            controller_owned: true,
        }
    }

    /// Native XItem+0x28 runs Handler before the attached EXItemAnimator_Script.
    /// The Script clock advances exactly one serialized frame per fixed XItem update.
    /// A live WaitForHit requires an exact contact observation; expiry releases it without one.
    pub(crate) fn step_fixed_update(
        &mut self,
        valid_hit: Option<bool>,
    ) -> Option<NativeSweeperBossOwnedHealthPickupStep> {
        let mut next = *self;
        let frame_before = next.script_frame;
        let _handler = next.handler.step_update();
        let mut step = NativeSweeperBossOwnedHealthPickupStep {
            script_frame_before: frame_before,
            script_frame_after: frame_before,
            remaining_seconds: next.handler.remaining_seconds,
            ..Default::default()
        };

        match frame_before {
            HEALTH_PICKUP_WAIT_FOR_HIT_FRAME => {
                let valid_hit = if next.handler.remaining_seconds >= 0.0 {
                    valid_hit?
                } else {
                    false
                };
                if next.handler.wait_for_hit_holds_timeline(valid_hit) {
                    step.wait_for_hit_held = true;
                } else {
                    next.script_frame = next.script_frame.saturating_add(1);
                }
            }
            HEALTH_PICKUP_INVENTORY_ADD_FRAME => {
                let inventory = next.handler.inventory_add_step();
                step.inventory_add_requested = inventory.add_health_replenish;
                if inventory.clear_controller_backref && next.controller_owned {
                    next.controller_owned = false;
                    step.controller_ownership_cleared = true;
                }
                next.script_frame = next.script_frame.saturating_add(1);
            }
            _ if next.script_frame < HEALTH_PICKUP_SCRIPT_LENGTH => {
                next.script_frame = next.script_frame.saturating_add(1);
            }
            _ => {}
        }

        step.script_frame_after = next.script_frame;
        step.completed = next.script_frame >= HEALTH_PICKUP_SCRIPT_LENGTH;
        *self = next;
        Some(step)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeSweeperBossControllerSnapshot {
    pub difficulty: u8,
    pub eye_hit_points: [u8; EYE_COUNT],
}

impl Default for NativeSweeperBossControllerSnapshot {
    fn default() -> Self {
        Self {
            difficulty: INITIAL_DIFFICULTY,
            eye_hit_points: [INITIAL_EYE_STATUS; EYE_COUNT],
        }
    }
}

// Transform an Eye-local anchor into the Ratchet owner-space position used by native logic.
pub(crate) fn sweeper_ratchet_anchor(
    eye_position: [f32; 3],
    eye_rotation: [f32; 3],
    local_center: [f32; 3],
) -> [f32; 4] {
    let (sin_x, cos_x) = eye_rotation[0].sin_cos();
    let (sin_y, cos_y) = eye_rotation[1].sin_cos();
    let (sin_z, cos_z) = eye_rotation[2].sin_cos();

    // Robots.exe 0x00506353 rotation-order 4, followed by 0x004E8708 point transform.
    let m00 = sin_x * sin_z * sin_y + cos_z * cos_y;
    let m01 = sin_z * cos_x;
    let m02 = sin_x * sin_z * cos_y - cos_z * sin_y;
    let m10 = sin_x * cos_z * sin_y - sin_z * cos_y;
    let m11 = cos_z * cos_x;
    let m12 = sin_x * cos_z * cos_y + sin_z * sin_y;
    let m20 = sin_y * cos_x;
    let m21 = -sin_x;
    let m22 = cos_y * cos_x;

    [
        local_center[0] * m00 + local_center[1] * m10 + local_center[2] * m20 + eye_position[0],
        local_center[0] * m01 + local_center[1] * m11 + local_center[2] * m21 + eye_position[1],
        local_center[0] * m02 + local_center[1] * m12 + local_center[2] * m22 + eye_position[2],
        0.0,
    ]
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeSweeperRatchetRelocationStep {
    pub anchor: [f32; 4],
    pub next_current_eye: u8,
    pub requested_anim_mode: Option<u32>,
    /// Native relocation helper 0x004CFCC0 dispatches jump AnimModes through
    /// Ratchet Handler vslot +0xE4 = 0x00405210, which sets force flag +0x484.
    /// This matters when two consecutive relocation steps both use JumpLeft/Right.
    pub force_anim_mode_reapply: bool,
    pub requested_yaw: Option<f32>,
    pub latch: bool,
    pub needs_new_target: bool,
}

pub(crate) fn sweeper_ratchet_relocation_step(
    current_eye: u8,
    target_eye: u8,
    anchors: [[f32; 4]; EYE_COUNT],
) -> Option<NativeSweeperRatchetRelocationStep> {
    let current = usize::from(current_eye);
    if current >= EYE_COUNT || usize::from(target_eye) >= EYE_COUNT {
        return None;
    }
    let anchor = anchors[current];
    if current_eye == target_eye {
        return Some(NativeSweeperRatchetRelocationStep {
            anchor,
            next_current_eye: current_eye,
            requested_anim_mode: None,
            force_anim_mode_reapply: false,
            requested_yaw: None,
            latch: true,
            needs_new_target: true,
        });
    }

    let (next_current_eye, requested_anim_mode, requested_yaw) = if current_eye < target_eye {
        (current_eye + 1, JUMP_RIGHT_ANIM_MODE, RAT_JUMP_RIGHT_YAW)
    } else {
        (current_eye - 1, JUMP_LEFT_ANIM_MODE, RAT_JUMP_LEFT_YAW)
    };
    Some(NativeSweeperRatchetRelocationStep {
        anchor,
        next_current_eye,
        requested_anim_mode: Some(requested_anim_mode),
        force_anim_mode_reapply: true,
        requested_yaw: Some(requested_yaw),
        latch: true,
        needs_new_target: false,
    })
}

pub(crate) fn sweeper_ratchet_target_from_draws(
    current_eye: u8,
    draws: &[u32],
) -> Option<(u8, u8)> {
    if usize::from(current_eye) >= EYE_COUNT {
        return None;
    }
    for (index, draw) in draws.iter().copied().enumerate() {
        let candidate = (draw % EYE_COUNT as u32) as u8;
        if candidate != current_eye {
            return Some((candidate, (index + 1) as u8));
        }
    }
    None
}

pub(crate) fn sweeper_ratchet_native_angle_lerp(current: f32, target: f32, alpha: f32) -> f32 {
    let mut base = current;
    let delta = target - base;
    if delta >= -std::f32::consts::PI {
        if delta > std::f32::consts::PI {
            base += std::f32::consts::TAU;
        }
    } else {
        base -= std::f32::consts::TAU;
    }

    let value = (target - base) * alpha + base;
    if value.abs() < std::f32::consts::PI {
        return value;
    }
    let wrapped = (value as f64 % std::f64::consts::TAU) as f32;
    if wrapped.abs() < std::f32::consts::PI {
        return wrapped;
    }
    if value >= 0.0 {
        wrapped - std::f32::consts::TAU
    } else {
        wrapped + std::f32::consts::TAU
    }
}

pub(crate) fn sweeper_ratchet_face_player_yaw(
    current_yaw: f32,
    ratchet_position: [f32; 4],
    player_position: [f32; 4],
) -> f32 {
    let target =
        (player_position[0] - ratchet_position[0]).atan2(player_position[2] - ratchet_position[2]);
    sweeper_ratchet_native_angle_lerp(current_yaw, target, RAT_FACE_PLAYER_ALPHA)
}

// Pure XItem owner transform for the Ratchet R_Hand missile launch query.
pub(crate) fn sweeper_ratchet_missile_launch_position(
    ratchet_position: [f32; 4],
    ratchet_yaw: f32,
    local_hand_position: [f32; 3],
) -> [f32; 4] {
    sweeper_ratchet_anchor(
        [
            ratchet_position[0],
            ratchet_position[1],
            ratchet_position[2],
        ],
        [0.0, ratchet_yaw, 0.0],
        local_hand_position,
    )
}

/// Exact no-spread ballistic setup used by the first retained Ratchet projectile.
pub(crate) fn sweeper_ratchet_projectile_initial_velocity(
    launch: [f32; 4],
    target: [f32; 4],
) -> Option<[f32; 4]> {
    if launch
        .iter()
        .chain(target.iter())
        .any(|value| !value.is_finite())
    {
        return None;
    }
    let dx = target[0] - launch[0];
    let dy = target[1] - launch[1];
    let dz = target[2] - launch[2];
    let horizontal_distance = (dx * dx + dz * dz).sqrt();
    let dt = 1.0 / RAT_SCRIPT_FIXED_HZ;
    let apex_height = dy + RAT_PROJECTILE_APEX_EXTRA_HEIGHT;
    let vertical_velocity = if apex_height < 0.0 {
        0.0
    } else {
        (2.0 * RAT_PROJECTILE_GRAVITY * apex_height).sqrt() * (1.0 + dt)
    };
    let discriminant = vertical_velocity * vertical_velocity - 2.0 * RAT_PROJECTILE_GRAVITY * dy;
    if !discriminant.is_finite() || discriminant < 0.0 {
        return None;
    }
    let root = discriminant.sqrt();
    let denominator = -RAT_PROJECTILE_GRAVITY;
    let t0 = (root - vertical_velocity) / denominator;
    let t1 = (-vertical_velocity - root) / denominator;
    let flight_time = t0.max(t1);
    if !flight_time.is_finite() || flight_time <= 0.001 {
        return None;
    }
    let horizontal_speed = horizontal_distance / flight_time;
    let (vx, vz) = if horizontal_distance <= f32::EPSILON {
        (0.0, 0.0)
    } else {
        (
            dx / horizontal_distance * horizontal_speed,
            dz / horizontal_distance * horizontal_speed,
        )
    };
    Some([vx, vertical_velocity, vz, 0.0])
}

/// Repeat the proven gravity-first owner integration for N legal world-phase steps.
pub(crate) fn sweeper_ratchet_ballistic_position_after_steps(
    launch: [f32; 4],
    initial_velocity: [f32; 4],
    steps: u64,
) -> Option<[f32; 4]> {
    if launch
        .iter()
        .chain(initial_velocity.iter())
        .any(|value| !value.is_finite())
    {
        return None;
    }
    let dt = 1.0 / RAT_SCRIPT_FIXED_HZ;
    let mut position = launch;
    let mut velocity = initial_velocity;
    for _ in 0..steps {
        velocity[1] -= RAT_PROJECTILE_GRAVITY * dt;
        position[0] += velocity[0] * dt;
        position[1] += velocity[1] * dt;
        position[2] += velocity[2] * dt;
    }
    Some(position)
}

fn sweeper_distance3(a: [f32; 4], b: [f32; 4]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn sweeper_horizontal_distance_xz(a: [f32; 4], b: [f32; 4]) -> f32 {
    let dx = a[0] - b[0];
    let dz = a[2] - b[2];
    (dx * dx + dz * dz).sqrt()
}

/// Generic retained RatchetMissile query proof. Projectile Physics contributes exactly one
/// gravity-first 1/60 step before each Handler query. Rodney uses his current owner position.
/// Shipped m10 MalfBots have no XZ movement command, so horizontal separation from their spawn
/// owner position is sufficient even while gravity/animation changes Y. Eye-spawned RollerBots
/// may move, so use a conservative speed*time XZ envelope derived from the number of completed
/// post-Ratchet updates rather than any absolute map tick. Transporter-carried/released MalfBot
/// and RollerBot reuse those same envelopes; unsupported character families remain fail-closed.
fn sweeper_owned_ratchet_projectile_query_guaranteed_miss(
    projectile: &NativeSweeperBossOwnedRatchetProjectileRuntime,
    player_owner_position: [f32; 4],
    transporter_owner_position: Option<[f32; 4]>,
    health_pickup_owner_position: Option<[f32; 4]>,
    live_ai: &NativeSweeperBossLiveAiRegistry,
    post_ratchet_ai: &NativeSweeperBossPostRatchetAiRuntime,
) -> Option<bool> {
    let projectile_steps = u64::from(projectile.handler_updates_completed).checked_add(1)?;
    let projectile_position = sweeper_ratchet_ballistic_position_after_steps(
        projectile.launch_position,
        projectile.initial_velocity,
        projectile_steps,
    )?;
    if projectile_position.iter().any(|value| !value.is_finite())
        || player_owner_position.iter().any(|value| !value.is_finite())
    {
        return None;
    }

    let player_clear = RODNEY_HIT_AREA_MAX_OWNER_OFFSET
        + RODNEY_HIT_AREA_RADIUS
        + RAT_PROJECTILE_ATTACK_POINT_RADIUS;
    if sweeper_distance3(projectile_position, player_owner_position) <= player_clear {
        return Some(false);
    }
    if let Some(transporter_owner_position) = transporter_owner_position {
        if transporter_owner_position
            .iter()
            .any(|value| !value.is_finite())
        {
            return None;
        }
        let transporter_clear =
            TRANSPORTER_HIT_AREA_OWNER_RADIUS_BOUND + RAT_PROJECTILE_ATTACK_POINT_RADIUS;
        if sweeper_distance3(projectile_position, transporter_owner_position) <= transporter_clear {
            return Some(false);
        }
    }
    if let Some(health_pickup_owner_position) = health_pickup_owner_position {
        if health_pickup_owner_position
            .iter()
            .any(|value| !value.is_finite())
        {
            return None;
        }
        let health_pickup_clear =
            HEALTH_PICKUP_HIT_AREA_OWNER_RADIUS_BOUND + RAT_PROJECTILE_ATTACK_POINT_RADIUS;
        if sweeper_distance3(projectile_position, health_pickup_owner_position)
            <= health_pickup_clear
        {
            return Some(false);
        }
    }

    for live in live_ai.entries.values() {
        if live.position.iter().any(|value| !value.is_finite()) {
            return None;
        }
        match (live.file?, live.source) {
            (
                SWEEPER_MALFBOT_FILE,
                NativeSweeperBossLiveAiSource::EyeSpawn
                | NativeSweeperBossLiveAiSource::SerializedTrigger { .. }
                | NativeSweeperBossLiveAiSource::TransporterCarry { .. }
                | NativeSweeperBossLiveAiSource::TransporterReleased { .. },
            ) => {
                let clear = MALFBOT_HIT_AREA_MAX_OWNER_ENCLOSING_RADIUS
                    + RAT_PROJECTILE_ATTACK_POINT_RADIUS;
                if sweeper_horizontal_distance_xz(projectile_position, live.position) <= clear {
                    return Some(false);
                }
            }
            (
                SWEEPER_ROLLERBOT_FILE,
                NativeSweeperBossLiveAiSource::EyeSpawn
                | NativeSweeperBossLiveAiSource::TransporterCarry { .. }
                | NativeSweeperBossLiveAiSource::TransporterReleased { .. },
            ) => {
                let NativeSweeperBossPostRatchetAiCharacterRuntime::RollerBot(roller) =
                    post_ratchet_ai.entries.get(&live.id)?
                else {
                    return None;
                };
                let owner_displacement_bound =
                    roller.updates_completed as f32 * (ROLLERBOT_SPEED / RAT_SCRIPT_FIXED_HZ);
                let clear = owner_displacement_bound
                    + ROLLERBOT_HIT_AREA_MAX_OWNER_ENCLOSING_RADIUS
                    + RAT_PROJECTILE_ATTACK_POINT_RADIUS;
                if sweeper_horizontal_distance_xz(projectile_position, live.position) <= clear {
                    return Some(false);
                }
            }
            _ => return None,
        }
    }
    Some(true)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct NativeSweeperRatchetTravelInput {
    pub current_eye: u8,
    pub target_eye: u8,
    pub relocation_latch: bool,
    /// Native handler +0x45C low byte written by HT_ScriptEvents_SetScriptValue (0x16000007).
    /// Value 1 is the jump-step handshake consumed by the relocation state here.
    pub script_value_status: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct NativeSweeperRatchetTravelStep {
    pub request_relocation: bool,
    pub clear_relocation_latch: bool,
    pub clear_script_value_status: bool,
    pub enter_appear: bool,
    pub requested_anim_mode: Option<u32>,
    pub forced_yaw: Option<f32>,
}

pub(crate) fn sweeper_ratchet_travel_step(
    input: NativeSweeperRatchetTravelInput,
) -> NativeSweeperRatchetTravelStep {
    if !input.relocation_latch {
        return NativeSweeperRatchetTravelStep {
            request_relocation: true,
            ..Default::default()
        };
    }
    if input.script_value_status != RAT_SCRIPT_VALUE_SIGNAL_1 {
        return NativeSweeperRatchetTravelStep::default();
    }
    if input.current_eye != input.target_eye {
        return NativeSweeperRatchetTravelStep {
            clear_relocation_latch: true,
            clear_script_value_status: true,
            ..Default::default()
        };
    }
    NativeSweeperRatchetTravelStep {
        clear_relocation_latch: true,
        enter_appear: true,
        requested_anim_mode: Some(APPEAR_ANIM_MODE),
        forced_yaw: Some(RAT_ARRIVAL_YAW),
        ..Default::default()
    }
}

// Controller phase state is kept separate from Ratchet ownership and XItem scheduling.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct NativeSweeperRatchetDamagePhaseState {
    /// Ratchet handler +0x494. Native ctor clears it; controller sets it once at difficulty >= 10.
    pub armed: bool,
    /// Ratchet handler +0x495. Ratchet state8/state9 signal1 sets it; controller consumes/clears it.
    pub acknowledgement: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct NativeSweeperRatchetDamagePhaseStep {
    pub newly_armed: bool,
    pub stall_controller: bool,
    pub clear_acknowledgement: bool,
    pub reset_pattern_gate_to_ten: bool,
    pub cleanup_live_ai_characters: bool,
    pub dispatch_controller_link6_event: bool,
}

impl NativeSweeperRatchetDamagePhaseState {
    pub(crate) fn step(
        &mut self,
        difficulty: &mut u8,
        max_difficulty: u8,
    ) -> NativeSweeperRatchetDamagePhaseStep {
        if *difficulty > max_difficulty {
            *difficulty = max_difficulty;
        }

        let mut step = NativeSweeperRatchetDamagePhaseStep::default();
        if *difficulty >= RAT_DAMAGE_PHASE_DIFFICULTY && !self.armed {
            self.armed = true;
            step.newly_armed = true;
            step.cleanup_live_ai_characters = true;
            step.dispatch_controller_link6_event = true;
        }

        if !self.armed {
            return step;
        }
        if !self.acknowledgement {
            step.stall_controller = true;
            return step;
        }

        self.acknowledgement = false;
        step.clear_acknowledgement = true;
        step.reset_pattern_gate_to_ten = true;
        step
    }
}

/// Exact per-main-update Ratchet +0x48E damage-accept latch after the state switch.
/// The native main update clears the byte before dispatching state 0..9.

pub(crate) fn sweeper_ratchet_damage_accept_latch(top_state: u8, script_value_status: u8) -> bool {
    match top_state {
        3 => script_value_status != 0,
        5..=8 => true,
        _ => false,
    }
}

/// State8 Attack2 and state9 hit-recovery both acknowledge the armed controller phase on signal1.

pub(crate) fn sweeper_ratchet_damage_phase_acknowledgement(
    top_state: u8,
    script_value_status: u8,
) -> bool {
    matches!(top_state, 8 | 9) && script_value_status == RAT_SCRIPT_VALUE_SIGNAL_1
}

// Native controller reducer for damage phase, Eye pressure, pattern scheduling, and health.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeSweeperBossControllerRuntime {
    pub difficulty: u8,
    pub damage_phase: NativeSweeperRatchetDamagePhaseState,
    pub eye_pressure: NativeSweeperBossEyePressureState,
    pub pattern_scheduler: NativeSweeperBossPatternSchedulerState,
    pub health_scheduler: NativeSweeperBossHealthSchedulerState,
}

impl Default for NativeSweeperBossControllerRuntime {
    fn default() -> Self {
        Self {
            difficulty: INITIAL_DIFFICULTY,
            damage_phase: NativeSweeperRatchetDamagePhaseState::default(),
            eye_pressure: NativeSweeperBossEyePressureState::default(),
            pattern_scheduler: NativeSweeperBossPatternSchedulerState::default(),
            health_scheduler: NativeSweeperBossHealthSchedulerState::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeSweeperBossControllerTickInput {
    /// Ratchet handler +0x495 observed at the start of this TriggerManager/controller service.
    pub ratchet_damage_acknowledgement: bool,
    /// Eye handler +0x3D8 states used by the >5-spawn pressure/open-window branch.
    pub eye_lifecycle_states: [i32; EYE_COUNT],
    /// Eye handler +0x3E0/+0x3F4 state used by pattern scheduling.
    pub pattern_eyes: [NativeSweeperBossPatternEyeState; EYE_COUNT],
    /// Live XItemManager membership deriving XItemHandler_AI_Character.
    pub live_ai_character_count: u32,
    /// Controller +0x136 after native transporter-liveness refresh.
    pub transporter_active: bool,
    /// Controller +0x10C ownership state.
    pub health_pickup_active: bool,
    pub player_handler_present: bool,
    pub player_current_health: f32,
    pub player_max_health: f32,
    /// FUN_00509C48 draw consumed only if eye-pressure selection actually runs.
    pub pressure_random_eye: u32,
    /// Separate FUN_00509C48 draw consumed only if pattern selection runs.
    pub pattern_random_mod5: u8,
    /// FUN_00509C6E draw consumed only if the health timer expires and rearms.
    pub health_timer_random_unit: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeSweeperBossControllerTickStep {
    pub damage_phase: NativeSweeperRatchetDamagePhaseStep,
    pub eye_pressure: Option<NativeSweeperBossEyePressureStep>,
    pub pattern: Option<NativeSweeperBossPatternSchedulerStep>,
    pub health: Option<NativeSweeperBossHealthSchedulerStep>,
}

impl NativeSweeperBossControllerRuntime {
    /// Eye handlers increment native controller +0x110 later in the gameplay frame.
    /// Recording here preserves that those requests become visible on the next controller service.
    pub(crate) fn record_eye_spawn_requests(&mut self, count: u32) {
        self.eye_pressure.spawn_request_count =
            self.eye_pressure.spawn_request_count.wrapping_add(count);
    }

    /// Exact recovered ordering inside XTrigger_Sweeper_Boss_Controller::0x004CDF10 after
    /// initialization/transporter refresh: damage-phase gate -> eye pressure -> pattern -> health.
    pub(crate) fn step_controller_tick(
        &mut self,
        patterns: &RobotsSweeperBossPatterns,
        input: NativeSweeperBossControllerTickInput,
    ) -> NativeSweeperBossControllerTickStep {
        self.damage_phase.acknowledgement = input.ratchet_damage_acknowledgement;
        let damage_phase = self.damage_phase.step(&mut self.difficulty, MAX_DIFFICULTY);
        if damage_phase.stall_controller {
            return NativeSweeperBossControllerTickStep {
                damage_phase,
                eye_pressure: None,
                pattern: None,
                health: None,
            };
        }
        if damage_phase.reset_pattern_gate_to_ten {
            self.pattern_scheduler.gate_counter = 10.0;
        }

        let eye_pressure = self
            .eye_pressure
            .step(input.eye_lifecycle_states, input.pressure_random_eye);
        let pattern = self.pattern_scheduler.step(
            patterns,
            NativeSweeperBossPatternSchedulerInput {
                difficulty: self.difficulty,
                random_mod5: input.pattern_random_mod5,
                live_ai_character_count: input.live_ai_character_count,
                transporter_active: input.transporter_active,
                eyes: input.pattern_eyes,
            },
        );
        let health = self
            .health_scheduler
            .step(NativeSweeperBossHealthSchedulerInput {
                pickup_active: input.health_pickup_active,
                player_handler_present: input.player_handler_present,
                player_current_health: input.player_current_health,
                player_max_health: input.player_max_health,
                timer_random_unit: input.health_timer_random_unit,
            });
        NativeSweeperBossControllerTickStep {
            damage_phase,
            eye_pressure: Some(eye_pressure),
            pattern: Some(pattern),
            health: Some(health),
        }
    }
}

// Ratchet top-state reducer; frame clocks and controller ordering are composed separately.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeSweeperRatchetRuntime {
    pub top_state: u8,
    pub current_eye: u8,
    pub target_eye: u8,
    pub relocation_latch: bool,
    pub damage_accept_latch: bool,
    pub damage_acknowledgement: bool,
    pub hit_points: u8,
    pub combat: NativeSweeperRatchetCombatState,
}

impl Default for NativeSweeperRatchetRuntime {
    fn default() -> Self {
        Self {
            top_state: 0,
            current_eye: 2,
            target_eye: 2,
            relocation_latch: false,
            damage_accept_latch: false,
            damage_acknowledgement: false,
            hit_points: RAT_INITIAL_HIT_POINTS,
            combat: NativeSweeperRatchetCombatState::default(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct NativeSweeperRatchetTickInput<'a> {
    pub script_value_status: u8,
    pub current_yaw: f32,
    pub player_position: [f32; 4],
    pub anchors: [[f32; 4]; EYE_COUNT],
    pub relocation_target_draws: &'a [u32],
    pub combat: NativeSweeperRatchetCombatInput,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct NativeSweeperRatchetTickStep {
    pub requested_anim_mode: Option<u32>,
    /// True only for native +0xE4 / 0x00405210 force requests. Ordinary +0xDC
    /// requests stay suppressed when the requested AnimMode already equals +0x478.
    pub force_anim_mode_reapply: bool,
    pub forced_yaw: Option<f32>,
    pub owner_position: Option<[f32; 4]>,
    pub facing_yaw: Option<f32>,
    pub damage_accept_latch: bool,
    pub controller_damage_acknowledgement: bool,
    pub clear_script_value_status: bool,
    pub relocation_random_draws_used: u8,
    pub combat_random_draws_used: u8,
    pub needs_more_relocation_rng: bool,
    pub skips_common_pose_tail: bool,
}

impl NativeSweeperRatchetRuntime {
    /// Whether the next top-state Handler needs relocation-target RNG before it can complete.
    /// This is derived from the travel state rather than an absolute fixed tick.
    pub(crate) fn needs_relocation_target_rng(&self, script_value_status: u8) -> bool {
        if self.top_state != 1 || self.relocation_latch {
            return false;
        }
        let travel = sweeper_ratchet_travel_step(NativeSweeperRatchetTravelInput {
            current_eye: self.current_eye,
            target_eye: self.target_eye,
            relocation_latch: self.relocation_latch,
            script_value_status,
        });
        travel.request_relocation && self.current_eye == self.target_eye
    }

    pub(crate) fn step_top_state(
        &mut self,
        input: NativeSweeperRatchetTickInput<'_>,
    ) -> NativeSweeperRatchetTickStep {
        let entry_state = self.top_state;
        self.damage_accept_latch =
            sweeper_ratchet_damage_accept_latch(entry_state, input.script_value_status);
        if sweeper_ratchet_damage_phase_acknowledgement(entry_state, input.script_value_status) {
            self.damage_acknowledgement = true;
        }
        let mut step = NativeSweeperRatchetTickStep {
            damage_accept_latch: self.damage_accept_latch,
            controller_damage_acknowledgement: self.damage_acknowledgement,
            ..Default::default()
        };

        match entry_state {
            0 => {
                step.requested_anim_mode = Some(IDLE_ATTACK_ANIM_MODE);
                self.top_state = 4;
                self.combat.counter = 0.0;
            }
            1 => {
                step.skips_common_pose_tail = true;
                let travel = sweeper_ratchet_travel_step(NativeSweeperRatchetTravelInput {
                    current_eye: self.current_eye,
                    target_eye: self.target_eye,
                    relocation_latch: self.relocation_latch,
                    script_value_status: input.script_value_status,
                });
                if travel.request_relocation {
                    let Some(relocation) = sweeper_ratchet_relocation_step(
                        self.current_eye,
                        self.target_eye,
                        input.anchors,
                    ) else {
                        return step;
                    };
                    step.owner_position = Some(relocation.anchor);
                    self.current_eye = relocation.next_current_eye;
                    self.relocation_latch = relocation.latch;
                    self.combat.command_latch = u8::from(self.relocation_latch);
                    step.requested_anim_mode = relocation.requested_anim_mode;
                    step.force_anim_mode_reapply = relocation.force_anim_mode_reapply;
                    step.forced_yaw = relocation.requested_yaw;
                    if relocation.needs_new_target {
                        if let Some((target, draws)) = sweeper_ratchet_target_from_draws(
                            self.current_eye,
                            input.relocation_target_draws,
                        ) {
                            self.target_eye = target;
                            step.relocation_random_draws_used = draws;
                        } else {
                            step.needs_more_relocation_rng = true;
                            step.relocation_random_draws_used =
                                input.relocation_target_draws.len().min(u8::MAX as usize) as u8;
                        }
                    }
                    return step;
                }
                if travel.clear_relocation_latch {
                    self.relocation_latch = false;
                    self.combat.command_latch = 0;
                }
                if travel.clear_script_value_status {
                    step.clear_script_value_status = true;
                }
                if travel.enter_appear {
                    self.top_state = 3;
                    step.requested_anim_mode = travel.requested_anim_mode;
                    step.forced_yaw = travel.forced_yaw;
                }
                return step;
            }
            2 => {
                if input.script_value_status == RAT_SCRIPT_VALUE_SIGNAL_1 {
                    self.top_state = 1;
                }
            }
            3 => {
                if input.script_value_status == RAT_SCRIPT_VALUE_SIGNAL_2 {
                    step.requested_anim_mode = Some(IDLE_ATTACK_ANIM_MODE);
                    self.top_state = 4;
                    self.combat.counter = 0.0;
                }
            }
            4 => {
                self.combat.command_latch = u8::from(self.relocation_latch);
                let combat = self.combat.step(input.combat);
                self.relocation_latch = self.combat.command_latch != 0;
                step.combat_random_draws_used = combat.random_draws_used;
                if let Some(anim_mode) = combat.requested_anim_mode {
                    self.top_state = self.combat.action;
                    step.requested_anim_mode = Some(anim_mode);
                }
            }
            5 | 6 | 7 => {
                if input.script_value_status == RAT_SCRIPT_VALUE_SIGNAL_1 {
                    step.requested_anim_mode = Some(IDLE_ATTACK_ANIM_MODE);
                    self.top_state = 4;
                    self.combat.counter = 0.0;
                }
            }
            8 => {
                if input.script_value_status == RAT_SCRIPT_VALUE_SIGNAL_2 {
                    step.requested_anim_mode = Some(IDLE_ATTACK_ANIM_MODE);
                    self.top_state = 4;
                    self.combat.counter = 0.0;
                }
            }
            9 => {
                if input.script_value_status == RAT_SCRIPT_VALUE_SIGNAL_1 {
                    self.top_state = 2;
                    step.requested_anim_mode = Some(DISAPPEAR_ANIM_MODE);
                    self.relocation_latch = false;
                    self.combat.command_latch = 0;
                    self.combat.phase = 0;
                }
            }
            _ => {}
        }

        let current = usize::from(self.current_eye);
        let owner_position = input.anchors.get(current).copied().unwrap_or([0.0; 4]);
        step.owner_position = Some(owner_position);
        step.facing_yaw = Some(sweeper_ratchet_face_player_yaw(
            input.current_yaw,
            owner_position,
            input.player_position,
        ));
        step
    }

    pub(crate) fn controller_consumed_damage_acknowledgement(&mut self) {
        self.damage_acknowledgement = false;
    }
}

// Exact Ratchet Handler to family-4 animator frame composition.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeSweeperRatchetFrameRuntime {
    pub ratchet: NativeSweeperRatchetRuntime,
    pub applied_anim_mode: u32,
    pub pending_anim_mode: Option<u32>,
    pub script_value_status: u8,
    pub script: NativeSweeperRatchetScriptRuntime,
}

impl Default for NativeSweeperRatchetFrameRuntime {
    fn default() -> Self {
        Self {
            ratchet: NativeSweeperRatchetRuntime::default(),
            applied_anim_mode: BOSS_ANIM_MODE,
            pending_anim_mode: None,
            script_value_status: 0,
            script: NativeSweeperRatchetScriptRuntime::default(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct NativeSweeperRatchetFrameInput<'a> {
    pub current_yaw: f32,
    pub player_position: [f32; 4],
    pub anchors: [[f32; 4]; EYE_COUNT],
    pub relocation_target_draws: &'a [u32],
    pub combat: NativeSweeperRatchetCombatInput,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct NativeSweeperRatchetFrameStep {
    pub applied_anim_mode: Option<u32>,
    pub requested_anim_mode: Option<u32>,
    pub script_value_status_before_handler: u8,
    pub script_value_status_after_animator: u8,
    pub ratchet: NativeSweeperRatchetTickStep,
    pub script: NativeSweeperRatchetScriptStep,
}

impl NativeSweeperRatchetFrameRuntime {
    pub(crate) fn step_frame(
        &mut self,
        scripts: &RobotsSweeperRatchetScripts,
        input: NativeSweeperRatchetFrameInput<'_>,
    ) -> NativeSweeperRatchetFrameStep {
        // Native Handler update begins with vslot +0xEC = 0x004050F0. A
        // successfully queued target always reaches the apply attempt here. Ordinary
        // +0xDC = 0x004051E0 suppresses same-current requests before they can become
        // pending; relocation helper 0x004CFCC0 instead calls +0xE4 = 0x00405210,
        // sets force flag +0x484, and may therefore queue the same AnimMode again.
        // Successful apply resets +0x45C and restarts the selected family-4 layer.
        let mut applied_anim_mode = None;
        if let Some(pending) = self.pending_anim_mode.take() {
            self.applied_anim_mode = pending;
            self.script_value_status = 0;
            if let Some(script) = sweeper_ratchet_script_for_anim_mode(pending) {
                self.script.restart(script);
            } else {
                self.script.clear();
            }
            applied_anim_mode = Some(pending);
        }

        let status_before_handler = self.script_value_status;
        let ratchet = self.ratchet.step_top_state(NativeSweeperRatchetTickInput {
            script_value_status: status_before_handler,
            current_yaw: input.current_yaw,
            player_position: input.player_position,
            anchors: input.anchors,
            relocation_target_draws: input.relocation_target_draws,
            combat: input.combat,
        });

        if ratchet.clear_script_value_status {
            self.script_value_status = 0;
        }
        if let Some(requested) = ratchet.requested_anim_mode {
            if ratchet.force_anim_mode_reapply || requested != self.applied_anim_mode {
                self.pending_anim_mode = Some(requested);
            }
        }

        // XItem 0x004E8188 runs Handler +0x0C first and only afterwards walks
        // attached animators through 0x004E8126. Therefore the currently
        // applied AnimMode gets one family-4 update after the Handler, and a
        // crossed SetScriptValue writes +0x45C for the *next* Handler update.
        let script = self.script.step(scripts);
        if let Some(status) = script.script_value_status {
            self.script_value_status = status;
        }

        NativeSweeperRatchetFrameStep {
            applied_anim_mode,
            requested_anim_mode: ratchet.requested_anim_mode,
            script_value_status_before_handler: status_before_handler,
            script_value_status_after_animator: self.script_value_status,
            ratchet,
            script,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeSweeperBossLiveAiSource {
    EyeSpawn,
    SerializedTrigger {
        trigger_index: usize,
    },
    /// `0x00469450` has appended this AI XItem to the Transporter carried-vector,
    /// cleared XItem `+0x154`, and cleared the source trigger's owned `+0x68` slot.
    TransporterCarry {
        trigger_index: usize,
    },
    /// The Transporter XItem was reset/reloaded; the AI remains live but is no longer
    /// counted by the new handler's carried-vector gate `+0x538 < trigger+0x80`.
    TransporterReleased {
        trigger_index: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeSweeperBossLiveAiCharacter {
    pub id: u32,
    pub source: NativeSweeperBossLiveAiSource,
    pub config_index: u8,
    pub file: Option<u32>,
    pub position: [f32; 4],
    pub yaw: f32,
    /// Controller cleanup calls AI +0x12C, which marks the owner XItem for destruction.
    /// Native manager membership remains visible until 0x00444E80 at end of gameplay tick.
    pub pending_destroy: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct NativeSweeperBossLiveAiRegistry {
    next_id: u32,
    entries: BTreeMap<u32, NativeSweeperBossLiveAiCharacter>,
    serialized_trigger_ids: BTreeMap<usize, u32>,
}

impl NativeSweeperBossLiveAiRegistry {
    pub(crate) fn registered_count(&self) -> u32 {
        self.entries.len().min(u32::MAX as usize) as u32
    }

    pub(crate) fn has_pickup_drop_capable_character(
        &self,
        patterns: &RobotsSweeperBossPatterns,
    ) -> bool {
        self.entries.values().any(|entry| {
            patterns
                .monster_pickup_drop_count(entry.config_index)
                .is_none_or(|count| count != 0)
        })
    }

    pub(crate) fn register_spawn(
        &mut self,
        patterns: &RobotsSweeperBossPatterns,
        config_index: u8,
        transform: NativeSweeperBossSpawnTransform,
    ) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        self.entries.insert(
            id,
            NativeSweeperBossLiveAiCharacter {
                id,
                source: NativeSweeperBossLiveAiSource::EyeSpawn,
                config_index,
                file: patterns.monster_file(config_index),
                position: transform.position,
                yaw: transform.yaw,
                pending_destroy: false,
            },
        );
        id
    }

    /// Called only after the native serialized trigger has actually created and
    /// registered its XItem (for type10 this is near-band `0x0044D110 -> +0x24`).
    /// Native trigger+0x68 already makes this ownership idempotent while alive.
    pub(crate) fn register_serialized_trigger_after_native_create(
        &mut self,
        trigger_index: usize,
        config_index: u8,
        file: u32,
        position: [f32; 4],
        yaw: f32,
    ) -> (u32, bool) {
        if let Some(id) = self.serialized_trigger_ids.get(&trigger_index).copied() {
            if self.entries.contains_key(&id) {
                return (id, false);
            }
            self.serialized_trigger_ids.remove(&trigger_index);
        }
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        self.entries.insert(
            id,
            NativeSweeperBossLiveAiCharacter {
                id,
                source: NativeSweeperBossLiveAiSource::SerializedTrigger { trigger_index },
                config_index,
                file: Some(file),
                position,
                yaw,
                pending_destroy: false,
            },
        );
        self.serialized_trigger_ids.insert(trigger_index, id);
        (id, true)
    }

    pub(crate) fn serialized_trigger_live_id(&self, trigger_index: usize) -> Option<u32> {
        let id = self.serialized_trigger_ids.get(&trigger_index).copied()?;
        self.entries.contains_key(&id).then_some(id)
    }

    /// Mirrors the successful Transporter handoff after target `+0x24`: the AI XItem
    /// is appended to handler `+0x53C`, then XItem `+0x154` and trigger `+0x68` are cleared.
    /// The XItem stays registered in the live manager, but the serialized trigger is free
    /// to create another monster on the next attempt.
    pub(crate) fn transfer_serialized_trigger_to_transporter(
        &mut self,
        trigger_index: usize,
    ) -> Option<u32> {
        let id = self.serialized_trigger_ids.remove(&trigger_index)?;
        let entry = self.entries.get_mut(&id)?;
        entry.source = NativeSweeperBossLiveAiSource::TransporterCarry { trigger_index };
        Some(id)
    }

    pub(crate) fn transporter_carried_count(&self) -> u32 {
        self.entries
            .values()
            .filter(|entry| {
                matches!(
                    entry.source,
                    NativeSweeperBossLiveAiSource::TransporterCarry { .. }
                )
            })
            .count()
            .min(u32::MAX as usize) as u32
    }

    pub(crate) fn release_transporter_carries(&mut self) -> u32 {
        let mut released = 0u32;
        for entry in self.entries.values_mut() {
            if let NativeSweeperBossLiveAiSource::TransporterCarry { trigger_index } = entry.source
            {
                entry.source = NativeSweeperBossLiveAiSource::TransporterReleased { trigger_index };
                released = released.wrapping_add(1);
            }
        }
        released
    }

    /// Mirrors a native handler `+0x12C` request. For common AI vtables this is
    /// `0x00453640 -> 0x00443EE0`: only the XItem destroy bit is queued here;
    /// manager membership remains live until `0x00444E80` flushes the request.
    pub(crate) fn mark_live_id_pending_destroy_after_native_request(&mut self, id: u32) -> bool {
        let Some(entry) = self.entries.get_mut(&id) else {
            return false;
        };
        if entry.pending_destroy {
            return false;
        }
        entry.pending_destroy = true;
        true
    }

    /// Mirrors controller AI +0x12C cleanup: mark existing live XItems only.
    /// New spawns later in the same XItem phase are not retroactively marked.
    pub(crate) fn mark_all_pending_destroy(&mut self) -> u32 {
        let mut marked = 0u32;
        for entry in self.entries.values_mut() {
            if !entry.pending_destroy {
                entry.pending_destroy = true;
                marked = marked.wrapping_add(1);
            }
        }
        marked
    }

    /// Mirrors 0x00444E80, which runs after the XItem update loops and finally
    /// removes registration flags / destroys XItems carrying the 0x10 bit.
    pub(crate) fn flush_pending_destroy_end_of_tick(&mut self) -> u32 {
        let removed = self
            .entries
            .iter()
            .filter_map(|(id, entry)| entry.pending_destroy.then_some((*id, entry.source)))
            .collect::<Vec<_>>();
        for (id, source) in &removed {
            self.entries.remove(id);
            if let NativeSweeperBossLiveAiSource::SerializedTrigger { trigger_index } = source {
                self.serialized_trigger_ids.remove(trigger_index);
            }
        }
        removed.len().min(u32::MAX as usize) as u32
    }

    pub(crate) fn entry(&self, id: u32) -> Option<&NativeSweeperBossLiveAiCharacter> {
        self.entries.get(&id)
    }

    pub(crate) fn entries(&self) -> impl Iterator<Item = &NativeSweeperBossLiveAiCharacter> {
        self.entries.values()
    }

    pub(crate) fn sync_live_pose(&mut self, id: u32, position: [f32; 4], yaw: f32) -> bool {
        let Some(entry) = self.entries.get_mut(&id) else {
            return false;
        };
        entry.position = position;
        entry.yaw = yaw;
        true
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeSweeperBossTransporterEvent {
    /// `XItemHandler_Transporter::0x00469450` successfully creates/releases one
    /// carried monster and increments creator trigger `+0xE4`.
    SuccessfulMonsterSpawn,
    /// Transporter path event type 0x0B reaches `0x0044C380(...,0x1000,0)`,
    /// then `0x0047FE20 -> 0x0044BBB0` reloads serialized trigger state;
    /// `0x0047F910` resets runtime `+0xE4` to zero.
    ResetReload,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeSweeperBossTransporterSpawnTarget {
    pub trigger_index: usize,
    pub config_index: u8,
    pub position: [f32; 4],
    pub yaw: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeSweeperBossTransporterSpawnConfig {
    pub targets: [Option<NativeSweeperBossTransporterSpawnTarget>; 4],
    pub targets_complete: bool,
    pub max_successful_spawns: u32,
    pub max_carried_monsters: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativeSweeperBossTransporterLatchRuntime {
    /// Native trigger `+0x4C bit0`: set means inactive and eligible for Eye case6 activation.
    pub trigger_inactive: bool,
    /// Native trigger `+0xE4`: count of successful monster spawns from the DropShip handler.
    pub successful_spawn_count: u32,
    /// Boss controller `+0x136`, consumed by controller/Pattern/Ratchet blocker logic.
    pub controller_latch: bool,
}

impl Default for NativeSweeperBossTransporterLatchRuntime {
    fn default() -> Self {
        // Shipped m10_boss link5/type73 has game_flags 0xE001, so bit0 starts set.
        Self {
            trigger_inactive: true,
            successful_spawn_count: 0,
            controller_latch: false,
        }
    }
}

impl NativeSweeperBossTransporterLatchRuntime {
    /// Exact controller `0x004CDF10` refresh: while +0x136 is set it resolves
    /// link5 and clears the latch as soon as Transporter trigger `+0xE4 != 0`.
    pub(crate) fn refresh_before_controller(&mut self) -> bool {
        if self.controller_latch && self.successful_spawn_count != 0 {
            self.controller_latch = false;
            true
        } else {
            false
        }
    }

    /// Eye case6 only activates a Transporter whose trigger `+0x4C bit0` is set.
    /// Native `0x0044C380(...,1,...) -> +0x1C=0x0044D260` clears that bit and
    /// `0x004CF1D0` immediately sets controller `+0x136`.
    pub(crate) fn activate_from_eye_case6(&mut self) -> bool {
        if !self.trigger_inactive {
            return false;
        }
        self.trigger_inactive = false;
        self.controller_latch = true;
        true
    }

    pub(crate) fn apply_handler_event(&mut self, event: NativeSweeperBossTransporterEvent) {
        match event {
            NativeSweeperBossTransporterEvent::SuccessfulMonsterSpawn => {
                self.successful_spawn_count = self.successful_spawn_count.wrapping_add(1);
            }
            NativeSweeperBossTransporterEvent::ResetReload => {
                self.trigger_inactive = true;
                self.successful_spawn_count = 0;
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeSweeperBossOwnedRatchetProjectileRuntime {
    pub launch_position: [f32; 4],
    pub initial_velocity: [f32; 4],
    pub handler_updates_completed: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeSweeperBossTriggerEvent {
    ControllerLink6CommonMask1,
    #[cfg(test)]
    PlayerLink0CommonMask1,
}

// Fixed-order boss runtime owns deferred live-AI membership across controller and XItem phases.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeSweeperBossReplayRuntime {
    pub controller: NativeSweeperBossControllerRuntime,
    pub ratchet: NativeSweeperRatchetFrameRuntime,
    pub ratchet_owner_yaw: Option<f32>,
    pub live_ai: NativeSweeperBossLiveAiRegistry,
    pub post_ratchet_ai: NativeSweeperBossPostRatchetAiRuntime,
    pub eye_commands: [NativeSweeperBossEyeCommandRuntime; EYE_COUNT],
    pub eye_runtimes: [super::sweeper_boss_eye::NativeSweeperBossEyeRuntime; EYE_COUNT],
    pub transporter: NativeSweeperBossTransporterLatchRuntime,
    pub health_pickup: Option<NativeSweeperBossOwnedHealthPickupRuntime>,
    pub ratchet_projectiles: Vec<NativeSweeperBossOwnedRatchetProjectileRuntime>,
    pub trigger_events: Vec<NativeSweeperBossTriggerEvent>,
    /// Once any boss-side producer can create a native Pickup candidate, the outer
    /// `0x00444E0A` ordinal tail is no longer provably zero until runtime reset.
    pub post_xitem_pickup_dynamic_risk: bool,
}

impl Default for NativeSweeperBossReplayRuntime {
    fn default() -> Self {
        Self {
            controller: NativeSweeperBossControllerRuntime::default(),
            ratchet: NativeSweeperRatchetFrameRuntime::default(),
            ratchet_owner_yaw: None,
            live_ai: NativeSweeperBossLiveAiRegistry::default(),
            post_ratchet_ai: NativeSweeperBossPostRatchetAiRuntime::default(),
            eye_commands: [NativeSweeperBossEyeCommandRuntime::default(); EYE_COUNT],
            eye_runtimes: [super::sweeper_boss_eye::NativeSweeperBossEyeRuntime::default();
                EYE_COUNT],
            transporter: NativeSweeperBossTransporterLatchRuntime::default(),
            health_pickup: None,
            ratchet_projectiles: Vec::new(),
            trigger_events: Vec::new(),
            post_xitem_pickup_dynamic_risk: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeSweeperBossControllerPhaseInput {
    pub eye_lifecycle_states: [i32; EYE_COUNT],
    pub pattern_eyes: [NativeSweeperBossPatternEyeState; EYE_COUNT],
    pub health_pickup_active: bool,
    pub player_handler_present: bool,
    pub player_current_health: f32,
    pub player_max_health: f32,
    pub pressure_random_eye: u32,
    pub pattern_random_mod5: u8,
    pub health_timer_random_unit: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeSweeperBossOwnedControllerPhaseInput {
    pub player_health_state_known: bool,
    pub player_handler_present: bool,
    pub player_current_health: f32,
    pub player_max_health: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeSweeperBossControllerPhaseStep {
    pub controller_observed_ratchet_acknowledgement: bool,
    pub controller_cleared_ratchet_acknowledgement: bool,
    pub controller_live_ai_character_count: u32,
    pub ai_cleanup_marked: u32,
    pub transporter_latch_cleared_before_controller: bool,
    pub health_pickup_spawn_plan: Option<NativeSweeperBossHealthPickupSpawnPlan>,
    pub controller: NativeSweeperBossControllerTickStep,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeSweeperBossXItemPreRatchetStep {
    pub eye_steps: [NativeSweeperBossEyeCommandStep; EYE_COUNT],
    pub eye_spawn_requests_recorded_after_controller: u32,
    pub eye_spawn_random_draws_used: u8,
    pub transporter_activation_requests: u32,
    pub transporter_activations_applied: u32,
    pub transporter_spawn_count_after_xitem_phase: u32,
    pub transporter_active_for_ratchet: bool,
    pub ratchet_live_ai_character_count: u32,
    pub ratchet_blocking_handler_count: u32,
    pub needs_more_eye_spawn_rng: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct NativeSweeperBossOwnedXItemPhaseInput<'a> {
    pub roller_path_graph: Option<&'a NativeSweeperRollerbotPathGraph>,
    pub transporter_events_known: bool,
    pub transporter_events: &'a [NativeSweeperBossTransporterEvent],
    pub transporter_spawn: Option<&'a NativeSweeperBossTransporterSpawnConfig>,
    pub transporter_spawn_attempts: u16,
    pub transporter_owner_position: Option<[f32; 4]>,
    pub transporter_spawn_position: Option<[f32; 4]>,
    /// `None` means native contact state is unknown. `Some(false/true)` is an exact
    /// WaitForHit observation and is ignored until the health Script reaches frame17.
    pub health_pickup_valid_hit: Option<bool>,
    pub ratchet_missile_hand_local: Option<[f32; 3]>,
    pub player_position: [f32; 4],
    pub anchors: [[f32; 4]; EYE_COUNT],
    pub eye_spawn_sources: [NativeSweeperBossEyeSpawnSource; EYE_COUNT],
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeSweeperBossOwnedXItemPhaseStep {
    pub post_ratchet_ai: Vec<NativeSweeperBossPostRatchetAiCharacterStep>,
    pub health_pickup: Option<NativeSweeperBossOwnedHealthPickupStep>,
    pub pre_ratchet_rng: NativeSweeperBossPreRatchetRngStep,
    pub xitem: NativeSweeperBossXItemPhaseStep,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeSweeperBossXItemPhaseStep {
    pub ratchet_live_ai_character_count: u32,
    pub ratchet_blocking_handler_count: u32,
    pub ai_flushed_end_of_tick: u32,
    pub eye_steps: [NativeSweeperBossEyeCommandStep; EYE_COUNT],
    pub eye_spawn_requests_recorded_after_controller: u32,
    pub eye_spawn_random_draws_used: u8,
    pub transporter_activation_requests: u32,
    pub transporter_activations_applied: u32,
    pub transporter_spawn_count_after_xitem_phase: u32,
    pub transporter_active_for_ratchet: bool,
    pub needs_more_eye_spawn_rng: bool,
    pub ratchet: NativeSweeperRatchetFrameStep,
}

impl NativeSweeperBossReplayRuntime {
    pub(crate) fn owned_eye_lifecycle_states(&self) -> [i32; EYE_COUNT] {
        std::array::from_fn(|index| self.eye_runtimes[index].lifecycle.state)
    }

    pub(crate) fn owned_pattern_eyes(&self) -> [NativeSweeperBossPatternEyeState; EYE_COUNT] {
        std::array::from_fn(|index| NativeSweeperBossPatternEyeState {
            main_state: self.eye_runtimes[index].lifecycle.state,
            command_state: self.eye_commands[index].command_state,
        })
    }

    pub(crate) fn mark_post_xitem_pickup_dynamic_risk(&mut self) {
        self.post_xitem_pickup_dynamic_risk = true;
    }

    pub(crate) fn post_xitem_pickup_dynamic_risk(&self) -> bool {
        self.post_xitem_pickup_dynamic_risk
    }

    pub(crate) fn step_owned_ratchet_projectiles_pre_ratchet(
        &mut self,
        player_position: [f32; 4],
        transporter_owner_position: Option<[f32; 4]>,
    ) -> Option<u32> {
        if self.ratchet_projectiles.is_empty() {
            return Some(0);
        }
        let transporter_owner_position = if self.transporter.trigger_inactive {
            None
        } else {
            Some(transporter_owner_position?)
        };
        let health_pickup_owner_position = self
            .health_pickup
            .as_ref()
            .map(|pickup| pickup.spawn_plan.position);

        let live_ai = &self.live_ai;
        let post_ratchet_ai = &self.post_ratchet_ai;
        let mut next_projectiles = self.ratchet_projectiles.clone();
        let mut updated = 0u32;
        for projectile in &mut next_projectiles {
            if !sweeper_owned_ratchet_projectile_query_guaranteed_miss(
                projectile,
                player_position,
                transporter_owner_position,
                health_pickup_owner_position,
                live_ai,
                post_ratchet_ai,
            )? {
                return None;
            }
            projectile.handler_updates_completed =
                projectile.handler_updates_completed.checked_add(1)?;
            updated = updated.checked_add(1)?;
        }
        self.ratchet_projectiles = next_projectiles;
        Some(updated)
    }

    pub(crate) fn register_owned_ratchet_projectile(
        &mut self,
        owner_position: [f32; 4],
        owner_yaw: f32,
        local_hand_position: [f32; 3],
        target_position: [f32; 4],
    ) -> Option<()> {
        let launch_position =
            sweeper_ratchet_missile_launch_position(owner_position, owner_yaw, local_hand_position);
        if launch_position.iter().any(|value| !value.is_finite()) || launch_position[3] != 0.0 {
            return None;
        }
        let initial_velocity =
            sweeper_ratchet_projectile_initial_velocity(launch_position, target_position)?;
        self.ratchet_projectiles
            .push(NativeSweeperBossOwnedRatchetProjectileRuntime {
                launch_position,
                initial_velocity,
                handler_updates_completed: 0,
            });
        Some(())
    }

    pub(crate) fn initialize_owned_ratchet_yaw(&mut self, yaw: f32) {
        if self.ratchet_owner_yaw.is_none() && yaw.is_finite() {
            self.ratchet_owner_yaw = Some(yaw);
        }
    }

    pub(crate) fn apply_owned_eye_pressure(
        &mut self,
        pressure: Option<NativeSweeperBossEyePressureStep>,
    ) {
        let Some(selected_eye) = pressure.and_then(|step| step.selected_eye) else {
            return;
        };
        if let Some(eye) = self.eye_runtimes.get_mut(selected_eye) {
            eye.request_open();
        }
    }

    pub(crate) fn step_owned_eye_xitem_phase(
        &mut self,
        patterns: &RobotsSweeperBossPatterns,
        scripts: &super::sweeper_boss_script::RobotsSweeperEyeScripts,
        eye_spawn_sources: [NativeSweeperBossEyeSpawnSource; EYE_COUNT],
        variant_random_mod100_draws: &[u8],
    ) -> super::sweeper_boss_eye::NativeSweeperBossOwnedEyeXItemPhaseStep {
        let eye_runtimes = &mut self.eye_runtimes;
        let eye_commands = &mut self.eye_commands;
        let live_ai = &mut self.live_ai;
        let mut result =
            super::sweeper_boss_eye::NativeSweeperBossOwnedEyeXItemPhaseStep::default();
        let mut random_offset = 0usize;

        for eye_ordinal in 0..EYE_COUNT {
            let command = &mut eye_commands[eye_ordinal];
            let source = eye_spawn_sources[eye_ordinal];
            let available_draws = variant_random_mod100_draws
                .get(random_offset..)
                .unwrap_or_default();
            let (eye_step, command_step) = eye_runtimes[eye_ordinal]
                .step_fixed_update_interleaved(scripts, || {
                    command.step(patterns, live_ai, source, available_draws)
                });
            random_offset =
                random_offset.saturating_add(usize::from(command_step.random_draws_used));
            result.random_draws_used = result
                .random_draws_used
                .saturating_add(command_step.random_draws_used);
            result.transporter_activation_requests = result
                .transporter_activation_requests
                .saturating_add(command_step.transporter_activation_requests);
            result.eye_steps[eye_ordinal] = eye_step;
            result.command_steps[eye_ordinal] = command_step;
        }

        result
    }

    /// Transactional TriggerManager/controller service with process-global RNG ownership.
    /// A zero-valued preview is safe because pressure, pattern and health draw eligibility is
    /// determined entirely by entry runtime state; their random values only choose outputs after
    /// each branch has already committed to consuming its draw. Native order is preserved:
    /// pressure -> pattern -> health.
    pub(crate) fn step_controller_phase_owned_rng(
        &mut self,
        patterns: &RobotsSweeperBossPatterns,
        rng: &mut crate::map_runtime::RuntimeRobotsGlobalRngState,
        input: NativeSweeperBossOwnedControllerPhaseInput,
    ) -> Option<NativeSweeperBossControllerPhaseStep> {
        let phase_input = |runtime: &NativeSweeperBossReplayRuntime,
                           pressure_random_eye,
                           pattern_random_mod5,
                           health_timer_random_unit| {
            NativeSweeperBossControllerPhaseInput {
                eye_lifecycle_states: runtime.owned_eye_lifecycle_states(),
                pattern_eyes: runtime.owned_pattern_eyes(),
                health_pickup_active: runtime
                    .health_pickup
                    .is_some_and(|pickup| pickup.controller_owned),
                player_handler_present: input.player_handler_present,
                player_current_health: input.player_current_health,
                player_max_health: input.player_max_health,
                pressure_random_eye,
                pattern_random_mod5,
                health_timer_random_unit,
            }
        };

        let mut preview = self.clone();
        let preview_input = phase_input(&preview, 0, 0, 0.0);
        let preview_step = preview.step_controller_phase(patterns, preview_input);
        let pressure_draws = preview_step
            .controller
            .eye_pressure
            .map(|step| step.random_draws_used)
            .unwrap_or_default();
        let pattern_draws = preview_step
            .controller
            .pattern
            .map(|step| step.random_draws_used)
            .unwrap_or_default();
        let health_draws = preview_step
            .controller
            .health
            .map(|step| step.random_draws_used)
            .unwrap_or_default();
        let health_spawn_requested = preview_step
            .controller
            .health
            .is_some_and(|step| step.spawn_requested);
        if pressure_draws > 1 || pattern_draws > 1 || health_draws > 1 {
            return None;
        }
        if health_draws != 0 && !input.player_health_state_known {
            return None;
        }

        let mut next_rng = *rng;
        let pressure_random_eye = if pressure_draws != 0 {
            next_rng.next_u32()?
        } else {
            0
        };
        let pattern_random_mod5 = if pattern_draws != 0 {
            (next_rng.next_u32()? % 5) as u8
        } else {
            0
        };
        let health_timer_random_unit = if health_draws != 0 {
            next_rng.next_unit_f32()?
        } else {
            0.0
        };
        let health_spawn_random_unit = if health_spawn_requested {
            next_rng.next_unit_f32()?
        } else {
            0.0
        };

        let mut next = self.clone();
        let next_input = phase_input(
            &next,
            pressure_random_eye,
            pattern_random_mod5,
            health_timer_random_unit,
        );
        let mut step = next.step_controller_phase(patterns, next_input);
        let actual_pressure_draws = step
            .controller
            .eye_pressure
            .map(|step| step.random_draws_used)
            .unwrap_or_default();
        let actual_pattern_draws = step
            .controller
            .pattern
            .map(|step| step.random_draws_used)
            .unwrap_or_default();
        let actual_health_draws = step
            .controller
            .health
            .map(|step| step.random_draws_used)
            .unwrap_or_default();
        let actual_health_spawn_requested = step
            .controller
            .health
            .is_some_and(|health| health.spawn_requested);
        if (
            actual_pressure_draws,
            actual_pattern_draws,
            actual_health_draws,
        ) != (pressure_draws, pattern_draws, health_draws)
            || actual_health_spawn_requested != health_spawn_requested
        {
            return None;
        }
        if actual_health_spawn_requested {
            if next
                .health_pickup
                .is_some_and(|pickup| pickup.controller_owned)
            {
                return None;
            }
            let spawn_plan = sweeper_boss_health_pickup_spawn_plan(health_spawn_random_unit);
            next.health_pickup = Some(NativeSweeperBossOwnedHealthPickupRuntime::new(spawn_plan));
            step.health_pickup_spawn_plan = Some(spawn_plan);
        }
        next.apply_owned_eye_pressure(step.controller.eye_pressure);

        *self = next;
        *rng = next_rng;
        Some(step)
    }

    /// TriggerManager-owned Sweeper controller phase. This stops before any Eye,
    /// Transporter, Ratchet Handler, animator, or end-of-tick XItemManager work.
    pub(crate) fn step_controller_phase(
        &mut self,
        patterns: &RobotsSweeperBossPatterns,
        input: NativeSweeperBossControllerPhaseInput,
    ) -> NativeSweeperBossControllerPhaseStep {
        let observed_ack = self.ratchet.ratchet.damage_acknowledgement;
        // 0x004CDF10 refreshes +0x136 before damage/pattern work. A Transporter
        // spawn from the previous XItem phase therefore clears the blocker here.
        let transporter_latch_cleared_before_controller =
            self.transporter.refresh_before_controller();
        let controller_live_ai_character_count = self.live_ai.registered_count();
        let mut controller_pattern_eyes = input.pattern_eyes;
        for (eye, runtime) in controller_pattern_eyes
            .iter_mut()
            .zip(self.eye_commands.iter())
        {
            eye.command_state = runtime.command_state;
        }
        let controller = self.controller.step_controller_tick(
            patterns,
            NativeSweeperBossControllerTickInput {
                ratchet_damage_acknowledgement: observed_ack,
                eye_lifecycle_states: input.eye_lifecycle_states,
                pattern_eyes: controller_pattern_eyes,
                live_ai_character_count: controller_live_ai_character_count,
                transporter_active: self.transporter.controller_latch,
                health_pickup_active: input.health_pickup_active,
                player_handler_present: input.player_handler_present,
                player_current_health: input.player_current_health,
                player_max_health: input.player_max_health,
                pressure_random_eye: input.pressure_random_eye,
                pattern_random_mod5: input.pattern_random_mod5,
                health_timer_random_unit: input.health_timer_random_unit,
            },
        );
        let cleared_ack = controller.damage_phase.clear_acknowledgement;
        if cleared_ack {
            self.ratchet
                .ratchet
                .controller_consumed_damage_acknowledgement();
        }
        let ai_cleanup_marked = if controller.damage_phase.cleanup_live_ai_characters {
            self.live_ai.mark_all_pending_destroy()
        } else {
            0
        };
        if controller.damage_phase.dispatch_controller_link6_event {
            self.trigger_events
                .push(NativeSweeperBossTriggerEvent::ControllerLink6CommonMask1);
        }

        // Controller writes the five Eye command fields during TriggerManager.
        // Their priority-0x14 Handler clocks run later in the XItem phase.
        if let Some(pattern) = controller.pattern {
            for (runtime, command) in self.eye_commands.iter_mut().zip(pattern.commands) {
                if let Some(command) = command {
                    runtime.assign(command);
                }
            }
        }

        NativeSweeperBossControllerPhaseStep {
            controller_observed_ratchet_acknowledgement: observed_ack,
            controller_cleared_ratchet_acknowledgement: cleared_ack,
            controller_live_ai_character_count,
            ai_cleanup_marked,
            transporter_latch_cleared_before_controller,
            health_pickup_spawn_plan: None,
            controller,
        }
    }

    fn finish_xitem_pre_ratchet_after_eyes(
        &mut self,
        transporter_events: &[NativeSweeperBossTransporterEvent],
        eye_steps: [NativeSweeperBossEyeCommandStep; EYE_COUNT],
        spawn_requests: u32,
        spawn_random_draws_used: u8,
        transporter_activation_requests: u32,
        needs_more_eye_spawn_rng: bool,
    ) -> NativeSweeperBossXItemPreRatchetStep {
        let mut transporter_activations_applied = 0u32;
        for _ in 0..transporter_activation_requests {
            if self.transporter.activate_from_eye_case6() {
                transporter_activations_applied = transporter_activations_applied.wrapping_add(1);
            }
        }
        self.controller.record_eye_spawn_requests(spawn_requests);

        for event in transporter_events {
            if matches!(event, NativeSweeperBossTransporterEvent::ResetReload) {
                self.live_ai.release_transporter_carries();
            }
            self.transporter.apply_handler_event(*event);
        }
        let transporter_spawn_count_after_xitem_phase = self.transporter.successful_spawn_count;
        let ratchet_live_ai_character_count = self.live_ai.registered_count();
        let transporter_active_for_ratchet = self.transporter.controller_latch;
        let ratchet_blocking_handler_count =
            ratchet_live_ai_character_count.wrapping_add(u32::from(transporter_active_for_ratchet));

        NativeSweeperBossXItemPreRatchetStep {
            eye_steps,
            eye_spawn_requests_recorded_after_controller: spawn_requests,
            eye_spawn_random_draws_used: spawn_random_draws_used,
            transporter_activation_requests,
            transporter_activations_applied,
            transporter_spawn_count_after_xitem_phase,
            transporter_active_for_ratchet,
            ratchet_live_ai_character_count,
            ratchet_blocking_handler_count,
            needs_more_eye_spawn_rng,
        }
    }

    fn step_ratchet_after_pre(
        &mut self,
        scripts: &RobotsSweeperRatchetScripts,
        pre: NativeSweeperBossXItemPreRatchetStep,
        ratchet_current_yaw: f32,
        player_position: [f32; 4],
        anchors: [[f32; 4]; EYE_COUNT],
        relocation_target_draws: &[u32],
        ratchet_primary_random_mod100: u8,
        ratchet_secondary_random_mod100: u8,
    ) -> NativeSweeperRatchetFrameStep {
        self.ratchet.step_frame(
            scripts,
            NativeSweeperRatchetFrameInput {
                current_yaw: ratchet_current_yaw,
                player_position,
                anchors,
                relocation_target_draws,
                combat: NativeSweeperRatchetCombatInput {
                    primary_random_mod100: ratchet_primary_random_mod100,
                    secondary_random_mod100: ratchet_secondary_random_mod100,
                    blocking_handler_count: pre.ratchet_blocking_handler_count,
                },
            },
        )
    }

    fn finish_xitem_after_ratchet(
        &mut self,
        pre: NativeSweeperBossXItemPreRatchetStep,
        ratchet: NativeSweeperRatchetFrameStep,
    ) -> NativeSweeperBossXItemPhaseStep {
        let ai_flushed_end_of_tick = self.live_ai.flush_pending_destroy_end_of_tick();
        NativeSweeperBossXItemPhaseStep {
            ratchet_live_ai_character_count: pre.ratchet_live_ai_character_count,
            ratchet_blocking_handler_count: pre.ratchet_blocking_handler_count,
            ai_flushed_end_of_tick,
            eye_steps: pre.eye_steps,
            eye_spawn_requests_recorded_after_controller: pre
                .eye_spawn_requests_recorded_after_controller,
            eye_spawn_random_draws_used: pre.eye_spawn_random_draws_used,
            transporter_activation_requests: pre.transporter_activation_requests,
            transporter_activations_applied: pre.transporter_activations_applied,
            transporter_spawn_count_after_xitem_phase: pre
                .transporter_spawn_count_after_xitem_phase,
            transporter_active_for_ratchet: pre.transporter_active_for_ratchet,
            needs_more_eye_spawn_rng: pre.needs_more_eye_spawn_rng,
            ratchet,
        }
    }

    fn consume_owned_transporter_spawn_attempts(
        &mut self,
        patterns: &RobotsSweeperBossPatterns,
        config: Option<&NativeSweeperBossTransporterSpawnConfig>,
        attempts: u16,
        spawn_position: Option<[f32; 4]>,
        rng: &mut crate::map_runtime::RuntimeRobotsGlobalRngState,
    ) -> Option<(Vec<NativeSweeperBossSpawnFactoryRngStep>, u32)> {
        if attempts == 0 {
            return Some((Vec::new(), 0));
        }
        if self.transporter.trigger_inactive {
            return None;
        }
        let config = config?;
        if !config.targets_complete {
            return None;
        }
        let candidates = config.targets.iter().flatten().copied().collect::<Vec<_>>();
        if candidates.is_empty() {
            return Some((Vec::new(), 0));
        }

        let mut factory_steps = Vec::new();
        let mut random_draws_used = 0u32;
        for attempt_ordinal in 0..attempts {
            if self.transporter.successful_spawn_count >= config.max_successful_spawns
                || self.live_ai.transporter_carried_count() >= config.max_carried_monsters
            {
                break;
            }

            let selection_draw = rng.next_u32()?;
            random_draws_used = random_draws_used.wrapping_add(1);
            let target = candidates[(selection_draw % candidates.len() as u32) as usize];
            let file = patterns.monster_file(target.config_index)?;

            if let Some(existing_id) = self
                .live_ai
                .serialized_trigger_live_id(target.trigger_index)
            {
                let existing = self.live_ai.entry(existing_id)?;
                if existing.config_index != target.config_index || existing.file != Some(file) {
                    return None;
                }
            } else {
                let factory = sweeper_boss_consume_spawn_factory_rng(
                    NativeSweeperBossSpawnOrigin::Transporter {
                        target_trigger_index: target.trigger_index,
                        attempt_ordinal,
                    },
                    target.config_index,
                    patterns,
                    rng,
                )?;
                random_draws_used = random_draws_used.wrapping_add(factory.random_draws_used());
                let (_, created) = self
                    .live_ai
                    .register_serialized_trigger_after_native_create(
                        target.trigger_index,
                        target.config_index,
                        file,
                        spawn_position?,
                        target.yaw,
                    );
                if !created {
                    return None;
                }
                factory_steps.push(factory);
            }

            self.live_ai
                .transfer_serialized_trigger_to_transporter(target.trigger_index)?;
            self.transporter
                .apply_handler_event(NativeSweeperBossTransporterEvent::SuccessfulMonsterSpawn);
        }

        Some((factory_steps, random_draws_used))
    }

    /// Transactional owned-RNG replay slice from Eye priority-0x14 work through Ratchet and the
    /// boss-owned end-of-tick cleanup. Constructor/setup draws are consumed before fresh AI is
    /// visible to Ratchet; relocation and combat draws are then derived from the post-Eye state.
    /// Equal-priority AI updates that run after Ratchet remain a separate scheduler slice.
    pub(crate) fn step_xitem_phase_owned_rng(
        &mut self,
        patterns: &RobotsSweeperBossPatterns,
        scripts: &RobotsSweeperRatchetScripts,
        eye_scripts: &super::sweeper_boss_script::RobotsSweeperEyeScripts,
        rng: &mut crate::map_runtime::RuntimeRobotsGlobalRngState,
        input: NativeSweeperBossOwnedXItemPhaseInput<'_>,
    ) -> Option<NativeSweeperBossOwnedXItemPhaseStep> {
        let mut next = self.clone();
        let mut next_rng = *rng;
        let mut pre_ratchet_rng = sweeper_boss_consume_ready_eye_pre_ratchet_rng(
            &next.eye_commands,
            patterns,
            &mut next_rng,
        )?;
        let eye_phase = next.step_owned_eye_xitem_phase(
            patterns,
            eye_scripts,
            input.eye_spawn_sources,
            &pre_ratchet_rng.variant_random_mod100_draws,
        );
        let spawn_requests = eye_phase
            .command_steps
            .iter()
            .fold(0u32, |total, step| total.wrapping_add(step.spawn_requests));
        let needs_more_eye_spawn_rng = eye_phase
            .command_steps
            .iter()
            .any(|step| step.needs_more_spawn_rng);
        if !input.transporter_events_known
            && (!next.transporter.trigger_inactive
                || next.transporter.controller_latch
                || eye_phase.transporter_activation_requests != 0
                || input.transporter_spawn_attempts != 0)
        {
            return None;
        }
        if needs_more_eye_spawn_rng
            || usize::from(eye_phase.random_draws_used)
                != pre_ratchet_rng.variant_random_mod100_draws.len()
        {
            return None;
        }
        if input.transporter_spawn_attempts != 0
            && input
                .transporter_events
                .iter()
                .any(|event| matches!(event, NativeSweeperBossTransporterEvent::ResetReload))
        {
            return None;
        }
        let (transporter_factory_steps, transporter_random_draws_used) = next
            .consume_owned_transporter_spawn_attempts(
                patterns,
                input.transporter_spawn,
                input.transporter_spawn_attempts,
                input
                    .transporter_spawn_position
                    .or(input.transporter_owner_position),
                &mut next_rng,
            )?;
        pre_ratchet_rng.random_draws_used = pre_ratchet_rng
            .random_draws_used
            .wrapping_add(transporter_random_draws_used);
        pre_ratchet_rng
            .factory_steps
            .extend(transporter_factory_steps);
        let pre = next.finish_xitem_pre_ratchet_after_eyes(
            input.transporter_events,
            eye_phase.command_steps,
            spawn_requests,
            eye_phase.random_draws_used,
            eye_phase.transporter_activation_requests,
            needs_more_eye_spawn_rng,
        );

        let registered_post_ratchet_ai = next
            .post_ratchet_ai
            .register_factory_steps(&next.live_ai, &pre_ratchet_rng.factory_steps)?;
        if registered_post_ratchet_ai
            != pre_ratchet_rng.factory_steps.len().min(u32::MAX as usize) as u32
        {
            return None;
        }

        next.step_owned_ratchet_projectiles_pre_ratchet(
            input.player_position,
            input.transporter_owner_position,
        )?;

        let mut relocation_target_draws = Vec::new();
        if next
            .ratchet
            .ratchet
            .needs_relocation_target_rng(next.ratchet.script_value_status)
        {
            let current_eye = next.ratchet.ratchet.current_eye;
            for _ in 0..256 {
                let draw = next_rng.next_u32()?;
                relocation_target_draws.push(draw);
                if (draw % EYE_COUNT as u32) as u8 != current_eye {
                    break;
                }
            }
            if relocation_target_draws
                .last()
                .is_none_or(|draw| (draw % EYE_COUNT as u32) as u8 == current_eye)
            {
                return None;
            }
        }

        let (ratchet_primary_random_mod100, ratchet_secondary_random_mod100, combat_draws) =
            if next.ratchet.ratchet.top_state == 4 {
                let primary = (next_rng.next_u32()? % 100) as u8;
                let draws = next
                    .ratchet
                    .ratchet
                    .combat
                    .random_draws_required(pre.ratchet_blocking_handler_count, primary);
                if !(1..=2).contains(&draws) {
                    return None;
                }
                let secondary = if draws == 2 {
                    (next_rng.next_u32()? % 100) as u8
                } else {
                    0
                };
                (primary, secondary, draws)
            } else {
                (0, 0, 0)
            };

        let ratchet_current_yaw = next.ratchet_owner_yaw?;
        let ratchet = next.step_ratchet_after_pre(
            scripts,
            pre,
            ratchet_current_yaw,
            input.player_position,
            input.anchors,
            &relocation_target_draws,
            ratchet_primary_random_mod100,
            ratchet_secondary_random_mod100,
        );
        if ratchet.ratchet.needs_more_relocation_rng
            || usize::from(ratchet.ratchet.relocation_random_draws_used)
                != relocation_target_draws.len()
            || ratchet.ratchet.combat_random_draws_used != combat_draws
        {
            return None;
        }
        next.ratchet_owner_yaw = ratchet
            .ratchet
            .forced_yaw
            .or(ratchet.ratchet.facing_yaw)
            .or(Some(ratchet_current_yaw));

        if ratchet.script.missile_event {
            next.register_owned_ratchet_projectile(
                ratchet.ratchet.owner_position?,
                next.ratchet_owner_yaw?,
                input.ratchet_missile_hand_local?,
                input.player_position,
            )?;
        }

        let post_ratchet_ai = next.post_ratchet_ai.step_owned_rng(
            &next.live_ai,
            input.player_position,
            input.roller_path_graph,
            &mut next_rng,
        )?;
        let health_pickup = if let Some(pickup) = next.health_pickup.as_mut() {
            Some(pickup.step_fixed_update(input.health_pickup_valid_hit)?)
        } else {
            None
        };
        if health_pickup.is_some_and(|step| step.completed) {
            next.health_pickup = None;
        }
        // Latch producer risk before the native end-of-tick destroy flush. A Monster may be
        // created earlier in this XItem slice and become pending-destroy during Ratchet cleanup
        // in the same frame; checking only after the flush would miss that transient producer.
        if next.live_ai.has_pickup_drop_capable_character(patterns) {
            next.mark_post_xitem_pickup_dynamic_risk();
        }
        let xitem = next.finish_xitem_after_ratchet(pre, ratchet);

        *self = next;
        *rng = next_rng;
        Some(NativeSweeperBossOwnedXItemPhaseStep {
            post_ratchet_ai,
            health_pickup,
            pre_ratchet_rng,
            xitem,
        })
    }
}

// Exact Ratchet combat reducer used by the live boss state machine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeSweeperRatchetCombatState {
    pub phase: u8,
    pub action: u8,
    pub counter: f32,
    pub command_latch: u8,
}

impl Default for NativeSweeperRatchetCombatState {
    fn default() -> Self {
        Self {
            phase: 0,
            action: 0,
            counter: 0.0,
            command_latch: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativeSweeperRatchetCombatInput {
    /// The first native `FUN_00509C48() % 100` draw. Native consumes this every update,
    /// including the first fifty phase-0 updates that do not select an AnimMode.
    pub primary_random_mod100: u8,
    /// The second `% 100` draw. Native consumes this only when selecting one of the three
    /// IdleCombat modes.
    pub secondary_random_mod100: u8,
    /// Native count after combining the controller `+0x136` flag with matching live manager
    /// handlers derived from descriptor `0x005E217C`.
    pub blocking_handler_count: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativeSweeperRatchetCombatStep {
    pub requested_anim_mode: Option<u32>,
    pub random_draws_used: u8,
}

impl NativeSweeperRatchetCombatState {
    /// Number of process-global `%100` draws the next native combat update consumes after the
    /// first value is known. `FUN_004CFE00` always consumes the primary draw; only the
    /// IdleCombat selector consumes a secondary draw.
    pub(crate) fn random_draws_required(
        &self,
        blocking_handler_count: u32,
        primary_random_mod100: u8,
    ) -> u8 {
        let primary = primary_random_mod100 % 100;
        match self.phase {
            0 => {
                if self.counter + 1.0 <= 50.0 || blocking_handler_count == 0 || primary < 50 {
                    1
                } else {
                    2
                }
            }
            2 if primary >= 50 => 2,
            _ => 1,
        }
    }

    pub(crate) fn step(
        &mut self,
        input: NativeSweeperRatchetCombatInput,
    ) -> NativeSweeperRatchetCombatStep {
        self.counter += 1.0;
        let primary = input.primary_random_mod100 % 100;
        let secondary = input.secondary_random_mod100 % 100;
        let mut random_draws_used = 1;

        let request = match self.phase {
            0 => {
                if self.counter <= 50.0 {
                    None
                } else {
                    self.phase = 1;
                    self.counter = 0.0;
                    if input.blocking_handler_count == 0 {
                        if primary < 66 {
                            self.action = 8;
                            Some(ATTACK2_ANIM_MODE)
                        } else {
                            self.action = 6;
                            Some(ATTACK_ANIM_MODE)
                        }
                    } else if primary < 50 {
                        self.action = 6;
                        self.phase = 2;
                        Some(ATTACK_ANIM_MODE)
                    } else {
                        self.action = 5;
                        random_draws_used = 2;
                        Some(idle_combat_mode(secondary))
                    }
                }
            }
            1 => {
                self.phase = 2;
                self.action = 6;
                Some(ATTACK_ANIM_MODE)
            }
            2 => {
                self.phase = 3;
                if primary < 50 {
                    self.action = 2;
                    self.phase = 0;
                    Some(DISAPPEAR_ANIM_MODE)
                } else {
                    self.action = 5;
                    random_draws_used = 2;
                    Some(idle_combat_mode(secondary))
                }
            }
            3 => {
                self.action = 2;
                self.phase = 0;
                Some(DISAPPEAR_ANIM_MODE)
            }
            _ => None,
        };

        if request.is_some() {
            self.command_latch = 0;
        }
        NativeSweeperRatchetCombatStep {
            requested_anim_mode: request,
            random_draws_used,
        }
    }
}

fn idle_combat_mode(random_mod100: u8) -> u32 {
    if random_mod100 < 33 {
        IDLE_COMBAT1_ANIM_MODE
    } else if random_mod100 < 66 {
        IDLE_COMBAT2_ANIM_MODE
    } else {
        IDLE_COMBAT3_ANIM_MODE
    }
}

#[path = "sweeper_boss_post_ratchet_ai.rs"]
mod post_ratchet_ai;
pub(crate) use post_ratchet_ai::*;
