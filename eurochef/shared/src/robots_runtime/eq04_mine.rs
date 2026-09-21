use serde::Serialize;

use super::{
    ai_character::RobotsAiPatrolConfig, ai_pursue::RobotsAiPursueConfig,
    locomotion::RobotsAiTurnRateInput,
};

pub const ROBOTS_EQ04_MINE_EXPLOSION_UID: u32 = 0x5200_0006;
pub const ROBOTS_EQ04_MINE_SELF_DESTRUCT_RADIUS: f32 = 1.0;
pub const ROBOTS_EQ04_MINE_MOVE_SPEED: f32 = 8.0;

/// EQ04 builder `0x00463390 -> AI_Patrol::Setup 0x0046BCF0`.
pub const fn eq04_mine_patrol_config() -> RobotsAiPatrolConfig {
    RobotsAiPatrolConfig {
        base_yaw_radians: 0.0,
        interval_seconds: 3,
        target_locomotion_scalar: 0.0,
        turn_rate: RobotsAiTurnRateInput::Default,
    }
}

/// EQ04 builder `0x00463390 -> AI_Pursue::Setup 0x0046D8C0`.
pub const fn eq04_mine_pursue_config() -> RobotsAiPursueConfig {
    RobotsAiPursueConfig::new(15.0, 1.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RobotsEq04MineBehaviorWinner {
    Patrol,
    Fall,
    Pursue,
}

/// Native `0x00457140` selector in EQ04 builder insertion order:
/// Patrol -> Fall -> Pursue. Native replaces the winner only on a strict
/// priority increase, therefore the first node keeps equal-priority ties.
pub fn eq04_mine_behavior_winner(
    patrol_priority: u8,
    fall_priority: u8,
    pursue_priority: u8,
) -> Option<RobotsEq04MineBehaviorWinner> {
    let candidates = [
        (patrol_priority, RobotsEq04MineBehaviorWinner::Patrol),
        (fall_priority, RobotsEq04MineBehaviorWinner::Fall),
        (pursue_priority, RobotsEq04MineBehaviorWinner::Pursue),
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

/// EQ04 class update `0x00463590`: after common AI update, self-destruct when
/// the gameplay Player is strictly inside radius 1.0 in XZ. Y is ignored.
pub fn eq04_mine_self_destruct_ready(
    owner_position_xyz: [f32; 3],
    player_position_xyz: Option<[f32; 3]>,
) -> bool {
    let Some(player) = player_position_xyz else {
        return false;
    };
    let dx = player[0] - owner_position_xyz[0];
    let dz = player[2] - owner_position_xyz[2];
    dx * dx + dz * dz
        < ROBOTS_EQ04_MINE_SELF_DESTRUCT_RADIUS * ROBOTS_EQ04_MINE_SELF_DESTRUCT_RADIUS
}

/// EQ04 Handler slot25 (+0x64) override `0x00463540`. The callback is invoked
/// symmetrically for handler peers by the contact dispatcher. EQ04 reacts only
/// when the peer owns an XItem whose `+0x264` category is exactly 0 and this
/// Mine's XItem `+0x10 bit0x10` is clear. The native side effect is the same
/// explosion UID followed by Handler +0x12C deferred destruction.
pub fn eq04_mine_peer_contact_self_destruct_ready(
    peer_owner_category: Option<u32>,
    owner_xitem_flag_10_bit_10: bool,
) -> bool {
    peer_owner_category == Some(0) && !owner_xitem_flag_10_bit_10
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eq04_builder_configs_match_native_constants() {
        let patrol = eq04_mine_patrol_config();
        assert_eq!(patrol.base_yaw_radians.to_bits(), 0.0f32.to_bits());
        assert_eq!(patrol.interval_seconds, 3);
        assert_eq!(patrol.target_locomotion_scalar.to_bits(), 0.0f32.to_bits());
        assert_eq!(patrol.turn_rate, RobotsAiTurnRateInput::Default);

        let pursue = eq04_mine_pursue_config();
        assert_eq!(pursue.radius.to_bits(), 15.0f32.to_bits());
        assert_eq!(pursue.locomotion_scalar.to_bits(), 1.0f32.to_bits());
        assert_eq!(pursue.priority, 0x1e);
        assert_eq!(ROBOTS_EQ04_MINE_EXPLOSION_UID, 0x5200_0006);
        assert_eq!(ROBOTS_EQ04_MINE_MOVE_SPEED, 8.0);
    }

    #[test]
    fn eq04_selector_preserves_builder_order_on_ties() {
        assert_eq!(
            eq04_mine_behavior_winner(10, 10, 10),
            Some(RobotsEq04MineBehaviorWinner::Patrol)
        );
        assert_eq!(
            eq04_mine_behavior_winner(10, 0x82, 0x1e),
            Some(RobotsEq04MineBehaviorWinner::Fall)
        );
        assert_eq!(
            eq04_mine_behavior_winner(10, 1, 0x1e),
            Some(RobotsEq04MineBehaviorWinner::Pursue)
        );
    }

    #[test]
    fn eq04_self_destruct_is_strict_xz_radius_and_ignores_y() {
        let owner = [0.0, 10.0, 0.0];
        assert!(eq04_mine_self_destruct_ready(
            owner,
            Some([0.5, -999.0, 0.5])
        ));
        assert!(!eq04_mine_self_destruct_ready(
            owner,
            Some([1.0, 10.0, 0.0])
        ));
        assert!(!eq04_mine_self_destruct_ready(owner, None));
    }

    #[test]
    fn eq04_peer_contact_requires_category_zero_and_live_owner_flag() {
        assert!(eq04_mine_peer_contact_self_destruct_ready(Some(0), false));
        assert!(!eq04_mine_peer_contact_self_destruct_ready(Some(1), false));
        assert!(!eq04_mine_peer_contact_self_destruct_ready(None, false));
        assert!(!eq04_mine_peer_contact_self_destruct_ready(Some(0), true));
    }
}
