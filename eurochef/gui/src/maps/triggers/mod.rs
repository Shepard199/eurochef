mod boss_ratchet;
mod camera;
mod camera_marker;
mod lift;
mod monster;
mod monster_transporter;
mod npc;
mod object_audio;
mod platform;
mod vehicle;
mod watchbot;

pub use camera::controller_plan as robots_camera_controller_plan;
pub use object_audio::{
    direct_profile as robots_direct_object_audio_profile,
    is_consumer as robots_object_audio_is_consumer, is_enabled as robots_object_audio_is_enabled,
    profile_for_source as robots_object_audio_profile_for_source, ObjectAudioProfile,
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
