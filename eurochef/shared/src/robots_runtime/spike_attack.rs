use serde::Serialize;

use super::events::event_type;

pub const ROBOTS_SPIKE_ATTACK_PRIORITY: u8 = 0x32;
pub const ROBOTS_SPIKE_ATTACK_ENTRY_RADIUS: f32 = 7.0;
pub const ROBOTS_SPIKE_ATTACK_SUSTAIN_RADIUS: f32 = 10.0;
pub const ROBOTS_SPIKE_ATTACK_SPECIAL_SWITCH_DISTANCE_SQUARED: f32 = 2.0;
pub const ROBOTS_SPIKE_ATTACK_START_ANIM_MODE: u32 = 0x0900_002f;
pub const ROBOTS_SPIKE_ATTACK_EXIT_ANIM_MODE: u32 = 0x0900_0031;
pub const ROBOTS_SPIKE_ATTACK_MAIN_ANIM_MODE: u32 = 0x0900_0030;
pub const ROBOTS_SPIKE_ATTACK_SPECIAL_ANIM_MODE: u32 = 0x0900_00ea;
pub const ROBOTS_SPIKE_ATTACK_SPECIAL_TURN_RATE_RADIANS_PER_SECOND: f32 =
    std::f32::consts::FRAC_PI_2;
pub const ROBOTS_SPIKE_ATTACK_SPECIAL_MAX_YAW_PER_TICK: f32 =
    ROBOTS_SPIKE_ATTACK_SPECIAL_TURN_RATE_RADIANS_PER_SECOND / 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsSpikeAttackConfig {
    pub start_anim_mode: u32,
    pub exit_anim_mode: u32,
    pub main_anim_mode: u32,
    pub special_anim_mode: u32,
    pub entry_radius: f32,
    pub sustain_radius: f32,
    pub special_switch_distance_squared: f32,
    pub priority: u8,
}

