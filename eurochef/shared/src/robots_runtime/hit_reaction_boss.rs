use serde::Serialize;

use super::hit_reaction::{
    RobotsAcceptedHitReactionInput, RobotsAiHitReactionState, RobotsHitReactionResult,
};

pub const ROBOTS_BOSS_EXEC_HIT_FLAG_IMMEDIATE_THRESHOLD: u32 = 0x0000_0100;
pub const ROBOTS_BOSS_EXEC_HIT_SERIAL_SUPPRESSION_MASK: u32 = 0x0000_0050;
pub const ROBOTS_BOSS_EXEC_HIT_ANIM_MODE: u32 = 0x0900_002A;
pub const ROBOTS_BOSS_EXEC_FALLBACK_ANIM_MODE: u32 = 0x0900_00F7;
pub const ROBOTS_BOSS_EXEC_HIT_STATE: u32 = 6;
pub const ROBOTS_BOSS_EXEC_FALLBACK_STATE: u32 = 7;
pub const ROBOTS_BOSS_EXEC_HIT_COUNT_THRESHOLD: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsBossExecHitInput {
    /// Native Handler +0x3C0 must be non-null/non-zero.
    pub runtime_gate_present: bool,
    /// Native Handler +0x3C8.
    pub state: u32,
    /// Native Handler +0x3CC.
    pub phase: u32,
    pub source_is_player: bool,
    pub secondary_source_is_player: bool,
    pub query_flags: u32,
    pub query_serial: u16,
    /// Native Handler +0x3E0.
    pub last_query_serial: u16,
    /// Native Handler +0x3DC.
    pub player_hit_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsBossExecHitResult {
    pub reaction_accepted: bool,
    pub query_hit_still_commits: bool,
    pub state: u32,
    pub state_mirror: Option<u32>,
    pub requested_anim_mode: Option<u32>,
    pub last_query_serial: u16,
    pub player_hit_count: u32,
    /// Native helper `0x00484550`; it owns its separate eight-step event cycle.
    pub invoke_exec_cycle_helper: bool,
}

/// `XItemHandler_Boss_Exec::ApplyHit` (`0x004C99E0`).
///
/// The handler admits only states 4/5/7/8 and phases 1/4. Player-origin hits can
/// accumulate three distinct serials (unless flags 0x10/0x40 suppress counting),
/// while flag 0x100 triggers the threshold immediately. Threshold handling calls
/// the already-separated Exec cycle helper, then enters state 6 / anim 0x0900002A.
pub fn robots_apply_boss_exec_hit(input: RobotsBossExecHitInput) -> RobotsBossExecHitResult {
    let rejected = || RobotsBossExecHitResult {
        reaction_accepted: false,
        query_hit_still_commits: true,
        state: input.state,
        state_mirror: None,
        requested_anim_mode: None,
        last_query_serial: input.last_query_serial,
        player_hit_count: input.player_hit_count,
        invoke_exec_cycle_helper: false,
    };

    if !input.runtime_gate_present || !matches!(input.state, 4 | 5 | 7 | 8) {
        return rejected();
    }
    if !matches!(input.phase, 1 | 4) {
        return rejected();
    }

    let player_source = input.source_is_player || input.secondary_source_is_player;
    let mut last_query_serial = input.last_query_serial;
    let mut player_hit_count = input.player_hit_count;
    let mut threshold = false;

    if player_source {
        if input.query_flags & ROBOTS_BOSS_EXEC_HIT_FLAG_IMMEDIATE_THRESHOLD != 0 {
            threshold = true;
        } else if input.query_flags & ROBOTS_BOSS_EXEC_HIT_SERIAL_SUPPRESSION_MASK == 0
            && input.query_serial != input.last_query_serial
        {
            last_query_serial = input.query_serial;
            player_hit_count = player_hit_count.wrapping_add(1);
            threshold = player_hit_count >= ROBOTS_BOSS_EXEC_HIT_COUNT_THRESHOLD;
        }
    }

    if threshold {
        return RobotsBossExecHitResult {
            reaction_accepted: true,
            query_hit_still_commits: true,
            state: ROBOTS_BOSS_EXEC_HIT_STATE,
            state_mirror: Some(ROBOTS_BOSS_EXEC_HIT_STATE),
            requested_anim_mode: Some(ROBOTS_BOSS_EXEC_HIT_ANIM_MODE),
            last_query_serial,
            player_hit_count,
            invoke_exec_cycle_helper: true,
        };
    }

    if matches!(input.state, 5 | 8) && matches!(input.phase, 1 | 4) {
        return RobotsBossExecHitResult {
            reaction_accepted: true,
            query_hit_still_commits: true,
            state: ROBOTS_BOSS_EXEC_FALLBACK_STATE,
            state_mirror: Some(ROBOTS_BOSS_EXEC_FALLBACK_STATE),
            requested_anim_mode: Some(ROBOTS_BOSS_EXEC_FALLBACK_ANIM_MODE),
            last_query_serial,
            player_hit_count,
            invoke_exec_cycle_helper: false,
        };
    }

    RobotsBossExecHitResult {
        reaction_accepted: true,
        query_hit_still_commits: true,
        state: input.state,
        state_mirror: None,
        requested_anim_mode: None,
        last_query_serial,
        player_hit_count,
        invoke_exec_cycle_helper: false,
    }
}

pub const ROBOTS_BOSS_EXEC_CYCLE_THRESHOLD: u32 = 8;
pub const ROBOTS_BOSS_EXEC_CUTSCENE_EVENT_MASK: u32 = 0x0000_0101;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
pub struct RobotsBossExecCycleState {
    /// Native BossExec field +0xF8. `0x00484550` increments it and never resets it.
    pub threshold_invocations: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsBossExecCutsceneDispatchPlan {
    pub outer_link_index: u8,
    /// First XTrigger_Cutscene found among that outer trigger's links 0..7.
    pub nested_link_index: u8,
    pub event_mask: u32,
    /// Native `0x0044C380` receives the BossExec handler as the event source/context.
    pub pass_exec_as_event_source: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsBossExecCycleInput {
    /// For each BossExec owner link 0..7, host resolves the first nested
    /// XTrigger_Cutscene link index returned by native `0x0044CDB0(type,0)`.
    /// `None` means the outer link is absent or has no nested Cutscene link.
    pub first_nested_cutscene_by_outer_link: [Option<u8>; 8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsBossExecCycleResult {
    pub threshold_invocations: u32,
    pub graph_scan_performed: bool,
    pub cutscene_dispatch: Option<RobotsBossExecCutsceneDispatchPlan>,
    /// Native `0x00484430` runs whenever the incremented count is >= 8,
    /// regardless of whether a Cutscene target was found.
    pub invoke_exec_reset: bool,
}

/// Engine-neutral form of native BossExec cycle helper `0x00484550`.
///
/// Search order is exact: owner links 0..7, then each outer trigger's nested
/// links 0..7 for `XTrigger_Cutscene` (descriptor `0x005EA93C`). The +0xF8
/// counter is monotonic/wrapping; it is not reset by `0x00484430`.
pub fn robots_apply_boss_exec_cycle(
    state: &mut RobotsBossExecCycleState,
    input: RobotsBossExecCycleInput,
) -> RobotsBossExecCycleResult {
    state.threshold_invocations = state.threshold_invocations.wrapping_add(1);
    if state.threshold_invocations < ROBOTS_BOSS_EXEC_CYCLE_THRESHOLD {
        return RobotsBossExecCycleResult {
            threshold_invocations: state.threshold_invocations,
            graph_scan_performed: false,
            cutscene_dispatch: None,
            invoke_exec_reset: false,
        };
    }

    let cutscene_dispatch = input
        .first_nested_cutscene_by_outer_link
        .iter()
        .enumerate()
        .find_map(|(outer_link_index, nested_link_index)| {
            nested_link_index.map(|nested_link_index| RobotsBossExecCutsceneDispatchPlan {
                outer_link_index: outer_link_index as u8,
                nested_link_index,
                event_mask: ROBOTS_BOSS_EXEC_CUTSCENE_EVENT_MASK,
                pass_exec_as_event_source: true,
            })
        });

    RobotsBossExecCycleResult {
        threshold_invocations: state.threshold_invocations,
        graph_scan_performed: true,
        cutscene_dispatch,
        invoke_exec_reset: true,
    }
}

pub const ROBOTS_BOSS_EXEC_RESET_EVENT_MASK: u32 = 0x0000_0200;
pub const ROBOTS_BOSS_EXEC_RESET_GLOBAL_MASK: u32 = 0x0000_0200;
pub const ROBOTS_BOSS_EXEC_RESET_PICKUP_FLAG: u8 = 0x02;
pub const ROBOTS_BOSS_EXEC_BOMB_EXPLOSION_UID: u32 = 0x5200_0021;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsBossExecResetLocalState {
    /// Native +0xF0; non-null is released before clearing the slot.
    pub resource_handle_present: bool,
    /// Native +0xE4.
    pub field_e4: u32,
    /// Native +0xEC.
    pub field_ec: u32,
    /// Native byte +0x100.
    pub flag_100: bool,
    /// Native +0xFC.
    pub field_fc: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsBossExecResetLocalResult {
    pub release_resource_handle: bool,
    pub dispatch_linked_boss_exec_event_mask: u32,
    pub set_global_mask: u32,
}

/// Local part of native encounter reset `0x00484430`.
pub fn robots_apply_boss_exec_reset_local(
    state: &mut RobotsBossExecResetLocalState,
) -> RobotsBossExecResetLocalResult {
    let release_resource_handle = state.resource_handle_present;
    state.resource_handle_present = false;
    state.field_e4 = 0;
    state.field_ec = 0;
    state.flag_100 = false;
    state.field_fc = 0;

    RobotsBossExecResetLocalResult {
        release_resource_handle,
        dispatch_linked_boss_exec_event_mask: ROBOTS_BOSS_EXEC_RESET_EVENT_MASK,
        set_global_mask: ROBOTS_BOSS_EXEC_RESET_GLOBAL_MASK,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsBossExecResetRuntimeKind {
    Pickup,
    AiCharacter,
    BossExecBomb,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum RobotsBossExecResetRuntimeAction {
    None,
    PickupSetFlag {
        mask: u8,
    },
    AiCharacterAction {
        action: u32,
        force: bool,
    },
    BossExecBombCleanup {
        set_owner_pending_destroy: bool,
        explosion_uid: u32,
    },
}

/// Per-runtime-item branch of `0x00484430`. Host/UE enumerates live Actors;
/// shared keeps the exact class-specific reset action.
pub fn robots_boss_exec_reset_runtime_action(
    kind: RobotsBossExecResetRuntimeKind,
) -> RobotsBossExecResetRuntimeAction {
    match kind {
        RobotsBossExecResetRuntimeKind::Pickup => RobotsBossExecResetRuntimeAction::PickupSetFlag {
            mask: ROBOTS_BOSS_EXEC_RESET_PICKUP_FLAG,
        },
        RobotsBossExecResetRuntimeKind::AiCharacter => {
            RobotsBossExecResetRuntimeAction::AiCharacterAction {
                action: 0,
                force: true,
            }
        }
        RobotsBossExecResetRuntimeKind::BossExecBomb => {
            RobotsBossExecResetRuntimeAction::BossExecBombCleanup {
                set_owner_pending_destroy: true,
                explosion_uid: ROBOTS_BOSS_EXEC_BOMB_EXPLOSION_UID,
            }
        }
        RobotsBossExecResetRuntimeKind::Other => RobotsBossExecResetRuntimeAction::None,
    }
}

/// Native scans owner links 0..7 and dispatches event 0x200 only to links whose
/// inheritance chain reaches `XTrigger_BossExecutive` descriptor 0x005E9040.
pub fn robots_boss_exec_reset_link_event(link_is_boss_executive: bool) -> Option<u32> {
    link_is_boss_executive.then_some(ROBOTS_BOSS_EXEC_RESET_EVENT_MASK)
}

pub const ROBOTS_BOSS_SEWER_REQUIRED_HIT_FLAG: u32 = 0x0000_1000;
pub const ROBOTS_BOSS_SEWER_STAGE_EVENT_MASK: u32 = 0x0000_0101;
pub const ROBOTS_BOSS_SEWER_CANON_EVENT_MASK: u32 = 0x0000_0100;
pub const ROBOTS_BOSS_SEWER_SPECIAL_ANIM_DATUM_UID: u32 = 0x0D00_0006;
pub const ROBOTS_BOSS_SEWER_STAGE_SCRIPT_UID: u32 = 0x0400_0219;
/// Native `DAT_005EFFC4[0..13]` used by `0x004CC180`.
/// Stages 0..12 all reference the same script resource; slot13 is a zero sentinel.
pub const ROBOTS_BOSS_SEWER_STAGE_RESOURCE_UIDS: [u32; 14] = [
    ROBOTS_BOSS_SEWER_STAGE_SCRIPT_UID,
    ROBOTS_BOSS_SEWER_STAGE_SCRIPT_UID,
    ROBOTS_BOSS_SEWER_STAGE_SCRIPT_UID,
    ROBOTS_BOSS_SEWER_STAGE_SCRIPT_UID,
    ROBOTS_BOSS_SEWER_STAGE_SCRIPT_UID,
    ROBOTS_BOSS_SEWER_STAGE_SCRIPT_UID,
    ROBOTS_BOSS_SEWER_STAGE_SCRIPT_UID,
    ROBOTS_BOSS_SEWER_STAGE_SCRIPT_UID,
    ROBOTS_BOSS_SEWER_STAGE_SCRIPT_UID,
    ROBOTS_BOSS_SEWER_STAGE_SCRIPT_UID,
    ROBOTS_BOSS_SEWER_STAGE_SCRIPT_UID,
    ROBOTS_BOSS_SEWER_STAGE_SCRIPT_UID,
    ROBOTS_BOSS_SEWER_STAGE_SCRIPT_UID,
    0,
];

pub fn robots_boss_sewer_stage_resource_uid(stage_index: i32) -> Option<u32> {
    usize::try_from(stage_index)
        .ok()
        .and_then(|index| ROBOTS_BOSS_SEWER_STAGE_RESOURCE_UIDS.get(index).copied())
}

/// Native `0x00484C00(index)` is required only on the even/final stage branches.
/// This mapping is shared so GUI/UE resolve the exact link7 chain before calling
/// the hit reducer without duplicating the stage switch.
pub fn robots_boss_sewer_stage_link7_chain_index(next_stage_value: u8) -> Option<u8> {
    match next_stage_value {
        0 => Some(4),
        2 => Some(3),
        4 => Some(2),
        6 => Some(1),
        8 => Some(0),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsBossSewerHitState {
    /// Native Handler +0x6A0.
    pub state: u32,
    /// Whether Handler +0x69C is non-null. Native writes target+0x24 = -1 on
    /// the stage transitions that clear it.
    pub local_target_present: bool,
    /// Native Handler +0x6A8 is reset to zero after every admitted Sewer hit.
    pub field_6a8: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsBossSewerHitHostInput {
    pub owner_present: bool,
    pub creator_present: bool,
    /// Result of native `0x00484C00(index)`: starting from the creator trigger,
    /// follow EXTrigger link7 `index + 1` times. Irrelevant for default odd stages.
    pub stage_link7_target_present: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsBossSewerStageEventPlan {
    pub link7_chain_index: u8,
    pub event_mask: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsBossSewerHitResult {
    pub reaction_accepted: bool,
    pub query_hit_still_commits: bool,
    /// Native `0x004CC180(health-1)` runs before the stage switch on every
    /// admitted hit and owns the stage resource swap.
    pub stage_resource_index: Option<u8>,
    pub stage_resource_uid: Option<u32>,
    pub stage_event: Option<RobotsBossSewerStageEventPlan>,
    pub clear_local_target_handle: bool,
    pub request_special_anim_datum: Option<u32>,
    /// Default odd-stage branch calls `0x00452200` before common AI ApplyHit.
    pub clear_common_ai_local_latches: bool,
    pub common_ai_result: Option<RobotsHitReactionResult>,
}

/// Exact Handler-level state reducer for `XItemHandler_Boss_Sewer::ApplyHit`
/// (`0x004CBE10`). `ai.health` is the native byte at +0x62E.
pub fn robots_apply_boss_sewer_hit(
    state: &mut RobotsBossSewerHitState,
    ai: &mut RobotsAiHitReactionState,
    hit: RobotsAcceptedHitReactionInput,
    host: RobotsBossSewerHitHostInput,
    debug_force_kill_non_exempt: bool,
) -> RobotsBossSewerHitResult {
    let rejected = || RobotsBossSewerHitResult {
        reaction_accepted: false,
        query_hit_still_commits: true,
        stage_resource_index: None,
        stage_resource_uid: None,
        stage_event: None,
        clear_local_target_handle: false,
        request_special_anim_datum: None,
        clear_common_ai_local_latches: false,
        common_ai_result: None,
    };

    if !host.owner_present
        || matches!(state.state, 2 | 3 | 6 | 7 | 8 | 9 | 10 | 11)
        || hit.query_flags & ROBOTS_BOSS_SEWER_REQUIRED_HIT_FLAG == 0
    {
        return rejected();
    }

    let next_stage_value = ai.health.wrapping_sub(1);
    let mut result = RobotsBossSewerHitResult {
        reaction_accepted: true,
        query_hit_still_commits: true,
        stage_resource_index: Some(next_stage_value),
        stage_resource_uid: robots_boss_sewer_stage_resource_uid(next_stage_value as i32),
        stage_event: None,
        clear_local_target_handle: false,
        request_special_anim_datum: None,
        clear_common_ai_local_latches: false,
        common_ai_result: None,
    };

    match next_stage_value {
        0 => {
            if host.creator_present && host.stage_link7_target_present {
                result.stage_event = Some(RobotsBossSewerStageEventPlan {
                    link7_chain_index: robots_boss_sewer_stage_link7_chain_index(next_stage_value)
                        .expect("stage 0 has a native link7 chain"),
                    event_mask: ROBOTS_BOSS_SEWER_STAGE_EVENT_MASK,
                });
            }
            if state.local_target_present {
                result.clear_local_target_handle = true;
                state.local_target_present = false;
            }
            state.state = 11;
        }
        2 | 4 | 6 | 8 => {
            let next_state = match next_stage_value {
                2 => 10,
                4 => 9,
                6 => 8,
                8 => 7,
                _ => unreachable!(),
            };

            if next_stage_value == 6 {
                result.request_special_anim_datum = Some(ROBOTS_BOSS_SEWER_SPECIAL_ANIM_DATUM_UID);
            }

            if host.creator_present && host.stage_link7_target_present {
                result.stage_event = Some(RobotsBossSewerStageEventPlan {
                    link7_chain_index: robots_boss_sewer_stage_link7_chain_index(next_stage_value)
                        .expect("even native stage has a link7 chain"),
                    event_mask: ROBOTS_BOSS_SEWER_STAGE_EVENT_MASK,
                });
                if state.local_target_present {
                    result.clear_local_target_handle = true;
                    state.local_target_present = false;
                }
                state.state = next_state;
                ai.health = ai.health.wrapping_sub(1);
            }
        }
        _ => {
            state.state = 6;
            if state.local_target_present {
                result.clear_local_target_handle = true;
                state.local_target_present = false;
            }
            result.clear_common_ai_local_latches = true;
            // Native ignores the common AI callback return and itself returns 1.
            result.common_ai_result = Some(ai.apply_hit(hit, debug_force_kill_non_exempt));
        }
    }

    state.field_6a8 = 0;
    result
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsBossSewerCanonHitInput {
    pub source_is_player: bool,
    pub secondary_source_is_player: bool,
    /// Native Handler +0x390.
    pub armed_latch: bool,
    pub owner_present: bool,
    pub creator_present: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsBossSewerCanonHitResult {
    pub reaction_accepted: bool,
    pub query_hit_still_commits: bool,
    pub dispatch_creator_event_mask: Option<u32>,
}

/// `XItemHandler_Boss_Sewer_Canon::ApplyHit` (`0x004CCC20`).
pub fn robots_apply_boss_sewer_canon_hit(
    input: RobotsBossSewerCanonHitInput,
) -> RobotsBossSewerCanonHitResult {
    let accepted = (input.source_is_player || input.secondary_source_is_player)
        && input.armed_latch
        && input.owner_present
        && input.creator_present;
    RobotsBossSewerCanonHitResult {
        reaction_accepted: accepted,
        query_hit_still_commits: true,
        dispatch_creator_event_mask: accepted.then_some(ROBOTS_BOSS_SEWER_CANON_EVENT_MASK),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::robots_runtime::hit_reaction::ROBOTS_AI_HIT_CAPABILITY_FLAG;

    fn exec() -> RobotsBossExecHitInput {
        RobotsBossExecHitInput {
            runtime_gate_present: true,
            state: 4,
            phase: 1,
            source_is_player: true,
            secondary_source_is_player: false,
            query_flags: 0,
            query_serial: 10,
            last_query_serial: u16::MAX,
            player_hit_count: 0,
        }
    }

    #[test]
    fn boss_exec_rejects_wrong_gate_state_or_phase() {
        let mut input = exec();
        input.runtime_gate_present = false;
        assert!(!robots_apply_boss_exec_hit(input).reaction_accepted);
        input = exec();
        input.state = 6;
        assert!(!robots_apply_boss_exec_hit(input).reaction_accepted);
        input = exec();
        input.phase = 2;
        assert!(!robots_apply_boss_exec_hit(input).reaction_accepted);
    }

    #[test]
    fn boss_exec_counts_distinct_player_serials_and_thresholds_on_third() {
        let mut input = exec();
        input.player_hit_count = 2;
        let result = robots_apply_boss_exec_hit(input);
        assert!(result.reaction_accepted);
        assert_eq!(result.player_hit_count, 3);
        assert_eq!(result.last_query_serial, 10);
        assert!(result.invoke_exec_cycle_helper);
        assert_eq!(result.state, 6);
        assert_eq!(
            result.requested_anim_mode,
            Some(ROBOTS_BOSS_EXEC_HIT_ANIM_MODE)
        );
    }

    #[test]
    fn boss_exec_duplicate_or_suppressed_serial_does_not_increment_count() {
        let mut input = exec();
        input.last_query_serial = 10;
        input.player_hit_count = 2;
        let result = robots_apply_boss_exec_hit(input);
        assert_eq!(result.player_hit_count, 2);
        assert!(!result.invoke_exec_cycle_helper);

        input = exec();
        input.player_hit_count = 2;
        input.query_flags = 0x10;
        let result = robots_apply_boss_exec_hit(input);
        assert_eq!(result.player_hit_count, 2);
        assert!(!result.invoke_exec_cycle_helper);
    }

    #[test]
    fn boss_exec_flag_0x100_thresholds_immediately_and_state5_has_fallback_hit() {
        let mut input = exec();
        input.query_flags = ROBOTS_BOSS_EXEC_HIT_FLAG_IMMEDIATE_THRESHOLD;
        let immediate = robots_apply_boss_exec_hit(input);
        assert!(immediate.invoke_exec_cycle_helper);
        assert_eq!(immediate.state, 6);

        input = exec();
        input.source_is_player = false;
        input.state = 5;
        let fallback = robots_apply_boss_exec_hit(input);
        assert!(fallback.reaction_accepted);
        assert_eq!(fallback.state, 7);
        assert_eq!(
            fallback.requested_anim_mode,
            Some(ROBOTS_BOSS_EXEC_FALLBACK_ANIM_MODE)
        );
    }

    #[test]
    fn boss_exec_cycle_waits_until_eighth_invocation_then_dispatches_first_nested_cutscene() {
        let mut state = RobotsBossExecCycleState {
            threshold_invocations: 7,
        };
        let result = robots_apply_boss_exec_cycle(
            &mut state,
            RobotsBossExecCycleInput {
                first_nested_cutscene_by_outer_link: [
                    None,
                    None,
                    Some(5),
                    Some(1),
                    None,
                    None,
                    None,
                    None,
                ],
            },
        );
        assert_eq!(result.threshold_invocations, 8);
        assert!(result.graph_scan_performed);
        assert!(result.invoke_exec_reset);
        assert_eq!(
            result.cutscene_dispatch,
            Some(RobotsBossExecCutsceneDispatchPlan {
                outer_link_index: 2,
                nested_link_index: 5,
                event_mask: 0x101,
                pass_exec_as_event_source: true,
            })
        );
    }

    #[test]
    fn boss_exec_cycle_counter_is_not_reset_after_threshold() {
        let mut state = RobotsBossExecCycleState {
            threshold_invocations: 8,
        };
        let result = robots_apply_boss_exec_cycle(
            &mut state,
            RobotsBossExecCycleInput {
                first_nested_cutscene_by_outer_link: [
                    Some(0),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                ],
            },
        );
        assert_eq!(result.threshold_invocations, 9);
        assert!(result.graph_scan_performed);
        assert!(result.invoke_exec_reset);
        assert_eq!(result.cutscene_dispatch.unwrap().outer_link_index, 0);
    }

    #[test]
    fn boss_exec_cycle_threshold_scan_resets_even_without_cutscene_target() {
        let mut state = RobotsBossExecCycleState {
            threshold_invocations: 7,
        };
        let result = robots_apply_boss_exec_cycle(
            &mut state,
            RobotsBossExecCycleInput {
                first_nested_cutscene_by_outer_link: [None; 8],
            },
        );
        assert_eq!(result.threshold_invocations, 8);
        assert!(result.graph_scan_performed);
        assert_eq!(result.cutscene_dispatch, None);
        assert!(result.invoke_exec_reset);
    }

    #[test]
    fn boss_exec_cycle_before_threshold_does_not_scan_or_reset() {
        let mut state = RobotsBossExecCycleState {
            threshold_invocations: 5,
        };
        let result = robots_apply_boss_exec_cycle(
            &mut state,
            RobotsBossExecCycleInput {
                first_nested_cutscene_by_outer_link: [Some(0); 8],
            },
        );
        assert_eq!(result.threshold_invocations, 6);
        assert!(!result.graph_scan_performed);
        assert_eq!(result.cutscene_dispatch, None);
        assert!(!result.invoke_exec_reset);
    }

    #[test]
    fn boss_exec_reset_clears_exact_local_fields_and_requests_global_mask() {
        let mut state = RobotsBossExecResetLocalState {
            resource_handle_present: true,
            field_e4: 10,
            field_ec: 20,
            flag_100: true,
            field_fc: 30,
        };
        let result = robots_apply_boss_exec_reset_local(&mut state);
        assert!(result.release_resource_handle);
        assert_eq!(result.dispatch_linked_boss_exec_event_mask, 0x200);
        assert_eq!(result.set_global_mask, 0x200);
        assert_eq!(
            state,
            RobotsBossExecResetLocalState {
                resource_handle_present: false,
                field_e4: 0,
                field_ec: 0,
                flag_100: false,
                field_fc: 0,
            }
        );
    }

    #[test]
    fn boss_exec_reset_runtime_classes_map_to_exact_native_actions() {
        assert_eq!(
            robots_boss_exec_reset_runtime_action(RobotsBossExecResetRuntimeKind::Pickup),
            RobotsBossExecResetRuntimeAction::PickupSetFlag { mask: 0x02 }
        );
        assert_eq!(
            robots_boss_exec_reset_runtime_action(RobotsBossExecResetRuntimeKind::AiCharacter),
            RobotsBossExecResetRuntimeAction::AiCharacterAction {
                action: 0,
                force: true,
            }
        );
        assert_eq!(
            robots_boss_exec_reset_runtime_action(RobotsBossExecResetRuntimeKind::BossExecBomb),
            RobotsBossExecResetRuntimeAction::BossExecBombCleanup {
                set_owner_pending_destroy: true,
                explosion_uid: 0x5200_0021,
            }
        );
        assert_eq!(
            robots_boss_exec_reset_runtime_action(RobotsBossExecResetRuntimeKind::Other),
            RobotsBossExecResetRuntimeAction::None
        );
    }

    #[test]
    fn boss_exec_reset_link_event_only_targets_boss_executive_triggers() {
        assert_eq!(robots_boss_exec_reset_link_event(true), Some(0x200));
        assert_eq!(robots_boss_exec_reset_link_event(false), None);
    }

    fn ai(health: u8) -> RobotsAiHitReactionState {
        RobotsAiHitReactionState {
            query_flags_snapshot: 0,
            query_serial_snapshot: u16::MAX,
            hit_metadata_snapshot: 0,
            last_query_serial: u16::MAX,
            capability_flags: ROBOTS_AI_HIT_CAPABILITY_FLAG,
            health,
            got_hit_latch: false,
            owner_category: 1,
        }
    }

    fn sewer_hit(serial: u16) -> RobotsAcceptedHitReactionInput {
        RobotsAcceptedHitReactionInput {
            query_flags: ROBOTS_BOSS_SEWER_REQUIRED_HIT_FLAG,
            query_serial: serial,
            hit_metadata: 0,
            source_is_candidate_owner: false,
            secondary_source_is_candidate_owner: false,
        }
    }

    fn sewer_state() -> RobotsBossSewerHitState {
        RobotsBossSewerHitState {
            state: 1,
            local_target_present: true,
            field_6a8: 9,
        }
    }

    #[test]
    fn sewer_stage_resource_table_matches_native_14_dword_table() {
        for stage in 0..=12 {
            assert_eq!(
                robots_boss_sewer_stage_resource_uid(stage),
                Some(ROBOTS_BOSS_SEWER_STAGE_SCRIPT_UID)
            );
        }
        assert_eq!(robots_boss_sewer_stage_resource_uid(13), Some(0));
        assert_eq!(robots_boss_sewer_stage_resource_uid(-1), None);
        assert_eq!(robots_boss_sewer_stage_resource_uid(14), None);
    }

    #[test]
    fn sewer_requires_owner_allowed_state_and_flag_0x1000() {
        let mut state = sewer_state();
        let mut ai = ai(9);
        let mut host = RobotsBossSewerHitHostInput {
            owner_present: false,
            creator_present: true,
            stage_link7_target_present: true,
        };
        assert!(
            !robots_apply_boss_sewer_hit(&mut state, &mut ai, sewer_hit(1), host, false)
                .reaction_accepted
        );
        host.owner_present = true;
        let mut hit = sewer_hit(2);
        hit.query_flags = 0;
        assert!(
            !robots_apply_boss_sewer_hit(&mut state, &mut ai, hit, host, false).reaction_accepted
        );
    }

    #[test]
    fn sewer_even_stage_only_decrements_health_when_link7_chain_target_exists() {
        let mut state = sewer_state();
        let mut ai_state = ai(9); // next stage value 8 -> creator link7 chain index 0 / state7
        let missing = robots_apply_boss_sewer_hit(
            &mut state,
            &mut ai_state,
            sewer_hit(3),
            RobotsBossSewerHitHostInput {
                owner_present: true,
                creator_present: true,
                stage_link7_target_present: false,
            },
            false,
        );
        assert!(missing.reaction_accepted);
        assert_eq!(missing.stage_resource_index, Some(8));
        assert_eq!(
            missing.stage_resource_uid,
            Some(ROBOTS_BOSS_SEWER_STAGE_SCRIPT_UID)
        );
        assert_eq!(ai_state.health, 9);
        assert_eq!(state.state, 1);
        assert_eq!(state.field_6a8, 0);

        state = sewer_state();
        ai_state = ai(9);
        let found = robots_apply_boss_sewer_hit(
            &mut state,
            &mut ai_state,
            sewer_hit(4),
            RobotsBossSewerHitHostInput {
                owner_present: true,
                creator_present: true,
                stage_link7_target_present: true,
            },
            false,
        );
        assert_eq!(ai_state.health, 8);
        assert_eq!(state.state, 7);
        assert_eq!(
            found.stage_event,
            Some(RobotsBossSewerStageEventPlan {
                link7_chain_index: 0,
                event_mask: 0x101,
            })
        );
        assert!(found.clear_local_target_handle);
    }

    #[test]
    fn sewer_health7_stage_requests_special_anim_datum_independent_of_link7_target() {
        let mut state = sewer_state();
        let mut ai_state = ai(7); // next stage value 6
        let result = robots_apply_boss_sewer_hit(
            &mut state,
            &mut ai_state,
            sewer_hit(5),
            RobotsBossSewerHitHostInput {
                owner_present: true,
                creator_present: false,
                stage_link7_target_present: false,
            },
            false,
        );
        assert_eq!(result.request_special_anim_datum, Some(0x0D00_0006));
        assert_eq!(ai_state.health, 7);
        assert_eq!(state.state, 1);
    }

    #[test]
    fn sewer_final_stage_enters_state11_without_decrementing_health() {
        let mut state = sewer_state();
        let mut ai_state = ai(1);
        let result = robots_apply_boss_sewer_hit(
            &mut state,
            &mut ai_state,
            sewer_hit(6),
            RobotsBossSewerHitHostInput {
                owner_present: true,
                creator_present: true,
                stage_link7_target_present: true,
            },
            false,
        );
        assert!(result.reaction_accepted);
        assert_eq!(result.stage_resource_index, Some(0));
        assert_eq!(ai_state.health, 1);
        assert_eq!(state.state, 11);
        assert_eq!(result.stage_event.unwrap().link7_chain_index, 4);
    }

    #[test]
    fn sewer_default_stage_enters_state6_and_runs_common_ai_without_using_its_return() {
        let mut state = sewer_state();
        let mut ai_state = ai(10); // next stage value 9 -> default branch
        let result = robots_apply_boss_sewer_hit(
            &mut state,
            &mut ai_state,
            sewer_hit(7),
            RobotsBossSewerHitHostInput {
                owner_present: true,
                creator_present: true,
                stage_link7_target_present: true,
            },
            false,
        );
        assert!(result.reaction_accepted);
        assert_eq!(state.state, 6);
        assert!(result.clear_common_ai_local_latches);
        let common = result.common_ai_result.unwrap();
        assert!(common.reaction_accepted);
        assert_eq!(ai_state.health, 9); // stage resource saw 9 first; common AI then removes one HP
        assert_eq!(ai_state.last_query_serial, 7);
        assert!(ai_state.got_hit_latch);
    }

    #[test]
    fn sewer_canon_requires_player_source_armed_latch_owner_and_creator() {
        let accepted = robots_apply_boss_sewer_canon_hit(RobotsBossSewerCanonHitInput {
            source_is_player: true,
            secondary_source_is_player: false,
            armed_latch: true,
            owner_present: true,
            creator_present: true,
        });
        assert!(accepted.reaction_accepted);
        assert_eq!(accepted.dispatch_creator_event_mask, Some(0x100));

        let rejected = robots_apply_boss_sewer_canon_hit(RobotsBossSewerCanonHitInput {
            source_is_player: false,
            secondary_source_is_player: false,
            armed_latch: true,
            owner_present: true,
            creator_present: true,
        });
        assert!(!rejected.reaction_accepted);
        assert!(rejected.query_hit_still_commits);
        assert_eq!(rejected.dispatch_creator_event_mask, None);
    }
}
