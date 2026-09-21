use serde::Serialize;

pub const ROBOTS_THREE_PHASE_ATTACK_INITIAL_INACTIVE_TICKS: u32 = 0x1_0000;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsThreePhaseAttackConfig {
    pub inner_radius: f32,
    pub outer_radius: f32,
    /// Node +0x40. The family currently calls common geometry helper with yaw=false,
    /// but the native setup still preserves this field.
    pub yaw_tolerance_radians: f32,
    pub vertical_limit: f32,
    pub start_anim_mode: u32,
    pub active_anim_mode: u32,
    pub end_anim_mode: u32,
    /// Node +0x78. Native -1 means unlimited Active SetupIdle loops.
    pub setup_idle_limit: Option<u32>,
    /// Node +0x74 added to atan2 while target tracking is enabled.
    pub tracking_yaw_offset_radians: f32,
    /// Node +0x80.
    pub tracking_enabled: bool,
    pub priority: u8,
    pub reentry_delay_ticks: u32,
    /// Common behavior byte +0x20 passed as the third argument to `0x00454EE0`.
    /// Zero is the default animation-state slot; EB13 explicitly selects slot9.
    pub animation_state_slot: u8,
    /// Raw node +0x48 flags. Gate bits1/8 have known meanings; EB13 sets bit2,
    /// which does not affect `0x00450B10` eligibility and is retained for the host.
    pub native_flags_48: u32,
}

impl RobotsThreePhaseAttackConfig {
    /// EQ02 MineBot builder `0x0045FEE0`.
    pub const fn eq02_minebot() -> Self {
        Self {
            inner_radius: 5.0,
            outer_radius: 19.0,
            yaw_tolerance_radians: std::f32::consts::FRAC_PI_4,
            vertical_limit: 1000.0,
            start_anim_mode: 0x0900_003E,
            active_anim_mode: 0x0900_003F,
            end_anim_mode: 0x0900_0040,
            setup_idle_limit: Some(3),
            tracking_yaw_offset_radians: std::f32::consts::PI,
            tracking_enabled: true,
            priority: 0x51,
            reentry_delay_ticks: 120,
            animation_state_slot: 0,
            native_flags_48: 0,
        }
    }

    /// EB07 MineBot builder `0x0045ECF0`. It reuses the EQ02 geometry/modes but
    /// allows five Active SetupIdle cycles before entering the end phase.
    pub const fn eb07_minebot() -> Self {
        Self {
            inner_radius: 5.0,
            outer_radius: 19.0,
            yaw_tolerance_radians: std::f32::consts::FRAC_PI_4,
            vertical_limit: 1000.0,
            start_anim_mode: 0x0900_003E,
            active_anim_mode: 0x0900_003F,
            end_anim_mode: 0x0900_0040,
            setup_idle_limit: Some(5),
            tracking_yaw_offset_radians: std::f32::consts::PI,
            tracking_enabled: true,
            priority: 0x32,
            reentry_delay_ticks: 120,
            animation_state_slot: 0,
            native_flags_48: 0,
        }
    }

