use serde::Serialize;

use super::{
    bounce_navmesh::{
        RobotsBounceNavMeshPhase, RobotsBounceNavMeshRuntimeState,
        ROBOTS_BOUNCE_NAVMESH_COOLDOWN_TICKS, ROBOTS_BOUNCE_NAVMESH_IMPACT_ANIM_MODE,
        ROBOTS_BOUNCE_NAVMESH_INITIAL_YAW_MODULUS, ROBOTS_BOUNCE_NAVMESH_ONE_DEGREE_RADIANS,
        ROBOTS_BOUNCE_NAVMESH_PRIORITY, ROBOTS_BOUNCE_NAVMESH_TURN_RATE_RADIANS_PER_SECOND,
    },
    generic_attack::{RobotsGenericAttackConfig, ROBOTS_GENERIC_ATTACK_PRIORITY},
    scrambled_hit::RobotsScrambledHitConfig,
};

pub type RobotsDodgemBouncePhase = RobotsBounceNavMeshPhase;
pub type RobotsDodgemBounceNavMeshRuntimeState = RobotsBounceNavMeshRuntimeState;

pub const ROBOTS_DODGEM_BOUNCE_PRIORITY: u8 = ROBOTS_BOUNCE_NAVMESH_PRIORITY;
/// EW07 ctor `0x00462330` overrides the common zero speed range with 4..7.
pub const ROBOTS_DODGEM_MIN_MOVE_SPEED: f32 = 4.0;
pub const ROBOTS_DODGEM_MAX_MOVE_SPEED: f32 = 7.0;
pub const ROBOTS_DODGEM_BOUNCE_PROBE_RADIUS: f32 = 1.1;
pub const ROBOTS_DODGEM_TURN_RATE_RADIANS_PER_SECOND: f32 =
    ROBOTS_BOUNCE_NAVMESH_TURN_RATE_RADIANS_PER_SECOND;
pub const ROBOTS_DODGEM_BOUNCE_ANIM_MODE: u32 = ROBOTS_BOUNCE_NAVMESH_IMPACT_ANIM_MODE;
pub const ROBOTS_DODGEM_IDLE_ANIM_MODE: u32 = 0x0900_0087;
pub const ROBOTS_DODGEM_ATTACK_ANIM_MODE: u32 = 0x0900_0027;
pub const ROBOTS_DODGEM_BOUNCE_COOLDOWN_TICKS: u32 = ROBOTS_BOUNCE_NAVMESH_COOLDOWN_TICKS;
pub const ROBOTS_DODGEM_INITIAL_YAW_MODULUS: u32 = ROBOTS_BOUNCE_NAVMESH_INITIAL_YAW_MODULUS;
pub const ROBOTS_DODGEM_ONE_DEGREE_RADIANS: f32 = ROBOTS_BOUNCE_NAVMESH_ONE_DEGREE_RADIANS;
pub const ROBOTS_DODGEM_BOUNCE_CORRECTION_LENGTH_SQUARED_MIN: f32 = f32::from_bits(0x3a83_126f);

pub const fn dodgem_attack_config() -> RobotsGenericAttackConfig {
    RobotsGenericAttackConfig {
        primary_anim_mode: ROBOTS_DODGEM_ATTACK_ANIM_MODE,
        secondary_anim_mode: None,
        inner_radius: 0.0,
        outer_radius: 8.0,
        yaw_tolerance_radians: std::f32::consts::TAU,
        vertical_limit: 1000.0,
        reentry_delay_ticks: 0,
        secondary_repeat_count: 0,
        sticky_while_active: false,
        priority: ROBOTS_GENERIC_ATTACK_PRIORITY,
    }
}

