use serde::Serialize;

use super::watchbot::{
    RobotsWatchbotComponentTransitionSnapshotBits, RobotsWatchbotOwnerRuntime,
    ROBOTS_WATCHBOT_CONTROL_COMPONENT_MODE,
};

/// GameWnd `+0x50F` (`DAT_007B3207`). Constructor `0x004D0570` clears it.
pub const ROBOTS_GAME_CONTROL_MODE_DEFAULT: u8 = 0;
/// Native WatchBot-control mode selected by Player `0x004B0120`.
pub const ROBOTS_GAME_CONTROL_MODE_WATCHBOT: u8 = 1;
pub const ROBOTS_WATCHBOT_CONTROL_PLAYER_STATE: u8 = 0x35;
pub const ROBOTS_WATCHBOT_CONTROL_ANIM_MODE: u32 = 0x0900_00c1;
pub const ROBOTS_WATCHBOT_EXIT_SOUND_FALLBACK: u32 = 0x1b00_0025;
pub const ROBOTS_WATCHBOT_ENTER_SOUND_EVEN: u32 = 0x1b00_003f;
pub const ROBOTS_WATCHBOT_ENTER_SOUND_ODD: u32 = 0x1b00_0044;
pub const ROBOTS_WATCHBOT_INTERACTION_CODE: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsGameControlModeTransition {
    pub previous_mode: u8,
    pub current_mode: u8,
    pub entered_watchbot_control: bool,
    pub exited_watchbot_control: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsWatchbotControlEnterOutcome {
    pub mode_transition: RobotsGameControlModeTransition,
    pub next_player_state_6de: u8,
    pub requested_anim_mode_uid: u32,
    /// WatchBot Handler `+0x4A4` target written by `0x004900D0/0x0048FF20`.
    pub requested_watchbot_component_mode: u32,
    /// Player +0x4D4, retained until the later WatchBot-control exit branch.
    pub exit_sound_uid_4d4: u32,
    /// Native consumes one process-global RNG draw and selects by bit0. `None`
    /// means the host has no proven global-seed anchor.
    pub entry_sound_uid: Option<u32>,
    /// Native `0x004B0120` writes process-global `DAT_007B25A8 = 1`.
    pub set_input_latch_7b25a8: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsGameControlRuntime {
    /// Exact GameWnd `+0x50F` byte owned by native setter `0x004D1570`.
    pub mode_50f: u8,
    /// One-shot Player-service work created by a proved `1 -> !=1` edge.
    watchbot_exit_pending: bool,
}

impl Default for RobotsGameControlRuntime {
    fn default() -> Self {
        Self {
            mode_50f: ROBOTS_GAME_CONTROL_MODE_DEFAULT,
            watchbot_exit_pending: false,
        }
    }
}

impl RobotsGameControlRuntime {
    /// Native `0x004D1570` changes GameWnd `+0x50F` only when the requested mode
    /// differs. HUD invalidation belongs to the presentation host; this shared
    /// contract only exposes the gameplay edge.
    pub fn set_mode(&mut self, requested_mode: u8) -> Option<RobotsGameControlModeTransition> {
        let previous_mode = self.mode_50f;
        if previous_mode == requested_mode {
            return None;
        }
        self.mode_50f = requested_mode;
        let transition = RobotsGameControlModeTransition {
            previous_mode,
            current_mode: requested_mode,
            entered_watchbot_control: previous_mode != ROBOTS_GAME_CONTROL_MODE_WATCHBOT
                && requested_mode == ROBOTS_GAME_CONTROL_MODE_WATCHBOT,
            exited_watchbot_control: previous_mode == ROBOTS_GAME_CONTROL_MODE_WATCHBOT
                && requested_mode != ROBOTS_GAME_CONTROL_MODE_WATCHBOT,
        };
        if transition.exited_watchbot_control {
            self.watchbot_exit_pending = true;
        } else if transition.entered_watchbot_control {
            // Restoring mode1 before Player services the exit cancels the pending edge.
            self.watchbot_exit_pending = false;
        }
        Some(transition)
    }

    /// Exact gameplay transition owned by Player `0x004B0120`, after caller
    /// `0x004BCCA0` accepted the action-button edge for a focused category 0x4C XItem.
    pub fn enter_watchbot_control(
        &mut self,
        watchbot_owner: &mut RobotsWatchbotOwnerRuntime,
        focused_xitem_snapshot: RobotsWatchbotComponentTransitionSnapshotBits,
        configured_exit_sound_uid: Option<u32>,
        global_rng_draw: Option<u32>,
    ) -> Option<RobotsWatchbotControlEnterOutcome> {
        if !watchbot_owner.control_entry_ready()
            || self.mode_50f != ROBOTS_GAME_CONTROL_MODE_DEFAULT
        {
            return None;
        }
        let mode_transition = self.set_mode(ROBOTS_GAME_CONTROL_MODE_WATCHBOT)?;
        if !watchbot_owner.request_component_mode_with_context(
            ROBOTS_WATCHBOT_CONTROL_COMPONENT_MODE,
            Some(focused_xitem_snapshot),
            true,
            false,
            None,
        ) {
            return None;
        }
        let entry_sound_uid = global_rng_draw.map(|draw| {
            if draw & 1 == 0 {
                ROBOTS_WATCHBOT_ENTER_SOUND_EVEN
            } else {
                ROBOTS_WATCHBOT_ENTER_SOUND_ODD
            }
        });
        Some(RobotsWatchbotControlEnterOutcome {
            mode_transition,
            next_player_state_6de: ROBOTS_WATCHBOT_CONTROL_PLAYER_STATE,
            requested_anim_mode_uid: ROBOTS_WATCHBOT_CONTROL_ANIM_MODE,
            requested_watchbot_component_mode: ROBOTS_WATCHBOT_CONTROL_COMPONENT_MODE,
            exit_sound_uid_4d4: configured_exit_sound_uid
                .unwrap_or(ROBOTS_WATCHBOT_EXIT_SOUND_FALLBACK),
            entry_sound_uid,
            set_input_latch_7b25a8: true,
        })
    }

    pub fn watchbot_exit_pending(&self) -> bool {
        self.watchbot_exit_pending
    }

    pub fn complete_watchbot_exit(&mut self) {
        self.watchbot_exit_pending = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watchbot_entry_requires_ready_handler_and_preserves_exact_native_effects() {
        let mut runtime = RobotsGameControlRuntime::default();
        let mut watchbot = RobotsWatchbotOwnerRuntime::default();
        let snapshot = RobotsWatchbotComponentTransitionSnapshotBits::from_f32(
            [1.0, 2.0, 3.0, 1.0],
            [0.0, 0.5, 0.0, 1.0],
        );
        assert!(runtime
            .enter_watchbot_control(&mut watchbot, snapshot, None, Some(0))
            .is_none());
        assert_eq!(runtime.mode_50f, ROBOTS_GAME_CONTROL_MODE_DEFAULT);

        assert!(watchbot.service_player_owner(true, 0));
        let even = runtime
            .enter_watchbot_control(&mut watchbot, snapshot, None, Some(0x1234))
            .unwrap();
        assert_eq!(runtime.mode_50f, ROBOTS_GAME_CONTROL_MODE_WATCHBOT);
        assert!(even.mode_transition.entered_watchbot_control);
        assert_eq!(
            even.next_player_state_6de,
            ROBOTS_WATCHBOT_CONTROL_PLAYER_STATE
        );
        assert_eq!(
            even.requested_anim_mode_uid,
            ROBOTS_WATCHBOT_CONTROL_ANIM_MODE
        );
        assert_eq!(even.requested_watchbot_component_mode, 2);
        assert_eq!(watchbot.pending_component_mode_4a4, 2);
        assert_eq!(even.exit_sound_uid_4d4, ROBOTS_WATCHBOT_EXIT_SOUND_FALLBACK);
        assert_eq!(even.entry_sound_uid, Some(ROBOTS_WATCHBOT_ENTER_SOUND_EVEN));
        assert!(even.set_input_latch_7b25a8);

        runtime.set_mode(ROBOTS_GAME_CONTROL_MODE_DEFAULT).unwrap();
        watchbot.complete_component_mode_transition();
        let odd = runtime
            .enter_watchbot_control(&mut watchbot, snapshot, Some(0x1b00_00aa), Some(0x1235))
            .unwrap();
        assert_eq!(odd.exit_sound_uid_4d4, 0x1b00_00aa);
        assert_eq!(odd.entry_sound_uid, Some(ROBOTS_WATCHBOT_ENTER_SOUND_ODD));

        runtime.set_mode(ROBOTS_GAME_CONTROL_MODE_DEFAULT).unwrap();
        watchbot.complete_component_mode_transition();
        let unknown_rng = runtime
            .enter_watchbot_control(&mut watchbot, snapshot, None, None)
            .unwrap();
        assert_eq!(unknown_rng.entry_sound_uid, None);
    }

    #[test]
    fn game_control_mode_setter_is_edge_triggered_like_native_4d1570() {
        let mut runtime = RobotsGameControlRuntime::default();
        assert!(runtime.set_mode(ROBOTS_GAME_CONTROL_MODE_DEFAULT).is_none());

        let enter = runtime.set_mode(ROBOTS_GAME_CONTROL_MODE_WATCHBOT).unwrap();
        assert!(enter.entered_watchbot_control);
        assert!(!enter.exited_watchbot_control);
        assert_eq!(enter.previous_mode, 0);
        assert_eq!(enter.current_mode, 1);

        assert!(runtime
            .set_mode(ROBOTS_GAME_CONTROL_MODE_WATCHBOT)
            .is_none());

        let exit = runtime.set_mode(2).unwrap();
        assert!(!exit.entered_watchbot_control);
        assert!(exit.exited_watchbot_control);
        assert_eq!(exit.previous_mode, 1);
        assert_eq!(exit.current_mode, 2);
        assert!(runtime.watchbot_exit_pending());

        runtime.complete_watchbot_exit();
        assert!(!runtime.watchbot_exit_pending());

        runtime.set_mode(ROBOTS_GAME_CONTROL_MODE_WATCHBOT).unwrap();
        runtime.set_mode(0).unwrap();
        assert!(runtime.watchbot_exit_pending());
        runtime.set_mode(ROBOTS_GAME_CONTROL_MODE_WATCHBOT).unwrap();
        assert!(!runtime.watchbot_exit_pending());
    }
}
