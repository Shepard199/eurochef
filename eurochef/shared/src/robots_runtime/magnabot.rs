use serde::Serialize;

use super::{
    events::event_type,
    locomotion::{
        ROBOTS_AI_DIRECTIONAL_TURN_MODE_FLAG, ROBOTS_ANIM_MODE_MOVE,
        ROBOTS_ANIM_MODE_TURN_ON_SPOT_L, ROBOTS_ANIM_MODE_TURN_ON_SPOT_R,
        ROBOTS_FIXED_STEP_SECONDS, ROBOTS_RADIANS_TO_DEGREES,
    },
};

pub const ROBOTS_MAGNABOT_TURN_ENTER_DEGREES: f32 = 30.0;
pub const ROBOTS_MAGNABOT_TURN_EXIT_DEGREES: f32 = 5.0;
pub const ROBOTS_MAGNABOT_TRANSITION_COUNTDOWN_TICKS: i32 = 30;
pub const ROBOTS_MAGNABOT_IDLE_ATTACK_ANIM_MODE: u32 = 0x0900_0004;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[repr(u8)]
pub enum RobotsMagnaBotTurnPhase {
    #[default]
    Idle = 0,
    AwaitingSetupIdle = 1,
    /// Native +0x118/+0x114 preserve state2 handling, but exhaustive instruction
    /// scans of this executable found no writer that makes MagnaBot enter state2.
    /// Keep the branch for binary parity without manufacturing an activation path.
    DormantIdleAttack = 2,
    Turning = 3,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RobotsMagnaBotTurnRuntimeState {
    /// Handler+0x640.
    pub phase: RobotsMagnaBotTurnPhase,
    /// Handler+0x644.
    pub countdown_ticks: i32,
    /// Handler+0x648.
    pub change_anim_mode_latch: bool,
    /// Handler+0x64C.
    pub queued_anim_mode: u32,
    /// Handler+0x649. Cleared before common update; +0x10C sets it after Move.
    pub locomotion_serviced_this_update: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RobotsMagnaBotTurnAction {
    None,
    RequestAnimMode(u32),
    BaseTurn {
        yaw_error_radians_bits: u32,
        turn_rate_radians_per_second_bits: u32,
        anim_mode: u32,
    },
}

impl RobotsMagnaBotTurnAction {
    pub fn base_turn(
        yaw_error_radians: f32,
        turn_rate_radians_per_second: f32,
        anim_mode: u32,
    ) -> Self {
        Self::BaseTurn {
            yaw_error_radians_bits: yaw_error_radians.to_bits(),
            turn_rate_radians_per_second_bits: turn_rate_radians_per_second.to_bits(),
            anim_mode,
        }
    }

    pub fn yaw_error_radians(self) -> Option<f32> {
        match self {
            Self::BaseTurn {
                yaw_error_radians_bits,
                ..
            } => Some(f32::from_bits(yaw_error_radians_bits)),
            _ => None,
        }
    }

    pub fn turn_rate_radians_per_second(self) -> Option<f32> {
        match self {
            Self::BaseTurn {
                turn_rate_radians_per_second_bits,
                ..
            } => Some(f32::from_bits(turn_rate_radians_per_second_bits)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsMagnaBotResolvedTurnStep {
    pub owner_yaw_radians: f32,
    pub requested_anim_mode: u32,
    pub direct_owner_yaw_write: bool,
}

impl RobotsMagnaBotTurnRuntimeState {
    /// Native handler +0x34 `0x00464D20` clears +0x649 before invoking common AI update.
    /// Finalize any host-aborted previous update first; normal production flow also
    /// calls `finish_update` at the end of the same fixed step.
    pub fn begin_update(&mut self) {
        self.finish_update();
        self.locomotion_serviced_this_update = false;
    }

    /// Native Magna +0x10C `0x00464D60` calls common Move then sets +0x649=1.
    pub fn record_locomotion_service(&mut self) {
        self.locomotion_serviced_this_update = true;
    }

    /// Tail of `0x00464D20`: a fixed update that never reached +0x10C clears +0x640.
    pub fn finish_update(&mut self) {
        if !self.locomotion_serviced_this_update {
            self.phase = RobotsMagnaBotTurnPhase::Idle;
        }
    }

    /// Magna event ingress `0x00464EA0` local cases. Unknown events are deliberately
    /// absent here because native delegates them to the common AI_Character family.
    pub fn apply_event(&mut self, event_type_value: u32, native_arg0: Option<u32>) {
        match event_type_value {
            event_type::SETUP_IDLE => match self.phase {
                RobotsMagnaBotTurnPhase::AwaitingSetupIdle => {
                    self.phase = RobotsMagnaBotTurnPhase::Turning;
                }
                RobotsMagnaBotTurnPhase::DormantIdleAttack => {
                    self.phase = RobotsMagnaBotTurnPhase::Idle;
                }
                _ => {}
            },
            event_type::CHANGE_ANIM_MODE => {
                self.change_anim_mode_latch = true;
                if let Some(anim_mode) = native_arg0 {
                    self.queued_anim_mode = anim_mode;
                }
            }
            _ => {}
        }
    }

    /// Magna +0x114 `0x00464D80`.
    pub fn turn_gate(&mut self, yaw_error_radians: f32) -> bool {
        let abs_degrees = (yaw_error_radians * ROBOTS_RADIANS_TO_DEGREES).abs();
        match self.phase {
            RobotsMagnaBotTurnPhase::Idle => {
                abs_degrees >= ROBOTS_MAGNABOT_TURN_ENTER_DEGREES && self.queued_anim_mode != 0
            }
            RobotsMagnaBotTurnPhase::AwaitingSetupIdle => true,
            RobotsMagnaBotTurnPhase::DormantIdleAttack => {
                self.countdown_ticks -= 1;
                if self.countdown_ticks < 1 {
                    self.phase = RobotsMagnaBotTurnPhase::Idle;
                    false
                } else {
                    true
                }
            }
            RobotsMagnaBotTurnPhase::Turning => {
                if abs_degrees < ROBOTS_MAGNABOT_TURN_EXIT_DEGREES {
                    self.phase = RobotsMagnaBotTurnPhase::Idle;
                    false
                } else {
                    true
                }
            }
        }
    }

    /// Magna +0x118 `0x00464E00`. `turn_rate_radians_per_second` and `anim_mode`
    /// are the two stack arguments deliberately left by the caller after +0x13C.
    pub fn turn_action(
        &mut self,
        current_anim_mode: u32,
        yaw_error_radians: f32,
        turn_rate_radians_per_second: f32,
        anim_mode: u32,
    ) -> RobotsMagnaBotTurnAction {
        match self.phase {
            RobotsMagnaBotTurnPhase::Idle => {
                self.countdown_ticks = ROBOTS_MAGNABOT_TRANSITION_COUNTDOWN_TICKS;
                self.phase = RobotsMagnaBotTurnPhase::AwaitingSetupIdle;
                self.change_anim_mode_latch = current_anim_mode != ROBOTS_ANIM_MODE_MOVE;
                RobotsMagnaBotTurnAction::None
            }
            RobotsMagnaBotTurnPhase::AwaitingSetupIdle => {
                if self.change_anim_mode_latch {
                    RobotsMagnaBotTurnAction::RequestAnimMode(self.queued_anim_mode)
                } else {
                    RobotsMagnaBotTurnAction::None
                }
            }
            RobotsMagnaBotTurnPhase::DormantIdleAttack => {
                RobotsMagnaBotTurnAction::RequestAnimMode(ROBOTS_MAGNABOT_IDLE_ATTACK_ANIM_MODE)
            }
            RobotsMagnaBotTurnPhase::Turning => RobotsMagnaBotTurnAction::base_turn(
                yaw_error_radians,
                turn_rate_radians_per_second,
                anim_mode,
            ),
        }
    }
}

/// Exact observable part of base +0x118 `0x00452FB0` used by Magna state3.
pub fn resolve_magnabot_base_turn(
    action: RobotsMagnaBotTurnAction,
    handler_flags_628: u32,
    current_owner_yaw_radians: f32,
    runtime_rate_scale: f32,
) -> Option<RobotsMagnaBotResolvedTurnStep> {
    let RobotsMagnaBotTurnAction::BaseTurn {
        yaw_error_radians_bits,
        turn_rate_radians_per_second_bits,
        anim_mode,
    } = action
    else {
        return None;
    };
    let yaw_error_radians = f32::from_bits(yaw_error_radians_bits);
    if handler_flags_628 & ROBOTS_AI_DIRECTIONAL_TURN_MODE_FLAG != 0 {
        return Some(RobotsMagnaBotResolvedTurnStep {
            owner_yaw_radians: current_owner_yaw_radians,
            requested_anim_mode: if yaw_error_radians >= 0.0 {
                ROBOTS_ANIM_MODE_TURN_ON_SPOT_R
            } else {
                ROBOTS_ANIM_MODE_TURN_ON_SPOT_L
            },
            direct_owner_yaw_write: false,
        });
    }
    let max_turn_step = f32::from_bits(turn_rate_radians_per_second_bits)
        * runtime_rate_scale.max(0.0)
        * ROBOTS_FIXED_STEP_SECONDS;
    Some(RobotsMagnaBotResolvedTurnStep {
        owner_yaw_radians: current_owner_yaw_radians
            + yaw_error_radians.clamp(-max_turn_step.abs(), max_turn_step.abs()),
        requested_anim_mode: anim_mode,
        direct_owner_yaw_write: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reachable_transition_matches_native_state0_1_3_path() {
        let mut state = RobotsMagnaBotTurnRuntimeState::default();
        assert!(!state.turn_gate(1.0));
        state.apply_event(event_type::CHANGE_ANIM_MODE, Some(0x0900_0025));
        assert!(state.turn_gate(std::f32::consts::FRAC_PI_2));
        assert_eq!(
            state.turn_action(
                ROBOTS_ANIM_MODE_MOVE,
                std::f32::consts::FRAC_PI_2,
                std::f32::consts::PI,
                0x0900_0028,
            ),
            RobotsMagnaBotTurnAction::None
        );
        assert_eq!(state.phase, RobotsMagnaBotTurnPhase::AwaitingSetupIdle);
        state.apply_event(event_type::SETUP_IDLE, None);
        assert_eq!(state.phase, RobotsMagnaBotTurnPhase::Turning);
        assert!(state.turn_gate((5.01f32).to_radians()));
        assert!(!state.turn_gate((4.99f32).to_radians()));
        assert_eq!(state.phase, RobotsMagnaBotTurnPhase::Idle);
    }

    #[test]
    fn queued_non_move_mode_is_reissued_while_waiting_for_setup_idle() {
        let mut state = RobotsMagnaBotTurnRuntimeState {
            queued_anim_mode: 0x0900_0025,
            ..Default::default()
        };
        let first = state.turn_action(0x0900_0004, 1.0, std::f32::consts::PI, 0x0900_0028);
        assert_eq!(first, RobotsMagnaBotTurnAction::None);
        assert!(state.change_anim_mode_latch);
        let second = state.turn_action(0x0900_0004, 1.0, std::f32::consts::PI, 0x0900_0028);
        assert_eq!(
            second,
            RobotsMagnaBotTurnAction::RequestAnimMode(0x0900_0025)
        );
    }

    #[test]
    fn dormant_state2_semantics_are_preserved_without_an_activation_api() {
        let mut state = RobotsMagnaBotTurnRuntimeState {
            phase: RobotsMagnaBotTurnPhase::DormantIdleAttack,
            countdown_ticks: 2,
            ..Default::default()
        };
        assert!(state.turn_gate(1.0));
        assert_eq!(state.countdown_ticks, 1);
        assert_eq!(
            state.turn_action(0, 1.0, std::f32::consts::PI, 0x0900_0028),
            RobotsMagnaBotTurnAction::RequestAnimMode(ROBOTS_MAGNABOT_IDLE_ATTACK_ANIM_MODE)
        );
        state.apply_event(event_type::SETUP_IDLE, None);
        assert_eq!(state.phase, RobotsMagnaBotTurnPhase::Idle);
    }

    #[test]
    fn update_without_move_service_clears_only_transition_phase() {
        let mut state = RobotsMagnaBotTurnRuntimeState {
            phase: RobotsMagnaBotTurnPhase::Turning,
            queued_anim_mode: 0x0900_0025,
            change_anim_mode_latch: true,
            ..Default::default()
        };
        state.begin_update();
        state.finish_update();
        assert_eq!(state.phase, RobotsMagnaBotTurnPhase::Idle);
        assert_eq!(state.queued_anim_mode, 0x0900_0025);
        assert!(state.change_anim_mode_latch);

        state.phase = RobotsMagnaBotTurnPhase::Turning;
        state.locomotion_serviced_this_update = true;
        state.begin_update();
        state.record_locomotion_service();
        state.finish_update();
        assert_eq!(state.phase, RobotsMagnaBotTurnPhase::Turning);
    }

    #[test]
    fn base_turn_uses_pi_rate_and_directional_modes() {
        let action = RobotsMagnaBotTurnAction::base_turn(1.0, std::f32::consts::PI, 0x0900_0028);
        let direct = resolve_magnabot_base_turn(action, 0, 0.0, 1.0).unwrap();
        assert!(direct.direct_owner_yaw_write);
        assert!((direct.owner_yaw_radians - std::f32::consts::PI / 60.0).abs() < 1.0e-6);
        assert_eq!(direct.requested_anim_mode, 0x0900_0028);

        let directional =
            resolve_magnabot_base_turn(action, ROBOTS_AI_DIRECTIONAL_TURN_MODE_FLAG, 0.0, 1.0)
                .unwrap();
        assert!(!directional.direct_owner_yaw_write);
        assert_eq!(
            directional.requested_anim_mode,
            ROBOTS_ANIM_MODE_TURN_ON_SPOT_R
        );
    }
}
