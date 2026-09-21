use super::{
    attachment_rotation::postmultiply_local_y_rotation,
    follow_network_path::RobotsFollowNetworkPathConfig,
    generic_attack::RobotsGenericAttackConfig,
    locomotion::{ai_character_physics_velocity_from_scalar, ROBOTS_FIXED_STEP_SECONDS},
};

pub const ROBOTS_EW09_FILE_UID: u32 = 0x0100_0045;
pub const ROBOTS_EW09_ATTACHMENT_ANIMATION_UID: u32 = 0x0300_0012;
pub const ROBOTS_EW09_ATTACHMENT_BONE_UID: u32 = 0x0E00_0070;
pub const ROBOTS_EW09_HANDLER_FLAGS_OR: u32 = 0x0008_0000;

pub const ROBOTS_EW09_SPEED_MIN: f32 = 2.0;
pub const ROBOTS_EW09_SPEED_MAX: f32 = 8.0;
pub const ROBOTS_EW09_PATROL_SCALAR: f32 = 0.4;
pub const ROBOTS_EW09_ATTACK_MOVEMENT_SCALAR: f32 = 0.4;
pub const ROBOTS_EW09_FOLLOW_PATH_PRIORITY: u8 = 0x10;
pub const ROBOTS_EW09_FOLLOW_PATH_SCALAR: f32 = 0.4;
pub const ROBOTS_EW09_ATTACK_ANIM_MODE: u32 = 0x0900_0025;
pub const ROBOTS_EW09_ATTACK_INNER_RADIUS: f32 = 2.0;
pub const ROBOTS_EW09_ATTACK_OUTER_RADIUS: f32 = 4.0;
pub const ROBOTS_EW09_ATTACK_YAW_TOLERANCE_RADIANS: f32 = f32::from_bits(0x3F06_0A92);
pub const ROBOTS_EW09_ATTACK_REENTRY_TICKS: u32 = 60;

/// `0x00463190` uses these exact constants around `0x00454CA0`.
const ROBOTS_EW09_ATTACHMENT_SPEED_LERP: f32 = f32::from_bits(0x3ECC_CCCD); // 0.4
const ROBOTS_EW09_ATTACHMENT_ROTATION_SCALE: f32 = f32::from_bits(0x3DA6_48A1);
const ROBOTS_TWO_PI: f32 = f32::from_bits(0x40C9_0FDB);

pub const fn ew09_follow_network_path_config() -> RobotsFollowNetworkPathConfig {
    RobotsFollowNetworkPathConfig {
        priority: ROBOTS_EW09_FOLLOW_PATH_PRIORITY,
        target_locomotion_scalar: ROBOTS_EW09_FOLLOW_PATH_SCALAR,
    }
}

/// EW09 builder `0x00463010 -> 0x0044F8A0/0x0044FA80`.
/// The node is the shared generic Attack family with one derived execute side effect:
/// `0x0044FAF0` calls owner vslot +0x110 with scalar 0.4 every active tick.
pub const fn ew09_attack_config() -> RobotsGenericAttackConfig {
    RobotsGenericAttackConfig {
        primary_anim_mode: ROBOTS_EW09_ATTACK_ANIM_MODE,
        secondary_anim_mode: None,
        inner_radius: ROBOTS_EW09_ATTACK_INNER_RADIUS,
        outer_radius: ROBOTS_EW09_ATTACK_OUTER_RADIUS,
        yaw_tolerance_radians: ROBOTS_EW09_ATTACK_YAW_TOLERANCE_RADIANS,
        vertical_limit: 1000.0,
        reentry_delay_ticks: ROBOTS_EW09_ATTACK_REENTRY_TICKS,
        secondary_repeat_count: 0,
        sticky_while_active: true,
        priority: 0x32,
    }
}

/// Effective Handler+0x628 after EW09 init `0x00462F90` ORs DAT_005E21A8.
pub const fn ew09_effective_handler_flags(serialized_flags: u32) -> u32 {
    serialized_flags | ROBOTS_EW09_HANDLER_FLAGS_OR
}

/// Derived Attack execute `0x0044FAF0 -> owner +0x110(0.4)` using EW09's
/// class speed range seeded by `0x00462F90`.
pub fn ew09_attack_character_physics_velocity(owner_yaw_radians: f32) -> [f32; 3] {
    ai_character_physics_velocity_from_scalar(
        owner_yaw_radians,
        ROBOTS_EW09_SPEED_MIN,
        ROBOTS_EW09_SPEED_MAX,
        ROBOTS_EW09_ATTACK_MOVEMENT_SCALAR,
        1.0,
    )
}

/// Exact per-fixed-tick local-Y attachment angle from `0x00463190`.
/// Native computes `lerp(2,8,0.4) * (1/60) * 0x3DA648A1 * 2pi`.
pub fn ew09_attachment_rotation_delta_radians() -> f32 {
    let speed = (ROBOTS_EW09_SPEED_MAX - ROBOTS_EW09_SPEED_MIN) * ROBOTS_EW09_ATTACHMENT_SPEED_LERP
        + ROBOTS_EW09_SPEED_MIN;
    ROBOTS_FIXED_STEP_SECONDS * speed * ROBOTS_EW09_ATTACHMENT_ROTATION_SCALE * ROBOTS_TWO_PI
}

pub fn step_ew09_attachment_rotation(current_quaternion_xyzw: [f32; 4]) -> [f32; 4] {
    postmultiply_local_y_rotation(
        current_quaternion_xyzw,
        ew09_attachment_rotation_delta_radians(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_attack_and_path_constants_match_00463010_and_00463230() {
        let attack = ew09_attack_config();
        assert_eq!(attack.primary_anim_mode, 0x0900_0025);
        assert_eq!(attack.inner_radius, 2.0);
        assert_eq!(attack.outer_radius, 4.0);
        assert_eq!(attack.yaw_tolerance_radians.to_bits(), 0x3F06_0A92);
        assert_eq!(attack.reentry_delay_ticks, 60);
        assert_eq!(ew09_follow_network_path_config().priority, 0x10);
        assert_eq!(
            ew09_follow_network_path_config().target_locomotion_scalar,
            0.4
        );
    }

    #[test]
    fn init_and_derived_attack_motion_match_class_contract() {
        assert_eq!(ew09_effective_handler_flags(1), 0x0008_0001);
        let velocity = ew09_attack_character_physics_velocity(0.0);
        assert_eq!(velocity[0], 0.0);
        assert_eq!(velocity[1], 0.0);
        assert_eq!(velocity[2], 4.4);
    }

    #[test]
    fn attachment_rotation_uses_exact_native_constants() {
        let delta = ew09_attachment_rotation_delta_radians();
        let expected = ROBOTS_FIXED_STEP_SECONDS
            * 4.4
            * f32::from_bits(0x3DA6_48A1)
            * f32::from_bits(0x40C9_0FDB);
        assert_eq!(delta.to_bits(), expected.to_bits());
        let q = step_ew09_attachment_rotation([0.0, 0.0, 0.0, 1.0]);
        assert!(q[1] > 0.0);
        assert!(q[3] < 1.0);
    }
}
