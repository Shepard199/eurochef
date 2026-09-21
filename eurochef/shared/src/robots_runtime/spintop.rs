use serde::Serialize;

use super::{ai_character::RobotsAiPatrolConfig, locomotion::RobotsAiTurnRateInput};

pub const ROBOTS_SPINTOP_BOUNCE_PROBE_RADIUS: f32 = 0.8;
pub const ROBOTS_SPINTOP_MIN_MOVE_SPEED: f32 = 4.0;
pub const ROBOTS_SPINTOP_MAX_MOVE_SPEED: f32 = 7.0;
pub const ROBOTS_SPINTOP_COMMON_IDLE_PRIORITY: u8 = 2;

pub const fn spintop_patrol_config() -> RobotsAiPatrolConfig {
    RobotsAiPatrolConfig {
        base_yaw_radians: 0.0,
        interval_seconds: 5,
        target_locomotion_scalar: 0.0,
        turn_rate: RobotsAiTurnRateInput::Default,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RobotsSpinTopBehaviorWinner {
    CommonIdle,
    Patrol,
    BounceNavMesh,
    ScrambledHit,
    ElectroHit,
    MagneticHit,
}

/// Common Monster ctor installs Idle04 before class builder 0x0045F960, so
/// strict greater-than tie semantics preserve Idle over later low-priority nodes.
pub fn spintop_behavior_winner(
    patrol_priority: u8,
    bounce_priority: u8,
    scrambled_priority: u8,
    electro_priority: u8,
    magnetic_priority: u8,
) -> Option<RobotsSpinTopBehaviorWinner> {
    let candidates = [
        (
            ROBOTS_SPINTOP_COMMON_IDLE_PRIORITY,
            RobotsSpinTopBehaviorWinner::CommonIdle,
        ),
        (patrol_priority, RobotsSpinTopBehaviorWinner::Patrol),
        (bounce_priority, RobotsSpinTopBehaviorWinner::BounceNavMesh),
        (
            scrambled_priority,
            RobotsSpinTopBehaviorWinner::ScrambledHit,
        ),
        (electro_priority, RobotsSpinTopBehaviorWinner::ElectroHit),
        (magnetic_priority, RobotsSpinTopBehaviorWinner::MagneticHit),
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
    fn builder_constants_match_0045f910_0045f960() {
        let patrol = spintop_patrol_config();
        assert_eq!(patrol.interval_seconds, 5);
        assert_eq!(patrol.target_locomotion_scalar, 0.0);
        assert_eq!(patrol.turn_rate, RobotsAiTurnRateInput::Default);
        assert_eq!(ROBOTS_SPINTOP_BOUNCE_PROBE_RADIUS, 0.8);
        assert_eq!(
            crate::robots_runtime::bounce_navmesh::ROBOTS_BOUNCE_NAVMESH_TURN_RATE_RADIANS_PER_SECOND,
            std::f32::consts::PI * 4.0
        );
        assert_eq!(
            crate::robots_runtime::bounce_navmesh::ROBOTS_BOUNCE_NAVMESH_IMPACT_ANIM_MODE,
            0x0900_0029
        );
        assert_eq!(ROBOTS_SPINTOP_MIN_MOVE_SPEED, 4.0);
        assert_eq!(ROBOTS_SPINTOP_MAX_MOVE_SPEED, 7.0);
    }

    #[test]
    fn selector_preserves_common_idle_and_native_builder_ties() {
        assert_eq!(
            spintop_behavior_winner(1, 1, 1, 1, 1),
            Some(RobotsSpinTopBehaviorWinner::CommonIdle)
        );
        assert_eq!(
            spintop_behavior_winner(10, 0x0b, 1, 1, 1),
            Some(RobotsSpinTopBehaviorWinner::BounceNavMesh)
        );
        assert_eq!(
            spintop_behavior_winner(1, 1, 0x55, 0x55, 0x55),
            Some(RobotsSpinTopBehaviorWinner::ScrambledHit)
        );
    }
}
