use serde::Serialize;

use super::{
    ai_character::RobotsAiPatrolConfig, locomotion::RobotsAiTurnRateInput,
    standard_monster::RobotsStandardMonsterBehaviorWinner,
};

pub const ROBOTS_EF01_PERIODIC_IDLE_BASE_DELAY_TICKS: i32 = 180;
pub const ROBOTS_EF01_PERIODIC_IDLE_ANIM_MODES: [u32; 2] = [0x0900_0006, 0x0900_0007];
pub const ROBOTS_EF01_ELECTRO_HIT_ANIM_MODE: u32 = 0x0900_007d;
pub const ROBOTS_EF01_MAGNETIC_HIT_ANIM_MODE: u32 = 0x0900_00e9;
pub const ROBOTS_EF01_PATROL_NAVMESH2_PARAMETER: f32 = 20.0;
pub const ROBOTS_EF01_PURSUE_NAV_RADIUS: f32 = 1.5;
pub const ROBOTS_EF01_PERMANENT_SOUND_UID: u32 = 0x1af0_0150;
pub const ROBOTS_EF01_PERMANENT_SOUND_NATIVE_PARAMETER: u32 = 100;
pub const ROBOTS_EF01_CREATOR_PATH_UID_BASE: u32 = 0x0b00_0000;