    /// EB13 KnightBot builder `0x00465770`, secondary behavior container +0x4D8.
    pub const fn eb13_knightbot_secondary() -> Self {
        Self {
            inner_radius: 0.0,
            outer_radius: 8.0,
            yaw_tolerance_radians: std::f32::consts::TAU,
            vertical_limit: 1000.0,
            start_anim_mode: 0x0900_003E,
            active_anim_mode: 0x0900_003F,
            end_anim_mode: 0x0900_0040,
            setup_idle_limit: None,
            tracking_yaw_offset_radians: 0.0,
            tracking_enabled: false,
            priority: 0x32,
            reentry_delay_ticks: 0,
            animation_state_slot: 9,
            native_flags_48: 0x2,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub enum RobotsThreePhaseAttackPhase {
    #[default]
    Inactive,
    Start,
    Active,
    End,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsThreePhaseAttackRuntimeState {
    pub phase: RobotsThreePhaseAttackPhase,
    pub setup_idle_count: u32,
    pub active: bool,
    pub completion_latch: bool,
    pub active_ticks: u32,
    pub inactive_ticks: u32,
}

impl Default for RobotsThreePhaseAttackRuntimeState {
    fn default() -> Self {
        Self {
            phase: RobotsThreePhaseAttackPhase::Inactive,
            setup_idle_count: 0,
            active: false,
            completion_latch: false,
            active_ticks: 0,
            inactive_ticks: ROBOTS_THREE_PHASE_ATTACK_INITIAL_INACTIVE_TICKS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsThreePhaseAttackInput {
    pub owner_position_xyz: [f32; 3],
    pub player_position_xyz: [f32; 3],
    pub target_visible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsThreePhaseAttackStep {
    pub phase: RobotsThreePhaseAttackPhase,
    pub requested_anim_mode: Option<u32>,
    pub tracking_target_yaw_radians: Option<f32>,
    pub request_attack_action: bool,
    pub complete: bool,
}

pub fn three_phase_attack_geometry_gate(
    config: RobotsThreePhaseAttackConfig,
    owner_position_xyz: [f32; 3],
    player_position_xyz: [f32; 3],
    target_visible: bool,
) -> bool {
    if !target_visible
        || !owner_position_xyz.iter().all(|value| value.is_finite())
        || !player_position_xyz.iter().all(|value| value.is_finite())
    {
        return false;
    }
    let dx = owner_position_xyz[0] - player_position_xyz[0];
    let dy = owner_position_xyz[1] - player_position_xyz[1];
    let dz = owner_position_xyz[2] - player_position_xyz[2];
    let distance_squared = dx * dx + dy * dy + dz * dz;
    distance_squared >= config.inner_radius * config.inner_radius
        && distance_squared <= config.outer_radius * config.outer_radius
        && dy.abs() <= config.vertical_limit
}

/// Common Behavior vslot +0x20 timing followed by three-phase priority callback
/// `0x00450B10`. The native direct callback returns zero for a geometry/LOS miss,
/// but one when the class-wide Monster attack gate vetoes an otherwise eligible
/// child; preserve that distinction because AI_AttackGroup compares raw bytes.
pub fn three_phase_attack_priority(
    state: RobotsThreePhaseAttackRuntimeState,
    config: RobotsThreePhaseAttackConfig,
    input: RobotsThreePhaseAttackInput,
    class_attack_allowed: bool,
) -> u8 {
    if !state.active {
        if state.inactive_ticks < config.reentry_delay_ticks {
            return 1;
        }
    } else if state.completion_latch && config.reentry_delay_ticks != 0 {
        return 1;
    } else if !state.completion_latch {
        return config.priority;
    }

    if !three_phase_attack_geometry_gate(
        config,
        input.owner_position_xyz,
        input.player_position_xyz,
        input.target_visible,
    ) {
        return 0;
    }
    if !class_attack_allowed {
        return 1;
    }
    config.priority
}

pub fn enter_three_phase_attack(state: &mut RobotsThreePhaseAttackRuntimeState) {
    state.phase = RobotsThreePhaseAttackPhase::Start;
    state.setup_idle_count = 0;
    state.active = true;
    state.completion_latch = false;
    state.active_ticks = 0;
}

pub fn leave_three_phase_attack(state: &mut RobotsThreePhaseAttackRuntimeState) {
    state.phase = RobotsThreePhaseAttackPhase::Inactive;
    state.active = false;
    state.inactive_ticks = 0;
}

pub fn tick_three_phase_attack(state: &mut RobotsThreePhaseAttackRuntimeState) {
    if state.active {
        state.active_ticks = state.active_ticks.saturating_add(1);
    } else {
        state.inactive_ticks = state.inactive_ticks.saturating_add(1);
    }
}

/// Native Event callback `0x00450C60` for SetupIdle.
pub fn three_phase_attack_setup_idle(state: &mut RobotsThreePhaseAttackRuntimeState) {
    match state.phase {
        RobotsThreePhaseAttackPhase::Start => state.phase = RobotsThreePhaseAttackPhase::Active,
        RobotsThreePhaseAttackPhase::Active => {
            state.setup_idle_count = state.setup_idle_count.saturating_add(1)
        }
        RobotsThreePhaseAttackPhase::End => {
            state.phase = RobotsThreePhaseAttackPhase::Complete;
            state.completion_latch = true;
        }
        RobotsThreePhaseAttackPhase::Inactive | RobotsThreePhaseAttackPhase::Complete => {}
    }
}

/// Configurable execute slice of behavior family `0x004508A0/0x00450B90`.
pub fn step_three_phase_attack(
    state: &mut RobotsThreePhaseAttackRuntimeState,
    config: RobotsThreePhaseAttackConfig,
    input: RobotsThreePhaseAttackInput,
) -> RobotsThreePhaseAttackStep {
    if matches!(
        state.phase,
        RobotsThreePhaseAttackPhase::Inactive | RobotsThreePhaseAttackPhase::Complete
    ) {
        return RobotsThreePhaseAttackStep {
            phase: state.phase,
            requested_anim_mode: None,
            tracking_target_yaw_radians: None,
            request_attack_action: false,
            complete: state.phase == RobotsThreePhaseAttackPhase::Complete,
        };
    }

    let request_attack_action = state.phase == RobotsThreePhaseAttackPhase::Active;
    let tracking_target_yaw_radians = if config.tracking_enabled {
        let dx = input.player_position_xyz[0] - input.owner_position_xyz[0];
        let dz = input.player_position_xyz[2] - input.owner_position_xyz[2];
        (dx.is_finite() && dz.is_finite()).then_some(if dx == 0.0 && dz == 0.0 {
            config.tracking_yaw_offset_radians
        } else {
            dx.atan2(dz) + config.tracking_yaw_offset_radians
        })
    } else {
        None
    };

    let attack_eligible = three_phase_attack_geometry_gate(
        config,
        input.owner_position_xyz,
        input.player_position_xyz,
        input.target_visible,
    );
    let repeat_limit_reached = config
        .setup_idle_limit
        .is_some_and(|limit| state.setup_idle_count >= limit);
    if !attack_eligible || repeat_limit_reached {
        state.phase = RobotsThreePhaseAttackPhase::End;
    }

    let requested_anim_mode = match state.phase {
        RobotsThreePhaseAttackPhase::Start => Some(config.start_anim_mode),
        RobotsThreePhaseAttackPhase::Active => Some(config.active_anim_mode),
        RobotsThreePhaseAttackPhase::End => Some(config.end_anim_mode),
        RobotsThreePhaseAttackPhase::Inactive | RobotsThreePhaseAttackPhase::Complete => None,
    };
    RobotsThreePhaseAttackStep {
        phase: state.phase,
        requested_anim_mode,
        tracking_target_yaw_radians,
        request_attack_action,
        complete: state.phase == RobotsThreePhaseAttackPhase::Complete,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minebot_config_matches_0045fee0() {
        let config = RobotsThreePhaseAttackConfig::eq02_minebot();
        assert_eq!(config.inner_radius, 5.0);
        assert_eq!(config.outer_radius, 19.0);
        assert_eq!(config.setup_idle_limit, Some(3));
        assert_eq!(config.tracking_yaw_offset_radians, std::f32::consts::PI);
        assert!(config.tracking_enabled);
        assert_eq!(config.priority, 0x51);
        assert_eq!(config.reentry_delay_ticks, 120);
        assert_eq!(config.animation_state_slot, 0);
    }

    #[test]
    fn eb13_secondary_config_matches_00465770() {
        let config = RobotsThreePhaseAttackConfig::eb13_knightbot_secondary();
        assert_eq!(config.inner_radius, 0.0);
        assert_eq!(config.outer_radius, 8.0);
        assert_eq!(config.yaw_tolerance_radians, std::f32::consts::TAU);
        assert_eq!(config.setup_idle_limit, None);
        assert!(!config.tracking_enabled);
        assert_eq!(config.priority, 0x32);
        assert_eq!(config.reentry_delay_ticks, 0);
        assert_eq!(config.animation_state_slot, 9);
        assert_eq!(config.native_flags_48, 0x2);
    }

    #[test]
    fn eb07_group_child_config_and_priority_match_native_family() {
        let config = RobotsThreePhaseAttackConfig::eb07_minebot();
        assert_eq!(config.inner_radius, 5.0);
        assert_eq!(config.outer_radius, 19.0);
        assert_eq!(config.setup_idle_limit, Some(5));
        assert_eq!(config.priority, 0x32);
        assert_eq!(config.reentry_delay_ticks, 120);
        assert!(config.tracking_enabled);

        let state = RobotsThreePhaseAttackRuntimeState::default();
        let input = RobotsThreePhaseAttackInput {
            owner_position_xyz: [0.0, 0.0, 0.0],
            player_position_xyz: [0.0, 0.0, 10.0],
            target_visible: true,
        };
        assert_eq!(
            three_phase_attack_priority(state, config, input, true),
            0x32
        );
        assert_eq!(
            three_phase_attack_priority(
                state,
                config,
                RobotsThreePhaseAttackInput {
                    target_visible: false,
                    ..input
                },
                true,
            ),
            0
        );
        assert_eq!(three_phase_attack_priority(state, config, input, false), 1);
    }

    #[test]
    fn setup_idle_drives_three_phase_state_machine() {
        let config = RobotsThreePhaseAttackConfig::eq02_minebot();
        let mut state = RobotsThreePhaseAttackRuntimeState::default();
        enter_three_phase_attack(&mut state);
        assert_eq!(state.phase, RobotsThreePhaseAttackPhase::Start);
        three_phase_attack_setup_idle(&mut state);
        assert_eq!(state.phase, RobotsThreePhaseAttackPhase::Active);
        for _ in 0..3 {
            three_phase_attack_setup_idle(&mut state);
        }
        let step = step_three_phase_attack(
            &mut state,
            config,
            RobotsThreePhaseAttackInput {
                owner_position_xyz: [0.0, 0.0, 0.0],
                player_position_xyz: [0.0, 0.0, 10.0],
                target_visible: true,
            },
        );
        assert_eq!(step.phase, RobotsThreePhaseAttackPhase::End);
        three_phase_attack_setup_idle(&mut state);
        assert_eq!(state.phase, RobotsThreePhaseAttackPhase::Complete);
        assert!(state.completion_latch);
    }
}
