mod boss_ratchet;
mod camera;
mod camera_marker;
mod fluid;
mod lift;
mod monster;
mod monster_transporter;
mod npc;
mod object_audio;
mod platform;
mod sweeper_boss;
mod sweeper_boss_eye;
mod sweeper_boss_map;
mod sweeper_boss_script;
mod vehicle;
mod watchbot;

pub use camera::{
    controller_plan as robots_camera_controller_plan,
    viewport_runtime as robots_camera_viewport_runtime, NativeCameraFadeTransitionRuntime,
    NativeCameraOwnershipRuntime, NativeCameraSequenceRuntime, NativeCameraShakeRuntime,
    NativeCameraViewportPose, NativeCameraViewportRuntime, NativeDefaultPlayerCameraRuntime,
    NATIVE_FADE_HIGH_THRESHOLD, NATIVE_FADE_IN_STATE, NATIVE_FADE_LOW_THRESHOLD,
    NATIVE_FADE_OUT_STATE,
};
pub(crate) use fluid::{
    initial_shared_rng_draw_count as robots_fluid_initial_shared_rng_draw_count,
    TYPE as ROBOTS_FLUID_TYPE,
};
pub(crate) use monster_transporter::{
    route_distance_squared as robots_monster_transporter_route_distance_squared,
    NativeMonsterTransporterFixedStep, NativeMonsterTransporterPathEvent,
    NativeMonsterTransporterRuntime,
};
pub use object_audio::{
    direct_profile as robots_direct_object_audio_profile,
    is_consumer as robots_object_audio_is_consumer, is_enabled as robots_object_audio_is_enabled,
    profile_for_source as robots_object_audio_profile_for_source, ObjectAudioProfile,
};
#[cfg(test)]
pub(crate) use sweeper_boss::NativeSweeperBossTriggerEvent;
pub(crate) use sweeper_boss::{
    sweeper_boss_spawn_selection as robots_sweeper_boss_spawn_selection,
    sweeper_boss_spawn_transform as robots_sweeper_boss_spawn_transform,
    sweeper_health_pickup_player_contact_guaranteed_miss,
    sweeper_ratchet_anchor as robots_sweeper_ratchet_anchor, NativeSweeperBossControllerSnapshot,
    NativeSweeperBossGenericAiBootstrap, NativeSweeperBossLiveAiSource,
    NativeSweeperBossOwnedControllerPhaseInput, NativeSweeperBossOwnedXItemPhaseInput,
    NativeSweeperBossReplayRuntime, NativeSweeperBossSpawnSelection,
    NativeSweeperBossSpawnTransform, NativeSweeperBossTransporterEvent, RobotsSweeperBossPatterns,
    RobotsSweeperRatchetScripts, APPEAR_ANIM_MODE as ROBOTS_SWEEPER_APPEAR_ANIM_MODE,
    ATTACK_ANIMATION as ROBOTS_SWEEPER_ATTACK_ANIMATION,
    ATTACK_ANIM_MODE as ROBOTS_SWEEPER_ATTACK_ANIM_MODE,
    ATTACK_ANIM_SET as ROBOTS_SWEEPER_ATTACK_ANIM_SET,
    ATTACK_SCRIPT as ROBOTS_SWEEPER_ATTACK_SCRIPT, BOSS_ANIM_MODE as ROBOTS_SWEEPER_BOSS_ANIM_MODE,
    CONTROLLER_RUNTIME_CLASS_CODE as ROBOTS_SWEEPER_CONTROLLER_RUNTIME_CLASS_CODE,
    CONTROLLER_SAVE_SIZE as ROBOTS_SWEEPER_CONTROLLER_SAVE_SIZE,
    CONTROLLER_SERVICE_RADIUS as ROBOTS_SWEEPER_CONTROLLER_SERVICE_RADIUS,
    CONTROLLER_TYPE as ROBOTS_SWEEPER_CONTROLLER_TYPE, EYE_COUNT as ROBOTS_SWEEPER_EYE_COUNT,
    EYE_INITIAL_HIT_POINTS as ROBOTS_SWEEPER_EYE_INITIAL_HIT_POINTS,
    EYE_OPEN_SECONDS as ROBOTS_SWEEPER_EYE_OPEN_SECONDS,
    EYE_PRESSURE_THRESHOLD as ROBOTS_SWEEPER_EYE_PRESSURE_THRESHOLD,
    EYE_TYPE as ROBOTS_SWEEPER_EYE_TYPE,
    HEALTH_PICKUP_INVENTORY_ADD_EVENT as ROBOTS_SWEEPER_HEALTH_PICKUP_INVENTORY_ADD_EVENT,
    HEALTH_PICKUP_ITEM as ROBOTS_SWEEPER_HEALTH_PICKUP_ITEM,
    HEALTH_PICKUP_LIFETIME_SECONDS as ROBOTS_SWEEPER_HEALTH_PICKUP_LIFETIME_SECONDS,
    HEALTH_PICKUP_REGISTRATION_MASK as ROBOTS_SWEEPER_HEALTH_PICKUP_REGISTRATION_MASK,
    HEALTH_PICKUP_ROTATION_PER_UPDATE as ROBOTS_SWEEPER_HEALTH_PICKUP_ROTATION_PER_UPDATE,
    HEALTH_PICKUP_SCRIPT as ROBOTS_SWEEPER_HEALTH_PICKUP_SCRIPT,
    HEALTH_PICKUP_SPAWN_X_BIAS as ROBOTS_SWEEPER_HEALTH_PICKUP_SPAWN_X_BIAS,
    HEALTH_PICKUP_SPAWN_X_SCALE as ROBOTS_SWEEPER_HEALTH_PICKUP_SPAWN_X_SCALE,
    HEALTH_PICKUP_SPAWN_Z as ROBOTS_SWEEPER_HEALTH_PICKUP_SPAWN_Z,
    HEALTH_PICKUP_TIMER_INITIAL_SECONDS as ROBOTS_SWEEPER_HEALTH_PICKUP_TIMER_INITIAL_SECONDS,
    HEALTH_PICKUP_TIMER_JITTER_SECONDS as ROBOTS_SWEEPER_HEALTH_PICKUP_TIMER_JITTER_SECONDS,
    HEALTH_PICKUP_WAIT_FOR_HIT_EVENT as ROBOTS_SWEEPER_HEALTH_PICKUP_WAIT_FOR_HIT_EVENT,
    INITIAL_DIFFICULTY as ROBOTS_SWEEPER_INITIAL_DIFFICULTY,
    JUMP_LEFT_ANIM_MODE as ROBOTS_SWEEPER_JUMP_LEFT_ANIM_MODE,
    JUMP_RIGHT_ANIM_MODE as ROBOTS_SWEEPER_JUMP_RIGHT_ANIM_MODE,
    MAX_DIFFICULTY as ROBOTS_SWEEPER_MAX_DIFFICULTY, MISSILE_EVENT as ROBOTS_SWEEPER_MISSILE_EVENT,
    MISSILE_LAUNCH_BONE as ROBOTS_SWEEPER_MISSILE_LAUNCH_BONE,
    MISSILE_LIVE_BONE_FRAME as ROBOTS_SWEEPER_MISSILE_LIVE_BONE_FRAME,
    MISSILE_RESOURCE_FILE as ROBOTS_SWEEPER_MISSILE_RESOURCE_FILE,
    MISSILE_SCRIPT as ROBOTS_SWEEPER_MISSILE_SCRIPT,
    MONSTER_CREATE_RESOURCE as ROBOTS_SWEEPER_MONSTER_CREATE_RESOURCE,
    MONSTER_PHYSICS_DESCRIPTOR as ROBOTS_SWEEPER_MONSTER_PHYSICS_DESCRIPTOR,
    MONSTER_RUNTIME_TYPE as ROBOTS_SWEEPER_MONSTER_RUNTIME_TYPE,
    MONSTER_UPDATE_REGISTRATION_MASK as ROBOTS_SWEEPER_MONSTER_UPDATE_REGISTRATION_MASK,
    MONSTER_UPDATE_REGISTRATION_PRIORITY as ROBOTS_SWEEPER_MONSTER_UPDATE_REGISTRATION_PRIORITY,
    MONSTER_XITEM_REGISTRATION_FLAGS as ROBOTS_SWEEPER_MONSTER_XITEM_REGISTRATION_FLAGS,
    PATTERN_FILE as ROBOTS_SWEEPER_PATTERN_FILE,
    PATTERN_ROW_COUNT as ROBOTS_SWEEPER_PATTERN_ROW_COUNT,
    RAT_ARRIVAL_YAW as ROBOTS_SWEEPER_RAT_ARRIVAL_YAW,
    RAT_BASE_SCRIPT as ROBOTS_SWEEPER_RAT_BASE_SCRIPT,
    RAT_DAMAGE_PHASE_DIFFICULTY as ROBOTS_SWEEPER_RAT_DAMAGE_PHASE_DIFFICULTY,
    RAT_DEATH_ANIM_MODE as ROBOTS_SWEEPER_RAT_DEATH_ANIM_MODE,
    RAT_DEATH_ANIM_SET as ROBOTS_SWEEPER_RAT_DEATH_ANIM_SET,
    RAT_DEATH_SCRIPT as ROBOTS_SWEEPER_RAT_DEATH_SCRIPT,
    RAT_DEATH_SIGNAL_FRAME as ROBOTS_SWEEPER_RAT_DEATH_SIGNAL_FRAME,
    RAT_ENTITY as ROBOTS_SWEEPER_RAT_ENTITY,
    RAT_FACE_PLAYER_ALPHA as ROBOTS_SWEEPER_RAT_FACE_PLAYER_ALPHA,
    RAT_HIT_BACK_ANIM_MODE as ROBOTS_SWEEPER_RAT_HIT_BACK_ANIM_MODE,
    RAT_HIT_BACK_ANIM_SET as ROBOTS_SWEEPER_RAT_HIT_BACK_ANIM_SET,
    RAT_HIT_BACK_SCRIPT as ROBOTS_SWEEPER_RAT_HIT_BACK_SCRIPT,
    RAT_HIT_BACK_SIGNAL_FRAME as ROBOTS_SWEEPER_RAT_HIT_BACK_SIGNAL_FRAME,
    RAT_HIT_FORWARD_ANIM_MODE as ROBOTS_SWEEPER_RAT_HIT_FORWARD_ANIM_MODE,
    RAT_HIT_FORWARD_ANIM_SET as ROBOTS_SWEEPER_RAT_HIT_FORWARD_ANIM_SET,
    RAT_HIT_FORWARD_SCRIPT as ROBOTS_SWEEPER_RAT_HIT_FORWARD_SCRIPT,
    RAT_HIT_FORWARD_SIGNAL_FRAME as ROBOTS_SWEEPER_RAT_HIT_FORWARD_SIGNAL_FRAME,
    RAT_HIT_FORWARD_THRESHOLD as ROBOTS_SWEEPER_RAT_HIT_FORWARD_THRESHOLD,
    RAT_INITIAL_HIT_POINTS as ROBOTS_SWEEPER_RAT_INITIAL_HIT_POINTS,
    RAT_JUMP_LEFT_YAW as ROBOTS_SWEEPER_RAT_JUMP_LEFT_YAW,
    RAT_JUMP_RIGHT_YAW as ROBOTS_SWEEPER_RAT_JUMP_RIGHT_YAW,
    RAT_POSITION_DATUM as ROBOTS_SWEEPER_RAT_POSITION_DATUM,
    RAT_RESOURCE_FILE as ROBOTS_SWEEPER_RAT_RESOURCE_FILE,
    RAT_SCRIPT_VALUE_EVENT as ROBOTS_SWEEPER_RAT_SCRIPT_VALUE_EVENT,
    RAT_SCRIPT_VALUE_SIGNAL_1 as ROBOTS_SWEEPER_RAT_SCRIPT_VALUE_SIGNAL_1,
    RAT_SCRIPT_VALUE_SIGNAL_2 as ROBOTS_SWEEPER_RAT_SCRIPT_VALUE_SIGNAL_2,
};
pub(crate) use sweeper_boss_map::{
    resolve_sweeper_boss_map_bindings, NativeSweeperBossMapBindings,
};
pub(crate) use sweeper_boss_script::RobotsSweeperEyeScripts;

