use serde::Serialize;

pub const ROBOTS_PLAYER_ACTION_48000000: u32 = 0x4800_0000;
pub const ROBOTS_PLAYER_ACTION_48000003: u32 = 0x4800_0003;
pub const ROBOTS_PLAYER_ACTION_48000004: u32 = 0x4800_0004;
pub const ROBOTS_PLAYER_ACTION_SCRAP_GUN: u32 = 0x4800_0005;
pub const ROBOTS_PLAYER_ACTION_48000008: u32 = 0x4800_0008;
pub const ROBOTS_PLAYER_ACTION_4800000B: u32 = 0x4800_000b;
pub const ROBOTS_PLAYER_ACTION_FLAG_4000: u32 = 0x4000;
pub const ROBOTS_PLAYER_ACTION_FLAG_WATCHBOT_CONTROL: u32 = 0x800;
pub const ROBOTS_PLAYER_WATCHBOT_CONTROL_MODE_ACTIVE: u8 = 1;
pub const ROBOTS_PLAYER_WATCHBOT_EXIT_BLOCKED_STATE: u8 = 0x35;
pub const ROBOTS_PLAYER_WATCHBOT_EXIT_ANIM_MODE: u32 = 0x0900_00c2;
pub const ROBOTS_SCRAP_GUN_INVENTORY_UIDS: [u32; 4] =
    [0x4800_0021, 0x4800_0022, 0x4800_0023, 0x4800_0024];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsPlayerActionKind {
    Mode0,
    Mode1,
    Mode2,
    ScrapGun,
    Mode4,
    Mode6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsPlayerActionProfile {
    pub action_uid: u32,
    pub native_mode: u8,
    pub kind: RobotsPlayerActionKind,
    /// Native `0x004AFDF0` local `BL == 0` only for actions 0x48000003/04.
    /// `DAT_007B3170` receives the inverse after queue/attach.
    pub sets_global_input_lock: bool,
    /// Only M10-used mode0 and mode3 factories/classes are closed far enough to
    /// materialize a logical action instance without inventing class semantics.
    pub production_attach_proven: bool,
}

pub fn robots_player_action_profile(action_uid: u32) -> Option<RobotsPlayerActionProfile> {
    let (native_mode, kind, sets_global_input_lock, production_attach_proven) = match action_uid {
        ROBOTS_PLAYER_ACTION_48000000 => (0, RobotsPlayerActionKind::Mode0, false, true),
        ROBOTS_PLAYER_ACTION_48000003 => (1, RobotsPlayerActionKind::Mode1, true, false),
        ROBOTS_PLAYER_ACTION_48000004 => (2, RobotsPlayerActionKind::Mode2, true, false),
        ROBOTS_PLAYER_ACTION_SCRAP_GUN => (3, RobotsPlayerActionKind::ScrapGun, false, true),
        ROBOTS_PLAYER_ACTION_48000008 => (4, RobotsPlayerActionKind::Mode4, false, false),
        // `0x004AFDF0` maps 0x4800000B to native mode byte 6.
        ROBOTS_PLAYER_ACTION_4800000B => (6, RobotsPlayerActionKind::Mode6, false, false),
        _ => return None,
    };
    Some(RobotsPlayerActionProfile {
        action_uid,
        native_mode,
        kind,
        sets_global_input_lock,
        production_attach_proven,
    })
}

pub fn robots_player_action_immediate_gate(
    player_state_6de: u8,
    active_action_mode_15: Option<u8>,
) -> bool {
    // `0x004C0A20`: an active action whose mode byte is 6 blocks replacement.
    if active_action_mode_15 == Some(6) {
        return false;
    }
    !matches!(
        player_state_6de,
        0x01 | 0x0e
            | 0x0f
            | 0x10
            | 0x13
            | 0x14
            | 0x15
            | 0x16
            | 0x17
            | 0x18
            | 0x19
            | 0x1d
            | 0x1e
            | 0x20
            | 0x21
            | 0x22
            | 0x2c
            | 0x2d
            | 0x2e
            | 0x2f
            | 0x30
            | 0x31
            | 0x3e
    )
}

fn robots_player_action_attach_state_gate(player_state_6de: u8) -> bool {
    // Player vslot +0x14C = 0x004B70E0 has a smaller hard reject than the
    // queue gate. It matters when a previously queued UID is requested again.
    !matches!(player_state_6de, 0x1d | 0x2f | 0x3e)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsPlayerActionInstance {
    pub action_uid: u32,
    pub native_mode_15: u8,
    pub kind: RobotsPlayerActionKind,
    /// `XWeapon_ScrapGun::+0x08 0x004C8430` picks the first positive count from
    /// native inventory UIDs 0x48000021..24, or keeps -1 when all are empty.
    pub scrap_gun_inventory_uid: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsPlayerActionRequestStatus {
    Queued,
    Attached,
    AttachRejectedByPlayerState,
    AttachClassUnimplemented,
    UnsupportedAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsPlayerActionRequestOutcome {
    pub status: RobotsPlayerActionRequestStatus,
    pub profile: Option<RobotsPlayerActionProfile>,
    pub replaced_action: Option<RobotsPlayerActionInstance>,
    pub active_action: Option<RobotsPlayerActionInstance>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsPlayerWatchbotControlExitOutcome {
    pub retried_action: Option<RobotsPlayerActionRequestOutcome>,
    /// Player vslot +0xDC -> 0x004AFD60 -> generic `0x004051E0` AnimMode request.
    pub requested_anim_mode_uid: u32,
    pub next_player_state_6de: u8,
    /// Player +0x4D4, consumed by `0x00508B08(..., 15, 15, 1, 0)` then reset to -1.
    pub exit_sound_uid: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RobotsPlayerActionRuntime {
    /// XItemHandler_Player +0x5E8. Constructor `0x004AD1D0` initializes -1.
    pub pending_action_uid_5e8: Option<u32>,
    /// XItemHandler_Player +0x5CC. Constructor initializes 0x48000000.
    pub committed_action_uid_5cc: u32,
    /// XItemHandler_Player +0x6EC object, projected without native pointers.
    pub active_action: Option<RobotsPlayerActionInstance>,
    /// XItemHandler_Player +0x6D4. Ordinary `0x004AFDF0` clears bit0x4000 first.
    pub handler_flags_6d4: u32,
    /// Global DAT_007B3170 written by `0x004AFDF0`.
    pub global_input_lock_7b3170: bool,
    /// Exact 0x0049C3C0 request after successful attach: mode3 => 1, others => 2.
    pub last_post_attach_mode: Option<u8>,
    /// Player +0x4D4 armed by WatchBot-control entry and consumed on successful exit.
    pub watchbot_exit_sound_uid_4d4: Option<u32>,
}

impl Default for RobotsPlayerActionRuntime {
    fn default() -> Self {
        Self {
            pending_action_uid_5e8: None,
            committed_action_uid_5cc: ROBOTS_PLAYER_ACTION_48000000,
            active_action: None,
            handler_flags_6d4: 0,
            global_input_lock_7b3170: false,
            last_post_attach_mode: None,
            watchbot_exit_sound_uid_4d4: None,
        }
    }
}

impl RobotsPlayerActionRuntime {
    pub fn request_cutscene_action<F>(
        &mut self,
        action_uid: u32,
        player_state_6de: u8,
        mut inventory_count: F,
    ) -> RobotsPlayerActionRequestOutcome
    where
        F: FnMut(u32) -> i32,
    {
        let Some(profile) = robots_player_action_profile(action_uid) else {
            return RobotsPlayerActionRequestOutcome {
                status: RobotsPlayerActionRequestStatus::UnsupportedAction,
                profile: None,
                replaced_action: None,
                active_action: self.active_action,
            };
        };

        self.handler_flags_6d4 &= !ROBOTS_PLAYER_ACTION_FLAG_4000;
        let active_mode = self.active_action.map(|action| action.native_mode_15);
        if self.pending_action_uid_5e8 != Some(action_uid)
            && !robots_player_action_immediate_gate(player_state_6de, active_mode)
        {
            self.global_input_lock_7b3170 = profile.sets_global_input_lock;
            self.pending_action_uid_5e8 = Some(action_uid);
            return RobotsPlayerActionRequestOutcome {
                status: RobotsPlayerActionRequestStatus::Queued,
                profile: Some(profile),
                replaced_action: None,
                active_action: self.active_action,
            };
        }

        if !robots_player_action_attach_state_gate(player_state_6de) {
            return RobotsPlayerActionRequestOutcome {
                status: RobotsPlayerActionRequestStatus::AttachRejectedByPlayerState,
                profile: Some(profile),
                replaced_action: None,
                active_action: self.active_action,
            };
        }
        if !profile.production_attach_proven {
            return RobotsPlayerActionRequestOutcome {
                status: RobotsPlayerActionRequestStatus::AttachClassUnimplemented,
                profile: Some(profile),
                replaced_action: None,
                active_action: self.active_action,
            };
        }

        // Player +0x14C destroys the old action before constructing the new one.
        let replaced_action = self.active_action.take();
        let scrap_gun_inventory_uid = (profile.kind == RobotsPlayerActionKind::ScrapGun)
            .then(|| {
                ROBOTS_SCRAP_GUN_INVENTORY_UIDS
                    .into_iter()
                    .find(|uid| inventory_count(*uid) > 0)
            })
            .flatten();
        let action = RobotsPlayerActionInstance {
            action_uid,
            native_mode_15: profile.native_mode,
            kind: profile.kind,
            scrap_gun_inventory_uid,
        };
        self.active_action = Some(action);
        self.committed_action_uid_5cc = action_uid;
        self.global_input_lock_7b3170 = profile.sets_global_input_lock;
        // Cutscene always supplies second arg=1, so successful attach clears +0x5E8.
        self.pending_action_uid_5e8 = None;
        self.last_post_attach_mode = Some(if profile.native_mode == 3 { 1 } else { 2 });

        RobotsPlayerActionRequestOutcome {
            status: RobotsPlayerActionRequestStatus::Attached,
            profile: Some(profile),
            replaced_action,
            active_action: Some(action),
        }
    }

    /// Native `0x004B0D30`: consume the exact pending `+0x5E8` UID by routing it
    /// back through `0x004AFDF0(uid, 1)`. This is deliberately not a per-frame
    /// poll. The proved ordinary retry occurs in the WatchBot-control exit branch
    /// of `0x004B02D0` when GameWnd `+0x50F != 1` and Player `+0x6DE != 0x35`.
    /// `FUN_00472AF0(1) == 0x0D` only gates the independent control-pose copy.
    pub fn retry_pending_cutscene_action<F>(
        &mut self,
        player_state_6de: u8,
        inventory_count: F,
    ) -> Option<RobotsPlayerActionRequestOutcome>
    where
        F: FnMut(u32) -> i32,
    {
        let pending_uid = self.pending_action_uid_5e8?;
        Some(self.request_cutscene_action(pending_uid, player_state_6de, inventory_count))
    }

    pub fn arm_watchbot_exit_sound(&mut self, sound_uid: u32) {
        self.watchbot_exit_sound_uid_4d4 = Some(sound_uid);
    }

    /// Exact exit branch of Player `0x004B02D0` used by WatchBot control.
    /// Retry runs against the current Player state before native writes state 2.
    pub fn finish_watchbot_control_exit<F>(
        &mut self,
        control_mode_7b3207: u8,
        player_state_6de: u8,
        exit_sound_uid_4d4: Option<u32>,
        inventory_count: F,
    ) -> Option<RobotsPlayerWatchbotControlExitOutcome>
    where
        F: FnMut(u32) -> i32,
    {
        self.handler_flags_6d4 |= ROBOTS_PLAYER_ACTION_FLAG_WATCHBOT_CONTROL;
        if control_mode_7b3207 == ROBOTS_PLAYER_WATCHBOT_CONTROL_MODE_ACTIVE
            || player_state_6de == ROBOTS_PLAYER_WATCHBOT_EXIT_BLOCKED_STATE
        {
            return None;
        }

        let exit_sound_uid = exit_sound_uid_4d4.or(self.watchbot_exit_sound_uid_4d4);
        let retried_action = self.retry_pending_cutscene_action(player_state_6de, inventory_count);
        self.watchbot_exit_sound_uid_4d4 = None;
        Some(RobotsPlayerWatchbotControlExitOutcome {
            retried_action,
            requested_anim_mode_uid: ROBOTS_PLAYER_WATCHBOT_EXIT_ANIM_MODE,
            next_player_state_6de: 2,
            exit_sound_uid,
        })
    }

    pub fn set_cutscene_flag_4000(&mut self) {
        self.handler_flags_6d4 |= ROBOTS_PLAYER_ACTION_FLAG_4000;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m10_mode3_attaches_scrapgun_and_selects_first_positive_inventory_uid() {
        let mut runtime = RobotsPlayerActionRuntime::default();
        let outcome = runtime.request_cutscene_action(ROBOTS_PLAYER_ACTION_SCRAP_GUN, 2, |uid| {
            if uid == 0x4800_0023 {
                5
            } else {
                0
            }
        });
        assert_eq!(outcome.status, RobotsPlayerActionRequestStatus::Attached);
        assert_eq!(runtime.pending_action_uid_5e8, None);
        assert_eq!(
            runtime.committed_action_uid_5cc,
            ROBOTS_PLAYER_ACTION_SCRAP_GUN
        );
        assert_eq!(runtime.last_post_attach_mode, Some(1));
        assert_eq!(
            runtime.active_action.unwrap().scrap_gun_inventory_uid,
            Some(0x4800_0023)
        );
    }

    #[test]
    fn blocked_player_state_queues_new_cutscene_action_without_replacing_active_action() {
        let mut runtime = RobotsPlayerActionRuntime::default();
        runtime.request_cutscene_action(ROBOTS_PLAYER_ACTION_SCRAP_GUN, 2, |_| 0);
        let before = runtime.active_action;
        let outcome = runtime.request_cutscene_action(ROBOTS_PLAYER_ACTION_48000000, 0x0e, |_| 0);
        assert_eq!(outcome.status, RobotsPlayerActionRequestStatus::Queued);
        assert_eq!(
            runtime.pending_action_uid_5e8,
            Some(ROBOTS_PLAYER_ACTION_48000000)
        );
        assert_eq!(runtime.active_action, before);
    }

    #[test]
    fn repeated_pending_request_reaches_native_attach_gate_and_death_state_can_reject_it() {
        let mut runtime = RobotsPlayerActionRuntime::default();
        runtime.pending_action_uid_5e8 = Some(ROBOTS_PLAYER_ACTION_SCRAP_GUN);
        let outcome = runtime.request_cutscene_action(ROBOTS_PLAYER_ACTION_SCRAP_GUN, 0x1d, |_| 0);
        assert_eq!(
            outcome.status,
            RobotsPlayerActionRequestStatus::AttachRejectedByPlayerState
        );
        assert_eq!(
            runtime.pending_action_uid_5e8,
            Some(ROBOTS_PLAYER_ACTION_SCRAP_GUN)
        );
    }

    #[test]
    fn watchbot_control_exit_retry_uses_pre_exit_player_state_before_state_two() {
        let mut runtime = RobotsPlayerActionRuntime::default();
        let queued = runtime.request_cutscene_action(ROBOTS_PLAYER_ACTION_SCRAP_GUN, 0x0e, |_| 0);
        assert_eq!(queued.status, RobotsPlayerActionRequestStatus::Queued);
        let exit = runtime
            .finish_watchbot_control_exit(0, 0x0e, Some(0x1af0_02c5), |uid| {
                if uid == 0x4800_0021 {
                    1
                } else {
                    0
                }
            })
            .unwrap();
        let retried = exit.retried_action.unwrap();
        assert_eq!(retried.status, RobotsPlayerActionRequestStatus::Attached);
        assert_eq!(runtime.pending_action_uid_5e8, None);
        assert_eq!(
            runtime.committed_action_uid_5cc,
            ROBOTS_PLAYER_ACTION_SCRAP_GUN
        );
        assert_ne!(
            runtime.handler_flags_6d4 & ROBOTS_PLAYER_ACTION_FLAG_WATCHBOT_CONTROL,
            0
        );
        assert_eq!(
            exit.requested_anim_mode_uid,
            ROBOTS_PLAYER_WATCHBOT_EXIT_ANIM_MODE
        );
        assert_eq!(exit.next_player_state_6de, 2);
        assert_eq!(exit.exit_sound_uid, Some(0x1af0_02c5));
        assert_eq!(
            runtime.active_action.unwrap().scrap_gun_inventory_uid,
            Some(0x4800_0021)
        );
    }

    #[test]
    fn watchbot_control_exit_stays_in_active_mode_or_blocked_state() {
        let mut active = RobotsPlayerActionRuntime::default();
        assert!(active
            .finish_watchbot_control_exit(
                ROBOTS_PLAYER_WATCHBOT_CONTROL_MODE_ACTIVE,
                0x0e,
                None,
                |_| 0,
            )
            .is_none());
        assert_ne!(
            active.handler_flags_6d4 & ROBOTS_PLAYER_ACTION_FLAG_WATCHBOT_CONTROL,
            0
        );

        let mut blocked = RobotsPlayerActionRuntime::default();
        blocked.arm_watchbot_exit_sound(0x1b00_0025);
        assert!(
            blocked
                .finish_watchbot_control_exit(
                    0,
                    ROBOTS_PLAYER_WATCHBOT_EXIT_BLOCKED_STATE,
                    None,
                    |_| 0,
                )
                .is_none()
        );
        assert_ne!(
            blocked.handler_flags_6d4 & ROBOTS_PLAYER_ACTION_FLAG_WATCHBOT_CONTROL,
            0
        );
        assert_eq!(blocked.watchbot_exit_sound_uid_4d4, Some(0x1b00_0025));

        let completed = blocked
            .finish_watchbot_control_exit(0, 2, None, |_| 0)
            .unwrap();
        assert_eq!(completed.exit_sound_uid, Some(0x1b00_0025));
        assert_eq!(blocked.watchbot_exit_sound_uid_4d4, None);
    }

    #[test]
    fn pending_retry_is_a_noop_without_native_5e8_request() {
        let mut runtime = RobotsPlayerActionRuntime::default();
        assert!(runtime.retry_pending_cutscene_action(2, |_| 0).is_none());
    }

    #[test]
    fn repeated_m10_mode3_replaces_previous_scrapgun_immediately_in_normal_state() {
        let mut runtime = RobotsPlayerActionRuntime::default();
        runtime.request_cutscene_action(ROBOTS_PLAYER_ACTION_SCRAP_GUN, 2, |_| 0);
        let outcome = runtime.request_cutscene_action(ROBOTS_PLAYER_ACTION_SCRAP_GUN, 2, |_| 0);
        assert_eq!(outcome.status, RobotsPlayerActionRequestStatus::Attached);
        assert!(matches!(
            outcome.replaced_action,
            Some(RobotsPlayerActionInstance {
                kind: RobotsPlayerActionKind::ScrapGun,
                ..
            })
        ));
    }

    #[test]
    fn ordinary_action_request_clears_cutscene_flag_4000_and_mode0_uses_post_attach_mode2() {
        let mut runtime = RobotsPlayerActionRuntime::default();
        runtime.set_cutscene_flag_4000();
        assert_ne!(
            runtime.handler_flags_6d4 & ROBOTS_PLAYER_ACTION_FLAG_4000,
            0
        );
        let outcome = runtime.request_cutscene_action(ROBOTS_PLAYER_ACTION_48000000, 2, |_| 0);
        assert_eq!(outcome.status, RobotsPlayerActionRequestStatus::Attached);
        assert_eq!(
            runtime.handler_flags_6d4 & ROBOTS_PLAYER_ACTION_FLAG_4000,
            0
        );
        assert_eq!(runtime.last_post_attach_mode, Some(2));
    }
}