impl RobotsSpikeAttackConfig {
    /// SpikeBot builder `0x0045BC50` -> setup `0x004573B0`.
    pub const fn spikebot() -> Self {
        Self {
            start_anim_mode: ROBOTS_SPIKE_ATTACK_START_ANIM_MODE,
            exit_anim_mode: ROBOTS_SPIKE_ATTACK_EXIT_ANIM_MODE,
            main_anim_mode: ROBOTS_SPIKE_ATTACK_MAIN_ANIM_MODE,
            special_anim_mode: ROBOTS_SPIKE_ATTACK_SPECIAL_ANIM_MODE,
            entry_radius: ROBOTS_SPIKE_ATTACK_ENTRY_RADIUS,
            sustain_radius: ROBOTS_SPIKE_ATTACK_SUSTAIN_RADIUS,
            special_switch_distance_squared: ROBOTS_SPIKE_ATTACK_SPECIAL_SWITCH_DISTANCE_SQUARED,
            priority: ROBOTS_SPIKE_ATTACK_PRIORITY,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub enum RobotsSpikeAttackPhase {
    #[default]
    Start,
    Exit,
    Main,
    Special,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RobotsSpikeAttackRuntimeState {
    pub active: bool,
    pub phase: RobotsSpikeAttackPhase,
    pub completed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsSpikeAttackPriorityInput {
    pub target_present: bool,
    /// Engine-neutral projection of Handler+0x5FA target-awareness/visibility.
    pub target_visible: bool,
    pub target_distance_squared: f32,
}

pub fn spike_attack_priority(
    config: RobotsSpikeAttackConfig,
    state: RobotsSpikeAttackRuntimeState,
    input: RobotsSpikeAttackPriorityInput,
) -> u8 {
    if state.active && !state.completed {
        return config.priority;
    }
    if input.target_present
        && input.target_visible
        && input.target_distance_squared < config.entry_radius * config.entry_radius
    {
        config.priority
    } else {
        1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsSpikeAttackStepInput {
    pub target_present: bool,
    pub target_visible: bool,
    pub target_distance_squared: f32,
    /// Result of native `0x004578F0 -> 0x0046F050` on the current Monster NavMesh.
    pub nav_direct_reachable: bool,
    /// Exact effective `0x004552B0` result. The host owns game/player state and
    /// Player reaction-window data; the attack reducer only consumes the boolean.
    pub player_reaction_active: bool,
    /// Handler vslot +0x13C relative target yaw.
    pub relative_target_yaw_radians: f32,
    /// Handler+0x45C, written by common `SET_SCRIPT_VALUE` event handling.
    pub owner_turn_blocked: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct RobotsSpikeAttackStep {
    pub requested_anim_mode: Option<u32>,
    /// Direct yaw delta written by native state4. This is deliberately separate
    /// from TurnOnSpot locomotion because mode EA remains active while yaw changes.
    pub owner_yaw_delta_radians: f32,
}

pub fn enter_spike_attack(state: &mut RobotsSpikeAttackRuntimeState) {
    state.active = true;
    state.phase = RobotsSpikeAttackPhase::Start;
    state.completed = false;
}

pub fn leave_spike_attack(state: &mut RobotsSpikeAttackRuntimeState) {
    state.active = false;
    state.completed = false;
}

fn spike_special_hold(config: RobotsSpikeAttackConfig, input: RobotsSpikeAttackStepInput) -> bool {
    input.nav_direct_reachable
        && (!input.player_reaction_active
            || input.target_distance_squared > config.special_switch_distance_squared)
}

pub fn step_spike_attack(
    config: RobotsSpikeAttackConfig,
    state: &mut RobotsSpikeAttackRuntimeState,
    input: RobotsSpikeAttackStepInput,
) -> RobotsSpikeAttackStep {
    if matches!(
        state.phase,
        RobotsSpikeAttackPhase::Main | RobotsSpikeAttackPhase::Special
    ) {
        let sustain = input.target_present
            && input.target_visible
            && input.target_distance_squared < config.sustain_radius * config.sustain_radius;
        if !sustain {
            state.phase = RobotsSpikeAttackPhase::Exit;
        } else if spike_special_hold(config, input) {
            state.phase = RobotsSpikeAttackPhase::Special;
        } else {
            state.phase = RobotsSpikeAttackPhase::Main;
        }
    }

    match state.phase {
        RobotsSpikeAttackPhase::Start => RobotsSpikeAttackStep {
            requested_anim_mode: Some(config.start_anim_mode),
            owner_yaw_delta_radians: 0.0,
        },
        RobotsSpikeAttackPhase::Exit => RobotsSpikeAttackStep {
            requested_anim_mode: Some(config.exit_anim_mode),
            owner_yaw_delta_radians: 0.0,
        },
        RobotsSpikeAttackPhase::Main => RobotsSpikeAttackStep {
            requested_anim_mode: Some(config.main_anim_mode),
            owner_yaw_delta_radians: 0.0,
        },
        RobotsSpikeAttackPhase::Special => RobotsSpikeAttackStep {
            requested_anim_mode: Some(config.special_anim_mode),
            owner_yaw_delta_radians: if input.owner_turn_blocked {
                0.0
            } else {
                input.relative_target_yaw_radians.clamp(
                    -ROBOTS_SPIKE_ATTACK_SPECIAL_MAX_YAW_PER_TICK,
                    ROBOTS_SPIKE_ATTACK_SPECIAL_MAX_YAW_PER_TICK,
                )
            },
        },
    }
}

pub fn apply_spike_attack_event(state: &mut RobotsSpikeAttackRuntimeState, event: u32) -> bool {
    if event != event_type::SETUP_IDLE {
        return false;
    }
    match state.phase {
        RobotsSpikeAttackPhase::Start => state.phase = RobotsSpikeAttackPhase::Main,
        RobotsSpikeAttackPhase::Exit => state.completed = true,
        RobotsSpikeAttackPhase::Main | RobotsSpikeAttackPhase::Special => {}
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(distance_squared: f32) -> RobotsSpikeAttackStepInput {
        RobotsSpikeAttackStepInput {
            target_present: true,
            target_visible: true,
            target_distance_squared: distance_squared,
            nav_direct_reachable: true,
            player_reaction_active: true,
            relative_target_yaw_radians: 0.0,
            owner_turn_blocked: false,
        }
    }

    #[test]
    fn builder_constants_and_entry_priority_match_native_spike_node() {
        let config = RobotsSpikeAttackConfig::spikebot();
        assert_eq!(config.start_anim_mode, 0x0900_002f);
        assert_eq!(config.exit_anim_mode, 0x0900_0031);
        assert_eq!(config.main_anim_mode, 0x0900_0030);
        assert_eq!(config.special_anim_mode, 0x0900_00ea);
        assert_eq!(config.entry_radius, 7.0);
        assert_eq!(config.sustain_radius, 10.0);
        assert_eq!(config.special_switch_distance_squared, 2.0);
        assert_eq!(config.priority, 0x32);

        let state = RobotsSpikeAttackRuntimeState::default();
        assert_eq!(
            spike_attack_priority(
                config,
                state,
                RobotsSpikeAttackPriorityInput {
                    target_present: true,
                    target_visible: true,
                    target_distance_squared: 48.99,
                }
            ),
            0x32
        );
        assert_eq!(
            spike_attack_priority(
                config,
                state,
                RobotsSpikeAttackPriorityInput {
                    target_present: true,
                    target_visible: true,
                    target_distance_squared: 49.0,
                }
            ),
            1
        );
    }

    #[test]
    fn setup_idle_and_sustain_transitions_match_native_states() {
        let config = RobotsSpikeAttackConfig::spikebot();
        let mut state = RobotsSpikeAttackRuntimeState::default();
        enter_spike_attack(&mut state);
        assert_eq!(
            step_spike_attack(config, &mut state, input(4.0)).requested_anim_mode,
            Some(0x0900_002f)
        );
        assert!(apply_spike_attack_event(&mut state, event_type::SETUP_IDLE));
        assert_eq!(state.phase, RobotsSpikeAttackPhase::Main);

        let mut main = input(4.0);
        main.nav_direct_reachable = false;
        assert_eq!(
            step_spike_attack(config, &mut state, main).requested_anim_mode,
            Some(0x0900_0030)
        );

        let mut lost = input(100.0);
        lost.nav_direct_reachable = false;
        assert_eq!(
            step_spike_attack(config, &mut state, lost).requested_anim_mode,
            Some(0x0900_0031)
        );
        assert!(apply_spike_attack_event(&mut state, event_type::SETUP_IDLE));
        assert!(state.completed);
    }

    #[test]
    fn special_branch_and_turn_clamp_match_native_state4() {
        let config = RobotsSpikeAttackConfig::spikebot();
        let mut state = RobotsSpikeAttackRuntimeState {
            active: true,
            phase: RobotsSpikeAttackPhase::Main,
            completed: false,
        };
        let mut special = input(3.0);
        special.relative_target_yaw_radians = 1.0;
        let step = step_spike_attack(config, &mut state, special);
        assert_eq!(state.phase, RobotsSpikeAttackPhase::Special);
        assert_eq!(step.requested_anim_mode, Some(0x0900_00ea));
        assert!((step.owner_yaw_delta_radians - std::f32::consts::PI / 120.0).abs() < 1.0e-7);

        let mut near_reacting = input(2.0);
        near_reacting.relative_target_yaw_radians = -1.0;
        let step = step_spike_attack(config, &mut state, near_reacting);
        assert_eq!(state.phase, RobotsSpikeAttackPhase::Main);
        assert_eq!(step.requested_anim_mode, Some(0x0900_0030));

        state.phase = RobotsSpikeAttackPhase::Main;
        let mut no_reaction = input(1.0);
        no_reaction.player_reaction_active = false;
        no_reaction.owner_turn_blocked = true;
        let step = step_spike_attack(config, &mut state, no_reaction);
        assert_eq!(state.phase, RobotsSpikeAttackPhase::Special);
        assert_eq!(step.owner_yaw_delta_radians, 0.0);
    }
}