#[cfg(test)]
pub(crate) use sweeper_boss::{
    NativeSweeperRatchetScriptEventKind, ATTACK2_ANIM_MODE as ROBOTS_SWEEPER_ATTACK2_ANIM_MODE,
};

pub fn robots_trigger_path_hash(trigger_type: u32, data: &[Option<u32>]) -> Option<u32> {
    let hash = match trigger_type {
        boss_ratchet::TYPE => boss_ratchet::path_hash(data),
        camera::TYPE => camera::path_hash(data),
        camera_marker::TYPE => camera_marker::path_hash(data),
        platform::TYPE => platform::path_hash(data),
        lift::TYPE => lift::path_hash(data),
        trigger_type if monster::is_base_type(trigger_type) => {
            monster::path_hash(trigger_type, data)
        }
        monster_transporter::TYPE => monster_transporter::primary_path_hash(data),
        vehicle::TYPE => vehicle::path_hash(data),
        watchbot::TYPE => watchbot::path_hash(data),
        _ => None,
    }?;
    (!matches!(hash, 0 | u32::MAX | 0x0B00_0000)).then_some(hash)
}

pub fn robots_trigger_path_data_slot(trigger_type: u32) -> Option<usize> {
    match trigger_type {
        boss_ratchet::TYPE => Some(0),
        camera::TYPE => Some(1),
        camera_marker::TYPE => Some(4),
        platform::TYPE => Some(2),
        lift::TYPE | monster_transporter::TYPE | vehicle::TYPE | watchbot::TYPE => Some(1),
        trigger_type if monster::is_base_type(trigger_type) => Some(2),
        _ => None,
    }
}

