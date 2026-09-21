use serde::Serialize;

use super::locomotion::{shortest_yaw_delta, step_ai_direct_turn_request, RobotsAiDirectTurnStep};

pub const ROBOTS_TURN_THEN_ATTACK_INITIAL_INACTIVE_TICKS: u32 = 0x1_0000;
pub const ROBOTS_TURN_THEN_ATTACK_PRIORITY: u8 = 0x32;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsTurnThenAttackConfig {
    /// Node +0x6C. Enter starts here and execute calls owner vslot +0x118.
    pub turn_anim_mode: u32,
    /// Node +0x68. Execute switches here once post-turn |yaw error| <= tolerance.
    pub attack_anim_mode: u32,
    /// Node +0x38/+0x3C are stored squared by setup `0x00450120`.
    pub inner_radius: f32,
    pub outer_radius: f32,
    /// Node +0x40. Checked after the +0x118 turn request, not by geometry gate.
    pub yaw_completion_tolerance_radians: f32,
    /// Node +0x70, forwarded to owner vslot +0x118.
    pub turn_rate_radians_per_second: f32,
    /// Node +0x64, consumed by common attack geometry helper `0x0044F3C0(false)`.
    pub vertical_limit: f32,
    /// Common behavior base +0x18.
    pub reentry_delay_ticks: u32,
    pub priority: u8,
}

impl RobotsTurnThenAttackConfig {
    /// Monster_2Rockets builder `0x0045B700`, AttackGroup child0.
    pub const fn monster_2rockets_primary() -> Self {
        Self {
            turn_anim_mode: 0x0900_0026,
            attack_anim_mode: 0x0900_0025,
            inner_radius: 1.9,
            outer_radius: 15.0,
            yaw_completion_tolerance_radians: f32::from_bits(0x3c8e_fa35),
            turn_rate_radians_per_second: std::f32::consts::PI,
            vertical_limit: 1000.0,
            reentry_delay_ticks: 120,
            priority: ROBOTS_TURN_THEN_ATTACK_PRIORITY,
        }
    }

    /// Monster_2Rockets builder `0x0045B700`, AttackGroup child1.
    pub const fn monster_2rockets_secondary() -> Self {
        Self {
            turn_anim_mode: 0x0900_0026,
            attack_anim_mode: 0x0900_0027,
            inner_radius: 0.0,
            outer_radius: 2.0,
            yaw_completion_tolerance_radians: f32::from_bits(0x3c8e_fa35),
            turn_rate_radians_per_second: std::f32::consts::PI,
            vertical_limit: 1000.0,
            reentry_delay_ticks: 30,
            priority: ROBOTS_TURN_THEN_ATTACK_PRIORITY,
        }
    }

    /// ShieldBot builder `0x0045E240`. Unlike the visually similar Launcher
    /// annulus, the final raw setup argument is exactly 1.0, so this attack is
    /// intentionally constrained to near-level targets.
    pub const fn shieldbot() -> Self {
        Self {
            turn_anim_mode: 0x0900_0003,
            attack_anim_mode: 0x0900_0027,
            inner_radius: 2.0,
            outer_radius: 15.0,
            yaw_completion_tolerance_radians: f32::from_bits(0x3c8e_fa35),
            turn_rate_radians_per_second: std::f32::consts::TAU,
            vertical_limit: 1.0,
            reentry_delay_ticks: 30,
            priority: ROBOTS_TURN_THEN_ATTACK_PRIORITY,
        }
    }

    /// EB05 GuardBot builder `0x0045DD50`. GuardBot's AttackGroup contains only
    /// this child, so the host must bind it at child index zero rather than the
    /// KnightBot/Launcher index-one convention.
    pub const fn guardbot() -> Self {
        Self {
            turn_anim_mode: 0x0900_0003,
            attack_anim_mode: 0x0900_0025,
            inner_radius: 0.0,
            outer_radius: 12.0,
            yaw_completion_tolerance_radians: f32::from_bits(0x3c8e_fa35),
            turn_rate_radians_per_second: std::f32::consts::TAU,
            vertical_limit: 1000.0,
            reentry_delay_ticks: 60,
            priority: ROBOTS_TURN_THEN_ATTACK_PRIORITY,
        }
    }

