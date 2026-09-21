use serde::Serialize;

use super::events::RobotsDoorCommandKind;

pub const ROBOTS_DOOR_DISTANCE_SCALE: f32 = 0.1;
pub const ROBOTS_DOOR_DISTANCE_FALLBACK_SQUARED: f32 = 99_999.0;
pub const ROBOTS_DOOR_HINT_TEXT_NONE_UID: u32 = 0x4500_0000;
pub const ROBOTS_DOOR_FIXED_RETURN_VALUE: u32 = 3;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RobotsDoorRuntimeState {
    pub initialized: bool,
    pub state_e4: u8,
    pub request_e5: u8,
    pub proximity_latch_ec: u8,
    pub request_ed: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsDoorDistanceQueryPlan {
    pub direct_player: bool,
    pub runtime_category_scan_order: [Option<u8>; 4],
    pub horizontal_only: bool,
}

impl RobotsDoorDistanceQueryPlan {
    /// XTrigger_Door helper 0x00489600. data[3] is the source selector and
    /// data[4] bit1 switches the shared distance helper from XYZ to XZ only.
    /// Selector 4 really scans category 0 twice; preserve that native duplication.
    pub const fn from_serialized(serialized_data3: u32, serialized_data4: u32) -> Self {
        let (direct_player, runtime_category_scan_order) = match serialized_data3 {
            0 => (true, [None, None, None, None]),
            1 => (false, [Some(0), None, None, None]),
            2 => (false, [Some(1), None, None, None]),
            3 => (false, [Some(3), None, None, None]),
            4 => (false, [Some(0), Some(1), Some(3), Some(0)]),
            _ => (false, [None, None, None, None]),
        };
        Self {
            direct_player,
            runtime_category_scan_order,
            horizontal_only: serialized_data4 & 2 != 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsDoorFixedHostInput {
    /// Result of Door nearest-target helper 0x00489600. Use 99999.0 when no
    /// target is found, matching the caller's native local seed.
    pub nearest_distance_squared: f32,
    /// XZ distance from gameplay-viewpoint XItem global 0x007B25B0. None keeps
    /// the native 99999.0 fallback.
    pub viewpoint_distance_squared: Option<f32>,
    /// Handler +0xB08 for that viewpoint XItem, when its Handler exists.
    pub viewpoint_control_mode: Option<u32>,
    /// XZ distance from native camera globals 0x007B2BC0/0x007B2BC8.
    pub camera_distance_squared: f32,
    /// XZ distance from current Player Handler +0x550 auxiliary XItem, if any.
    pub player_aux_distance_squared: Option<f32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RobotsDoorFixedStep {
    pub return_value: u32,
    pub show_hint_text_uid: Option<u32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RobotsDoorScriptCommandResult {
    pub return_value: u32,
    /// Native helper 0x004891B0 conditionally synchronizes a linked door record.
    /// `Some(value)` means the helper was invoked and the linked record, when
    /// present/valid, receives this closed-state value.
    pub sync_linked_closed_state: Option<bool>,
}

impl RobotsDoorRuntimeState {
    /// XTrigger_Door::Initialise 0x00488F70:
    /// +E4=0, +E5=low_byte(data[0]), +EC=0, +ED=0.
    pub fn initialize(&mut self, serialized_data0: u32) {
        self.initialized = true;
        self.state_e4 = 0;
        self.request_e5 = serialized_data0 as u8;
        self.proximity_latch_ec = 0;
        self.request_ed = 0;
    }

    /// XTrigger_Door gameplay tick 0x004892D0 after the host has resolved the
    /// 0x00489600 nearest-object query and the explicit XZ close guards. data[1]/data[2]
    /// are signed tenths-of-a-unit radii and data[5] is the optional Text UID used
    /// by the one-shot proximity hint latch at +0xEC.
    pub fn advance_fixed(
        &mut self,
        serialized_data1: u32,
        serialized_data2: u32,
        serialized_data5: u32,
        host: RobotsDoorFixedHostInput,
    ) -> RobotsDoorFixedStep {
        let mut step = RobotsDoorFixedStep {
            return_value: ROBOTS_DOOR_FIXED_RETURN_VALUE,
            show_hint_text_uid: None,
        };
        let open_radius_squared = door_radius_squared(serialized_data1);

        if self.request_e5 == 1 {
            let valid_hint = !matches!(
                serialized_data5,
                0 | u32::MAX | ROBOTS_DOOR_HINT_TEXT_NONE_UID
            );
            if valid_hint {
                if self.proximity_latch_ec == 0 {
                    if host.nearest_distance_squared < open_radius_squared {
                        self.proximity_latch_ec = 1;
                        step.show_hint_text_uid = Some(serialized_data5);
                    }
                } else if open_radius_squared <= host.nearest_distance_squared {
                    self.proximity_latch_ec = 0;
                }
            }
            return step;
        }

        self.proximity_latch_ec = 0;
        if self.state_e4 == 1 {
            if self.request_ed != 0 {
                self.state_e4 = 0;
                return step;
            }

            let close_radius_squared = door_radius_squared(serialized_data2);
            if close_radius_squared <= host.nearest_distance_squared {
                let viewpoint_distance = host
                    .viewpoint_distance_squared
                    .unwrap_or(ROBOTS_DOOR_DISTANCE_FALLBACK_SQUARED);
                if close_radius_squared <= viewpoint_distance {
                    if host.viewpoint_control_mode == Some(2) {
                        return step;
                    }
                    if host.camera_distance_squared < viewpoint_distance {
                        return step;
                    }
                }
                if host
                    .player_aux_distance_squared
                    .is_some_and(|distance| distance < close_radius_squared)
                {
                    return step;
                }
                self.state_e4 = 0;
            }
            return step;
        }

        if self.request_ed == 0 && host.nearest_distance_squared < open_radius_squared {
            self.state_e4 = 1;
        }
        step
    }

    /// XTrigger_Door::Event 0x00489260. Event bits modify request latches;
    /// actual +E4 motion/state advancement belongs to the Door gameplay tick.
    pub fn dispatch_trigger_event(&mut self, serialized_data0: u32, event_mask: u32) {
        if !self.initialized {
            self.initialize(serialized_data0);
        }
        if event_mask & 0x400 != 0 {
            if self.state_e4 == 1 {
                self.request_ed = 1;
            } else {
                self.request_e5 = 1;
            }
        }
        if event_mask & 0x800 != 0 {
            self.request_ed = 0;
            self.request_e5 = 0;
        }
    }

    /// XItemHandler_Door::HandleScriptCmdEvent 0x0040CE20 local cases.
    /// The returned value is the exact native Script-command result for the
    /// recovered Door state. Linked-record writes are emitted as a host action
    /// instead of hard-wiring an engine object reference into shared runtime.
    pub fn dispatch_script_command(
        &mut self,
        command: RobotsDoorCommandKind,
    ) -> RobotsDoorScriptCommandResult {
        match command {
            RobotsDoorCommandKind::Open => {
                if self.state_e4 == 1 {
                    RobotsDoorScriptCommandResult {
                        return_value: 0,
                        sync_linked_closed_state: Some(false),
                    }
                } else {
                    RobotsDoorScriptCommandResult {
                        return_value: 1,
                        sync_linked_closed_state: None,
                    }
                }
            }
            RobotsDoorCommandKind::Close => RobotsDoorScriptCommandResult {
                return_value: u32::from(self.state_e4 != 0),
                sync_linked_closed_state: None,
            },
            RobotsDoorCommandKind::Opened => RobotsDoorScriptCommandResult::default(),
            RobotsDoorCommandKind::Closed => {
                let result = RobotsDoorScriptCommandResult {
                    return_value: 0,
                    sync_linked_closed_state: Some(self.state_e4 == 0),
                };
                if self.request_ed != 0 {
                    self.request_e5 = 1;
                    self.request_ed = 0;
                }
                result
            }
        }
    }
}

fn door_radius_squared(serialized_tenths: u32) -> f32 {
    let radius = (serialized_tenths as i32 as f32 * ROBOTS_DOOR_DISTANCE_SCALE).abs();
    radius * radius
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialization_and_trigger_requests_match_native_e4_e5_ed_contract() {
        let mut state = RobotsDoorRuntimeState::default();
        state.dispatch_trigger_event(3, 0);
        assert!(state.initialized);
        assert_eq!(state.state_e4, 0);
        assert_eq!(state.request_e5, 3);
        assert_eq!(state.request_ed, 0);

        state.dispatch_trigger_event(3, 0x400);
        assert_eq!(state.request_e5, 1);
        assert_eq!(state.request_ed, 0);

        state.state_e4 = 1;
        state.dispatch_trigger_event(3, 0x400);
        assert_eq!(state.request_ed, 1);

        state.dispatch_trigger_event(3, 0x800);
        assert_eq!(state.request_e5, 0);
        assert_eq!(state.request_ed, 0);
    }

    #[test]
    fn open_and_close_preserve_native_wait_results() {
        let mut state = RobotsDoorRuntimeState::default();
        state.initialize(0);

        let open_wait = state.dispatch_script_command(RobotsDoorCommandKind::Open);
        assert_eq!(open_wait.return_value, 1);
        assert_eq!(open_wait.sync_linked_closed_state, None);

        state.state_e4 = 1;
        let open_done = state.dispatch_script_command(RobotsDoorCommandKind::Open);
        assert_eq!(open_done.return_value, 0);
        assert_eq!(open_done.sync_linked_closed_state, Some(false));

        let close_wait = state.dispatch_script_command(RobotsDoorCommandKind::Close);
        assert_eq!(close_wait.return_value, 1);
        state.state_e4 = 0;
        assert_eq!(
            state
                .dispatch_script_command(RobotsDoorCommandKind::Close)
                .return_value,
            0
        );
    }

    #[test]
    fn closed_syncs_linked_state_and_transfers_pending_request_latch() {
        let mut state = RobotsDoorRuntimeState::default();
        state.initialize(0);
        state.request_ed = 1;

        let result = state.dispatch_script_command(RobotsDoorCommandKind::Closed);
        assert_eq!(result.return_value, 0);
        assert_eq!(result.sync_linked_closed_state, Some(true));
        assert_eq!(state.request_e5, 1);
        assert_eq!(state.request_ed, 0);
    }

    fn fixed_host(nearest_distance_squared: f32) -> RobotsDoorFixedHostInput {
        RobotsDoorFixedHostInput {
            nearest_distance_squared,
            viewpoint_distance_squared: Some(100.0),
            viewpoint_control_mode: None,
            camera_distance_squared: 100.0,
            player_aux_distance_squared: None,
        }
    }

    #[test]
    fn distance_query_plan_preserves_native_selector_and_horizontal_flag() {
        let player = RobotsDoorDistanceQueryPlan::from_serialized(0, 0);
        assert!(player.direct_player);
        assert_eq!(player.runtime_category_scan_order, [None; 4]);
        assert!(!player.horizontal_only);

        let all = RobotsDoorDistanceQueryPlan::from_serialized(4, 2);
        assert!(!all.direct_player);
        assert_eq!(
            all.runtime_category_scan_order,
            [Some(0), Some(1), Some(3), Some(0)]
        );
        assert!(all.horizontal_only);
    }

    #[test]
    fn fixed_tick_uses_strict_open_boundary_and_one_shot_hint_latch() {
        let mut state = RobotsDoorRuntimeState::default();
        state.initialize(0);

        state.advance_fixed(20, 30, 0, fixed_host(4.0));
        assert_eq!(state.state_e4, 0);
        state.advance_fixed(20, 30, 0, fixed_host(3.99));
        assert_eq!(state.state_e4, 1);

        state.initialize(1);
        let first = state.advance_fixed(20, 30, 0x4500_008C, fixed_host(3.0));
        assert_eq!(first.return_value, ROBOTS_DOOR_FIXED_RETURN_VALUE);
        assert_eq!(first.show_hint_text_uid, Some(0x4500_008C));
        assert_eq!(state.proximity_latch_ec, 1);

        let held = state.advance_fixed(20, 30, 0x4500_008C, fixed_host(3.0));
        assert_eq!(held.show_hint_text_uid, None);
        assert_eq!(state.proximity_latch_ec, 1);

        state.advance_fixed(20, 30, 0x4500_008C, fixed_host(4.0));
        assert_eq!(state.proximity_latch_ec, 0);
        let rearmed = state.advance_fixed(20, 30, 0x4500_008C, fixed_host(3.0));
        assert_eq!(rearmed.show_hint_text_uid, Some(0x4500_008C));
    }

    #[test]
    fn fixed_tick_close_guards_match_native_order_and_pending_close_preempts_distance() {
        let mut state = RobotsDoorRuntimeState::default();
        state.initialize(0);
        state.state_e4 = 1;
        state.advance_fixed(20, 30, 0, fixed_host(9.0));
        assert_eq!(state.state_e4, 0);

        state.state_e4 = 1;
        let mut host = fixed_host(9.0);
        host.viewpoint_control_mode = Some(2);
        state.advance_fixed(20, 30, 0, host);
        assert_eq!(state.state_e4, 1);

        let mut host = fixed_host(9.0);
        host.camera_distance_squared = 50.0;
        state.advance_fixed(20, 30, 0, host);
        assert_eq!(state.state_e4, 1);

        let mut host = fixed_host(9.0);
        host.viewpoint_distance_squared = Some(4.0);
        host.player_aux_distance_squared = Some(8.0);
        state.advance_fixed(20, 30, 0, host);
        assert_eq!(state.state_e4, 1);

        state.request_ed = 1;
        state.advance_fixed(20, 30, 0, fixed_host(0.0));
        assert_eq!(state.state_e4, 0);
    }

    #[test]
    fn opened_is_a_true_local_noop() {
        let mut state = RobotsDoorRuntimeState {
            initialized: true,
            state_e4: 1,
            request_e5: 7,
            proximity_latch_ec: 5,
            request_ed: 9,
        };
        let before = state;
        let result = state.dispatch_script_command(RobotsDoorCommandKind::Opened);
        assert_eq!(result, RobotsDoorScriptCommandResult::default());
        assert_eq!(state, before);
    }
}
