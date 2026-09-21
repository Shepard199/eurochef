use std::f32::consts::{PI, TAU};

use serde::Serialize;

/// Robots runs gameplay Physics at a fixed 60 Hz step.
pub const ROBOTS_FIXED_STEP_SECONDS: f32 = 1.0 / 60.0;

pub const ROBOTS_ANIM_MODE_MOVE: u32 = 0x0900_0003;
pub const ROBOTS_ANIM_MODE_TURN_ON_SPOT: u32 = 0x0900_0028;
pub const ROBOTS_ANIM_MODE_TURN_ON_SPOT_L: u32 = 0x0900_0035;
pub const ROBOTS_ANIM_MODE_TURN_ON_SPOT_R: u32 = 0x0900_0036;

/// Base AI init `0x004514F0` seeds Handler+0x5D4 with exact bits `0x40FB53D2`.
pub const ROBOTS_AI_DEFAULT_TURN_RATE_RADIANS_PER_SECOND: f32 = f32::from_bits(0x40fb_53d2);
pub const ROBOTS_AI_LOCOMOTION_SCALAR_RATE_PER_SECOND: f32 = 1.0;
pub const ROBOTS_AI_TURN_IN_PLACE_ENABLE_FLAG: u32 = 0x2;
pub const ROBOTS_AI_DIRECTIONAL_TURN_MODE_FLAG: u32 = 0x4;
pub const ROBOTS_AI_MOVEMENT_ERROR_MIN_EXPECTED_DISTANCE: f32 = f32::from_bits(0x3a83_126f);
pub const ROBOTS_AI_TURN_IN_PLACE_ENTER_DEGREES: f32 = 30.0;
pub const ROBOTS_AI_TURN_IN_PLACE_EXIT_DEGREES: f32 = 5.0;
pub const ROBOTS_RADIANS_TO_DEGREES: f32 = 57.295_776;

/// Shipped EB10 RollerBot setup (`0x00462C00`) stores 10.0 at Handler+0x664.
/// Native `0x004681F0` integrates this value into retained forward velocity,
/// so this is acceleration-like input, not an already-integrated world speed.
pub const ROBOTS_ROLLERBOT_FORWARD_ACCELERATION: f32 = 10.0;