/// EF01 builder `0x00463680 -> AI_Patrol::Setup 0x0046BCF0`.
pub const fn ef01_mine_patrol_config() -> RobotsAiPatrolConfig {
    RobotsAiPatrolConfig {
        base_yaw_radians: 0.0,
        interval_seconds: 3,
        target_locomotion_scalar: 0.0,
        turn_rate: RobotsAiTurnRateInput::Default,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsEf01MineFirstUpdateRoute {
    FollowFlyingPath { path_uid: u32 },
    PursueNavMesh,
}

/// Handler +0x100 `0x00463970`: a non-sentinel 0x0B-family creator path selects
/// `AI_FollowFlyingPath`; otherwise EF01 appends `AI_PursueNavMesh`.
pub const fn ef01_mine_first_update_route(
    creator_path_uid: Option<u32>,
) -> RobotsEf01MineFirstUpdateRoute {
    match creator_path_uid {
        Some(uid)
            if uid != ROBOTS_EF01_CREATOR_PATH_UID_BASE
                && (uid & ROBOTS_EF01_CREATOR_PATH_UID_BASE)
                    == ROBOTS_EF01_CREATOR_PATH_UID_BASE =>
        {
            RobotsEf01MineFirstUpdateRoute::FollowFlyingPath { path_uid: uid }
        }
        _ => RobotsEf01MineFirstUpdateRoute::PursueNavMesh,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsEf01MineFirstUpdateEffects {
    /// Native `0x00455E30(0)` in the FollowFlyingPath branch. `None` means the
    /// PursueNavMesh branch leaves the common Monster ctor state unchanged.
    pub handler_606_mode: Option<bool>,
    /// Direct Physics+0x08 bit0x2 write in `0x00463970`.
    pub physics_object_flag_bit2: bool,
    /// Handler+0x644. Native `+0x170` uses this to suppress ordinary route refresh.
    pub flying_path_latch_644: bool,
}

/// Side effects applied only by EF01 first-update `0x00463970`, after the common
/// first-update and route-node choice. Keep these separate from the node config:
/// they are handler/physics state, not FollowFlyingPath behavior parameters.
pub const fn ef01_mine_first_update_effects(
    route: RobotsEf01MineFirstUpdateRoute,
) -> RobotsEf01MineFirstUpdateEffects {
    match route {
        RobotsEf01MineFirstUpdateRoute::FollowFlyingPath { .. } => {
            RobotsEf01MineFirstUpdateEffects {
                handler_606_mode: Some(false),
                physics_object_flag_bit2: true,
                flying_path_latch_644: true,
            }
        }
        RobotsEf01MineFirstUpdateRoute::PursueNavMesh => RobotsEf01MineFirstUpdateEffects {
            handler_606_mode: None,
            physics_object_flag_bit2: false,
            flying_path_latch_644: false,
        },
    }
}

/// EF01 +0x64 `0x004638E0`. Native dispatches common Monster action
/// `0x004550A0(0,1)` only for a peer whose XItem owner category +0x264 is zero,
/// while this Mine's XItem +0x10 bit0x10 is still clear.
pub const fn ef01_mine_peer_contact_requests_monster_action(
    peer_owner_category: Option<u32>,
    owner_xitem_flag_10_bit_10: bool,
) -> bool {
    matches!(peer_owner_category, Some(0)) && !owner_xitem_flag_10_bit_10
}

/// Native selector `0x00457140` preserves insertion order on equal priority.
/// Builder order is Patrol -> PeriodicIdle -> ElectroHit -> MagneticHit ->
/// PatrolNavMesh2. Handler first-update `0x00463970` appends exactly one route
/// node afterwards: FollowFlyingPath or PursueNavMesh.
#[allow(clippy::too_many_arguments)]
pub fn ef01_mine_behavior_winner(
    patrol_priority: u8,
    periodic_idle_priority: u8,
    electro_hit_priority: u8,
    magnetic_hit_priority: u8,
    patrol_navmesh2_priority: u8,
    follow_flying_path_priority: u8,
    pursue_navmesh_priority: u8,
) -> Option<RobotsStandardMonsterBehaviorWinner> {
    let candidates = [
        (patrol_priority, RobotsStandardMonsterBehaviorWinner::Patrol),
        (
            periodic_idle_priority,
            RobotsStandardMonsterBehaviorWinner::PeriodicIdle,
        ),
        (
            electro_hit_priority,
            RobotsStandardMonsterBehaviorWinner::ElectroHit,
        ),
        (
            magnetic_hit_priority,
            RobotsStandardMonsterBehaviorWinner::MagneticHit,
        ),
        (
            patrol_navmesh2_priority,
            RobotsStandardMonsterBehaviorWinner::PatrolNavMesh2,
        ),
        (
            follow_flying_path_priority,
            RobotsStandardMonsterBehaviorWinner::FollowFlyingPath,
        ),
        (
            pursue_navmesh_priority,
            RobotsStandardMonsterBehaviorWinner::PursueNavMesh,
        ),
    ];
    let mut best_priority = 1u8;
    let mut winner = None;
    for (priority, candidate) in candidates {
        if priority > best_priority {
            best_priority = priority;
            winner = Some(candidate);
        }
    }
    winner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_constants_match_native_ef01_composition() {
        let patrol = ef01_mine_patrol_config();
        assert_eq!(patrol.interval_seconds, 3);
        assert_eq!(patrol.target_locomotion_scalar.to_bits(), 0.0f32.to_bits());
        assert_eq!(ROBOTS_EF01_PERIODIC_IDLE_BASE_DELAY_TICKS, 180);
        assert_eq!(
            ROBOTS_EF01_PERIODIC_IDLE_ANIM_MODES,
            [0x0900_0006, 0x0900_0007]
        );
        assert_eq!(ROBOTS_EF01_ELECTRO_HIT_ANIM_MODE, 0x0900_007d);
        assert_eq!(ROBOTS_EF01_MAGNETIC_HIT_ANIM_MODE, 0x0900_00e9);
        assert_eq!(
            ROBOTS_EF01_PATROL_NAVMESH2_PARAMETER.to_bits(),
            20.0f32.to_bits()
        );
        assert_eq!(ROBOTS_EF01_PURSUE_NAV_RADIUS.to_bits(), 1.5f32.to_bits());
    }

    #[test]
    fn first_update_path_uid_selects_flying_path_only_for_real_0b_family_uid() {
        assert_eq!(
            ef01_mine_first_update_route(Some(0x0b00_0049)),
            RobotsEf01MineFirstUpdateRoute::FollowFlyingPath {
                path_uid: 0x0b00_0049
            }
        );
        assert_eq!(
            ef01_mine_first_update_route(Some(0x0b00_0000)),
            RobotsEf01MineFirstUpdateRoute::PursueNavMesh
        );
        assert_eq!(
            ef01_mine_first_update_route(None),
            RobotsEf01MineFirstUpdateRoute::PursueNavMesh
        );
    }

    #[test]
    fn first_update_flying_path_applies_handler606_physics_bit2_and_644_latch() {
        let flying = ef01_mine_first_update_effects(
            RobotsEf01MineFirstUpdateRoute::FollowFlyingPath { path_uid: 0x0b00_0049 },
        );
        assert_eq!(flying.handler_606_mode, Some(false));
        assert!(flying.physics_object_flag_bit2);
        assert!(flying.flying_path_latch_644);

        let pursue = ef01_mine_first_update_effects(RobotsEf01MineFirstUpdateRoute::PursueNavMesh);
        assert_eq!(pursue.handler_606_mode, None);
        assert!(!pursue.physics_object_flag_bit2);
        assert!(!pursue.flying_path_latch_644);
    }

    #[test]
    fn selector_preserves_ef01_builder_then_first_update_order_on_ties() {
        assert_eq!(
            ef01_mine_behavior_winner(10, 10, 10, 10, 10, 10, 10),
            Some(RobotsStandardMonsterBehaviorWinner::Patrol)
        );
        assert_eq!(
            ef01_mine_behavior_winner(1, 1, 0x20, 0x20, 0x0b, 0x1f, 1),
            Some(RobotsStandardMonsterBehaviorWinner::ElectroHit)
        );
        assert_eq!(
            ef01_mine_behavior_winner(1, 1, 1, 1, 0x0b, 0x1f, 0x1f),
            Some(RobotsStandardMonsterBehaviorWinner::FollowFlyingPath)
        );
    }

    #[test]
    fn peer_contact_gate_matches_class_specific_monster_action_dispatch() {
        assert!(ef01_mine_peer_contact_requests_monster_action(
            Some(0),
            false
        ));
        assert!(!ef01_mine_peer_contact_requests_monster_action(
            Some(1),
            false
        ));
        assert!(!ef01_mine_peer_contact_requests_monster_action(None, false));
        assert!(!ef01_mine_peer_contact_requests_monster_action(
            Some(0),
            true
        ));
    }
}
