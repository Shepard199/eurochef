use serde::Serialize;

pub const ROBOTS_FIX_SWITCH_INTERACTION_RADIUS: f32 = 2.5;
pub const ROBOTS_FIX_SWITCH_SERVICE_EVENT: u32 = 0x0000_1000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct RobotsFixSwitchRuntimeState {
    pub initialized: bool,
    pub state_e4: u8,
    pub active_e8: u8,
    pub timer_ec: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RobotsFixSwitchEventStep {
    /// Native XTrigger_FixSwitch::Event marks live Handler +0x3C4 so the
    /// FixSwitch XItem reapplies visual progress on its next handler update.
    pub mark_owned_handler_progress_dirty: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct RobotsFixSwitchProgressStep {
    /// Native 0x0048A930 return value: timer/duration clamped by the exact
    /// x87 comparison order to [0, 1], or 0 when duration is not positive.
    pub normalized_progress: f32,
    /// Completion sets trigger bit20; the common deferred eight-link consumer
    /// owns the actual output dispatch.
    pub request_deferred_output: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RobotsFixSwitchVisualAction {
    #[default]
    None,
    /// Native animator vslot +0x20 with argument zero. Its deeper presentation
    /// meaning is intentionally left engine-owned until that generic vslot is named.
    InvokeAnimatorControlZero,
    /// First valid application routes the interpolated frame through the already
    /// recovered EXItemAnimator_Script seek path ending at 0x004FA74C.
    SeekScriptFrameBits { frame_bits: u32 },
    /// Subsequent applications call the current animator vslot +0x24 with the
    /// interpolated frame divided by the resolver-owned +0x0C scalar. Keep the
    /// scalar name neutral rather than inventing a time/FPS semantic.
    SetAnimatorScalarBits { scalar_bits: u32 },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RobotsFixSwitchVisualApplyPlan {
    pub action: RobotsFixSwitchVisualAction,
    pub clear_dirty_latch_3c4: bool,
    pub clear_first_apply_latch_3c5: bool,
    pub store_last_progress_bits_3c0: Option<u32>,
    pub clear_owner_animator_flag_0x02: bool,
}

/// XItemHandler_FixSwitch progress applier 0x00412B90. The host owns the live
/// animator and resolver; shared owns only the exact branch/math/latch contract.
pub fn plan_fix_switch_visual_apply(
    normalized_progress: f32,
    first_marker_frame: Option<i32>,
    second_marker_frame: Option<i32>,
    first_apply: bool,
    resolved_animator_divisor: Option<f32>,
) -> RobotsFixSwitchVisualApplyPlan {
    let (Some(first_marker_frame), Some(second_marker_frame)) =
        (first_marker_frame, second_marker_frame)
    else {
        return RobotsFixSwitchVisualApplyPlan {
            action: RobotsFixSwitchVisualAction::InvokeAnimatorControlZero,
            ..RobotsFixSwitchVisualApplyPlan::default()
        };
    };

    // Native x87 order is FILD first, FILD second, FSUB first, FMUL progress,
    // FADD first. Preserve that explicit order rather than algebraically folding it.
    let first = first_marker_frame as f32;
    let second = second_marker_frame as f32;
    let delta = second - first;
    let interpolated_frame = delta * normalized_progress + first;

    let (action, clear_first_apply_latch_3c5) = if first_apply {
        (
            RobotsFixSwitchVisualAction::SeekScriptFrameBits {
                frame_bits: interpolated_frame.to_bits(),
            },
            true,
        )
    } else {
        (
            resolved_animator_divisor
                .map(
                    |divisor| RobotsFixSwitchVisualAction::SetAnimatorScalarBits {
                        scalar_bits: (interpolated_frame / divisor).to_bits(),
                    },
                )
                .unwrap_or_default(),
            false,
        )
    };

    RobotsFixSwitchVisualApplyPlan {
        action,
        clear_dirty_latch_3c4: true,
        clear_first_apply_latch_3c5,
        store_last_progress_bits_3c0: Some(normalized_progress.to_bits()),
        clear_owner_animator_flag_0x02: true,
    }
}

impl RobotsFixSwitchRuntimeState {
    /// XTrigger_FixSwitch initialiser 0x0048A910.
    pub fn initialize(&mut self, serialized_data0: u32) {
        self.initialized = true;
        self.active_e8 = 1;
        self.state_e4 = serialized_data0 as u8;
        self.timer_ec = 0.0;
    }

    /// XTrigger_FixSwitch +0x6C = 0x0048A9D0 local service branch.
    /// Common trigger-mask semantics stay in the ordinary trigger dispatcher.
    pub fn dispatch_trigger_event(
        &mut self,
        serialized_data0: u32,
        event_mask: u32,
        owned_handler_present: bool,
    ) -> RobotsFixSwitchEventStep {
        if event_mask & ROBOTS_FIX_SWITCH_SERVICE_EVENT == 0 {
            return RobotsFixSwitchEventStep::default();
        }
        self.initialize(serialized_data0);
        RobotsFixSwitchEventStep {
            mark_owned_handler_progress_dirty: owned_handler_present,
        }
    }

    /// Trigger progress helper 0x0048A930. Native advances this only from the
    /// live FixSwitch Handler interaction/update path, not from TriggerManager.
    pub fn advance_progress(
        &mut self,
        duration_seconds: f32,
        fixed_delta_seconds: f32,
    ) -> RobotsFixSwitchProgressStep {
        let next_timer = self.timer_ec + fixed_delta_seconds;
        self.timer_ec = next_timer;

        let mut normalized_progress = 0.0;
        if 0.0 < duration_seconds {
            let ratio = next_timer / duration_seconds;
            if 0.0 <= ratio {
                normalized_progress = if 1.0 < ratio { 1.0 } else { ratio };
            }
        }

        let mut request_deferred_output = false;
        if duration_seconds < next_timer {
            self.active_e8 = 0;
            self.timer_ec = duration_seconds;
            request_deferred_output = self.state_e4 == 1;
        }

        RobotsFixSwitchProgressStep {
            normalized_progress,
            request_deferred_output,
        }
    }

    /// XTrigger_FixSwitch vslot +0xF8 = 0x0048B3B0.
    pub const fn interaction_active(&self) -> bool {
        self.active_e8 == 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_and_service_event_preserve_native_trigger_state_and_handler_dirty_edge() {
        let mut state = RobotsFixSwitchRuntimeState::default();
        state.initialize(0x102);
        assert!(state.initialized);
        assert_eq!(state.state_e4, 2);
        assert_eq!(state.active_e8, 1);
        assert_eq!(state.timer_ec, 0.0);
        assert!(state.interaction_active());

        state.timer_ec = 4.0;
        let ignored = state.dispatch_trigger_event(1, 0x100, true);
        assert_eq!(ignored, RobotsFixSwitchEventStep::default());
        assert_eq!(state.timer_ec, 4.0);

        let reset = state.dispatch_trigger_event(1, ROBOTS_FIX_SWITCH_SERVICE_EVENT, true);
        assert!(reset.mark_owned_handler_progress_dirty);
        assert_eq!(state.state_e4, 1);
        assert_eq!(state.active_e8, 1);
        assert_eq!(state.timer_ec, 0.0);
    }

    #[test]
    fn progress_uses_strict_completion_and_queues_deferred_output_only_for_state_one() {
        let mut state = RobotsFixSwitchRuntimeState::default();
        state.initialize(1);

        let first = state.advance_progress(0.03, 0.02);
        assert!((first.normalized_progress - (2.0 / 3.0)).abs() < 0.000_001);
        assert!(!first.request_deferred_output);
        assert!(state.interaction_active());

        let completed = state.advance_progress(0.03, 0.02);
        assert_eq!(completed.normalized_progress, 1.0);
        assert!(completed.request_deferred_output);
        assert_eq!(state.active_e8, 0);
        assert_eq!(state.timer_ec, 0.03);

        state.initialize(0);
        let completed_inactive = state.advance_progress(0.01, 0.02);
        assert!(!completed_inactive.request_deferred_output);
        assert_eq!(state.active_e8, 0);
    }

    #[test]
    fn nonpositive_duration_matches_native_zero_progress_then_strict_completion() {
        let mut state = RobotsFixSwitchRuntimeState::default();
        state.initialize(1);
        let step = state.advance_progress(0.0, 1.0 / 60.0);
        assert_eq!(step.normalized_progress, 0.0);
        assert!(step.request_deferred_output);
        assert_eq!(state.timer_ec, 0.0);
        assert_eq!(state.active_e8, 0);
    }

    #[test]
    fn visual_first_apply_interpolates_markers_and_requests_existing_script_seek() {
        let plan = plan_fix_switch_visual_apply(0.25, Some(10), Some(30), true, None);
        assert_eq!(
            plan.action,
            RobotsFixSwitchVisualAction::SeekScriptFrameBits {
                frame_bits: 15.0f32.to_bits(),
            }
        );
        assert!(plan.clear_dirty_latch_3c4);
        assert!(plan.clear_first_apply_latch_3c5);
        assert_eq!(plan.store_last_progress_bits_3c0, Some(0.25f32.to_bits()));
        assert!(plan.clear_owner_animator_flag_0x02);
    }

    #[test]
    fn visual_subsequent_apply_uses_resolver_divisor_for_animator_scalar() {
        let plan = plan_fix_switch_visual_apply(0.5, Some(10), Some(30), false, Some(20.0));
        assert_eq!(
            plan.action,
            RobotsFixSwitchVisualAction::SetAnimatorScalarBits {
                scalar_bits: 1.0f32.to_bits(),
            }
        );
        assert!(plan.clear_dirty_latch_3c4);
        assert!(!plan.clear_first_apply_latch_3c5);
        assert!(plan.clear_owner_animator_flag_0x02);
    }

    #[test]
    fn visual_valid_markers_without_resolver_still_commit_native_local_bookkeeping() {
        let plan = plan_fix_switch_visual_apply(0.5, Some(10), Some(30), false, None);
        assert_eq!(plan.action, RobotsFixSwitchVisualAction::None);
        assert!(plan.clear_dirty_latch_3c4);
        assert_eq!(plan.store_last_progress_bits_3c0, Some(0.5f32.to_bits()));
        assert!(plan.clear_owner_animator_flag_0x02);
    }

    #[test]
    fn visual_missing_two_marker_rows_invokes_control_zero_without_clearing_dirty() {
        let plan = plan_fix_switch_visual_apply(0.5, Some(10), None, true, Some(20.0));
        assert_eq!(
            plan.action,
            RobotsFixSwitchVisualAction::InvokeAnimatorControlZero
        );
        assert!(!plan.clear_dirty_latch_3c4);
        assert!(!plan.clear_first_apply_latch_3c5);
        assert_eq!(plan.store_last_progress_bits_3c0, None);
        assert!(!plan.clear_owner_animator_flag_0x02);
    }
}