pub fn robots_camera_mode(trigger_type: u32, data: &[Option<u32>]) -> Option<u32> {
    (trigger_type == camera::TYPE)
        .then(|| camera::mode(data))
        .flatten()
}

pub fn robots_camera_scaled_data4(trigger_type: u32, data: &[Option<u32>]) -> Option<f32> {
    (trigger_type == camera::TYPE)
        .then(|| camera::scaled_data4(data))
        .flatten()
}

pub fn robots_camera_scaled_data5(trigger_type: u32, data: &[Option<u32>]) -> Option<f32> {
    (trigger_type == camera::TYPE)
        .then(|| camera::scaled_data5(data))
        .flatten()
}

pub fn robots_camera_flags(trigger_type: u32, data: &[Option<u32>]) -> Option<u32> {
    match trigger_type {
        camera::TYPE => camera::flags(data),
        camera_marker::TYPE => camera_marker::flags(data),
        _ => None,
    }
}

pub fn robots_camera_marker_scaled_data0(trigger_type: u32, data: &[Option<u32>]) -> Option<f32> {
    (trigger_type == camera_marker::TYPE)
        .then(|| camera_marker::scaled_data0(data))
        .flatten()
}

pub fn robots_monster_transporter_secondary_path_hash(
    trigger_type: u32,
    data: &[Option<u32>],
) -> Option<u32> {
    if trigger_type != monster_transporter::TYPE {
        return None;
    }
    let hash = monster_transporter::secondary_path_hash(data)?;
    (!matches!(hash, 0 | u32::MAX | 0x0B00_0000)).then_some(hash)
}

