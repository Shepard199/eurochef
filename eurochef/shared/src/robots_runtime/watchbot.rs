use serde::Serialize;

pub const ROBOTS_WATCHBOT_TARGET_FOLLOW_COMPONENT_MODE: u32 = 1;
pub const ROBOTS_WATCHBOT_CONTROL_COMPONENT_MODE: u32 = 2;
pub const ROBOTS_WATCHBOT_TRIGGER_PATH_MODE: u32 = 3;
pub const ROBOTS_WATCHBOT_NO_PATH_UID: u32 = 0x0B00_0000;
pub const ROBOTS_WATCHBOT_DISTANCE_SCALE: f32 = f32::from_bits(0x3DCC_CCCD);
pub const ROBOTS_WATCHBOT_DISTANCE_SQUARED_SCALE: f32 = f32::from_bits(0x3C23_D70B);
pub const ROBOTS_WATCHBOT_COMPONENT_MODE_MIN: u32 = 1;
pub const ROBOTS_WATCHBOT_COMPONENT_MODE_MAX: u32 = 3;
pub const ROBOTS_WATCHBOT_COMPONENT_TRANSITION_EPSILON: f32 = f32::from_bits(0x3A83_126F); // 0.001

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsWatchbotTriggerContract {
    pub mode: u32,
    pub path_uid: u32,
    pub flags: u32,
    pub enter_distance_raw: i32,
    pub leave_distance_raw: i32,
}

impl RobotsWatchbotTriggerContract {
    pub fn from_serialized_data(data: &[Option<u32>]) -> Option<Self> {
        Some(Self {
            mode: data.first().copied().flatten()?,
            path_uid: data.get(1).copied().flatten()?,
            flags: data.get(2).copied().flatten()?,
            enter_distance_raw: data.get(3).copied().flatten()? as i32,
            leave_distance_raw: data.get(4).copied().flatten()? as i32,
        })
    }

    pub fn enter_distance_squared(self) -> f32 {
        let raw = self.enter_distance_raw as f32;
        raw * raw * ROBOTS_WATCHBOT_DISTANCE_SQUARED_SCALE
    }

    pub fn leave_distance_squared(self) -> Option<f32> {
        let leave = self.leave_distance_raw as f32 * ROBOTS_WATCHBOT_DISTANCE_SCALE;
        let enter = self.enter_distance_raw as f32 * ROBOTS_WATCHBOT_DISTANCE_SCALE;
        if leave <= 0.0 || leave <= enter {
            return None;
        }
        let raw = self.leave_distance_raw as f32;
        Some(raw * raw * ROBOTS_WATCHBOT_DISTANCE_SQUARED_SCALE)
    }

    pub fn should_enter(self, distance_squared: f32) -> bool {
        distance_squared <= self.enter_distance_squared()
    }

