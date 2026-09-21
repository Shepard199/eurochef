use serde::Serialize;

use super::locomotion::RobotsAiTurnRateInput;

pub const ROBOTS_STALK_NAVMESH_PRIORITY: u8 = 0x28;
pub const ROBOTS_STALK_NAVMESH_TURN_ENTER_RADIANS: f32 = f32::from_bits(0x3f06_0a92);
pub const ROBOTS_STALK_NAVMESH_TURN_EXIT_RADIANS: f32 = f32::from_bits(0x3c8e_fa35);
pub const ROBOTS_STALK_NAVMESH_TURN_RATE_RADIANS_PER_SECOND: f32 = f32::from_bits(0x3fc9_0fdb);
pub const ROBOTS_STALK_NAVMESH_TIMER_EPSILON: f32 = f32::from_bits(0x3a83_126f);
pub const ROBOTS_STALK_NAVMESH_DELAY_JITTER_SCALE: f32 = f32::from_bits(0x3c23_d70a);
pub const ROBOTS_STALK_NAVMESH_MIN_DELAY_SECONDS: f32 = 1.0;
pub const ROBOTS_STALK_NAVMESH_IDLE_ANIM_MODE: u32 = 0x0900_0004;
pub const ROBOTS_STALK_NAVMESH_FIXED_STEP_SECONDS: f32 = 1.0 / 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsStalkNavMeshConfig {
    pub enter_distance: f32,
    pub retain_distance: f32,
    pub optional_anim_mode: Option<u32>,
    pub animation_delay_seconds: f32,
}

impl RobotsStalkNavMeshConfig {
    /// Monster_2Rockets builder `0x0045B700`.
    pub const fn monster_2rockets() -> Self {
        Self {
            enter_distance: 5.0,
            retain_distance: 7.5,
            optional_anim_mode: None,
            animation_delay_seconds: 5.0,
        }
    }

    pub const fn ew10_minion() -> Self {
        Self {
            enter_distance: 5.0,
            retain_distance: 7.5,
            optional_anim_mode: None,
            animation_delay_seconds: 0.0,
        }
    }

    pub const fn eb14_minion() -> Self {
        Self {
            enter_distance: 5.0,
            retain_distance: 7.5,
            optional_anim_mode: Some(0x0900_002d),
            animation_delay_seconds: 5.0,
        }
    }

    /// EB15 Launcher builder `0x00465FF0`.
    pub const fn eb15_launcher() -> Self {
        Self {
            enter_distance: 7.0,
            retain_distance: 10.0,
            optional_anim_mode: Some(0x0900_002d),
            animation_delay_seconds: 5.0,
        }
    }

