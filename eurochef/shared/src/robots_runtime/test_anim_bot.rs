use serde::Serialize;

use super::{
    ai_character::RobotsAiPatrolConfig,
    ai_pursue::RobotsAiPursueConfig,
    generic_attack::RobotsGenericAttackConfig,
    hit_reaction::RobotsCommonAiHitConfig,
    locomotion::RobotsAiTurnRateInput,
};

pub const ROBOTS_TEST_ANIM_PATROL_ANIM_MODE: u32 = 0x0900_0003;
pub const ROBOTS_TEST_ANIM_PERIODIC_IDLE_ANIM_MODES: [u32; 5] = [
    0x0900_0006,
    0x0900_0007,
    0x0900_0008,
    0x0900_0009,
    0x0900_000A,
];
pub const ROBOTS_TEST_ANIM_ATTACK_ANIM_MODES: [u32; 5] = [
    0x0900_0025,
    0x0900_0027,
    0x0900_0037,
    0x0900_0038,
    0x0900_0039,
];
pub const ROBOTS_TEST_ANIM_PERIODIC_IDLE_BASE_DELAY_TICKS: i32 = 180;
pub const ROBOTS_TEST_ANIM_ATTACK_REENTRY_TICKS: u32 = 60;
pub const ROBOTS_TEST_ANIM_ATTACK_OUTER_RADIUS: f32 = 2.0;
pub const ROBOTS_TEST_ANIM_ATTACK_YAW_TOLERANCE_RADIANS: f32 =
    f32::from_bits(0x3E86_0A92);
pub const ROBOTS_TEST_ANIM_HANDLER_FLAGS_ANIM_35_36_OR_MASK: u32 = 0x0000_0006;

pub const fn test_anim_patrol_config() -> RobotsAiPatrolConfig {
    RobotsAiPatrolConfig {
        base_yaw_radians: 0.0,
        interval_seconds: 7,
        target_locomotion_scalar: 0.2,
        turn_rate: RobotsAiTurnRateInput::Default,
    }
}

pub const fn test_anim_pursue_config() -> RobotsAiPursueConfig {
    RobotsAiPursueConfig::new(10.0, 1.0)
}

pub const fn test_anim_attack_config(anim_mode: u32) -> RobotsGenericAttackConfig {
    RobotsGenericAttackConfig::standard_monster_attack_with_reentry(
        anim_mode,
        ROBOTS_TEST_ANIM_ATTACK_OUTER_RADIUS,
        ROBOTS_TEST_ANIM_ATTACK_YAW_TOLERANCE_RADIANS,
        ROBOTS_TEST_ANIM_ATTACK_REENTRY_TICKS,
    )
}

pub const fn test_anim_common_hit_config() -> RobotsCommonAiHitConfig {
    RobotsCommonAiHitConfig::fast_front_back()
}

pub const fn test_anim_fatal_hit_config() -> RobotsCommonAiHitConfig {
    RobotsCommonAiHitConfig::fast_front_back_fatal()
}

