use serde::Serialize;

pub const ROBOTS_WATCHBOT_COMPONENT_BASE_OFFSET: u32 = 0x490;
pub const ROBOTS_WATCHBOT_COMPONENT_INDEX_OFFSET: u32 = 0x4A0;
pub const ROBOTS_WATCHBOT_COMPONENT_HIT_VSLOT_OFFSET: u32 = 0x2C;
pub const ROBOTS_WATCHBOT_COMPONENT_STATE_TRANSITION_TARGET: u32 = 0x0049_37C0;
pub const ROBOTS_WATCHBOT_COMPONENT_HIT_STATE: u32 = 7;
pub const ROBOTS_WATCHBOT_COMPONENT_SHARED_IGNORE_STATE: u32 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsWatchBotComponentKind {
    Slot1Primary,
    Slot2Shared,
    Slot3Shared,
}

impl RobotsWatchBotComponentKind {
    pub fn from_index(index: u32) -> Option<Self> {
        match index {
            1 => Some(Self::Slot1Primary),
            2 => Some(Self::Slot2Shared),
            3 => Some(Self::Slot3Shared),
            _ => None,
        }
    }
}

/// Native `0x004937C0(state, force)` component-state request. WatchBot hit
/// callbacks always use `(7, 0)`: the setter only invokes exit/enter vslot +0x34
/// when the requested state differs from the current state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsWatchBotComponentStateRequest {
    pub state: u32,
    pub force: bool,
}

/// Host-resolved snapshot for `XItemHandler_WatchBot::ApplyHit` (`0x00490C10`).
///
/// Native selects `handler + 0x490 + current_component_index*4`; WatchBot ctor
/// `0x0048FC20` creates component objects in slots 1..3 and leaves slot0 null.
/// Pointer resolution remains host-side so UE5.8 never needs the original layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsWatchBotHitInput {
    pub current_component_index: u32,
    pub current_component_present: bool,
    /// Native component +0x04. Required to preserve the slot2/3 state12 guard.
    pub current_component_state: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsWatchBotHitResult {
    pub reaction_accepted: bool,
    /// Native outer query `0x00425C70` commits the geometric hit regardless of
    /// the +0xC8 return value.
    pub query_hit_still_commits: bool,
    pub component_kind: Option<RobotsWatchBotComponentKind>,
    pub state_request: Option<RobotsWatchBotComponentStateRequest>,
}

/// Exact Handler/component hit reducer for native `0x00490C10` plus the three
/// constructor-owned component `+0x2C` implementations.
///
/// - slot1 vtable `0x005ECCF0`, +0x2C=`0x004932F0`: always requests state7.
/// - slot2 vtable `0x005ECEB8`, +0x2C=`0x00496450`: requests state7 unless state12.
/// - slot3 vtable `0x005ECF50`, +0x2C=`0x00496450`: same state12 guard.
///
/// The outer WatchBot Handler returns 1 whenever the selected component pointer
/// is non-null, even when the slot2/3 state12 guard suppresses the transition.
pub fn robots_apply_watchbot_hit(input: RobotsWatchBotHitInput) -> RobotsWatchBotHitResult {
    if !input.current_component_present {
        return RobotsWatchBotHitResult {
            reaction_accepted: false,
            query_hit_still_commits: true,
            component_kind: None,
            state_request: None,
        };
    }

    let Some(component_kind) =
        RobotsWatchBotComponentKind::from_index(input.current_component_index)
    else {
        // Canonical ctor owns only slots1..3; unknown populated indices are not a
        // shipped-game state and fail closed instead of inventing a component ABI.
        return RobotsWatchBotHitResult {
            reaction_accepted: false,
            query_hit_still_commits: true,
            component_kind: None,
            state_request: None,
        };
    };

    let suppress_transition = matches!(
        component_kind,
        RobotsWatchBotComponentKind::Slot2Shared | RobotsWatchBotComponentKind::Slot3Shared
    ) && input.current_component_state
        == ROBOTS_WATCHBOT_COMPONENT_SHARED_IGNORE_STATE;

    RobotsWatchBotHitResult {
        reaction_accepted: true,
        query_hit_still_commits: true,
        component_kind: Some(component_kind),
        state_request: (!suppress_transition).then_some(RobotsWatchBotComponentStateRequest {
            state: ROBOTS_WATCHBOT_COMPONENT_HIT_STATE,
            force: false,
        }),
    }
}

