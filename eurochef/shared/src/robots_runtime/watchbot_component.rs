use serde::Serialize;

use super::attachment_rotation::postmultiply_local_y_rotation;

pub const ROBOTS_WATCHBOT_COMPONENT_EPSILON: f32 = f32::from_bits(0x3A83_126F); // 0.001
pub const ROBOTS_WATCHBOT_COMPONENT_NEGATIVE_EPSILON: f32 = f32::from_bits(0xBA83_126F); // -0.001
pub const ROBOTS_WATCHBOT_COMPONENT_MAX_HORIZONTAL_SPEED: f32 = f32::from_bits(0x3FC0_0000); // 1.5
pub const ROBOTS_WATCHBOT_COMPONENT_STEERING_ROLL_SCALE: f32 = f32::from_bits(0x3E4C_CCCD); // 0.2
pub const ROBOTS_WATCHBOT_COMPONENT_YAW_INPUT_SCALE: f32 = f32::from_bits(0x3DCC_CCCD); // 0.1
pub const ROBOTS_WATCHBOT_COMPONENT_ACCELERATION_SCALE: f32 = f32::from_bits(0x3E80_0000); // 0.25
pub const ROBOTS_WATCHBOT_COMPONENT_MOVING_DAMPING: f32 = f32::from_bits(0x3F73_3333); // 0.95
pub const ROBOTS_WATCHBOT_COMPONENT_PI: f32 = f32::from_bits(0x4049_0FDB);
pub const ROBOTS_WATCHBOT_COMPONENT_FIXED_STEP_SECONDS: f32 = f32::from_bits(0x3C88_8889); // 1/60
pub const ROBOTS_WATCHBOT_COMPONENT_HALF: f32 = f32::from_bits(0x3F00_0000); // 0.5
pub const ROBOTS_WATCHBOT_COMPONENT_ATTACHMENT_AUDIO_CENTER: f32 = 6.0;
pub const ROBOTS_WATCHBOT_COMPONENT_ATTACHMENT_AUDIO_SCALE: f32 = 750.0;
pub const ROBOTS_WATCHBOT_COMPONENT_RECOVERY_INPUT_MAGNITUDE: f32 = 0.5;
pub const ROBOTS_WATCHBOT_COMPONENT_RECOVERY_LATCH_UPDATES: u8 = 0x78;
pub const ROBOTS_WATCHBOT_COMPONENT_IDLE_TIMER_MODULUS: u32 = 0xF0;
pub const ROBOTS_WATCHBOT_COMPONENT_IDLE_TIMER_BASE: u16 = 0x3C;
pub const ROBOTS_WATCHBOT_COMPONENT_STEERING_DISTANCE_SQUARED_THRESHOLD: f32 =
    f32::from_bits(0x3DCC_CCCD); // 0.1
