use serde::Serialize;

use super::{
    events::event_type,
    locomotion::{shortest_yaw_delta, RobotsAiTurnRateInput},
};

pub const ROBOTS_SHUNT_ATTACK_PRIORITY: u8 = 0x32;
pub const ROBOTS_SHUNT_ATTACK_OUTER_RADIUS: f32 = 25.0;
pub const ROBOTS_SHUNT_ATTACK_FACE_EPSILON_RADIANS: f32 = f32::from_bits(0x3c8e_fa35);
pub const ROBOTS_SHUNT_ATTACK_TURN_RATE_RADIANS_PER_SECOND: f32 = f32::from_bits(0x3fc9_0fdb);
pub const ROBOTS_SHUNT_ATTACK_TURN_ANIM_MODE: u32 = 0x0900_0028;
pub const ROBOTS_SHUNT_ATTACK_PRIMARY_ANIM_MODE: u32 = 0x0900_0027;
pub const ROBOTS_SHUNT_ATTACK_BEAM_ANIM_MODE: u32 = 0x0900_0038;
pub const ROBOTS_SHUNT_ATTACK_HANDLER_ANIM_MODE: u32 = 0x0900_0039;
pub const ROBOTS_SHUNT_ATTACK_FINISH_ANIM_MODE: u32 = 0x0900_0037;
pub const ROBOTS_SHUNT_ATTACK_BEAM_TARGET_YAW_LIMIT_RADIANS: f32 = f32::from_bits(0x3fc9_0fdb);
pub const ROBOTS_SHUNT_ATTACK_COLLISION_CORRECTION_EPSILON_SQUARED: f32 =
    f32::from_bits(0x3a83_126f);
pub const ROBOTS_SHUNT_ATTACK_COLLISION_CORRECTION_YAW_LIMIT_RADIANS: f32 =
    f32::from_bits(0x4016_cbe4);

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsShuntAttackConfig {
    pub outer_radius: f32,
    pub face_epsilon_radians: f32,
}

impl RobotsShuntAttackConfig {
    /// ShuntBotBoss builder `0x0045CAF0` -> `0x00450260` setup.
    pub const fn boss() -> Self {
        Self {
            outer_radius: 25.0,
            face_epsilon_radians: f32::from_bits(0x3c8e_fa35),
        }
    }