pub const fn test_anim_effective_handler_flags(
    handler_flags_628: u32,
    has_anim_35: bool,
    has_anim_36: bool,
) -> u32 {
    if has_anim_35 && has_anim_36 {
        handler_flags_628 | ROBOTS_TEST_ANIM_HANDLER_FLAGS_ANIM_35_36_OR_MASK
    } else {
        handler_flags_628
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RobotsTestAnimBehaviorWinner {
    CommonIdle,
    Patrol,
    Pursue,
    PeriodicIdle,
    AttackGroup,
    ScrambledHit,
    ElectroHit,
    MagneticHit,
    CommonHit,
}

/// TestAnimBot builder 0x00468590 appends nodes after the common Monster Idle
/// node in this exact order: Patrol, direct Pursue, PeriodicIdle, AttackGroup,
/// ScrambledHit, ElectroHit, MagneticHit, HitFatal, CommonHit. HitFatal is
/// serviced by the common fatal host before this selector, so it is omitted here.
/// Native selector comparison is strict >, preserving the first node on ties.
#[allow(clippy::too_many_arguments)]
pub fn test_anim_behavior_winner(
    common_idle_priority: u8,
    patrol_priority: u8,
    pursue_priority: u8,
    periodic_idle_priority: u8,
    attack_group_priority: u8,
    scrambled_hit_priority: u8,
    electro_hit_priority: u8,
    magnetic_hit_priority: u8,
    common_hit_priority: u8,
) -> Option<RobotsTestAnimBehaviorWinner> {
    let candidates = [
        (common_idle_priority, RobotsTestAnimBehaviorWinner::CommonIdle),
        (patrol_priority, RobotsTestAnimBehaviorWinner::Patrol),
        (pursue_priority, RobotsTestAnimBehaviorWinner::Pursue),
        (
            periodic_idle_priority,
            RobotsTestAnimBehaviorWinner::PeriodicIdle,
        ),
        (
            attack_group_priority,
            RobotsTestAnimBehaviorWinner::AttackGroup,
        ),
        (
            scrambled_hit_priority,
            RobotsTestAnimBehaviorWinner::ScrambledHit,
        ),
        (
            electro_hit_priority,
            RobotsTestAnimBehaviorWinner::ElectroHit,
        ),
        (
            magnetic_hit_priority,
            RobotsTestAnimBehaviorWinner::MagneticHit,
        ),
        (common_hit_priority, RobotsTestAnimBehaviorWinner::CommonHit),
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
    fn builder_constants_match_00468590() {
        let patrol = test_anim_patrol_config();
        assert_eq!(patrol.interval_seconds, 7);
        assert_eq!(patrol.target_locomotion_scalar.to_bits(), 0.2f32.to_bits());
        assert_eq!(test_anim_pursue_config(), RobotsAiPursueConfig::new(10.0, 1.0));
        assert_eq!(ROBOTS_TEST_ANIM_PERIODIC_IDLE_BASE_DELAY_TICKS, 180);
        assert_eq!(
            ROBOTS_TEST_ANIM_PERIODIC_IDLE_ANIM_MODES,
            [0x0900_0006, 0x0900_0007, 0x0900_0008, 0x0900_0009, 0x0900_000A]
        );
        assert_eq!(
            ROBOTS_TEST_ANIM_ATTACK_ANIM_MODES,
            [0x0900_0025, 0x0900_0027, 0x0900_0037, 0x0900_0038, 0x0900_0039]
        );
        for mode in ROBOTS_TEST_ANIM_ATTACK_ANIM_MODES {
            let attack = test_anim_attack_config(mode);
            assert_eq!(attack.primary_anim_mode, mode);
            assert_eq!(attack.outer_radius.to_bits(), 2.0f32.to_bits());
            assert_eq!(
                attack.yaw_tolerance_radians.to_bits(),
                ROBOTS_TEST_ANIM_ATTACK_YAW_TOLERANCE_RADIANS.to_bits()
            );
            assert_eq!(attack.reentry_delay_ticks, 60);
            assert!(attack.sticky_while_active);
        }
        assert_eq!(
            test_anim_effective_handler_flags(0x100, true, true),
            0x106
        );
        assert_eq!(
            test_anim_effective_handler_flags(0x100, true, false),
            0x100
        );
    }

    #[test]
    fn selector_preserves_native_builder_order_on_equal_priorities() {
        assert_eq!(
            test_anim_behavior_winner(2, 10, 30, 20, 50, 85, 200, 105, 100),
            Some(RobotsTestAnimBehaviorWinner::ElectroHit)
        );
        assert_eq!(
            test_anim_behavior_winner(2, 10, 30, 20, 50, 100, 1, 1, 100),
            Some(RobotsTestAnimBehaviorWinner::ScrambledHit)
        );
    }

    #[test]
    fn hit_configs_match_fast_front_back_builder_values() {
        assert_eq!(
            test_anim_common_hit_config(),
            RobotsCommonAiHitConfig::fast_front_back()
        );
        assert_eq!(
            test_anim_fatal_hit_config(),
            RobotsCommonAiHitConfig::fast_front_back_fatal()
        );
    }
}