pub const ROBOTS_WATCHBOT_COMPONENT_STEERING_CLOSE_TARGET_XZ_SQUARED: f32 = 2.0;
pub const ROBOTS_WATCHBOT_COMPONENT_STEERING_YAW_ALPHA: f32 = f32::from_bits(0x3D00_0000); // 1/32
pub const ROBOTS_WATCHBOT_COMPONENT_STEERING_AVOIDANCE_ALPHA: f32 = f32::from_bits(0x3DCC_CCCD); // 0.1
pub const ROBOTS_WATCHBOT_COMPONENT_STEERING_AVOIDANCE_MAX: f32 = f32::from_bits(0x3E99_999A); // 0.3
pub const ROBOTS_WATCHBOT_COMPONENT_STEERING_NORMALIZE_EPSILON: f32 = f32::from_bits(0x33D6_BF95);
pub const ROBOTS_WATCHBOT_COMPONENT_STOP_BLOCKED_ANIM_MODES: [u32; 3] =
    [0x0900_0071, 0x0900_0072, 0x0900_00A8];

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsWatchbotComponentStopInput {
    /// WatchBot owner XItem position from +0xD0/+0xD4/+0xD8.
    pub owner_position_xyz: [f32; 3],
    /// XItemHandler_WatchBot target lane +0x4DC/+0x4E4 projected to X/Z.
    pub handler_target_xz: [f32; 2],
    /// Current AnimMode from the owner handler +0x478.
    pub current_anim_mode_uid: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsWatchbotComponentRecoveryContact {
    /// Host result for native collision/material probe in shared vslot +0x88
    /// (`0x00495A10`).
    pub special_contact: bool,
    /// Recovery heading stored at component +0x64 when the latch is armed.
    pub recovery_yaw_radians: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsWatchbotComponentAttachmentSpinStep {
    /// Native child XItem quaternion lane +0xE0/+0xE4/+0xE8/+0xEC after
    /// `current * delta_yaw`. Native does not renormalize here.
    pub child_quaternion_xyzw: [f32; 4],
    /// Child XItem +0x7E is cleared after the quaternion write.
    pub mark_child_transform_dirty: bool,
    /// Integer payload fanned out through Handler +0x2C8 -> `0x005078D2` as
    /// SFX command opcode 0x1D. Receiver-side semantic name remains unresolved.
    pub audio_command_1d_value: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsWatchbotComponentSteeringInput {
    pub owner_position_xyzw: [f32; 4],
    pub owner_rotation_xyzw: [f32; 4],
    /// Wrapper `0x00494390` passes the same point as movement and yaw target.
    pub desired_target_xyzw: [f32; 4],
    /// Handler-bound component +0x28 XItem transform used by the close-target branch.
    pub bound_target_position_xyzw: [f32; 4],
    /// Component +0x04. States 2/6/9 suppress the near-target Player-yaw override.
    pub internal_state: u32,
    pub player_yaw_radians: Option<f32>,
    /// DAT_007B2BE0..BEC when the native global override condition is active.
    /// Native adds +1.0 only to Y for movement; yaw reference remains desired_target.
    pub global_override_position_xyzw: Option<[f32; 4]>,
    pub damping: f32,
    pub strength: f32,
    /// Native param6. Mode1 passes false, so close-target avoidance is enabled.
    pub suppress_close_target_avoidance: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsWatchbotComponentSteeringStep {
    pub effective_target_xyzw: [f32; 4],
    pub distance_squared_to_target: f32,
    /// Exact payload written through `0x004568C0`; host owns the real transform.
    pub owner_rotation_xyzw: [f32; 4],
    pub mark_owner_transform_dirty: bool,
    /// Component +0x30..+0x3C after acceleration and damping.
    pub velocity_xyzw_30: [f32; 4],
    /// Native copies the damped velocity verbatim to owner body +0x150/+0x2C..38.
    pub body_accumulator_xyzw: [f32; 4],
    pub close_target_avoidance_applied: bool,
}

/// Common WatchBot component setup timer from `0x004935A0`.
///
/// Every concrete component that enters through this base setup consumes one
/// process-global `FUN_00509C48` draw and stores `(draw % 240) + 60` at +0x8E.
pub fn watchbot_component_idle_timer_from_draw(draw: u32) -> u16 {
    (draw % ROBOTS_WATCHBOT_COMPONENT_IDLE_TIMER_MODULUS) as u16
        + ROBOTS_WATCHBOT_COMPONENT_IDLE_TIMER_BASE
}

/// Wrapper `0x00494390` + common WatchBot steering helper `0x004943B0`.
/// World visibility remains outside this helper; the caller supplies the selected target.
pub fn watchbot_component_steering_step(
    current_velocity_xyzw_30: [f32; 4],
    input: RobotsWatchbotComponentSteeringInput,
) -> RobotsWatchbotComponentSteeringStep {
    let mut effective_target = input.desired_target_xyzw;
    if let Some(global) = input.global_override_position_xyzw {
        effective_target[0] = global[0];
        effective_target[1] = global[1] + 1.0;
        effective_target[2] = global[2];
    }

    let dx = effective_target[0] - input.owner_position_xyzw[0];
    let dy = effective_target[1] - input.owner_position_xyzw[1];
    let dz = effective_target[2] - input.owner_position_xyzw[2];
    let horizontal_squared = dx * dx + dz * dz;
    let distance_squared = horizontal_squared + dy * dy;
    let movement_yaw = native_atan2(dx, dz);
    let movement_pitch = native_atan2(dy, dz);

    let reference_dx = input.desired_target_xyzw[0] - input.owner_position_xyzw[0];
    let reference_dz = input.desired_target_xyzw[2] - input.owner_position_xyzw[2];
    let mut reference_yaw = native_atan2(reference_dx, reference_dz);
    if horizontal_squared < 1.0 && !matches!(input.internal_state, 2 | 6 | 9) {
        if let Some(player_yaw) = input.player_yaw_radians {
            reference_yaw = player_yaw;
        }
    }
    let yaw_delta =
        native_watchbot_wrap_angle(reference_yaw as f64 - input.owner_rotation_xyzw[1] as f64);
    let owner_yaw = (input.owner_rotation_xyzw[1] as f64
        + yaw_delta * ROBOTS_WATCHBOT_COMPONENT_STEERING_YAW_ALPHA as f64)
        as f32;
    let owner_rotation_xyzw = [0.0, owner_yaw, 0.0, input.owner_rotation_xyzw[3]];

    let mut acceleration = [0.0f32; 4];
    let mut close_target_avoidance_applied = false;
    if ROBOTS_WATCHBOT_COMPONENT_STEERING_DISTANCE_SQUARED_THRESHOLD < distance_squared {
        if ROBOTS_WATCHBOT_COMPONENT_STEERING_DISTANCE_SQUARED_THRESHOLD <= horizontal_squared {
            acceleration[0] = native_sin(movement_yaw) * input.strength;
            acceleration[2] = native_cos(movement_yaw) * input.strength;

            let target_dx = input.bound_target_position_xyzw[0] - input.owner_position_xyzw[0];
            let target_dz = input.bound_target_position_xyzw[2] - input.owner_position_xyzw[2];
            if !input.suppress_close_target_avoidance
                && target_dx * target_dx + target_dz * target_dz
                    < ROBOTS_WATCHBOT_COMPONENT_STEERING_CLOSE_TARGET_XZ_SQUARED
            {
                if let Some(adjusted) = watchbot_close_target_avoidance(
                    input.owner_position_xyzw,
                    input.bound_target_position_xyzw,
                    effective_target,
                ) {
                    acceleration[0] = adjusted[0];
                    acceleration[2] = adjusted[2];
                    close_target_avoidance_applied = true;
                }
            }
        }
        acceleration[1] = native_sin(movement_pitch) * input.strength;
    }

    let mut velocity = current_velocity_xyzw_30;
    for index in 0..4 {
        velocity[index] = (velocity[index] + acceleration[index]) * input.damping;
    }

    RobotsWatchbotComponentSteeringStep {
        effective_target_xyzw: effective_target,
        distance_squared_to_target: distance_squared,
        owner_rotation_xyzw,
        mark_owner_transform_dirty: true,
        velocity_xyzw_30: velocity,
        body_accumulator_xyzw: velocity,
        close_target_avoidance_applied,
    }
}

fn watchbot_close_target_avoidance(
    owner: [f32; 4],
    bound_target: [f32; 4],
    desired: [f32; 4],
) -> Option<[f32; 4]> {
    let owner_from_target = [
        owner[0] - bound_target[0],
        owner[1] - bound_target[1],
        owner[2] - bound_target[2],
    ];
    let desired_from_target = [
        desired[0] - bound_target[0],
        desired[1] - bound_target[1],
        desired[2] - bound_target[2],
    ];
    let desired_from_owner = [
        desired[0] - owner[0],
        desired[1] - owner[1],
        desired[2] - owner[2],
    ];

    let owner_distance = native_length3(owner_from_target);
    let desired_target_distance = native_length3(desired_from_target);
    let desired_owner_distance = native_length3(desired_from_owner);
    if desired_owner_distance <= desired_target_distance {
        return None;
    }

    let owner_yaw = native_atan2(owner_from_target[0], owner_from_target[2]);
    let owner_horizontal = native_hypot(owner_from_target[0], owner_from_target[2]);
    let owner_angle = native_atan2(owner_horizontal, owner_from_target[1]);
    let owner_vertical_projection = native_cos(owner_angle) * owner_distance;

    let desired_yaw = native_atan2(desired_from_target[0], desired_from_target[2]);
    let desired_horizontal = native_hypot(desired_from_target[0], desired_from_target[2]);
    let desired_angle = native_atan2(desired_horizontal, desired_from_target[1]);
    let desired_vertical_projection = native_cos(desired_angle) * desired_target_distance;

    let vertical_projection = owner_vertical_projection
        + (desired_vertical_projection - owner_vertical_projection)
            * ROBOTS_WATCHBOT_COMPONENT_STEERING_AVOIDANCE_ALPHA;
    let yaw = (owner_yaw as f64
        + native_watchbot_wrap_angle(desired_yaw as f64 - owner_yaw as f64)
            * ROBOTS_WATCHBOT_COMPONENT_STEERING_AVOIDANCE_ALPHA as f64) as f32;
    let angle = (owner_angle as f64
        + native_watchbot_wrap_angle(desired_angle as f64 - owner_angle as f64)
            * ROBOTS_WATCHBOT_COMPONENT_STEERING_AVOIDANCE_ALPHA as f64) as f32;

    let intermediate = [
        bound_target[0] + native_sin(yaw) * vertical_projection,
        bound_target[1] + native_tan(angle) * vertical_projection,
        bound_target[2] + native_cos(yaw) * vertical_projection,
    ];
    let toward_intermediate = [
        intermediate[0] - owner[0],
        intermediate[1] - owner[1],
        intermediate[2] - owner[2],
    ];
    let distance = native_length3(toward_intermediate);
    let horizontal = native_hypot(toward_intermediate[0], toward_intermediate[2]);
    let scale = distance.min(ROBOTS_WATCHBOT_COMPONENT_STEERING_AVOIDANCE_MAX);
    let (x, z) = if ROBOTS_WATCHBOT_COMPONENT_STEERING_NORMALIZE_EPSILON < horizontal {
        (
            toward_intermediate[0] / horizontal,
            toward_intermediate[2] / horizontal,
        )
    } else {
        // Native skips normalization below the epsilon and retains the raw X/Z lanes.
        (toward_intermediate[0], toward_intermediate[2])
    };
    Some([x * scale, 0.0, z * scale, 0.0])
}

fn native_watchbot_wrap_angle(mut angle: f64) -> f64 {
    let pi = ROBOTS_WATCHBOT_COMPONENT_PI as f64;
    let tau = f32::from_bits(0x40C9_0FDB) as f64;
    while angle < pi {
        angle += tau;
    }
    while pi < angle {
        angle -= tau;
    }
    angle
}

fn native_atan2(y: f32, x: f32) -> f32 {
    (y as f64).atan2(x as f64) as f32
}

fn native_sin(value: f32) -> f32 {
    (value as f64).sin() as f32
}

fn native_cos(value: f32) -> f32 {
    (value as f64).cos() as f32
}

fn native_tan(value: f32) -> f32 {
    (value as f64).tan() as f32
}

fn native_hypot(x: f32, z: f32) -> f32 {
    ((x as f64) * (x as f64) + (z as f64) * (z as f64)).sqrt() as f32
}

fn native_length3(value: [f32; 3]) -> f32 {
    ((value[0] as f64) * (value[0] as f64)
        + (value[1] as f64) * (value[1] as f64)
        + (value[2] as f64) * (value[2] as f64))
        .sqrt() as f32
}

/// Shared native stop predicate `0x00494EC0` used by WatchBot component modes2/3.
pub fn watchbot_component_stop_predicate(
    velocity_xyzw_30: [f32; 4],
    input: RobotsWatchbotComponentStopInput,
) -> bool {
    let velocity_squared = velocity_xyzw_30[0] * velocity_xyzw_30[0]
        + velocity_xyzw_30[1] * velocity_xyzw_30[1]
        + velocity_xyzw_30[2] * velocity_xyzw_30[2];
    if velocity_squared >= ROBOTS_WATCHBOT_COMPONENT_EPSILON {
        return false;
    }

    let dx = input.handler_target_xz[0] - input.owner_position_xyz[0];
    let dz = input.handler_target_xz[1] - input.owner_position_xyz[2];
    if dx * dx + dz * dz >= ROBOTS_WATCHBOT_COMPONENT_EPSILON {
        return false;
    }

    !ROBOTS_WATCHBOT_COMPONENT_STOP_BLOCKED_ANIM_MODES.contains(&input.current_anim_mode_uid)
}

/// Shared latch/counter mutation from WatchBot component vslot +0x88
/// (`0x00495A10`). Collision projection itself stays host-owned.
pub fn service_watchbot_component_recovery(
    recovery_latched_94: &mut bool,
    recovery_updates_95: &mut u8,
    control_magnitude_60: &mut f32,
    control_yaw_64: &mut f32,
    contact: Option<RobotsWatchbotComponentRecoveryContact>,
) {
    if !*recovery_latched_94 {
        if let Some(contact) = contact.filter(|contact| contact.special_contact) {
            *recovery_latched_94 = true;
            *recovery_updates_95 = 0;
            *control_magnitude_60 = ROBOTS_WATCHBOT_COMPONENT_RECOVERY_INPUT_MAGNITUDE;
            *control_yaw_64 = contact.recovery_yaw_radians;
        }
        return;
    }

    *recovery_updates_95 = recovery_updates_95.wrapping_add(1);
    if *recovery_updates_95 > ROBOTS_WATCHBOT_COMPONENT_RECOVERY_LATCH_UPDATES {
        *recovery_updates_95 = 0;
        *recovery_latched_94 = false;
    }
}

/// Shared WatchBot component vslot +0x6C (`0x00495580`) used by mode2 and mode3.
///
/// `spin_scalar_78` becomes a local-Y delta of `spin * pi / 60`, then is
/// post-multiplied onto the attached child XItem quaternion. The same update sends
/// `trunc((spin - 6) * 750)` through the Handler +0x2C8 SFX command list.
pub fn watchbot_component_attachment_spin_step(
    spin_scalar_78: f32,
    current_child_quaternion_xyzw: [f32; 4],
) -> RobotsWatchbotComponentAttachmentSpinStep {
    let angle = spin_scalar_78
        * ROBOTS_WATCHBOT_COMPONENT_PI
        * ROBOTS_WATCHBOT_COMPONENT_FIXED_STEP_SECONDS;
    let child_quaternion_xyzw = postmultiply_local_y_rotation(current_child_quaternion_xyzw, angle);
    let audio_command_1d_value = ((spin_scalar_78
        - ROBOTS_WATCHBOT_COMPONENT_ATTACHMENT_AUDIO_CENTER)
        * ROBOTS_WATCHBOT_COMPONENT_ATTACHMENT_AUDIO_SCALE) as i32;

    RobotsWatchbotComponentAttachmentSpinStep {
        child_quaternion_xyzw,
        mark_child_transform_dirty: true,
        audio_command_1d_value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steering_input() -> RobotsWatchbotComponentSteeringInput {
        RobotsWatchbotComponentSteeringInput {
            owner_position_xyzw: [0.0, 0.0, 0.0, 0.0],
            owner_rotation_xyzw: [0.0, 1.0, 0.0, 7.0],
            desired_target_xyzw: [0.0, 0.0, 10.0, 0.0],
            bound_target_position_xyzw: [100.0, 0.0, 100.0, 0.0],
            internal_state: 3,
            player_yaw_radians: None,
            global_override_position_xyzw: None,
            damping: 0.95,
            strength: 0.3,
            suppress_close_target_avoidance: false,
        }
    }

    #[test]
    fn shared_steering_direct_path_matches_native_accumulate_damp_and_rotation_write() {
        let step = watchbot_component_steering_step([0.0; 4], steering_input());
        assert!((step.distance_squared_to_target - 100.0).abs() < 1.0e-6);
        assert!((step.velocity_xyzw_30[0]).abs() < 1.0e-6);
        assert!((step.velocity_xyzw_30[1]).abs() < 1.0e-6);
        assert!((step.velocity_xyzw_30[2] - 0.285).abs() < 1.0e-6);
        assert_eq!(step.velocity_xyzw_30, step.body_accumulator_xyzw);
        assert!((step.owner_rotation_xyzw[1] - 0.96875).abs() < 1.0e-6);
        assert_eq!(step.owner_rotation_xyzw[0], 0.0);
        assert_eq!(step.owner_rotation_xyzw[2], 0.0);
        assert_eq!(step.owner_rotation_xyzw[3], 7.0);
        assert!(step.mark_owner_transform_dirty);
        assert!(!step.close_target_avoidance_applied);
    }

    #[test]
    fn shared_steering_keeps_strict_point_one_distance_gate() {
        let mut input = steering_input();
        input.desired_target_xyzw = [
            0.0,
            0.0,
            ROBOTS_WATCHBOT_COMPONENT_STEERING_DISTANCE_SQUARED_THRESHOLD.sqrt(),
            0.0,
        ];
        let initial = [1.0, 2.0, 3.0, 4.0];
        let step = watchbot_component_steering_step(initial, input);
        for index in 0..4 {
            assert!((step.velocity_xyzw_30[index] - initial[index] * input.damping).abs() < 1.0e-6);
        }
    }

    #[test]
    fn shared_steering_global_override_moves_target_but_not_yaw_reference() {
        let mut input = steering_input();
        input.owner_rotation_xyzw[1] = 0.0;
        input.global_override_position_xyzw = Some([10.0, 2.0, 0.0, 9.0]);
        let step = watchbot_component_steering_step([0.0; 4], input);
        assert_eq!(step.effective_target_xyzw, [10.0, 3.0, 0.0, 0.0]);
        assert!(step.owner_rotation_xyzw[1].abs() < 1.0e-6);
        assert!(step.velocity_xyzw_30[0] > 0.28);
        assert!(step.velocity_xyzw_30[1] > 0.28);
        assert!(step.velocity_xyzw_30[2].abs() < 1.0e-5);
    }

    #[test]
    fn shared_steering_near_target_uses_player_yaw_except_states_two_six_nine() {
        let mut input = steering_input();
        input.owner_rotation_xyzw[1] = 0.0;
        input.desired_target_xyzw = [0.0, 0.0, 0.5, 0.0];
        input.player_yaw_radians = Some(2.0);
        let step = watchbot_component_steering_step([0.0; 4], input);
        assert!((step.owner_rotation_xyzw[1] - 0.0625).abs() < 1.0e-6);

        input.internal_state = 2;
        let blocked = watchbot_component_steering_step([0.0; 4], input);
        assert!(blocked.owner_rotation_xyzw[1].abs() < 1.0e-6);
    }

    #[test]
    fn shared_steering_close_target_branch_caps_horizontal_acceleration() {
        let mut input = steering_input();
        input.owner_rotation_xyzw[1] = 0.0;
        input.bound_target_position_xyzw = [0.5, 0.0, 1.0, 0.0];
        input.desired_target_xyzw = [0.0, 0.0, 3.0, 0.0];
        input.damping = 1.0;
        let step = watchbot_component_steering_step([0.0; 4], input);
        assert!(step.close_target_avoidance_applied);
        let horizontal = (step.velocity_xyzw_30[0] * step.velocity_xyzw_30[0]
            + step.velocity_xyzw_30[2] * step.velocity_xyzw_30[2])
            .sqrt();
        assert!(horizontal <= ROBOTS_WATCHBOT_COMPONENT_STEERING_AVOIDANCE_MAX + 1.0e-6);
    }

    #[test]
    fn shared_watchbot_angle_wrap_uses_native_positive_pi_boundary() {
        let pi = ROBOTS_WATCHBOT_COMPONENT_PI as f64;
        assert!((native_watchbot_wrap_angle(-pi) - pi).abs() < 1.0e-7);
        assert!(native_watchbot_wrap_angle(0.0).abs() < 1.0e-7);
    }

    #[test]
    fn shared_stop_predicate_preserves_native_strict_threshold_and_anim_blocks() {
        let base = RobotsWatchbotComponentStopInput {
            owner_position_xyz: [1.0, 0.0, 2.0],
            handler_target_xz: [1.0, 2.0],
            current_anim_mode_uid: 0x0900_0003,
        };
        assert!(watchbot_component_stop_predicate([0.0; 4], base));
        assert!(!watchbot_component_stop_predicate(
            [ROBOTS_WATCHBOT_COMPONENT_EPSILON.sqrt(), 0.0, 0.0, 0.0],
            base,
        ));
        for current_anim_mode_uid in ROBOTS_WATCHBOT_COMPONENT_STOP_BLOCKED_ANIM_MODES {
            assert!(!watchbot_component_stop_predicate(
                [0.0; 4],
                RobotsWatchbotComponentStopInput {
                    current_anim_mode_uid,
                    ..base
                },
            ));
        }
    }

    #[test]
    fn shared_recovery_latch_matches_native_contact_and_timeout_rules() {
        let mut latched = false;
        let mut updates = 0;
        let mut magnitude = 0.0;
        let mut yaw = 0.0;
        service_watchbot_component_recovery(
            &mut latched,
            &mut updates,
            &mut magnitude,
            &mut yaw,
            Some(RobotsWatchbotComponentRecoveryContact {
                special_contact: true,
                recovery_yaw_radians: 0.75,
            }),
        );
        assert!(latched);
        assert_eq!(updates, 0);
        assert_eq!(
            magnitude,
            ROBOTS_WATCHBOT_COMPONENT_RECOVERY_INPUT_MAGNITUDE
        );
        assert_eq!(yaw, 0.75);

        for _ in 0..=ROBOTS_WATCHBOT_COMPONENT_RECOVERY_LATCH_UPDATES {
            service_watchbot_component_recovery(
                &mut latched,
                &mut updates,
                &mut magnitude,
                &mut yaw,
                None,
            );
        }
        assert!(!latched);
        assert_eq!(updates, 0);
    }

    #[test]
    fn shared_attachment_spin_postmultiplies_local_y_delta_without_normalization() {
        let step = watchbot_component_attachment_spin_step(6.0, [0.0, 0.0, 0.0, 1.0]);
        let half_angle = 6.0
            * ROBOTS_WATCHBOT_COMPONENT_PI
            * ROBOTS_WATCHBOT_COMPONENT_FIXED_STEP_SECONDS
            * ROBOTS_WATCHBOT_COMPONENT_HALF;
        assert!(step.child_quaternion_xyzw[0].abs() < 1.0e-7);
        assert!((step.child_quaternion_xyzw[1] - (half_angle as f64).sin() as f32).abs() < 1.0e-7);
        assert!(step.child_quaternion_xyzw[2].abs() < 1.0e-7);
        assert!((step.child_quaternion_xyzw[3] - (half_angle as f64).cos() as f32).abs() < 1.0e-7);
        assert!(step.mark_child_transform_dirty);
        assert_eq!(step.audio_command_1d_value, 0);
    }

    #[test]
    fn shared_setup_timer_keeps_native_modulus_and_base() {
        assert_eq!(watchbot_component_idle_timer_from_draw(0), 60);
        assert_eq!(watchbot_component_idle_timer_from_draw(239), 299);
        assert_eq!(watchbot_component_idle_timer_from_draw(240), 60);
    }

    #[test]
    fn shared_attachment_spin_keeps_native_audio_command_center_and_scale() {
        assert_eq!(
            watchbot_component_attachment_spin_step(2.0, [0.0, 0.0, 0.0, 1.0])
                .audio_command_1d_value,
            -3000
        );
        assert_eq!(
            watchbot_component_attachment_spin_step(10.0, [0.0, 0.0, 0.0, 1.0])
                .audio_command_1d_value,
            3000
        );
    }
}
