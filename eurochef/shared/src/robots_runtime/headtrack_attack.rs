use serde::Serialize;

use super::{
    generic_attack::{
        RobotsGenericAttackRuntimeState, ROBOTS_GENERIC_ATTACK_INITIAL_INACTIVE_TICKS,
    },
    locomotion::shortest_yaw_delta,
};

pub const ROBOTS_HEADTRACK_ATTACK_TRACKING_ANIM_MODE: u32 = 0x0900_0004;
pub const ROBOTS_HEADTRACK_ATTACK_PRIORITY: u8 = 0x32;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsHeadtrackAttackConfig {
    /// Node +0x68, populated by setup `0x0044FD30`.
    pub attack_anim_mode: u32,
    /// Node +0x38/+0x3C are stored squared by setup.
    pub inner_radius: f32,
    pub outer_radius: f32,
    /// Node +0x40. This is checked against Handler+0x5E8 head yaw after
    /// applying the per-tick clamp; it is not a body-yaw selector gate.
    pub yaw_completion_tolerance_radians: f32,
    /// Node +0x64, consumed by the common attack geometry gate.
    pub vertical_limit: f32,
    /// Common behavior base +0x18.
    pub reentry_delay_ticks: u32,
    /// Derived vtable constants +0x40/+0x44 at 0x005E2078/0x005E207C.
    pub headtrack_step_radians: f32,
    pub priority: u8,
}

