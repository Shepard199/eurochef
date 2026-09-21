use serde::Serialize;

pub const ROBOTS_PLAYER_FOCUS_DEFAULT_RANGE: f32 = 2.5;
pub const ROBOTS_PLAYER_FOCUS_VERTICAL_TOLERANCE: f32 = 1.5;
pub const ROBOTS_PLAYER_FOCUS_ACTIVATION_PAD_RANGE: f32 = 1.5;
pub const ROBOTS_XITEM_CATEGORY_NPC: u32 = 0x0B;
pub const ROBOTS_XITEM_CATEGORY_ACTIVATION_PAD: u32 = 0x4C;
pub const ROBOTS_XITEM_CATEGORY_SLIDE_UNDER: u32 = 0x58;
pub const ROBOTS_XITEM_CATEGORY_ALERT_ICON: u32 = 0x59;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsAlertIconFocusInput {
    pub player_position: [f32; 3],
    pub trigger_position: [f32; 3],
    pub active_e4: bool,
    pub xitem_exists: bool,
    pub horizontal_radius: f32,
    pub vertical_tolerance: f32,
    pub blocked_by_global_mode: bool,
    pub current_owner_is_candidate: bool,
    pub current_owner_present: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsSlideUnderOwnerView {
    pub trigger_position: [f32; 3],
    pub outer_radius: f32,
    pub is_candidate: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsSlideUnderInput {
    pub player_position: [f32; 3],
    pub trigger_position: [f32; 3],
    pub state_e4: u8,
    pub xitem_exists: bool,
    pub inner_radius: f32,
    pub outer_radius: f32,
    pub player_state: u8,
    pub current_focus_is_candidate: bool,
    pub current_slide_under: Option<RobotsSlideUnderOwnerView>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsSlideUnderOwnerDecision {
    KeepCurrent,
    SelectCandidate,
    ClearCurrentAndDeactivateLinks,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsSlideUnderStep {
    pub focus: RobotsPlayerFocusDecision,
    pub slide_under: RobotsSlideUnderOwnerDecision,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsPlayerFocusOwnerView {
    pub category: u32,
    pub position: [f32; 3],
    pub is_player_xitem: bool,
    pub is_candidate: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsActivationPadFocusInput {
    pub player_position: [f32; 3],
    pub player_yaw: f32,
    pub player_state: u8,
    pub current_interaction_code: u8,
    pub current_owner: Option<RobotsPlayerFocusOwnerView>,
    pub candidate_focus_point: [f32; 3],
    pub xitem_exists: bool,
    pub watchbot_controller_enabled: bool,
    pub game_control_mode: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsPlayerFocusCandidateInput {
    pub player_position: [f32; 3],
    pub player_yaw: f32,
    pub player_state: u8,
    pub current_interaction_code: u8,
    pub current_owner: Option<RobotsPlayerFocusOwnerView>,
    pub candidate_category: u32,
    pub candidate_focus_point: [f32; 3],
    pub range: f32,
    pub npc_proxy_state9_blocked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsPlayerFocusDecision {
    KeepCurrent,
    ClearCurrent,
    SelectCandidate,
}

/// Exact focus-owner projection of `XTrigger_AlertIcon +0x60 = 0x004837A0`.
/// The native callback mutates only Player Handler `+0x52C`; Trigger/XItem
/// lifetime stays owned by TriggerManager.
pub fn plan_alert_icon_focus(input: RobotsAlertIconFocusInput) -> RobotsPlayerFocusDecision {
    let dx = input.player_position[0] - input.trigger_position[0];
    let dz = input.player_position[2] - input.trigger_position[2];
    let vertical = (input.player_position[1] - input.trigger_position[1]).abs();
    let radius = input.horizontal_radius.abs();
    let inside = input.active_e4
        && input.xitem_exists
        && !input.blocked_by_global_mode
        && vertical <= input.vertical_tolerance.abs()
        && dx * dx + dz * dz <= radius * radius;

    if inside {
        if input.current_owner_present {
            RobotsPlayerFocusDecision::KeepCurrent
        } else {
            RobotsPlayerFocusDecision::SelectCandidate
        }
    } else if input.current_owner_is_candidate {
        RobotsPlayerFocusDecision::ClearCurrent
    } else {
        RobotsPlayerFocusDecision::KeepCurrent
    }
}

/// Exact Player-side projection of `XTrigger_SlideUnder +0x60 = 0x00483410`.
/// Native keeps two independent Player Handler pointers: `+0x52C` is the focus
/// XItem and `+0x6BC` is the active SlideUnder trigger. The latter is serviced
/// only in Player states `0x0F/0x10` and emits link0/link1 `0x200` on exit.
pub fn plan_slide_under_player(input: RobotsSlideUnderInput) -> RobotsSlideUnderStep {
    let dx = input.player_position[0] - input.trigger_position[0];
    let dz = input.player_position[2] - input.trigger_position[2];
    let planar = dx * dx + dz * dz;
    let vertical = (input.player_position[1] - input.trigger_position[1]).abs();
    let inner = input.inner_radius.abs();
    let outer = input.outer_radius.abs();

    // First native phase writes Player Handler +0x52C directly. Unlike
    // AlertIcon it can replace an already populated focus owner.
    let mut focus = if input.state_e4 == 1 && planar < outer * outer && vertical <= outer {
        if input.xitem_exists {
            RobotsPlayerFocusDecision::SelectCandidate
        } else {
            RobotsPlayerFocusDecision::ClearCurrent
        }
    } else if input.current_focus_is_candidate {
        RobotsPlayerFocusDecision::ClearCurrent
    } else {
        RobotsPlayerFocusDecision::KeepCurrent
    };

    if input.state_e4 == 0 || !matches!(input.player_state, 0x0F | 0x10) {
        return RobotsSlideUnderStep {
            focus,
            slide_under: RobotsSlideUnderOwnerDecision::KeepCurrent,
        };
    }

    let slide_under = match input.current_slide_under {
        Some(current) if current.is_candidate => {
            // Native really compares abs(Y) with outer_radius^2 here. This is
            // instruction-confirmed oddity at 0x00483659..0x00483679.
            let outer_squared = outer * outer;
            if planar >= outer_squared || vertical > outer_squared {
                RobotsSlideUnderOwnerDecision::ClearCurrentAndDeactivateLinks
            } else {
                RobotsSlideUnderOwnerDecision::KeepCurrent
            }
        }
        Some(current) => {
            let current_dx = input.player_position[0] - current.trigger_position[0];
            let current_dz = input.player_position[2] - current.trigger_position[2];
            let current_planar = current_dx * current_dx + current_dz * current_dz;
            let current_vertical = (input.player_position[1] - current.trigger_position[1]).abs();
            let current_outer_squared = current.outer_radius.abs() * current.outer_radius.abs();

            // Another SlideUnder retains ownership while this asymmetric native
            // predicate holds; the vertical bound is squared, not linear.
            if current_planar < current_outer_squared && current_vertical <= current_outer_squared {
                RobotsSlideUnderOwnerDecision::KeepCurrent
            } else if planar < outer * outer && vertical < outer {
                RobotsSlideUnderOwnerDecision::SelectCandidate
            } else {
                RobotsSlideUnderOwnerDecision::KeepCurrent
            }
        }
        None => {
            if planar < inner * inner && vertical < inner {
                RobotsSlideUnderOwnerDecision::SelectCandidate
            } else {
                // In this exact branch native clears +0x52C after the earlier
                // outer-zone write if it now points at this SlideUnder XItem.
                if focus == RobotsPlayerFocusDecision::SelectCandidate {
                    focus = RobotsPlayerFocusDecision::ClearCurrent;
                }
                RobotsSlideUnderOwnerDecision::KeepCurrent
            }
        }
    };

    RobotsSlideUnderStep { focus, slide_under }
}

fn planar_distance_squared(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dz = a[2] - b[2];
    dx * dx + dz * dz
}

fn distance_squared(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    dx * dx + dy * dy + dz * dz
}

fn reject_candidate(current: Option<RobotsPlayerFocusOwnerView>) -> RobotsPlayerFocusDecision {
    match current {
        Some(owner) if owner.is_candidate => RobotsPlayerFocusDecision::ClearCurrent,
        Some(_) => RobotsPlayerFocusDecision::KeepCurrent,
        None => RobotsPlayerFocusDecision::ClearCurrent,
    }
}

pub fn robots_player_focus_owner_preserves_range(category: u32, interaction_code: u8) -> bool {
    match category {
        ROBOTS_XITEM_CATEGORY_NPC => interaction_code != 0x0B,
        0x23
        | ROBOTS_XITEM_CATEGORY_ACTIVATION_PAD
        | 0x4E
        | ROBOTS_XITEM_CATEGORY_SLIDE_UNDER
        | ROBOTS_XITEM_CATEGORY_ALERT_ICON => true,
        _ => false,
    }
}

/// ActivationPad-specific gate around the common Player focus selector. Native
/// `XItemHandler_Interactive::0x004127C0` asks its creator trigger for eligibility
/// and range before calling `0x004BC280`; `XTrigger_ActivationPad` supplies the
/// WatchBot-controller item gate and exact 1.5-unit range.
pub fn plan_activation_pad_focus_candidate(
    input: RobotsActivationPadFocusInput,
) -> RobotsPlayerFocusDecision {
    if !input.xitem_exists || !input.watchbot_controller_enabled || input.game_control_mode != 0 {
        return reject_candidate(input.current_owner);
    }
    plan_player_focus_candidate(RobotsPlayerFocusCandidateInput {
        player_position: input.player_position,
        player_yaw: input.player_yaw,
        player_state: input.player_state,
        current_interaction_code: input.current_interaction_code,
        current_owner: input.current_owner,
        candidate_category: ROBOTS_XITEM_CATEGORY_ACTIVATION_PAD,
        candidate_focus_point: input.candidate_focus_point,
        range: ROBOTS_PLAYER_FOCUS_ACTIVATION_PAD_RANGE,
        npc_proxy_state9_blocked: false,
    })
}

/// Recovered subset of Player focus-owner selector `0x004BC280` used by both
/// known external callers (`0x0046B3C0` and `0x004127C0`). Both pass an angle
/// limit of PI, so the native angle gate is intentionally absent here.
pub fn plan_player_focus_candidate(
    input: RobotsPlayerFocusCandidateInput,
) -> RobotsPlayerFocusDecision {
    let mut current = input.current_owner;
    let range_squared = input.range * input.range;

    if let Some(owner) = current {
        if !owner.is_player_xitem {
            if matches!(input.player_state, 0x1E | 0x1F) {
                return RobotsPlayerFocusDecision::KeepCurrent;
            }
            if input.candidate_category != owner.category
                && !robots_player_focus_owner_preserves_range(
                    owner.category,
                    input.current_interaction_code,
                )
                && planar_distance_squared(input.player_position, owner.position) > range_squared
            {
                current = None;
            }
        }
    }

    if planar_distance_squared(input.player_position, input.candidate_focus_point) > range_squared
        || (input.player_position[1] - input.candidate_focus_point[1]).abs()
            > ROBOTS_PLAYER_FOCUS_VERTICAL_TOLERANCE
    {
        return reject_candidate(current);
    }

    let (sin_yaw, cos_yaw) = input.player_yaw.sin_cos();
    let forward_point = [
        input.player_position[0] + sin_yaw,
        input.player_position[1],
        input.player_position[2] + cos_yaw,
    ];
    if let Some(owner) = current {
        if distance_squared(owner.position, forward_point)
            < distance_squared(input.candidate_focus_point, forward_point)
        {
            return RobotsPlayerFocusDecision::KeepCurrent;
        }
    }

    if input.candidate_category == ROBOTS_XITEM_CATEGORY_NPC && input.npc_proxy_state9_blocked {
        return reject_candidate(current);
    }

    RobotsPlayerFocusDecision::SelectCandidate
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate() -> RobotsPlayerFocusCandidateInput {
        RobotsPlayerFocusCandidateInput {
            player_position: [0.0, 0.0, 0.0],
            player_yaw: 0.0,
            player_state: 0,
            current_interaction_code: 0,
            current_owner: None,
            candidate_category: ROBOTS_XITEM_CATEGORY_NPC,
            candidate_focus_point: [0.0, 0.0, 2.0],
            range: ROBOTS_PLAYER_FOCUS_DEFAULT_RANGE,
            npc_proxy_state9_blocked: false,
        }
    }

    #[test]
    fn slide_under_enters_inner_zone_and_clears_focus_in_outer_only_gap() {
        let mut input = RobotsSlideUnderInput {
            player_position: [0.5, 0.5, 0.0],
            trigger_position: [0.0, 0.0, 0.0],
            state_e4: 1,
            xitem_exists: true,
            inner_radius: 1.0,
            outer_radius: 2.0,
            player_state: 0x0F,
            current_focus_is_candidate: false,
            current_slide_under: None,
        };
        let entered = plan_slide_under_player(input);
        assert_eq!(entered.focus, RobotsPlayerFocusDecision::SelectCandidate);
        assert_eq!(
            entered.slide_under,
            RobotsSlideUnderOwnerDecision::SelectCandidate
        );

        input.player_position = [1.5, 0.0, 0.0];
        let outer_only = plan_slide_under_player(input);
        assert_eq!(outer_only.focus, RobotsPlayerFocusDecision::ClearCurrent);
        assert_eq!(
            outer_only.slide_under,
            RobotsSlideUnderOwnerDecision::KeepCurrent
        );
    }

    #[test]
    fn slide_under_outer_focus_is_planar_strict_vertical_inclusive_and_state_gated() {
        let mut input = RobotsSlideUnderInput {
            player_position: [0.0, 2.0, 0.0],
            trigger_position: [0.0, 0.0, 0.0],
            state_e4: 1,
            xitem_exists: true,
            inner_radius: 1.0,
            outer_radius: 2.0,
            player_state: 2,
            current_focus_is_candidate: false,
            current_slide_under: None,
        };
        let vertical_edge = plan_slide_under_player(input);
        assert_eq!(
            vertical_edge.focus,
            RobotsPlayerFocusDecision::SelectCandidate
        );
        assert_eq!(
            vertical_edge.slide_under,
            RobotsSlideUnderOwnerDecision::KeepCurrent
        );

        input.player_position = [2.0, 0.0, 0.0];
        assert_eq!(
            plan_slide_under_player(input).focus,
            RobotsPlayerFocusDecision::KeepCurrent
        );

        // Native focus tests E4==1, while the SlideUnder owner handshake only
        // requires E4!=0.
        input.player_position = [0.5, 0.0, 0.0];
        input.player_state = 0x0F;
        input.state_e4 = 2;
        let noncanonical_active = plan_slide_under_player(input);
        assert_eq!(
            noncanonical_active.focus,
            RobotsPlayerFocusDecision::KeepCurrent
        );
        assert_eq!(
            noncanonical_active.slide_under,
            RobotsSlideUnderOwnerDecision::SelectCandidate
        );
    }

    #[test]
    fn slide_under_preserves_native_squared_vertical_retention_and_exit_oddity() {
        let other = RobotsSlideUnderOwnerView {
            trigger_position: [0.0, 0.0, 0.0],
            outer_radius: 2.0,
            is_candidate: false,
        };
        let mut input = RobotsSlideUnderInput {
            player_position: [0.0, 3.5, 0.0],
            trigger_position: [10.0, 0.0, 0.0],
            state_e4: 1,
            xitem_exists: true,
            inner_radius: 1.0,
            outer_radius: 2.0,
            player_state: 0x10,
            current_focus_is_candidate: false,
            current_slide_under: Some(other),
        };
        assert_eq!(
            plan_slide_under_player(input).slide_under,
            RobotsSlideUnderOwnerDecision::KeepCurrent
        );

        input.trigger_position = [0.0, 0.0, 0.0];
        input.current_slide_under = Some(RobotsSlideUnderOwnerView {
            is_candidate: true,
            ..other
        });
        input.player_position = [0.0, 4.001, 0.0];
        assert_eq!(
            plan_slide_under_player(input).slide_under,
            RobotsSlideUnderOwnerDecision::ClearCurrentAndDeactivateLinks
        );
        input.player_position = [0.0, 4.0, 0.0];
        assert_eq!(
            plan_slide_under_player(input).slide_under,
            RobotsSlideUnderOwnerDecision::KeepCurrent
        );
    }

    #[test]
    fn alert_icon_focus_uses_inclusive_native_cylinder_and_never_steals_an_owner() {
        let mut input = RobotsAlertIconFocusInput {
            player_position: [2.0, 1.0, 0.0],
            trigger_position: [0.0, 0.0, 0.0],
            active_e4: true,
            xitem_exists: true,
            horizontal_radius: 2.0,
            vertical_tolerance: 1.0,
            blocked_by_global_mode: false,
            current_owner_is_candidate: false,
            current_owner_present: false,
        };
        assert_eq!(
            plan_alert_icon_focus(input),
            RobotsPlayerFocusDecision::SelectCandidate
        );
        input.current_owner_present = true;
        assert_eq!(
            plan_alert_icon_focus(input),
            RobotsPlayerFocusDecision::KeepCurrent
        );
        input.current_owner_is_candidate = true;
        input.player_position[0] = 2.001;
        assert_eq!(
            plan_alert_icon_focus(input),
            RobotsPlayerFocusDecision::ClearCurrent
        );
        input.player_position[0] = 0.0;
        input.current_owner_is_candidate = false;
        input.current_owner_present = false;
        input.blocked_by_global_mode = true;
        assert_eq!(
            plan_alert_icon_focus(input),
            RobotsPlayerFocusDecision::KeepCurrent
        );
    }

    #[test]
    fn activation_pad_requires_live_xitem_controller_mode_zero_and_exact_range() {
        let mut input = RobotsActivationPadFocusInput {
            player_position: [0.0, 0.0, 0.0],
            player_yaw: 0.0,
            player_state: 0,
            current_interaction_code: 0xff,
            current_owner: None,
            candidate_focus_point: [0.0, 0.0, ROBOTS_PLAYER_FOCUS_ACTIVATION_PAD_RANGE],
            xitem_exists: true,
            watchbot_controller_enabled: true,
            game_control_mode: 0,
        };
        assert_eq!(
            plan_activation_pad_focus_candidate(input),
            RobotsPlayerFocusDecision::SelectCandidate
        );

        input.candidate_focus_point[2] = ROBOTS_PLAYER_FOCUS_ACTIVATION_PAD_RANGE + 0.0001;
        assert_eq!(
            plan_activation_pad_focus_candidate(input),
            RobotsPlayerFocusDecision::ClearCurrent
        );
        input.candidate_focus_point[2] = 1.0;
        input.watchbot_controller_enabled = false;
        assert_eq!(
            plan_activation_pad_focus_candidate(input),
            RobotsPlayerFocusDecision::ClearCurrent
        );
        input.watchbot_controller_enabled = true;
        input.game_control_mode = 1;
        assert_eq!(
            plan_activation_pad_focus_candidate(input),
            RobotsPlayerFocusDecision::ClearCurrent
        );
        input.game_control_mode = 0;
        input.xitem_exists = false;
        assert_eq!(
            plan_activation_pad_focus_candidate(input),
            RobotsPlayerFocusDecision::ClearCurrent
        );
    }

    #[test]
    fn default_npc_candidate_uses_exact_range_and_vertical_boundaries() {
        let mut input = candidate();
        input.candidate_focus_point = [2.5, 1.5, 0.0];
        assert_eq!(
            plan_player_focus_candidate(input),
            RobotsPlayerFocusDecision::SelectCandidate
        );

        input.candidate_focus_point[0] = 2.5001;
        assert_eq!(
            plan_player_focus_candidate(input),
            RobotsPlayerFocusDecision::ClearCurrent
        );

        input.candidate_focus_point = [0.0, 1.5001, 2.0];
        assert_eq!(
            plan_player_focus_candidate(input),
            RobotsPlayerFocusDecision::ClearCurrent
        );
    }

    #[test]
    fn modal_states_freeze_an_existing_non_player_owner() {
        let mut input = candidate();
        input.player_state = 0x1E;
        input.current_owner = Some(RobotsPlayerFocusOwnerView {
            category: 0x20,
            position: [20.0, 0.0, 20.0],
            is_player_xitem: false,
            is_candidate: false,
        });
        assert_eq!(
            plan_player_focus_candidate(input),
            RobotsPlayerFocusDecision::KeepCurrent
        );
        input.player_state = 0x1F;
        assert_eq!(
            plan_player_focus_candidate(input),
            RobotsPlayerFocusDecision::KeepCurrent
        );
    }

    #[test]
    fn native_owner_categories_preserve_the_preclear_range_rule() {
        for category in [0x23, ROBOTS_XITEM_CATEGORY_ACTIVATION_PAD, 0x4E, 0x58, 0x59] {
            assert!(robots_player_focus_owner_preserves_range(category, 0));
        }
        assert!(robots_player_focus_owner_preserves_range(
            ROBOTS_XITEM_CATEGORY_NPC,
            2
        ));
        assert!(!robots_player_focus_owner_preserves_range(
            ROBOTS_XITEM_CATEGORY_NPC,
            0x0B
        ));
        assert!(!robots_player_focus_owner_preserves_range(0x20, 0));
    }

    #[test]
    fn nearer_current_owner_wins_against_candidate_focus_point() {
        let mut input = candidate();
        input.current_owner = Some(RobotsPlayerFocusOwnerView {
            category: 0x20,
            position: [0.0, 0.0, 1.25],
            is_player_xitem: false,
            is_candidate: false,
        });
        assert_eq!(
            plan_player_focus_candidate(input),
            RobotsPlayerFocusDecision::KeepCurrent
        );

        input.current_owner = Some(RobotsPlayerFocusOwnerView {
            category: 0x20,
            position: [0.0, 0.0, 2.4],
            is_player_xitem: false,
            is_candidate: false,
        });
        assert_eq!(
            plan_player_focus_candidate(input),
            RobotsPlayerFocusDecision::SelectCandidate
        );
    }

    #[test]
    fn rejected_current_candidate_is_cleared_but_other_owner_is_retained() {
        let mut input = candidate();
        input.candidate_focus_point = [3.0, 0.0, 0.0];
        input.current_owner = Some(RobotsPlayerFocusOwnerView {
            category: ROBOTS_XITEM_CATEGORY_NPC,
            position: [3.0, 0.0, 0.0],
            is_player_xitem: false,
            is_candidate: true,
        });
        assert_eq!(
            plan_player_focus_candidate(input),
            RobotsPlayerFocusDecision::ClearCurrent
        );

        input.current_owner = Some(RobotsPlayerFocusOwnerView {
            category: 0x23,
            position: [3.0, 0.0, 0.0],
            is_player_xitem: false,
            is_candidate: false,
        });
        assert_eq!(
            plan_player_focus_candidate(input),
            RobotsPlayerFocusDecision::KeepCurrent
        );
    }

    #[test]
    fn npc_proxy_state9_vetoes_selection_and_clears_same_candidate() {
        let mut input = candidate();
        input.npc_proxy_state9_blocked = true;
        assert_eq!(
            plan_player_focus_candidate(input),
            RobotsPlayerFocusDecision::ClearCurrent
        );
        input.current_owner = Some(RobotsPlayerFocusOwnerView {
            category: ROBOTS_XITEM_CATEGORY_NPC,
            position: input.candidate_focus_point,
            is_player_xitem: false,
            is_candidate: true,
        });
        assert_eq!(
            plan_player_focus_candidate(input),
            RobotsPlayerFocusDecision::ClearCurrent
        );
    }
}
