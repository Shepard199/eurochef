use serde::Serialize;

use super::locomotion::RobotsAiTurnRateInput;

pub const ROBOTS_CIRCLE_TARGET_PRIORITY: u8 = 0x28;
pub const ROBOTS_CIRCLE_TARGET_DIRECTION_RANDOM: u32 = 2;
pub const ROBOTS_CIRCLE_TARGET_LOCOMOTION_SCALAR: f32 = 1.0;
pub const ROBOTS_CIRCLE_TARGET_RIGHT_ANGLE_RADIANS: f32 = std::f32::consts::FRAC_PI_2;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsCircleTargetConfig {
    pub enter_radius: f32,
    pub retain_radius: f32,
    pub orbit_radius: f32,
    /// Native setup field node+0x30. Value 2 selects one process-RNG bit on enter;
    /// zero/one select the side directly.
    pub direction_mode: u32,
}

impl RobotsCircleTargetConfig {
    /// GuardBot builder `0x0045DD50 -> AI_CircleTarget::Setup 0x0046ACB0`.
    pub const fn guardbot() -> Self {
        Self {
            enter_radius: 15.0,
            retain_radius: 18.0,
            orbit_radius: 10.0,
            direction_mode: ROBOTS_CIRCLE_TARGET_DIRECTION_RANDOM,
        }
    }

