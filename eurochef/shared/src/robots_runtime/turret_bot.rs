use serde::Serialize;

use super::{
    headtrack_attack::RobotsHeadtrackAttackConfig, hit_reaction::RobotsCommonAiHitConfig,
    scrambled_hit::RobotsScrambledHitConfig,
};

pub const ROBOTS_TURRETBOT_PERIODIC_IDLE_BASE_DELAY_TICKS: i32 = 180;
pub const ROBOTS_TURRETBOT_PERIODIC_IDLE_ANIM_MODES: [u32; 2] = [0x0900_0006, 0x0900_0007];
pub const ROBOTS_TURRETBOT_HEAD_BONE_UID: u32 = 0x0E00_0009;

pub const fn turretbot_nonfatal_hit_config() -> RobotsCommonAiHitConfig {
    // Builder 0x0045CE30: AI_Hit setup(2A, 0, 0, 0, 0, 0, 0).
    RobotsCommonAiHitConfig {
        front_anim_mode: 0x0900_002A,
        back_anim_mode: 0,
        left_anim_mode: 0,
        right_anim_mode: 0,
        turn_rate_radians_per_second: 0.0,
    }
}

pub const fn turretbot_headtrack_attack_config() -> RobotsHeadtrackAttackConfig {
    RobotsHeadtrackAttackConfig::turretbot()
}

pub const fn turretbot_scrambled_hit_config() -> RobotsScrambledHitConfig {
    // Builder 0x0045CE30: AI_ScrambledHit setup(0, IdleAttack, 0, 5.0).
    RobotsScrambledHitConfig {
        start_anim_mode: 0,
        loop_anim_mode: 0x0900_0004,
        end_anim_mode: 0,
        loop_ticks: 5,
        priority: 0x55,
        required_query_flag: 0x0000_0008,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RobotsTurretBotBehaviorWinner {
    PeriodicIdle,
    CommonHit,
    AttackGroup,
    MagneticHit,
    ScrambledHit,
}

/// TurretBot builder `0x0045CE30` appends normal BehaviorHost nodes in this
/// exact order. Native selector `0x00457140` replaces the winner only on a
/// strictly greater priority, so equal priorities keep the earlier node.
pub fn turretbot_behavior_winner(
    periodic_idle_priority: u8,
    common_hit_priority: u8,
    attack_group_priority: u8,
    magnetic_hit_priority: u8,
    scrambled_hit_priority: u8,
) -> Option<RobotsTurretBotBehaviorWinner> {
    let mut best = 1;
    let mut winner = None;
    for (priority, candidate) in [
        (
            periodic_idle_priority,
            RobotsTurretBotBehaviorWinner::PeriodicIdle,
        ),
        (
            common_hit_priority,
            RobotsTurretBotBehaviorWinner::CommonHit,
        ),
        (
            attack_group_priority,
            RobotsTurretBotBehaviorWinner::AttackGroup,
        ),
        (
            magnetic_hit_priority,
            RobotsTurretBotBehaviorWinner::MagneticHit,
        ),
        (
            scrambled_hit_priority,
            RobotsTurretBotBehaviorWinner::ScrambledHit,
        ),
    ] {
        if priority > best {
            best = priority;
            winner = Some(candidate);
        }
    }
    winner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turretbot_builder_contract_keeps_headtrack_status_and_strict_host_order() {
        let hit = turretbot_nonfatal_hit_config();
        assert_eq!(hit.front_anim_mode, 0x0900_002A);
        assert_eq!(hit.back_anim_mode, 0);
        assert_eq!(hit.turn_rate_radians_per_second, 0.0);

        let attack = turretbot_headtrack_attack_config();
        assert_eq!(attack.attack_anim_mode, 0x0900_0025);
        assert_eq!(attack.outer_radius, 15.0);
        assert_eq!(attack.vertical_limit, 1.5);

        let scrambled = turretbot_scrambled_hit_config();
        assert_eq!(scrambled.loop_anim_mode, 0x0900_0004);
        assert_eq!(scrambled.loop_ticks, 5);
        assert_eq!(scrambled.priority, 0x55);

        // Earlier node wins exact ties.
        assert_eq!(
            turretbot_behavior_winner(1, 100, 100, 1, 1),
            Some(RobotsTurretBotBehaviorWinner::CommonHit)
        );
        assert_eq!(
            turretbot_behavior_winner(1, 1, 105, 105, 1),
            Some(RobotsTurretBotBehaviorWinner::AttackGroup)
        );
        assert_eq!(
            turretbot_behavior_winner(1, 1, 1, 85, 85),
            Some(RobotsTurretBotBehaviorWinner::MagneticHit)
        );
    }
}