    /// Normal ShuntBot builder `0x0045C620`.
    pub const fn shuntbot() -> Self {
        Self {
            enter_distance: 6.0,
            retain_distance: 8.0,
            optional_anim_mode: None,
            animation_delay_seconds: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub enum RobotsStalkNavMeshPhase {
    #[default]
    Facing,
    Turning,
    PendingAnimation,
    Animation,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct RobotsStalkNavMeshRuntimeState {
    pub active: bool,
    pub initialized: bool,
    pub phase: RobotsStalkNavMeshPhase,
    pub animation_delay_seconds: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsStalkNavMeshPriorityInput {
    pub target_available: bool,
    pub navigation_context_ready: bool,
    pub target_distance_squared: f32,
    /// Native `0x00455DA0 && Handler+0x158`: while this process-wide attack claim
    /// is active and the concrete class allows attacking, Stalk deliberately drops
    /// to the selector fallback priority instead of competing with attack nodes.
    pub attack_claim_suppresses_stalk: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsStalkNavMeshStep {
    pub requested_anim_mode: u32,
    pub steering_target_yaw_radians: Option<f32>,
    pub target_locomotion_scalar: f32,
    pub turn_rate: RobotsAiTurnRateInput,
    /// State 4 also invokes Handler vslot +0x130 after requesting its configured
    /// AnimMode. The host keeps that class seam explicit rather than hiding it in
    /// animation selection.
    pub service_handler_130: bool,
}

pub fn initialize_stalk_navmesh(
    state: &mut RobotsStalkNavMeshRuntimeState,
    config: RobotsStalkNavMeshConfig,
) {
    state.initialized = true;
    state.animation_delay_seconds = config.animation_delay_seconds;
}

pub fn stalk_navmesh_requests_event_throttle(
    state: RobotsStalkNavMeshRuntimeState,
    config: RobotsStalkNavMeshConfig,
) -> bool {
    state.phase == RobotsStalkNavMeshPhase::Facing
        && config.optional_anim_mode.is_some()
        && state.animation_delay_seconds < ROBOTS_STALK_NAVMESH_TIMER_EPSILON
}

pub fn stalk_navmesh_priority(
    state: RobotsStalkNavMeshRuntimeState,
    config: RobotsStalkNavMeshConfig,
    input: RobotsStalkNavMeshPriorityInput,
) -> u8 {
    if !input.target_available || !input.navigation_context_ready {
        return 1;
    }
    if state.active && state.phase == RobotsStalkNavMeshPhase::Animation {
        return ROBOTS_STALK_NAVMESH_PRIORITY;
    }
    if input.attack_claim_suppresses_stalk {
        return 1;
    }

    let limit = if state.active {
        config.retain_distance
    } else {
        config.enter_distance
    };
    let within = if state.active {
        input.target_distance_squared < limit * limit
    } else {
        input.target_distance_squared <= limit * limit
    };
    if within {
        ROBOTS_STALK_NAVMESH_PRIORITY
    } else {
        1
    }
}

pub fn enter_stalk_navmesh(state: &mut RobotsStalkNavMeshRuntimeState) {
    state.active = true;
    state.phase = RobotsStalkNavMeshPhase::Facing;
}

pub fn leave_stalk_navmesh(state: &mut RobotsStalkNavMeshRuntimeState) {
    state.active = false;
}

pub fn tick_stalk_navmesh(state: &mut RobotsStalkNavMeshRuntimeState) {
    if state.initialized {
        state.animation_delay_seconds -= ROBOTS_STALK_NAVMESH_FIXED_STEP_SECONDS;
    }
}

pub fn step_stalk_navmesh(
    state: &mut RobotsStalkNavMeshRuntimeState,
    config: RobotsStalkNavMeshConfig,
    target_yaw_radians: f32,
    relative_target_yaw_radians: f32,
    event_throttle_granted: bool,
) -> RobotsStalkNavMeshStep {
    match state.phase {
        RobotsStalkNavMeshPhase::Facing => {
            if relative_target_yaw_radians.abs() > ROBOTS_STALK_NAVMESH_TURN_ENTER_RADIANS {
                state.phase = RobotsStalkNavMeshPhase::Turning;
            }
            if config.optional_anim_mode.is_some()
                && state.animation_delay_seconds < ROBOTS_STALK_NAVMESH_TIMER_EPSILON
                && event_throttle_granted
            {
                // Native 0x0046AA50 requests process-global throttle id1 through
                // 0x004565B0 before it may enter state3/PendingAnimation.
                state.phase = RobotsStalkNavMeshPhase::PendingAnimation;
            }
        }
        RobotsStalkNavMeshPhase::Turning => {
            if relative_target_yaw_radians.abs() < ROBOTS_STALK_NAVMESH_TURN_EXIT_RADIANS {
                state.phase = RobotsStalkNavMeshPhase::Facing;
            }
        }
        RobotsStalkNavMeshPhase::PendingAnimation => {
            if relative_target_yaw_radians.abs() < ROBOTS_STALK_NAVMESH_TURN_EXIT_RADIANS {
                state.phase = RobotsStalkNavMeshPhase::Animation;
            }
        }
        RobotsStalkNavMeshPhase::Animation => {}
    }

    match state.phase {
        RobotsStalkNavMeshPhase::Facing => RobotsStalkNavMeshStep {
            requested_anim_mode: ROBOTS_STALK_NAVMESH_IDLE_ANIM_MODE,
            steering_target_yaw_radians: None,
            target_locomotion_scalar: 0.0,
            turn_rate: RobotsAiTurnRateInput::Explicit(
                ROBOTS_STALK_NAVMESH_TURN_RATE_RADIANS_PER_SECOND,
            ),
            service_handler_130: false,
        },
        RobotsStalkNavMeshPhase::Turning | RobotsStalkNavMeshPhase::PendingAnimation => {
            RobotsStalkNavMeshStep {
                requested_anim_mode: ROBOTS_STALK_NAVMESH_IDLE_ANIM_MODE,
                steering_target_yaw_radians: Some(target_yaw_radians),
                target_locomotion_scalar: 0.0,
                turn_rate: RobotsAiTurnRateInput::Explicit(
                    ROBOTS_STALK_NAVMESH_TURN_RATE_RADIANS_PER_SECOND,
                ),
                service_handler_130: false,
            }
        }
        RobotsStalkNavMeshPhase::Animation => RobotsStalkNavMeshStep {
            requested_anim_mode: config
                .optional_anim_mode
                .unwrap_or(ROBOTS_STALK_NAVMESH_IDLE_ANIM_MODE),
            steering_target_yaw_radians: None,
            target_locomotion_scalar: 0.0,
            turn_rate: RobotsAiTurnRateInput::Explicit(
                ROBOTS_STALK_NAVMESH_TURN_RATE_RADIANS_PER_SECOND,
            ),
            service_handler_130: true,
        },
    }
}

pub fn complete_stalk_navmesh_animation(
    state: &mut RobotsStalkNavMeshRuntimeState,
    config: RobotsStalkNavMeshConfig,
    draw: u32,
) -> bool {
    if state.phase != RobotsStalkNavMeshPhase::Animation {
        return false;
    }
    state.phase = RobotsStalkNavMeshPhase::Facing;
    let jitter = ((draw % 200) as i32 - 100) as f32 * ROBOTS_STALK_NAVMESH_DELAY_JITTER_SCALE;
    state.animation_delay_seconds =
        (config.animation_delay_seconds + jitter).max(ROBOTS_STALK_NAVMESH_MIN_DELAY_SECONDS);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_uses_native_five_seven_point_five_hysteresis() {
        let config = RobotsStalkNavMeshConfig::ew10_minion();
        let input = |distance: f32| RobotsStalkNavMeshPriorityInput {
            target_available: true,
            navigation_context_ready: true,
            target_distance_squared: distance * distance,
            attack_claim_suppresses_stalk: false,
        };
        let mut state = RobotsStalkNavMeshRuntimeState::default();
        assert_eq!(stalk_navmesh_priority(state, config, input(5.0)), 0x28);
        assert_eq!(stalk_navmesh_priority(state, config, input(5.001)), 1);
        enter_stalk_navmesh(&mut state);
        assert_eq!(stalk_navmesh_priority(state, config, input(7.499)), 0x28);
        assert_eq!(stalk_navmesh_priority(state, config, input(7.5)), 1);
    }

    #[test]
    fn eb14_turns_with_thirty_to_one_degree_hysteresis_then_plays_optional_mode() {
        let config = RobotsStalkNavMeshConfig::eb14_minion();
        let mut state = RobotsStalkNavMeshRuntimeState::default();
        initialize_stalk_navmesh(&mut state, config);
        enter_stalk_navmesh(&mut state);
        let step = step_stalk_navmesh(&mut state, config, 1.0, 0.6, false);
        assert_eq!(state.phase, RobotsStalkNavMeshPhase::Turning);
        assert_eq!(step.steering_target_yaw_radians, Some(1.0));

        state.animation_delay_seconds = 0.0;
        state.phase = RobotsStalkNavMeshPhase::Facing;
        assert!(stalk_navmesh_requests_event_throttle(state, config));
        let step = step_stalk_navmesh(&mut state, config, 1.0, 0.0, true);
        assert_eq!(state.phase, RobotsStalkNavMeshPhase::PendingAnimation);
        assert_eq!(step.steering_target_yaw_radians, Some(1.0));
        let step = step_stalk_navmesh(&mut state, config, 1.0, 0.0, false);
        assert_eq!(state.phase, RobotsStalkNavMeshPhase::Animation);
        assert_eq!(step.requested_anim_mode, 0x0900_002d);
        assert!(step.service_handler_130);
    }

    #[test]
    fn optional_animation_waits_for_process_global_id1_throttle_grant() {
        let config = RobotsStalkNavMeshConfig::eb14_minion();
        let mut state = RobotsStalkNavMeshRuntimeState::default();
        initialize_stalk_navmesh(&mut state, config);
        enter_stalk_navmesh(&mut state);
        state.animation_delay_seconds = 0.0;

        assert!(stalk_navmesh_requests_event_throttle(state, config));
        let denied = step_stalk_navmesh(&mut state, config, 0.0, 0.0, false);
        assert_eq!(state.phase, RobotsStalkNavMeshPhase::Facing);
        assert_eq!(denied.requested_anim_mode, ROBOTS_STALK_NAVMESH_IDLE_ANIM_MODE);

        let granted = step_stalk_navmesh(&mut state, config, 0.0, 0.0, true);
        assert_eq!(state.phase, RobotsStalkNavMeshPhase::PendingAnimation);
        assert_eq!(granted.requested_anim_mode, ROBOTS_STALK_NAVMESH_IDLE_ANIM_MODE);
    }

    #[test]
    fn setup_idle_resets_eb14_timer_with_native_percent_jitter_and_one_second_floor() {
        let config = RobotsStalkNavMeshConfig::eb14_minion();
        let mut state = RobotsStalkNavMeshRuntimeState {
            active: true,
            initialized: true,
            phase: RobotsStalkNavMeshPhase::Animation,
            animation_delay_seconds: 0.0,
        };
        assert!(complete_stalk_navmesh_animation(&mut state, config, 199));
        assert!((state.animation_delay_seconds - 5.99).abs() < 1.0e-6);
        state.phase = RobotsStalkNavMeshPhase::Animation;
        let zero_delay = RobotsStalkNavMeshConfig {
            animation_delay_seconds: 0.0,
            ..config
        };
        assert!(complete_stalk_navmesh_animation(&mut state, zero_delay, 0));
        assert_eq!(state.animation_delay_seconds, 1.0);
    }
}