    pub fn should_leave(self, distance_squared: f32) -> bool {
        self.leave_distance_squared()
            .is_some_and(|leave_squared| distance_squared > leave_squared)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsWatchbotTriggerRuntime {
    pub enabled_e4: bool,
    pub active_e5: bool,
}

impl Default for RobotsWatchbotTriggerRuntime {
    fn default() -> Self {
        Self {
            enabled_e4: true,
            active_e5: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct RobotsWatchbotTriggerStep {
    pub entered: bool,
    pub exited: bool,
    pub requested_component_mode: Option<u32>,
    pub path_changed: bool,
    pub owner_trigger_key: Option<u64>,
    pub active_trigger_count: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct RobotsWatchbotComponentTransitionSnapshotBits {
    /// Handler +0x4A8..+0x4B4 captured at request time.
    pub position_xyzw_bits: [u32; 4],
    /// Handler +0x4B8..+0x4C4 captured at request time.
    pub rotation_xyzw_bits: [u32; 4],
}

impl RobotsWatchbotComponentTransitionSnapshotBits {
    pub fn from_f32(position_xyzw: [f32; 4], rotation_xyzw: [f32; 4]) -> Self {
        Self {
            position_xyzw_bits: position_xyzw.map(f32::to_bits),
            rotation_xyzw_bits: rotation_xyzw.map(f32::to_bits),
        }
    }

    pub fn position_xyzw(self) -> [f32; 4] {
        self.position_xyzw_bits.map(f32::from_bits)
    }

    pub fn rotation_xyzw(self) -> [f32; 4] {
        self.rotation_xyzw_bits.map(f32::from_bits)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsWatchbotComponentTransitionGate {
    /// DAT_007B3220. Native scheduler does nothing while GameWnd is absent.
    pub game_window_exists: bool,
    /// GameWnd +0x338 transition phase byte.
    pub game_window_phase_338: u8,
    /// GameWnd +0x3B0 transition scalar.
    pub game_window_scalar_3b0: f32,
    /// Required only for the native 2/3 -> 1 Player-control release branch.
    pub player_watchbot_handler_exists: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsWatchbotComponentTransitionStep {
    pub previous_mode: u32,
    pub requested_mode: u32,
    pub seamless_mode2_mode3: bool,
    pub exit_previous_component: bool,
    /// Native `0x004D7DF0(phase, 0x1E, 0)` phase request. Kept neutral here.
    pub host_transition_phase: Option<u8>,
    pub request_game_control_mode_zero: bool,
    pub request_player_watchbot_release: bool,
    pub commit_transition: bool,
    pub setup_new_component: bool,
    pub transition_snapshot: Option<RobotsWatchbotComponentTransitionSnapshotBits>,
}

/// Request-side `+0x518` gate shared by `0x0048FF20`, `0x004900D0` and
/// target binding `0x00490850`. Modal GameWnd states force an otherwise deferred
/// request to become immediate.
pub fn watchbot_component_transition_deferred_after_state_gate(
    requested_deferred: bool,
    special_game_state_7b3021: bool,
    state_stack_top: Option<i32>,
) -> bool {
    if !requested_deferred {
        return false;
    }
    match (special_game_state_7b3021, state_stack_top) {
        (false, Some(2 | 3)) => false,
        (true, Some(1 | 2 | 3)) => false,
        _ => true,
    }
}

/// Engine-neutral projection of Player `+0x550 -> XItem+0x14C` used by native
/// `0x004AFF80`, `0x004B0120` and `0x004B02A0`. Native stores the current
/// WatchBot component at Handler `+0x4A0` and a pending target at `+0x4A4`;
/// zero in `+0x4A4` means no component transition is pending.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct RobotsWatchbotOwnerRuntime {
    pub owner_xitem_exists_550: bool,
    pub handler_attached: bool,
    /// WatchBot Handler +0x488 projected as the pointed EXGeoPath UID.
    pub current_path_uid_488: Option<u32>,
    /// WatchBot Handler +0x48C native trigger pointer projected as a stable host key.
    pub owner_trigger_key_48c: Option<u64>,
    pub current_component_mode_4a0: u32,
    pub pending_component_mode_4a4: u32,
    /// Handler +0x518. True means the transition is waiting on the native GameWnd
    /// phase handshake unless the request-side state gate forced it immediate.
    pub pending_component_deferred_518: bool,
    /// Exact request-time transition pose snapshot. Native captures this when the
    /// request is made; only mode2 entry from non-mode3 currently consumes it.
    pub pending_component_snapshot: Option<RobotsWatchbotComponentTransitionSnapshotBits>,
    /// Common component target XItem written by Handler vslot +0xD0
    /// (`0x00490850`) across slots +0x490..+0x49C; only +0x494/+0x498/+0x49C are concrete.
    /// Native stores a pointer; the portable runtime keeps a stable host key.
    pub component_target_key_28: Option<u64>,
    /// Shared active-trigger count at WatchBot Handler +0x510.
    pub active_trigger_count_510: i32,
}

impl RobotsWatchbotOwnerRuntime {
    /// Minimal gameplay-visible part of Player helper `0x004AFF80`. Native only
    /// creates the `+0x550` owner while HT_Upgrade_Watchbot is enabled, no owner
    /// exists yet, and GameWnd mode is not 2/3/4/6.
    pub fn service_player_owner(
        &mut self,
        watchbot_upgrade_enabled: bool,
        game_control_mode: u8,
    ) -> bool {
        if self.owner_xitem_exists_550
            || !watchbot_upgrade_enabled
            || matches!(game_control_mode, 2 | 3 | 4 | 6)
        {
            return false;
        }
        self.owner_xitem_exists_550 = true;
        self.handler_attached = true;
        self.current_path_uid_488 = None;
        self.owner_trigger_key_48c = None;
        self.current_component_mode_4a0 = 0;
        self.pending_component_mode_4a4 = 0;
        self.pending_component_deferred_518 = true;
        self.pending_component_snapshot = None;
        self.component_target_key_28 = None;
        self.active_trigger_count_510 = 0;
        true
    }

    pub fn clear_player_owner(&mut self) {
        *self = Self::default();
    }

    /// Exact readiness gate at the start of Player `0x004B0120`.
    pub fn control_entry_ready(self) -> bool {
        self.owner_xitem_exists_550 && self.handler_attached && self.pending_component_mode_4a4 == 0
    }

    pub fn current_path_uid_or_sentinel(self) -> u32 {
        self.current_path_uid_488
            .unwrap_or(ROBOTS_WATCHBOT_NO_PATH_UID)
    }

    pub fn trigger_matches(
        self,
        trigger_key: u64,
        contract: RobotsWatchbotTriggerContract,
    ) -> bool {
        if self.current_component_mode_4a0 != contract.mode {
            return false;
        }
        if contract.mode == ROBOTS_WATCHBOT_TRIGGER_PATH_MODE {
            return self.current_path_uid_or_sentinel() == contract.path_uid;
        }
        self.owner_trigger_key_48c == Some(trigger_key)
    }

    fn apply_trigger_contract(
        &mut self,
        contract: RobotsWatchbotTriggerContract,
        path_resolved: bool,
    ) -> RobotsWatchbotTriggerStep {
        let mut step = RobotsWatchbotTriggerStep::default();
        if self.current_component_mode_4a0 != contract.mode && self.pending_component_mode_4a4 == 0
        {
            if self.request_component_mode(contract.mode) {
                step.requested_component_mode = Some(contract.mode);
            }
        }
        if contract.mode == ROBOTS_WATCHBOT_TRIGGER_PATH_MODE
            && self.current_path_uid_or_sentinel() != contract.path_uid
        {
            self.current_path_uid_488 = if contract.path_uid == ROBOTS_WATCHBOT_NO_PATH_UID {
                None
            } else if path_resolved {
                Some(contract.path_uid)
            } else {
                None
            };
            step.path_changed = true;
        }
        step.owner_trigger_key = self.owner_trigger_key_48c;
        step.active_trigger_count = self.active_trigger_count_510;
        step
    }

    /// Common request path used by recovered WatchBot callers. Native callers in
    /// these paths pass defer=1; request-side GameWnd state may later force it to 0.
    pub fn request_component_mode(&mut self, mode: u32) -> bool {
        self.request_component_mode_with_context(mode, None, true, false, None)
    }

    /// Exact request-side projection for `0x0048FF20` / `0x004900D0`.
    pub fn request_component_mode_with_context(
        &mut self,
        mode: u32,
        snapshot: Option<RobotsWatchbotComponentTransitionSnapshotBits>,
        requested_deferred: bool,
        special_game_state_7b3021: bool,
        state_stack_top: Option<i32>,
    ) -> bool {
        if !self.owner_xitem_exists_550
            || !self.handler_attached
            || !(ROBOTS_WATCHBOT_COMPONENT_MODE_MIN..=ROBOTS_WATCHBOT_COMPONENT_MODE_MAX)
                .contains(&mode)
        {
            return false;
        }
        self.pending_component_mode_4a4 = mode;
        self.pending_component_deferred_518 =
            watchbot_component_transition_deferred_after_state_gate(
                requested_deferred,
                special_game_state_7b3021,
                state_stack_top,
            );
        self.pending_component_snapshot = snapshot;
        true
    }

    /// Handler target-binding vslot +0xD0 (`0x00490850`). Native writes the same
    /// target XItem into every populated component slot (+0x494/+0x498/+0x49C), then
    /// requests component mode1 with deferred flag 1.
    pub fn bind_component_target(&mut self, target_key: u64) -> bool {
        if !self.owner_xitem_exists_550 || !self.handler_attached {
            return false;
        }
        self.component_target_key_28 = Some(target_key);
        self.request_component_mode(ROBOTS_WATCHBOT_TARGET_FOLLOW_COMPONENT_MODE)
    }

    /// Per-service transition reducer for Handler `0x00490590`. It emits host
    /// effects without owning GameWnd; commit mutates only Handler current/pending lanes.
    pub fn service_component_mode_transition(
        &mut self,
        gate: RobotsWatchbotComponentTransitionGate,
    ) -> Option<RobotsWatchbotComponentTransitionStep> {
        if !gate.game_window_exists {
            return None;
        }
        let previous_mode = self.current_component_mode_4a0;
        let requested_mode = self.pending_component_mode_4a4;
        if requested_mode == 0
            || requested_mode == previous_mode
            || !(ROBOTS_WATCHBOT_COMPONENT_MODE_MIN..=ROBOTS_WATCHBOT_COMPONENT_MODE_MAX)
                .contains(&requested_mode)
        {
            return None;
        }

        let old_component_exists = (ROBOTS_WATCHBOT_COMPONENT_MODE_MIN
            ..=ROBOTS_WATCHBOT_COMPONENT_MODE_MAX)
            .contains(&previous_mode);
        let seamless_mode2_mode3 = matches!(
            (previous_mode, requested_mode),
            (
                ROBOTS_WATCHBOT_CONTROL_COMPONENT_MODE,
                ROBOTS_WATCHBOT_TRIGGER_PATH_MODE
            ) | (
                ROBOTS_WATCHBOT_TRIGGER_PATH_MODE,
                ROBOTS_WATCHBOT_CONTROL_COMPONENT_MODE
            )
        );
        let deferred = self.pending_component_deferred_518;
        let exit_previous_component = old_component_exists
            && previous_mode != 0
            && (!deferred || gate.game_window_phase_338 == 0);
        let release_control = exit_previous_component
            && matches!(
                previous_mode,
                ROBOTS_WATCHBOT_CONTROL_COMPONENT_MODE | ROBOTS_WATCHBOT_TRIGGER_PATH_MODE
            )
            && requested_mode == ROBOTS_WATCHBOT_TARGET_FOLLOW_COMPONENT_MODE;

        let waiting_for_host_phase = deferred
            && old_component_exists
            && !seamless_mode2_mode3
            && (ROBOTS_WATCHBOT_COMPONENT_TRANSITION_EPSILON <= gate.game_window_scalar_3b0
                || gate.game_window_phase_338 != 2);
        let commit_transition = !waiting_for_host_phase;
        let host_transition_phase = if deferred && old_component_exists && !seamless_mode2_mode3 {
            if commit_transition {
                Some(1)
            } else if exit_previous_component {
                Some(2)
            } else {
                None
            }
        } else {
            None
        };

        let step = RobotsWatchbotComponentTransitionStep {
            previous_mode,
            requested_mode,
            seamless_mode2_mode3,
            exit_previous_component,
            host_transition_phase,
            request_game_control_mode_zero: release_control,
            request_player_watchbot_release: release_control && gate.player_watchbot_handler_exists,
            commit_transition,
            setup_new_component: commit_transition,
            transition_snapshot: self.pending_component_snapshot,
        };

        if commit_transition {
            self.current_component_mode_4a0 = requested_mode;
            self.pending_component_mode_4a4 = 0;
            self.pending_component_deferred_518 = true;
            self.pending_component_snapshot = None;
        }
        Some(step)
    }

    /// Low-level commit tail retained for deterministic tests/hosts that already
    /// performed the `0x00490590` GameWnd handshake externally.
    pub fn complete_component_mode_transition(&mut self) -> Option<u32> {
        let pending = self.pending_component_mode_4a4;
        if !(ROBOTS_WATCHBOT_COMPONENT_MODE_MIN..=ROBOTS_WATCHBOT_COMPONENT_MODE_MAX)
            .contains(&pending)
        {
            return None;
        }
        self.current_component_mode_4a0 = pending;
        self.pending_component_mode_4a4 = 0;
        self.pending_component_deferred_518 = true;
        self.pending_component_snapshot = None;
        Some(pending)
    }
}

impl RobotsWatchbotTriggerRuntime {
    fn finish_step(
        owner: &RobotsWatchbotOwnerRuntime,
        mut step: RobotsWatchbotTriggerStep,
    ) -> RobotsWatchbotTriggerStep {
        step.owner_trigger_key = owner.owner_trigger_key_48c;
        step.active_trigger_count = owner.active_trigger_count_510;
        step
    }

    fn set_active(
        &mut self,
        trigger_key: u64,
        contract: RobotsWatchbotTriggerContract,
        owner: &mut RobotsWatchbotOwnerRuntime,
        active: bool,
        path_resolved: bool,
    ) -> RobotsWatchbotTriggerStep {
        self.active_e5 = active;
        if active {
            let step = owner.apply_trigger_contract(contract, path_resolved);
            return Self::finish_step(owner, step);
        }

        let mut step = RobotsWatchbotTriggerStep::default();
        if owner.owner_xitem_exists_550
            && owner.handler_attached
            && owner.trigger_matches(trigger_key, contract)
        {
            owner.active_trigger_count_510 = owner.active_trigger_count_510.wrapping_sub(1);
            if owner.active_trigger_count_510 == 0 {
                owner.owner_trigger_key_48c = None;
                // `0x00490B30` stores null at +0x488. Its second flag argument is
                // ignored by the native body in this executable.
                owner.current_path_uid_488 = None;
                step.path_changed = true;
                if owner.request_component_mode(ROBOTS_WATCHBOT_CONTROL_COMPONENT_MODE) {
                    step.requested_component_mode = Some(ROBOTS_WATCHBOT_CONTROL_COMPONENT_MODE);
                }
            }
        }
        Self::finish_step(owner, step)
    }

    /// Exact class-event ordering from `0x00483A90`: activate is handled first,
    /// then deactivate independently, so a combined 0x300 event ends inactive.
    pub fn dispatch_event(
        &mut self,
        trigger_key: u64,
        contract: RobotsWatchbotTriggerContract,
        owner: &mut RobotsWatchbotOwnerRuntime,
        event_mask: u32,
        path_resolved: bool,
    ) -> RobotsWatchbotTriggerStep {
        let mut result = RobotsWatchbotTriggerStep::default();
        if event_mask & 0x100 != 0 {
            result = self.set_active(trigger_key, contract, owner, true, path_resolved);
        }
        if event_mask & 0x200 != 0 {
            let deactivated = self.set_active(trigger_key, contract, owner, false, path_resolved);
            if deactivated.requested_component_mode.is_some() {
                result.requested_component_mode = deactivated.requested_component_mode;
            }
            result.path_changed |= deactivated.path_changed;
            result.owner_trigger_key = deactivated.owner_trigger_key;
            result.active_trigger_count = deactivated.active_trigger_count;
        }
        Self::finish_step(owner, result)
    }

    /// Fixed update `0x00483CE0`. `distance_squared` is measured from the live
    /// controlled WatchBot XItem, not from Rodney/Player.
    pub fn fixed_update(
        &mut self,
        trigger_key: u64,
        contract: RobotsWatchbotTriggerContract,
        owner: &mut RobotsWatchbotOwnerRuntime,
        distance_squared: f32,
        path_resolved: bool,
    ) -> RobotsWatchbotTriggerStep {
        if !self.enabled_e4 || !owner.owner_xitem_exists_550 || !owner.handler_attached {
            return Self::finish_step(owner, RobotsWatchbotTriggerStep::default());
        }

        if !self.active_e5 {
            if !contract.should_enter(distance_squared) {
                return Self::finish_step(owner, RobotsWatchbotTriggerStep::default());
            }
            let mut step = self.set_active(trigger_key, contract, owner, true, path_resolved);
            step.entered = true;
            if !owner.trigger_matches(trigger_key, contract) {
                owner.owner_trigger_key_48c = Some(trigger_key);
                owner.active_trigger_count_510 = 0;
            }
            owner.active_trigger_count_510 = owner.active_trigger_count_510.wrapping_add(1);
            return Self::finish_step(owner, step);
        }

        if contract.should_leave(distance_squared) {
            let mut step = self.set_active(trigger_key, contract, owner, false, path_resolved);
            step.exited = true;
            return Self::finish_step(owner, step);
        }
        if owner.active_trigger_count_510 != 0 {
            return Self::finish_step(owner, RobotsWatchbotTriggerStep::default());
        }

        let mut step = owner.apply_trigger_contract(contract, path_resolved);
        owner.owner_trigger_key_48c = Some(trigger_key);
        owner.active_trigger_count_510 = owner.active_trigger_count_510.wrapping_add(1);
        step.owner_trigger_key = owner.owner_trigger_key_48c;
        step.active_trigger_count = owner.active_trigger_count_510;
        step
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path_contract() -> RobotsWatchbotTriggerContract {
        RobotsWatchbotTriggerContract {
            mode: ROBOTS_WATCHBOT_TRIGGER_PATH_MODE,
            path_uid: 0x0B00_0123,
            flags: 3,
            enter_distance_raw: 15,
            leave_distance_raw: 30,
        }
    }

    #[test]
    fn trigger_distance_hysteresis_matches_native_inclusive_enter_and_strict_leave() {
        let contract = path_contract();
        let enter_squared = contract.enter_distance_squared();
        assert!(contract.should_enter(enter_squared));
        assert!(!contract.should_enter(f32::from_bits(enter_squared.to_bits() + 1)));
        let leave_squared = contract.leave_distance_squared().unwrap();
        assert!(!contract.should_leave(leave_squared));
        assert!(contract.should_leave(f32::from_bits(leave_squared.to_bits() + 1)));

        let mut no_leave = contract;
        no_leave.leave_distance_raw = 0;
        assert_eq!(no_leave.leave_distance_squared(), None);
        no_leave.leave_distance_raw = no_leave.enter_distance_raw;
        assert_eq!(no_leave.leave_distance_squared(), None);
        no_leave.leave_distance_raw = -30;
        assert_eq!(no_leave.leave_distance_squared(), None);
    }

    #[test]
    fn path_trigger_enter_hold_and_leave_update_shared_watchbot_owner_exactly() {
        let contract = path_contract();
        let mut owner = RobotsWatchbotOwnerRuntime::default();
        assert!(owner.service_player_owner(true, 0));
        owner.current_component_mode_4a0 = ROBOTS_WATCHBOT_TRIGGER_PATH_MODE;
        let mut trigger = RobotsWatchbotTriggerRuntime::default();

        let entered = trigger.fixed_update(7, contract, &mut owner, 2.25, true);
        assert!(entered.entered);
        assert!(trigger.active_e5);
        assert_eq!(owner.current_path_uid_488, Some(contract.path_uid));
        assert_eq!(owner.active_trigger_count_510, 1);
        assert_eq!(owner.pending_component_mode_4a4, 0);

        let held = trigger.fixed_update(7, contract, &mut owner, 4.0, true);
        assert!(!held.entered && !held.exited);
        assert_eq!(owner.active_trigger_count_510, 1);

        let exited = trigger.fixed_update(7, contract, &mut owner, 10.0, true);
        assert!(exited.exited);
        assert!(!trigger.active_e5);
        assert_eq!(owner.active_trigger_count_510, 0);
        assert_eq!(owner.current_path_uid_488, None);
        assert_eq!(
            owner.pending_component_mode_4a4,
            ROBOTS_WATCHBOT_CONTROL_COMPONENT_MODE
        );
    }

    #[test]
    fn compatible_path_triggers_share_native_510_refcount_before_cleanup() {
        let contract = path_contract();
        let mut owner = RobotsWatchbotOwnerRuntime::default();
        assert!(owner.service_player_owner(true, 0));
        owner.current_component_mode_4a0 = ROBOTS_WATCHBOT_TRIGGER_PATH_MODE;
        let mut first = RobotsWatchbotTriggerRuntime::default();
        let mut second = RobotsWatchbotTriggerRuntime::default();

        first.fixed_update(1, contract, &mut owner, 1.0, true);
        second.fixed_update(2, contract, &mut owner, 1.0, true);
        assert_eq!(owner.active_trigger_count_510, 2);
        assert_eq!(owner.current_path_uid_488, Some(contract.path_uid));

        first.dispatch_event(1, contract, &mut owner, 0x200, true);
        assert_eq!(owner.active_trigger_count_510, 1);
        assert_eq!(owner.current_path_uid_488, Some(contract.path_uid));
        assert_eq!(owner.pending_component_mode_4a4, 0);

        second.dispatch_event(2, contract, &mut owner, 0x200, true);
        assert_eq!(owner.active_trigger_count_510, 0);
        assert_eq!(owner.current_path_uid_488, None);
        assert_eq!(
            owner.pending_component_mode_4a4,
            ROBOTS_WATCHBOT_CONTROL_COMPONENT_MODE
        );
    }

    #[test]
    fn player_watchbot_owner_creation_and_pending_mode_gate_match_native_contract() {
        let mut runtime = RobotsWatchbotOwnerRuntime::default();
        assert!(!runtime.service_player_owner(false, 0));
        assert!(!runtime.control_entry_ready());
        for mode in [2, 3, 4, 6] {
            assert!(!runtime.service_player_owner(true, mode));
        }

        assert!(runtime.service_player_owner(true, 0));
        assert!(runtime.control_entry_ready());
        assert!(runtime.request_component_mode(ROBOTS_WATCHBOT_CONTROL_COMPONENT_MODE));
        assert!(!runtime.control_entry_ready());
        assert_eq!(runtime.complete_component_mode_transition(), Some(2));
        assert_eq!(runtime.current_component_mode_4a0, 2);
        assert!(runtime.control_entry_ready());

        runtime.clear_player_owner();
        assert!(!runtime.control_entry_ready());
    }

    #[test]
    fn handler_target_binding_is_generic_and_requests_mode1() {
        let mut runtime = RobotsWatchbotOwnerRuntime::default();
        assert!(!runtime.bind_component_target(0x1234));
        assert!(runtime.service_player_owner(true, 0));
        assert!(runtime.bind_component_target(0x1234));
        assert_eq!(runtime.component_target_key_28, Some(0x1234));
        assert_eq!(
            runtime.pending_component_mode_4a4,
            ROBOTS_WATCHBOT_TARGET_FOLLOW_COMPONENT_MODE
        );
        assert!(runtime.pending_component_deferred_518);
    }

    #[test]
    fn component_request_gate_and_valid_mode_range_match_native_component_array() {
        assert!(!watchbot_component_transition_deferred_after_state_gate(
            true,
            false,
            Some(2)
        ));
        assert!(!watchbot_component_transition_deferred_after_state_gate(
            true,
            true,
            Some(1)
        ));
        assert!(watchbot_component_transition_deferred_after_state_gate(
            true,
            false,
            Some(1)
        ));
        assert!(!watchbot_component_transition_deferred_after_state_gate(
            false, false, None
        ));

        let mut runtime = RobotsWatchbotOwnerRuntime::default();
        assert!(runtime.service_player_owner(true, 0));
        assert!(!runtime.request_component_mode(0));
        assert!(!runtime.request_component_mode(4));
        assert_eq!(runtime.pending_component_mode_4a4, 0);
    }

    #[test]
    fn deferred_nonseamless_transition_exits_then_waits_and_commits_on_phase_two() {
        let mut runtime = RobotsWatchbotOwnerRuntime::default();
        assert!(runtime.service_player_owner(true, 0));
        runtime.current_component_mode_4a0 = ROBOTS_WATCHBOT_CONTROL_COMPONENT_MODE;
        assert!(runtime.request_component_mode(ROBOTS_WATCHBOT_TARGET_FOLLOW_COMPONENT_MODE));

        let first = runtime
            .service_component_mode_transition(RobotsWatchbotComponentTransitionGate {
                game_window_exists: true,
                game_window_phase_338: 0,
                game_window_scalar_3b0: 1.0,
                player_watchbot_handler_exists: true,
            })
            .unwrap();
        assert!(first.exit_previous_component);
        assert_eq!(first.host_transition_phase, Some(2));
        assert!(first.request_game_control_mode_zero);
        assert!(first.request_player_watchbot_release);
        assert!(!first.commit_transition);
        assert_eq!(
            runtime.current_component_mode_4a0,
            ROBOTS_WATCHBOT_CONTROL_COMPONENT_MODE
        );
        assert_eq!(
            runtime.pending_component_mode_4a4,
            ROBOTS_WATCHBOT_TARGET_FOLLOW_COMPONENT_MODE
        );

        let committed = runtime
            .service_component_mode_transition(RobotsWatchbotComponentTransitionGate {
                game_window_exists: true,
                game_window_phase_338: 2,
                game_window_scalar_3b0: 0.0,
                player_watchbot_handler_exists: true,
            })
            .unwrap();
        assert!(!committed.exit_previous_component);
        assert_eq!(committed.host_transition_phase, Some(1));
        assert!(committed.commit_transition);
        assert_eq!(
            runtime.current_component_mode_4a0,
            ROBOTS_WATCHBOT_TARGET_FOLLOW_COMPONENT_MODE
        );
        assert_eq!(runtime.pending_component_mode_4a4, 0);
    }

    #[test]
    fn mode2_mode3_transition_is_seamless_and_commits_without_host_phase_wait() {
        let mut runtime = RobotsWatchbotOwnerRuntime::default();
        assert!(runtime.service_player_owner(true, 0));
        runtime.current_component_mode_4a0 = ROBOTS_WATCHBOT_CONTROL_COMPONENT_MODE;
        assert!(runtime.request_component_mode(ROBOTS_WATCHBOT_TRIGGER_PATH_MODE));
        let step = runtime
            .service_component_mode_transition(RobotsWatchbotComponentTransitionGate {
                game_window_exists: true,
                game_window_phase_338: 0,
                game_window_scalar_3b0: 1.0,
                player_watchbot_handler_exists: false,
            })
            .unwrap();
        assert!(step.seamless_mode2_mode3);
        assert!(step.exit_previous_component);
        assert_eq!(step.host_transition_phase, None);
        assert!(step.commit_transition);
        assert_eq!(
            runtime.current_component_mode_4a0,
            ROBOTS_WATCHBOT_TRIGGER_PATH_MODE
        );
    }
}
