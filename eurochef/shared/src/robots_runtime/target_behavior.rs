use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsProximityAnimBehaviorConfig {
    pub anim_mode: u32,
    pub enter_distance: f32,
    pub retain_distance: f32,
    pub yaw_tolerance_radians: f32,
    pub priority: u8,
}

impl RobotsProximityAnimBehaviorConfig {
    /// EB11 MagnaBot builder `0x00464810 -> 0x00458F40`.
    ///
    /// Native also stores the setup's first distance argument (0.0) at node+0x28,
    /// but the recovered gate/execute methods never read it, so it is intentionally
    /// not promoted into the engine-facing contract.
    pub const fn magnabot_idle_attack() -> Self {
        Self {
            anim_mode: 0x0900_0004,
            enter_distance: 12.0,
            retain_distance: 13.0,
            yaw_tolerance_radians: std::f32::consts::TAU,
            priority: 0x14,
        }
    }
}

/// Native `0x00458F90`: Player-target proximity gate with active-node distance
/// hysteresis. Distance and yaw comparisons are intentionally expressed as
/// "greater than" rejects so NaN follows the x87 unordered path and is not made
/// stricter than the original executable.
pub fn proximity_anim_priority(
    config: RobotsProximityAnimBehaviorConfig,
    node_active: bool,
    target_exists: bool,
    target_distance_squared: f32,
    target_yaw_error_radians: f32,
) -> u8 {
    if !target_exists {
        return 1;
    }
    let distance = if node_active {
        config.retain_distance
    } else {
        config.enter_distance
    };
    if target_distance_squared > distance * distance {
        return 1;
    }
    if target_yaw_error_radians.abs() > config.yaw_tolerance_radians {
        return 0;
    }
    config.priority
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsTargetFacingTurnBehaviorConfig {
    /// Native node+0x24. The node wins only while absolute target yaw is strictly
    /// greater than this value.
    pub yaw_deadzone_radians: f32,
    /// Native node+0x28, forwarded as the resolved turn-rate argument to handler
    /// vslot +0x118 after +0x13C returns target yaw error.
    pub turn_rate_radians_per_second: f32,
    pub inner_distance: f32,
    pub enter_distance: f32,
    pub retain_distance: f32,
    pub anim_mode: u32,
    pub priority: u8,
}

impl RobotsTargetFacingTurnBehaviorConfig {
    /// EB11 MagnaBot builder `0x00464810 -> 0x0046A1B0`.
    pub const fn magnabot() -> Self {
        Self {
            yaw_deadzone_radians: f32::from_bits(0x3c8e_fa35),
            turn_rate_radians_per_second: std::f32::consts::PI,
            inner_distance: 0.0,
            enter_distance: 12.0,
            retain_distance: 13.0,
            anim_mode: 0x0900_0028,
            priority: 0x19,
        }
    }
}

/// Native `0x0046A230`: annulus + active-node hysteresis + target-facing gate.
pub fn target_facing_turn_priority(
    config: RobotsTargetFacingTurnBehaviorConfig,
    node_active: bool,
    target_exists: bool,
    target_distance_squared: f32,
    target_yaw_error_radians: f32,
) -> u8 {
    if !target_exists {
        return 1;
    }
    if target_distance_squared < config.inner_distance * config.inner_distance {
        return 1;
    }
    let outer_distance = if node_active {
        config.retain_distance
    } else {
        config.enter_distance
    };
    if target_distance_squared > outer_distance * outer_distance {
        return 1;
    }
    if target_yaw_error_radians.abs() > config.yaw_deadzone_radians {
        config.priority
    } else {
        1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsTargetFacingTurnRequest {
    pub yaw_error_radians: f32,
    pub turn_rate_radians_per_second: f32,
    pub anim_mode: u32,
}

/// Native `0x0046A200`: +0x13C target-yaw result is forwarded to handler +0x118,
/// while the node's stored PI value and TurnOnSpot mode remain on the call stack.
pub const fn target_facing_turn_request(
    config: RobotsTargetFacingTurnBehaviorConfig,
    yaw_error_radians: f32,
) -> RobotsTargetFacingTurnRequest {
    RobotsTargetFacingTurnRequest {
        yaw_error_radians,
        turn_rate_radians_per_second: config.turn_rate_radians_per_second,
        anim_mode: config.anim_mode,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magnabot_proximity_idle_attack_matches_12_13_hysteresis_and_priority20() {
        let config = RobotsProximityAnimBehaviorConfig::magnabot_idle_attack();
        assert_eq!(
            proximity_anim_priority(config, false, true, 12.0f32.powi(2), 0.0),
            0x14
        );
        assert_eq!(
            proximity_anim_priority(config, false, true, 12.01f32.powi(2), 0.0),
            1
        );
        assert_eq!(
            proximity_anim_priority(config, true, true, 13.0f32.powi(2), 0.0),
            0x14
        );
        assert_eq!(
            proximity_anim_priority(config, true, true, 13.01f32.powi(2), 0.0),
            1
        );
        assert_eq!(proximity_anim_priority(config, false, false, 0.0, 0.0), 1);
        assert_eq!(config.anim_mode, 0x0900_0004);
    }

    #[test]
    fn magnabot_target_facing_turn_matches_zero_12_13_annulus_and_one_degree_deadzone() {
        let config = RobotsTargetFacingTurnBehaviorConfig::magnabot();
        assert_eq!(
            target_facing_turn_priority(config, false, true, 12.0f32.powi(2), 0.02),
            0x19
        );
        assert_eq!(
            target_facing_turn_priority(config, false, true, 12.01f32.powi(2), 0.02),
            1
        );
        assert_eq!(
            target_facing_turn_priority(config, true, true, 13.0f32.powi(2), 0.02),
            0x19
        );
        assert_eq!(
            target_facing_turn_priority(config, true, true, 13.01f32.powi(2), 0.02),
            1
        );
        assert_eq!(
            target_facing_turn_priority(config, false, true, 1.0, config.yaw_deadzone_radians),
            1
        );
        assert_eq!(
            target_facing_turn_priority(
                config,
                false,
                true,
                1.0,
                config.yaw_deadzone_radians + 0.0001
            ),
            0x19
        );
        let request = target_facing_turn_request(config, 0.5);
        assert_eq!(
            request.turn_rate_radians_per_second.to_bits(),
            std::f32::consts::PI.to_bits()
        );
        assert_eq!(request.anim_mode, 0x0900_0028);
    }
}
