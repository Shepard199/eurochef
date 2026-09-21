use serde::Serialize;

use super::locomotion::shortest_yaw_delta;

pub const ROBOTS_DOGBOT_ATTACK_ANIM_MODE: u32 = 0x0900_0027;
pub const ROBOTS_DOGBOT_ATTACK_PRIORITY: u8 = 0x32;
pub const ROBOTS_DOGBOT_ATTACK_INNER_RADIUS: f32 = 0.0;
pub const ROBOTS_DOGBOT_ATTACK_OUTER_RADIUS: f32 = 2.0;
pub const ROBOTS_DOGBOT_ATTACK_YAW_TOLERANCE_RADIANS: f32 = std::f32::consts::PI / 12.0;
pub const ROBOTS_DOGBOT_ATTACK_VERTICAL_LIMIT: f32 = 1000.0;
pub const ROBOTS_DOGBOT_ATTACK_POINT_DATUM: u32 = 0x1000_0009;
pub const ROBOTS_DOGBOT_ATTACK_HITCHECK_SCALAR: f32 = 6.0;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RobotsDogBotAttackRuntimeState {
    /// Generic Attack node active flag after selector enter. Once entered, native
    /// keeps the node alive until the animation/handler channel reports SetupIdle;
    /// the global cooldown is therefore only an entry gate, not an in-flight abort.
    pub active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsDogBotAttackInput {
    pub owner_position_xyz: [f32; 3],
    pub owner_yaw_radians: f32,
    pub target_position_xyz: [f32; 3],
    /// Handler +0x5FA, produced by the common target-visibility service.
    pub target_visible: bool,
    /// DogBot vslot +0x158 (`0x00455150`) after the host resolves global Player /
    /// GameWnd restrictions and the process-wide monster attack cooldown.
    pub class_attack_allowed: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RobotsDogBotAttackStep {
    pub entered: bool,
    pub active: bool,
    pub requested_anim_mode: Option<u32>,
}

/// Exact geometric part of DogBot's generic Attack gate.
///
/// Builder `0x0045B190 -> 0x0044EFB0` configures Attack2 with distance 0..2,
/// target-yaw tolerance 15 degrees, vertical limit 1000 and priority 0x32.
/// `0x0044F080 -> 0x0044F3C0(1)` additionally requires Handler+0x5FA visibility
/// and DogBot vslot +0x158. The latter remains a host input because it reads
/// process/global Player state rather than DogBot-owned fields.
pub fn dogbot_attack_gate(input: RobotsDogBotAttackInput) -> bool {
    if !input.target_visible
        || !input.class_attack_allowed
        || !input.owner_yaw_radians.is_finite()
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
    let inner_squared = ROBOTS_DOGBOT_ATTACK_INNER_RADIUS * ROBOTS_DOGBOT_ATTACK_INNER_RADIUS;
    let outer_squared = ROBOTS_DOGBOT_ATTACK_OUTER_RADIUS * ROBOTS_DOGBOT_ATTACK_OUTER_RADIUS;
    if distance_squared < inner_squared
        || distance_squared > outer_squared
        || dy.abs() > ROBOTS_DOGBOT_ATTACK_VERTICAL_LIMIT
    {
        return false;
    }

    let target_yaw = dx.atan2(dz);
    shortest_yaw_delta(input.owner_yaw_radians, target_yaw).abs()
        < ROBOTS_DOGBOT_ATTACK_YAW_TOLERANCE_RADIANS
}

/// Minimal owned state of the generic Attack node used by DogBot.
/// HitCheck itself is deliberately not duplicated here: real Attack2 AnimScript
/// emits `HT_ScriptEvents_HitCheck`, which belongs to the common AI hit-query host.
pub fn step_dogbot_attack(
    state: &mut RobotsDogBotAttackRuntimeState,
    attack_eligible: bool,
) -> RobotsDogBotAttackStep {
    if state.active {
        return RobotsDogBotAttackStep {
            entered: false,
            active: true,
            requested_anim_mode: Some(ROBOTS_DOGBOT_ATTACK_ANIM_MODE),
        };
    }
    if !attack_eligible {
        return RobotsDogBotAttackStep::default();
    }
    state.active = true;
    RobotsDogBotAttackStep {
        entered: true,
        active: true,
        requested_anim_mode: Some(ROBOTS_DOGBOT_ATTACK_ANIM_MODE),
    }
}

/// Common AI Handler `SetupIdle` invalidates the current animation ownership. For
/// DogBot Attack2 there is no secondary Attack animation, so the next selector pass
/// re-evaluates the entry gate instead of continuing the same attack node.
pub fn dogbot_attack_setup_idle(state: &mut RobotsDogBotAttackRuntimeState) {
    state.active = false;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attack_input(distance: f32) -> RobotsDogBotAttackInput {
        RobotsDogBotAttackInput {
            owner_position_xyz: [0.0, 0.0, 0.0],
            owner_yaw_radians: 0.0,
            target_position_xyz: [0.0, 0.0, distance],
            target_visible: true,
            class_attack_allowed: true,
        }
    }

    #[test]
    fn attack_gate_matches_native_distance_yaw_visibility_and_class_gate() {
        assert!(dogbot_attack_gate(attack_input(2.0)));
        assert!(!dogbot_attack_gate(attack_input(2.001)));

        let mut side = attack_input(1.0);
        side.target_position_xyz = [1.0, 0.0, 0.0];
        assert!(!dogbot_attack_gate(side));

        let mut hidden = attack_input(1.0);
        hidden.target_visible = false;
        assert!(!dogbot_attack_gate(hidden));

        let mut cooldown = attack_input(1.0);
        cooldown.class_attack_allowed = false;
        assert!(!dogbot_attack_gate(cooldown));
    }

    #[test]
    fn active_attack_survives_entry_gate_drop_until_setup_idle() {
        let mut state = RobotsDogBotAttackRuntimeState::default();
        let entered = step_dogbot_attack(&mut state, true);
        assert!(entered.entered && entered.active);
        assert_eq!(
            entered.requested_anim_mode,
            Some(ROBOTS_DOGBOT_ATTACK_ANIM_MODE)
        );

        let continuing = step_dogbot_attack(&mut state, false);
        assert!(!continuing.entered && continuing.active);
        assert_eq!(
            continuing.requested_anim_mode,
            Some(ROBOTS_DOGBOT_ATTACK_ANIM_MODE)
        );

        dogbot_attack_setup_idle(&mut state);
        assert_eq!(
            step_dogbot_attack(&mut state, false),
            RobotsDogBotAttackStep::default()
        );
    }
}