/// Exact native damping bits used by `0x00467F80` for retained motion lanes.
pub const ROBOTS_ROLLERBOT_VELOCITY_DAMPING: f32 = f32::from_bits(0x3f7c_28f6);

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum RobotsAiTurnRateInput {
    /// Native call argument `-1.0`: use Handler+0x5D4.
    Default,
    Explicit(f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsAiLocomotionRuntimeState {
    /// Handler+0x5D0.
    pub locomotion_scalar: f32,
    /// Handler+0x5F9, owned by vslot +0x114 / `0x00452F40`.
    pub turn_in_place_latch: bool,
}

impl Default for RobotsAiLocomotionRuntimeState {
    fn default() -> Self {
        Self {
            locomotion_scalar: 0.0,
            turn_in_place_latch: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsAiMovementErrorRuntimeState {
    /// Handler+0x510/+0x514/+0x518: expected displacement from the previous
    /// Character Physics velocity sample.
    pub expected_displacement_xyz: [f32; 3],
    /// Handler+0x520/+0x524/+0x528: owner position captured on the previous tick.
    pub previous_owner_position_xyz: [f32; 3],
    /// Handler+0x5E4.
    pub movement_error_ratio: f32,
}

impl Default for RobotsAiMovementErrorRuntimeState {
    fn default() -> Self {
        Self {
            expected_displacement_xyz: [0.0; 3],
            previous_owner_position_xyz: [0.0; 3],
            movement_error_ratio: 0.0,
        }
    }
}

/// Exact X/Z movement-error metric from base AI update `0x00451DF0`.
///
/// Native compares last tick's expected Character Physics displacement against
/// the actual owner displacement, stores `error_len / expected_len` in
/// Handler+0x5E4 when expected motion exceeds ~0.001, then seeds the next
/// expected displacement from the current physics velocity times the scaled
/// fixed-step duration. Y is retained in the native state but excluded from the
/// ratio itself.
pub fn step_ai_movement_error(
    state: &mut RobotsAiMovementErrorRuntimeState,
    current_owner_position_xyz: [f32; 3],
    character_physics_velocity_xyz: [f32; 3],
    runtime_rate_scale: f32,
) -> f32 {
    let actual_dx = current_owner_position_xyz[0] - state.previous_owner_position_xyz[0];
    let actual_dz = current_owner_position_xyz[2] - state.previous_owner_position_xyz[2];
    let error_dx = state.expected_displacement_xyz[0] - actual_dx;
    let error_dz = state.expected_displacement_xyz[2] - actual_dz;
    let error_len = (error_dx * error_dx + error_dz * error_dz).sqrt();
    let expected_len = (state.expected_displacement_xyz[0] * state.expected_displacement_xyz[0]
        + state.expected_displacement_xyz[2] * state.expected_displacement_xyz[2])
        .sqrt();

    state.movement_error_ratio = if expected_len > ROBOTS_AI_MOVEMENT_ERROR_MIN_EXPECTED_DISTANCE {
        error_len / expected_len
    } else {
        0.0
    };

    let dt = runtime_rate_scale.max(0.0) * ROBOTS_FIXED_STEP_SECONDS;
    state.expected_displacement_xyz = [
        character_physics_velocity_xyz[0] * dt,
        character_physics_velocity_xyz[1] * dt,
        character_physics_velocity_xyz[2] * dt,
    ];
    state.previous_owner_position_xyz = current_owner_position_xyz;
    state.movement_error_ratio
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsAiLocomotionInput {
    /// Handler+0x5D8 after native steering pre-pass `0x00452B40`.
    pub steering_target_yaw_radians: f32,
    /// First stack argument to `0x00452800`.
    pub target_locomotion_scalar: f32,
    /// Second stack argument to `0x00452800`; native `-1.0` means default rate.
    pub turn_rate: RobotsAiTurnRateInput,
    /// Handler+0x628 feature bits consumed by vslots +0x114/+0x118.
    pub handler_flags_628: u32,
    /// `0x00452800` resets +0x5D0 to zero when current AnimMode is not Move.
    pub move_mode_active_on_entry: bool,
    pub current_owner_yaw_radians: f32,
    /// Native DAT_00620034. Standard 60-rate runtime is exactly 1.0.
    pub runtime_rate_scale: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsAiLocomotionStep {
    pub owner_yaw_radians: f32,
    pub locomotion_scalar: f32,
    pub requested_anim_mode: u32,
    pub turn_in_place_active: bool,
    /// Directional TurnOnSpotL/R does not directly write owner yaw in
    /// `0x00452FB0`; that rotation belongs to animation/root-motion handling.
    pub direct_owner_yaw_write: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsAiDirectTurnStep {
    pub owner_yaw_radians: f32,
    pub requested_anim_mode: u32,
    pub direct_owner_yaw_write: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsAiSteeringAvoidanceEntry {
    /// Runtime entry +0x4C bit0. Disabled entries are skipped.
    pub disabled: bool,
    /// Native entry link0 gate: missing link is allowed; a resolved link whose
    /// vslot +0x80 returns zero suppresses this entry. The host resolves that
    /// trigger-specific dependency before entering the engine-neutral reducer.
    pub link0_gate_allows: bool,
    pub position_xyz: [f32; 3],
    /// Runtime entry +0x6C scaled by native 0.1.
    pub inner_radius: f32,
    /// Runtime entry +0x70 scaled by native 0.1.
    pub outer_radius: f32,
    /// Runtime entry +0x74. Mode 1 chooses avoidance side from preferred_yaw.
    pub side_mode: i32,
    /// Runtime entry +0x18, consumed only when side_mode == 1.
    pub preferred_yaw_radians: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsAiSteeringAvoidanceResult {
    pub target_yaw_radians: f32,
    pub matched_entry_index: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsLocomotionState {
    pub position_xyz: [f32; 3],
    pub yaw_radians: f32,
    pub linear_velocity_xyz: [f32; 3],
}

impl Default for RobotsLocomotionState {
    fn default() -> Self {
        Self {
            position_xyz: [0.0; 3],
            yaw_radians: 0.0,
            linear_velocity_xyz: [0.0; 3],
        }
    }
}

/// Mathematical contract for native angle-delta helper `0x004F21DB`:
/// shortest signed delta from `current` to `target` in [-PI, PI].
pub fn shortest_yaw_delta(current: f32, target: f32) -> f32 {
    let mut delta = (target - current + PI).rem_euclid(TAU) - PI;
    if delta == -PI && target - current > 0.0 {
        delta = PI;
    }
    delta
}

pub fn step_yaw_towards(current: f32, target: f32, max_delta_radians: f32) -> f32 {
    let max_delta = max_delta_radians.abs();
    current + shortest_yaw_delta(current, target).clamp(-max_delta, max_delta)
}

/// Engine-neutral contract for AI vslot `+0x118 = 0x00452FB0`.
/// Non-directional mode requests the caller-supplied AnimMode and directly
/// clamps the signed yaw error by `turn_rate * runtime_rate_scale / 60`.
/// Handler flag `0x4` instead delegates rotation to directional turn animation
/// and selects exact TurnOnSpotL/R without writing owner yaw directly.
pub fn step_ai_direct_turn_request(
    current_owner_yaw_radians: f32,
    yaw_error_radians: f32,
    max_turn_radians_per_second: f32,
    handler_flags_628: u32,
    requested_anim_mode: u32,
    runtime_rate_scale: f32,
) -> RobotsAiDirectTurnStep {
    if handler_flags_628 & ROBOTS_AI_DIRECTIONAL_TURN_MODE_FLAG != 0 {
        return RobotsAiDirectTurnStep {
            owner_yaw_radians: current_owner_yaw_radians,
            requested_anim_mode: if yaw_error_radians >= 0.0 {
                ROBOTS_ANIM_MODE_TURN_ON_SPOT_R
            } else {
                ROBOTS_ANIM_MODE_TURN_ON_SPOT_L
            },
            direct_owner_yaw_write: false,
        };
    }

    let max_delta =
        max_turn_radians_per_second.abs() * runtime_rate_scale.max(0.0) * ROBOTS_FIXED_STEP_SECONDS;
    RobotsAiDirectTurnStep {
        owner_yaw_radians: current_owner_yaw_radians
            + yaw_error_radians.clamp(-max_delta, max_delta),
        requested_anim_mode,
        direct_owner_yaw_write: true,
    }
}

fn signed_unit(value: f32) -> f32 {
    if value < 0.0 {
        -1.0
    } else if value > 0.0 {
        1.0
    } else {
        0.0
    }
}

/// Engine-neutral form of native AI steering pre-pass `0x00452B40`.
///
/// Native order is significant: entries are scanned in runtime container order
/// and the first matching avoidance sector rewrites Handler+0x5D8 and returns.
/// Invalid synthetic geometry fails closed instead of manufacturing NaN steering.
pub fn apply_ai_steering_avoidance_prepass(
    owner_position_xyz: [f32; 3],
    desired_yaw_radians: f32,
    entries: &[RobotsAiSteeringAvoidanceEntry],
) -> RobotsAiSteeringAvoidanceResult {
    for (entry_index, entry) in entries.iter().copied().enumerate() {
        if entry.disabled
            || !entry.link0_gate_allows
            || !entry.inner_radius.is_finite()
            || !entry.outer_radius.is_finite()
            || entry.inner_radius < 0.0
            || entry.outer_radius <= 0.0
            || !entry.position_xyz.iter().all(|value| value.is_finite())
        {
            continue;
        }

        let dx = entry.position_xyz[0] - owner_position_xyz[0];
        let dy = entry.position_xyz[1] - owner_position_xyz[1];
        let dz = entry.position_xyz[2] - owner_position_xyz[2];
        let distance_squared = dx * dx + dy * dy + dz * dz;
        if !distance_squared.is_finite()
            || distance_squared >= entry.outer_radius * entry.outer_radius
        {
            continue;
        }

        let entry_heading = dx.atan2(dz);
        let desired_from_entry = shortest_yaw_delta(entry_heading, desired_yaw_radians);
        if desired_from_entry.abs() > PI * 0.5 {
            continue;
        }

        let distance = distance_squared.max(0.0).sqrt();
        let half_angle = if entry.inner_radius <= 0.0 {
            continue;
        } else if distance > entry.inner_radius {
            (entry.inner_radius / distance)
                .clamp(-1.0, 1.0)
                .asin()
                .clamp(-PI * 0.5, PI * 0.5)
        } else {
            (1.0 - distance / entry.inner_radius) * (PI * 0.5) + PI * 0.5
        };

        if desired_from_entry.abs() > half_angle.abs() {
            continue;
        }

        let side = if entry.side_mode == 1 {
            signed_unit(shortest_yaw_delta(
                desired_yaw_radians,
                entry.preferred_yaw_radians,
            ))
        } else {
            signed_unit(desired_from_entry)
        };
        return RobotsAiSteeringAvoidanceResult {
            target_yaw_radians: entry_heading + half_angle.abs() * side,
            matched_entry_index: Some(entry_index),
        };
    }

    RobotsAiSteeringAvoidanceResult {
        target_yaw_radians: desired_yaw_radians,
        matched_entry_index: None,
    }
}

pub fn step_scalar_towards(current: f32, target: f32, max_delta: f32) -> f32 {
    let max_delta = max_delta.abs();
    current + (target - current).clamp(-max_delta, max_delta)
}

fn resolved_turn_rate(input: RobotsAiTurnRateInput) -> f32 {
    match input {
        RobotsAiTurnRateInput::Default => ROBOTS_AI_DEFAULT_TURN_RATE_RADIANS_PER_SECOND,
        RobotsAiTurnRateInput::Explicit(value) => value,
    }
}

/// Native `XItemHandler_AI_Character` Move contract (`0x00452800`) after
/// `0x00452B40` has already applied steering/avoidance corrections to +0x5D8.
///
/// This deliberately does not implement `0x00452B40` itself. Steering intent,
/// avoidance, locomotion smoothing and Physics remain separate UE-facing layers.
pub fn step_ai_locomotion_after_steering_prepass(
    state: &mut RobotsAiLocomotionRuntimeState,
    input: RobotsAiLocomotionInput,
) -> RobotsAiLocomotionStep {
    let runtime_rate_scale = input.runtime_rate_scale.max(0.0);
    let yaw_error = shortest_yaw_delta(
        input.current_owner_yaw_radians,
        input.steering_target_yaw_radians,
    );

    if input.handler_flags_628 & ROBOTS_AI_TURN_IN_PLACE_ENABLE_FLAG != 0 {
        let yaw_error_degrees = (yaw_error * ROBOTS_RADIANS_TO_DEGREES).abs();
        if state.turn_in_place_latch {
            if yaw_error_degrees < ROBOTS_AI_TURN_IN_PLACE_EXIT_DEGREES {
                state.turn_in_place_latch = false;
            }
        } else if yaw_error_degrees > ROBOTS_AI_TURN_IN_PLACE_ENTER_DEGREES {
            state.turn_in_place_latch = true;
        }
    }

    if state.turn_in_place_latch {
        if input.handler_flags_628 & ROBOTS_AI_DIRECTIONAL_TURN_MODE_FLAG != 0 {
            return RobotsAiLocomotionStep {
                owner_yaw_radians: input.current_owner_yaw_radians,
                locomotion_scalar: state.locomotion_scalar,
                requested_anim_mode: if yaw_error >= 0.0 {
                    ROBOTS_ANIM_MODE_TURN_ON_SPOT_R
                } else {
                    ROBOTS_ANIM_MODE_TURN_ON_SPOT_L
                },
                turn_in_place_active: true,
                direct_owner_yaw_write: false,
            };
        }

        let max_turn_step =
            resolved_turn_rate(input.turn_rate) * runtime_rate_scale * ROBOTS_FIXED_STEP_SECONDS;
        return RobotsAiLocomotionStep {
            owner_yaw_radians: input.current_owner_yaw_radians
                + yaw_error.clamp(-max_turn_step.abs(), max_turn_step.abs()),
            locomotion_scalar: state.locomotion_scalar,
            requested_anim_mode: ROBOTS_ANIM_MODE_TURN_ON_SPOT,
            turn_in_place_active: true,
            direct_owner_yaw_write: true,
        };
    }

    let raw_turn_input = match input.turn_rate {
        RobotsAiTurnRateInput::Default => -1.0,
        RobotsAiTurnRateInput::Explicit(value) => value,
    };
    let mut owner_yaw_radians = input.current_owner_yaw_radians;
    let direct_owner_yaw_write = raw_turn_input.abs() > 0.001;
    if direct_owner_yaw_write {
        let max_turn_step =
            resolved_turn_rate(input.turn_rate) * runtime_rate_scale * ROBOTS_FIXED_STEP_SECONDS;
        owner_yaw_radians += yaw_error.clamp(-max_turn_step.abs(), max_turn_step.abs());
    }

    if !input.move_mode_active_on_entry {
        state.locomotion_scalar = 0.0;
    }
    state.locomotion_scalar = step_scalar_towards(
        state.locomotion_scalar,
        input.target_locomotion_scalar,
        ROBOTS_AI_LOCOMOTION_SCALAR_RATE_PER_SECOND
            * runtime_rate_scale
            * ROBOTS_FIXED_STEP_SECONDS,
    );

    RobotsAiLocomotionStep {
        owner_yaw_radians,
        locomotion_scalar: state.locomotion_scalar,
        requested_anim_mode: ROBOTS_ANIM_MODE_MOVE,
        turn_in_place_active: false,
        direct_owner_yaw_write,
    }
}

/// Native forward basis: X=sin(yaw), Y=0, Z=cos(yaw).
pub fn forward_from_yaw(yaw_radians: f32) -> [f32; 3] {
    [yaw_radians.sin(), 0.0, yaw_radians.cos()]
}

/// Native vslot +0x110 (`0x00452E70`) maps locomotion scalar to the configured
/// Handler+0x5DC..+0x5E0 speed range, clamped exactly to that range.
pub fn ai_move_speed_from_scalar(min_speed: f32, max_speed: f32, scalar: f32) -> f32 {
    let raw = (max_speed - min_speed) * scalar + min_speed;
    if raw < max_speed {
        if raw < min_speed {
            min_speed
        } else {
            raw
        }
    } else {
        max_speed
    }
}

/// Character Physics velocity written by `0x00452E70` to +0x1B8..+0x1C4.
pub fn ai_character_physics_velocity_from_scalar(
    yaw_radians: f32,
    min_speed: f32,
    max_speed: f32,
    locomotion_scalar: f32,
    runtime_rate_scale: f32,
) -> [f32; 3] {
    let speed = ai_move_speed_from_scalar(min_speed, max_speed, locomotion_scalar)
        * runtime_rate_scale.max(0.0);
    let forward = forward_from_yaw(yaw_radians);
    [forward[0] * speed, 0.0, forward[2] * speed]
}

/// Accumulate the Roller-specific acceleration-like forward command into
/// retained velocity. `0x004681F0` performs direction * input * fixed_step.
pub fn accumulate_forward_acceleration(
    velocity_xyz: &mut [f32; 3],
    yaw_radians: f32,
    forward_acceleration: f32,
    delta_seconds: f32,
) {
    let forward = forward_from_yaw(yaw_radians);
    for axis in 0..3 {
        velocity_xyz[axis] += forward[axis] * forward_acceleration * delta_seconds;
    }
}

/// `0x00467F80` damps retained Roller motion lanes before copying them to
/// Character Physics. Do not replace this with UE braking until parity says so.
pub fn damp_velocity(velocity_xyz: &mut [f32; 3], damping: f32) {
    for value in velocity_xyz {
        *value *= damping;
    }
}

/// Base Physics slot0 `0x00419400` integrates motion lanes into owner XItem
/// position using the fixed-step factor.
pub fn integrate_owner_position(
    position_xyz: &mut [f32; 3],
    velocity_xyz: [f32; 3],
    delta_seconds: f32,
) {
    for axis in 0..3 {
        position_xyz[axis] += velocity_xyz[axis] * delta_seconds;
    }
}

/// Roller-specific proven order:
/// steer -> acceleration accumulation -> damping -> Character Physics integrate.
pub fn step_accel_damped_locomotion(
    state: &mut RobotsLocomotionState,
    target_yaw_radians: f32,
    max_turn_radians_per_second: f32,
    forward_acceleration: f32,
    velocity_damping: f32,
    delta_seconds: f32,
) {
    let delta_seconds = delta_seconds.max(0.0);
    state.yaw_radians = step_yaw_towards(
        state.yaw_radians,
        target_yaw_radians,
        max_turn_radians_per_second * delta_seconds,
    );
    accumulate_forward_acceleration(
        &mut state.linear_velocity_xyz,
        state.yaw_radians,
        forward_acceleration,
        delta_seconds,
    );
    damp_velocity(&mut state.linear_velocity_xyz, velocity_damping);
    integrate_owner_position(
        &mut state.position_xyz,
        state.linear_velocity_xyz,
        delta_seconds,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_forward_basis_is_x_sin_z_cos() {
        assert_eq!(forward_from_yaw(0.0), [0.0, 0.0, 1.0]);
        let right = forward_from_yaw(PI * 0.5);
        assert!((right[0] - 1.0).abs() < 1.0e-6);
        assert!(right[2].abs() < 1.0e-6);
    }

    #[test]
    fn roller_first_fixed_step_accumulates_then_damps_then_integrates() {
        let mut state = RobotsLocomotionState::default();
        step_accel_damped_locomotion(
            &mut state,
            0.0,
            0.0,
            ROBOTS_ROLLERBOT_FORWARD_ACCELERATION,
            ROBOTS_ROLLERBOT_VELOCITY_DAMPING,
            ROBOTS_FIXED_STEP_SECONDS,
        );
        let expected_velocity =
            10.0 * ROBOTS_FIXED_STEP_SECONDS * ROBOTS_ROLLERBOT_VELOCITY_DAMPING;
        let expected_position = expected_velocity * ROBOTS_FIXED_STEP_SECONDS;
        assert!((state.linear_velocity_xyz[2] - expected_velocity).abs() < 1.0e-6);
        assert!((state.position_xyz[2] - expected_position).abs() < 1.0e-6);
    }

    #[test]
    fn zero_acceleration_stationary_subset_does_not_translate() {
        let mut state = RobotsLocomotionState::default();
        step_accel_damped_locomotion(&mut state, PI, PI, 0.0, 1.0, ROBOTS_FIXED_STEP_SECONDS);
        assert_eq!(state.position_xyz, [0.0; 3]);
        assert_eq!(state.linear_velocity_xyz, [0.0; 3]);
    }

    #[test]
    fn ai_movement_error_uses_previous_expected_vs_actual_xz_displacement() {
        let mut state = RobotsAiMovementErrorRuntimeState::default();

        // First native-style sample has no expected displacement yet, so +0x5E4
        // stays zero while the next expected step is seeded from velocity.
        assert_eq!(
            step_ai_movement_error(&mut state, [10.0, 2.0, 20.0], [60.0, 0.0, 0.0], 1.0),
            0.0
        );
        assert_eq!(state.expected_displacement_xyz, [1.0, 0.0, 0.0]);

        // Expected +1 X and actual +1 X is exact tracking.
        assert_eq!(
            step_ai_movement_error(&mut state, [11.0, 99.0, 20.0], [60.0, 0.0, 0.0], 1.0),
            0.0
        );

        // Expected +1 X but only +0.5 X actual => normalized error 0.5.
        let error = step_ai_movement_error(&mut state, [11.5, -500.0, 20.0], [60.0, 0.0, 0.0], 1.0);
        assert!((error - 0.5).abs() < 1.0e-6);
    }

    #[test]
    fn ai_movement_error_reports_full_stall_and_ignores_y_only_error() {
        let mut state = RobotsAiMovementErrorRuntimeState {
            expected_displacement_xyz: [0.0, 123.0, 2.0],
            previous_owner_position_xyz: [5.0, -10.0, 7.0],
            movement_error_ratio: 0.0,
        };
        let error = step_ai_movement_error(&mut state, [5.0, 1000.0, 7.0], [0.0, 0.0, 120.0], 1.0);
        assert!((error - 1.0).abs() < 1.0e-6);
        assert_eq!(state.expected_displacement_xyz, [0.0, 0.0, 2.0]);
    }

    #[test]
    fn ai_move_scalar_ramps_by_one_sixtieth_per_fixed_tick() {
        let mut state = RobotsAiLocomotionRuntimeState::default();
        let result = step_ai_locomotion_after_steering_prepass(
            &mut state,
            RobotsAiLocomotionInput {
                steering_target_yaw_radians: 0.0,
                target_locomotion_scalar: 1.0,
                turn_rate: RobotsAiTurnRateInput::Default,
                handler_flags_628: 0,
                move_mode_active_on_entry: false,
                current_owner_yaw_radians: 0.0,
                runtime_rate_scale: 1.0,
            },
        );
        assert_eq!(result.requested_anim_mode, ROBOTS_ANIM_MODE_MOVE);
        assert!((result.locomotion_scalar - ROBOTS_FIXED_STEP_SECONDS).abs() < 1.0e-6);
    }

    #[test]
    fn ai_default_turn_rate_is_seven_and_a_half_degrees_per_fixed_tick() {
        let mut state = RobotsAiLocomotionRuntimeState::default();
        let result = step_ai_locomotion_after_steering_prepass(
            &mut state,
            RobotsAiLocomotionInput {
                steering_target_yaw_radians: PI * 0.5,
                target_locomotion_scalar: 1.0,
                turn_rate: RobotsAiTurnRateInput::Default,
                handler_flags_628: 0,
                move_mode_active_on_entry: true,
                current_owner_yaw_radians: 0.0,
                runtime_rate_scale: 1.0,
            },
        );
        assert!((result.owner_yaw_radians - PI / 24.0).abs() < 1.0e-6);
    }

    #[test]
    fn direct_turn_request_clamps_pi_over_two_rate_to_one_and_a_half_degrees_per_tick() {
        let result = step_ai_direct_turn_request(
            0.0,
            PI * 0.5,
            PI * 0.5,
            0,
            ROBOTS_ANIM_MODE_TURN_ON_SPOT,
            1.0,
        );
        assert!(result.direct_owner_yaw_write);
        assert_eq!(result.requested_anim_mode, ROBOTS_ANIM_MODE_TURN_ON_SPOT);
        assert!((result.owner_yaw_radians - PI / 120.0).abs() < 1.0e-6);
    }

    #[test]
    fn direct_turn_request_directional_flag_delegates_yaw_to_left_right_animation() {
        let right = step_ai_direct_turn_request(
            0.25,
            0.5,
            PI * 0.5,
            ROBOTS_AI_DIRECTIONAL_TURN_MODE_FLAG,
            ROBOTS_ANIM_MODE_TURN_ON_SPOT,
            1.0,
        );
        assert!(!right.direct_owner_yaw_write);
        assert_eq!(right.owner_yaw_radians, 0.25);
        assert_eq!(right.requested_anim_mode, ROBOTS_ANIM_MODE_TURN_ON_SPOT_R);

        let left = step_ai_direct_turn_request(
            0.25,
            -0.5,
            PI * 0.5,
            ROBOTS_AI_DIRECTIONAL_TURN_MODE_FLAG,
            ROBOTS_ANIM_MODE_TURN_ON_SPOT,
            1.0,
        );
        assert_eq!(left.requested_anim_mode, ROBOTS_ANIM_MODE_TURN_ON_SPOT_L);
    }

    #[test]
    fn turn_in_place_uses_exact_thirty_five_degree_hysteresis() {
        let mut state = RobotsAiLocomotionRuntimeState::default();
        let enter = step_ai_locomotion_after_steering_prepass(
            &mut state,
            RobotsAiLocomotionInput {
                steering_target_yaw_radians: 31.0 / ROBOTS_RADIANS_TO_DEGREES,
                target_locomotion_scalar: 1.0,
                turn_rate: RobotsAiTurnRateInput::Default,
                handler_flags_628: ROBOTS_AI_TURN_IN_PLACE_ENABLE_FLAG,
                move_mode_active_on_entry: true,
                current_owner_yaw_radians: 0.0,
                runtime_rate_scale: 1.0,
            },
        );
        assert!(enter.turn_in_place_active);
        assert_eq!(enter.requested_anim_mode, ROBOTS_ANIM_MODE_TURN_ON_SPOT);

        let exit = step_ai_locomotion_after_steering_prepass(
            &mut state,
            RobotsAiLocomotionInput {
                steering_target_yaw_radians: 4.0 / ROBOTS_RADIANS_TO_DEGREES,
                target_locomotion_scalar: 1.0,
                turn_rate: RobotsAiTurnRateInput::Default,
                handler_flags_628: ROBOTS_AI_TURN_IN_PLACE_ENABLE_FLAG,
                move_mode_active_on_entry: true,
                current_owner_yaw_radians: 0.0,
                runtime_rate_scale: 1.0,
            },
        );
        assert!(!exit.turn_in_place_active);
        assert_eq!(exit.requested_anim_mode, ROBOTS_ANIM_MODE_MOVE);
    }

    #[test]
    fn directional_turn_mode_requests_left_without_direct_yaw_write() {
        let mut state = RobotsAiLocomotionRuntimeState {
            locomotion_scalar: 0.5,
            turn_in_place_latch: true,
        };
        let result = step_ai_locomotion_after_steering_prepass(
            &mut state,
            RobotsAiLocomotionInput {
                steering_target_yaw_radians: -PI * 0.5,
                target_locomotion_scalar: 1.0,
                turn_rate: RobotsAiTurnRateInput::Default,
                handler_flags_628: ROBOTS_AI_TURN_IN_PLACE_ENABLE_FLAG
                    | ROBOTS_AI_DIRECTIONAL_TURN_MODE_FLAG,
                move_mode_active_on_entry: true,
                current_owner_yaw_radians: 0.0,
                runtime_rate_scale: 1.0,
            },
        );
        assert_eq!(result.requested_anim_mode, ROBOTS_ANIM_MODE_TURN_ON_SPOT_L);
        assert!(!result.direct_owner_yaw_write);
        assert_eq!(result.owner_yaw_radians, 0.0);
        assert_eq!(result.locomotion_scalar, 0.5);
    }

    #[test]
    fn generic_ai_velocity_interpolates_configured_min_max_speed() {
        let velocity = ai_character_physics_velocity_from_scalar(0.0, 2.0, 8.0, 0.5, 1.0);
        assert_eq!(velocity, [0.0, 0.0, 5.0]);
        assert_eq!(ai_move_speed_from_scalar(2.0, 8.0, -1.0), 2.0);
        assert_eq!(ai_move_speed_from_scalar(2.0, 8.0, 2.0), 8.0);
    }

    fn avoidance_entry(position_xyz: [f32; 3]) -> RobotsAiSteeringAvoidanceEntry {
        RobotsAiSteeringAvoidanceEntry {
            disabled: false,
            link0_gate_allows: true,
            position_xyz,
            inner_radius: 2.0,
            outer_radius: 20.0,
            side_mode: 0,
            preferred_yaw_radians: 0.0,
        }
    }

    #[test]
    fn steering_avoidance_ignores_entries_outside_outer_radius() {
        let result = apply_ai_steering_avoidance_prepass(
            [0.0, 0.0, 0.0],
            0.25,
            &[avoidance_entry([0.0, 0.0, 25.0])],
        );
        assert_eq!(result.matched_entry_index, None);
        assert_eq!(result.target_yaw_radians, 0.25);
    }

    #[test]
    fn steering_avoidance_outside_inner_radius_uses_asin_sector() {
        let result = apply_ai_steering_avoidance_prepass(
            [0.0, 0.0, 0.0],
            0.1,
            &[avoidance_entry([0.0, 0.0, 10.0])],
        );
        let expected = (2.0f32 / 10.0).asin();
        assert_eq!(result.matched_entry_index, Some(0));
        assert!((result.target_yaw_radians - expected).abs() < 1.0e-6);
    }

    #[test]
    fn steering_avoidance_inside_inner_radius_expands_sector_toward_pi() {
        let mut entry = avoidance_entry([0.0, 0.0, 5.0]);
        entry.inner_radius = 10.0;
        let result = apply_ai_steering_avoidance_prepass([0.0, 0.0, 0.0], 0.1, &[entry]);
        assert_eq!(result.matched_entry_index, Some(0));
        assert!((result.target_yaw_radians - 3.0 * PI / 4.0).abs() < 1.0e-6);
    }

    #[test]
    fn steering_avoidance_mode1_uses_preferred_yaw_to_choose_side() {
        let mut entry = avoidance_entry([0.0, 0.0, 10.0]);
        entry.side_mode = 1;
        entry.preferred_yaw_radians = -1.0;
        let result = apply_ai_steering_avoidance_prepass([0.0, 0.0, 0.0], 0.1, &[entry]);
        let expected = -(2.0f32 / 10.0).asin();
        assert_eq!(result.matched_entry_index, Some(0));
        assert!((result.target_yaw_radians - expected).abs() < 1.0e-6);
    }

    #[test]
    fn steering_avoidance_first_matching_runtime_entry_wins() {
        let first = avoidance_entry([0.0, 0.0, 10.0]);
        let mut second = avoidance_entry([2.0, 0.0, 10.0]);
        second.inner_radius = 4.0;
        let result = apply_ai_steering_avoidance_prepass([0.0, 0.0, 0.0], 0.1, &[first, second]);
        assert_eq!(result.matched_entry_index, Some(0));
        assert!((result.target_yaw_radians - (2.0f32 / 10.0).asin()).abs() < 1.0e-6);
    }
}
