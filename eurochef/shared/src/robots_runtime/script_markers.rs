use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RobotsScriptMarkerStateSource {
    ContextState { value: f32 },
    MarkerFrame { frame: i16 },
}

impl RobotsScriptMarkerStateSource {
    pub fn resolve(self, frames_per_second: f32) -> Option<f32> {
        match self {
            Self::ContextState { value } => value.is_finite().then_some(value),
            Self::MarkerFrame { frame } => (frames_per_second.is_finite()
                && frames_per_second > 0.0)
                .then_some(frame as f32 / frames_per_second),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsInteractiveLightAlternateStateInput {
    pub context_state_value: f32,
    pub marker_name_nonempty: bool,
    pub marker_table_count: u32,
    pub first_marker_frame: Option<i16>,
    pub owner_runtime_state_144: Option<u32>,
    pub animation_target_present: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsInteractiveLightAlternateStatePlan {
    pub state_source: RobotsScriptMarkerStateSource,
    pub apply_to_animation_target: bool,
    pub invalid_marker_table_diagnostic: bool,
}

/// Shared local body used by `XItemHandler_Interactive::HandleScriptCmdEvent`
/// (`0x004125C0`) and `XItemHandler_Light::HandleScriptCmdEvent`
/// (`0x00413880`) for SetAlternateState (`0x1600000E`). The native bodies are
/// instruction-equivalent apart from their diagnostic strings.
pub const fn interactive_light_alternate_state_plan(
    input: RobotsInteractiveLightAlternateStateInput,
) -> RobotsInteractiveLightAlternateStatePlan {
    let mut state_source = RobotsScriptMarkerStateSource::ContextState {
        value: input.context_state_value,
    };
    let mut invalid_marker_table_diagnostic = false;

    if input.marker_name_nonempty {
        match input.first_marker_frame {
            Some(frame) if input.marker_table_count != 0 && frame >= 0 => {
                if matches!(input.owner_runtime_state_144, Some(state) if state != 0 && state != 4)
                {
                    state_source = RobotsScriptMarkerStateSource::MarkerFrame { frame };
                }
            }
            _ => invalid_marker_table_diagnostic = true,
        }
    }

    RobotsInteractiveLightAlternateStatePlan {
        state_source,
        apply_to_animation_target: input.animation_target_present,
        invalid_marker_table_diagnostic,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsHazardAlternateStatePlan {
    pub state_source: RobotsScriptMarkerStateSource,
    pub apply_to_animation_target: bool,
}

/// Hazard (`0x004113A0`) uses the same marker-frame substitution as
/// Interactive/Light, but silently keeps the context state when the marker
/// table is absent/invalid instead of emitting the Interactive/Light warning.
pub const fn hazard_alternate_state_plan(
    input: RobotsInteractiveLightAlternateStateInput,
) -> RobotsHazardAlternateStatePlan {
    let common = interactive_light_alternate_state_plan(input);
    RobotsHazardAlternateStatePlan {
        state_source: common.state_source,
        apply_to_animation_target: common.apply_to_animation_target,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsScriptMarkerClearInput {
    pub marker_guard_3a4: i16,
    pub context_present: bool,
    pub marker_name_storage_present: bool,
    pub clear_marker_name: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsScriptMarkerClearPlan {
    pub blocked_by_marker_guard: bool,
    pub clear_handler_state_3a0: bool,
    pub clear_context_state_40: bool,
    pub clear_marker_name_first_byte: bool,
}

/// Common ClearStateMarker state reset. Interactive/Light pass
/// `clear_marker_name=true`; ScriptLifecycle uses the same `+3A4 == -1` guard
/// and `+3A0/context+0x40` reset but does not clear the marker-name byte itself.
pub const fn script_marker_clear_plan(
    input: RobotsScriptMarkerClearInput,
) -> RobotsScriptMarkerClearPlan {
    if input.marker_guard_3a4 != -1 {
        return RobotsScriptMarkerClearPlan {
            blocked_by_marker_guard: true,
            clear_handler_state_3a0: false,
            clear_context_state_40: false,
            clear_marker_name_first_byte: false,
        };
    }

    RobotsScriptMarkerClearPlan {
        blocked_by_marker_guard: false,
        clear_handler_state_3a0: true,
        clear_context_state_40: input.context_present,
        clear_marker_name_first_byte: input.clear_marker_name && input.marker_name_storage_present,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alternate_input() -> RobotsInteractiveLightAlternateStateInput {
        RobotsInteractiveLightAlternateStateInput {
            context_state_value: 0.25,
            marker_name_nonempty: true,
            marker_table_count: 2,
            first_marker_frame: Some(15),
            owner_runtime_state_144: Some(1),
            animation_target_present: true,
        }
    }

    #[test]
    fn interactive_and_light_use_marker_frame_when_native_owner_state_allows_it() {
        let plan = interactive_light_alternate_state_plan(alternate_input());
        assert_eq!(
            plan.state_source,
            RobotsScriptMarkerStateSource::MarkerFrame { frame: 15 }
        );
        assert_eq!(plan.state_source.resolve(30.0), Some(0.5));
        assert!(plan.apply_to_animation_target);
        assert!(!plan.invalid_marker_table_diagnostic);
    }

    #[test]
    fn alternate_state_keeps_context_value_for_native_owner_states_zero_and_four() {
        for owner_state in [0, 4] {
            let mut input = alternate_input();
            input.owner_runtime_state_144 = Some(owner_state);
            let plan = interactive_light_alternate_state_plan(input);
            assert_eq!(
                plan.state_source,
                RobotsScriptMarkerStateSource::ContextState { value: 0.25 }
            );
        }
    }

    #[test]
    fn interactive_light_report_invalid_marker_table_but_hazard_silently_keeps_context_state() {
        let mut input = alternate_input();
        input.marker_table_count = 0;
        input.first_marker_frame = None;

        let interactive_light = interactive_light_alternate_state_plan(input);
        assert!(interactive_light.invalid_marker_table_diagnostic);
        assert_eq!(
            interactive_light.state_source,
            RobotsScriptMarkerStateSource::ContextState { value: 0.25 }
        );

        let hazard = hazard_alternate_state_plan(input);
        assert_eq!(
            hazard.state_source,
            RobotsScriptMarkerStateSource::ContextState { value: 0.25 }
        );
        assert!(hazard.apply_to_animation_target);
    }

    #[test]
    fn clear_state_marker_guard_and_family_name_clear_policy_are_explicit() {
        let blocked = script_marker_clear_plan(RobotsScriptMarkerClearInput {
            marker_guard_3a4: 2,
            context_present: true,
            marker_name_storage_present: true,
            clear_marker_name: true,
        });
        assert!(blocked.blocked_by_marker_guard);
        assert!(!blocked.clear_handler_state_3a0);

        let interactive_light = script_marker_clear_plan(RobotsScriptMarkerClearInput {
            marker_guard_3a4: -1,
            context_present: true,
            marker_name_storage_present: true,
            clear_marker_name: true,
        });
        assert!(interactive_light.clear_handler_state_3a0);
        assert!(interactive_light.clear_context_state_40);
        assert!(interactive_light.clear_marker_name_first_byte);

        let lifecycle = script_marker_clear_plan(RobotsScriptMarkerClearInput {
            marker_guard_3a4: -1,
            context_present: true,
            marker_name_storage_present: true,
            clear_marker_name: false,
        });
        assert!(lifecycle.clear_handler_state_3a0);
        assert!(lifecycle.clear_context_state_40);
        assert!(!lifecycle.clear_marker_name_first_byte);
    }

    #[test]
    fn marker_frame_resolution_fails_closed_without_valid_fps() {
        let source = RobotsScriptMarkerStateSource::MarkerFrame { frame: 15 };
        assert_eq!(source.resolve(0.0), None);
        assert_eq!(source.resolve(f32::NAN), None);
    }
}
