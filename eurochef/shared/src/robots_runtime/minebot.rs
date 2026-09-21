use serde::Serialize;

use super::{
    locomotion::{
        step_ai_locomotion_after_steering_prepass, RobotsAiLocomotionInput,
        RobotsAiLocomotionRuntimeState, RobotsAiLocomotionStep, RobotsAiTurnRateInput,
    },
    three_phase_attack::{
        enter_three_phase_attack, step_three_phase_attack, three_phase_attack_geometry_gate,
        three_phase_attack_setup_idle, RobotsThreePhaseAttackConfig, RobotsThreePhaseAttackInput,
        RobotsThreePhaseAttackPhase, RobotsThreePhaseAttackRuntimeState,
        RobotsThreePhaseAttackStep,
    },
};

pub const ROBOTS_MINEBOT_MOVE_ENTER_RADIUS: f32 = 12.0;
pub const ROBOTS_MINEBOT_MOVE_EXIT_RADIUS: f32 = 16.0;
pub const ROBOTS_MINEBOT_ATTACK_INNER_RADIUS: f32 = 5.0;
pub const ROBOTS_MINEBOT_ATTACK_OUTER_RADIUS: f32 = 19.0;
pub const ROBOTS_MINEBOT_ATTACK_VERTICAL_LIMIT: f32 = 1000.0;
pub const ROBOTS_MINEBOT_MOVE_PRIORITY: u8 = 0x50;
pub const ROBOTS_MINEBOT_ATTACK_PRIORITY: u8 = 0x51;
pub const ROBOTS_MINEBOT_ATTACK_START_ANIM_MODE: u32 = 0x0900_003e;
pub const ROBOTS_MINEBOT_ATTACK_ACTIVE_ANIM_MODE: u32 = 0x0900_003f;
pub const ROBOTS_MINEBOT_ATTACK_END_ANIM_MODE: u32 = 0x0900_0040;
pub const ROBOTS_MINEBOT_ATTACK_SETUP_IDLE_LIMIT: u32 = 3;
pub const ROBOTS_MINEBOT_ATTACK_TRACKING_YAW_OFFSET_RADIANS: f32 = std::f32::consts::PI;
/// Native `0x004530B0` receives DAT_005E1FA4 while MineBot Attack tracking is
/// enabled. The constant is pi/60, so owner yaw changes by at most 3 degrees per
/// fixed AI update.
pub const ROBOTS_MINEBOT_ATTACK_TRACKING_MAX_YAW_PER_TICK: f32 = std::f32::consts::PI / 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RobotsMineBotMoveAttackWinner {
    Neither,
    Move,
    Attack,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct RobotsMineBotMoveRuntimeState {
    /// Active flag owned by the native distance behavior node.
    pub move_active: bool,
    /// Shared XItemHandler_AI_Character locomotion state (+0x5D0/+0x5F9).
    pub locomotion: RobotsAiLocomotionRuntimeState,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsMineBotMoveInput {
    pub owner_position_xyz: [f32; 3],
    pub owner_yaw_radians: f32,
    pub player_position_xyz: [f32; 3],
    pub handler_flags_628: u32,
    /// True only when the host current AnimMode is already Move on entry to
    /// native 0x00452800. The locomotion primitive owns the exact scalar reset.
    pub move_mode_active_on_entry: bool,
    pub runtime_rate_scale: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsMineBotMoveStep {
    pub move_active: bool,
    pub target_yaw_radians: Option<f32>,
    pub locomotion: Option<RobotsAiLocomotionStep>,
}

pub fn minebot_move_gate(
    owner_position_xyz: [f32; 3],
    player_position_xyz: [f32; 3],
    was_active: bool,
) -> bool {
    if !owner_position_xyz.iter().all(|value| value.is_finite())
        || !player_position_xyz.iter().all(|value| value.is_finite())
    {
        return false;
    }
    let dx = owner_position_xyz[0] - player_position_xyz[0];
    let dy = owner_position_xyz[1] - player_position_xyz[1];
    let dz = owner_position_xyz[2] - player_position_xyz[2];
    let distance_squared = dx * dx + dy * dy + dz * dz;
    let radius = if was_active {
        ROBOTS_MINEBOT_MOVE_EXIT_RADIUS
    } else {
        ROBOTS_MINEBOT_MOVE_ENTER_RADIUS
    };
    distance_squared < radius * radius
}

/// Engine-neutral geometric/visibility gate used by the MineBot Attack child.
/// Native `0x00450AB0` stores squared 5/19 radii; `0x0044F3C0(false)` adds
/// target visibility and the |dy| <= 1000 limit. Host-specific LOS stays outside.
pub fn minebot_attack_gate(
    owner_position_xyz: [f32; 3],
    player_position_xyz: [f32; 3],
    target_visible: bool,
) -> bool {
    three_phase_attack_geometry_gate(
        RobotsThreePhaseAttackConfig::eq02_minebot(),
        owner_position_xyz,
        player_position_xyz,
        target_visible,
    )
}

/// Native local arbitration between MineBot Move priority 0x50 and Attack 0x51.
/// The rest of the class behavior graph is intentionally outside this reducer.
pub fn minebot_move_attack_winner(
    move_eligible: bool,
    attack_eligible: bool,
) -> RobotsMineBotMoveAttackWinner {
    if attack_eligible && ROBOTS_MINEBOT_ATTACK_PRIORITY > ROBOTS_MINEBOT_MOVE_PRIORITY {
        RobotsMineBotMoveAttackWinner::Attack
    } else if move_eligible {
        RobotsMineBotMoveAttackWinner::Move
    } else {
        RobotsMineBotMoveAttackWinner::Neither
    }
}

pub type RobotsMineBotAttackPhase = RobotsThreePhaseAttackPhase;

pub type RobotsMineBotAttackRuntimeState = RobotsThreePhaseAttackRuntimeState;
pub type RobotsMineBotAttackInput = RobotsThreePhaseAttackInput;
pub type RobotsMineBotAttackStep = RobotsThreePhaseAttackStep;

pub fn enter_minebot_attack(state: &mut RobotsMineBotAttackRuntimeState) {
    enter_three_phase_attack(state);
}

/// Native Attack-node event callback `0x00450C60` for HT_ScriptEvents_SetupIdle.
/// Other events are delegated by the original node and stay outside this reducer.
pub fn minebot_attack_setup_idle(state: &mut RobotsMineBotAttackRuntimeState) {
    three_phase_attack_setup_idle(state);
}

/// Compatibility wrapper for the shipped EQ02 configuration of the reusable
/// `0x004508A0/0x00450B90` three-phase attack family.
pub fn step_minebot_attack(
    state: &mut RobotsMineBotAttackRuntimeState,
    input: RobotsMineBotAttackInput,
) -> RobotsMineBotAttackStep {
    step_three_phase_attack(state, RobotsThreePhaseAttackConfig::eq02_minebot(), input)
}

/// UE-facing slice of the shipped EQ02 MineBot Move node (0x0046A420).
///
/// The behavior node owns only the 12/16 hysteresis and the request to face the
/// gameplay Player while calling the common AI Move vslot with (1.0, -1.0).
/// Attack priority/LOS and other MineBot nodes remain separate selectors.
pub fn step_minebot_move(
    state: &mut RobotsMineBotMoveRuntimeState,
    input: RobotsMineBotMoveInput,
) -> RobotsMineBotMoveStep {
    state.move_active = minebot_move_gate(
        input.owner_position_xyz,
        input.player_position_xyz,
        state.move_active,
    );
    if !state.move_active {
        return RobotsMineBotMoveStep {
            move_active: false,
            target_yaw_radians: None,
            locomotion: None,
        };
    }

    let dx = input.player_position_xyz[0] - input.owner_position_xyz[0];
    let dz = input.player_position_xyz[2] - input.owner_position_xyz[2];
    let target_yaw_radians = if dx.abs() <= f32::EPSILON && dz.abs() <= f32::EPSILON {
        input.owner_yaw_radians
    } else {
        dx.atan2(dz)
    };
    let locomotion = step_ai_locomotion_after_steering_prepass(
        &mut state.locomotion,
        RobotsAiLocomotionInput {
            steering_target_yaw_radians: target_yaw_radians,
            target_locomotion_scalar: 1.0,
            turn_rate: RobotsAiTurnRateInput::Default,
            handler_flags_628: input.handler_flags_628,
            move_mode_active_on_entry: input.move_mode_active_on_entry,
            current_owner_yaw_radians: input.owner_yaw_radians,
            runtime_rate_scale: input.runtime_rate_scale,
        },
    );

    RobotsMineBotMoveStep {
        move_active: true,
        target_yaw_radians: Some(target_yaw_radians),
        locomotion: Some(locomotion),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::robots_runtime::locomotion::{
        ROBOTS_AI_DIRECTIONAL_TURN_MODE_FLAG, ROBOTS_AI_TURN_IN_PLACE_ENABLE_FLAG,
        ROBOTS_ANIM_MODE_MOVE, ROBOTS_ANIM_MODE_TURN_ON_SPOT_R,
    };

    #[test]
    fn move_gate_preserves_native_twelve_sixteen_hysteresis() {
        let owner = [0.0, 0.0, 0.0];
        assert!(minebot_move_gate(owner, [11.99, 0.0, 0.0], false));
        assert!(!minebot_move_gate(owner, [12.0, 0.0, 0.0], false));
        assert!(minebot_move_gate(owner, [15.99, 0.0, 0.0], true));
        assert!(!minebot_move_gate(owner, [16.0, 0.0, 0.0], true));
    }

    #[test]
    fn attack_gate_preserves_native_ring_visibility_and_vertical_limit() {
        let owner = [0.0, 0.0, 0.0];
        assert!(!minebot_attack_gate(owner, [6.0, 0.0, 0.0], false));
        assert!(!minebot_attack_gate(owner, [4.999, 0.0, 0.0], true));
        assert!(minebot_attack_gate(owner, [5.0, 0.0, 0.0], true));
        assert!(minebot_attack_gate(owner, [19.0, 0.0, 0.0], true));
        assert!(!minebot_attack_gate(owner, [19.001, 0.0, 0.0], true));
        assert!(!minebot_attack_gate(owner, [5.0, 1000.001, 0.0], true));
    }

    #[test]
    fn attack_priority_preempts_move_locally() {
        assert_eq!(
            minebot_move_attack_winner(true, true),
            RobotsMineBotMoveAttackWinner::Attack
        );
        assert_eq!(
            minebot_move_attack_winner(true, false),
            RobotsMineBotMoveAttackWinner::Move
        );
        assert_eq!(
            minebot_move_attack_winner(false, false),
            RobotsMineBotMoveAttackWinner::Neither
        );
    }

    #[test]
    fn attack_phase_machine_is_setup_idle_driven() {
        let mut state = RobotsMineBotAttackRuntimeState::default();
        enter_minebot_attack(&mut state);
        let start = step_minebot_attack(
            &mut state,
            RobotsMineBotAttackInput {
                owner_position_xyz: [0.0, 0.0, 0.0],
                player_position_xyz: [10.0, 0.0, 0.0],
                target_visible: true,
            },
        );
        assert_eq!(start.phase, RobotsMineBotAttackPhase::Start);
        assert_eq!(
            start.requested_anim_mode,
            Some(ROBOTS_MINEBOT_ATTACK_START_ANIM_MODE)
        );
        assert!(!start.request_attack_action);

        minebot_attack_setup_idle(&mut state);
        let active = step_minebot_attack(
            &mut state,
            RobotsMineBotAttackInput {
                owner_position_xyz: [0.0, 0.0, 0.0],
                player_position_xyz: [10.0, 0.0, 0.0],
                target_visible: true,
            },
        );
        assert_eq!(active.phase, RobotsMineBotAttackPhase::Active);
        assert_eq!(
            active.requested_anim_mode,
            Some(ROBOTS_MINEBOT_ATTACK_ACTIVE_ANIM_MODE)
        );
        assert!(active.request_attack_action);

        for _ in 0..ROBOTS_MINEBOT_ATTACK_SETUP_IDLE_LIMIT {
            minebot_attack_setup_idle(&mut state);
        }
        let end = step_minebot_attack(
            &mut state,
            RobotsMineBotAttackInput {
                owner_position_xyz: [0.0, 0.0, 0.0],
                player_position_xyz: [10.0, 0.0, 0.0],
                target_visible: true,
            },
        );
        assert_eq!(end.phase, RobotsMineBotAttackPhase::End);
        assert_eq!(
            end.requested_anim_mode,
            Some(ROBOTS_MINEBOT_ATTACK_END_ANIM_MODE)
        );
        assert!(end.request_attack_action);

        minebot_attack_setup_idle(&mut state);
        let complete = step_minebot_attack(
            &mut state,
            RobotsMineBotAttackInput {
                owner_position_xyz: [0.0, 0.0, 0.0],
                player_position_xyz: [10.0, 0.0, 0.0],
                target_visible: true,
            },
        );
        assert_eq!(complete.phase, RobotsMineBotAttackPhase::Complete);
        assert!(complete.complete);
        assert_eq!(complete.requested_anim_mode, None);
    }

    #[test]
    fn attack_loss_of_gate_enters_end_without_inventing_completion() {
        let mut state = RobotsMineBotAttackRuntimeState::default();
        enter_minebot_attack(&mut state);
        minebot_attack_setup_idle(&mut state);
        let step = step_minebot_attack(
            &mut state,
            RobotsMineBotAttackInput {
                owner_position_xyz: [0.0, 0.0, 0.0],
                player_position_xyz: [20.0, 0.0, 0.0],
                target_visible: true,
            },
        );
        assert_eq!(step.phase, RobotsMineBotAttackPhase::End);
        assert_eq!(
            step.requested_anim_mode,
            Some(ROBOTS_MINEBOT_ATTACK_END_ANIM_MODE)
        );
        assert!(step.request_attack_action);
        assert!(!step.complete);
    }

    #[test]
    fn move_step_targets_player_and_requests_common_move() {
        let mut state = RobotsMineBotMoveRuntimeState::default();
        let step = step_minebot_move(
            &mut state,
            RobotsMineBotMoveInput {
                owner_position_xyz: [0.0, 0.0, 0.0],
                owner_yaw_radians: 0.0,
                player_position_xyz: [0.0, 0.0, 10.0],
                handler_flags_628: 0,
                move_mode_active_on_entry: false,
                runtime_rate_scale: 1.0,
            },
        );
        assert!(step.move_active);
        assert_eq!(step.target_yaw_radians, Some(0.0));
        assert_eq!(
            step.locomotion.unwrap().requested_anim_mode,
            ROBOTS_ANIM_MODE_MOVE
        );
        assert!(state.locomotion.locomotion_scalar > 0.0);
    }

    #[test]
    fn directional_turn_flag_keeps_rotation_owned_by_turn_animation() {
        let mut state = RobotsMineBotMoveRuntimeState::default();
        let step = step_minebot_move(
            &mut state,
            RobotsMineBotMoveInput {
                owner_position_xyz: [0.0, 0.0, 0.0],
                owner_yaw_radians: 0.0,
                player_position_xyz: [10.0, 0.0, 0.0],
                handler_flags_628: ROBOTS_AI_TURN_IN_PLACE_ENABLE_FLAG
                    | ROBOTS_AI_DIRECTIONAL_TURN_MODE_FLAG,
                move_mode_active_on_entry: false,
                runtime_rate_scale: 1.0,
            },
        );
        let locomotion = step.locomotion.unwrap();
        assert!(step.move_active);
        assert_eq!(
            locomotion.requested_anim_mode,
            ROBOTS_ANIM_MODE_TURN_ON_SPOT_R
        );
        assert!(!locomotion.direct_owner_yaw_write);
        assert_eq!(locomotion.owner_yaw_radians, 0.0);
        assert!(state.locomotion.turn_in_place_latch);
    }
}