impl RobotsHeadtrackAttackConfig {
    /// TurretBot builder `0x0045CE30` -> `AI_HeadtrackAttack::Setup 0x0044FD30`.
    pub const fn turretbot() -> Self {
        Self {
            attack_anim_mode: 0x0900_0025,
            inner_radius: 0.0,
            outer_radius: 15.0,
            yaw_completion_tolerance_radians: f32::from_bits(0x3c8e_fa35),
            vertical_limit: 1.5,
            reentry_delay_ticks: 120,
            headtrack_step_radians: f32::from_bits(0x3cd6_7750),
            priority: ROBOTS_HEADTRACK_ATTACK_PRIORITY,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub enum RobotsHeadtrackAttackPhase {
    #[default]
    Inactive,
    Headtracking,
    Attacking,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsHeadtrackAttackRuntimeState {
    /// Reuses only the common behavior-node lifecycle counters/latches.
    pub base: RobotsGenericAttackRuntimeState,
    pub phase: RobotsHeadtrackAttackPhase,
    /// TurretBot Handler+0x5E8. Native applies this to HT_AnimBone_Head; it is
    /// intentionally separate from owner/body yaw.
    pub headtrack_yaw_radians: f32,
}

impl Default for RobotsHeadtrackAttackRuntimeState {
    fn default() -> Self {
        Self {
            base: RobotsGenericAttackRuntimeState {
                inactive_ticks: ROBOTS_GENERIC_ATTACK_INITIAL_INACTIVE_TICKS,
                ..RobotsGenericAttackRuntimeState::default()
            },
            phase: RobotsHeadtrackAttackPhase::Inactive,
            headtrack_yaw_radians: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsHeadtrackAttackGateInput {
    pub owner_position_xyz: [f32; 3],
    pub target_position_xyz: [f32; 3],
    pub target_visible: bool,
    pub class_attack_allowed: bool,
}

pub fn headtrack_attack_geometry_gate(
    config: RobotsHeadtrackAttackConfig,
    input: RobotsHeadtrackAttackGateInput,
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

/// Native base selector `0x00456F20` + `AI_HeadtrackAttack` gate
/// `0x0044FEA0`. Active/incomplete nodes remain sticky at their configured
/// priority; the one-degree value belongs to Execute's head-yaw completion test.
pub fn headtrack_attack_priority(
    state: RobotsHeadtrackAttackRuntimeState,
    config: RobotsHeadtrackAttackConfig,
    input: Option<RobotsHeadtrackAttackGateInput>,
) -> u8 {
    if !state.base.active {
        if state.base.inactive_ticks < config.reentry_delay_ticks {
            return 1;
        }
    } else if state.base.completion_latch {
        if config.reentry_delay_ticks != 0 {
            return 1;
        }
    } else {
        return config.priority;
    }

    if input.is_some_and(|input| headtrack_attack_geometry_gate(config, input)) {
        config.priority
    } else {
        1
    }
}

pub fn enter_headtrack_attack(state: &mut RobotsHeadtrackAttackRuntimeState) {
    state.base.active = true;
    state.base.completion_latch = false;
    state.base.active_ticks = 0;
    state.phase = RobotsHeadtrackAttackPhase::Headtracking;
}

pub fn leave_headtrack_attack(state: &mut RobotsHeadtrackAttackRuntimeState) {
    state.base.active = false;
    state.base.inactive_ticks = 0;
    state.phase = RobotsHeadtrackAttackPhase::Inactive;
}

pub fn tick_headtrack_attack(state: &mut RobotsHeadtrackAttackRuntimeState) {
    if state.base.active {
        state.base.active_ticks = state.base.active_ticks.saturating_add(1);
    } else {
        state.base.inactive_ticks = state.base.inactive_ticks.saturating_add(1);
    }
}

pub fn headtrack_attack_setup_idle(state: &mut RobotsHeadtrackAttackRuntimeState) {
    if state.phase == RobotsHeadtrackAttackPhase::Attacking {
        state.base.completion_latch = true;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsHeadtrackAttackStepInput {
    pub owner_position_xyz: [f32; 3],
    /// Exact `FUN_00454DE0` gameplay target. None is meaningful: native
    /// HeadtrackAttack immediately switches from tracking to Attack25.
    pub target_position_xyz: Option<[f32; 3]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsHeadtrackAttackStep {
    pub phase: RobotsHeadtrackAttackPhase,
    pub requested_anim_mode: Option<u32>,
    pub headtrack_yaw_radians: f32,
    /// Native Execute calls owner vslot +0x130 while Attack25 is active.
    pub service_registered_hit_queries: bool,
}

pub fn step_headtrack_attack(
    state: &mut RobotsHeadtrackAttackRuntimeState,
    config: RobotsHeadtrackAttackConfig,
    input: RobotsHeadtrackAttackStepInput,
) -> RobotsHeadtrackAttackStep {
    if !state.base.active {
        return RobotsHeadtrackAttackStep {
            phase: state.phase,
            requested_anim_mode: None,
            headtrack_yaw_radians: state.headtrack_yaw_radians,
            service_registered_hit_queries: false,
        };
    }

    if state.phase == RobotsHeadtrackAttackPhase::Headtracking {
        match input.target_position_xyz {
            None => {
                state.phase = RobotsHeadtrackAttackPhase::Attacking;
            }
            Some(target)
                if input
                    .owner_position_xyz
                    .iter()
                    .all(|value| value.is_finite())
                    && target.iter().all(|value| value.is_finite())
                    && state.headtrack_yaw_radians.is_finite() =>
            {
                let dx = target[0] - input.owner_position_xyz[0];
                let dz = target[2] - input.owner_position_xyz[2];
                let target_yaw = dx.atan2(dz);
                let delta = shortest_yaw_delta(state.headtrack_yaw_radians, target_yaw).clamp(
                    -config.headtrack_step_radians,
                    config.headtrack_step_radians,
                );
                state.headtrack_yaw_radians += delta;
                if shortest_yaw_delta(state.headtrack_yaw_radians, target_yaw).abs()
                    < config.yaw_completion_tolerance_radians
                {
                    state.phase = RobotsHeadtrackAttackPhase::Attacking;
                }
            }
            Some(_) => {}
        }
    }

    let requested_anim_mode = match state.phase {
        RobotsHeadtrackAttackPhase::Inactive => None,
        RobotsHeadtrackAttackPhase::Headtracking => {
            Some(ROBOTS_HEADTRACK_ATTACK_TRACKING_ANIM_MODE)
        }
        RobotsHeadtrackAttackPhase::Attacking => Some(config.attack_anim_mode),
    };
    RobotsHeadtrackAttackStep {
        phase: state.phase,
        requested_anim_mode,
        headtrack_yaw_radians: state.headtrack_yaw_radians,
        service_registered_hit_queries: state.phase == RobotsHeadtrackAttackPhase::Attacking,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gate(target: [f32; 3]) -> RobotsHeadtrackAttackGateInput {
        RobotsHeadtrackAttackGateInput {
            owner_position_xyz: [0.0; 3],
            target_position_xyz: target,
            target_visible: true,
            class_attack_allowed: true,
        }
    }

    #[test]
    fn turretbot_headtrack_contract_keeps_head_yaw_separate_and_switches_at_one_degree() {
        let config = RobotsHeadtrackAttackConfig::turretbot();
        assert_eq!(config.attack_anim_mode, 0x0900_0025);
        assert_eq!(config.outer_radius, 15.0);
        assert_eq!(config.vertical_limit, 1.5);
        assert_eq!(config.reentry_delay_ticks, 120);
        assert!((config.yaw_completion_tolerance_radians - 1.0_f32.to_radians()).abs() < 1.0e-6);
        assert!((config.headtrack_step_radians - 1.5_f32.to_radians()).abs() < 1.0e-6);

        let mut state = RobotsHeadtrackAttackRuntimeState::default();
        assert_eq!(
            headtrack_attack_priority(state, config, Some(gate([10.0, 0.0, 0.0]))),
            0x32
        );
        enter_headtrack_attack(&mut state);

        let first = step_headtrack_attack(
            &mut state,
            config,
            RobotsHeadtrackAttackStepInput {
                owner_position_xyz: [0.0; 3],
                target_position_xyz: Some([10.0, 0.0, 0.0]),
            },
        );
        assert_eq!(first.phase, RobotsHeadtrackAttackPhase::Headtracking);
        assert_eq!(
            first.requested_anim_mode,
            Some(ROBOTS_HEADTRACK_ATTACK_TRACKING_ANIM_MODE)
        );
        assert!((first.headtrack_yaw_radians - 1.5_f32.to_radians()).abs() < 1.0e-6);

        state.headtrack_yaw_radians = 89.25_f32.to_radians();
        let aligned = step_headtrack_attack(
            &mut state,
            config,
            RobotsHeadtrackAttackStepInput {
                owner_position_xyz: [0.0; 3],
                target_position_xyz: Some([10.0, 0.0, 0.0]),
            },
        );
        assert_eq!(aligned.phase, RobotsHeadtrackAttackPhase::Attacking);
        assert_eq!(aligned.requested_anim_mode, Some(0x0900_0025));
        assert!(aligned.service_registered_hit_queries);

        headtrack_attack_setup_idle(&mut state);
        assert!(state.base.completion_latch);
        leave_headtrack_attack(&mut state);
        for _ in 0..119 {
            tick_headtrack_attack(&mut state);
        }
        assert_eq!(
            headtrack_attack_priority(state, config, Some(gate([0.0, 0.0, 10.0]))),
            1
        );
        tick_headtrack_attack(&mut state);
        assert_eq!(
            headtrack_attack_priority(state, config, Some(gate([0.0, 0.0, 10.0]))),
            0x32
        );
    }

    #[test]
    fn missing_gameplay_target_switches_active_headtrack_to_attack_immediately() {
        let config = RobotsHeadtrackAttackConfig::turretbot();
        let mut state = RobotsHeadtrackAttackRuntimeState::default();
        enter_headtrack_attack(&mut state);
        let step = step_headtrack_attack(
            &mut state,
            config,
            RobotsHeadtrackAttackStepInput {
                owner_position_xyz: [0.0; 3],
                target_position_xyz: None,
            },
        );
        assert_eq!(step.phase, RobotsHeadtrackAttackPhase::Attacking);
        assert_eq!(step.requested_anim_mode, Some(config.attack_anim_mode));
    }
}