pub const fn dodgem_secondary_scrambled_config() -> RobotsScrambledHitConfig {
    RobotsScrambledHitConfig {
        start_anim_mode: 0x0900_008b,
        loop_anim_mode: 0x0900_008d,
        end_anim_mode: 0x0900_008c,
        loop_ticks: 180,
        priority: 0x55,
        required_query_flag: 0x0000_0008,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RobotsDodgemPrimaryWinner {
    BounceNavMesh,
    ScrambledHit,
    MagneticHit,
}

pub fn dodgem_primary_winner(
    bounce_priority: u8,
    scrambled_priority: u8,
    magnetic_priority: u8,
) -> Option<RobotsDodgemPrimaryWinner> {
    let candidates = [
        (bounce_priority, RobotsDodgemPrimaryWinner::BounceNavMesh),
        (scrambled_priority, RobotsDodgemPrimaryWinner::ScrambledHit),
        (magnetic_priority, RobotsDodgemPrimaryWinner::MagneticHit),
    ];
    let mut best_priority = 1u8;
    let mut selected = None;
    for (priority, winner) in candidates {
        if best_priority < priority {
            best_priority = priority;
            selected = Some(winner);
        }
    }
    selected
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RobotsDodgemSecondaryWinner {
    Idle,
    Attack,
    ScrambledHit,
}

pub fn dodgem_secondary_winner(
    idle_priority: u8,
    attack_priority: u8,
    scrambled_priority: u8,
) -> Option<RobotsDodgemSecondaryWinner> {
    let candidates = [
        (idle_priority, RobotsDodgemSecondaryWinner::Idle),
        (attack_priority, RobotsDodgemSecondaryWinner::Attack),
        (
            scrambled_priority,
            RobotsDodgemSecondaryWinner::ScrambledHit,
        ),
    ];
    let mut best_priority = 1u8;
    let mut selected = None;
    for (priority, winner) in candidates {
        if best_priority < priority {
            best_priority = priority;
            selected = Some(winner);
        }
    }
    selected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_draw_health_scalars_and_bounce_match_native_constants() {
        let mut state = RobotsDodgemBounceNavMeshRuntimeState::from_setup_draw(358);
        assert_eq!(
            state.steering_target_yaw_radians.to_bits(),
            (358.0f32 * ROBOTS_DODGEM_ONE_DEGREE_RADIANS).to_bits()
        );
        assert_eq!(
            RobotsDodgemBounceNavMeshRuntimeState::target_locomotion_scalar(3),
            0.0
        );
        assert_eq!(
            RobotsDodgemBounceNavMeshRuntimeState::target_locomotion_scalar(2),
            0.25
        );
        assert_eq!(
            RobotsDodgemBounceNavMeshRuntimeState::target_locomotion_scalar(1),
            0.5
        );
        assert!(state.apply_boundary_bounce(0.25, 1.0));
        assert_eq!(state.bounce_cooldown_ticks, 15);
        assert!(!state.apply_boundary_bounce(0.25, 2.0));
        for _ in 0..15 {
            state.tick();
        }
        assert_eq!(state.bounce_cooldown_ticks, 0);
    }

    #[test]
    fn raw_query_snapshot_drives_impact_animation_and_setup_idle_clears_hit_latch() {
        let mut state = RobotsDodgemBounceNavMeshRuntimeState::from_setup_draw(0);
        state.enter();
        assert!(state.observe_query_serial(7, 2, Some(1.25)));
        assert_eq!(state.steering_target_yaw_radians, 1.25);
        state.request_impact_animation();
        let mut got_hit = true;
        state.setup_idle(&mut got_hit);
        assert_eq!(state.phase, RobotsDodgemBouncePhase::Moving);
        assert!(!got_hit);
        assert!(!state.observe_query_serial(7, 2, None));
        assert!(!state.observe_query_serial(u16::MAX, 2, None));
        assert!(!state.observe_query_serial(8, 0, None));
    }

    #[test]
    fn builder_configs_and_host_order_match_native() {
        let attack = dodgem_attack_config();
        assert_eq!(attack.primary_anim_mode, 0x0900_0027);
        assert_eq!(attack.inner_radius, 0.0);
        assert_eq!(attack.outer_radius, 8.0);
        assert_eq!(attack.yaw_tolerance_radians, std::f32::consts::TAU);
        assert_eq!(attack.reentry_delay_ticks, 0);
        assert!(!attack.sticky_while_active);

        let scrambled = dodgem_secondary_scrambled_config();
        assert_eq!(
            (
                scrambled.start_anim_mode,
                scrambled.loop_anim_mode,
                scrambled.end_anim_mode,
                scrambled.loop_ticks,
            ),
            (0x0900_008b, 0x0900_008d, 0x0900_008c, 180)
        );

        assert_eq!(
            dodgem_primary_winner(0x0b, 0x55, 0x69),
            Some(RobotsDodgemPrimaryWinner::MagneticHit)
        );
        assert_eq!(
            dodgem_primary_winner(0x55, 0x55, 1),
            Some(RobotsDodgemPrimaryWinner::BounceNavMesh)
        );
        assert_eq!(
            dodgem_secondary_winner(2, 0x32, 0x55),
            Some(RobotsDodgemSecondaryWinner::ScrambledHit)
        );
        assert_eq!(
            dodgem_secondary_winner(0x32, 0x32, 1),
            Some(RobotsDodgemSecondaryWinner::Idle)
        );
    }
}