pub fn robots_trigger_path_is_proven(
    trigger_type: u32,
    data: &[Option<u32>],
    path_hashcode: u32,
) -> bool {
    robots_trigger_path_hash(trigger_type, data) == Some(path_hashcode)
        || robots_monster_transporter_secondary_path_hash(trigger_type, data) == Some(path_hashcode)
}

pub fn robots_monster_is_family(trigger_type: u32) -> bool {
    monster::is_family_type(trigger_type)
}

pub fn robots_monster_runtime_selector(trigger_type: u32, data: &[Option<u32>]) -> Option<u32> {
    monster::runtime_selector(trigger_type, data)
}

pub fn robots_monster_proximity_radius(trigger_type: u32, data: &[Option<u32>]) -> Option<f32> {
    monster::proximity_radius(trigger_type, data)
}

pub fn robots_monster_test_runtime_value(trigger_type: u32, data: &[Option<u32>]) -> Option<u32> {
    monster::test_runtime_value(trigger_type, data)
}

pub fn robots_monster_data4_value(trigger_type: u32, data: &[Option<u32>]) -> Option<u32> {
    monster::data4_value(trigger_type, data)
}

pub fn robots_monster_flags(trigger_type: u32, data: &[Option<u32>]) -> Option<u32> {
    monster::flags(trigger_type, data)
}