    /// EW08 Flambe/FlambeLarge shared builder `0x00462760`. This node is
    /// inserted directly into the BehaviorHost, not wrapped in AttackGroup.
    pub const fn flambe() -> Self {
        Self {
            turn_anim_mode: 0x0900_0003,
            attack_anim_mode: 0x0900_0027,
            inner_radius: 0.0,
            outer_radius: 12.0,
            yaw_completion_tolerance_radians: f32::from_bits(0x3c8e_fa35),
            turn_rate_radians_per_second: std::f32::consts::TAU,
            vertical_limit: 2.0,
            reentry_delay_ticks: 60,
            priority: ROBOTS_TURN_THEN_ATTACK_PRIORITY,
        }
    }

    /// EQ03 Spider builder `0x00464F70`, the only AttackGroup child.
    /// Native uses Attack37 with no turn animation/rate, a 5..15 unit annulus,
    /// fifteen-degree yaw completion tolerance and a 600-tick re-entry delay.
    pub const fn eq03_spider() -> Self {
        Self {
            turn_anim_mode: 0,
            attack_anim_mode: 0x0900_0037,
            inner_radius: 5.0,
            outer_radius: 15.0,
            yaw_completion_tolerance_radians: f32::from_bits(0x3e86_0a92),
            turn_rate_radians_per_second: 0.0,
            vertical_limit: 1000.0,
            reentry_delay_ticks: 600,
            priority: ROBOTS_TURN_THEN_ATTACK_PRIORITY,
        }
    }

    /// EB13 KnightBot builder `0x00465770` special AttackGroup child.
    pub const fn eb13_knightbot() -> Self {
        Self {
            turn_anim_mode: 0x0900_0003,
            attack_anim_mode: 0x0900_0027,
            inner_radius: 3.0,
            outer_radius: 12.0,
            yaw_completion_tolerance_radians: f32::from_bits(0x3c8e_fa35),
            turn_rate_radians_per_second: std::f32::consts::TAU,
            vertical_limit: 1000.0,
            reentry_delay_ticks: 60,
            priority: ROBOTS_TURN_THEN_ATTACK_PRIORITY,
        }
    }