    /// Normal ShuntBot builder `0x0045C620` -> the same `0x00450260` node.
    pub const fn normal() -> Self {
        Self {
            outer_radius: 10.0,
            face_epsilon_radians: f32::from_bits(0x3db2_b8c2),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RobotsShuntAttackPhase {
    FaceTarget,
    Primary,
    Beam,
    HandlerBranch,
    Finish,
}

impl Default for RobotsShuntAttackPhase {
    fn default() -> Self {
        Self::FaceTarget
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RobotsShuntAttackRuntimeState {
    pub active: bool,
    pub phase: RobotsShuntAttackPhase,
    pub completed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsShuntAttackPriorityInput {
    /// Result of inherited/common target, visibility, Handler attack-permission
    /// and process attack-claim gates. Those stay in the common AI host.
    pub common_attack_gate_ready: bool,
    pub target_distance_squared: f32,
}

pub fn shunt_attack_priority(
    state: RobotsShuntAttackRuntimeState,
    input: RobotsShuntAttackPriorityInput,
) -> u8 {
    shunt_attack_priority_configured(RobotsShuntAttackConfig::boss(), state, input)
}

pub fn shunt_attack_priority_configured(
    config: RobotsShuntAttackConfig,
    state: RobotsShuntAttackRuntimeState,
    input: RobotsShuntAttackPriorityInput,
) -> u8 {
    if state.active && !state.completed {
        return ROBOTS_SHUNT_ATTACK_PRIORITY;
    }
    if input.common_attack_gate_ready
        && input.target_distance_squared <= config.outer_radius.powi(2)
    {
        ROBOTS_SHUNT_ATTACK_PRIORITY
    } else {
        1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsShuntAttackStepInput {
    /// Native Handler-relative target yaw returned by vslot +0x13C.
    pub relative_target_yaw_radians: f32,
    /// Native Handler vslot +0x130 result. `0x00455DD0` services the registered
    /// hit-query records and returns true when one reports a hit.
    pub hit_query_completed: bool,
    /// `0x00454DE0` target lookup result used by native state3.
    pub target_present: bool,
    /// `0x004558D0` target/visibility gate used by native state3.
    pub target_visible: bool,
    /// Common monster collision-correction accumulator Handler+0x5C0/+0x5C8.
    /// This is not CharacterPhysics velocity. Engine hosts should feed the
    /// correction produced by their character collision adapter.
    pub collision_correction_xz: [f32; 2],
    /// Handler owner/facing yaw corresponding to native Handler+0xE4.
    pub owner_yaw_radians: f32,
}

pub fn shunt_attack_beam_phase_hold(input: RobotsShuntAttackStepInput) -> bool {
    if !input.target_present
        || !input.target_visible
        || input.relative_target_yaw_radians.abs()
            >= ROBOTS_SHUNT_ATTACK_BEAM_TARGET_YAW_LIMIT_RADIANS
    {
        return false;
    }

    let [x, z] = input.collision_correction_xz;
    let correction_squared = x * x + z * z;
    if correction_squared <= ROBOTS_SHUNT_ATTACK_COLLISION_CORRECTION_EPSILON_SQUARED {
        return true;
    }

    let correction_yaw = z.atan2(x);
    shortest_yaw_delta(input.owner_yaw_radians, correction_yaw).abs()
        <= ROBOTS_SHUNT_ATTACK_COLLISION_CORRECTION_YAW_LIMIT_RADIANS
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct RobotsShuntAttackStep {
    pub requested_anim_mode: Option<u32>,
    pub service_turn_toward_target: bool,
    pub turn_rate: Option<RobotsAiTurnRateInput>,
}

pub fn enter_shunt_attack(state: &mut RobotsShuntAttackRuntimeState) {
    state.active = true;
    state.phase = RobotsShuntAttackPhase::FaceTarget;
    state.completed = false;
}

pub fn leave_shunt_attack(state: &mut RobotsShuntAttackRuntimeState) {
    state.active = false;
    state.completed = false;
}

pub fn step_shunt_attack(
    state: &mut RobotsShuntAttackRuntimeState,
    input: RobotsShuntAttackStepInput,
) -> RobotsShuntAttackStep {
    step_shunt_attack_configured(RobotsShuntAttackConfig::boss(), state, input)
}

pub fn step_shunt_attack_configured(
    config: RobotsShuntAttackConfig,
    state: &mut RobotsShuntAttackRuntimeState,
    input: RobotsShuntAttackStepInput,
) -> RobotsShuntAttackStep {
    match state.phase {
        RobotsShuntAttackPhase::FaceTarget => {
            if input.relative_target_yaw_radians.abs() < config.face_epsilon_radians {
                state.phase = RobotsShuntAttackPhase::Primary;
            }
        }
        RobotsShuntAttackPhase::Beam => {
            if input.hit_query_completed {
                state.phase = RobotsShuntAttackPhase::HandlerBranch;
            } else if !shunt_attack_beam_phase_hold(input) {
                state.phase = RobotsShuntAttackPhase::Finish;
            }
        }
        RobotsShuntAttackPhase::Primary
        | RobotsShuntAttackPhase::HandlerBranch
        | RobotsShuntAttackPhase::Finish => {}
    }

    match state.phase {
        RobotsShuntAttackPhase::FaceTarget => RobotsShuntAttackStep {
            requested_anim_mode: None,
            service_turn_toward_target: true,
            turn_rate: Some(RobotsAiTurnRateInput::Explicit(
                ROBOTS_SHUNT_ATTACK_TURN_RATE_RADIANS_PER_SECOND,
            )),
        },
        RobotsShuntAttackPhase::Primary => RobotsShuntAttackStep {
            requested_anim_mode: Some(ROBOTS_SHUNT_ATTACK_PRIMARY_ANIM_MODE),
            ..Default::default()
        },
        RobotsShuntAttackPhase::Beam => RobotsShuntAttackStep {
            requested_anim_mode: Some(ROBOTS_SHUNT_ATTACK_BEAM_ANIM_MODE),
            ..Default::default()
        },
        RobotsShuntAttackPhase::HandlerBranch => RobotsShuntAttackStep {
            requested_anim_mode: Some(ROBOTS_SHUNT_ATTACK_HANDLER_ANIM_MODE),
            ..Default::default()
        },
        RobotsShuntAttackPhase::Finish => RobotsShuntAttackStep {
            requested_anim_mode: Some(ROBOTS_SHUNT_ATTACK_FINISH_ANIM_MODE),
            ..Default::default()
        },
    }
}

/// Shunt overrides only SetupIdle state transitions. CreateProjectile,
/// AttachBeam, DetachBeam and ParticleHitcheck are inherited common AI_Attack
/// event semantics and are classified by `generic_attack::classify_attack_script_event`.
pub fn apply_shunt_attack_event(state: &mut RobotsShuntAttackRuntimeState, event: u32) -> bool {
    if event != event_type::SETUP_IDLE {
        return false;
    }
    if state.phase == RobotsShuntAttackPhase::Primary {
        state.phase = RobotsShuntAttackPhase::Beam;
    } else if matches!(
        state.phase,
        RobotsShuntAttackPhase::HandlerBranch | RobotsShuntAttackPhase::Finish
    ) {
        state.completed = true;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step_input(relative_target_yaw_radians: f32) -> RobotsShuntAttackStepInput {
        RobotsShuntAttackStepInput {
            relative_target_yaw_radians,
            hit_query_completed: false,
            target_present: true,
            target_visible: true,
            collision_correction_xz: [0.0, 0.0],
            owner_yaw_radians: 0.0,
        }
    }

    #[test]
    fn priority_and_builder_constants_match_native_shunt_node() {
        assert_eq!(ROBOTS_SHUNT_ATTACK_PRIORITY, 0x32);
        assert_eq!(ROBOTS_SHUNT_ATTACK_OUTER_RADIUS, 25.0);
        assert_eq!(
            ROBOTS_SHUNT_ATTACK_FACE_EPSILON_RADIANS.to_bits(),
            0x3c8e_fa35
        );
        assert_eq!(ROBOTS_SHUNT_ATTACK_TURN_ANIM_MODE, 0x0900_0028);
        assert_eq!(ROBOTS_SHUNT_ATTACK_PRIMARY_ANIM_MODE, 0x0900_0027);
        assert_eq!(ROBOTS_SHUNT_ATTACK_BEAM_ANIM_MODE, 0x0900_0038);
        assert_eq!(ROBOTS_SHUNT_ATTACK_HANDLER_ANIM_MODE, 0x0900_0039);
        assert_eq!(ROBOTS_SHUNT_ATTACK_FINISH_ANIM_MODE, 0x0900_0037);
        let idle = RobotsShuntAttackRuntimeState::default();
        assert_eq!(
            shunt_attack_priority(
                idle,
                RobotsShuntAttackPriorityInput {
                    common_attack_gate_ready: true,
                    target_distance_squared: 625.0,
                },
            ),
            0x32
        );
        assert_eq!(
            shunt_attack_priority(
                idle,
                RobotsShuntAttackPriorityInput {
                    common_attack_gate_ready: true,
                    target_distance_squared: 625.01,
                },
            ),
            1
        );
        let mut active = idle;
        enter_shunt_attack(&mut active);
        assert_eq!(
            shunt_attack_priority(
                active,
                RobotsShuntAttackPriorityInput {
                    common_attack_gate_ready: false,
                    target_distance_squared: f32::INFINITY,
                },
            ),
            ROBOTS_SHUNT_ATTACK_PRIORITY
        );
    }

    #[test]
    fn normal_shunt_builder_reuses_same_node_with_ten_unit_five_degree_gate() {
        let config = RobotsShuntAttackConfig::normal();
        assert_eq!(config.outer_radius, 10.0);
        assert_eq!(config.face_epsilon_radians.to_bits(), 0x3db2_b8c2);
        assert_eq!(
            shunt_attack_priority_configured(
                config,
                RobotsShuntAttackRuntimeState::default(),
                RobotsShuntAttackPriorityInput {
                    common_attack_gate_ready: true,
                    target_distance_squared: 100.0,
                },
            ),
            ROBOTS_SHUNT_ATTACK_PRIORITY
        );
        assert_eq!(
            shunt_attack_priority_configured(
                config,
                RobotsShuntAttackRuntimeState::default(),
                RobotsShuntAttackPriorityInput {
                    common_attack_gate_ready: true,
                    target_distance_squared: 100.01,
                },
            ),
            1
        );

        let mut state = RobotsShuntAttackRuntimeState::default();
        enter_shunt_attack(&mut state);
        let still_turning = step_shunt_attack_configured(config, &mut state, step_input(0.1));
        assert!(still_turning.service_turn_toward_target);
        assert_eq!(state.phase, RobotsShuntAttackPhase::FaceTarget);
        let primary = step_shunt_attack_configured(config, &mut state, step_input(0.05));
        assert_eq!(state.phase, RobotsShuntAttackPhase::Primary);
        assert_eq!(
            primary.requested_anim_mode,
            Some(ROBOTS_SHUNT_ATTACK_PRIMARY_ANIM_MODE)
        );
    }

    #[test]
    fn setup_idle_drives_primary_beam_and_completion_branches() {
        let mut state = RobotsShuntAttackRuntimeState::default();
        enter_shunt_attack(&mut state);
        let turn = step_shunt_attack(&mut state, step_input(0.1));
        assert!(turn.service_turn_toward_target);
        assert_eq!(state.phase, RobotsShuntAttackPhase::FaceTarget);

        let primary = step_shunt_attack(&mut state, step_input(0.0));
        assert_eq!(state.phase, RobotsShuntAttackPhase::Primary);
        assert_eq!(primary.requested_anim_mode, Some(0x0900_0027));
        apply_shunt_attack_event(&mut state, event_type::SETUP_IDLE);
        assert_eq!(state.phase, RobotsShuntAttackPhase::Beam);

        let mut hit = step_input(0.0);
        hit.hit_query_completed = true;
        let handler = step_shunt_attack(&mut state, hit);
        assert_eq!(handler.requested_anim_mode, Some(0x0900_0039));
        assert_eq!(state.phase, RobotsShuntAttackPhase::HandlerBranch);
        apply_shunt_attack_event(&mut state, event_type::SETUP_IDLE);
        assert!(state.completed);
    }

    #[test]
    fn beam_phase_uses_target_visibility_and_collision_correction_contract() {
        let base = step_input(0.0);
        assert!(shunt_attack_beam_phase_hold(base));

        let mut lost_target = base;
        lost_target.target_present = false;
        assert!(!shunt_attack_beam_phase_hold(lost_target));

        let mut hidden = base;
        hidden.target_visible = false;
        assert!(!shunt_attack_beam_phase_hold(hidden));

        let mut outside_target_yaw = base;
        outside_target_yaw.relative_target_yaw_radians =
            ROBOTS_SHUNT_ATTACK_BEAM_TARGET_YAW_LIMIT_RADIANS;
        assert!(!shunt_attack_beam_phase_hold(outside_target_yaw));

        let mut aligned_correction = base;
        aligned_correction.collision_correction_xz = [1.0, 0.0];
        aligned_correction.owner_yaw_radians = 0.0;
        assert!(shunt_attack_beam_phase_hold(aligned_correction));

        let mut opposing_correction = base;
        opposing_correction.collision_correction_xz = [-1.0, 0.0];
        opposing_correction.owner_yaw_radians = 0.0;
        assert!(!shunt_attack_beam_phase_hold(opposing_correction));
    }

    #[test]
    fn non_setup_idle_events_are_left_to_common_attack_host() {
        let mut state = RobotsShuntAttackRuntimeState::default();
        assert!(!apply_shunt_attack_event(
            &mut state,
            event_type::ATTACH_BEAM
        ));
        assert!(!apply_shunt_attack_event(
            &mut state,
            event_type::CREATE_PROJECTILE,
        ));
        assert!(!apply_shunt_attack_event(
            &mut state,
            event_type::PARTICLE_HITCHECK,
        ));
    }
}