pub fn robots_monster_data15_value(trigger_type: u32, data: &[Option<u32>]) -> Option<u32> {
    monster::data15_value(trigger_type, data)
}

pub fn robots_npc_runtime_selector(trigger_type: u32, data: &[Option<u32>]) -> Option<u32> {
    (trigger_type == npc::TYPE)
        .then(|| npc::runtime_selector(data))
        .flatten()
}

pub fn robots_npc_runtime_uid(trigger_type: u32, data: &[Option<u32>]) -> Option<u32> {
    (trigger_type == npc::TYPE)
        .then(|| npc::runtime_uid(data))
        .flatten()
}

pub fn robots_npc_flags(trigger_type: u32, data: &[Option<u32>]) -> Option<u32> {
    (trigger_type == npc::TYPE)
        .then(|| npc::flags(data))
        .flatten()
}

pub fn robots_npc_text_group(trigger_type: u32, data: &[Option<u32>]) -> Option<u32> {
    (trigger_type == npc::TYPE)
        .then(|| npc::text_group(data))
        .flatten()
}

pub fn robots_npc_alternate_cutscenes(
    trigger_type: u32,
    data: &[Option<u32>],
) -> Option<[Option<u32>; 4]> {
    (trigger_type == npc::TYPE).then(|| npc::alternate_cutscenes(data))
}

pub fn robots_npc_cutscene_is_null(hash: u32) -> bool {
    npc::is_null_cutscene(hash)
}

pub fn robots_watchbot_mode(trigger_type: u32, data: &[Option<u32>]) -> Option<u32> {
    (trigger_type == watchbot::TYPE)
        .then(|| watchbot::mode(data))
        .flatten()
}

pub fn robots_watchbot_flags(trigger_type: u32, data: &[Option<u32>]) -> Option<u32> {
    (trigger_type == watchbot::TYPE)
        .then(|| watchbot::flags(data))
        .flatten()
}

pub fn robots_watchbot_enter_distance(trigger_type: u32, data: &[Option<u32>]) -> Option<f32> {
    (trigger_type == watchbot::TYPE)
        .then(|| watchbot::enter_distance(data))
        .flatten()
}

pub fn robots_watchbot_leave_distance(trigger_type: u32, data: &[Option<u32>]) -> Option<f32> {
    (trigger_type == watchbot::TYPE)
        .then(|| watchbot::leave_distance(data))
        .flatten()
}

pub fn robots_monster_transporter_path_speed(
    trigger_type: u32,
    data: &[Option<u32>],
) -> Option<f32> {
    (trigger_type == monster_transporter::TYPE)
        .then(|| monster_transporter::path_speed(data))
        .flatten()
}

pub fn robots_trigger_runtime_path_speed(trigger_type: u32, data: &[Option<u32>]) -> Option<f32> {
    match trigger_type {
        platform::TYPE => platform::speed(data),
        lift::TYPE => lift::speed(data),
        vehicle::TYPE => vehicle::speed(data),
        _ => None,
    }
}

pub fn robots_trigger_runtime_path_acceleration(
    trigger_type: u32,
    data: &[Option<u32>],
) -> Option<f32> {
    match trigger_type {
        platform::TYPE => platform::acceleration(data),
        lift::TYPE => lift::acceleration(data),
        vehicle::TYPE => vehicle::acceleration(data),
        _ => None,
    }
}

pub fn robots_trigger_platform_angular_velocity(
    trigger_type: u32,
    data: &[Option<u32>],
) -> Option<glam::Vec3> {
    (trigger_type == platform::TYPE)
        .then(|| platform::angular_velocity(data))
        .flatten()
}

pub(super) fn float(data: &[Option<u32>], slot: usize) -> Option<f32> {
    let value = f32::from_bits(data.get(slot).copied().flatten()?);
    value.is_finite().then_some(value)
}

pub(super) fn scaled_speed(value: Option<f32>) -> f32 {
    let speed = value.unwrap_or_default() * 0.1;
    if speed.abs() > f32::EPSILON {
        speed.abs()
    } else {
        1.0
    }
}