pub const ROBOTS_WATCHBOT_HIT_ANIMATION_MODE_UID: u32 = 0x0900_00A8;
pub const ROBOTS_WATCHBOT_BODY_ENTITY_UID: u32 = 0x0200_00A8;
pub const ROBOTS_WATCHBOT_EYELID_L_ENTITY_UID: u32 = 0x0200_0098;
pub const ROBOTS_WATCHBOT_EYELID_R_ENTITY_UID: u32 = 0x0200_0097;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsWatchBotComponentStatePhase {
    Enter,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsWatchBotChannelTogglePlan {
    /// Native owner animator at XItem+0x144 controls these three fixed HT_Entity channels.
    pub apply_fixed_channels: bool,
    pub fixed_entity_uids: [u32; 3],
    /// Native Handler +0x4C8 component contributes its field[8] channel when present.
    pub dynamic_entity_uid: Option<u32>,
    pub value: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsWatchBotState7TransitionInput {
    pub component_kind: RobotsWatchBotComponentKind,
    pub phase: RobotsWatchBotComponentStatePhase,
    pub owner_animator_present: bool,
    pub dynamic_entity_uid: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsWatchBotState7TransitionPlan {
    pub animation_mode_uid: Option<u32>,
    pub channel_toggle: Option<RobotsWatchBotChannelTogglePlan>,
}

/// Exact state7 side effects reached through common component state dispatcher
/// `0x00493A60` after WatchBot hit requests state7.
///
/// All three components enter state7 by starting mode `0x090000A8` through
/// `0x00494AB0`. Only slot1 has an exit side effect: `0x00492700(1)` calls
/// WatchBot owner helper `0x00490CC0(1)`, which forwards value=true to Body,
/// EyelidL, EyelidR and the optional dynamic Handler+0x4C8 channel.
pub fn robots_watchbot_state7_transition(
    input: RobotsWatchBotState7TransitionInput,
) -> RobotsWatchBotState7TransitionPlan {
    match input.phase {
        RobotsWatchBotComponentStatePhase::Enter => RobotsWatchBotState7TransitionPlan {
            animation_mode_uid: Some(ROBOTS_WATCHBOT_HIT_ANIMATION_MODE_UID),
            channel_toggle: None,
        },
        RobotsWatchBotComponentStatePhase::Exit
            if input.component_kind == RobotsWatchBotComponentKind::Slot1Primary =>
        {
            RobotsWatchBotState7TransitionPlan {
                animation_mode_uid: None,
                channel_toggle: Some(RobotsWatchBotChannelTogglePlan {
                    apply_fixed_channels: input.owner_animator_present,
                    fixed_entity_uids: [
                        ROBOTS_WATCHBOT_BODY_ENTITY_UID,
                        ROBOTS_WATCHBOT_EYELID_L_ENTITY_UID,
                        ROBOTS_WATCHBOT_EYELID_R_ENTITY_UID,
                    ],
                    dynamic_entity_uid: input.dynamic_entity_uid,
                    value: true,
                }),
            }
        }
        RobotsWatchBotComponentStatePhase::Exit => RobotsWatchBotState7TransitionPlan {
            animation_mode_uid: None,
            channel_toggle: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_component_rejects_reaction_but_outer_query_still_commits() {
        let result = robots_apply_watchbot_hit(RobotsWatchBotHitInput {
            current_component_index: 2,
            current_component_present: false,
            current_component_state: 0,
        });
        assert!(!result.reaction_accepted);
        assert!(result.query_hit_still_commits);
        assert_eq!(result.component_kind, None);
        assert_eq!(result.state_request, None);
    }

    #[test]
    fn slot1_always_requests_native_state7_without_force() {
        let result = robots_apply_watchbot_hit(RobotsWatchBotHitInput {
            current_component_index: 1,
            current_component_present: true,
            current_component_state: 12,
        });
        assert!(result.reaction_accepted);
        assert_eq!(
            result.component_kind,
            Some(RobotsWatchBotComponentKind::Slot1Primary)
        );
        assert_eq!(
            result.state_request,
            Some(RobotsWatchBotComponentStateRequest {
                state: 7,
                force: false,
            })
        );
    }

    #[test]
    fn shared_slots_request_state7_except_when_current_state_is12() {
        for index in [2, 3] {
            let normal = robots_apply_watchbot_hit(RobotsWatchBotHitInput {
                current_component_index: index,
                current_component_present: true,
                current_component_state: 6,
            });
            assert!(normal.reaction_accepted);
            assert_eq!(normal.state_request.unwrap().state, 7);

            let state12 = robots_apply_watchbot_hit(RobotsWatchBotHitInput {
                current_component_index: index,
                current_component_present: true,
                current_component_state: 12,
            });
            assert!(state12.reaction_accepted);
            assert!(state12.query_hit_still_commits);
            assert_eq!(state12.state_request, None);
        }
    }

    #[test]
    fn state7_enter_starts_exact_native_mode_for_all_component_kinds() {
        for component_kind in [
            RobotsWatchBotComponentKind::Slot1Primary,
            RobotsWatchBotComponentKind::Slot2Shared,
            RobotsWatchBotComponentKind::Slot3Shared,
        ] {
            let plan = robots_watchbot_state7_transition(RobotsWatchBotState7TransitionInput {
                component_kind,
                phase: RobotsWatchBotComponentStatePhase::Enter,
                owner_animator_present: false,
                dynamic_entity_uid: None,
            });
            assert_eq!(plan.animation_mode_uid, Some(0x0900_00A8));
            assert_eq!(plan.channel_toggle, None);
        }
    }

    #[test]
    fn slot1_state7_exit_toggles_exact_watchbot_entity_channels_true() {
        let plan = robots_watchbot_state7_transition(RobotsWatchBotState7TransitionInput {
            component_kind: RobotsWatchBotComponentKind::Slot1Primary,
            phase: RobotsWatchBotComponentStatePhase::Exit,
            owner_animator_present: true,
            dynamic_entity_uid: Some(0x0200_0123),
        });
        assert_eq!(plan.animation_mode_uid, None);
        assert_eq!(
            plan.channel_toggle,
            Some(RobotsWatchBotChannelTogglePlan {
                apply_fixed_channels: true,
                fixed_entity_uids: [0x0200_00A8, 0x0200_0098, 0x0200_0097],
                dynamic_entity_uid: Some(0x0200_0123),
                value: true,
            })
        );
    }

    #[test]
    fn shared_component_state7_exit_is_a_true_noop() {
        for component_kind in [
            RobotsWatchBotComponentKind::Slot2Shared,
            RobotsWatchBotComponentKind::Slot3Shared,
        ] {
            let plan = robots_watchbot_state7_transition(RobotsWatchBotState7TransitionInput {
                component_kind,
                phase: RobotsWatchBotComponentStatePhase::Exit,
                owner_animator_present: true,
                dynamic_entity_uid: Some(0x0200_0123),
            });
            assert_eq!(plan.animation_mode_uid, None);
            assert_eq!(plan.channel_toggle, None);
        }
    }

    #[test]
    fn unknown_populated_component_index_fails_closed() {
        let result = robots_apply_watchbot_hit(RobotsWatchBotHitInput {
            current_component_index: 4,
            current_component_present: true,
            current_component_state: 1,
        });
        assert!(!result.reaction_accepted);
        assert!(result.query_hit_still_commits);
        assert_eq!(result.component_kind, None);
    }
}