    /// EW08 Flambe/FlambeLarge builder `0x00462760` passes the same four
    /// CircleTarget arguments as GuardBot: 15, 18, 10, direction mode 2.
    pub const fn flambe() -> Self {
        Self::guardbot()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RobotsCircleTargetRuntimeState {
    /// Node+0x34. Zero chooses the negative angular offset; any nonzero value
    /// chooses the positive angular offset.
    pub direction_side: u32,
    pub initialized: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsCircleTargetStep {
    pub steering_target_yaw_radians: f32,
    pub target_locomotion_scalar: f32,
    pub turn_rate: RobotsAiTurnRateInput,
}

/// Native priority callback `0x0046ACF0`. The common node active byte supplies
/// the 15/18-unit hysteresis edge; target visibility is Handler+0x5FA.
pub fn circle_target_priority(
    config: RobotsCircleTargetConfig,
    was_active: bool,
    target_visible: bool,
    owner_position_xyz: [f32; 3],
    target_position_xyz: Option<[f32; 3]>,
) -> u8 {
    let Some(target) = target_position_xyz else {
        return 1;
    };
    if !target_visible {
        return 1;
    }
    let dx = owner_position_xyz[0] - target[0];
    let dy = owner_position_xyz[1] - target[1];
    let dz = owner_position_xyz[2] - target[2];
    let distance_squared = dx * dx + dy * dy + dz * dz;
    let eligible = if was_active {
        distance_squared < config.retain_radius * config.retain_radius
    } else {
        distance_squared <= config.enter_radius * config.enter_radius
    };
    if eligible {
        ROBOTS_CIRCLE_TARGET_PRIORITY
    } else {
        1
    }
}

/// Native enter callback `0x0046ADD0`. Direction mode 2 consumes exactly one
/// process-global RNG draw and keeps only its low bit. Returning false means the
/// host lacks the required RNG anchor and must not commit the node transition.
pub fn enter_circle_target(
    state: &mut RobotsCircleTargetRuntimeState,
    config: RobotsCircleTargetConfig,
    process_rng_draw: Option<u32>,
) -> bool {
    let direction_side = if config.direction_mode == ROBOTS_CIRCLE_TARGET_DIRECTION_RANDOM {
        let Some(draw) = process_rng_draw else {
            return false;
        };
        u32::from(draw & 1 != 0)
    } else {
        config.direction_mode
    };
    state.direction_side = direction_side;
    state.initialized = true;
    true
}

fn native_circle_offset_radians(distance: f32, orbit_radius: f32) -> f32 {
    if distance < orbit_radius {
        return ((1.0 - distance / orbit_radius) * ROBOTS_CIRCLE_TARGET_RIGHT_ANGLE_RADIANS
            + ROBOTS_CIRCLE_TARGET_RIGHT_ANGLE_RADIANS)
            .abs();
    }

    let ratio = orbit_radius / distance;
    let angle = if ratio < -1.0 {
        -ROBOTS_CIRCLE_TARGET_RIGHT_ANGLE_RADIANS
    } else if ratio > 1.0 {
        ROBOTS_CIRCLE_TARGET_RIGHT_ANGLE_RADIANS
    } else {
        let acos = ratio.acos();
        if acos >= ROBOTS_CIRCLE_TARGET_RIGHT_ANGLE_RADIANS {
            ROBOTS_CIRCLE_TARGET_RIGHT_ANGLE_RADIANS
        } else if acos < -ROBOTS_CIRCLE_TARGET_RIGHT_ANGLE_RADIANS {
            -ROBOTS_CIRCLE_TARGET_RIGHT_ANGLE_RADIANS
        } else {
            acos
        }
    };
    angle.abs()
}

/// Native execute callback `0x0046AE00`. Distance is full XYZ, while the final
/// facing angle is the target bearing in XZ plus/minus the native orbit offset.
/// Movement itself remains owned by common Monster vslot +0x10C / locomotion.
pub fn step_circle_target(
    state: RobotsCircleTargetRuntimeState,
    config: RobotsCircleTargetConfig,
    owner_position_xyz: [f32; 3],
    target_position_xyz: Option<[f32; 3]>,
) -> Option<RobotsCircleTargetStep> {
    let target = target_position_xyz?;
    if !state.initialized {
        return None;
    }
    let dx = target[0] - owner_position_xyz[0];
    let dy = target[1] - owner_position_xyz[1];
    let dz = target[2] - owner_position_xyz[2];
    let distance = (dx * dx + dy * dy + dz * dz).max(0.0).sqrt();
    if config.orbit_radius == 0.0 && distance == 0.0 {
        return None;
    }
    let offset = native_circle_offset_radians(distance, config.orbit_radius);
    let signed_offset = if state.direction_side == 0 {
        -offset
    } else {
        offset
    };
    Some(RobotsCircleTargetStep {
        steering_target_yaw_radians: dx.atan2(dz) + signed_offset,
        target_locomotion_scalar: ROBOTS_CIRCLE_TARGET_LOCOMOTION_SCALAR,
        turn_rate: RobotsAiTurnRateInput::Default,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guardbot_priority_uses_native_15_18_hysteresis_and_visibility() {
        let config = RobotsCircleTargetConfig::guardbot();
        assert_eq!(
            circle_target_priority(config, false, true, [0.0; 3], Some([15.0, 0.0, 0.0])),
            ROBOTS_CIRCLE_TARGET_PRIORITY
        );
        assert_eq!(
            circle_target_priority(config, false, true, [0.0; 3], Some([15.01, 0.0, 0.0])),
            1
        );
        assert_eq!(
            circle_target_priority(config, true, true, [0.0; 3], Some([17.99, 0.0, 0.0])),
            ROBOTS_CIRCLE_TARGET_PRIORITY
        );
        assert_eq!(
            circle_target_priority(config, true, true, [0.0; 3], Some([18.0, 0.0, 0.0])),
            1
        );
        assert_eq!(
            circle_target_priority(config, true, false, [0.0; 3], Some([10.0, 0.0, 0.0])),
            1
        );
    }

    #[test]
    fn random_direction_enter_consumes_only_low_bit_and_fails_closed_without_rng() {
        let config = RobotsCircleTargetConfig::guardbot();
        let mut state = RobotsCircleTargetRuntimeState::default();
        assert!(!enter_circle_target(&mut state, config, None));
        assert!(!state.initialized);
        assert!(enter_circle_target(&mut state, config, Some(0x1234)));
        assert_eq!(state.direction_side, 0);
        assert!(enter_circle_target(&mut state, config, Some(0x1235)));
        assert_eq!(state.direction_side, 1);
    }

    #[test]
    fn execute_preserves_native_orbit_angle_piecewise_behavior() {
        let config = RobotsCircleTargetConfig::guardbot();
        let state = RobotsCircleTargetRuntimeState {
            direction_side: 1,
            initialized: true,
        };
        let near =
            step_circle_target(state, config, [0.0; 3], Some([5.0, 0.0, 0.0])).expect("near step");
        assert!(
            (near.steering_target_yaw_radians
                - (std::f32::consts::FRAC_PI_2 + 3.0 * std::f32::consts::PI / 4.0))
                .abs()
                <= 1.0e-6
        );

        let exact = step_circle_target(state, config, [0.0; 3], Some([10.0, 0.0, 0.0]))
            .expect("exact orbit step");
        assert!((exact.steering_target_yaw_radians - std::f32::consts::FRAC_PI_2).abs() <= 1.0e-6);

        let far =
            step_circle_target(state, config, [0.0; 3], Some([20.0, 0.0, 0.0])).expect("far step");
        assert!(
            (far.steering_target_yaw_radians
                - (std::f32::consts::FRAC_PI_2 + std::f32::consts::PI / 3.0))
                .abs()
                <= 1.0e-6
        );
        assert_eq!(far.target_locomotion_scalar, 1.0);
        assert_eq!(far.turn_rate, RobotsAiTurnRateInput::Default);
    }
}