    /// EB15 Launcher builder `0x00465FF0` special AttackGroup child.
    /// Native passes AnimMode 0 to the turn request and switches to Attack25 once
    /// the post-turn yaw error reaches one degree.
    pub const fn eb15_launcher() -> Self {
        Self {
            turn_anim_mode: 0,
            attack_anim_mode: 0x0900_0025,
            inner_radius: 2.0,
            outer_radius: 15.0,
            yaw_completion_tolerance_radians: f32::from_bits(0x3c8e_fa35),
            turn_rate_radians_per_second: std::f32::consts::TAU,
            vertical_limit: 1000.0,
            reentry_delay_ticks: 120,
            priority: ROBOTS_TURN_THEN_ATTACK_PRIORITY,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub enum RobotsTurnThenAttackPhase {
    #[default]
    Inactive,
    Turning,
    Attacking,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsTurnThenAttackRuntimeState {
    pub active: bool,
    pub completion_latch: bool,
    pub active_ticks: u32,
    pub inactive_ticks: u32,
    pub phase: RobotsTurnThenAttackPhase,
}

impl Default for RobotsTurnThenAttackRuntimeState {
    fn default() -> Self {
        Self {
            active: false,
            completion_latch: false,
            active_ticks: 0,
            inactive_ticks: ROBOTS_TURN_THEN_ATTACK_INITIAL_INACTIVE_TICKS,
            phase: RobotsTurnThenAttackPhase::Inactive,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsTurnThenAttackGateInput {
    pub owner_position_xyz: [f32; 3],
    pub target_position_xyz: [f32; 3],
    pub target_visible: bool,
    pub class_attack_allowed: bool,
}

pub fn turn_then_attack_geometry_gate(
    config: RobotsTurnThenAttackConfig,
    input: RobotsTurnThenAttackGateInput,
) -> bool {
    if !input.target_visible
        || !input.class_attack_allowed
        || !input
            .owner_position_xyz
            .iter()
            .all(|value| value.is_finite())
        || !input
            .target_position_xyz
            .iter()
            .all(|value| value.is_finite())
    {
        return false;
    }

    let dx = input.target_position_xyz[0] - input.owner_position_xyz[0];
    let dy = input.target_position_xyz[1] - input.owner_position_xyz[1];
    let dz = input.target_position_xyz[2] - input.owner_position_xyz[2];
    let distance_squared = dx * dx + dy * dy + dz * dz;
    distance_squared >= config.inner_radius * config.inner_radius
        && distance_squared <= config.outer_radius * config.outer_radius
        && dy.abs() <= config.vertical_limit
}

/// Native common selector wrapper `0x00456F20` plus this family gate `0x0044FEA0`.
/// Active, incomplete nodes are sticky; SetupIdle completion exposes the geometry
/// gate again only when the common re-entry contract permits it.
pub fn turn_then_attack_priority(
    state: RobotsTurnThenAttackRuntimeState,
    config: RobotsTurnThenAttackConfig,
    input: RobotsTurnThenAttackGateInput,
) -> u8 {
    if !state.active {
        if state.inactive_ticks < config.reentry_delay_ticks {
            return 1;
        }
    } else {
        if state.completion_latch && config.reentry_delay_ticks != 0 {
            return 1;
        }
        if !state.completion_latch {
            return config.priority;
        }
    }

    if turn_then_attack_geometry_gate(config, input) {
        config.priority
    } else {
        1
    }
}

pub fn enter_turn_then_attack(state: &mut RobotsTurnThenAttackRuntimeState) {
    state.active = true;
    state.completion_latch = false;
    state.active_ticks = 0;
    state.phase = RobotsTurnThenAttackPhase::Turning;
}

pub fn leave_turn_then_attack(state: &mut RobotsTurnThenAttackRuntimeState) {
    state.active = false;
    state.inactive_ticks = 0;
    state.phase = RobotsTurnThenAttackPhase::Inactive;
}

pub fn tick_turn_then_attack(state: &mut RobotsTurnThenAttackRuntimeState) {
    if state.active {
        state.active_ticks = state.active_ticks.saturating_add(1);
    } else {
        state.inactive_ticks = state.inactive_ticks.saturating_add(1);
    }
}

/// SetupIdle handler `0x00450230`: only the attack AnimMode completes the node.
pub fn turn_then_attack_setup_idle(state: &mut RobotsTurnThenAttackRuntimeState) {
    if state.phase == RobotsTurnThenAttackPhase::Attacking {
        state.completion_latch = true;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsTurnThenAttackStepInput {
    pub owner_position_xyz: [f32; 3],
    pub owner_yaw_radians: f32,
    pub target_position_xyz: [f32; 3],
    pub handler_flags_628: u32,
    pub runtime_rate_scale: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsTurnThenAttackStep {
    pub phase: RobotsTurnThenAttackPhase,
    pub requested_anim_mode: Option<u32>,
    pub direct_turn: Option<RobotsAiDirectTurnStep>,
    pub request_attack_action: bool,
}

/// Execute `0x00450170`. Turning delegates to common AI vslot +0x118; only after
/// the post-turn yaw error reaches the configured tolerance does the next update
/// enter the attack AnimMode and invoke owner vslot +0x130.
pub fn step_turn_then_attack(
    state: &mut RobotsTurnThenAttackRuntimeState,
    config: RobotsTurnThenAttackConfig,
    input: RobotsTurnThenAttackStepInput,
) -> RobotsTurnThenAttackStep {
    if !state.active {
        return RobotsTurnThenAttackStep {
            phase: state.phase,
            requested_anim_mode: None,
            direct_turn: None,
            request_attack_action: false,
        };
    }

    if state.phase == RobotsTurnThenAttackPhase::Turning {
        let dx = input.target_position_xyz[0] - input.owner_position_xyz[0];
        let dz = input.target_position_xyz[2] - input.owner_position_xyz[2];
        let target_yaw = if dx == 0.0 && dz == 0.0 {
            input.owner_yaw_radians
        } else {
            dx.atan2(dz)
        };
        let yaw_error = shortest_yaw_delta(input.owner_yaw_radians, target_yaw);
        let direct_turn = step_ai_direct_turn_request(
            input.owner_yaw_radians,
            yaw_error,
            config.turn_rate_radians_per_second,
            input.handler_flags_628,
            config.turn_anim_mode,
            input.runtime_rate_scale,
        );
        let post_turn_error = shortest_yaw_delta(direct_turn.owner_yaw_radians, target_yaw);
        if post_turn_error.abs() <= config.yaw_completion_tolerance_radians {
            state.phase = RobotsTurnThenAttackPhase::Attacking;
        }
        return RobotsTurnThenAttackStep {
            phase: state.phase,
            requested_anim_mode: Some(direct_turn.requested_anim_mode),
            direct_turn: Some(direct_turn),
            request_attack_action: false,
        };
    }

    RobotsTurnThenAttackStep {
        phase: state.phase,
        requested_anim_mode: Some(config.attack_anim_mode),
        direct_turn: None,
        request_attack_action: state.phase == RobotsTurnThenAttackPhase::Attacking,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gate_input(distance: f32) -> RobotsTurnThenAttackGateInput {
        RobotsTurnThenAttackGateInput {
            owner_position_xyz: [0.0, 0.0, 0.0],
            target_position_xyz: [0.0, 0.0, distance],
            target_visible: true,
            class_attack_allowed: true,
        }
    }

    #[test]
    fn eb13_geometry_is_annulus_without_yaw_gate() {
        let config = RobotsTurnThenAttackConfig::eb13_knightbot();
        let state = RobotsTurnThenAttackRuntimeState::default();
        assert_eq!(
            turn_then_attack_priority(state, config, gate_input(2.99)),
            1
        );
        assert_eq!(
            turn_then_attack_priority(state, config, gate_input(3.0)),
            0x32
        );
        assert_eq!(
            turn_then_attack_priority(state, config, gate_input(12.0)),
            0x32
        );
        assert_eq!(
            turn_then_attack_priority(state, config, gate_input(12.01)),
            1
        );
    }

    #[test]
    fn eb13_turns_before_requesting_attack27() {
        let config = RobotsTurnThenAttackConfig::eb13_knightbot();
        let mut state = RobotsTurnThenAttackRuntimeState::default();
        enter_turn_then_attack(&mut state);
        let first = step_turn_then_attack(
            &mut state,
            config,
            RobotsTurnThenAttackStepInput {
                owner_position_xyz: [0.0, 0.0, 0.0],
                owner_yaw_radians: 0.0,
                target_position_xyz: [10.0, 0.0, 0.0],
                handler_flags_628: 0,
                runtime_rate_scale: 1.0,
            },
        );
        assert_eq!(first.requested_anim_mode, Some(0x0900_0003));
        assert!(first.direct_turn.is_some());
        assert!(!first.request_attack_action);

        state.phase = RobotsTurnThenAttackPhase::Turning;
        let aligned = step_turn_then_attack(
            &mut state,
            config,
            RobotsTurnThenAttackStepInput {
                owner_position_xyz: [0.0, 0.0, 0.0],
                owner_yaw_radians: 0.0,
                target_position_xyz: [0.0, 0.0, 10.0],
                handler_flags_628: 0,
                runtime_rate_scale: 1.0,
            },
        );
        assert_eq!(state.phase, RobotsTurnThenAttackPhase::Attacking);
        assert_eq!(aligned.requested_anim_mode, Some(0x0900_0003));
        let attack = step_turn_then_attack(
            &mut state,
            config,
            RobotsTurnThenAttackStepInput {
                owner_position_xyz: [0.0, 0.0, 0.0],
                owner_yaw_radians: 0.0,
                target_position_xyz: [0.0, 0.0, 10.0],
                handler_flags_628: 0,
                runtime_rate_scale: 1.0,
            },
        );
        assert_eq!(attack.requested_anim_mode, Some(0x0900_0027));
        assert!(attack.request_attack_action);
        turn_then_attack_setup_idle(&mut state);
        assert!(state.completion_latch);
    }
}
