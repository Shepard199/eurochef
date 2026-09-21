use serde::Serialize;

use super::ai_character::RobotsAiHandlerClass;

pub const ROBOTS_BASE_MONSTER_IDLE_ANIM_MODE: u32 = 0x0900_0004;
pub const ROBOTS_BASE_MONSTER_IDLE_PRIORITY: u8 = 2;

/// MonsterBase constructor `0x004514F0` installs one common `AI_Idle` node
/// before invoking class vslot +0x108. Only the real base handler keeps the
/// base vtable `0x005E2920` with no-op builder `0x004190D0`. EM07 PiranhaBot
/// and TestAnimBot have their own vtables/builders and must use specialized hosts.
pub const fn uses_base_monster_idle_only(handler_class: RobotsAiHandlerClass) -> bool {
    matches!(handler_class, RobotsAiHandlerClass::MonsterBase)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RobotsBaseMonsterIdleRuntimeState {
    pub active: bool,
}

impl RobotsBaseMonsterIdleRuntimeState {
    pub const fn priority() -> u8 {
        ROBOTS_BASE_MONSTER_IDLE_PRIORITY
    }

    pub fn enter(&mut self) {
        self.active = true;
    }

    pub fn leave(&mut self) {
        self.active = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_monster_idle_only_family_is_exact_and_uses_native_idle04_priority2() {
        assert!(uses_base_monster_idle_only(
            RobotsAiHandlerClass::MonsterBase
        ));
        assert!(!uses_base_monster_idle_only(
            RobotsAiHandlerClass::Em07PiranhaBot
        ));
        assert!(!uses_base_monster_idle_only(
            RobotsAiHandlerClass::TestAnimBot
        ));
        assert!(!uses_base_monster_idle_only(RobotsAiHandlerClass::SpinTop));
        assert_eq!(ROBOTS_BASE_MONSTER_IDLE_ANIM_MODE, 0x0900_0004);
        assert_eq!(RobotsBaseMonsterIdleRuntimeState::priority(), 2);
    }
}
