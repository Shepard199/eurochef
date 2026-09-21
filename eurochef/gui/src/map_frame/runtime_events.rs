use super::runtime_ai::{locomotion_root_motion_policy, monster_nav_regions};
use super::*;

const ROBOTS_CAMERA_SEQUENCE_TYPE: u32 = 85;
const ROBOTS_SCRIPT_TRIGGER_TYPE: u32 = 4;
const ROBOTS_MONSTER_TRANSPORTER_TYPE: u32 = 73;
// ProcessedTrigger.ttype stores the serialized EXGeoTriggerType code, not the
// native runtime class-registry id. Keep this namespace explicit here.
const ROBOTS_ACTIVATION_PAD_TYPE: u32 = 36;
const ROBOTS_COUNTER_TYPE: u32 = 6;
const ROBOTS_TIMER_TYPE: u32 = 5;
const ROBOTS_MESSAGE_RELAY_TYPE: u32 = 13;
const ROBOTS_GROUP_TYPE: u32 = 14;
const ROBOTS_DOOR_TYPE: u32 = 21;
const ROBOTS_INTERACT_TYPES: [u32; 4] = [22, 23, 24, 49];
const ROBOTS_FIX_SWITCH_TYPE: u32 = 31;
const ROBOTS_DISPLAY_MESSAGE_TYPE: u32 = 39;
const ROBOTS_BALL_TRACK_TYPE: u32 = 40;
const ROBOTS_PATTERN_TYPE: u32 = 43;
const ROBOTS_SHOP_TYPE: u32 = 44;
const ROBOTS_CHANGE_LEVEL_TYPE: u32 = 15;
const ROBOTS_CUTSCENE_TYPE: u32 = 19;
const ROBOTS_NPC_TYPE: u32 = 48;
const ROBOTS_FLUID_TYPE: u32 = 50;
const ROBOTS_DISTANCE_TYPE: u32 = 57;
const ROBOTS_MISSION_TYPE: u32 = 53;
const ROBOTS_CLOCK_TYPE: u32 = 55;
const ROBOTS_SLIDE_UNDER_TYPE: u32 = 59;
const ROBOTS_ALERT_ICON_TYPE: u32 = 61;
const ROBOTS_WATCHBOT_TYPE: u32 = 60;
const ROBOTS_CAMERA_VALUES_TYPE: u32 = 35;
const ROBOTS_BOSS_EXECUTIVE_TYPE: u32 = 68;
const ROBOTS_BOSS_SEWER_TYPE: u32 = 75;
const ROBOTS_BOSS_SEWER_CANON_TYPE: u32 = 76;
const ROBOTS_LIGHT_TYPE: u32 = 77;
const ROBOTS_TUTORIAL_TYPE: u32 = 78;
const ROBOTS_SCRIPT_DYNAMIC_RAYCAST_FLAG: u32 = 0x100;
const ROBOTS_SCRIPT_UNSUPPORTED_DISTANCE_FLAGS: u32 = 0x0008_0000 | 0x0800_0000;
const ROBOTS_NATIVE_FIXED_SECONDS: f64 = 1.0 / 60.0;
const ROBOTS_NATIVE_MAX_FIXED_STEPS_PER_FRAME: usize = 200_000;

fn take_global_gameplay_rng_draws<const N: usize>(
    rng: &mut RuntimeRobotsGlobalRngState,
) -> Option<[u32; N]> {
    let mut draws = [0u32; N];
    for draw in &mut draws {
        *draw = rng.next_u32()?;
    }
    Some(draws)
}

fn native_cutscene_player_action_uid(action: RobotsCutscenePlayerAction) -> Option<u32> {
    Some(match action {
        RobotsCutscenePlayerAction::Action48000000 => ROBOTS_PLAYER_ACTION_48000000,
        RobotsCutscenePlayerAction::Action48000003 => ROBOTS_PLAYER_ACTION_48000003,
        RobotsCutscenePlayerAction::Action48000004 => ROBOTS_PLAYER_ACTION_48000004,
        RobotsCutscenePlayerAction::Action48000005 => ROBOTS_PLAYER_ACTION_SCRAP_GUN,
        RobotsCutscenePlayerAction::Action48000008 => ROBOTS_PLAYER_ACTION_48000008,
        RobotsCutscenePlayerAction::Action4800000b => ROBOTS_PLAYER_ACTION_4800000B,
        RobotsCutscenePlayerAction::SetHandlerFlag4000 => return None,
    })
}

fn apply_native_cutscene_player_property(
    player_items: &mut RobotsPlayerItemState,
    mode: Option<i32>,
) {
    let Some(mode) = mode else {
        return;
    };
    player_items.set_cutscene_property_enabled(mode == 1);
}

fn apply_native_cutscene_player_action(
    runtime: &mut RobotsPlayerActionRuntime,
    player_state_6de: u8,
    player_items: &RobotsPlayerItemState,
    action: RobotsCutscenePlayerAction,
) {
    if action == RobotsCutscenePlayerAction::SetHandlerFlag4000 {
        runtime.set_cutscene_flag_4000();
        return;
    }
    let Some(action_uid) = native_cutscene_player_action_uid(action) else {
        return;
    };
    runtime.request_cutscene_action(action_uid, player_state_6de, |uid| {
        player_items.stored_current(uid) as i32
    });
}

#[cfg(test)]
fn robots_fluid_setup_draws(trigger: &ProcessedTrigger) -> usize {
    if trigger.ttype != ROBOTS_FLUID_TYPE {
        return 0;
    }
    let width = trigger.data.first().copied().flatten().unwrap_or_default();
    let count = trigger.data.get(5).copied().flatten().unwrap_or_default() as i32;
    if width == 2 || count <= 1 {
        0
    } else {
        (count - 1) as usize
    }
}

fn preview_fluid_trigger_manager_shared_rng(
    state: RuntimeScriptTriggerLifecycleState,
    creates_this_update: u32,
    action: crate::map_runtime::RuntimeCommonTriggerLifecycleAction,
    flags: u32,
    proximity_factor: f32,
    rng: RuntimeRobotsGlobalRngState,
    shared_rng_order_known: bool,
    initial_shared_rng_draws: Option<u32>,
    has_unowned_periodic_rng: bool,
) -> Option<(
    RuntimeScriptTriggerLifecycleState,
    u32,
    RuntimeRobotsGlobalRngState,
)> {
    let mut next_state = state;
    let mut next_creates = creates_this_update;
    next_state.advance_trigger_manager(action, flags, proximity_factor, &mut next_creates);
    let created = !state.xitem_exists && next_state.xitem_exists;
    if !created {
        return Some((next_state, next_creates, rng));
    }
    if !shared_rng_order_known || has_unowned_periodic_rng {
        return None;
    }

    let mut next_rng = rng;
    for _ in 0..initial_shared_rng_draws? {
        next_rng.next_u32()?;
    }
    Some((next_state, next_creates, next_rng))
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct NativePickupTriggerManagerPreview {
    lifecycle: RuntimeScriptTriggerLifecycleState,
    pickup: RobotsPickupRuntimeState,
    creates_this_update: u32,
    rng: RuntimeRobotsGlobalRngState,
    spawned: Option<RobotsPickupSpawnPlan>,
}

fn preview_pickup_trigger_manager_shared_rng(
    lifecycle: RuntimeScriptTriggerLifecycleState,
    pickup: RobotsPickupRuntimeState,
    creates_this_update: u32,
    action: crate::map_runtime::RuntimeCommonTriggerLifecycleAction,
    flags: u32,
    proximity_factor: f32,
    serialized_trigger_type: u32,
    data: &[Option<u32>],
    player_item_preflight_result: Option<u32>,
    rng: RuntimeRobotsGlobalRngState,
    shared_rng_order_known: bool,
) -> Option<NativePickupTriggerManagerPreview> {
    let mut next_lifecycle = lifecycle;
    let mut next_pickup = pickup;
    let mut next_creates = creates_this_update;
    let mut next_rng = rng;
    let mut spawned = None;
    let mut create_allowed = false;

    if lifecycle.would_attempt_create(action, creates_this_update) {
        match next_pickup.prepare_create(
            serialized_trigger_type,
            data,
            player_item_preflight_result,
        ) {
            RobotsPickupCreateGate::Ready(layout) => {
                if !shared_rng_order_known {
                    return None;
                }
                let random_unit = next_rng.next_unit_f32()?;
                spawned = Some(layout.with_random_unit(random_unit)?);
                create_allowed = true;
            }
            RobotsPickupCreateGate::Suppressed | RobotsPickupCreateGate::InvalidLayout => {}
            RobotsPickupCreateGate::UnresolvedPlayerItemPreflight => return None,
        }
    }

    next_lifecycle.advance_trigger_manager_with_create_gate(
        action,
        flags,
        proximity_factor,
        &mut next_creates,
        create_allowed,
    );
    Some(NativePickupTriggerManagerPreview {
        lifecycle: next_lifecycle,
        pickup: next_pickup,
        creates_this_update: next_creates,
        rng: next_rng,
        spawned,
    })
}

fn commit_native_slide_under_player_step(
    runtime: &mut NativePlayerFocusRuntimeState,
    key: u64,
    step: RobotsSlideUnderStep,
) -> bool {
    match step.focus {
        RobotsPlayerFocusDecision::KeepCurrent => {}
        RobotsPlayerFocusDecision::ClearCurrent => runtime.current_owner = None,
        RobotsPlayerFocusDecision::SelectCandidate => {
            runtime.current_owner = Some(NativePlayerFocusOwner {
                key,
                category: ROBOTS_XITEM_CATEGORY_SLIDE_UNDER,
            });
        }
    }

    match step.slide_under {
        RobotsSlideUnderOwnerDecision::KeepCurrent => false,
        RobotsSlideUnderOwnerDecision::SelectCandidate => {
            runtime.current_slide_under = Some(key);
            false
        }
        RobotsSlideUnderOwnerDecision::ClearCurrentAndDeactivateLinks => {
            runtime.current_slide_under = None;
            true
        }
    }
}

fn active_camera_trigger_after_event(
    map: &ProcessedMap,
    ownership: &mut NativeCameraOwnershipRuntime,
    current: Option<usize>,
    trigger_index: usize,
    trigger_type: u32,
    event_mask: u32,
) -> Option<usize> {
    if trigger_type != 1 {
        return current;
    }
    let mut active = current;
    // XTrigger_Camera::Event at 0x00480630 tests 0x100 first and then
    // independently tests 0x200. Mirror that order because +0xF0 maintains
    // native owner/proxy nesting at controller +0xB24/+0xB28.
    if event_mask & ROBOTS_EVENT_ACTIVATE != 0 && ownership.activate(map, trigger_index).is_some() {
        active = Some(trigger_index);
    }
    if event_mask & ROBOTS_EVENT_DEACTIVATE != 0 {
        let current_mode = active
            .and_then(|index| map.triggers.get(index))
            .and_then(|trigger| robots_camera_mode(trigger.ttype, &trigger.data));
        let current_path_hashcode = active
            .and_then(|index| map.triggers.get(index))
            .and_then(|trigger| robots_trigger_path_hash(trigger.ttype, &trigger.data));
        if let Some(current_mode) = current_mode {
            if ownership
                .release(map, trigger_index, current_mode, current_path_hashcode)
                .is_some()
                && ownership.nesting_count == 0
            {
                active = None;
            }
        }
    }
    active
}

#[allow(dead_code)] // Production host seam; native XHudShop activation ingress is the next boundary.
fn apply_native_shop_purchase(
    shop_database: Option<&RobotsShopDatabase>,
    inventory_definitions: &[RobotsInventoryDefinition],
    gameplay_state: &mut RobotsScriptGameplayState,
    player_items: &mut RobotsPlayerItemState,
    shop_uid: u32,
    slot: usize,
    scrap_removal_locked: bool,
) -> Option<RobotsShopPurchaseOutcome> {
    let database = shop_database?;
    let player_item_definition_uids = database.ordinary_player_item_definition_uids();
    database.purchase_offer(
        shop_uid,
        slot,
        inventory_definitions,
        &mut gameplay_state.inventory,
        player_items,
        &player_item_definition_uids,
        scrap_removal_locked,
    )
}

#[allow(dead_code)] // Production host seam; invoked by UE/input integration rather than current editor UI.
fn interact_native_watchbot_focus_transition(
    game_control: &mut RobotsGameControlRuntime,
    player_action: &mut RobotsPlayerActionRuntime,
    player_focus: &mut NativePlayerFocusRuntimeState,
    watchbot_owner: &mut RobotsWatchbotOwnerRuntime,
    focused_xitem_snapshot: RobotsWatchbotComponentTransitionSnapshotBits,
    configured_exit_sound_uid_4d4: Option<u32>,
    global_rng_draw: Option<u32>,
) -> Option<RobotsWatchbotControlEnterOutcome> {
    let focused = player_focus.current_owner?;
    if focused.category != ROBOTS_XITEM_CATEGORY_ACTIVATION_PAD {
        return None;
    }

    player_focus.current_interaction_code = ROBOTS_WATCHBOT_INTERACTION_CODE;
    let outcome = game_control.enter_watchbot_control(
        watchbot_owner,
        focused_xitem_snapshot,
        configured_exit_sound_uid_4d4,
        global_rng_draw,
    );
    // `0x004BCCA0` clears Player+0x52C after every accepted category0x4C
    // interaction, even if current GameWnd mode prevents `0x004B0120`.
    player_focus.current_owner = None;
    if let Some(outcome) = outcome {
        player_focus.player_state = outcome.next_player_state_6de;
        player_action.arm_watchbot_exit_sound(outcome.exit_sound_uid_4d4);
        return Some(outcome);
    }
    None
}

#[allow(dead_code)] // Production host seam; serviced by Player/UE integration, not editor UI.
fn service_native_watchbot_control_exit_transition(
    game_control: &mut RobotsGameControlRuntime,
    player_action: &mut RobotsPlayerActionRuntime,
    player_state_6de: &mut u8,
    player_items: &RobotsPlayerItemState,
    exit_sound_uid_4d4: Option<u32>,
) -> Option<RobotsPlayerWatchbotControlExitOutcome> {
    if !game_control.watchbot_exit_pending() {
        return None;
    }
    let outcome = player_action.finish_watchbot_control_exit(
        game_control.mode_50f,
        *player_state_6de,
        exit_sound_uid_4d4,
        |uid| player_items.stored_current(uid) as i32,
    )?;
    *player_state_6de = outcome.next_player_state_6de;
    game_control.complete_watchbot_exit();
    Some(outcome)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NativeWatchbotComponentTransitionOutcome {
    pub transition: RobotsWatchbotComponentTransitionStep,
    /// Present only for committed non-mode3 -> mode2 entry `0x00495490`.
    pub mode2_entry_placement: Option<RobotsWatchbotMode2EntryPlacementPlan>,
    /// Native `0x00490590` invokes mode3 vslot +0x38 after setup; the host resolves
    /// Handler +0x488 to the real EXGeoPath and calls the existing bind seam.
    pub mode3_path_rebind_required: bool,
    /// Common component setup `0x004935A0` consumes exactly one process-global draw.
    pub setup_rng_draw: Option<u32>,
}

#[allow(dead_code)] // Thin owner-only core reused by MapFrame and focused regressions.
fn service_native_watchbot_component_mode_transition_runtime(
    game_control: &mut RobotsGameControlRuntime,
    watchbot_owner: &mut RobotsWatchbotOwnerRuntime,
    mode1: &mut RobotsWatchbotMode1Runtime,
    mode2: &mut RobotsWatchbotMode2Runtime,
    mode3: &mut RobotsWatchbotMode3Runtime,
    global_rng: &mut RuntimeRobotsGlobalRngState,
    gate: RobotsWatchbotComponentTransitionGate,
) -> Option<NativeWatchbotComponentTransitionOutcome> {
    let previous_mode = watchbot_owner.current_component_mode_4a0;
    let pending_mode = watchbot_owner.pending_component_mode_4a4;
    let previous_mode3_velocity = mode3.velocity_xyzw_30;

    if pending_mode == ROBOTS_WATCHBOT_CONTROL_COMPONENT_MODE
        && previous_mode != ROBOTS_WATCHBOT_TRIGGER_PATH_MODE
        && watchbot_owner.pending_component_snapshot.is_none()
    {
        return None;
    }

    let mut preview_owner = *watchbot_owner;
    let preview = preview_owner.service_component_mode_transition(gate)?;
    let setup_rng_draw = if preview.commit_transition {
        Some(global_rng.next_u32()?)
    } else {
        None
    };

    let transition = watchbot_owner.service_component_mode_transition(gate)?;
    if transition.request_game_control_mode_zero {
        game_control.set_mode(ROBOTS_GAME_CONTROL_MODE_DEFAULT);
    }

    if !transition.commit_transition {
        return Some(NativeWatchbotComponentTransitionOutcome {
            transition,
            mode2_entry_placement: None,
            mode3_path_rebind_required: false,
            setup_rng_draw: None,
        });
    }

    let mode = transition.requested_mode;
    let mode2_entry_placement = if mode == ROBOTS_WATCHBOT_CONTROL_COMPONENT_MODE
        && previous_mode != ROBOTS_WATCHBOT_TRIGGER_PATH_MODE
    {
        let snapshot = transition.transition_snapshot?;
        Some(watchbot_mode2_entry_placement_plan(
            snapshot.position_xyzw(),
            snapshot.rotation_xyzw(),
        ))
    } else {
        None
    };

    if mode == ROBOTS_WATCHBOT_TARGET_FOLLOW_COMPONENT_MODE {
        mode1.enter_component_with_setup_draw(setup_rng_draw);
    }
    if mode == ROBOTS_WATCHBOT_CONTROL_COMPONENT_MODE {
        mode2.enter_component_with_setup_draw(
            previous_mode,
            previous_mode3_velocity,
            setup_rng_draw,
        );
    } else {
        *mode2 = RobotsWatchbotMode2Runtime::default();
    }
    *mode3 = RobotsWatchbotMode3Runtime::default();

    Some(NativeWatchbotComponentTransitionOutcome {
        transition,
        mode2_entry_placement,
        mode3_path_rebind_required: mode == ROBOTS_WATCHBOT_TRIGGER_PATH_MODE,
        setup_rng_draw,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NativeWatchbotHitOutcome {
    pub hit: RobotsWatchBotHitResult,
    /// Native `0x004937C0(state, force=0)` commits only when the requested state
    /// differs from the current component state. Mode1 remains host-owned; mode2/3
    /// are committed into their existing shared runtimes below.
    pub committed_state_request: Option<RobotsWatchBotComponentStateRequest>,
    /// State7 enter vslot +0x34 side effects. None when the setter is a same-state
    /// no-op or the slot2/3 state12 guard suppresses the request.
    pub state7_entry: Option<RobotsWatchBotState7TransitionPlan>,
}

#[allow(dead_code)] // Thin owner/runtime core reused by MapFrame and focused regressions.
fn apply_native_watchbot_hit_runtime(
    watchbot_owner: &RobotsWatchbotOwnerRuntime,
    mode2: &mut RobotsWatchbotMode2Runtime,
    mode3: &mut RobotsWatchbotMode3Runtime,
    mode1_current_component_state: u32,
    current_component_present: bool,
) -> NativeWatchbotHitOutcome {
    let component_mode = watchbot_owner.current_component_mode_4a0;
    let current_component_state = match component_mode {
        ROBOTS_WATCHBOT_TARGET_FOLLOW_COMPONENT_MODE => mode1_current_component_state,
        ROBOTS_WATCHBOT_CONTROL_COMPONENT_MODE => mode2.internal_state,
        ROBOTS_WATCHBOT_TRIGGER_PATH_MODE => mode3.internal_state,
        _ => 0,
    };
    let hit = robots_apply_watchbot_hit(RobotsWatchBotHitInput {
        current_component_index: component_mode,
        current_component_present,
        current_component_state,
    });
    let Some(request) = hit.state_request else {
        return NativeWatchbotHitOutcome {
            hit,
            committed_state_request: None,
            state7_entry: None,
        };
    };
    let Some(component_kind) = hit.component_kind else {
        return NativeWatchbotHitOutcome {
            hit,
            committed_state_request: None,
            state7_entry: None,
        };
    };
    if current_component_state == request.state && !request.force {
        return NativeWatchbotHitOutcome {
            hit,
            committed_state_request: None,
            state7_entry: None,
        };
    }

    debug_assert_eq!(request.state, ROBOTS_WATCHBOT_COMPONENT_HIT_STATE);
    match component_kind {
        RobotsWatchBotComponentKind::Slot1Primary => {}
        RobotsWatchBotComponentKind::Slot2Shared => mode2.set_internal_state(request.state),
        RobotsWatchBotComponentKind::Slot3Shared => mode3.set_internal_state(request.state),
    }
    NativeWatchbotHitOutcome {
        hit,
        committed_state_request: Some(request),
        state7_entry: Some(robots_watchbot_state7_transition(
            RobotsWatchBotState7TransitionInput {
                component_kind,
                phase: RobotsWatchBotComponentStatePhase::Enter,
                owner_animator_present: false,
                dynamic_entity_uid: None,
            },
        )),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeBossExecCycleOutcome {
    pub cycle: RobotsBossExecCycleResult,
    /// Concrete Cutscene target resolved from the native 8x8 owner-link scan.
    pub cutscene_trigger_index: Option<usize>,
}

#[allow(dead_code)] // Actor-owned cycle state; MapFrame only resolves/dispatches the trigger graph.
fn service_native_boss_exec_cycle_runtime(
    triggers: &[ProcessedTrigger],
    exec_trigger_index: usize,
    state: &mut RobotsBossExecCycleState,
) -> Option<NativeBossExecCycleOutcome> {
    let exec_trigger = triggers.get(exec_trigger_index)?;
    if exec_trigger.ttype != ROBOTS_BOSS_EXECUTIVE_TYPE {
        return None;
    }

    let mut first_nested_cutscene_by_outer_link = [None; 8];
    for (outer_ordinal, raw_outer_link) in exec_trigger.links.iter().take(8).enumerate() {
        let Some(outer_index) = robots_trigger_link_index(*raw_outer_link, triggers.len()) else {
            continue;
        };
        let Some(outer_trigger) = triggers.get(outer_index) else {
            continue;
        };
        first_nested_cutscene_by_outer_link[outer_ordinal] =
            outer_trigger.links.iter().take(8).enumerate().find_map(
                |(nested_ordinal, raw_nested_link)| {
                    let nested_index = robots_trigger_link_index(*raw_nested_link, triggers.len())?;
                    (triggers.get(nested_index)?.ttype == ROBOTS_CUTSCENE_TYPE)
                        .then_some(nested_ordinal as u8)
                },
            );
    }

    let cycle = robots_apply_boss_exec_cycle(
        state,
        RobotsBossExecCycleInput {
            first_nested_cutscene_by_outer_link,
        },
    );
    let cutscene_trigger_index = cycle.cutscene_dispatch.and_then(|dispatch| {
        let outer_link = *exec_trigger.links.get(dispatch.outer_link_index as usize)?;
        let outer_index = robots_trigger_link_index(outer_link, triggers.len())?;
        let outer_trigger = triggers.get(outer_index)?;
        let nested_link = *outer_trigger
            .links
            .get(dispatch.nested_link_index as usize)?;
        let nested_index = robots_trigger_link_index(nested_link, triggers.len())?;
        (triggers.get(nested_index)?.ttype == ROBOTS_CUTSCENE_TYPE).then_some(nested_index)
    });

    Some(NativeBossExecCycleOutcome {
        cycle,
        cutscene_trigger_index,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeBossSewerHitOutcome {
    pub hit: RobotsBossSewerHitResult,
    /// Exact target reached by creator -> link7 for stage_event.link7_chain_index + 1 hops.
    /// Event-source identity remains host-owned because no durable native proof names it yet.
    pub stage_event_target_trigger_index: Option<usize>,
}

#[allow(dead_code)] // Actor state/AI stay outside MapFrame; this only binds host trigger storage.
fn apply_native_boss_sewer_hit_runtime(
    triggers: &[ProcessedTrigger],
    creator_trigger_index: Option<usize>,
    state: &mut RobotsBossSewerHitState,
    ai: &mut RobotsAiHitReactionState,
    hit: RobotsAcceptedHitReactionInput,
    owner_present: bool,
    debug_force_kill_non_exempt: bool,
) -> NativeBossSewerHitOutcome {
    let next_stage_value = ai.health.wrapping_sub(1);
    let stage_chain_index = robots_boss_sewer_stage_link7_chain_index(next_stage_value);
    let valid_creator_index = creator_trigger_index.filter(|&index| {
        triggers
            .get(index)
            .is_some_and(|trigger| trigger.ttype == ROBOTS_BOSS_SEWER_TYPE)
    });
    let stage_event_target_trigger_index = valid_creator_index.and_then(|creator_index| {
        let chain_index = stage_chain_index?;
        robots_follow_trigger_link7_stage_chain(
            creator_index,
            triggers.len(),
            chain_index,
            |node, ordinal| triggers.get(node)?.links.get(ordinal).copied(),
        )
    });

    let result = robots_apply_boss_sewer_hit(
        state,
        ai,
        hit,
        RobotsBossSewerHitHostInput {
            owner_present,
            creator_present: valid_creator_index.is_some(),
            stage_link7_target_present: stage_event_target_trigger_index.is_some(),
        },
        debug_force_kill_non_exempt,
    );
    let event_target = result.stage_event.and_then(|event| {
        debug_assert_eq!(Some(event.link7_chain_index), stage_chain_index);
        stage_event_target_trigger_index
    });

    NativeBossSewerHitOutcome {
        hit: result,
        stage_event_target_trigger_index: event_target,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeBossSewerCanonHitOutcome {
    pub hit: RobotsBossSewerCanonHitResult,
    /// Concrete serialized BossSewerCanon creator target. Accepted native dispatch
    /// uses common trigger source/context 0 (`0x0044C380(creator, 0x100, 0)`).
    pub creator_event_target_trigger_index: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
pub struct NativeBallTrackOrdinaryIterationBegin {
    /// None means the native circular free-slot scan found no available loaded ball,
    /// or the supplied per-call burst runtime was already complete. In that case no
    /// path resolution is required and the adapter commits any free-slot RNG draw now.
    pub request: Option<RobotsBallTrackSpawnIterationRequest>,
    pub rng_draws_consumed: u64,
    next_rng: RuntimeRobotsGlobalRngState,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NativeBallTrackOrdinaryIterationCommitOutcome {
    /// None is the native non-loop last-to-first rejection after RNG/path resolution.
    /// Some contains the child spawn plan and whether the same `0x004E3F00` call loops.
    pub spawn: Option<RobotsBallTrackSpawnIterationCommit>,
    pub rng_draws_consumed: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NativeBallTrackAlternateRowOutcome {
    pub request: RobotsBallTrackAlternateRowRequest,
    pub schedule_active: bool,
    pub path_lane: usize,
    pub path_hashcode: u32,
    /// Native `0x004E41A0` advances every lane with the controller/context from
    /// the first path object, not the current lane object.
    pub controller_path_hashcode: u32,
    pub path_advance_distance: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NativeBallTrackAlternateTickOutcome {
    pub row: Option<NativeBallTrackAlternateRowOutcome>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeBallTrackAlternateSpawnOutcome {
    pub plan: Option<RobotsBallTrackAlternateSpawnPlan>,
    pub rng_draws_consumed: u64,
}

#[allow(dead_code)] // Actor/XItem fields stay host-owned; this only validates creator storage.
fn apply_native_boss_sewer_canon_hit_runtime(
    triggers: &[ProcessedTrigger],
    creator_trigger_index: Option<usize>,
    source_is_player: bool,
    secondary_source_is_player: bool,
    armed_latch: bool,
    owner_present: bool,
) -> NativeBossSewerCanonHitOutcome {
    let valid_creator_index = creator_trigger_index.filter(|&index| {
        triggers
            .get(index)
            .is_some_and(|trigger| trigger.ttype == ROBOTS_BOSS_SEWER_CANON_TYPE)
    });
    let hit = robots_apply_boss_sewer_canon_hit(RobotsBossSewerCanonHitInput {
        source_is_player,
        secondary_source_is_player,
        armed_latch,
        owner_present,
        creator_present: valid_creator_index.is_some(),
    });
    NativeBossSewerCanonHitOutcome {
        creator_event_target_trigger_index: hit
            .dispatch_creator_event_mask
            .and(valid_creator_index),
        hit,
    }
}

#[allow(dead_code)] // Reuses the trigger graph's existing Door runtime; no second owner.
fn apply_native_door_script_command_runtime(
    trigger: &ProcessedTrigger,
    graph: &mut RuntimeTriggerGraphState,
    command: RobotsDoorCommandKind,
) -> Option<RobotsDoorScriptCommandResult> {
    if trigger.ttype != ROBOTS_DOOR_TYPE {
        return None;
    }
    if !graph.door.initialized {
        graph
            .door
            .initialize(trigger.data.first().copied().flatten().unwrap_or_default());
    }
    Some(graph.door.dispatch_script_command(command))
}

fn native_door_distance_query_plan(
    trigger: &ProcessedTrigger,
) -> Option<RobotsDoorDistanceQueryPlan> {
    (trigger.ttype == ROBOTS_DOOR_TYPE).then(|| {
        RobotsDoorDistanceQueryPlan::from_serialized(
            trigger.data.get(3).copied().flatten().unwrap_or_default(),
            trigger.data.get(4).copied().flatten().unwrap_or_default(),
        )
    })
}

fn advance_native_door_fixed_runtime(
    trigger: &ProcessedTrigger,
    graph: &mut RuntimeTriggerGraphState,
    host: RobotsDoorFixedHostInput,
) -> Option<RobotsDoorFixedStep> {
    if trigger.ttype != ROBOTS_DOOR_TYPE {
        return None;
    }
    if !graph.door.initialized {
        graph
            .door
            .initialize(trigger.data.first().copied().flatten().unwrap_or_default());
    }
    Some(graph.door.advance_fixed(
        trigger.data.get(1).copied().flatten().unwrap_or_default(),
        trigger.data.get(2).copied().flatten().unwrap_or_default(),
        trigger.data.get(5).copied().flatten().unwrap_or_default(),
        host,
    ))
}

fn apply_native_fix_switch_event_runtime(
    trigger: &ProcessedTrigger,
    graph: &mut RuntimeTriggerGraphState,
    event_mask: u32,
    owned_handler_present: bool,
) -> Option<RobotsFixSwitchEventStep> {
    if trigger.ttype != ROBOTS_FIX_SWITCH_TYPE {
        return None;
    }
    let serialized_data0 = trigger.data.first().copied().flatten().unwrap_or_default();
    if !graph.fix_switch.initialized {
        graph.fix_switch.initialize(serialized_data0);
    }
    Some(graph.dispatch_fix_switch(serialized_data0, event_mask, owned_handler_present))
}

fn advance_native_fix_switch_progress_runtime(
    trigger: &ProcessedTrigger,
    graph: &mut RuntimeTriggerGraphState,
    fixed_delta_seconds: f32,
) -> Option<RobotsFixSwitchProgressStep> {
    if trigger.ttype != ROBOTS_FIX_SWITCH_TYPE {
        return None;
    }
    if !graph.fix_switch.initialized {
        graph
            .fix_switch
            .initialize(trigger.data.first().copied().flatten().unwrap_or_default());
    }
    let duration_seconds = trigger
        .data
        .get(2)
        .copied()
        .flatten()
        .map(f32::from_bits)
        .unwrap_or_default();
    let step = graph
        .fix_switch
        .advance_progress(duration_seconds, fixed_delta_seconds);
    if step.request_deferred_output {
        graph.deferred_fire = true;
    }
    Some(step)
}

fn apply_native_ball_track_event_runtime(
    trigger: &ProcessedTrigger,
    graph: &mut RuntimeTriggerGraphState,
    event_mask: u32,
    owned_track_present: bool,
) -> Option<RobotsBallTrackEventStep> {
    if trigger.ttype != ROBOTS_BALL_TRACK_TYPE {
        return None;
    }
    let two_phase_stop = trigger
        .data
        .get(1)
        .copied()
        .flatten()
        .map(|value| value as i32 > 0)
        .unwrap_or(false);
    Some(
        graph
            .ball_track
            .dispatch_event(owned_track_present, two_phase_stop, event_mask),
    )
}

fn preview_native_ball_track_ordinary_iteration(
    burst: &RobotsBallTrackOrdinaryBurstRuntime,
    slots: &[RobotsBallTrackSpawnSlot],
    path_lane: usize,
    path_lane_count: usize,
    path_hashcode: u32,
    runtime_speed: f32,
    start_distance_min: f32,
    start_distance_max: f32,
    rng: RuntimeRobotsGlobalRngState,
) -> Option<NativeBallTrackOrdinaryIterationBegin> {
    if burst.complete {
        return Some(NativeBallTrackOrdinaryIterationBegin {
            request: None,
            rng_draws_consumed: 0,
            next_rng: rng,
        });
    }

    let mut next_rng = rng;
    let draws_before = next_rng.draws_from_anchor;
    let free_slot_draw = if slots.len() > 1 {
        Some(next_rng.next_u32()?)
    } else {
        None
    };
    let selected_slot = select_ball_track_free_slot(slots, free_slot_draw).ok()?;
    let Some(selected_slot) = selected_slot else {
        return Some(NativeBallTrackOrdinaryIterationBegin {
            request: None,
            rng_draws_consumed: next_rng.draws_from_anchor.wrapping_sub(draws_before),
            next_rng,
        });
    };

    let variant = slots.get(selected_slot)?.variant;
    let (start_unit, auxiliary_unit) = if matches!(variant, 1 | 2) {
        (None, None)
    } else {
        (
            Some(next_rng.next_unit_f32()?),
            Some(next_rng.next_unit_f32()?),
        )
    };
    let request = burst
        .prepare_iteration(
            slots,
            free_slot_draw,
            path_lane,
            path_lane_count,
            path_hashcode,
            runtime_speed,
            start_distance_min,
            start_distance_max,
            start_unit,
            auxiliary_unit,
        )
        .ok()?;
    Some(NativeBallTrackOrdinaryIterationBegin {
        request,
        rng_draws_consumed: next_rng.draws_from_anchor.wrapping_sub(draws_before),
        next_rng,
    })
}

fn commit_native_ball_track_ordinary_iteration_runtime(
    rng: &mut RuntimeRobotsGlobalRngState,
    burst: &mut RobotsBallTrackOrdinaryBurstRuntime,
    begin: NativeBallTrackOrdinaryIterationBegin,
    resolved_path_parameter: f32,
    last_node_index: f32,
    path_allows_last_to_first_segment: bool,
) -> Option<NativeBallTrackOrdinaryIterationCommitOutcome> {
    let request = begin.request?;
    let mut next_burst = *burst;
    let spawn = next_burst
        .commit_iteration(
            request,
            resolved_path_parameter,
            last_node_index,
            path_allows_last_to_first_segment,
        )
        .ok()?;
    *burst = next_burst;
    *rng = begin.next_rng;
    Some(NativeBallTrackOrdinaryIterationCommitOutcome {
        spawn,
        rng_draws_consumed: begin.rng_draws_consumed,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct NativeBallTrackAlternateConfig {
    path_lane: usize,
    path_lane_count: usize,
    path_hashcode: u32,
    controller_path_hashcode: u32,
    path_node_count: usize,
    schedule_sheet_index: u32,
    schedule_row_count: usize,
    first_path_runtime_speed: f32,
    spacing_distance: f32,
    cycle_limit: i32,
}

fn resolve_native_ball_track_alternate_config(
    map: &ProcessedMap,
    trigger: &ProcessedTrigger,
    path_lane: usize,
) -> Option<NativeBallTrackAlternateConfig> {
    if trigger.ttype != ROBOTS_BALL_TRACK_TYPE
        || trigger
            .data
            .get(1)
            .copied()
            .flatten()
            .map(|value| value as i32 > 0)
            != Some(true)
        || path_lane >= 8
    {
        return None;
    }
    let pair = map
        .ball_track_pairs
        .get(&trigger.data.get(2).copied().flatten()?)?;
    let schedule_sheet_index = trigger.data.get(3).copied().flatten()?;
    let schedule = map.ball_track_schedules.get(&schedule_sheet_index)?;
    let current_path_row = pair.paths.get(path_lane)?;
    let controller_path_row = pair.paths.first()?;
    let current_path = map
        .paths
        .iter()
        .find(|path| path.hashcode == current_path_row.path_hashcode)?;
    // Native `0x004E41A0` uses lane-zero's controller for every lane's
    // `0x00420B60` distance-to-parameter advance, so that resource must exist too.
    map.paths
        .iter()
        .find(|path| path.hashcode == controller_path_row.path_hashcode)?;

    Some(NativeBallTrackAlternateConfig {
        path_lane,
        path_lane_count: pair.paths.len(),
        path_hashcode: current_path_row.path_hashcode,
        controller_path_hashcode: controller_path_row.path_hashcode,
        path_node_count: current_path.nodes.len(),
        schedule_sheet_index,
        schedule_row_count: schedule.rows.len(),
        first_path_runtime_speed: controller_path_row.runtime_speed,
        spacing_distance: f32::from_bits(trigger.data.get(4).copied().flatten()?),
        cycle_limit: trigger.data.get(5).copied().flatten()? as i32,
    })
}

fn native_ball_track_alternate_row_outcome(
    map: &ProcessedMap,
    config: NativeBallTrackAlternateConfig,
    request: RobotsBallTrackAlternateRowRequest,
) -> Option<NativeBallTrackAlternateRowOutcome> {
    let schedule = map.ball_track_schedules.get(&config.schedule_sheet_index)?;
    let row = schedule.rows.get(request.schedule_row_index)?;
    Some(NativeBallTrackAlternateRowOutcome {
        request,
        schedule_active: row[config.path_lane] != 0,
        path_lane: config.path_lane,
        path_hashcode: config.path_hashcode,
        controller_path_hashcode: config.controller_path_hashcode,
        path_advance_distance: -config.spacing_distance,
    })
}

fn preview_native_ball_track_alternate_spawn(
    slots: &[RobotsBallTrackSpawnSlot],
    row: NativeBallTrackAlternateRowOutcome,
    path_lane_count: usize,
    first_path_runtime_speed: f32,
    rng: RuntimeRobotsGlobalRngState,
) -> Option<(
    NativeBallTrackAlternateSpawnOutcome,
    RuntimeRobotsGlobalRngState,
)> {
    if !row.schedule_active {
        return Some((
            NativeBallTrackAlternateSpawnOutcome {
                plan: None,
                rng_draws_consumed: 0,
            },
            rng,
        ));
    }
    let mut next_rng = rng;
    let draws_before = next_rng.draws_from_anchor;
    let free_slot_draw = if slots.len() > 1 {
        Some(next_rng.next_u32()?)
    } else {
        None
    };
    let selected_slot = select_ball_track_free_slot(slots, free_slot_draw).ok()?;
    let Some(selected_slot) = selected_slot else {
        return Some((
            NativeBallTrackAlternateSpawnOutcome {
                plan: None,
                rng_draws_consumed: next_rng.draws_from_anchor.wrapping_sub(draws_before),
            },
            next_rng,
        ));
    };
    let plan = plan_ball_track_alternate_spawn_for_slot(
        slots,
        selected_slot,
        row.path_lane,
        path_lane_count,
        row.path_hashcode,
        first_path_runtime_speed,
        row.request.path_parameter,
    )
    .ok()?;
    Some((
        NativeBallTrackAlternateSpawnOutcome {
            plan: Some(plan),
            rng_draws_consumed: next_rng.draws_from_anchor.wrapping_sub(draws_before),
        },
        next_rng,
    ))
}

impl MapFrame {
    /// Native GameWnd `0x004D1570` boundary. The shared owner stores exact `+0x50F`
    /// and creates a one-shot Player-service edge only for `1 -> !=1`.
    pub fn set_native_game_control_mode(
        &mut self,
        requested_mode: u8,
    ) -> Option<RobotsGameControlModeTransition> {
        self.native_game_control_runtime.set_mode(requested_mode)
    }

    /// Exact Player helper `0x004AFF80` owner-service boundary. Native invokes it
    /// after Player persistent restore during Player trigger creation and from a
    /// few later Player paths. Keeping this explicit lets UE call it after any
    /// future WatchBot-upgrade ingress without inventing per-frame polling.
    pub fn service_native_watchbot_owner(&mut self) -> bool {
        let upgrade_enabled = self
            .native_player_item_state
            .is_enabled(ROBOTS_PLAYER_ITEM_WATCHBOT_UPGRADE);
        let created = self
            .native_watchbot_owner_runtime
            .service_player_owner(upgrade_enabled, self.native_game_control_runtime.mode_50f);
        if created {
            self.native_watchbot_owner_pose = None;
        }
        created
    }

    /// Common WatchBot Handler target-binding vslot +0xD0 (`0x00490850`). The
    /// target is intentionally a generic runtime key rather than a Player-only lane;
    /// native scans slots +0x490..+0x49C and writes +0x28 only on populated components.
    pub fn bind_native_watchbot_component_target(&mut self, target_key: u64) -> bool {
        self.native_watchbot_owner_runtime
            .bind_component_target(target_key)
    }

    /// Exact Handler scheduler boundary for `0x00490590`. The host supplies the
    /// current GameWnd transition phase/scalar; shared decides whether the old
    /// component exits, a fade/phase handshake is requested, or the pending mode
    /// can commit. Component setup RNG is preflighted transactionally.
    pub fn service_native_watchbot_component_mode_transition(
        &mut self,
        gate: RobotsWatchbotComponentTransitionGate,
    ) -> Option<NativeWatchbotComponentTransitionOutcome> {
        let outcome = service_native_watchbot_component_mode_transition_runtime(
            &mut self.native_game_control_runtime,
            &mut self.native_watchbot_owner_runtime,
            &mut self.native_watchbot_mode1_runtime,
            &mut self.native_watchbot_mode2_runtime,
            &mut self.native_watchbot_mode3_runtime,
            &mut self.native_global_gameplay_rng,
            gate,
        )?;
        if let Some(placement) = outcome.mode2_entry_placement {
            let _ = self.store_native_watchbot_owner_pose(
                [
                    placement.owner_position_xyzw[0],
                    placement.owner_position_xyzw[1],
                    placement.owner_position_xyzw[2],
                ],
                None,
            );
        }
        Some(outcome)
    }

    /// UE-facing WatchBot hit boundary for Handler `0x00490C10` and the active
    /// component +0x2C callback. Mode1 component state is still host-owned; mode2/3
    /// state lives in their existing shared runtimes and is committed here.
    pub fn apply_native_watchbot_hit(
        &mut self,
        mode1_current_component_state: u32,
        current_component_present: bool,
    ) -> NativeWatchbotHitOutcome {
        apply_native_watchbot_hit_runtime(
            &self.native_watchbot_owner_runtime,
            &mut self.native_watchbot_mode2_runtime,
            &mut self.native_watchbot_mode3_runtime,
            mode1_current_component_state,
            current_component_present,
        )
    }

    /// BossExec helper `0x00484550` host boundary. The actor/UE owner keeps its
    /// monotonic +0xF8 cycle state; MapFrame resolves the native 8x8 trigger scan
    /// and dispatches the selected Cutscene through the existing event path.
    pub fn service_native_boss_exec_cycle(
        &mut self,
        map: &ProcessedMap,
        exec_trigger_index: usize,
        state: &mut RobotsBossExecCycleState,
        wall_time: f64,
    ) -> Option<RobotsBossExecCycleResult> {
        let outcome =
            service_native_boss_exec_cycle_runtime(&map.triggers, exec_trigger_index, state)?;
        if let (Some(dispatch), Some(target_index)) = (
            outcome.cycle.cutscene_dispatch,
            outcome.cutscene_trigger_index,
        ) {
            debug_assert!(dispatch.pass_exec_as_event_source);
            self.dispatch_runtime_event_from(
                map,
                target_index,
                dispatch.event_mask,
                wall_time,
                Some(exec_trigger_index),
            );
        }
        Some(outcome.cycle)
    }

    /// BossSewer `0x004CBE10` UE-facing hit boundary. Shared owns the native stage
    /// reducer/resource UID; this host seam only resolves creator->link7 storage.
    /// The returned event target is explicit because its native sender/context is
    /// not yet durably named and must not be invented by the editor host.
    pub fn apply_native_boss_sewer_hit(
        &mut self,
        map: &ProcessedMap,
        creator_trigger_index: Option<usize>,
        state: &mut RobotsBossSewerHitState,
        ai: &mut RobotsAiHitReactionState,
        hit: RobotsAcceptedHitReactionInput,
        owner_present: bool,
        debug_force_kill_non_exempt: bool,
    ) -> NativeBossSewerHitOutcome {
        apply_native_boss_sewer_hit_runtime(
            &map.triggers,
            creator_trigger_index,
            state,
            ai,
            hit,
            owner_present,
            debug_force_kill_non_exempt,
        )
    }

    /// BossSewerCanon `0x004CCC20` UE-facing hit boundary. Native dispatches the
    /// accepted creator event through `0x0044C380(creator, 0x100, 0)`, so the event
    /// source/context is explicitly null rather than an unresolved host choice.
    pub fn apply_native_boss_sewer_canon_hit(
        &mut self,
        map: &ProcessedMap,
        creator_trigger_index: Option<usize>,
        source_is_player: bool,
        secondary_source_is_player: bool,
        armed_latch: bool,
        owner_present: bool,
        wall_time: f64,
    ) -> NativeBossSewerCanonHitOutcome {
        let outcome = apply_native_boss_sewer_canon_hit_runtime(
            &map.triggers,
            creator_trigger_index,
            source_is_player,
            secondary_source_is_player,
            armed_latch,
            owner_present,
        );
        if let (Some(event_mask), Some(target_index)) = (
            outcome.hit.dispatch_creator_event_mask,
            outcome.creator_event_target_trigger_index,
        ) {
            self.dispatch_runtime_event_from(map, target_index, event_mask, wall_time, None);
        }
        outcome
    }

    /// Door `+0x5C = 0x0040CE20` Script-command boundary. The same trigger-graph
    /// state that receives Door trigger events owns E4/E5/ED; linked-record sync is
    /// returned as an explicit host action instead of inventing an engine pointer.
    pub fn apply_native_door_script_command(
        &mut self,
        map: &ProcessedMap,
        trigger_index: usize,
        command: RobotsDoorCommandKind,
    ) -> Option<RobotsDoorScriptCommandResult> {
        let trigger = map.triggers.get(trigger_index)?;
        let graph = self
            .native_trigger_graph
            .entry(Self::runtime_event_key(map.hashcode, trigger_index))
            .or_default();
        apply_native_door_script_command_runtime(trigger, graph, command)
    }

    /// Native Door helper `0x00489600` query description. UE resolves the selected
    /// Player/runtime-category distances; shared retains the exact selector semantics.
    pub fn native_door_distance_query_plan(
        &self,
        map: &ProcessedMap,
        trigger_index: usize,
    ) -> Option<RobotsDoorDistanceQueryPlan> {
        native_door_distance_query_plan(map.triggers.get(trigger_index)?)
    }

    /// XTrigger_Door fixed gameplay tick `0x004892D0`. The caller supplies world
    /// distances resolved from `native_door_distance_query_plan`; E4/E5/EC/ED remain
    /// in the same trigger-graph Door runtime used by trigger and Script commands.
    pub fn advance_native_door_fixed(
        &mut self,
        map: &ProcessedMap,
        trigger_index: usize,
        host: RobotsDoorFixedHostInput,
    ) -> Option<RobotsDoorFixedStep> {
        let trigger = map.triggers.get(trigger_index)?;
        let graph = self
            .native_trigger_graph
            .entry(Self::runtime_event_key(map.hashcode, trigger_index))
            .or_default();
        advance_native_door_fixed_runtime(trigger, graph, host)
    }

    /// XTrigger_FixSwitch local event boundary. The trigger E4/E8/EC owner stays
    /// in the shared reducer; a live UE FixSwitch component can request the exact
    /// Handler +0x3C4 dirty edge without storing native pointers in shared state.
    pub fn apply_native_fix_switch_event(
        &mut self,
        map: &ProcessedMap,
        trigger_index: usize,
        event_mask: u32,
        owned_handler_present: bool,
    ) -> Option<RobotsFixSwitchEventStep> {
        let trigger = map.triggers.get(trigger_index)?;
        let graph = self
            .native_trigger_graph
            .entry(Self::runtime_event_key(map.hashcode, trigger_index))
            .or_default();
        apply_native_fix_switch_event_runtime(trigger, graph, event_mask, owned_handler_present)
    }

    /// FixSwitch Handler-owned progress clock (`0x00412A10 -> 0x0048A930`).
    /// This must be called by the live FixSwitch Actor/component update, not by
    /// TriggerManager. Completion reuses the existing deferred eight-link output.
    pub fn advance_native_fix_switch_progress(
        &mut self,
        map: &ProcessedMap,
        trigger_index: usize,
        fixed_delta_seconds: f32,
    ) -> Option<RobotsFixSwitchProgressStep> {
        let trigger = map.triggers.get(trigger_index)?;
        let graph = self
            .native_trigger_graph
            .entry(Self::runtime_event_key(map.hashcode, trigger_index))
            .or_default();
        advance_native_fix_switch_progress_runtime(trigger, graph, fixed_delta_seconds)
    }

    /// Read-only live Pickup XItem payload for UE/host integration. TriggerManager
    /// owns lifetime; the payload exists only while that exact trigger owns an XItem.
    pub fn native_pickup_spawn_plan(
        &self,
        map: &ProcessedMap,
        trigger_index: usize,
    ) -> Option<RobotsPickupSpawnPlan> {
        let trigger = map.triggers.get(trigger_index)?;
        if !is_robots_pickup_serialized_type(trigger.ttype) {
            return None;
        }
        let key = Self::runtime_event_key(map.hashcode, trigger_index);
        if !self
            .native_pickup_trigger_lifecycle
            .get(&key)
            .is_some_and(|state| state.xitem_exists)
        {
            return None;
        }
        self.native_pickup_xitems
            .get(&key)
            .map(|runtime| runtime.spawn)
    }

    /// Read-only Handler-owned Pickup state for the UE/host boundary. This is
    /// deliberately separate from the serialized Trigger +0xE8 suppression state.
    pub fn native_pickup_handler_state(
        &self,
        map: &ProcessedMap,
        trigger_index: usize,
    ) -> Option<RobotsPickupHandlerRuntimeState> {
        let trigger = map.triggers.get(trigger_index)?;
        if !is_robots_pickup_serialized_type(trigger.ttype) {
            return None;
        }
        let key = Self::runtime_event_key(map.hashcode, trigger_index);
        self.native_pickup_xitems
            .get(&key)
            .map(|runtime| runtime.handler)
    }

    /// XTrigger_BallTrack +0x6C lifecycle boundary. The live Track XItem/Actor is
    /// host-owned; shared returns exact create, first-stop and cleanup actions.
    pub fn apply_native_ball_track_event(
        &mut self,
        map: &ProcessedMap,
        trigger_index: usize,
        event_mask: u32,
        owned_track_present: bool,
    ) -> Option<RobotsBallTrackEventStep> {
        let trigger = map.triggers.get(trigger_index)?;
        let graph = self
            .native_trigger_graph
            .entry(Self::runtime_event_key(map.hashcode, trigger_index))
            .or_default();
        apply_native_ball_track_event_runtime(trigger, graph, event_mask, owned_track_present)
    }

    /// Begin one iteration of ordinary BallTrack `0x004E3F00`. The live Track Actor
    /// owns the per-call burst state and loaded child slots. RNG is previewed on a copy;
    /// a found-slot iteration is not committed until `finish_native_ball_track_ordinary_iteration`
    /// resolves the path parameter. A no-free-slot result needs no path query, so native
    /// free-slot RNG consumption is committed immediately and the burst is marked complete.
    pub fn begin_native_ball_track_ordinary_iteration(
        &mut self,
        map: &ProcessedMap,
        trigger_index: usize,
        path_lane: usize,
        burst: &mut RobotsBallTrackOrdinaryBurstRuntime,
        slots: &[RobotsBallTrackSpawnSlot],
    ) -> Option<NativeBallTrackOrdinaryIterationBegin> {
        let trigger = map.triggers.get(trigger_index)?;
        if trigger.ttype != ROBOTS_BALL_TRACK_TYPE
            || trigger
                .data
                .get(1)
                .copied()
                .flatten()
                .map(|value| value as i32 > 0)
                .unwrap_or(false)
        {
            return None;
        }
        let sheet_index = trigger.data.get(2).copied().flatten()?;
        let pair = map.ball_track_pairs.get(&sheet_index)?;
        let path = pair.paths.get(path_lane)?;
        let outcome = preview_native_ball_track_ordinary_iteration(
            burst,
            slots,
            path_lane,
            pair.paths.len(),
            path.path_hashcode,
            path.runtime_speed,
            path.start_distance_min,
            path.start_distance_max,
            self.native_global_gameplay_rng,
        )?;
        if outcome.request.is_none() {
            self.native_global_gameplay_rng = outcome.next_rng;
            burst.complete = true;
        }
        Some(outcome)
    }

    /// Finish one found-slot ordinary BallTrack iteration after the host/path component
    /// resolves the exact native path parameter. Successful native resolution commits
    /// the previewed global RNG and the Actor-owned burst state atomically. A non-loop
    /// last-to-first rejection still commits RNG because native already consumed it.
    pub fn finish_native_ball_track_ordinary_iteration(
        &mut self,
        burst: &mut RobotsBallTrackOrdinaryBurstRuntime,
        begin: NativeBallTrackOrdinaryIterationBegin,
        resolved_path_parameter: f32,
        last_node_index: f32,
        path_allows_last_to_first_segment: bool,
    ) -> Option<NativeBallTrackOrdinaryIterationCommitOutcome> {
        commit_native_ball_track_ordinary_iteration_runtime(
            &mut self.native_global_gameplay_rng,
            burst,
            begin,
            resolved_path_parameter,
            last_node_index,
            path_allows_last_to_first_segment,
        )
    }

    /// Track Handler vslot +0x34 (`0x004E39F0`) alternate-lane entry. The lane
    /// runtime is owned by the live Track Actor/component, matching native `+0x374`;
    /// Maps supplies immutable spreadsheet/path context only.
    pub fn begin_native_ball_track_alternate_lane_tick(
        &self,
        map: &ProcessedMap,
        trigger_index: usize,
        path_lane: usize,
        lane_runtime: &mut RobotsBallTrackAlternateLaneRuntime,
        spawn_stopped: bool,
    ) -> Option<NativeBallTrackAlternateTickOutcome> {
        let trigger = map.triggers.get(trigger_index)?;
        let config = resolve_native_ball_track_alternate_config(map, trigger, path_lane)?;
        let request = lane_runtime
            .begin_tick(
                spawn_stopped,
                config.path_node_count,
                config.schedule_row_count,
                config.cycle_limit,
                config.first_path_runtime_speed * ROBOTS_BALL_TRACK_FIXED_STEP_SECONDS,
                config.spacing_distance,
            )
            .ok()?;
        let row = match request {
            Some(request) => Some(native_ball_track_alternate_row_outcome(
                map, config, request,
            )?),
            None => None,
        };
        Some(NativeBallTrackAlternateTickOutcome { row })
    }

    /// Complete one alternate schedule-row after the host/path component evaluates
    /// native `0x00420B60(controller_lane0, current_parameter, -spacing_distance)`.
    /// The returned row, when present, belongs to the same native call and therefore
    /// does not add another fixed distance delta or recheck the cycle limit.
    pub fn finish_native_ball_track_alternate_lane_row(
        &self,
        map: &ProcessedMap,
        trigger_index: usize,
        path_lane: usize,
        lane_runtime: &mut RobotsBallTrackAlternateLaneRuntime,
        next_path_parameter: f32,
    ) -> Option<NativeBallTrackAlternateTickOutcome> {
        let trigger = map.triggers.get(trigger_index)?;
        let config = resolve_native_ball_track_alternate_config(map, trigger, path_lane)?;
        lane_runtime
            .finish_row(next_path_parameter, config.schedule_row_count)
            .ok()?;
        let request = lane_runtime
            .prepare_followup_row(config.spacing_distance)
            .ok()?;
        let row = match request {
            Some(request) => Some(native_ball_track_alternate_row_outcome(
                map, config, request,
            )?),
            None => None,
        };
        Some(NativeBallTrackAlternateTickOutcome { row })
    }

    /// Spawn side of an alternate schedule row. Inactive schedule cells consume no
    /// RNG. Active cells call the same native free-slot search `0x004E4360`, so an
    /// anchored global RNG advances by one draw when more than one loaded slot exists.
    pub fn plan_native_ball_track_alternate_row_spawn(
        &mut self,
        map: &ProcessedMap,
        trigger_index: usize,
        row: NativeBallTrackAlternateRowOutcome,
        slots: &[RobotsBallTrackSpawnSlot],
    ) -> Option<NativeBallTrackAlternateSpawnOutcome> {
        let trigger = map.triggers.get(trigger_index)?;
        let config = resolve_native_ball_track_alternate_config(map, trigger, row.path_lane)?;
        if row.path_hashcode != config.path_hashcode
            || row.controller_path_hashcode != config.controller_path_hashcode
        {
            return None;
        }
        let (outcome, next_rng) = preview_native_ball_track_alternate_spawn(
            slots,
            row,
            config.path_lane_count,
            config.first_path_runtime_speed,
            self.native_global_gameplay_rng,
        )?;
        self.native_global_gameplay_rng = next_rng;
        Some(outcome)
    }

    /// Phase 1 of WatchBot component-mode1 locomotion (`0x00492C90`). Target
    /// transforms and target Handler +0x6BC payloads are supplied by the host; this
    /// computes the exact native orbit/temporary point that UE should trace toward.
    pub fn prepare_native_watchbot_mode1_target(
        &self,
        input: RobotsWatchbotMode1PrepareInput,
    ) -> Option<RobotsWatchbotMode1PreparedTarget> {
        if self
            .native_watchbot_owner_runtime
            .current_component_mode_4a0
            != ROBOTS_WATCHBOT_TARGET_FOLLOW_COMPONENT_MODE
            || self
                .native_watchbot_owner_runtime
                .component_target_key_28
                .is_none()
        {
            return None;
        }
        Some(self.native_watchbot_mode1_runtime.prepare_target(input))
    }

    /// Phase 2 of mode1 locomotion after the host visibility/collision query. A
    /// blocked result consumes exactly one draw from the existing process-global RNG;
    /// an unknown seed fails closed before the prepared timer/orbit state is committed.
    pub fn resolve_native_watchbot_mode1_visibility(
        &mut self,
        prepared: RobotsWatchbotMode1PreparedTarget,
        path_clear: bool,
        owner_flag_161_bit2: bool,
        fallback_position_xyzw: [f32; 4],
        far_transition_suppressed: bool,
    ) -> Option<RobotsWatchbotMode1Step> {
        if self
            .native_watchbot_owner_runtime
            .current_component_mode_4a0
            != ROBOTS_WATCHBOT_TARGET_FOLLOW_COMPONENT_MODE
        {
            return None;
        }
        let blocked_path_rng_draw = if path_clear {
            None
        } else {
            Some(self.native_global_gameplay_rng.next_u32()?)
        };
        self.native_watchbot_mode1_runtime.resolve_visibility(
            prepared,
            RobotsWatchbotMode1VisibilityInput {
                path_clear,
                owner_flag_161_bit2,
                fallback_position_xyzw,
                far_transition_suppressed,
                blocked_path_rng_draw,
            },
        )
    }

    /// Phase 3 of mode1 locomotion: exact common wrapper `0x00494390` / helper
    /// `0x004943B0` after target selection and visibility resolution. Shared owns
    /// component velocity math; host applies the returned owner rotation and body
    /// accumulator to the real WatchBot Actor/physics body.
    pub fn advance_native_watchbot_mode1_steering(
        &mut self,
        input: RobotsWatchbotMode1SteeringInput,
    ) -> Option<RobotsWatchbotComponentSteeringStep> {
        if self
            .native_watchbot_owner_runtime
            .current_component_mode_4a0
            != ROBOTS_WATCHBOT_TARGET_FOLLOW_COMPONENT_MODE
            || self
                .native_watchbot_owner_runtime
                .component_target_key_28
                .is_none()
        {
            return None;
        }
        let owner_position_xyz = [
            input.owner_position_xyzw[0],
            input.owner_position_xyzw[1],
            input.owner_position_xyzw[2],
        ];
        let step = advance_watchbot_mode1_steering(&mut self.native_watchbot_mode1_runtime, input);
        let _ = self
            .store_native_watchbot_owner_pose(owner_position_xyz, Some(step.owner_rotation_xyzw));
        Some(step)
    }

    /// Native WatchBot component-mode2 input boundary (`0x00496460`). Input
    /// acquisition stays host-owned; this applies exact axis/action semantics and
    /// mirrors the synchronous mode1 request through the existing Handler owner.
    pub fn apply_native_watchbot_mode2_input(
        &mut self,
        input: RobotsWatchbotMode2InputSample,
    ) -> Option<RobotsWatchbotMode2InputStep> {
        if self
            .native_watchbot_owner_runtime
            .current_component_mode_4a0
            != ROBOTS_WATCHBOT_CONTROL_COMPONENT_MODE
        {
            return None;
        }
        let step = self.native_watchbot_mode2_runtime.apply_input(input);
        if let Some(mode) = step.requested_component_mode {
            self.native_watchbot_owner_runtime
                .request_component_mode(mode);
        }
        Some(step)
    }

    /// Transactional state-entry boundary for recovered mode2 states 1/2/3/7/8/12.
    /// RNG comes only from the existing process-global gameplay stream; an unknown
    /// editor-session seed fails closed before the state/timer is mutated.
    pub fn enter_native_watchbot_mode2_internal_state(
        &mut self,
        state: u32,
        current_anim_mode_uid: u32,
    ) -> Option<RobotsWatchbotMode2StateEntryPlan> {
        if self
            .native_watchbot_owner_runtime
            .current_component_mode_4a0
            != ROBOTS_WATCHBOT_CONTROL_COMPONENT_MODE
        {
            return None;
        }
        let required =
            RobotsWatchbotMode2Runtime::state_entry_rng_draw_count(state, current_anim_mode_uid)?;
        if required > 0 && self.native_global_gameplay_rng.seed().is_none() {
            return None;
        }
        let mut draws = [0u32; 2];
        for draw in &mut draws[..required] {
            *draw = self.native_global_gameplay_rng.next_u32()?;
        }
        self.native_watchbot_mode2_runtime.enter_internal_state(
            state,
            current_anim_mode_uid,
            &draws[..required],
        )
    }

    /// Native mode2 free-flight fixed step (`0x00495ED0` + `0x00493800`). The
    /// returned world motion is added by the real host physics/body accumulator.
    pub fn advance_native_watchbot_mode2_pre_physics(
        &mut self,
        input: RobotsWatchbotMode2FixedInput,
    ) -> Option<RobotsWatchbotMode2MotionStep> {
        if self
            .native_watchbot_owner_runtime
            .current_component_mode_4a0
            != ROBOTS_WATCHBOT_CONTROL_COMPONENT_MODE
        {
            return None;
        }
        let step = self
            .native_watchbot_mode2_runtime
            .fixed_pre_physics(input)?;
        let _ = self.update_native_watchbot_owner_rotation(step.owner_rotation_euler4);
        Some(step)
    }

    /// Host commit after mode1/mode2 physics has applied the shared accumulator to
    /// the real WatchBot XItem. No synthetic integration happens inside MapFrame.
    pub fn commit_native_watchbot_owner_post_physics_pose(
        &mut self,
        post_physics_position: Vec3,
        owner_rotation_xyzw: [f32; 4],
    ) -> bool {
        if !matches!(
            self.native_watchbot_owner_runtime
                .current_component_mode_4a0,
            ROBOTS_WATCHBOT_TARGET_FOLLOW_COMPONENT_MODE | ROBOTS_WATCHBOT_CONTROL_COMPONENT_MODE
        ) {
            return false;
        }
        self.store_native_watchbot_owner_pose(
            post_physics_position.to_array(),
            Some(owner_rotation_xyzw),
        )
    }

    /// Shared component attachment update (`0x00495580`) for the proven mode2
    /// +0x78 producer. The host owns the actual child transform and audio objects;
    /// this returns the exact quaternion/dirty/SFX-command mutation plan only.
    pub fn plan_native_watchbot_mode2_attachment_spin(
        &self,
        current_child_quaternion_xyzw: [f32; 4],
    ) -> Option<RobotsWatchbotComponentAttachmentSpinStep> {
        if self
            .native_watchbot_owner_runtime
            .current_component_mode_4a0
            != ROBOTS_WATCHBOT_CONTROL_COMPONENT_MODE
        {
            return None;
        }
        Some(watchbot_component_attachment_spin_step(
            self.native_watchbot_mode2_runtime.attachment_spin_78,
            current_child_quaternion_xyzw,
        ))
    }

    /// Bind the currently selected WatchBot mode3 EXGeoPath to its dedicated
    /// component runtime. The live XItem transform/velocity stays host-owned.
    pub fn bind_native_watchbot_mode3_path(
        &mut self,
        map: &ProcessedMap,
        controlled_watchbot_position: Vec3,
        current_velocity_xyzw: [f32; 4],
        runtime_rate_scale: f32,
    ) -> bool {
        if self
            .native_watchbot_owner_runtime
            .current_component_mode_4a0
            != ROBOTS_WATCHBOT_TRIGGER_PATH_MODE
        {
            self.native_watchbot_mode3_runtime = RobotsWatchbotMode3Runtime::default();
            return false;
        }
        let Some(path_uid) = self.native_watchbot_owner_runtime.current_path_uid_488 else {
            self.native_watchbot_mode3_runtime.clear_path();
            return false;
        };
        let Some(path) = map.paths.iter().find(|path| path.hashcode == path_uid) else {
            self.native_watchbot_mode3_runtime.clear_path();
            return false;
        };
        let points = path
            .nodes
            .iter()
            .map(|node| node.position.to_array())
            .collect::<Vec<_>>();
        let bound = self.native_watchbot_mode3_runtime.bind_path(
            &points,
            controlled_watchbot_position.to_array(),
            current_velocity_xyzw,
            runtime_rate_scale,
        );
        if bound {
            let _ = self.update_native_watchbot_owner_position(controlled_watchbot_position);
        }
        bound
    }

    /// Native mode3 input projection (`0x004970E0`) after the host has resolved
    /// its actual controller/input source.
    pub fn set_native_watchbot_mode3_control_input(
        &mut self,
        input: RobotsWatchbotMode3ControlInput,
    ) -> bool {
        if self
            .native_watchbot_owner_runtime
            .current_component_mode_4a0
            != ROBOTS_WATCHBOT_TRIGGER_PATH_MODE
            || !self.native_watchbot_mode3_runtime.path_bound()
        {
            return false;
        }
        self.native_watchbot_mode3_runtime.set_control_input(input);
        true
    }

    /// Explicit state ingress for native mode3 internal states. Collision/animation
    /// owners decide when state8 is entered; Maps does not manufacture that event.
    pub fn set_native_watchbot_mode3_internal_state(&mut self, state: u32) -> bool {
        if self
            .native_watchbot_owner_runtime
            .current_component_mode_4a0
            != ROBOTS_WATCHBOT_TRIGGER_PATH_MODE
            || !self.native_watchbot_mode3_runtime.path_bound()
        {
            return false;
        }
        self.native_watchbot_mode3_runtime.set_internal_state(state);
        true
    }

    /// First half of native mode3 vslot +0x7C. Apply the returned rotation and
    /// velocity through host physics before calling `finish_native_watchbot_mode3_physics()`.
    pub fn advance_native_watchbot_mode3_pre_physics(
        &mut self,
        input: RobotsWatchbotMode3FixedInput,
    ) -> Option<RobotsWatchbotMode3PrePhysicsStep> {
        if self
            .native_watchbot_owner_runtime
            .current_component_mode_4a0
            != ROBOTS_WATCHBOT_TRIGGER_PATH_MODE
        {
            return None;
        }
        let step = self
            .native_watchbot_mode3_runtime
            .fixed_pre_physics(input)?;
        let _ = self.update_native_watchbot_owner_rotation(step.owner_rotation_euler4);
        Some(step)
    }

    /// Native mode3 vslot +0x7C tail after motion/collision has produced the real
    /// post-physics WatchBot position. This is where native recomputes +0xF8.
    pub fn finish_native_watchbot_mode3_physics(
        &mut self,
        post_physics_watchbot_position: Vec3,
        controller_yaw_radians: f32,
    ) -> Option<RobotsWatchbotMode3PostPhysicsStep> {
        if self
            .native_watchbot_owner_runtime
            .current_component_mode_4a0
            != ROBOTS_WATCHBOT_TRIGGER_PATH_MODE
        {
            return None;
        }
        let step = self.native_watchbot_mode3_runtime.finish_after_physics(
            post_physics_watchbot_position.to_array(),
            controller_yaw_radians,
        )?;
        let _ = self.update_native_watchbot_owner_position(post_physics_watchbot_position);
        Some(step)
    }

    /// UE/host fixed-step boundary for serialized `XTrigger_Watchbot` type60.
    /// The caller supplies the live controlled WatchBot XItem position; Maps does
    /// not substitute Rodney's pose when that owner transform is unavailable.
    pub fn advance_native_watchbot_triggers_fixed(
        &mut self,
        map: &ProcessedMap,
        controlled_watchbot_position: Vec3,
    ) -> Vec<(usize, RobotsWatchbotTriggerStep)> {
        let mut steps = Vec::new();
        for (trigger_index, trigger) in map.triggers.iter().enumerate() {
            if trigger.ttype != ROBOTS_WATCHBOT_TYPE
                || self.native_trigger_disabled(map.hashcode, trigger_index, trigger)
            {
                continue;
            }
            let Some(contract) = RobotsWatchbotTriggerContract::from_serialized_data(&trigger.data)
            else {
                continue;
            };
            let key = Self::runtime_event_key(map.hashcode, trigger_index);
            let path_resolved = contract.path_uid == ROBOTS_WATCHBOT_NO_PATH_UID
                || map
                    .paths
                    .iter()
                    .any(|path| path.hashcode == contract.path_uid);
            let distance_squared = trigger
                .position
                .distance_squared(controlled_watchbot_position);
            let step = self
                .native_trigger_graph
                .entry(key)
                .or_default()
                .watchbot
                .fixed_update(
                    key,
                    contract,
                    &mut self.native_watchbot_owner_runtime,
                    distance_squared,
                    path_resolved,
                );
            steps.push((trigger_index, step));
        }
        steps
    }

    /// Accepted action-button boundary for Player `0x004BCCA0` with a focused
    /// native category 0x4C ActivationPad. Input ownership stays outside Maps;
    /// this method starts exactly after native input vslot `+0x2C(4)` succeeds.
    pub fn interact_native_watchbot_focus(
        &mut self,
        focused_position_xyzw: [f32; 4],
        focused_rotation_xyzw: [f32; 4],
        configured_exit_sound_uid_4d4: Option<u32>,
    ) -> Option<RobotsWatchbotControlEnterOutcome> {
        let focused = self.native_player_focus_runtime.current_owner?;
        if focused.category != ROBOTS_XITEM_CATEGORY_ACTIVATION_PAD {
            return None;
        }

        let can_enter = self.native_watchbot_owner_runtime.control_entry_ready()
            && self.native_game_control_runtime.mode_50f == ROBOTS_GAME_CONTROL_MODE_DEFAULT;
        let global_rng_draw = if can_enter {
            self.native_global_gameplay_rng.next_u32()
        } else {
            None
        };
        interact_native_watchbot_focus_transition(
            &mut self.native_game_control_runtime,
            &mut self.native_player_action_runtime,
            &mut self.native_player_focus_runtime,
            &mut self.native_watchbot_owner_runtime,
            RobotsWatchbotComponentTransitionSnapshotBits::from_f32(
                focused_position_xyzw,
                focused_rotation_xyzw,
            ),
            configured_exit_sound_uid_4d4,
            global_rng_draw,
        )
    }

    /// Player-service boundary for a pending WatchBot-control exit. Unlike the old
    /// raw seam this cannot fire unless GameWnd actually crossed `1 -> !=1`, and a
    /// blocked Player state keeps the edge pending for the next Player service.
    pub fn service_native_watchbot_control_exit(
        &mut self,
        exit_sound_uid_4d4: Option<u32>,
    ) -> Option<RobotsPlayerWatchbotControlExitOutcome> {
        service_native_watchbot_control_exit_transition(
            &mut self.native_game_control_runtime,
            &mut self.native_player_action_runtime,
            &mut self.native_player_focus_runtime.player_state,
            &self.native_player_item_state,
            exit_sound_uid_4d4,
        )
    }

    /// Low-level UE/host boundary for the one-shot WatchBot-control exit branch in Player
    /// `0x004B02D0`. Prefer `service_native_watchbot_control_exit()` for production
    /// flow; this raw seam remains useful for direct host replay/tests.
    pub fn finish_native_watchbot_control_exit(
        &mut self,
        control_mode_7b3207: u8,
        exit_sound_uid_4d4: Option<u32>,
    ) -> Option<RobotsPlayerWatchbotControlExitOutcome> {
        let player_state = self.native_player_focus_runtime.player_state;
        let player_items = &self.native_player_item_state;
        let outcome = self
            .native_player_action_runtime
            .finish_watchbot_control_exit(
                control_mode_7b3207,
                player_state,
                exit_sound_uid_4d4,
                |uid| player_items.stored_current(uid) as i32,
            )?;
        self.native_player_focus_runtime.player_state = outcome.next_player_state_6de;
        Some(outcome)
    }

    /// Production host seam for native `XHudShop` purchases. Shop data stays
    /// immutable in `ProcessedMap`; the two already-existing gameplay owners
    /// receive the mutations. UI/UE presentation consumes the returned typed
    /// outcome, including `ExitShop`, instead of owning purchase semantics.
    pub fn purchase_native_shop_offer(
        &mut self,
        map: &ProcessedMap,
        shop_uid: u32,
        slot: usize,
        scrap_removal_locked: bool,
    ) -> Option<RobotsShopPurchaseOutcome> {
        apply_native_shop_purchase(
            map.shop_database.as_ref(),
            &map.inventory_definitions,
            &mut self.native_script_gameplay_state,
            &mut self.native_player_item_state,
            shop_uid,
            slot,
            scrap_removal_locked,
        )
    }

    /// UE-facing convenience boundary: native XHudShop owns one active group at
    /// a time, so presentation normally buys from the lifecycle-selected group.
    pub fn purchase_active_native_shop_offer(
        &mut self,
        map: &ProcessedMap,
        slot: usize,
        scrap_removal_locked: bool,
    ) -> Option<RobotsShopPurchaseOutcome> {
        let shop_uid = self.native_shop_lifecycle.active_shop_uid?;
        self.purchase_native_shop_offer(map, shop_uid, slot, scrap_removal_locked)
    }

    /// Exact host completion boundary for `0x00482E70`, called by XHudShop while
    /// its close transition finishes. This is intentionally separate from Shop
    /// trigger event 0x200, matching native ownership/timing.
    pub fn finish_native_shop_hud_teardown(
        &mut self,
        map: &ProcessedMap,
        shop_trigger_index: usize,
        wall_time: f64,
    ) -> bool {
        let Some(shop_trigger) = map.triggers.get(shop_trigger_index) else {
            return false;
        };
        if shop_trigger.ttype != ROBOTS_SHOP_TYPE {
            return false;
        }
        let linked_npc_index =
            Self::native_first_linked_trigger_index(map, shop_trigger, ROBOTS_NPC_TYPE);
        let npc_first_cutscene_index = linked_npc_index.and_then(|npc_index| {
            map.triggers.get(npc_index).and_then(|npc| {
                Self::native_first_linked_trigger_index(map, npc, ROBOTS_CUTSCENE_TYPE)
            })
        });
        let shop_cutscene_indices = shop_trigger
            .links
            .iter()
            .take(8)
            .filter_map(|link| map_trigger_by_link(map, *link))
            .filter_map(|(index, linked)| (linked.ttype == ROBOTS_CUTSCENE_TYPE).then_some(index))
            .collect::<Vec<_>>();
        let plan = plan_shop_hud_teardown(
            linked_npc_index.is_some(),
            npc_first_cutscene_index.is_some(),
            shop_cutscene_indices.len(),
        );
        if plan.dispatch_npc_first_cutscene {
            if let Some(target_index) = npc_first_cutscene_index {
                self.dispatch_runtime_event_from(
                    map,
                    target_index,
                    ROBOTS_SHOP_EVENT_OPEN,
                    wall_time,
                    Some(shop_trigger_index),
                );
            }
        }
        for list_index in plan.shop_cutscene_dispatch_order {
            if let Some(&target_index) = shop_cutscene_indices.get(list_index) {
                self.dispatch_runtime_event_from(
                    map,
                    target_index,
                    ROBOTS_SHOP_EVENT_OPEN,
                    wall_time,
                    Some(shop_trigger_index),
                );
            }
        }
        if let Some(player_state) = plan.player_state {
            self.native_player_focus_runtime.player_state = player_state;
        }
        self.native_shop_lifecycle.close_hud();
        true
    }

    fn native_trigger_disabled(
        &self,
        map_hashcode: u32,
        trigger_index: usize,
        trigger: &ProcessedTrigger,
    ) -> bool {
        let key = Self::runtime_event_key(map_hashcode, trigger_index);
        self.native_common_trigger_events
            .get(&key)
            .filter(|state| state.initialized)
            .map(|state| state.disabled)
            .unwrap_or(trigger.game_flags & 1 != 0)
    }

    pub(super) fn runtime_event_key(map_hash: u32, trigger_index: usize) -> u64 {
        ((map_hash as u64) << 32) | trigger_index as u64
    }

    fn native_first_linked_trigger_index(
        map: &ProcessedMap,
        trigger: &ProcessedTrigger,
        target_type: u32,
    ) -> Option<usize> {
        trigger.links.iter().take(8).find_map(|link| {
            let (index, linked) = map_trigger_by_link(map, *link)?;
            (linked.ttype == target_type).then_some(index)
        })
    }

    fn finalize_native_cutscene(
        &mut self,
        map: &ProcessedMap,
        trigger_index: usize,
        wall_time: f64,
    ) -> Option<RobotsCutsceneFinalizeBranch> {
        let trigger = map.triggers.get(trigger_index)?;
        let key = Self::runtime_event_key(map.hashcode, trigger_index);
        let active_display_mask = self.native_cutscene_host_runtime.display_mask
            | if self.native_message_presentation.message_box_exists {
                0x8
            } else {
                0
            };
        self.native_cutscene_host_runtime.display_mask = active_display_mask;
        self.native_cutscene_host_runtime.global_cutscene_active = true;
        self.native_cutscene_host_runtime.mission_owner_present =
            self.native_mission_owner.is_some();
        self.native_cutscene_host_runtime.player_health = self
            .native_sweeper_player_health_known
            .then_some(RobotsCutscenePlayerHealthState {
                current: self.native_sweeper_player_current_health,
                max: self.native_sweeper_player_max_health,
            });
        let input = {
            let state = self.native_cutscene_runtime.get(&key)?;
            if !state.lifecycle.xitem_exists {
                return None;
            }
            RobotsCutsceneFinalizeInput {
                marker_time_bits_3a0: state.state_marker_runtime.marker_time_bits_3a0,
                audio_active_16c3: state.audio_active_16c3,
                music_audio_present_16c4: state.music_audio_present_16c4,
                creator_flags_e8: state.creator_runtime_flags_e8,
                player_handler_state_6de: Some(self.native_player_focus_runtime.player_state),
                saved_display_mask_16b4: state.saved_display_mask_16b4,
                active_display_mask,
                pending_swap_character_16bd: state.pending_swap_character_16bd,
                // DAT_007B3207 is only consumed after SwapCharacter mode1. The
                // shipped M10 Cutscene scripts do not use that command; preserve
                // the unresolved Player-mode boundary as None rather than guessing.
                player_mode: None,
            }
        };
        let plan = plan_cutscene_finalize(input);
        let mut dispatches = Vec::new();
        for effect in plan.effects.iter().copied() {
            self.native_cutscene_host_runtime
                .apply_finalize_effect(effect);
            match effect {
                RobotsCutsceneFinalizeEffect::DestroyDisplayMask { mask } if mask & 0x8 != 0 => {
                    if self.native_message_presentation.message_box_exists {
                        self.native_message_presentation.pending_destroy = true;
                    }
                }
                RobotsCutsceneFinalizeEffect::RestoreDisplayMask { mask } if mask & 0x8 != 0 => {
                    self.native_message_presentation.message_box_exists = true;
                    self.native_message_presentation.pending_destroy = false;
                }
                RobotsCutsceneFinalizeEffect::ReleaseCutsceneHandler {
                    restore_display_mask,
                } => {
                    if restore_display_mask & 0x8 != 0 {
                        self.native_message_presentation.message_box_exists = true;
                        self.native_message_presentation.pending_destroy = false;
                    } else if self.native_message_presentation.message_box_exists {
                        self.native_message_presentation.pending_destroy = true;
                    }
                }
                RobotsCutsceneFinalizeEffect::DispatchFirstTriggerType { target, event_mask } => {
                    let target_type = match target {
                        RobotsCutsceneFinalizeDispatchTarget::ChangeLevel => {
                            ROBOTS_CHANGE_LEVEL_TYPE
                        }
                        RobotsCutsceneFinalizeDispatchTarget::Cutscene => ROBOTS_CUTSCENE_TYPE,
                    };
                    if let Some(target_index) =
                        Self::native_first_linked_trigger_index(map, trigger, target_type)
                    {
                        dispatches.push((target_index, event_mask));
                    }
                }
                _ => {}
            }
        }
        if let Some(health) = self.native_cutscene_host_runtime.player_health {
            self.native_sweeper_player_current_health = health.current;
            self.native_sweeper_player_max_health = health.max;
        }

        if let Some(state) = self.native_cutscene_runtime.get_mut(&key) {
            state.record_finalize_effects(plan.effects.clone());
            for effect in &plan.effects {
                match *effect {
                    RobotsCutsceneFinalizeEffect::FinalizeScriptAnimatorTail {
                        marker_time_bits_3a0,
                        ..
                    } => {
                        state.active_script_time_seconds =
                            Some(f32::from_bits(marker_time_bits_3a0));
                    }
                    RobotsCutsceneFinalizeEffect::StopAndClearCutsceneAudio => {
                        state.audio_active_16c3 = false;
                    }
                    RobotsCutsceneFinalizeEffect::ResetCreatorActivationOverride => {
                        state.pending_alternate_uid = None;
                    }
                    _ => {}
                }
            }
            match plan.branch {
                RobotsCutsceneFinalizeBranch::Ordinary => state.complete_ordinary_finalize(),
                RobotsCutsceneFinalizeBranch::ChangeLevel
                | RobotsCutsceneFinalizeBranch::LinkedCutscene
                | RobotsCutsceneFinalizeBranch::SpecialPlayerState => {
                    state.release_after_finalize()
                }
            }
        }

        if plan.branch != RobotsCutsceneFinalizeBranch::Ordinary {
            // Handler cleanup reaches XItem 0x00443EE0 -> creator +0x84;
            // Cutscene +0x84 ends at common trigger +0x20 (0x0044D270), which
            // sets trigger runtime bit0. Mirror that direct disable hook without
            // fabricating a synthetic event-mask dispatch on the source Cutscene.
            self.native_common_trigger_events
                .entry(key)
                .or_default()
                .apply_disable_hook(trigger);
        }
        for (target_index, event_mask) in dispatches {
            self.dispatch_runtime_event(map, target_index, event_mask, wall_time);
        }
        Some(plan.branch)
    }

    fn native_npc_focus_geometry(
        map: &ProcessedMap,
        npc: &ProcessedTrigger,
        live_owner_position: Vec3,
    ) -> (Vec3, f32) {
        for link in npc.links.iter().take(8) {
            let Some((_, linked)) = map_trigger_by_link(map, *link) else {
                continue;
            };
            if linked.ttype != ROBOTS_DISTANCE_TYPE {
                continue;
            }
            let raw = linked.data.first().copied().flatten().unwrap_or_default() as i32;
            return (linked.position, ((raw as f32) * 0.1).abs());
        }
        (live_owner_position, ROBOTS_PLAYER_FOCUS_DEFAULT_RANGE)
    }

    fn native_slide_under_owner_view(
        &self,
        map: &ProcessedMap,
        candidate_key: u64,
    ) -> Option<RobotsSlideUnderOwnerView> {
        let key = self.native_player_focus_runtime.current_slide_under?;
        if (key >> 32) as u32 != map.hashcode {
            return None;
        }
        let trigger = map.triggers.get(key as u32 as usize)?;
        if trigger.ttype != ROBOTS_SLIDE_UNDER_TYPE {
            return None;
        }
        let outer_radius =
            (trigger.data.get(1).copied().flatten().unwrap_or_default() as i32 as f32 * 0.1).abs();
        Some(RobotsSlideUnderOwnerView {
            trigger_position: trigger.position.to_array(),
            outer_radius,
            is_candidate: key == candidate_key,
        })
    }

    fn native_player_focus_owner_view(
        &self,
        map: &ProcessedMap,
        candidate_key: u64,
    ) -> Option<RobotsPlayerFocusOwnerView> {
        let owner = self.native_player_focus_runtime.current_owner?;
        let position = match owner.category {
            ROBOTS_XITEM_CATEGORY_NPC => {
                self.runtime_character_bodies
                    .get(&owner.key)?
                    .owner_position
            }
            ROBOTS_XITEM_CATEGORY_ACTIVATION_PAD
            | ROBOTS_XITEM_CATEGORY_SLIDE_UNDER
            | ROBOTS_XITEM_CATEGORY_ALERT_ICON => {
                if (owner.key >> 32) as u32 != map.hashcode {
                    return None;
                }
                map.triggers.get(owner.key as u32 as usize)?.position
            }
            _ => return None,
        };
        Some(RobotsPlayerFocusOwnerView {
            category: owner.category,
            position: position.to_array(),
            is_player_xitem: false,
            is_candidate: owner.key == candidate_key,
        })
    }

    fn advance_native_npc_behavior_fixed(&mut self, map: &ProcessedMap, player_position: Vec3) {
        let nav_regions = monster_nav_regions(map);
        let gameplay_target_position = self.native_ai_gameplay_target_position(player_position);
        for (trigger_index, trigger) in map.triggers.iter().enumerate() {
            if trigger.ttype != ROBOTS_NPC_TYPE {
                continue;
            }
            let key = Self::runtime_event_key(map.hashcode, trigger_index);
            let xitem_exists = self
                .native_ai_trigger_lifecycle
                .get(&key)
                .is_some_and(|state| state.xitem_exists);
            if !xitem_exists {
                self.native_npc_behavior.remove(&key);
                self.native_monster_navigation.remove(&key);
                continue;
            }

            let contract = decode_npc_serialized_contract_slice(&trigger.data);
            let diner_profile = contract.flags & 0x20 != 0;
            let Some(body) = self.runtime_character_bodies.get(&key) else {
                self.native_npc_behavior.remove(&key);
                continue;
            };
            let Some(handler_flags_628) = trigger
                .character_visual
                .as_ref()
                .map(|visual| visual.handler_flags_628)
            else {
                self.native_npc_behavior.remove(&key);
                continue;
            };
            let owner_position = body.owner_position;
            let owner_rotation = body.owner_rotation;
            let random_idle_candidates: &[u32] = if diner_profile {
                &ROBOTS_NPC_DINER_PERIODIC_ANIM_MODES
            } else {
                &ROBOTS_NPC_RANDOM_IDLE_ANIM_MODES
            };
            let random_idle_modes = random_idle_candidates
                .iter()
                .copied()
                .take_while(|mode| body.animation_modes.contains_key(mode))
                .collect::<Vec<_>>();
            let distance_squared = gameplay_target_position
                .map(|target| owner_position.distance_squared(target))
                .unwrap_or(f32::INFINITY);
            let follow_path = if diner_profile {
                None
            } else {
                contract
                    .data1
                    .filter(|uid| *uid != 0x0B00_0000 && (*uid & 0xFF00_0000) == 0x0B00_0000)
                    .and_then(|uid| map.paths.iter().find(|path| path.hashcode == uid))
            };
            let follow_path_nodes = follow_path.map(|path| {
                path.nodes
                    .iter()
                    .map(|node| eurochef_shared::robots_runtime::follow_network_path::RobotsFollowNetworkPathNode {
                        position_xyz: (path.position + node.position).to_array(),
                        size_x: node.size.x,
                        value: node.value,
                    })
                    .collect::<Vec<_>>()
            });
            let flag2_present = !diner_profile && contract.flags & 0x2 != 0;
            let flag2_nav = if flag2_present {
                let nav_state = self.native_monster_navigation.entry(key).or_default();
                if nav_state.maintain(&nav_regions, owner_position.to_array()) {
                    nav_state
                        .region_ordinal
                        .zip(nav_state.face_index)
                        .zip(nav_state.group_flags0)
                        .map(|((region_ordinal, face_index), group_flags0)| {
                            (region_ordinal, face_index, group_flags0)
                        })
                } else {
                    None
                }
            } else {
                self.native_monster_navigation.remove(&key);
                None
            };

            let mut forward = owner_rotation * Vec3::Z;
            forward.y = 0.0;
            let owner_yaw = if forward.is_finite() && forward.length_squared() > f32::EPSILON {
                forward.x.atan2(forward.z)
            } else {
                0.0
            };
            let yaw_error = gameplay_target_position
                .map(|target| {
                    let to_target = target - owner_position;
                    shortest_yaw_delta(owner_yaw, to_target.x.atan2(to_target.z))
                })
                .unwrap_or_default();

            if diner_profile {
                let runtime = self.native_npc_behavior.entry(key).or_default();
                if !runtime.diner_profile_bootstrapped {
                    // 0x0046B5BB..0x0046B5E3: 0x00455E30(0), Physics+0x08 bit2,
                    // paired 0x004535D0(1)/0x00453600(1). Handler+0x604 is also
                    // cleared natively; the current NPC host has no consumer of that lane.
                    bootstrap_npc_diner_physics(&mut runtime.physics);
                    runtime.diner_profile_bootstrapped = true;
                }
            }

            // 0x004591C0 consumes one gameplay-global RNG draw when the random-idle
            // node is configured. Unknown editor-session RNG provenance fails closed.
            let random_needs_init = !random_idle_modes.is_empty()
                && self
                    .native_npc_behavior
                    .get(&key)
                    .is_none_or(|runtime| !runtime.random_idle.initialized);
            let initial_random_draw = random_needs_init
                .then(|| self.native_global_gameplay_rng.next_u32())
                .flatten();

            let (random_priority, proximity_priority, previous_node) = {
                let runtime = self.native_npc_behavior.entry(key).or_default();
                if let Some(draw) = initial_random_draw {
                    initialize_npc_random_idle(&mut runtime.random_idle, draw);
                }
                (
                    if random_idle_modes.is_empty() {
                        1
                    } else {
                        npc_random_idle_priority(&runtime.random_idle)
                    },
                    if diner_profile || contract.flags & 0x10 != 0 {
                        1
                    } else {
                        npc_proximity_priority(
                            runtime.proximity.node_active,
                            gameplay_target_position.is_some(),
                            distance_squared,
                        )
                    },
                    runtime.active_node,
                )
            };
            // `0x0046CE40` also checks the process-global game-state stack. The
            // map preview represents the ordinary gameplay state, so none of the
            // native blocked top states 1/2/3/0x0F is active here.
            let follow_path_priority = if follow_path_nodes
                .as_ref()
                .is_some_and(|nodes| !nodes.is_empty())
            {
                ROBOTS_NPC_FOLLOW_NETWORK_PATH_PRIORITY
            } else {
                1
            };
            let flag2_priority = npc_flag2_priority(flag2_nav.is_some(), false);
            let diner_idle_priority = if diner_profile {
                ROBOTS_NPC_DINER_IDLE_PRIORITY
            } else {
                1
            };

            // Common selector 0x00457140 picks the highest low-byte priority:
            // RandomIdle 0x16 > Proximity 0x15 > FollowNetworkPath 0x0F >
            // PatrolNavMesh2 0x0B > diner AI_Idle 0x02. The diner profile does not
            // construct the middle three nodes, so IdleDiner1 is its ordinary fallback.
            let mut winner = if random_priority == ROBOTS_NPC_RANDOM_IDLE_PRIORITY {
                Some(NativeNpcBehaviorNode::RandomIdle)
            } else if proximity_priority == ROBOTS_NPC_PROXIMITY_PRIORITY {
                Some(NativeNpcBehaviorNode::Proximity)
            } else if follow_path_priority == ROBOTS_NPC_FOLLOW_NETWORK_PATH_PRIORITY {
                Some(NativeNpcBehaviorNode::FollowPath)
            } else if flag2_priority == ROBOTS_NPC_FLAG2_BEHAVIOR_PRIORITY {
                Some(NativeNpcBehaviorNode::Flag2)
            } else if diner_idle_priority == ROBOTS_NPC_DINER_IDLE_PRIORITY {
                Some(NativeNpcBehaviorNode::DinerIdle)
            } else {
                None
            };

            let random_enter_draws = if winner == Some(NativeNpcBehaviorNode::RandomIdle)
                && previous_node != Some(NativeNpcBehaviorNode::RandomIdle)
            {
                self.native_global_gameplay_rng
                    .next_u32()
                    .zip(self.native_global_gameplay_rng.next_u32())
            } else {
                None
            };
            if winner == Some(NativeNpcBehaviorNode::RandomIdle)
                && previous_node != Some(NativeNpcBehaviorNode::RandomIdle)
                && random_enter_draws.is_none()
            {
                // Native always owns this process-global stream. Without a proven
                // seed, do not fabricate selection or delay draws in the editor.
                winner = if proximity_priority == ROBOTS_NPC_PROXIMITY_PRIORITY {
                    Some(NativeNpcBehaviorNode::Proximity)
                } else if follow_path_priority == ROBOTS_NPC_FOLLOW_NETWORK_PATH_PRIORITY {
                    Some(NativeNpcBehaviorNode::FollowPath)
                } else if flag2_priority == ROBOTS_NPC_FLAG2_BEHAVIOR_PRIORITY {
                    Some(NativeNpcBehaviorNode::Flag2)
                } else if diner_idle_priority == ROBOTS_NPC_DINER_IDLE_PRIORITY {
                    Some(NativeNpcBehaviorNode::DinerIdle)
                } else {
                    None
                };
            }

            {
                let runtime = self.native_npc_behavior.get_mut(&key).unwrap();
                if previous_node == Some(NativeNpcBehaviorNode::RandomIdle)
                    && winner != Some(NativeNpcBehaviorNode::RandomIdle)
                {
                    leave_npc_random_idle(&mut runtime.random_idle);
                }
                if previous_node == Some(NativeNpcBehaviorNode::Proximity)
                    && winner != Some(NativeNpcBehaviorNode::Proximity)
                {
                    runtime.proximity.node_active = false;
                }
                if previous_node == Some(NativeNpcBehaviorNode::FollowPath)
                    && winner != Some(NativeNpcBehaviorNode::FollowPath)
                {
                    runtime.follow_path.leave();
                }
                if winner == Some(NativeNpcBehaviorNode::Proximity)
                    && previous_node != Some(NativeNpcBehaviorNode::Proximity)
                {
                    runtime.proximity.node_active = true;
                }
                if winner == Some(NativeNpcBehaviorNode::FollowPath)
                    && previous_node != Some(NativeNpcBehaviorNode::FollowPath)
                {
                    let node_count = follow_path_nodes.as_ref().map_or(0, Vec::len);
                    if !runtime.follow_path.enter(node_count) {
                        winner = None;
                    }
                }
                runtime.active_node = winner;
            }

            match winner {
                Some(NativeNpcBehaviorNode::RandomIdle) => {
                    let selected_anim_mode = {
                        let runtime = self.native_npc_behavior.get_mut(&key).unwrap();
                        if previous_node != Some(NativeNpcBehaviorNode::RandomIdle) {
                            let (select_draw, delay_draw) = random_enter_draws.unwrap();
                            enter_npc_random_idle(
                                &mut runtime.random_idle,
                                &random_idle_modes,
                                select_draw,
                                delay_draw,
                            )
                        } else {
                            runtime.random_idle.selected_anim_mode
                        }
                    };
                    if let Some(anim_mode) = selected_anim_mode {
                        let events = {
                            let runtime = self.native_npc_behavior.get_mut(&key).unwrap();
                            runtime.last_action = None;
                            runtime.requested_anim_mode = Some(anim_mode);
                            runtime.direct_owner_yaw_write = false;
                            if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                runtime.animation.advance(
                                    body,
                                    anim_mode,
                                    None,
                                    ROBOTS_NATIVE_FIXED_SECONDS as f32,
                                    NativeAiRootMotionPolicy::NONE,
                                )
                            } else {
                                Vec::new()
                            }
                        };
                        if events.iter().any(|event| {
                            event.event_type
                                == eurochef_shared::robots_runtime::events::event_type::SETUP_IDLE
                        }) {
                            let runtime = self.native_npc_behavior.get_mut(&key).unwrap();
                            complete_npc_random_idle_setup_idle(&mut runtime.random_idle);
                            runtime.animation.setup_idle();
                        }
                    }
                }
                Some(NativeNpcBehaviorNode::Proximity) => {
                    let action = {
                        let runtime = self.native_npc_behavior.get_mut(&key).unwrap();
                        let action = step_npc_proximity_face_player(
                            &mut runtime.proximity,
                            contract.flags & 0x8 == 0,
                            yaw_error,
                        );
                        runtime.last_action = Some(action);
                        action
                    };
                    match action {
                        RobotsNpcProximityAction::RequestAnimMode { anim_mode } => {
                            let events = {
                                let runtime = self.native_npc_behavior.get_mut(&key).unwrap();
                                runtime.requested_anim_mode = Some(anim_mode);
                                runtime.direct_owner_yaw_write = false;
                                if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                    runtime.animation.advance(
                                        body,
                                        anim_mode,
                                        None,
                                        ROBOTS_NATIVE_FIXED_SECONDS as f32,
                                        NativeAiRootMotionPolicy::NONE,
                                    )
                                } else {
                                    Vec::new()
                                }
                            };
                            if events.iter().any(|event| {
                                event.event_type
                                    == eurochef_shared::robots_runtime::events::event_type::SETUP_IDLE
                            }) {
                                self.native_npc_behavior
                                    .get_mut(&key)
                                    .unwrap()
                                    .animation
                                    .setup_idle();
                            }
                        }
                        RobotsNpcProximityAction::TurnTowardPlayer {
                            yaw_error_radians,
                            max_turn_radians_per_second,
                            anim_mode,
                        } => {
                            let turn = step_ai_direct_turn_request(
                                owner_yaw,
                                yaw_error_radians,
                                max_turn_radians_per_second,
                                handler_flags_628,
                                anim_mode,
                                1.0,
                            );
                            let owner_yaw_write = turn
                                .direct_owner_yaw_write
                                .then_some(turn.owner_yaw_radians);
                            let root_motion_policy = if turn.direct_owner_yaw_write {
                                NativeAiRootMotionPolicy::NONE
                            } else {
                                NativeAiRootMotionPolicy::FULL
                            };
                            let events = {
                                let runtime = self.native_npc_behavior.get_mut(&key).unwrap();
                                runtime.requested_anim_mode = Some(turn.requested_anim_mode);
                                runtime.direct_owner_yaw_write = turn.direct_owner_yaw_write;
                                if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                    runtime.animation.advance(
                                        body,
                                        turn.requested_anim_mode,
                                        owner_yaw_write,
                                        ROBOTS_NATIVE_FIXED_SECONDS as f32,
                                        root_motion_policy,
                                    )
                                } else {
                                    Vec::new()
                                }
                            };
                            if events.iter().any(|event| {
                                event.event_type
                                    == eurochef_shared::robots_runtime::events::event_type::SETUP_IDLE
                            }) {
                                self.native_npc_behavior
                                    .get_mut(&key)
                                    .unwrap()
                                    .animation
                                    .setup_idle();
                            }
                        }
                    }
                }
                Some(NativeNpcBehaviorNode::FollowPath) => {
                    let path_step = match (follow_path, follow_path_nodes.as_ref()) {
                        (Some(path), Some(nodes)) => {
                            let runtime = self.native_npc_behavior.get_mut(&key).unwrap();
                            runtime.follow_path.step(
                                npc_follow_network_path_config(),
                                path.path_type,
                                nodes,
                                owner_position.to_array(),
                            )
                        }
                        _ => None,
                    };
                    if let Some(path_step) = path_step {
                        if let Some(requested_anim_mode) = path_step.requested_anim_mode {
                            let events = {
                                let runtime = self.native_npc_behavior.get_mut(&key).unwrap();
                                runtime.last_action = None;
                                runtime.requested_anim_mode = Some(requested_anim_mode);
                                runtime.direct_owner_yaw_write = false;
                                if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                    runtime.animation.advance(
                                        body,
                                        requested_anim_mode,
                                        None,
                                        ROBOTS_NATIVE_FIXED_SECONDS as f32,
                                        NativeAiRootMotionPolicy::TRANSLATION_ONLY,
                                    )
                                } else {
                                    Vec::new()
                                }
                            };
                            if events.iter().any(|event| {
                                event.event_type
                                    == eurochef_shared::robots_runtime::events::event_type::SETUP_IDLE
                            }) {
                                let runtime = self.native_npc_behavior.get_mut(&key).unwrap();
                                runtime.follow_path.setup_idle();
                                runtime.animation.setup_idle();
                            }
                        } else if path_step.movement_enabled {
                            let locomotion = {
                                let runtime = self.native_npc_behavior.get_mut(&key).unwrap();
                                step_ai_locomotion_after_steering_prepass(
                                    &mut runtime.follow_path_locomotion,
                                    RobotsAiLocomotionInput {
                                        steering_target_yaw_radians: path_step.target_yaw_radians,
                                        target_locomotion_scalar: path_step
                                            .target_locomotion_scalar,
                                        turn_rate: RobotsAiTurnRateInput::Default,
                                        handler_flags_628,
                                        move_mode_active_on_entry: runtime
                                            .animation
                                            .is_mode(ROBOTS_ANIM_MODE_MOVE),
                                        current_owner_yaw_radians: owner_yaw,
                                        runtime_rate_scale: 1.0,
                                    },
                                )
                            };
                            let owner_yaw_write = locomotion
                                .direct_owner_yaw_write
                                .then_some(locomotion.owner_yaw_radians);
                            let root_motion_policy = locomotion_root_motion_policy(locomotion);
                            let runtime = self.native_npc_behavior.get_mut(&key).unwrap();
                            runtime.last_action = None;
                            runtime.requested_anim_mode = Some(locomotion.requested_anim_mode);
                            runtime.direct_owner_yaw_write = locomotion.direct_owner_yaw_write;
                            if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                let _ = runtime.animation.advance(
                                    body,
                                    locomotion.requested_anim_mode,
                                    owner_yaw_write,
                                    ROBOTS_NATIVE_FIXED_SECONDS as f32,
                                    root_motion_policy,
                                );
                            }
                        }
                    }
                }
                Some(NativeNpcBehaviorNode::Flag2) => {
                    let Some((region_ordinal, current_face, group_flags0)) = flag2_nav else {
                        continue;
                    };
                    let Some(nav) = nav_regions.get(region_ordinal).copied() else {
                        continue;
                    };

                    if previous_node != Some(NativeNpcBehaviorNode::Flag2) {
                        let enter_needs_rng = {
                            let runtime = self.native_npc_behavior.get(&key).unwrap();
                            npc_flag2_enter_needs_rebuild(&runtime.flag2, current_face)
                                && npc_flag2_rebuild_will_consume_rng(&runtime.flag2)
                        };
                        let enter_draws = if enter_needs_rng {
                            take_global_gameplay_rng_draws::<ROBOTS_NPC_FLAG2_ROUTE_SAMPLE_COUNT>(
                                &mut self.native_global_gameplay_rng,
                            )
                        } else {
                            None
                        };
                        let runtime = self.native_npc_behavior.get_mut(&key).unwrap();
                        enter_npc_flag2(
                            &mut runtime.flag2,
                            nav,
                            current_face,
                            group_flags0,
                            enter_draws,
                        );
                    }

                    // `0x00451DF0` runs before behavior execution and consumes the
                    // Character Physics velocity retained from the previous tick.
                    // Native root-motion application `0x004F3E8D` writes exactly
                    // that velocity lane, so the shared animation host exposes it
                    // explicitly instead of inferring velocity from owner motion.
                    let movement_error_ratio = {
                        let runtime = self.native_npc_behavior.get_mut(&key).unwrap();
                        let physics_velocity = runtime.animation.character_physics_velocity();
                        step_ai_movement_error(
                            &mut runtime.movement_error,
                            owner_position.to_array(),
                            physics_velocity.to_array(),
                            1.0,
                        )
                    };
                    let step_needs_rng = {
                        let runtime = self.native_npc_behavior.get(&key).unwrap();
                        npc_flag2_step_needs_rebuild(&runtime.flag2, movement_error_ratio)
                            && npc_flag2_rebuild_will_consume_rng(&runtime.flag2)
                    };
                    let step_draws = if step_needs_rng {
                        take_global_gameplay_rng_draws::<ROBOTS_NPC_FLAG2_ROUTE_SAMPLE_COUNT>(
                            &mut self.native_global_gameplay_rng,
                        )
                    } else {
                        None
                    };
                    let action = {
                        let runtime = self.native_npc_behavior.get_mut(&key).unwrap();
                        step_npc_flag2(
                            &mut runtime.flag2,
                            nav,
                            owner_position.to_array(),
                            current_face,
                            group_flags0,
                            movement_error_ratio,
                            step_draws,
                        )
                    };

                    match action {
                        RobotsNpcFlag2Action::RequestIdleAttack => {
                            let events = {
                                let runtime = self.native_npc_behavior.get_mut(&key).unwrap();
                                runtime.last_action = None;
                                runtime.requested_anim_mode = Some(ROBOTS_ANIM_MODE_IDLE_ATTACK);
                                runtime.direct_owner_yaw_write = false;
                                if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                    runtime.animation.advance(
                                        body,
                                        ROBOTS_ANIM_MODE_IDLE_ATTACK,
                                        None,
                                        ROBOTS_NATIVE_FIXED_SECONDS as f32,
                                        NativeAiRootMotionPolicy::NONE,
                                    )
                                } else {
                                    Vec::new()
                                }
                            };
                            if events.iter().any(|event| {
                                event.event_type
                                    == eurochef_shared::robots_runtime::events::event_type::SETUP_IDLE
                            }) {
                                self.native_npc_behavior
                                    .get_mut(&key)
                                    .unwrap()
                                    .animation
                                    .setup_idle();
                            }
                        }
                        RobotsNpcFlag2Action::SteerToWaypoint {
                            target_yaw_radians,
                            target_locomotion_scalar,
                            turn_rate_radians_per_second,
                        } => {
                            let locomotion = {
                                let runtime = self.native_npc_behavior.get_mut(&key).unwrap();
                                step_ai_locomotion_after_steering_prepass(
                                    &mut runtime.flag2_locomotion,
                                    RobotsAiLocomotionInput {
                                        steering_target_yaw_radians: target_yaw_radians,
                                        target_locomotion_scalar,
                                        turn_rate: RobotsAiTurnRateInput::Explicit(
                                            turn_rate_radians_per_second,
                                        ),
                                        handler_flags_628,
                                        move_mode_active_on_entry: runtime
                                            .animation
                                            .is_mode(ROBOTS_ANIM_MODE_MOVE),
                                        current_owner_yaw_radians: owner_yaw,
                                        runtime_rate_scale: 1.0,
                                    },
                                )
                            };
                            let owner_yaw_write = locomotion
                                .direct_owner_yaw_write
                                .then_some(locomotion.owner_yaw_radians);
                            let root_motion_policy = locomotion_root_motion_policy(locomotion);
                            let events = {
                                let runtime = self.native_npc_behavior.get_mut(&key).unwrap();
                                runtime.requested_anim_mode = Some(locomotion.requested_anim_mode);
                                runtime.direct_owner_yaw_write = locomotion.direct_owner_yaw_write;
                                if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                    runtime.animation.advance(
                                        body,
                                        locomotion.requested_anim_mode,
                                        owner_yaw_write,
                                        ROBOTS_NATIVE_FIXED_SECONDS as f32,
                                        root_motion_policy,
                                    )
                                } else {
                                    Vec::new()
                                }
                            };
                            if events.iter().any(|event| {
                                event.event_type
                                    == eurochef_shared::robots_runtime::events::event_type::SETUP_IDLE
                            }) {
                                self.native_npc_behavior
                                    .get_mut(&key)
                                    .unwrap()
                                    .animation
                                    .setup_idle();
                            }
                        }
                    }
                }
                Some(NativeNpcBehaviorNode::DinerIdle) => {
                    let events = {
                        let runtime = self.native_npc_behavior.get_mut(&key).unwrap();
                        runtime.last_action = None;
                        runtime.requested_anim_mode = Some(ROBOTS_NPC_DINER_IDLE_ANIM_MODE);
                        runtime.direct_owner_yaw_write = false;
                        if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                            runtime.animation.advance(
                                body,
                                ROBOTS_NPC_DINER_IDLE_ANIM_MODE,
                                None,
                                ROBOTS_NATIVE_FIXED_SECONDS as f32,
                                NativeAiRootMotionPolicy::NONE,
                            )
                        } else {
                            Vec::new()
                        }
                    };
                    if events.iter().any(|event| {
                        event.event_type
                            == eurochef_shared::robots_runtime::events::event_type::SETUP_IDLE
                    }) {
                        self.native_npc_behavior
                            .get_mut(&key)
                            .unwrap()
                            .animation
                            .setup_idle();
                    }
                }
                None => {
                    let runtime = self.native_npc_behavior.get_mut(&key).unwrap();
                    runtime.last_action = None;
                    runtime.requested_anim_mode = None;
                    runtime.direct_owner_yaw_write = false;
                    if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                        runtime.animation.reset(body);
                    }
                }
            }

            // 0x004570C0 executes the winner before ticking every behavior node.
            let runtime = self.native_npc_behavior.get_mut(&key).unwrap();
            tick_npc_random_idle(&mut runtime.random_idle);
            if flag2_present {
                tick_npc_flag2(&mut runtime.flag2, 1.0);
            }
        }
    }

    fn advance_native_player_focus_fixed(
        &mut self,
        map: &ProcessedMap,
        wall_time: f64,
        player_position: Vec3,
        player_yaw: f32,
    ) {
        if let Some(owner) = self.native_player_focus_runtime.current_owner {
            let xitem_exists = match owner.category {
                ROBOTS_XITEM_CATEGORY_NPC => self
                    .native_ai_trigger_lifecycle
                    .get(&owner.key)
                    .is_some_and(|state| state.xitem_exists),
                ROBOTS_XITEM_CATEGORY_ACTIVATION_PAD
                | ROBOTS_XITEM_CATEGORY_SLIDE_UNDER
                | ROBOTS_XITEM_CATEGORY_ALERT_ICON => self
                    .native_lightweight_trigger_lifecycle
                    .get(&owner.key)
                    .is_some_and(|state| state.xitem_exists),
                _ => true,
            };
            if !xitem_exists {
                self.native_player_focus_runtime.current_owner = None;
            }
        }

        for (trigger_index, trigger) in map.triggers.iter().enumerate() {
            let key = Self::runtime_event_key(map.hashcode, trigger_index);
            match trigger.ttype {
                ROBOTS_NPC_TYPE => {
                    let xitem_exists = self
                        .native_ai_trigger_lifecycle
                        .get(&key)
                        .is_some_and(|state| state.xitem_exists);
                    if !xitem_exists {
                        let _ = self.native_npc_focus_update(map, trigger_index, false);
                        continue;
                    }
                    let Some(body) = self.runtime_character_bodies.get(&key) else {
                        let _ = self.native_npc_focus_update(map, trigger_index, false);
                        continue;
                    };
                    let (focus_point, range) =
                        Self::native_npc_focus_geometry(map, trigger, body.owner_position);
                    let decision = plan_player_focus_candidate(RobotsPlayerFocusCandidateInput {
                        player_position: player_position.to_array(),
                        player_yaw,
                        player_state: self.native_player_focus_runtime.player_state,
                        current_interaction_code: self
                            .native_player_focus_runtime
                            .current_interaction_code,
                        current_owner: self.native_player_focus_owner_view(map, key),
                        candidate_category: ROBOTS_XITEM_CATEGORY_NPC,
                        candidate_focus_point: focus_point.to_array(),
                        range,
                        npc_proxy_state9_blocked: self
                            .native_player_focus_runtime
                            .npc_proxy_state9_blocked,
                    });
                    match decision {
                        RobotsPlayerFocusDecision::KeepCurrent => {}
                        RobotsPlayerFocusDecision::ClearCurrent => {
                            if self
                                .native_player_focus_runtime
                                .current_owner
                                .is_some_and(|owner| owner.key == key)
                            {
                                self.native_player_focus_runtime.current_owner = None;
                            }
                        }
                        RobotsPlayerFocusDecision::SelectCandidate => {
                            self.native_player_focus_runtime.current_owner =
                                Some(NativePlayerFocusOwner {
                                    key,
                                    category: ROBOTS_XITEM_CATEGORY_NPC,
                                });
                        }
                    }
                    let is_player_focus_target = self
                        .native_player_focus_runtime
                        .current_owner
                        .is_some_and(|owner| owner.key == key);
                    let _ =
                        self.native_npc_focus_update(map, trigger_index, is_player_focus_target);
                }
                ROBOTS_ACTIVATION_PAD_TYPE => {
                    let xitem_exists = self
                        .native_lightweight_trigger_lifecycle
                        .get(&key)
                        .is_some_and(|state| state.xitem_exists);
                    let decision =
                        plan_activation_pad_focus_candidate(RobotsActivationPadFocusInput {
                            player_position: player_position.to_array(),
                            player_yaw,
                            player_state: self.native_player_focus_runtime.player_state,
                            current_interaction_code: self
                                .native_player_focus_runtime
                                .current_interaction_code,
                            current_owner: self.native_player_focus_owner_view(map, key),
                            candidate_focus_point: trigger.position.to_array(),
                            xitem_exists,
                            watchbot_controller_enabled: self
                                .native_player_item_state
                                .is_enabled(ROBOTS_PLAYER_ITEM_WATCHBOT_CONTROLLER),
                            game_control_mode: self.native_game_control_runtime.mode_50f,
                        });
                    match decision {
                        RobotsPlayerFocusDecision::KeepCurrent => {}
                        RobotsPlayerFocusDecision::ClearCurrent => {
                            if self
                                .native_player_focus_runtime
                                .current_owner
                                .is_some_and(|owner| owner.key == key)
                            {
                                self.native_player_focus_runtime.current_owner = None;
                            }
                        }
                        RobotsPlayerFocusDecision::SelectCandidate => {
                            self.native_player_focus_runtime.current_owner =
                                Some(NativePlayerFocusOwner {
                                    key,
                                    category: ROBOTS_XITEM_CATEGORY_ACTIVATION_PAD,
                                });
                        }
                    }
                }
                ROBOTS_SLIDE_UNDER_TYPE => {
                    let xitem_exists = self
                        .native_lightweight_trigger_lifecycle
                        .get(&key)
                        .is_some_and(|state| state.xitem_exists);
                    let state_e4 = self
                        .native_trigger_graph
                        .get(&key)
                        .and_then(|state| state.owned_xitem_active_e4)
                        .unwrap_or_else(|| {
                            trigger.data.get(2).copied().flatten().unwrap_or_default() as u8
                        });
                    let scaled_radius = |slot: usize| {
                        (trigger
                            .data
                            .get(slot)
                            .copied()
                            .flatten()
                            .unwrap_or_default() as i32 as f32
                            * 0.1)
                            .abs()
                    };
                    let current_focus_is_candidate = self
                        .native_player_focus_runtime
                        .current_owner
                        .is_some_and(|owner| owner.key == key);
                    let step = plan_slide_under_player(RobotsSlideUnderInput {
                        player_position: player_position.to_array(),
                        trigger_position: trigger.position.to_array(),
                        state_e4,
                        xitem_exists,
                        inner_radius: scaled_radius(0),
                        outer_radius: scaled_radius(1),
                        player_state: self.native_player_focus_runtime.player_state,
                        current_focus_is_candidate,
                        current_slide_under: self.native_slide_under_owner_view(map, key),
                    });

                    if commit_native_slide_under_player_step(
                        &mut self.native_player_focus_runtime,
                        key,
                        step,
                    ) {
                        let outputs = (0..2usize)
                            .filter_map(|slot| {
                                let link = trigger.links.get(slot).copied()?;
                                let (target_index, _) = map_trigger_by_link(map, link)?;
                                Some(target_index)
                            })
                            .collect::<Vec<_>>();
                        for target_index in outputs {
                            self.dispatch_runtime_event_from(
                                map,
                                target_index,
                                0x200,
                                wall_time,
                                Some(trigger_index),
                            );
                        }
                    }
                }
                ROBOTS_ALERT_ICON_TYPE => {
                    let xitem_exists = self
                        .native_lightweight_trigger_lifecycle
                        .get(&key)
                        .is_some_and(|state| state.xitem_exists);
                    let active_e4 = self
                        .native_trigger_graph
                        .get(&key)
                        .and_then(|state| state.owned_xitem_active_e4)
                        .unwrap_or_else(|| {
                            trigger.data.get(3).copied().flatten().unwrap_or_default() as u8
                        })
                        == 1;
                    let horizontal_radius =
                        (trigger.data.get(1).copied().flatten().unwrap_or_default() as i32 as f32
                            * 0.1)
                            .abs();
                    let vertical_tolerance =
                        (trigger.data.get(2).copied().flatten().unwrap_or_default() as i32 as f32
                            * 0.1)
                            .abs();
                    let current_owner_is_candidate = self
                        .native_player_focus_runtime
                        .current_owner
                        .is_some_and(|owner| owner.key == key);
                    let blocked_by_active_mission = self.native_mission_owner.is_some()
                        && trigger.data.first().copied().flatten() == Some(10);
                    let decision = plan_alert_icon_focus(RobotsAlertIconFocusInput {
                        player_position: player_position.to_array(),
                        trigger_position: trigger.position.to_array(),
                        active_e4,
                        xitem_exists,
                        horizontal_radius,
                        vertical_tolerance,
                        blocked_by_global_mode: blocked_by_active_mission,
                        current_owner_is_candidate,
                        current_owner_present: self
                            .native_player_focus_runtime
                            .current_owner
                            .is_some(),
                    });
                    match decision {
                        RobotsPlayerFocusDecision::KeepCurrent => {}
                        RobotsPlayerFocusDecision::ClearCurrent => {
                            if current_owner_is_candidate {
                                self.native_player_focus_runtime.current_owner = None;
                            }
                        }
                        RobotsPlayerFocusDecision::SelectCandidate => {
                            self.native_player_focus_runtime.current_owner =
                                Some(NativePlayerFocusOwner {
                                    key,
                                    category: ROBOTS_XITEM_CATEGORY_ALERT_ICON,
                                });
                        }
                    }
                }
                _ => {}
            }
        }
    }

    pub(super) fn native_npc_focus_update(
        &mut self,
        map: &ProcessedMap,
        trigger_index: usize,
        is_player_focus_target: bool,
    ) -> Option<RobotsNpcFocusAction> {
        let trigger = map.triggers.get(trigger_index)?;
        (trigger.ttype == ROBOTS_NPC_TYPE).then_some(())?;
        let contract = decode_npc_serialized_contract_slice(&trigger.data);
        let key = Self::runtime_event_key(map.hashcode, trigger_index);
        let state = self.native_trigger_graph.entry(key).or_default();
        let action = update_npc_focus(&mut state.npc, &contract, is_player_focus_target);
        if matches!(action, RobotsNpcFocusAction::StopSimpleText) {
            // Native focus loss calls 0x0049C240, which clears the shared
            // XTextManager message queue rather than an NPC-local entry.
            self.native_message_presentation.clear_all();
        }
        Some(action)
    }

    pub(super) fn native_npc_interaction_plan(
        &mut self,
        map: &ProcessedMap,
        trigger_index: usize,
        wall_time: f64,
    ) -> Option<RobotsNpcInteractionAction> {
        let trigger = map.triggers.get(trigger_index)?;
        (trigger.ttype == ROBOTS_NPC_TYPE).then_some(())?;
        let contract = decode_npc_serialized_contract_slice(&trigger.data);

        let mission_index =
            Self::native_first_linked_trigger_index(map, trigger, ROBOTS_MISSION_TYPE);
        let tutorial_index = mission_index
            .is_none()
            .then(|| Self::native_first_linked_trigger_index(map, trigger, ROBOTS_TUTORIAL_TYPE))
            .flatten();

        let (mission_uid, mission_status, objective_progress) = if let Some(index) = mission_index {
            let mission = map.triggers.get(index)?;
            let mission_uid = mission.data.first().copied().flatten();
            let valid_mission_uid =
                mission_uid.filter(|uid| !matches!(*uid, 0 | u32::MAX | 0x5400_0000));
            let mission_status = valid_mission_uid.map(|uid| {
                RobotsNpcObjectiveStatus::from(
                    self.native_script_gameplay_state.missions.status(uid),
                )
            });
            let objective_progress = valid_mission_uid.and_then(|uid| {
                map.mission_definitions
                    .iter()
                    .copied()
                    .find(|definition| definition.mission_uid == uid)
                    .and_then(|definition| {
                        resolve_mission_objective_progress(
                            definition,
                            &map.inventory_definitions,
                            &self.native_script_gameplay_state.inventory,
                        )
                    })
            });
            (mission_uid, mission_status, objective_progress)
        } else {
            (None, None, None)
        };

        let tutorial_plan = if let Some(index) = tutorial_index {
            let key = Self::runtime_event_key(map.hashcode, index);
            let plan = {
                let state = self.native_trigger_graph.entry(key).or_default();
                plan_npc_tutorial_interaction(Some(&mut state.tutorial_interaction))
            };
            if let RobotsNpcTutorialInteractionPlan::Activate { event_mask } = plan {
                self.dispatch_runtime_event(map, index, event_mask, wall_time);
            }
            plan
        } else {
            RobotsNpcTutorialInteractionPlan::NotConsumed
        };

        let key = Self::runtime_event_key(map.hashcode, trigger_index);
        let state = self.native_trigger_graph.entry(key).or_default();
        Some(plan_npc_interaction(
            &contract,
            RobotsNpcInteractionInput {
                trigger_present: true,
                objective_uid: mission_uid,
                objective_status: mission_status,
                objective_progress,
                tutorial_consumes_interaction: tutorial_plan.consumed(),
                latch_e10c: state.npc.latch_e10c,
                latch_e10d: state.npc.latch_e10d,
            },
        ))
    }

    pub(super) fn native_npc_execute_interaction(
        &mut self,
        map: &ProcessedMap,
        trigger_index: usize,
        wall_time: f64,
    ) -> Option<RobotsNpcPresentationPlan> {
        let action = self.native_npc_interaction_plan(map, trigger_index, wall_time)?;
        let npc = map.triggers.get(trigger_index)?;
        let contract = decode_npc_serialized_contract_slice(&npc.data);
        let presentation = plan_npc_presentation(&contract, action);

        let (alternate_uid, copy_position, copy_rotation) = match presentation {
            RobotsNpcPresentationPlan::None => return Some(presentation),
            RobotsNpcPresentationPlan::StartSimpleText { text_group_uid } => {
                // 0x0046BBA0 checks HUD manager +0x1C / DAT_007B318C before
                // TextGroup resolution, so a busy message box consumes no group RNG.
                if self.native_message_presentation.message_box_exists {
                    return Some(presentation);
                }
                let message_uid = select_text_group_message(
                    map.text_groups.messages(text_group_uid),
                    &mut self.native_text_group_selection,
                    &mut self.native_process_lcg_seed,
                );
                if let Some(message_uid) = message_uid {
                    let Some(message) = map.text_groups.message_definition(message_uid) else {
                        return Some(presentation);
                    };
                    let sound_duration_seconds = if message.sound_uid & 0x7F00_0000 == 0x1A00_0000 {
                        self.sound_preview
                            .lock()
                            .native_sound_profile(message.sound_uid)
                            .map(|profile| profile.duration_seconds)
                    } else {
                        None
                    };
                    let npc_key = Self::runtime_event_key(map.hashcode, trigger_index);
                    let state = self.native_trigger_graph.entry(npc_key).or_default();
                    if start_npc_simple_text(&mut state.npc, &contract, false, true).is_some() {
                        let native_frame = (wall_time * 60.0) as f32;
                        let record = build_npc_simple_text_record(
                            message,
                            native_frame,
                            sound_duration_seconds,
                        );
                        self.native_message_presentation.enqueue(record);
                    }
                }
                return Some(presentation);
            }
            RobotsNpcPresentationPlan::ActivateCutscene {
                alternate_uid,
                copy_position,
                copy_rotation,
            } => (alternate_uid, copy_position, copy_rotation),
        };

        let cutscene_index =
            Self::native_first_linked_trigger_index(map, npc, ROBOTS_CUTSCENE_TYPE)?;
        let npc_key = Self::runtime_event_key(map.hashcode, trigger_index);
        let (live_position, live_rotation) = self
            .runtime_character_bodies
            .get(&npc_key)
            .map(|body| (body.owner_position, body.owner_rotation))
            .unwrap_or_else(|| {
                (
                    npc.position,
                    Quat::from_euler(
                        glam::EulerRot::ZXY,
                        npc.rotation[2],
                        npc.rotation[0],
                        npc.rotation[1],
                    ),
                )
            });
        let cutscene_key = Self::runtime_event_key(map.hashcode, cutscene_index);
        self.native_cutscene_runtime
            .entry(cutscene_key)
            .or_default()
            .prepare_npc_activation(
                alternate_uid,
                copy_position.then_some(live_position),
                copy_rotation.then_some(live_rotation),
            );

        if matches!(
            action,
            RobotsNpcInteractionAction::MissionCutscene {
                set_latch_e10d: true,
                ..
            }
        ) {
            self.native_trigger_graph
                .entry(npc_key)
                .or_default()
                .npc
                .latch_e10d = true;
        }

        // `XTrigger_NPC::ActivateCutscene` writes Cutscene +0xE4/pose first and
        // then routes exact event 0x101 through the ordinary trigger dispatcher.
        self.dispatch_runtime_event(map, cutscene_index, 0x101, wall_time);
        Some(presentation)
    }

    fn advance_native_script_trigger_lifecycle_fixed(
        &mut self,
        map: &ProcessedMap,
        wall_time: f64,
        player_position: Option<Vec3>,
        creates_this_update: &mut u32,
    ) -> Option<FxHashMap<u64, NativeMonsterTransporterFixedStep>> {
        if !self.native_script_trigger_lifecycle_valid
            || self.native_zone_runtime_map != Some(map.hashcode)
            || !self.apply_native_camera_viewport
            || !self.native_camera_viewpoint_valid
        {
            return None;
        }
        let Some(player_position) = player_position else {
            return None;
        };

        let mut transporter_steps = FxHashMap::default();
        let mut transporter_resets = Vec::new();
        let mut active_transporters = Vec::new();
        for (trigger_index, trigger) in map.triggers.iter().enumerate() {
            let key = Self::runtime_event_key(map.hashcode, trigger_index);
            if self.native_trigger_disabled(map.hashcode, trigger_index, trigger) {
                // 0x0044C119..0x0044C129: disabled triggers skip TriggerManager
                // distance/tick work. Only bit28 additionally requests +0x28(1)
                // cleanup; without bit28 an existing XItem keeps running.
                if trigger.game_flags & 0x1000_0000 != 0 {
                    if trigger.ttype == ROBOTS_CUTSCENE_TYPE {
                        self.native_cutscene_runtime
                            .entry(key)
                            .or_default()
                            .cleanup_zone_failed();
                    } else if robots_character_runtime_type(trigger.ttype).is_some() {
                        self.native_ai_trigger_lifecycle
                            .entry(key)
                            .or_default()
                            .cleanup_zone_failed();
                    } else if trigger.ttype == ROBOTS_MONSTER_TRANSPORTER_TYPE {
                        self.native_monster_transporter_lifecycle
                            .entry(key)
                            .or_default()
                            .cleanup_zone_failed();
                        self.native_monster_transporters.remove(&key);
                    } else if trigger.ttype == ROBOTS_BOSS_SEWER_CANON_TYPE {
                        self.native_camera_bit0_trigger_lifecycle
                            .entry(key)
                            .or_default()
                            .cleanup_zone_failed();
                    } else if trigger.ttype == ROBOTS_SCRIPT_TRIGGER_TYPE {
                        self.native_script_trigger_lifecycle
                            .entry(key)
                            .or_default()
                            .cleanup_zone_failed();
                    }
                }
                continue;
            }
            if trigger.ttype == ROBOTS_CUTSCENE_TYPE {
                // XTrigger_Cutscene vtable +0x24 = 0x00488340. The class event
                // slot +0x6C is the trivial 0x0048C840; actual activation is
                // therefore the common TriggerManager/CreateItem lifecycle.
                // NPC flags 0x100/0x200 may first overwrite the Cutscene trigger
                // pose, so distance/zone ownership must use the pending override.
                let effective_position = self
                    .native_cutscene_runtime
                    .get(&key)
                    .and_then(|state| state.position_override)
                    .unwrap_or(trigger.position);
                let Some(zone_index) = map.native_zone_index(effective_position) else {
                    self.native_cutscene_runtime
                        .entry(key)
                        .or_default()
                        .cleanup_zone_failed();
                    continue;
                };
                let Some(zone_state) = self.native_zone_runtime.get(zone_index).copied() else {
                    continue;
                };
                let has_change_level_link = trigger.links.iter().take(8).any(|link| {
                    map_trigger_by_link(map, *link)
                        .is_some_and(|(_, linked)| linked.ttype == ROBOTS_CHANGE_LEVEL_TYPE)
                });
                let has_cutscene_link = trigger.links.iter().take(8).any(|link| {
                    map_trigger_by_link(map, *link)
                        .is_some_and(|(_, linked)| linked.ttype == ROBOTS_CUTSCENE_TYPE)
                });
                let suppress_linked_cutscene =
                    trigger.data.get(3).copied().flatten().unwrap_or_default() & 0x2 != 0;
                let state = self.native_cutscene_runtime.entry(key).or_default();
                state.ensure_finalize_routes_initialized(
                    has_change_level_link,
                    has_cutscene_link,
                    suppress_linked_cutscene,
                );
                if !zone_state.activated || zone_state.visual_depth <= 0 {
                    state.cleanup_zone_failed();
                    continue;
                }
                let distance_squared = effective_position.distance_squared(player_position);
                let Some(event) =
                    runtime_trigger_distance_event(distance_squared, trigger.game_flags)
                else {
                    continue;
                };
                let action =
                    runtime_common_trigger_lifecycle_action(event, true, trigger.game_flags);
                let proximity_factor = runtime_trigger_normal_proximity_factor(distance_squared);
                let created_now = state.advance_trigger_manager(
                    action,
                    trigger.game_flags,
                    proximity_factor,
                    creates_this_update,
                );
                if created_now {
                    self.native_cutscene_host_runtime.global_cutscene_active = true;
                    // Native 0x00409730 resolves the Cutscene Script only after
                    // XTrigger_Cutscene::CreateItem has copied +0xE4 into handler
                    // +0x3B8. The alternate changes only the Script UID; owner
                    // file remains trigger +0x50 or the current EDB fallback.
                    state.bind_script_on_create(
                        self.file,
                        trigger.engine_options.visual_object_file,
                        trigger.engine_options.visual_object,
                    );
                }
                continue;
            }
            if robots_character_runtime_type(trigger.ttype).is_some() {
                // All eight shipped XTrigger_AI_Character families share +0x24 =
                // 0x0047E4F0 -> 0x0047E740. MonsterDatabase/resource resolution is
                // already attached to character_visual; without it native CreateItem
                // cannot be reproduced, so keep this trigger fail-closed.
                if trigger.character_visual.is_none() {
                    self.native_ai_trigger_lifecycle.remove(&key);
                    continue;
                }
                let creator_state = self
                    .native_ai_creator_runtime
                    .get(&key)
                    .copied()
                    .unwrap_or_default();
                let effective_position = if self
                    .native_ai_trigger_lifecycle
                    .get(&key)
                    .is_some_and(|state| state.xitem_exists)
                {
                    self.runtime_character_bodies
                        .get(&key)
                        .map(|body| body.owner_position)
                        .unwrap_or(trigger.position)
                } else {
                    creator_state.position_override.unwrap_or(trigger.position)
                };
                // Native XTrigger_AI_Character::CreateItem 0x0047E4F0 requires
                // creator +0x104 == 0. Common natural death sets it to one.
                if !creator_state.allows_create_item() {
                    continue;
                }
                let Some(zone_index) = map.native_zone_index(effective_position) else {
                    self.native_ai_trigger_lifecycle
                        .entry(key)
                        .or_default()
                        .cleanup_zone_failed();
                    continue;
                };
                let Some(zone_state) = self.native_zone_runtime.get(zone_index).copied() else {
                    continue;
                };
                let state = self.native_ai_trigger_lifecycle.entry(key).or_default();
                if !zone_state.activated || zone_state.visual_depth <= 0 {
                    state.cleanup_zone_failed();
                    continue;
                }
                let distance_squared = effective_position.distance_squared(player_position);
                let Some(event) =
                    runtime_trigger_distance_event(distance_squared, trigger.game_flags)
                else {
                    continue;
                };
                let action =
                    runtime_common_trigger_lifecycle_action(event, true, trigger.game_flags);
                let proximity_factor = runtime_trigger_normal_proximity_factor(distance_squared);
                state.advance_trigger_manager(
                    action,
                    trigger.game_flags,
                    proximity_factor,
                    creates_this_update,
                );
                continue;
            }
            if trigger.ttype == ROBOTS_MONSTER_TRANSPORTER_TYPE {
                let Some((_, _, primary)) = map_trigger_runtime_path(map, trigger) else {
                    self.native_monster_transporter_lifecycle.remove(&key);
                    self.native_monster_transporters.remove(&key);
                    continue;
                };
                let Some(zone_index) = map.native_zone_index(trigger.position) else {
                    self.native_monster_transporter_lifecycle
                        .entry(key)
                        .or_default()
                        .cleanup_zone_failed();
                    self.native_monster_transporters.remove(&key);
                    continue;
                };
                let Some(zone_state) = self.native_zone_runtime.get(zone_index).copied() else {
                    continue;
                };
                let state = self
                    .native_monster_transporter_lifecycle
                    .entry(key)
                    .or_default();
                if !zone_state.activated || zone_state.visual_depth <= 0 {
                    state.cleanup_zone_failed();
                    self.native_monster_transporters.remove(&key);
                    continue;
                }
                let Some(distance_squared) =
                    robots_monster_transporter_route_distance_squared(primary, player_position)
                else {
                    continue;
                };
                let Some(event) =
                    runtime_trigger_distance_event(distance_squared, trigger.game_flags)
                else {
                    continue;
                };
                let action =
                    runtime_common_trigger_lifecycle_action(event, true, trigger.game_flags);
                let proximity_factor = runtime_trigger_normal_proximity_factor(distance_squared);
                state.advance_trigger_manager(
                    action,
                    trigger.game_flags,
                    proximity_factor,
                    creates_this_update,
                );
                if state.xitem_exists {
                    active_transporters.push((key, trigger_index));
                } else {
                    self.native_monster_transporters.remove(&key);
                }
                continue;
            }

            if trigger.ttype == ROBOTS_BOSS_SEWER_CANON_TYPE
                && trigger.data.first().copied().flatten().unwrap_or_default() >= 1
            {
                let Some(zone_index) = map.native_zone_index(trigger.position) else {
                    self.native_camera_bit0_trigger_lifecycle
                        .entry(key)
                        .or_default()
                        .cleanup_zone_failed();
                    continue;
                };
                let Some(zone_state) = self.native_zone_runtime.get(zone_index).copied() else {
                    continue;
                };
                let state = self
                    .native_camera_bit0_trigger_lifecycle
                    .entry(key)
                    .or_default();
                if !zone_state.activated || zone_state.visual_depth <= 0 {
                    state.cleanup_zone_failed();
                    continue;
                }
                let distance_squared = trigger.position.distance_squared(player_position);
                let Some(event) =
                    runtime_trigger_distance_event(distance_squared, trigger.game_flags)
                else {
                    continue;
                };
                let action =
                    runtime_common_trigger_lifecycle_action(event, true, trigger.game_flags);
                let proximity_factor = runtime_trigger_normal_proximity_factor(distance_squared);
                state.advance_trigger_manager(
                    action,
                    trigger.game_flags,
                    proximity_factor,
                    creates_this_update,
                );
                continue;
            }

            if trigger.ttype != ROBOTS_SCRIPT_TRIGGER_TYPE
                || trigger.data.first().copied().flatten().unwrap_or_default()
                    & ROBOTS_SCRIPT_DYNAMIC_RAYCAST_FLAG
                    == 0
            {
                continue;
            }
            if trigger.game_flags & ROBOTS_SCRIPT_UNSUPPORTED_DISTANCE_FLAGS != 0 {
                self.native_script_trigger_lifecycle.remove(&key);
                continue;
            }
            let Some(zone_index) = map.native_zone_index(trigger.position) else {
                self.native_script_trigger_lifecycle
                    .entry(key)
                    .or_default()
                    .cleanup_zone_failed();
                continue;
            };
            let Some(zone_state) = self.native_zone_runtime.get(zone_index).copied() else {
                continue;
            };
            let state = self.native_script_trigger_lifecycle.entry(key).or_default();
            if !zone_state.activated || zone_state.visual_depth <= 0 {
                state.cleanup_zone_failed();
                continue;
            }

            let distance_squared = trigger.position.distance_squared(player_position);
            let Some(event) = runtime_trigger_distance_event(distance_squared, trigger.game_flags)
            else {
                continue;
            };
            let action = runtime_common_trigger_lifecycle_action(event, true, trigger.game_flags);
            let proximity_factor = runtime_trigger_normal_proximity_factor(distance_squared);
            state.advance_trigger_manager(
                action,
                trigger.game_flags,
                proximity_factor,
                creates_this_update,
            );
        }

        for (key, trigger_index) in active_transporters {
            let Some(trigger) = map.triggers.get(trigger_index) else {
                continue;
            };
            let Some((_, _, primary)) = map_trigger_runtime_path(map, trigger) else {
                continue;
            };
            let secondary =
                robots_monster_transporter_secondary_path_hash(trigger.ttype, &trigger.data)
                    .and_then(|hashcode| map.paths.iter().find(|path| path.hashcode == hashcode));
            let Some(speed) = robots_monster_transporter_path_speed(trigger.ttype, &trigger.data)
            else {
                continue;
            };
            if !self.native_monster_transporters.contains_key(&key) {
                let Some(runtime) = NativeMonsterTransporterRuntime::new(primary, secondary, speed)
                else {
                    continue;
                };
                self.native_monster_transporters.insert(key, runtime);
            }
            if let Some(runtime) = self.native_monster_transporters.get_mut(&key) {
                let step = runtime.advance_fixed_tick();
                if !step.events.is_empty() {
                    if step.events.iter().any(|event| {
                        matches!(event, NativeMonsterTransporterPathEvent::ResetReload)
                    }) {
                        transporter_resets.push(trigger_index);
                    }
                    transporter_steps.insert(key, step);
                }
            }
        }

        for trigger_index in transporter_resets {
            self.dispatch_runtime_event(map, trigger_index, 0x1000, wall_time);
        }

        // 0x00444D56 -> 0x00444E30 runs after TriggerManager in the same native
        // update. Script Handler state therefore updates +0x258 before the common
        // XItem opacity pass 0x00443FF0.
        for (trigger_index, trigger) in map.triggers.iter().enumerate() {
            let key = Self::runtime_event_key(map.hashcode, trigger_index);
            if trigger.ttype == ROBOTS_CUTSCENE_TYPE {
                let effective_position = self
                    .native_cutscene_runtime
                    .get(&key)
                    .and_then(|state| state.position_override)
                    .unwrap_or(trigger.position);
                let zone_visual_active = map
                    .native_zone_index(effective_position)
                    .and_then(|zone_index| self.native_zone_runtime.get(zone_index))
                    .is_some_and(|zone| zone.activated && zone.visual_depth > 0);
                let selected_script = self
                    .native_cutscene_runtime
                    .get(&key)
                    .and_then(|state| state.active_script)
                    .and_then(|selection| {
                        self.render_store
                            .read()
                            .get_script(selection.owner_file_uid, selection.selected_script_uid)
                            .cloned()
                    });
                let needs_audio_scan = selected_script.is_some()
                    && self.native_cutscene_runtime.get(&key).is_some_and(|state| {
                        state.lifecycle.xitem_exists && !state.script_audio_scan_initialized
                    });
                if needs_audio_scan {
                    let audio_scan = {
                        let mut preview = self.sound_preview.lock();
                        selected_script.as_ref().map(|script| {
                            scan_cutscene_script_audio(script, |sound_uid| {
                                if sound_uid & 0x7F00_0000 != 0x1A00_0000 {
                                    return None;
                                }
                                let details_uid = 0x1A00_0000 | (sound_uid & 0x000F_FFFF);
                                preview
                                    .native_sound_profile(details_uid)
                                    .map(|profile| profile.sample_streamed)
                            })
                        })
                    };
                    if let (Some(state), Some(audio_scan)) =
                        (self.native_cutscene_runtime.get_mut(&key), audio_scan)
                    {
                        state.bind_script_audio_scan(audio_scan);
                    }
                }
                let trigger_flags = trigger.data.get(1).copied().flatten().unwrap_or_default();
                let fade_progress = self.native_camera_fade_transition.fade.progress;
                let handler_input = RobotsCutsceneHandlerHostInput {
                    trigger_flags,
                    fade_available: true,
                    fade_low_reached: fade_progress <= NATIVE_FADE_LOW_THRESHOLD,
                    fade_high_reached: fade_progress >= NATIVE_FADE_HIGH_THRESHOLD,
                    creator_script_present: selected_script.is_some(),
                };
                let handler_effect = self
                    .native_cutscene_runtime
                    .get_mut(&key)
                    .and_then(|state| {
                        state.pending_handler_effects.clear();
                        let step = state.advance_cutscene_handler(handler_input);
                        if let Some(effect) = step.effect {
                            state.pending_handler_effects.push(effect);
                        }
                        step.effect
                    });
                let finalized_branch = match handler_effect {
                    Some(RobotsCutsceneHandlerHostEffect::RequestFadeOut { duration_updates }) => {
                        self.native_camera_fade_transition
                            .fade
                            .request(NATIVE_FADE_OUT_STATE, duration_updates);
                        None
                    }
                    Some(RobotsCutsceneHandlerHostEffect::RequestFadeIn { duration_updates }) => {
                        self.native_camera_fade_transition
                            .fade
                            .request(NATIVE_FADE_IN_STATE, duration_updates);
                        None
                    }
                    Some(RobotsCutsceneHandlerHostEffect::FinalizeCutscene) => {
                        self.finalize_native_cutscene(map, trigger_index, wall_time)
                    }
                    None => None,
                };
                if finalized_branch
                    .is_some_and(|branch| branch != RobotsCutsceneFinalizeBranch::Ordinary)
                {
                    continue;
                }

                let gameplay_state = &mut self.native_script_gameplay_state;
                let mut host_music_volume_percent = None;
                let mut host_effects = Vec::new();
                let mut restore_player_health = false;
                if let Some(state) = self.native_cutscene_runtime.get_mut(&key) {
                    state.advance_xitem(zone_visual_active);
                    state.pending_effects.clear();
                    if state.lifecycle.xitem_exists {
                        if let Some(script) = selected_script.as_ref() {
                            let mut effects = Vec::new();
                            let _step = state.advance_script(script, |event| {
                                let Some(execution) = execute_recovered_handler_script_command(
                                    RobotsHandlerScriptCommandFamily::Cutscene,
                                    event,
                                    &map.inventory_definitions,
                                    &map.mission_definitions,
                                    gameplay_state,
                                    false,
                                ) else {
                                    return RobotsScriptNativeEventResult::Hold;
                                };
                                if execution.mutations.iter().any(|mutation| {
                                    matches!(
                                        mutation,
                                        RobotsScriptCommandMutation::Inventory { item, outcome }
                                            if *item == ROBOTS_HEALTH_REPLENISH_UID
                                                && matches!(outcome.native_result_code, 1 | 4)
                                    )
                                }) {
                                    restore_player_health = true;
                                }
                                if let Some(effect) = plan_cutscene_effect(&execution.plan.semantic)
                                {
                                    effects.push(effect);
                                }
                                match execution.plan.return_policy {
                                    RobotsHandlerEventReturnPolicy::Zero => {
                                        RobotsScriptNativeEventResult::Continue
                                    }
                                    RobotsHandlerEventReturnPolicy::One => {
                                        RobotsScriptNativeEventResult::Hold
                                    }
                                    RobotsHandlerEventReturnPolicy::StateDependent
                                    | RobotsHandlerEventReturnPolicy::HostResult
                                    | RobotsHandlerEventReturnPolicy::Delegate => {
                                        // StateMarker is consumed internally by NativeCutsceneRuntimeState.
                                        // Other stateful families remain fail-closed until their exact host
                                        // adapter is recovered.
                                        RobotsScriptNativeEventResult::Hold
                                    }
                                }
                            });
                            let creator_data0 =
                                trigger.data.first().copied().flatten().unwrap_or_default();
                            for effect in effects.iter().copied() {
                                match effect {
                                    RobotsCutsceneEffect::SetPropertiesCutscene {
                                        cutscene_enabled,
                                        message_enabled,
                                        music_volume_percent,
                                        ..
                                    } => {
                                        state.apply_change_level_route_toggle(cutscene_enabled);
                                        state.apply_message_toggle(message_enabled, creator_data0);
                                        if music_volume_percent.is_some() {
                                            host_music_volume_percent = music_volume_percent;
                                        }
                                    }
                                    RobotsCutsceneEffect::SwapCharacter { mode } => {
                                        state.apply_swap_character_mode(mode);
                                    }
                                    _ => {}
                                }
                            }
                            host_effects.extend(effects.iter().copied());
                            state.pending_effects = effects;
                            state.active_script_time_seconds =
                                Some(script.time_at_frame(state.script_scheduler.frame));
                        } else {
                            state.active_script_time_seconds = None;
                        }
                    }
                }
                if restore_player_health && self.native_sweeper_player_health_known {
                    self.native_sweeper_player_current_health =
                        self.native_sweeper_player_max_health;
                }
                for effect in host_effects {
                    self.native_cutscene_host_runtime
                        .apply_script_effect(effect);
                    match effect {
                        RobotsCutsceneEffect::ShowMessage {
                            message_uid: Some(message_uid),
                            parameter_08: Some(parameter_08),
                            use_script_length: Some(use_script_length),
                            parameter_10: Some(parameter_10),
                            parameter_14: Some(parameter_14),
                            length: Some(command_length),
                            ..
                        } => {
                            let Some(script) = selected_script.as_ref() else {
                                continue;
                            };
                            let Some(message) = map.text_groups.message_definition(message_uid)
                            else {
                                continue;
                            };
                            let native_frame = (wall_time * 60.0) as f32;
                            if let Some(record) = build_cutscene_show_message_record(
                                message,
                                native_frame,
                                script.framerate,
                                command_length,
                                use_script_length,
                                parameter_08,
                                parameter_10,
                                parameter_14,
                            ) {
                                self.native_message_presentation.enqueue(record);
                            }
                        }
                        RobotsCutsceneEffect::MessageRelaySpecific {
                            event_mask: Some(event_mask),
                            link_mask: Some(link_mask),
                        } => self.dispatch_native_link_mask_event(
                            map,
                            trigger_index,
                            event_mask,
                            link_mask,
                            wall_time,
                        ),
                        RobotsCutsceneEffect::ShakeCamera {
                            magnitude: Some(magnitude),
                            duration_updates: Some(duration_updates),
                            primary_channel: Some(primary_channel),
                            player_channel: Some(player_channel),
                            ..
                        } => {
                            self.native_camera_shake.request(
                                magnitude,
                                duration_updates,
                                primary_channel,
                                player_channel,
                            );
                        }
                        RobotsCutsceneEffect::SetPropertiesPlayer { mode } => {
                            apply_native_cutscene_player_property(
                                &mut self.native_player_item_state,
                                mode,
                            );
                        }
                        RobotsCutsceneEffect::PerformActionCutscene {
                            action: Some(action),
                            ..
                        } => {
                            let player_state = self.native_player_focus_runtime.player_state;
                            apply_native_cutscene_player_action(
                                &mut self.native_player_action_runtime,
                                player_state,
                                &self.native_player_item_state,
                                action,
                            );
                        }
                        _ => {}
                    }
                }
                if let Some(percent) = host_music_volume_percent {
                    self.native_cutscene_host_runtime.apply_finalize_effect(
                        RobotsCutsceneFinalizeEffect::SetMusicVolumePercent { percent },
                    );
                }
                continue;
            }
            if robots_character_runtime_type(trigger.ttype).is_some() {
                let Some(state) = self.native_ai_trigger_lifecycle.get_mut(&key) else {
                    continue;
                };
                let zone_visual_active = map
                    .native_zone_index(trigger.position)
                    .and_then(|zone_index| self.native_zone_runtime.get(zone_index))
                    .is_some_and(|zone| zone.activated && zone.visual_depth > 0);
                state.advance_xitem(zone_visual_active);
                continue;
            }
            if trigger.ttype == ROBOTS_BOSS_SEWER_CANON_TYPE
                && trigger.data.first().copied().flatten().unwrap_or_default() >= 1
            {
                let Some(state) = self.native_camera_bit0_trigger_lifecycle.get_mut(&key) else {
                    continue;
                };
                let zone_visual_active = map
                    .native_zone_index(trigger.position)
                    .and_then(|zone_index| self.native_zone_runtime.get(zone_index))
                    .is_some_and(|zone| zone.activated && zone.visual_depth > 0);
                state.advance_xitem_before_camera(zone_visual_active);
                continue;
            }
            if trigger.ttype != ROBOTS_SCRIPT_TRIGGER_TYPE
                || trigger.data.first().copied().flatten().unwrap_or_default()
                    & ROBOTS_SCRIPT_DYNAMIC_RAYCAST_FLAG
                    == 0
            {
                continue;
            }
            let Some(state) = self.native_script_trigger_lifecycle.get_mut(&key) else {
                continue;
            };
            let zone_visual_active = map
                .native_zone_index(trigger.position)
                .and_then(|zone_index| self.native_zone_runtime.get(zone_index))
                .is_some_and(|zone| zone.activated && zone.visual_depth > 0);
            state.advance_xitem(zone_visual_active);
        }
        Some(transporter_steps)
    }

    fn native_trigger_manager_order(map: &ProcessedMap) -> Vec<usize> {
        const GLOBAL_GROUP_FLAGS: u32 = 0x0800_0006;
        let mut groups = vec![(-1isize, Vec::<usize>::new())];
        for (trigger_index, trigger) in map.triggers.iter().enumerate() {
            if trigger.game_flags & GLOBAL_GROUP_FLAGS != 0 {
                groups[0].1.push(trigger_index);
                continue;
            }
            let zone = map
                .native_zone_index(trigger.position)
                .map(|zone| zone as isize)
                .unwrap_or(-1);
            if let Some((_, members)) = groups
                .iter_mut()
                .find(|(group_zone, _)| *group_zone == zone)
            {
                members.push(trigger_index);
            } else {
                groups.push((zone, vec![trigger_index]));
            }
        }
        groups
            .into_iter()
            .flat_map(|(_, members)| members)
            .collect()
    }

    fn dispatch_native_group_broadcast(
        &mut self,
        map: &ProcessedMap,
        trigger_index: usize,
        event_mask: u32,
        wall_time: f64,
    ) {
        let Some(trigger) = map.triggers.get(trigger_index) else {
            return;
        };
        let targets = trigger
            .links
            .iter()
            .filter_map(|link| map_trigger_by_link(map, *link).map(|(index, _)| index))
            .collect::<Vec<_>>();
        for target_index in targets {
            self.dispatch_runtime_event(map, target_index, event_mask, wall_time);
        }
    }

    /// Native Generic Handler `MessageRelaySpecific 0x1600001A` routes one
    /// event only to selected creator-link slots through `0x0044C420`. The mask
    /// is truncated to eight bits before the link walk.
    fn dispatch_native_link_mask_event(
        &mut self,
        map: &ProcessedMap,
        trigger_index: usize,
        event_mask: u32,
        link_mask: u8,
        wall_time: f64,
    ) {
        let Some(trigger) = map.triggers.get(trigger_index) else {
            return;
        };
        let targets = trigger
            .links
            .iter()
            .take(8)
            .enumerate()
            .filter_map(|(slot, link)| {
                (link_mask & (1u8 << slot) != 0)
                    .then(|| map_trigger_by_link(map, *link).map(|(index, _)| index))
                    .flatten()
            })
            .collect::<Vec<_>>();
        for target_index in targets {
            self.dispatch_runtime_event_from(
                map,
                target_index,
                event_mask,
                wall_time,
                Some(trigger_index),
            );
        }
    }

    fn initialize_native_groups(&mut self, map: &ProcessedMap, wall_time: f64) {
        for trigger_index in 0..map.triggers.len() {
            let Some(trigger) = map.triggers.get(trigger_index) else {
                continue;
            };
            if trigger.ttype != ROBOTS_GROUP_TYPE {
                continue;
            }
            // Shipped corpus uses only simple Group mode: data[7]&1 == 0.
            if trigger.data.get(7).copied().flatten().unwrap_or_default() & 1 != 0 {
                continue;
            }
            let key = Self::runtime_event_key(map.hashcode, trigger_index);
            if self
                .native_trigger_graph
                .get(&key)
                .is_some_and(|state| state.initialized)
            {
                continue;
            }
            let active = trigger.data.first().copied().flatten().unwrap_or_default() == 1;
            let state = self.native_trigger_graph.entry(key).or_default();
            state.active = active;
            state.initialized = true;

            self.dispatch_native_group_broadcast(map, trigger_index, 0x10000, wall_time);
            self.dispatch_native_group_broadcast(
                map,
                trigger_index,
                if active {
                    ROBOTS_EVENT_ACTIVATE
                } else {
                    ROBOTS_EVENT_DEACTIVATE
                },
                wall_time,
            );
            self.dispatch_native_group_broadcast(
                map,
                trigger_index,
                if trigger.game_flags & 1 != 0 { 0xA } else { 5 },
                wall_time,
            );
        }
    }

    fn dispatch_native_pattern_mask(
        &mut self,
        map: &ProcessedMap,
        trigger_index: usize,
        mask: u8,
        wall_time: f64,
    ) {
        let Some(trigger) = map.triggers.get(trigger_index) else {
            return;
        };
        let outputs = trigger
            .links
            .iter()
            .enumerate()
            .filter_map(|(slot, link)| {
                map_trigger_by_link(map, *link).map(|(target_index, _)| {
                    let event = if mask & (1u8 << slot) != 0 {
                        ROBOTS_EVENT_ACTIVATE
                    } else {
                        ROBOTS_EVENT_DEACTIVATE
                    };
                    (target_index, event)
                })
            })
            .collect::<Vec<_>>();
        for (target_index, event) in outputs {
            self.dispatch_runtime_event(map, target_index, event, wall_time);
        }
    }

    fn initialize_native_patterns(&mut self, map: &ProcessedMap, wall_time: f64) {
        for (trigger_index, trigger) in map.triggers.iter().enumerate() {
            if trigger.ttype != ROBOTS_PATTERN_TYPE {
                continue;
            }
            let key = Self::runtime_event_key(map.hashcode, trigger_index);
            if self
                .native_trigger_graph
                .get(&key)
                .is_some_and(|state| state.initialized)
            {
                continue;
            }
            let pattern_uid = trigger.data.get(1).copied().flatten().unwrap_or_default();
            let Some(group) = map.pattern_groups.get(&pattern_uid) else {
                continue;
            };
            let active = trigger.data.first().copied().flatten().unwrap_or_default() == 1;
            let initial_mask = {
                let state = self.native_trigger_graph.entry(key).or_default();
                state.initialize_pattern(active, &group.masks);
                state.pattern_mask
            };

            // Native XTrigger_Pattern create order at 0x0048E040 differs from Group:
            // initialise each target, send the 5/0xA service event, then apply active mask.
            self.dispatch_native_group_broadcast(map, trigger_index, 0x10000, wall_time);
            self.dispatch_native_group_broadcast(
                map,
                trigger_index,
                if trigger.game_flags & 1 != 0 { 0xA } else { 5 },
                wall_time,
            );
            if active {
                self.dispatch_native_pattern_mask(map, trigger_index, initial_mask, wall_time);
            } else {
                self.dispatch_native_group_broadcast(
                    map,
                    trigger_index,
                    ROBOTS_EVENT_DEACTIVATE,
                    wall_time,
                );
            }
        }
    }

    fn advance_native_lightweight_trigger_lifecycle_for_index(
        &mut self,
        map: &ProcessedMap,
        trigger_index: usize,
        trigger: &ProcessedTrigger,
        player_position: Option<Vec3>,
        creates_this_update: &mut u32,
    ) {
        if !matches!(
            trigger.ttype,
            ROBOTS_ACTIVATION_PAD_TYPE | ROBOTS_SLIDE_UNDER_TYPE | ROBOTS_ALERT_ICON_TYPE
        ) {
            return;
        }
        let key = Self::runtime_event_key(map.hashcode, trigger_index);
        if self.native_trigger_disabled(map.hashcode, trigger_index, trigger) {
            if trigger.game_flags & 0x1000_0000 != 0 {
                self.native_lightweight_trigger_lifecycle
                    .entry(key)
                    .or_default()
                    .cleanup_zone_failed();
            }
            return;
        }
        if !self.native_script_trigger_lifecycle_valid
            || self.native_zone_runtime_map != Some(map.hashcode)
            || !self.apply_native_camera_viewport
            || !self.native_camera_viewpoint_valid
        {
            return;
        }
        let Some(player_position) = player_position else {
            return;
        };
        let Some(zone_index) = map.native_zone_index(trigger.position) else {
            self.native_lightweight_trigger_lifecycle
                .entry(key)
                .or_default()
                .cleanup_zone_failed();
            return;
        };
        let Some(zone_state) = self.native_zone_runtime.get(zone_index).copied() else {
            return;
        };
        // Native class create-item slots are 0x00483370 (SlideUnder) and
        // 0x00483740 (AlertIcon); ActivationPad inherits Interact +0x24=0x004899A0
        // and cleanup +0x28=0x0047D840. This reducer owns only their proved
        // creator/XItem lifetime; class-specific Player reactions remain separate
        // gameplay behavior rather than being fabricated here.
        let state = self
            .native_lightweight_trigger_lifecycle
            .entry(key)
            .or_default();
        if !zone_state.activated || zone_state.visual_depth <= 0 {
            state.cleanup_zone_failed();
            return;
        }
        let distance_squared = trigger.position.distance_squared(player_position);
        let Some(event) = runtime_trigger_distance_event(distance_squared, trigger.game_flags)
        else {
            return;
        };
        let action = runtime_common_trigger_lifecycle_action(event, true, trigger.game_flags);
        let proximity_factor = runtime_trigger_normal_proximity_factor(distance_squared);
        state.advance_trigger_manager(
            action,
            trigger.game_flags,
            proximity_factor,
            creates_this_update,
        );
    }

    fn advance_native_lightweight_trigger_xitems(&mut self, map: &ProcessedMap) {
        for (trigger_index, trigger) in map.triggers.iter().enumerate() {
            let key = Self::runtime_event_key(map.hashcode, trigger_index);
            let zone_visual_active = map
                .native_zone_index(trigger.position)
                .and_then(|zone_index| self.native_zone_runtime.get(zone_index))
                .is_some_and(|zone| zone.activated && zone.visual_depth > 0);
            if is_robots_pickup_serialized_type(trigger.ttype) {
                if let Some(state) = self.native_pickup_trigger_lifecycle.get_mut(&key) {
                    state.advance_xitem(zone_visual_active);
                }
                continue;
            }
            if trigger.ttype == ROBOTS_FLUID_TYPE {
                if let Some(state) = self.native_fluid_trigger_lifecycle.get_mut(&key) {
                    state.advance_xitem(zone_visual_active);
                }
                continue;
            }
            if !matches!(
                trigger.ttype,
                ROBOTS_ACTIVATION_PAD_TYPE | ROBOTS_SLIDE_UNDER_TYPE | ROBOTS_ALERT_ICON_TYPE
            ) {
                continue;
            }
            if let Some(state) = self.native_lightweight_trigger_lifecycle.get_mut(&key) {
                state.advance_xitem(zone_visual_active);
            }
        }
    }

    fn flush_native_message_presentation_effects(&mut self) {
        while let Some(effect) = self.native_message_presentation.take_effect() {
            match effect {
                RobotsMessagePresentationEffect::StartVoice { sound_uid, volume } => {
                    let Some(bank_uid) =
                        crate::sound_native::robots_sound_details_bank_uid(sound_uid)
                    else {
                        continue;
                    };
                    let mut spec = SoundVoiceSpec::one_shot(bank_uid);
                    spec.volume = volume;
                    self.sound_preview
                        .lock()
                        .request_voice(SoundVoiceKey::Message, spec);
                }
                RobotsMessagePresentationEffect::StopVoice => {
                    self.sound_preview
                        .lock()
                        .stop_voice(SoundVoiceKey::Message, 0.0);
                }
            }
        }
    }

    fn advance_native_message_presentation_fixed(&mut self, wall_time: f64) {
        if self.native_message_presentation.pending_destroy {
            self.native_message_presentation.complete_deferred_destroy();
        } else {
            let native_frame = (wall_time * 60.0) as f32;
            if self
                .native_message_presentation
                .active_is_timed_out(native_frame)
            {
                self.native_message_presentation.complete_active();
                if self.native_message_presentation.queue.is_empty() {
                    self.native_message_presentation.request_destroy_if_idle();
                }
            } else if self.native_message_presentation.active.is_none() {
                self.native_message_presentation.dequeue_front_into_active();
            }
        }
        self.flush_native_message_presentation_effects();
    }

    fn advance_native_trigger_graph_fixed(
        &mut self,
        map: &ProcessedMap,
        wall_time: f64,
        player_position: Option<Vec3>,
        creates_this_update: &mut u32,
    ) -> bool {
        self.initialize_native_groups(map, wall_time);
        self.initialize_native_patterns(map, wall_time);
        let mut shared_rng_order_known = true;
        for trigger_index in Self::native_trigger_manager_order(map) {
            let Some(trigger) = map.triggers.get(trigger_index) else {
                continue;
            };
            self.advance_native_lightweight_trigger_lifecycle_for_index(
                map,
                trigger_index,
                trigger,
                player_position,
                creates_this_update,
            );
            let key = Self::runtime_event_key(map.hashcode, trigger_index);
            let pickup_trigger = is_robots_pickup_serialized_type(trigger.ttype);
            if self.native_trigger_disabled(map.hashcode, trigger_index, trigger) {
                if trigger.game_flags & 0x1000_0000 != 0 {
                    if trigger.ttype == ROBOTS_SWEEPER_CONTROLLER_TYPE {
                        self.native_sweeper_boss_controller_lifecycle
                            .entry(key)
                            .or_default()
                            .cleanup_zone_failed();
                    } else if trigger.ttype == ROBOTS_FLUID_TYPE {
                        self.native_fluid_trigger_lifecycle
                            .entry(key)
                            .or_default()
                            .cleanup_zone_failed();
                    } else if pickup_trigger {
                        self.native_pickup_trigger_lifecycle
                            .entry(key)
                            .or_default()
                            .cleanup_zone_failed();
                        self.native_pickup_xitems.remove(&key);
                    }
                }
                continue;
            }

            if trigger.ttype == ROBOTS_SWEEPER_CONTROLLER_TYPE
                || trigger.ttype == ROBOTS_FLUID_TYPE
                || pickup_trigger
            {
                let Some(player_position) = player_position else {
                    continue;
                };
                let Some(zone_index) = map.native_zone_index(trigger.position) else {
                    if trigger.ttype == ROBOTS_SWEEPER_CONTROLLER_TYPE {
                        self.native_sweeper_boss_controller_lifecycle
                            .entry(key)
                            .or_default()
                            .cleanup_zone_failed();
                    } else if trigger.ttype == ROBOTS_FLUID_TYPE {
                        self.native_fluid_trigger_lifecycle
                            .entry(key)
                            .or_default()
                            .cleanup_zone_failed();
                    } else {
                        self.native_pickup_trigger_lifecycle
                            .entry(key)
                            .or_default()
                            .cleanup_zone_failed();
                        self.native_pickup_xitems.remove(&key);
                    }
                    continue;
                };
                let Some(zone_state) = self.native_zone_runtime.get(zone_index).copied() else {
                    continue;
                };
                if !zone_state.activated || zone_state.visual_depth <= 0 {
                    if trigger.ttype == ROBOTS_SWEEPER_CONTROLLER_TYPE {
                        self.native_sweeper_boss_controller_lifecycle
                            .entry(key)
                            .or_default()
                            .cleanup_zone_failed();
                    } else if trigger.ttype == ROBOTS_FLUID_TYPE {
                        self.native_fluid_trigger_lifecycle
                            .entry(key)
                            .or_default()
                            .cleanup_zone_failed();
                    } else {
                        self.native_pickup_trigger_lifecycle
                            .entry(key)
                            .or_default()
                            .cleanup_zone_failed();
                        self.native_pickup_xitems.remove(&key);
                    }
                    continue;
                }

                let distance_squared = trigger.position.distance_squared(player_position);
                let Some(event) =
                    runtime_trigger_distance_event(distance_squared, trigger.game_flags)
                else {
                    continue;
                };
                let action =
                    runtime_common_trigger_lifecycle_action(event, true, trigger.game_flags);
                let proximity_factor = runtime_trigger_normal_proximity_factor(distance_squared);

                if trigger.ttype == ROBOTS_SWEEPER_CONTROLLER_TYPE {
                    self.native_sweeper_boss_controller_lifecycle
                        .entry(key)
                        .or_default()
                        .advance_trigger_manager(
                            action,
                            trigger.game_flags,
                            proximity_factor,
                            creates_this_update,
                        );
                    // Base +0x50 invokes controller +0x60 on every Event0 service even if the
                    // create budget prevented +0x24 from materializing the Ratchet XItem.
                    if matches!(event, RuntimeTriggerDistanceEvent::Event0)
                        && !self.advance_native_sweeper_boss_controller_service_fixed(map)
                    {
                        // Once an earlier process-global consumer cannot be reproduced, every
                        // later consumer in this frame would otherwise advance a stale anchor.
                        self.native_global_gameplay_rng.invalidate();
                        shared_rng_order_known = false;
                    }
                } else if trigger.ttype == ROBOTS_FLUID_TYPE {
                    let current_state = self
                        .native_fluid_trigger_lifecycle
                        .get(&key)
                        .copied()
                        .unwrap_or_default();
                    let initial_shared_rng_draws = map
                        .fluid_initial_shared_rng_draws
                        .get(trigger_index)
                        .copied()
                        .flatten();
                    let has_unowned_periodic_rng = trigger.data.get(6).copied().flatten().is_some()
                        || trigger.data.get(7).copied().flatten().is_some();
                    match preview_fluid_trigger_manager_shared_rng(
                        current_state,
                        *creates_this_update,
                        action,
                        trigger.game_flags,
                        proximity_factor,
                        self.native_global_gameplay_rng,
                        shared_rng_order_known,
                        initial_shared_rng_draws,
                        has_unowned_periodic_rng,
                    ) {
                        Some((next_state, next_creates, next_rng)) => {
                            self.native_fluid_trigger_lifecycle.insert(key, next_state);
                            *creates_this_update = next_creates;
                            self.native_global_gameplay_rng = next_rng;
                        }
                        None => {
                            self.native_global_gameplay_rng.invalidate();
                            shared_rng_order_known = false;
                        }
                    }
                } else {
                    let current_lifecycle = self
                        .native_pickup_trigger_lifecycle
                        .get(&key)
                        .copied()
                        .unwrap_or_default();
                    let current_pickup = self
                        .native_trigger_graph
                        .get(&key)
                        .map(|state| state.pickup)
                        .unwrap_or_default();
                    // 0x0048A080 uses the fixed 0x4800000E..1D definition table.
                    // Those sixteen gate items are executable-hardcoded Player definitions;
                    // the live Player-item container still supplies current/already-owned state.
                    let player_item_preflight_result =
                        robots_pickup_special_player_item_uid(trigger.ttype, &trigger.data).map(
                            |uid| {
                                self.native_player_item_state
                                    .preflight_add_result(uid, 1, true)
                            },
                        );
                    match preview_pickup_trigger_manager_shared_rng(
                        current_lifecycle,
                        current_pickup,
                        *creates_this_update,
                        action,
                        trigger.game_flags,
                        proximity_factor,
                        trigger.ttype,
                        &trigger.data,
                        player_item_preflight_result,
                        self.native_global_gameplay_rng,
                        shared_rng_order_known,
                    ) {
                        Some(next) => {
                            self.native_pickup_trigger_lifecycle
                                .insert(key, next.lifecycle);
                            self.native_trigger_graph.entry(key).or_default().pickup = next.pickup;
                            *creates_this_update = next.creates_this_update;
                            self.native_global_gameplay_rng = next.rng;
                            if let Some(spawned) = next.spawned {
                                let handler = RobotsPickupHandlerRuntimeState::new(
                                    spawned.layout.handler_datum_uid,
                                    spawned.layout.quantity,
                                    &mut self.native_pickup_global_scheduler,
                                );
                                self.native_pickup_xitems.insert(
                                    key,
                                    NativePickupXItemRuntime {
                                        spawn: spawned,
                                        handler,
                                    },
                                );
                            }
                            if !next.lifecycle.xitem_exists {
                                self.native_pickup_xitems.remove(&key);
                            }
                        }
                        None => {
                            self.native_global_gameplay_rng.invalidate();
                            shared_rng_order_known = false;
                        }
                    }
                }
                continue;
            }

            let should_fire_before_tick = self
                .native_trigger_graph
                .get(&key)
                .is_some_and(|state| state.deferred_fire);
            if should_fire_before_tick {
                // Native clears bit20 after its output loop. Clearing it here is
                // equivalent for shipped Robots because Counter/Timer/Relay have
                // zero self-links in the complete corpus; downstream routers can
                // still re-arm an already-visited source for the next pass.
                if let Some(state) = self.native_trigger_graph.get_mut(&key) {
                    state.deferred_fire = false;
                }
            }
            if trigger.ttype == ROBOTS_TIMER_TYPE {
                let duration = trigger
                    .data
                    .first()
                    .copied()
                    .flatten()
                    .map(f32::from_bits)
                    .unwrap_or_default();
                self.native_trigger_graph
                    .entry(key)
                    .or_default()
                    .advance_timer_fixed(duration);
            }

            if trigger.ttype == ROBOTS_PATTERN_TYPE {
                let pattern_uid = trigger.data.get(1).copied().flatten().unwrap_or_default();
                if let Some(group) = map.pattern_groups.get(&pattern_uid) {
                    let override_interval = trigger
                        .data
                        .get(2)
                        .copied()
                        .flatten()
                        .map(f32::from_bits)
                        .unwrap_or_default();
                    let interval = if override_interval > 0.0 {
                        override_interval
                    } else {
                        group.default_interval
                    };
                    let next_mask = self
                        .native_trigger_graph
                        .entry(key)
                        .or_default()
                        .advance_pattern_fixed(interval, &group.masks);
                    if let Some(mask) = next_mask {
                        self.dispatch_native_pattern_mask(map, trigger_index, mask, wall_time);
                    }
                }
            }

            if trigger.ttype == ROBOTS_MISSION_TYPE {
                let mission_uid = trigger.data.first().copied().flatten().unwrap_or_default();
                let base_timer = trigger
                    .data
                    .get(1)
                    .copied()
                    .flatten()
                    .map(f32::from_bits)
                    .unwrap_or_default();
                let mission_flags = trigger.data.get(4).copied().flatten().unwrap_or_default();
                let has_status_manager = !matches!(mission_uid, 0 | 0x5400_0000);
                let objective_condition_met = map
                    .mission_definitions
                    .iter()
                    .copied()
                    .find(|mission| mission.mission_uid == mission_uid)
                    .and_then(|mission| {
                        resolve_mission_objective_progress(
                            mission,
                            &map.inventory_definitions,
                            &self.native_script_gameplay_state.inventory,
                        )
                    })
                    .is_some_and(|progress| progress.condition_met());
                let tick = self
                    .native_trigger_graph
                    .entry(key)
                    .or_default()
                    .advance_mission_fixed(
                        base_timer,
                        mission_flags,
                        None,  // Maps does not own native modal HUD modes 1/2/3.
                        false, // Player Handler state +0x6DE is not fabricated.
                        objective_condition_met,
                        has_status_manager,
                    );
                match tick.dispatch.owner_action {
                    NativeMissionOwnerAction::Keep => {}
                    NativeMissionOwnerAction::SetSelf => self.native_mission_owner = Some(key),
                    NativeMissionOwnerAction::Clear => self.native_mission_owner = None,
                }
                if let Some(status) = tick.dispatch.status_update {
                    if has_status_manager {
                        self.native_script_gameplay_state
                            .missions
                            .set_status(mission_uid, status);
                    }
                }
                if tick.clock_activate {
                    if let Some((clock_index, _)) = trigger.links.iter().find_map(|link| {
                        let (index, linked) = map_trigger_by_link(map, *link)?;
                        (linked.ttype == ROBOTS_CLOCK_TYPE).then_some((index, linked))
                    }) {
                        self.dispatch_runtime_event(map, clock_index, 0x100, wall_time);
                    }
                }
                if let Some(output_slot) = tick.dispatch.output_slot {
                    if let Some((target_index, _)) = trigger
                        .links
                        .get(output_slot)
                        .copied()
                        .and_then(|link| map_trigger_by_link(map, link))
                    {
                        self.dispatch_runtime_event(map, target_index, 0x101, wall_time);
                    }
                }
            }

            let should_fire = self
                .native_trigger_graph
                .get_mut(&key)
                .is_some_and(|_| should_fire_before_tick);
            if !should_fire {
                continue;
            }

            let outputs = (0..8usize)
                .filter_map(|slot| {
                    let link = trigger.links.get(slot).copied()?;
                    let (target_index, _) = map_trigger_by_link(map, link)?;
                    Some((
                        target_index,
                        runtime_trigger_deferred_link_event(trigger, slot),
                    ))
                })
                .collect::<Vec<_>>();
            for (target_index, event_mask) in outputs {
                self.dispatch_runtime_event(map, target_index, event_mask, wall_time);
            }
        }
        shared_rng_order_known
    }

    fn advance_native_camera_schedule(&mut self, map: &ProcessedMap, wall_time: f64) {
        let delta_seconds = self
            .native_camera_fixed_last_time
            .replace(wall_time)
            .map(|previous| (wall_time - previous).max(0.0))
            .unwrap_or_default();
        if !self.animate_runtime_paths {
            self.native_camera_fixed_accumulator = 0.0;
            return;
        }

        for (sequence_index, trigger) in map.triggers.iter().enumerate() {
            if trigger.ttype != ROBOTS_CAMERA_SEQUENCE_TYPE {
                continue;
            }
            let key = Self::runtime_event_key(map.hashcode, sequence_index);
            self.native_camera_sequences
                .entry(key)
                .or_insert_with(|| NativeCameraSequenceRuntime::idle(sequence_index));
        }

        let player_pose = self.native_gameplay_player_pose(map);
        let player_position = player_pose.map(|(position, _)| position);
        self.native_camera_fixed_accumulator += delta_seconds;
        let steps = (self.native_camera_fixed_accumulator / ROBOTS_NATIVE_FIXED_SECONDS)
            .floor()
            .min(ROBOTS_NATIVE_MAX_FIXED_STEPS_PER_FRAME as f64) as usize;
        self.native_camera_fixed_accumulator -= steps as f64 * ROBOTS_NATIVE_FIXED_SECONDS;

        for _ in 0..steps {
            // Native order: GameWnd fade first, then XTriggerManager/Camera_Sequence
            // (0x00444CE4 -> 0x0044BE10), then the XItem Handler/Camera phase
            // (0x00444D56 -> manager +0x0C -> Camera +0x34).
            self.native_camera_fade_transition.fixed_update_fade();
            self.native_camera_shake.fixed_update();
            let fade_state = self.native_camera_fade_transition.fade.state;
            let keys = self
                .native_camera_sequences
                .keys()
                .copied()
                .collect::<Vec<_>>();
            let mut emissions = Vec::new();
            let mut auto_started = Vec::new();
            let mut completed = Vec::new();
            for key in keys {
                let trigger_index = key as u32 as usize;
                let Some(trigger) = map.triggers.get(trigger_index) else {
                    continue;
                };
                if self.native_trigger_disabled(map.hashcode, trigger_index, trigger) {
                    continue;
                }
                let Some(runtime) = self.native_camera_sequences.get_mut(&key) else {
                    continue;
                };
                let was_active = runtime.state != 0;
                let emission = if !was_active {
                    player_position
                        .and_then(|position| runtime.auto_start_if_player_near(map, position))
                } else {
                    runtime.fixed_update(map, fade_state)
                };
                if let Some(emission) = emission {
                    if !was_active {
                        auto_started.push(key);
                    }
                    emissions.push(emission);
                }
                if was_active && runtime.state == 0 {
                    completed.push(key);
                }
            }

            for key in auto_started {
                self.runtime_event_states.entry(key).or_default().active = true;
            }
            for emission in emissions {
                self.dispatch_runtime_event(
                    map,
                    emission.target_trigger_index,
                    ROBOTS_EVENT_ACTIVATE,
                    wall_time,
                );
            }
            let mut creates_this_update = 0u32;
            let sweeper_runtime_before_trigger_manager = self.native_sweeper_boss_runtime.clone();
            let shared_rng_order_known = self.advance_native_trigger_graph_fixed(
                map,
                wall_time,
                player_position,
                &mut creates_this_update,
            );
            self.advance_native_message_presentation_fixed(wall_time);
            if !shared_rng_order_known {
                // A later TriggerManager RNG boundary cannot leave the boss after its controller
                // service while the same native frame's XItem slice is still unrepresented.
                self.native_sweeper_boss_runtime = sweeper_runtime_before_trigger_manager;
            }
            let sweeper_transporter_snapshot =
                self.native_sweeper_boss_bindings.as_ref().map(|bindings| {
                    let key =
                        Self::runtime_event_key(map.hashcode, bindings.transporter_trigger_index);
                    (
                        key,
                        self.native_monster_transporter_lifecycle.get(&key).cloned(),
                        self.native_monster_transporters.get(&key).cloned(),
                        self.native_common_trigger_events.get(&key).cloned(),
                        self.runtime_event_states.get(&key).cloned(),
                    )
                });
            let sweeper_controller_xitem_exists = self
                .native_sweeper_boss_bindings
                .as_ref()
                .and_then(|bindings| {
                    let key =
                        Self::runtime_event_key(map.hashcode, bindings.controller_trigger_index);
                    self.native_sweeper_boss_controller_lifecycle.get(&key)
                })
                .is_some_and(|state| state.xitem_exists);
            let transporter_steps = self.advance_native_script_trigger_lifecycle_fixed(
                map,
                wall_time,
                player_position,
                &mut creates_this_update,
            );
            if let Some((player_position, player_yaw)) = player_pose {
                self.advance_native_npc_behavior_fixed(map, player_position);
                self.advance_native_player_focus_fixed(map, wall_time, player_position, player_yaw);
            }
            let sweeper_xitem_committed = if shared_rng_order_known {
                self.advance_native_sweeper_boss_xitem_fixed(
                    map,
                    player_position,
                    transporter_steps.as_ref(),
                )
            } else {
                !sweeper_controller_xitem_exists
            };
            if !sweeper_xitem_committed {
                if let Some((key, lifecycle, runtime, common_event, preview_state)) =
                    sweeper_transporter_snapshot
                {
                    match lifecycle {
                        Some(state) => {
                            self.native_monster_transporter_lifecycle.insert(key, state);
                        }
                        None => {
                            self.native_monster_transporter_lifecycle.remove(&key);
                        }
                    }
                    match runtime {
                        Some(state) => {
                            self.native_monster_transporters.insert(key, state);
                        }
                        None => {
                            self.native_monster_transporters.remove(&key);
                        }
                    }
                    match common_event {
                        Some(state) => {
                            self.native_common_trigger_events.insert(key, state);
                        }
                        None => {
                            self.native_common_trigger_events.remove(&key);
                        }
                    }
                    match preview_state {
                        Some(state) => {
                            self.runtime_event_states.insert(key, state);
                        }
                        None => {
                            self.runtime_event_states.remove(&key);
                        }
                    }
                }
            }
            if shared_rng_order_known && sweeper_xitem_committed {
                if let Some(player_position) = player_position {
                    // Dynamic MalfBot/RollerBot are real XItems created inside the
                    // Sweeper slice. Run their common class host here, after factory
                    // bootstrap/PostRatchet ownership and before later XItem/Pickup
                    // consumers of the process-global gameplay RNG.
                    self.advance_native_dynamic_sweeper_ai_fixed(map, player_position, wall_time);
                }
            }
            self.advance_native_lightweight_trigger_xitems(map);
            self.advance_native_sweeper_boss_post_xitem_pickup_tail_fixed(map);
            self.native_camera_fade_transition.fixed_update_camera();

            for key in completed {
                if let Some(state) = self.runtime_event_states.get_mut(&key) {
                    state.active = false;
                }
            }
        }
    }

    pub(super) fn runtime_event_supported(map: &ProcessedMap, trigger: &ProcessedTrigger) -> bool {
        (robots_trigger_runtime_path_speed(trigger.ttype, &trigger.data).is_some()
            && map_trigger_runtime_path(map, trigger).is_some())
            || robots_object_audio_is_consumer(trigger.ttype)
            || matches!(
                trigger.ttype,
                0 | 1
                    | 20
                    | ROBOTS_COUNTER_TYPE
                    | ROBOTS_TIMER_TYPE
                    | ROBOTS_MESSAGE_RELAY_TYPE
                    | ROBOTS_GROUP_TYPE
                    | ROBOTS_PATTERN_TYPE
                    | ROBOTS_ACTIVATION_PAD_TYPE
                    | ROBOTS_WATCHBOT_TYPE
                    | ROBOTS_SHOP_TYPE
                    | ROBOTS_MISSION_TYPE
                    | ROBOTS_CLOCK_TYPE
            )
            || trigger.ttype == ROBOTS_CAMERA_VALUES_TYPE
            || trigger.ttype == ROBOTS_CAMERA_SEQUENCE_TYPE
    }

    pub(super) fn runtime_event_snapshots(
        &mut self,
        map: &ProcessedMap,
        wall_time: f64,
    ) -> Vec<Option<RuntimeEventPreviewSnapshot>> {
        if !self.native_runtime_event_gate {
            self.native_camera_fixed_last_time = Some(wall_time);
            self.native_camera_fixed_accumulator = 0.0;
            return vec![None; map.triggers.len()];
        }

        self.advance_native_camera_schedule(map, wall_time);

        let mut node_dispatches = Vec::new();
        for (index, trigger) in map.triggers.iter().enumerate() {
            if !Self::runtime_event_supported(map, trigger) {
                continue;
            }
            let state = self
                .runtime_event_states
                .entry(Self::runtime_event_key(map.hashcode, index))
                .or_default();
            let before = state.snapshot(trigger, self.runtime_path_playback_speed);
            if self.animate_runtime_paths {
                state.advance_runtime(map, trigger, wall_time, self.runtime_path_playback_speed);
            } else {
                state.hold(wall_time);
            }
            let after = state.snapshot(trigger, self.runtime_path_playback_speed);
            if before.active && after.active {
                node_dispatches.extend(
                    runtime_path_node_dispatches_between(
                        map,
                        trigger,
                        before.path_distance,
                        after.path_distance,
                    )
                    .into_iter()
                    .map(|dispatch| (index, dispatch)),
                );
            }
        }

        for (source_index, dispatch) in node_dispatches {
            let Some(source) = map.triggers.get(source_index) else {
                continue;
            };
            if let Some(state) = self
                .runtime_event_states
                .get_mut(&Self::runtime_event_key(map.hashcode, source_index))
            {
                state.record_node_dispatch(
                    dispatch.node_index,
                    match dispatch.event {
                        RuntimePathNodeEvent::DeactivateSelf => 4,
                        RuntimePathNodeEvent::DispatchLinked { .. } => 8,
                    },
                );
            }
            match dispatch.event {
                RuntimePathNodeEvent::DeactivateSelf => self.dispatch_runtime_event(
                    map,
                    source_index,
                    ROBOTS_EVENT_DEACTIVATE,
                    wall_time,
                ),
                RuntimePathNodeEvent::DispatchLinked {
                    event_mask,
                    link_mask,
                } => {
                    for link_slot in 0..8usize {
                        if link_mask & (1 << link_slot) == 0 {
                            continue;
                        }
                        let Some((target_index, _)) = source
                            .links
                            .get(link_slot)
                            .copied()
                            .and_then(|link| map_trigger_by_link(map, link))
                        else {
                            continue;
                        };
                        self.dispatch_runtime_event(map, target_index, event_mask, wall_time);
                    }
                }
            }
        }

        map.triggers
            .iter()
            .enumerate()
            .map(|(index, trigger)| {
                if !Self::runtime_event_supported(map, trigger) {
                    return None;
                }
                self.runtime_event_states
                    .get(&Self::runtime_event_key(map.hashcode, index))
                    .map(|state| state.snapshot(trigger, self.runtime_path_playback_speed))
            })
            .collect()
    }

    pub(super) fn runtime_event_snapshot(
        &mut self,
        map: &ProcessedMap,
        trigger_index: usize,
        wall_time: f64,
    ) -> Option<RuntimeEventPreviewSnapshot> {
        let trigger = map.triggers.get(trigger_index)?;
        if !self.native_runtime_event_gate || !Self::runtime_event_supported(map, trigger) {
            return None;
        }
        let state = self
            .runtime_event_states
            .entry(Self::runtime_event_key(map.hashcode, trigger_index))
            .or_default();
        if self.animate_runtime_paths {
            state.advance_runtime(map, trigger, wall_time, self.runtime_path_playback_speed);
        } else {
            state.hold(wall_time);
        }
        Some(state.snapshot(trigger, self.runtime_path_playback_speed))
    }

    pub(super) fn dispatch_runtime_event(
        &mut self,
        map: &ProcessedMap,
        trigger_index: usize,
        event_mask: u32,
        wall_time: f64,
    ) {
        self.dispatch_runtime_event_from(map, trigger_index, event_mask, wall_time, None);
    }

    fn dispatch_runtime_event_from(
        &mut self,
        map: &ProcessedMap,
        trigger_index: usize,
        event_mask: u32,
        wall_time: f64,
        sender_trigger_index: Option<usize>,
    ) {
        let Some(trigger) = map.triggers.get(trigger_index) else {
            return;
        };
        let concrete_supported = Self::runtime_event_supported(map, trigger);
        self.native_runtime_event_gate = true;
        let graph_key = Self::runtime_event_key(map.hashcode, trigger_index);
        let common_result = self
            .native_common_trigger_events
            .entry(graph_key)
            .or_default()
            .dispatch(trigger, event_mask);

        // Camera overrides common +0x20 at 0x004806B0: before setting the
        // disabled latch it calls Camera +0xF0(false), which is the same native
        // ownership-release ingress used by Camera event 0x200.
        if common_result.disable_hook && trigger.ttype == 1 {
            self.active_camera_trigger = active_camera_trigger_after_event(
                map,
                &mut self.native_camera_ownership,
                self.active_camera_trigger,
                trigger_index,
                trigger.ttype,
                ROBOTS_EVENT_DEACTIVATE,
            );
        }
        if common_result.forward_to_class {
            let serialized_data0 = trigger.data.first().copied().flatten().unwrap_or_default();
            match trigger.ttype {
                ROBOTS_SHOP_TYPE => {
                    let shop_uid = trigger.data.get(3).copied().flatten().unwrap_or_default();
                    let has_linked_npc =
                        Self::native_first_linked_trigger_index(map, trigger, ROBOTS_NPC_TYPE)
                            .is_some();
                    let linked_player_index =
                        Self::native_first_linked_trigger_index(map, trigger, 0);
                    let sender_is_cutscene = sender_trigger_index
                        .and_then(|index| map.triggers.get(index))
                        .is_some_and(|sender| sender.ttype == ROBOTS_CUTSCENE_TYPE);
                    let top_game_state = self
                        .native_cutscene_host_runtime
                        .game_state_stack
                        .last()
                        .copied()
                        .map(|state| state as i32);
                    let plan = plan_shop_trigger_event(
                        event_mask,
                        sender_is_cutscene,
                        top_game_state,
                        shop_uid,
                        has_linked_npc,
                    );
                    for effect in plan.effects {
                        match effect {
                            RobotsShopTriggerEffect::OpenHud {
                                shop_uid,
                                has_linked_npc,
                            } => {
                                let already_open =
                                    self.native_shop_lifecycle.active_shop_uid.is_some();
                                let group_exists =
                                    map.shop_database.as_ref().is_some_and(|database| {
                                        database
                                            .groups
                                            .iter()
                                            .any(|group| group.shop_uid == shop_uid)
                                    });
                                let open_succeeded = already_open || group_exists;
                                self.native_shop_lifecycle.set_open_result(
                                    shop_uid,
                                    has_linked_npc,
                                    open_succeeded,
                                );
                                let completion = complete_shop_open(
                                    open_succeeded,
                                    has_linked_npc,
                                    linked_player_index.is_some(),
                                );
                                if let Some(player_state) = completion.player_state {
                                    self.native_player_focus_runtime.player_state = player_state;
                                }
                                if let (Some(target_index), Some(linked_event)) =
                                    (linked_player_index, completion.dispatch_linked_player_event)
                                {
                                    self.dispatch_runtime_event_from(
                                        map,
                                        target_index,
                                        linked_event,
                                        wall_time,
                                        Some(trigger_index),
                                    );
                                }
                            }
                            RobotsShopTriggerEffect::CloseHud => {
                                self.native_shop_lifecycle.close_hud();
                            }
                        }
                    }
                }
                ROBOTS_DOOR_TYPE => self
                    .native_trigger_graph
                    .entry(graph_key)
                    .or_default()
                    .dispatch_door(serialized_data0, event_mask),
                ROBOTS_FIX_SWITCH_TYPE => {
                    let graph = self.native_trigger_graph.entry(graph_key).or_default();
                    let _ = apply_native_fix_switch_event_runtime(
                        trigger, graph, event_mask,
                        false, // Maps does not own the native FixSwitch Handler/XItem.
                    );
                }
                ROBOTS_DISPLAY_MESSAGE_TYPE => {
                    let duration_raw = trigger.data.get(1).copied().flatten().unwrap_or_default();
                    self.native_trigger_graph
                        .entry(graph_key)
                        .or_default()
                        .dispatch_display_message(
                            serialized_data0,
                            duration_raw,
                            event_mask,
                            None, // Maps does not create native modal HUD/GameWnd modes 1/2/3.
                        );
                }
                ROBOTS_CLOCK_TYPE => {
                    self.native_trigger_graph
                        .entry(graph_key)
                        .or_default()
                        .dispatch_clock(event_mask);
                }
                ROBOTS_MISSION_TYPE => {
                    let mission_uid = serialized_data0;
                    let base_timer = trigger
                        .data
                        .get(1)
                        .copied()
                        .flatten()
                        .map(f32::from_bits)
                        .unwrap_or_default();
                    let time_add = trigger
                        .data
                        .get(2)
                        .copied()
                        .flatten()
                        .map(f32::from_bits)
                        .unwrap_or_default();
                    let repeat_count =
                        trigger.data.get(3).copied().flatten().unwrap_or_default() as i32;
                    let mission_flags = trigger.data.get(4).copied().flatten().unwrap_or_default();
                    let mission_status = if matches!(mission_uid, 0 | 0x5400_0000) {
                        None
                    } else {
                        Some(
                            self.native_script_gameplay_state
                                .missions
                                .get(&mission_uid)
                                .copied()
                                .unwrap_or_default(),
                        )
                    };
                    let owner_relation = match self.native_mission_owner {
                        None => NativeMissionOwnerRelation::None,
                        Some(owner) if owner == graph_key => {
                            NativeMissionOwnerRelation::SelfTrigger
                        }
                        Some(_) => NativeMissionOwnerRelation::OtherTrigger,
                    };
                    let outcome = self
                        .native_trigger_graph
                        .entry(graph_key)
                        .or_default()
                        .dispatch_mission(
                            base_timer,
                            time_add,
                            repeat_count,
                            mission_flags,
                            event_mask,
                            mission_status,
                            owner_relation,
                        );
                    match outcome.owner_action {
                        NativeMissionOwnerAction::Keep => {}
                        NativeMissionOwnerAction::SetSelf => {
                            self.native_mission_owner = Some(graph_key);
                        }
                        NativeMissionOwnerAction::Clear => {
                            self.native_mission_owner = None;
                        }
                    }
                    if let Some(status) = outcome.status_update {
                        if !matches!(mission_uid, 0 | 0x5400_0000) {
                            self.native_script_gameplay_state
                                .missions
                                .set_status(mission_uid, status);
                        }
                    }
                    if let Some(output_slot) = outcome.output_slot {
                        if let Some((target_index, _)) = trigger
                            .links
                            .get(output_slot)
                            .copied()
                            .and_then(|link| map_trigger_by_link(map, link))
                        {
                            // XTrigger_Mission finalize 0x0048F070 forwards exact
                            // event 0x101 through link 6 (complete) or 7 (fail).
                            self.dispatch_runtime_event(map, target_index, 0x101, wall_time);
                        }
                    }
                }
                ROBOTS_BALL_TRACK_TYPE => {
                    let initial_created = trigger.game_flags & 0x1000_0000 != 0;
                    let graph = self.native_trigger_graph.entry(graph_key).or_default();
                    let _ = apply_native_ball_track_event_runtime(
                        trigger,
                        graph,
                        event_mask,
                        initial_created,
                    );
                }
                ROBOTS_LIGHT_TYPE | ROBOTS_TUTORIAL_TYPE => {
                    let state = self.native_trigger_graph.entry(graph_key).or_default();
                    if trigger.ttype == ROBOTS_LIGHT_TYPE {
                        state.dispatch_binary_active(event_mask);
                    } else {
                        state.dispatch_tutorial(event_mask);
                    }
                }
                ROBOTS_WATCHBOT_TYPE => {
                    if let Some(contract) =
                        RobotsWatchbotTriggerContract::from_serialized_data(&trigger.data)
                    {
                        let path_resolved = contract.path_uid == ROBOTS_WATCHBOT_NO_PATH_UID
                            || map
                                .paths
                                .iter()
                                .any(|path| path.hashcode == contract.path_uid);
                        self.native_trigger_graph
                            .entry(graph_key)
                            .or_default()
                            .watchbot
                            .dispatch_event(
                                graph_key,
                                contract,
                                &mut self.native_watchbot_owner_runtime,
                                event_mask,
                                path_resolved,
                            );
                    }
                }
                ROBOTS_ACTIVATION_PAD_TYPE => self
                    .native_trigger_graph
                    .entry(graph_key)
                    .or_default()
                    .dispatch_binary_active(event_mask),
                ROBOTS_SLIDE_UNDER_TYPE | ROBOTS_ALERT_ICON_TYPE => {
                    let initial_slot = if trigger.ttype == ROBOTS_SLIDE_UNDER_TYPE {
                        2
                    } else {
                        3
                    };
                    let initial_e4 = trigger
                        .data
                        .get(initial_slot)
                        .copied()
                        .flatten()
                        .unwrap_or_default() as u8;
                    let xitem_exists = self
                        .native_lightweight_trigger_lifecycle
                        .get(&graph_key)
                        .is_some_and(|state| state.xitem_exists);
                    self.native_trigger_graph
                        .entry(graph_key)
                        .or_default()
                        .dispatch_owned_xitem_binary(initial_e4, xitem_exists, event_mask);
                }
                ROBOTS_NPC_TYPE => self
                    .native_trigger_graph
                    .entry(graph_key)
                    .or_default()
                    .dispatch_npc(event_mask),
                ROBOTS_MONSTER_TRANSPORTER_TYPE if event_mask & 0x1000 != 0 => {
                    // 0x0047FE20 -> 0x0044BBB0 is a reload-to-serialized-record
                    // service reset. A live Transporter has bit28 set by common
                    // finalizer 0x0047D890, so reset first reaches class +0x28 =
                    // 0x0047D840(1) and destroys the owned XItem, then common
                    // reload 0x0044C8F0 calls class +0x10 = 0x0047F910, which
                    // clears +E4/+FC. Dropping both editor states reproduces that
                    // reset; the later TriggerManager pass recreates from the
                    // immutable serialized trigger/path and therefore parameter 0.
                    self.native_monster_transporter_lifecycle.remove(&graph_key);
                    self.native_monster_transporters.remove(&graph_key);
                }
                trigger_type if ROBOTS_INTERACT_TYPES.contains(&trigger_type) => self
                    .native_trigger_graph
                    .entry(graph_key)
                    .or_default()
                    .dispatch_binary_active(event_mask),
                _ => {}
            }
            if is_robots_pickup_serialized_type(trigger.ttype) {
                self.native_trigger_graph
                    .entry(graph_key)
                    .or_default()
                    .dispatch_pickup(event_mask);
            }
        }
        if !common_result.forward_to_class || !concrete_supported {
            return;
        }

        if trigger.ttype == 0 {
            if let Some(player_state) = runtime_player_trigger_state(trigger_index, trigger) {
                self.native_runtime_player_state = Some(player_state);
                // Player trigger creation calls persistent restore first and then
                // `0x004AFF80`; if HT_Upgrade_Watchbot is already restored this
                // creates the Player-owned WatchBot XItem at +0x550 here.
                self.service_native_watchbot_owner();
                let mut direction = player_state.rotation * Vec3::Z;
                if !direction.is_finite() || direction.length_squared() <= f32::EPSILON {
                    direction = Vec3::Z;
                }
                self.native_camera_player_preview
                    .set_pose_from_direction(player_state.position, direction);
                self.native_camera_player_preview_map = Some(map.hashcode);
                self.native_camera_player_preview_last_time = Some(wall_time);
            }
        }

        let mut group_broadcasts = Vec::new();
        match trigger.ttype {
            ROBOTS_COUNTER_TYPE => {
                let threshold = trigger.data.first().copied().flatten().unwrap_or_default();
                self.native_trigger_graph
                    .entry(graph_key)
                    .or_default()
                    .dispatch_counter(threshold, event_mask);
            }
            ROBOTS_TIMER_TYPE => {
                self.native_trigger_graph
                    .entry(graph_key)
                    .or_default()
                    .dispatch_timer(event_mask);
            }
            ROBOTS_MESSAGE_RELAY_TYPE if event_mask & ROBOTS_EVENT_ACTIVATE != 0 => {
                self.native_trigger_graph
                    .entry(graph_key)
                    .or_default()
                    .deferred_fire = true;
            }
            ROBOTS_GROUP_TYPE
                if trigger.data.get(7).copied().flatten().unwrap_or_default() & 1 == 0 =>
            {
                if event_mask & 1 != 0 {
                    group_broadcasts.push(5);
                }
                let state = self.native_trigger_graph.entry(graph_key).or_default();
                if event_mask & ROBOTS_EVENT_ACTIVATE != 0 && !state.active {
                    state.active = true;
                    group_broadcasts.push(ROBOTS_EVENT_ACTIVATE);
                }
                if event_mask & ROBOTS_EVENT_DEACTIVATE != 0 && state.active {
                    state.active = false;
                    group_broadcasts.push(ROBOTS_EVENT_DEACTIVATE);
                }
                for broadcast_event in group_broadcasts.drain(..) {
                    self.dispatch_native_group_broadcast(
                        map,
                        trigger_index,
                        broadcast_event,
                        wall_time,
                    );
                }
            }
            ROBOTS_PATTERN_TYPE => {
                if event_mask & 1 != 0 {
                    self.dispatch_native_group_broadcast(map, trigger_index, 5, wall_time);
                }
                let pattern_uid = trigger.data.get(1).copied().flatten().unwrap_or_default();
                if let Some(group) = map.pattern_groups.get(&pattern_uid) {
                    let initial_active =
                        trigger.data.first().copied().flatten().unwrap_or_default() == 1;
                    let mut activate_mask = None;
                    let mut deactivate = false;
                    {
                        let state = self.native_trigger_graph.entry(graph_key).or_default();
                        if !state.initialized {
                            state.initialize_pattern(initial_active, &group.masks);
                        }
                        if event_mask & ROBOTS_EVENT_ACTIVATE != 0 && !state.active {
                            state.active = true;
                            activate_mask = Some(state.pattern_mask);
                        }
                        if event_mask & ROBOTS_EVENT_DEACTIVATE != 0 && state.active {
                            state.active = false;
                            deactivate = true;
                        }
                    }
                    if let Some(mask) = activate_mask {
                        self.dispatch_native_pattern_mask(map, trigger_index, mask, wall_time);
                    }
                    if deactivate {
                        self.dispatch_native_group_broadcast(
                            map,
                            trigger_index,
                            ROBOTS_EVENT_DEACTIVATE,
                            wall_time,
                        );
                    }
                }
            }
            _ => {}
        }
        {
            let state = self
                .runtime_event_states
                .entry(Self::runtime_event_key(map.hashcode, trigger_index))
                .or_default();
            if self.animate_runtime_paths {
                state.advance_runtime(map, trigger, wall_time, self.runtime_path_playback_speed);
            } else {
                state.hold(wall_time);
            }
            state.dispatch(
                trigger,
                event_mask,
                wall_time,
                self.runtime_path_playback_speed,
            );
        }
        self.active_camera_trigger = active_camera_trigger_after_event(
            map,
            &mut self.native_camera_ownership,
            self.active_camera_trigger,
            trigger_index,
            trigger.ttype,
            event_mask,
        );
        if trigger.ttype == 1
            && event_mask & ROBOTS_EVENT_ACTIVATE != 0
            && self.active_camera_trigger == Some(trigger_index)
        {
            if let Some(plan) = robots_camera_controller_plan(map, trigger_index) {
                self.native_camera_fade_transition
                    .set_camera_mode(plan.controller_data3_raw);
            }
        }

        let sequence_key = Self::runtime_event_key(map.hashcode, trigger_index);
        let sequence_emission = if trigger.ttype == ROBOTS_CAMERA_SEQUENCE_TYPE
            && event_mask & ROBOTS_EVENT_ACTIVATE != 0
        {
            let runtime = self
                .native_camera_sequences
                .entry(sequence_key)
                .or_insert_with(|| NativeCameraSequenceRuntime::idle(trigger_index));
            if runtime.state == 0 {
                NativeCameraSequenceRuntime::start(map, trigger_index).map(|(started, emission)| {
                    *runtime = started;
                    emission
                })
            } else {
                None
            }
        } else {
            None
        };

        self.dispatch_object_audio_event(map, trigger_index, event_mask);
        if let Some(emission) = sequence_emission {
            self.dispatch_runtime_event(
                map,
                emission.target_trigger_index,
                ROBOTS_EVENT_ACTIVATE,
                wall_time,
            );
        }
    }

    pub(super) fn reset_runtime_event(
        &mut self,
        map: &ProcessedMap,
        trigger_index: usize,
        wall_time: f64,
    ) {
        let Some(trigger) = map.triggers.get(trigger_index) else {
            return;
        };
        let key = Self::runtime_event_key(map.hashcode, trigger_index);
        self.runtime_event_states
            .entry(key)
            .or_default()
            .reset(wall_time);
        self.native_camera_sequences.remove(&key);
        self.native_common_trigger_events.remove(&key);
        self.native_trigger_graph.remove(&key);
        self.native_cutscene_runtime.remove(&key);
        if trigger.ttype == ROBOTS_MISSION_TYPE {
            if self.native_mission_owner == Some(key) {
                self.native_mission_owner = None;
            }
            let mission_uid = trigger.data.first().copied().flatten().unwrap_or_default();
            if !matches!(mission_uid, 0 | 0x5400_0000) {
                self.native_script_gameplay_state
                    .missions
                    .remove(mission_uid);
            }
        }
        if self.active_camera_trigger == Some(trigger_index) {
            self.active_camera_trigger = None;
            self.native_camera_ownership = NativeCameraOwnershipRuntime::default();
            self.native_camera_fade_transition = NativeCameraFadeTransitionRuntime::default();
        }
        self.stop_object_audio_for_trigger(map.hashcode, trigger_index);
    }

    pub(super) fn reset_all_runtime_events(&mut self) {
        self.runtime_event_states.clear();
        self.active_camera_trigger = None;
        self.native_camera_ownership = NativeCameraOwnershipRuntime::default();
        self.native_camera_sequences.clear();
        self.native_camera_fade_transition = NativeCameraFadeTransitionRuntime::default();
        self.native_camera_shake = NativeCameraShakeRuntime::default();
        self.native_camera_fixed_last_time = None;
        self.native_camera_fixed_accumulator = 0.0;
        self.native_runtime_player_state = None;
        self.native_player_focus_runtime = NativePlayerFocusRuntimeState::default();
        self.native_game_control_runtime = RobotsGameControlRuntime::default();
        self.native_script_trigger_lifecycle.clear();
        self.native_ai_trigger_lifecycle.clear();
        self.native_ai_preview_live.clear();
        self.native_ai_preview_map = None;
        self.native_ai_last_hit_source_yaw.clear();
        self.native_ai_creator_runtime.clear();
        self.native_ai_deferred_destroy.clear();
        self.native_npc_behavior.clear();
        self.native_cutscene_runtime.clear();
        self.native_cutscene_host_runtime = RobotsCutsceneHostRuntimeState::default();
        self.native_message_presentation = NativeMessagePresentationState::default();
        self.native_lightweight_trigger_lifecycle.clear();
        self.native_fluid_trigger_lifecycle.clear();
        self.native_pickup_trigger_lifecycle.clear();
        self.native_pickup_xitems.clear();
        self.native_camera_bit0_trigger_lifecycle.clear();
        self.native_common_trigger_events.clear();
        self.native_trigger_graph.clear();
        self.native_mission_owner = None;
        self.native_script_gameplay_state.clear();
        // Runtime character owner poses are XItem/Physics state. A full runtime
        // reset must discard them so the next preview bootstraps from Trigger pose.
        self.runtime_character_bodies.clear();

        self.native_monster_transporter_lifecycle.clear();
        self.native_monster_transporters.clear();
        self.native_sweeper_boss_controller_lifecycle.clear();
        self.native_sweeper_boss_runtime_map = None;
        self.native_sweeper_boss_bindings = None;
        self.native_sweeper_boss_runtime = None;
        self.native_script_trigger_lifecycle_valid = false;
        self.runtime_motion_start_time = None;
        self.sound_preview
            .lock()
            .stop_group(SoundVoiceGroup::ObjectAudio, 0.03);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::maps::{ProcessedPath, ProcessedPathNode};
    use eurochef_edb::robots_ball_track::{
        RobotsBallPathEntry, RobotsBallTrackPair, RobotsBallTrackSchedule,
    };
    use eurochef_shared::robots_runtime::{
        ball_track::RobotsBallTrackSpawnParameterRequest,
        hit_reaction::ROBOTS_AI_HIT_CAPABILITY_FLAG,
        hit_reaction_boss::{
            ROBOTS_BOSS_SEWER_CANON_EVENT_MASK, ROBOTS_BOSS_SEWER_REQUIRED_HIT_FLAG,
            ROBOTS_BOSS_SEWER_STAGE_SCRIPT_UID,
        },
        hit_reaction_watchbot::{
            ROBOTS_WATCHBOT_COMPONENT_SHARED_IGNORE_STATE, ROBOTS_WATCHBOT_HIT_ANIMATION_MODE_UID,
        },
    };

    fn camera(path_hashcode: u32) -> ProcessedTrigger {
        let mut data = vec![None; 16];
        data[0] = Some(4);
        data[1] = Some(path_hashcode);
        ProcessedTrigger {
            file_offset: 0,
            link_ref: -1,
            type_index: 0,
            ttype: 1,
            tsubtype: None,
            debug: 0,
            game_flags: 0,
            trig_flags: 0,
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            scale: Vec3::ONE,
            data,
            links: vec![-1; 8],
            engine_options: eurochef_edb::map::EXGeoTriggerEngineOptions::default(),
            trigger_script: None,
            character_visual: None,
            incoming_links: vec![],
        }
    }

    fn trigger_with_links(ttype: u32, links: [i32; 8]) -> ProcessedTrigger {
        let mut trigger = camera(0);
        trigger.ttype = ttype;
        trigger.data = vec![None; 16];
        trigger.links = links.to_vec();
        trigger
    }

    #[test]
    fn slide_under_host_commit_reuses_player_focus_state_and_flags_link_deactivate() {
        let mut runtime = NativePlayerFocusRuntimeState::default();
        let key = 0x1234_5678_0000_0003;
        let entered = RobotsSlideUnderStep {
            focus: RobotsPlayerFocusDecision::SelectCandidate,
            slide_under: RobotsSlideUnderOwnerDecision::SelectCandidate,
        };
        assert!(!commit_native_slide_under_player_step(
            &mut runtime,
            key,
            entered
        ));
        assert_eq!(runtime.current_slide_under, Some(key));
        assert_eq!(
            runtime.current_owner,
            Some(NativePlayerFocusOwner {
                key,
                category: ROBOTS_XITEM_CATEGORY_SLIDE_UNDER,
            })
        );

        let exited = RobotsSlideUnderStep {
            focus: RobotsPlayerFocusDecision::ClearCurrent,
            slide_under: RobotsSlideUnderOwnerDecision::ClearCurrentAndDeactivateLinks,
        };
        assert!(commit_native_slide_under_player_step(
            &mut runtime,
            key,
            exited
        ));
        assert_eq!(runtime.current_slide_under, None);
        assert_eq!(runtime.current_owner, None);
    }

    #[test]
    fn watchbot_proxy_target_resolver_uses_live_proxy_only_in_player_proxy_states() {
        let rodney = Vec3::new(1.0, 2.0, 3.0);
        let watchbot = Vec3::new(10.0, 20.0, 30.0);

        assert_eq!(
            resolve_native_ai_gameplay_target_position(2, None, rodney, Some(watchbot)),
            Some(rodney)
        );
        assert_eq!(
            resolve_native_ai_gameplay_target_position(0x2a, None, rodney, Some(watchbot)),
            Some(watchbot)
        );
        assert_eq!(
            resolve_native_ai_gameplay_target_position(0x35, None, rodney, Some(watchbot)),
            Some(watchbot)
        );
        assert_eq!(
            resolve_native_ai_gameplay_target_position(0x35, None, rodney, None),
            None
        );
        for blocked_top in [1, 2, 0x0f] {
            assert_eq!(
                resolve_native_ai_gameplay_target_position(
                    2,
                    Some(blocked_top),
                    rodney,
                    Some(watchbot),
                ),
                None
            );
        }
        for suppressed_state in [0x01, 0x1d, 0x2f, 0x3e] {
            assert_eq!(
                resolve_native_ai_gameplay_target_position(
                    suppressed_state,
                    None,
                    rodney,
                    Some(watchbot),
                ),
                None
            );
        }
    }

    #[test]
    fn watchbot_component_scheduler_fails_closed_without_mode2_snapshot_or_rng_mutation() {
        let mut game_control = RobotsGameControlRuntime::default();
        let mut owner = RobotsWatchbotOwnerRuntime::default();
        let mut mode1 = RobotsWatchbotMode1Runtime::default();
        let mut mode2 = RobotsWatchbotMode2Runtime::default();
        let mut mode3 = RobotsWatchbotMode3Runtime::default();
        let mut rng = RuntimeRobotsGlobalRngState::fresh_process_startup();
        assert!(owner.service_player_owner(true, 0));
        assert!(owner.request_component_mode(ROBOTS_WATCHBOT_CONTROL_COMPONENT_MODE));

        assert!(service_native_watchbot_component_mode_transition_runtime(
            &mut game_control,
            &mut owner,
            &mut mode1,
            &mut mode2,
            &mut mode3,
            &mut rng,
            RobotsWatchbotComponentTransitionGate {
                game_window_exists: true,
                game_window_phase_338: 0,
                game_window_scalar_3b0: 0.0,
                player_watchbot_handler_exists: true,
            },
        )
        .is_none());
        assert_eq!(owner.current_component_mode_4a0, 0);
        assert_eq!(
            owner.pending_component_mode_4a4,
            ROBOTS_WATCHBOT_CONTROL_COMPONENT_MODE
        );
        assert_eq!(rng.draws_from_anchor, 0);
    }

    #[test]
    fn watchbot_component_scheduler_commits_initial_mode2_with_snapshot_and_one_setup_draw() {
        let mut game_control = RobotsGameControlRuntime::default();
        let mut owner = RobotsWatchbotOwnerRuntime::default();
        let mut mode1 = RobotsWatchbotMode1Runtime::default();
        let mut mode2 = RobotsWatchbotMode2Runtime::default();
        let mut mode3 = RobotsWatchbotMode3Runtime::default();
        let mut rng = RuntimeRobotsGlobalRngState::fresh_process_startup();
        let snapshot = RobotsWatchbotComponentTransitionSnapshotBits::from_f32(
            [10.0, 20.0, 30.0, 1.0],
            [0.0, core::f32::consts::FRAC_PI_2, 0.0, 1.0],
        );
        assert!(owner.service_player_owner(true, 0));
        assert!(owner.request_component_mode_with_context(
            ROBOTS_WATCHBOT_CONTROL_COMPONENT_MODE,
            Some(snapshot),
            true,
            false,
            None,
        ));

        let outcome = service_native_watchbot_component_mode_transition_runtime(
            &mut game_control,
            &mut owner,
            &mut mode1,
            &mut mode2,
            &mut mode3,
            &mut rng,
            RobotsWatchbotComponentTransitionGate {
                game_window_exists: true,
                game_window_phase_338: 0,
                game_window_scalar_3b0: 1.0,
                player_watchbot_handler_exists: true,
            },
        )
        .unwrap();
        assert!(outcome.transition.commit_transition);
        assert_eq!(
            owner.current_component_mode_4a0,
            ROBOTS_WATCHBOT_CONTROL_COMPONENT_MODE
        );
        assert_eq!(owner.pending_component_mode_4a4, 0);
        assert_eq!(rng.draws_from_anchor, 1);
        assert!(outcome.setup_rng_draw.is_some());
        let placement = outcome.mode2_entry_placement.unwrap();
        assert!((placement.owner_position_xyzw[0] - 11.5).abs() < 1.0e-6);
        assert!((placement.owner_position_xyzw[1] - 22.25).abs() < 1.0e-6);
        assert!((placement.owner_position_xyzw[2] - 30.0).abs() < 1.0e-6);
        assert!(mode2.idle_timer_8e.is_some());
    }

    #[test]
    fn watchbot_component_scheduler_mode2_to_mode3_resets_runtime_and_requests_path_rebind() {
        let mut game_control = RobotsGameControlRuntime::default();
        let mut owner = RobotsWatchbotOwnerRuntime::default();
        let mut mode1 = RobotsWatchbotMode1Runtime::default();
        let mut mode2 = RobotsWatchbotMode2Runtime::default();
        let mut mode3 = RobotsWatchbotMode3Runtime::default();
        let mut rng = RuntimeRobotsGlobalRngState::fresh_process_startup();

        assert!(owner.service_player_owner(true, 0));
        owner.current_component_mode_4a0 = ROBOTS_WATCHBOT_CONTROL_COMPONENT_MODE;
        owner.current_path_uid_488 = Some(0x1234_5678);
        assert!(owner.request_component_mode(ROBOTS_WATCHBOT_TRIGGER_PATH_MODE));
        assert!(mode3.bind_path(
            &[
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [2.0, 0.0, 0.0],
                [3.0, 0.0, 0.0],
            ],
            [0.0, 0.0, 0.0],
            [1.0, 2.0, 3.0, 0.0],
            1.0,
        ));
        assert!(mode3.path_bound());

        let outcome = service_native_watchbot_component_mode_transition_runtime(
            &mut game_control,
            &mut owner,
            &mut mode1,
            &mut mode2,
            &mut mode3,
            &mut rng,
            RobotsWatchbotComponentTransitionGate {
                game_window_exists: true,
                game_window_phase_338: 0,
                game_window_scalar_3b0: 1.0,
                player_watchbot_handler_exists: true,
            },
        )
        .unwrap();

        assert!(outcome.transition.commit_transition);
        assert!(outcome.transition.seamless_mode2_mode3);
        assert!(outcome.mode3_path_rebind_required);
        assert_eq!(outcome.mode2_entry_placement, None);
        assert!(outcome.setup_rng_draw.is_some());
        assert_eq!(
            owner.current_component_mode_4a0,
            ROBOTS_WATCHBOT_TRIGGER_PATH_MODE
        );
        assert_eq!(owner.current_path_uid_488, Some(0x1234_5678));
        assert_eq!(rng.draws_from_anchor, 1);
        assert!(!mode3.path_bound());
        assert_eq!(mode3.velocity_xyzw_30, [0.0; 4]);
    }

    #[test]
    fn watchbot_hit_adapter_commits_only_real_state7_transitions() {
        let mut owner = RobotsWatchbotOwnerRuntime::default();
        let mut mode2 = RobotsWatchbotMode2Runtime::default();
        let mut mode3 = RobotsWatchbotMode3Runtime::default();

        owner.current_component_mode_4a0 = ROBOTS_WATCHBOT_TARGET_FOLLOW_COMPONENT_MODE;
        let mode1_hit = apply_native_watchbot_hit_runtime(&owner, &mut mode2, &mut mode3, 3, true);
        assert!(mode1_hit.hit.reaction_accepted);
        assert!(mode1_hit.hit.query_hit_still_commits);
        assert_eq!(
            mode1_hit
                .committed_state_request
                .map(|request| request.state),
            Some(ROBOTS_WATCHBOT_COMPONENT_HIT_STATE)
        );
        assert_eq!(
            mode1_hit
                .state7_entry
                .and_then(|plan| plan.animation_mode_uid),
            Some(ROBOTS_WATCHBOT_HIT_ANIMATION_MODE_UID)
        );

        let same_state = apply_native_watchbot_hit_runtime(
            &owner,
            &mut mode2,
            &mut mode3,
            ROBOTS_WATCHBOT_COMPONENT_HIT_STATE,
            true,
        );
        assert!(same_state.hit.state_request.is_some());
        assert_eq!(same_state.committed_state_request, None);
        assert_eq!(same_state.state7_entry, None);

        owner.current_component_mode_4a0 = ROBOTS_WATCHBOT_CONTROL_COMPONENT_MODE;
        mode2.set_internal_state(3);
        let mode2_hit = apply_native_watchbot_hit_runtime(&owner, &mut mode2, &mut mode3, 0, true);
        assert_eq!(mode2.internal_state, ROBOTS_WATCHBOT_COMPONENT_HIT_STATE);
        assert!(mode2_hit.state7_entry.is_some());

        mode2.set_internal_state(ROBOTS_WATCHBOT_COMPONENT_SHARED_IGNORE_STATE);
        let mode2_guard =
            apply_native_watchbot_hit_runtime(&owner, &mut mode2, &mut mode3, 0, true);
        assert!(mode2_guard.hit.reaction_accepted);
        assert_eq!(mode2_guard.hit.state_request, None);
        assert_eq!(
            mode2.internal_state,
            ROBOTS_WATCHBOT_COMPONENT_SHARED_IGNORE_STATE
        );

        owner.current_component_mode_4a0 = ROBOTS_WATCHBOT_TRIGGER_PATH_MODE;
        mode3.set_internal_state(3);
        let mode3_hit = apply_native_watchbot_hit_runtime(&owner, &mut mode2, &mut mode3, 0, true);
        assert_eq!(mode3.internal_state, ROBOTS_WATCHBOT_COMPONENT_HIT_STATE);
        assert!(mode3_hit.state7_entry.is_some());

        mode3.set_internal_state(ROBOTS_WATCHBOT_COMPONENT_SHARED_IGNORE_STATE);
        let mode3_guard =
            apply_native_watchbot_hit_runtime(&owner, &mut mode2, &mut mode3, 0, true);
        assert!(mode3_guard.hit.reaction_accepted);
        assert_eq!(mode3_guard.hit.state_request, None);
        assert_eq!(
            mode3.internal_state,
            ROBOTS_WATCHBOT_COMPONENT_SHARED_IGNORE_STATE
        );

        let absent = apply_native_watchbot_hit_runtime(&owner, &mut mode2, &mut mode3, 0, false);
        assert!(!absent.hit.reaction_accepted);
        assert!(absent.hit.query_hit_still_commits);
        assert_eq!(absent.committed_state_request, None);
        assert_eq!(absent.state7_entry, None);
    }

    #[test]
    fn boss_exec_cycle_resolves_native_outer_then_first_nested_cutscene_only_at_threshold() {
        let triggers = vec![
            trigger_with_links(ROBOTS_BOSS_EXECUTIVE_TYPE, [1, 2, -1, -1, -1, -1, -1, -1]),
            trigger_with_links(ROBOTS_GROUP_TYPE, [3, -1, -1, -1, -1, -1, -1, -1]),
            trigger_with_links(ROBOTS_GROUP_TYPE, [-1, -1, 4, -1, -1, -1, -1, -1]),
            trigger_with_links(ROBOTS_DOOR_TYPE, [-1; 8]),
            trigger_with_links(ROBOTS_CUTSCENE_TYPE, [-1; 8]),
        ];
        let mut state = RobotsBossExecCycleState {
            threshold_invocations: 6,
        };

        let before = service_native_boss_exec_cycle_runtime(&triggers, 0, &mut state).unwrap();
        assert_eq!(before.cycle.threshold_invocations, 7);
        assert!(!before.cycle.graph_scan_performed);
        assert_eq!(before.cutscene_trigger_index, None);

        let threshold = service_native_boss_exec_cycle_runtime(&triggers, 0, &mut state).unwrap();
        assert_eq!(threshold.cycle.threshold_invocations, 8);
        assert!(threshold.cycle.graph_scan_performed);
        assert!(threshold.cycle.invoke_exec_reset);
        let dispatch = threshold.cycle.cutscene_dispatch.unwrap();
        assert_eq!(dispatch.outer_link_index, 1);
        assert_eq!(dispatch.nested_link_index, 2);
        assert_eq!(threshold.cutscene_trigger_index, Some(4));
    }

    #[test]
    fn boss_sewer_hit_binds_exact_link7_stage_target_without_gating_resource_swap() {
        let linked = vec![
            trigger_with_links(ROBOTS_BOSS_SEWER_TYPE, [-1, -1, -1, -1, -1, -1, -1, 1]),
            trigger_with_links(ROBOTS_GROUP_TYPE, [-1; 8]),
        ];
        let mut state = RobotsBossSewerHitState {
            state: 1,
            local_target_present: true,
            field_6a8: 9,
        };
        let mut ai = RobotsAiHitReactionState {
            query_flags_snapshot: 0,
            query_serial_snapshot: u16::MAX,
            hit_metadata_snapshot: 0,
            last_query_serial: u16::MAX,
            capability_flags: ROBOTS_AI_HIT_CAPABILITY_FLAG,
            health: 9,
            got_hit_latch: false,
            owner_category: 1,
        };
        let hit = RobotsAcceptedHitReactionInput {
            query_flags: ROBOTS_BOSS_SEWER_REQUIRED_HIT_FLAG,
            query_serial: 1,
            hit_metadata: 0,
            source_is_candidate_owner: false,
            secondary_source_is_candidate_owner: false,
        };

        let found = apply_native_boss_sewer_hit_runtime(
            &linked,
            Some(0),
            &mut state,
            &mut ai,
            hit,
            true,
            false,
        );
        assert!(found.hit.reaction_accepted);
        assert_eq!(found.hit.stage_resource_index, Some(8));
        assert_eq!(
            found.hit.stage_resource_uid,
            Some(ROBOTS_BOSS_SEWER_STAGE_SCRIPT_UID)
        );
        assert_eq!(found.hit.stage_event.unwrap().link7_chain_index, 0);
        assert_eq!(found.stage_event_target_trigger_index, Some(1));
        assert_eq!(state.state, 7);
        assert_eq!(ai.health, 8);

        let missing = vec![trigger_with_links(ROBOTS_BOSS_SEWER_TYPE, [-1; 8])];
        state = RobotsBossSewerHitState {
            state: 1,
            local_target_present: true,
            field_6a8: 9,
        };
        ai = RobotsAiHitReactionState {
            query_flags_snapshot: 0,
            query_serial_snapshot: u16::MAX,
            hit_metadata_snapshot: 0,
            last_query_serial: u16::MAX,
            capability_flags: ROBOTS_AI_HIT_CAPABILITY_FLAG,
            health: 9,
            got_hit_latch: false,
            owner_category: 1,
        };
        let missing = apply_native_boss_sewer_hit_runtime(
            &missing,
            Some(0),
            &mut state,
            &mut ai,
            RobotsAcceptedHitReactionInput {
                query_serial: 2,
                ..hit
            },
            true,
            false,
        );
        assert!(missing.hit.reaction_accepted);
        assert_eq!(missing.hit.stage_resource_index, Some(8));
        assert_eq!(
            missing.hit.stage_resource_uid,
            Some(ROBOTS_BOSS_SEWER_STAGE_SCRIPT_UID)
        );
        assert_eq!(missing.hit.stage_event, None);
        assert_eq!(missing.stage_event_target_trigger_index, None);
        assert_eq!(state.state, 1);
        assert_eq!(ai.health, 9);
    }

    #[test]
    fn boss_sewer_canon_hit_validates_creator_and_preserves_outer_query_commit() {
        let triggers = vec![trigger_with_links(ROBOTS_BOSS_SEWER_CANON_TYPE, [-1; 8])];

        let accepted =
            apply_native_boss_sewer_canon_hit_runtime(&triggers, Some(0), true, false, true, true);
        assert!(accepted.hit.reaction_accepted);
        assert!(accepted.hit.query_hit_still_commits);
        assert_eq!(
            accepted.hit.dispatch_creator_event_mask,
            Some(ROBOTS_BOSS_SEWER_CANON_EVENT_MASK)
        );
        assert_eq!(accepted.creator_event_target_trigger_index, Some(0));

        let accepted_secondary =
            apply_native_boss_sewer_canon_hit_runtime(&triggers, Some(0), false, true, true, true);
        assert!(accepted_secondary.hit.reaction_accepted);

        let wrong_creator = vec![trigger_with_links(ROBOTS_GROUP_TYPE, [-1; 8])];
        let rejected = apply_native_boss_sewer_canon_hit_runtime(
            &wrong_creator,
            Some(0),
            true,
            false,
            true,
            true,
        );
        assert!(!rejected.hit.reaction_accepted);
        assert!(rejected.hit.query_hit_still_commits);
        assert_eq!(rejected.hit.dispatch_creator_event_mask, None);
        assert_eq!(rejected.creator_event_target_trigger_index, None);

        let unarmed =
            apply_native_boss_sewer_canon_hit_runtime(&triggers, Some(0), true, false, false, true);
        assert!(!unarmed.hit.reaction_accepted);
        assert!(unarmed.hit.query_hit_still_commits);
        assert_eq!(unarmed.creator_event_target_trigger_index, None);
    }

    #[test]
    fn door_script_command_reuses_trigger_graph_state_and_returns_linked_sync_action() {
        let mut door = trigger_with_links(ROBOTS_DOOR_TYPE, [-1; 8]);
        door.data[0] = Some(3);
        let mut graph = RuntimeTriggerGraphState::default();

        let open_wait = apply_native_door_script_command_runtime(
            &door,
            &mut graph,
            RobotsDoorCommandKind::Open,
        )
        .unwrap();
        assert!(graph.door.initialized);
        assert_eq!(graph.door.request_e5, 3);
        assert_eq!(open_wait.return_value, 1);
        assert_eq!(open_wait.sync_linked_closed_state, None);

        graph.door.state_e4 = 1;
        let open_done = apply_native_door_script_command_runtime(
            &door,
            &mut graph,
            RobotsDoorCommandKind::Open,
        )
        .unwrap();
        assert_eq!(open_done.return_value, 0);
        assert_eq!(open_done.sync_linked_closed_state, Some(false));

        graph.door.state_e4 = 0;
        graph.door.request_ed = 1;
        let closed = apply_native_door_script_command_runtime(
            &door,
            &mut graph,
            RobotsDoorCommandKind::Closed,
        )
        .unwrap();
        assert_eq!(closed.sync_linked_closed_state, Some(true));
        assert_eq!(graph.door.request_e5, 1);
        assert_eq!(graph.door.request_ed, 0);

        let wrong = trigger_with_links(ROBOTS_GROUP_TYPE, [-1; 8]);
        assert_eq!(
            apply_native_door_script_command_runtime(
                &wrong,
                &mut graph,
                RobotsDoorCommandKind::Close,
            ),
            None
        );
    }

    #[test]
    fn door_fixed_seam_uses_serialized_query_plan_and_same_trigger_graph_state() {
        let mut door = trigger_with_links(ROBOTS_DOOR_TYPE, [-1; 8]);
        door.data[0] = Some(0);
        door.data[1] = Some(20);
        door.data[2] = Some(30);
        door.data[3] = Some(4);
        door.data[4] = Some(2);
        door.data[5] = Some(0x4500_008C);
        let plan = native_door_distance_query_plan(&door).unwrap();
        assert_eq!(
            plan.runtime_category_scan_order,
            [Some(0), Some(1), Some(3), Some(0)]
        );
        assert!(plan.horizontal_only);
        let mut graph = RuntimeTriggerGraphState::default();
        let opened = advance_native_door_fixed_runtime(
            &door,
            &mut graph,
            RobotsDoorFixedHostInput {
                nearest_distance_squared: 3.0,
                viewpoint_distance_squared: Some(100.0),
                viewpoint_control_mode: None,
                camera_distance_squared: 100.0,
                player_aux_distance_squared: None,
            },
        )
        .unwrap();
        assert_eq!(opened.return_value, 3);
        assert_eq!(opened.show_hint_text_uid, None);
        assert_eq!(graph.door.state_e4, 1);
        graph.door.request_e5 = 1;
        graph.door.state_e4 = 0;
        let hint = advance_native_door_fixed_runtime(
            &door,
            &mut graph,
            RobotsDoorFixedHostInput {
                nearest_distance_squared: 3.0,
                viewpoint_distance_squared: None,
                viewpoint_control_mode: None,
                camera_distance_squared: 99_999.0,
                player_aux_distance_squared: None,
            },
        )
        .unwrap();
        assert_eq!(hint.show_hint_text_uid, Some(0x4500_008C));
        assert_eq!(graph.door.proximity_latch_ec, 1);
    }
    #[test]
    fn fix_switch_host_seam_keeps_trigger_state_and_handler_dirty_edge_separate() {
        let mut fix_switch = trigger_with_links(ROBOTS_FIX_SWITCH_TYPE, [-1; 8]);
        fix_switch.data[0] = Some(1);
        fix_switch.data[2] = Some(0.03f32.to_bits());
        let mut graph = RuntimeTriggerGraphState::default();

        let ordinary =
            apply_native_fix_switch_event_runtime(&fix_switch, &mut graph, 0x101, true).unwrap();
        assert!(!ordinary.mark_owned_handler_progress_dirty);
        assert!(graph.fix_switch.initialized);
        assert_eq!(graph.fix_switch.state_e4, 1);
        assert!(graph.fix_switch.interaction_active());

        graph.fix_switch.timer_ec = 2.0;
        let service =
            apply_native_fix_switch_event_runtime(&fix_switch, &mut graph, 0x1000, true).unwrap();
        assert!(service.mark_owned_handler_progress_dirty);
        assert_eq!(graph.fix_switch.timer_ec, 0.0);

        let first =
            advance_native_fix_switch_progress_runtime(&fix_switch, &mut graph, 0.02).unwrap();
        assert!(!first.request_deferred_output);
        assert!(!graph.deferred_fire);
        let completed =
            advance_native_fix_switch_progress_runtime(&fix_switch, &mut graph, 0.02).unwrap();
        assert!(completed.request_deferred_output);
        assert!(graph.deferred_fire);
        assert!(!graph.fix_switch.interaction_active());
    }

    #[test]
    fn ball_track_host_seam_returns_exact_create_stop_and_cleanup_actions() {
        let mut trigger = trigger_with_links(ROBOTS_BALL_TRACK_TYPE, [-1; 8]);
        trigger.data[1] = Some(1);
        let mut graph = RuntimeTriggerGraphState::default();

        let create =
            apply_native_ball_track_event_runtime(&trigger, &mut graph, 0x100, false).unwrap();
        assert!(create.create_owned_track);
        assert!(graph.ball_track.owned_track_created);
        assert!(!graph.ball_track.spawn_stopped);

        let stop =
            apply_native_ball_track_event_runtime(&trigger, &mut graph, 0x200, true).unwrap();
        assert!(stop.stop_spawning);
        assert!(!stop.cleanup_owned_track);
        assert!(graph.ball_track.owned_track_created);
        assert!(graph.ball_track.spawn_stopped);

        let cleanup =
            apply_native_ball_track_event_runtime(&trigger, &mut graph, 0x200, true).unwrap();
        assert!(!cleanup.stop_spawning);
        assert!(cleanup.cleanup_owned_track);
        assert!(!graph.ball_track.owned_track_created);
        assert!(!graph.ball_track.spawn_stopped);
    }

    #[test]
    fn ball_track_ordinary_iteration_previews_rng_and_defers_found_slot_commit() {
        let ordinary_slots = [
            RobotsBallTrackSpawnSlot {
                live_handler_present: true,
                in_use: false,
                variant: 0,
            },
            RobotsBallTrackSpawnSlot {
                live_handler_present: true,
                in_use: false,
                variant: 0,
            },
        ];
        let burst = RobotsBallTrackOrdinaryBurstRuntime::default();
        assert!(preview_native_ball_track_ordinary_iteration(
            &burst,
            &ordinary_slots,
            0,
            2,
            0x0B00_000E,
            18.0,
            100.0,
            120.0,
            RuntimeRobotsGlobalRngState::default(),
        )
        .is_none());

        let anchored = RuntimeRobotsGlobalRngState::fresh_process_startup();
        let ordinary = preview_native_ball_track_ordinary_iteration(
            &burst,
            &ordinary_slots,
            0,
            2,
            0x0B00_000E,
            18.0,
            100.0,
            120.0,
            anchored,
        )
        .unwrap();
        assert_eq!(ordinary.rng_draws_consumed, 3);
        assert_eq!(ordinary.next_rng.draws_from_anchor, 3);
        assert!(matches!(
            ordinary.request.unwrap().parameter,
            RobotsBallTrackSpawnParameterRequest::AdvanceDistance { .. }
        ));

        let occupied_slots = [
            RobotsBallTrackSpawnSlot {
                live_handler_present: true,
                in_use: true,
                variant: 0,
            },
            RobotsBallTrackSpawnSlot {
                live_handler_present: true,
                in_use: true,
                variant: 0,
            },
            RobotsBallTrackSpawnSlot {
                live_handler_present: true,
                in_use: true,
                variant: 0,
            },
        ];
        let occupied = preview_native_ball_track_ordinary_iteration(
            &burst,
            &occupied_slots,
            0,
            1,
            0x0B00_000E,
            18.0,
            100.0,
            120.0,
            anchored,
        )
        .unwrap();
        assert_eq!(occupied.request, None);
        assert_eq!(occupied.rng_draws_consumed, 1);
        assert_eq!(occupied.next_rng.draws_from_anchor, 1);

        let special_slots = [
            RobotsBallTrackSpawnSlot {
                live_handler_present: true,
                in_use: false,
                variant: 2,
            },
            RobotsBallTrackSpawnSlot {
                live_handler_present: true,
                in_use: false,
                variant: 2,
            },
        ];
        let special = preview_native_ball_track_ordinary_iteration(
            &burst,
            &special_slots,
            0,
            1,
            0x0B00_000E,
            18.0,
            0.0,
            0.0,
            anchored,
        )
        .unwrap();
        assert_eq!(special.rng_draws_consumed, 1);
        assert_eq!(special.next_rng.draws_from_anchor, 1);
        assert_eq!(
            special.request.unwrap().parameter,
            RobotsBallTrackSpawnParameterRequest::NearestOwnerNodeIndex
        );
    }

    #[test]
    fn ball_track_ordinary_commit_is_atomic_across_path_resolution_rng_and_burst_state() {
        let slots = [
            RobotsBallTrackSpawnSlot {
                live_handler_present: true,
                in_use: false,
                variant: 0,
            },
            RobotsBallTrackSpawnSlot {
                live_handler_present: true,
                in_use: false,
                variant: 0,
            },
        ];
        let original_rng = RuntimeRobotsGlobalRngState::fresh_process_startup();
        let mut live_rng = original_rng;
        let mut burst = RobotsBallTrackOrdinaryBurstRuntime::default();
        let begin = preview_native_ball_track_ordinary_iteration(
            &burst,
            &slots,
            0,
            1,
            0x0B00_000E,
            18.0,
            100.0,
            120.0,
            live_rng,
        )
        .unwrap();
        assert_eq!(begin.rng_draws_consumed, 3);
        assert_eq!(live_rng, original_rng);
        assert_eq!(burst, RobotsBallTrackOrdinaryBurstRuntime::default());

        assert!(commit_native_ball_track_ordinary_iteration_runtime(
            &mut live_rng,
            &mut burst,
            begin,
            f32::NAN,
            4.0,
            false,
        )
        .is_none());
        assert_eq!(live_rng, original_rng);
        assert_eq!(burst, RobotsBallTrackOrdinaryBurstRuntime::default());

        let committed = commit_native_ball_track_ordinary_iteration_runtime(
            &mut live_rng,
            &mut burst,
            begin,
            1.25,
            4.0,
            false,
        )
        .unwrap();
        assert_eq!(committed.rng_draws_consumed, 3);
        assert_eq!(live_rng.draws_from_anchor, 3);
        assert_eq!(burst.previous_path_parameter, 1.25);
        assert_eq!(burst.spawned_count, 1);
        assert!(committed.spawn.unwrap().continue_burst);
    }

    #[test]
    fn ball_track_alternate_host_seam_resolves_schedule_path_and_rng_without_map_owned_lane_state()
    {
        let mut trigger = trigger_with_links(ROBOTS_BALL_TRACK_TYPE, [-1; 8]);
        trigger.data[1] = Some(1);
        trigger.data[2] = Some(16);
        trigger.data[3] = Some(1);
        trigger.data[4] = Some(5.0f32.to_bits());
        trigger.data[5] = Some(1000);

        let pair = RobotsBallTrackPair {
            sheet_index: 16,
            pool: vec![],
            paths: vec![
                RobotsBallPathEntry {
                    path_hashcode: 0x0B00_0010,
                    raw_value4: 0.0,
                    runtime_speed: 12.0,
                    start_distance_min: 0.0,
                    start_distance_max: 0.0,
                },
                RobotsBallPathEntry {
                    path_hashcode: 0x0B00_0011,
                    raw_value4: 0.0,
                    runtime_speed: 99.0,
                    start_distance_min: 0.0,
                    start_distance_max: 0.0,
                },
            ],
        };
        let schedule = RobotsBallTrackSchedule {
            sheet_index: 1,
            rows: vec![[0, 1, 0, 0, 0, 0, 0, 0], [1, 0, 0, 0, 0, 0, 0, 0]],
        };
        let make_path = |hashcode| ProcessedPath {
            hashcode,
            position: Vec3::ZERO,
            flags: 0,
            path_type: 0,
            nodes: (0..5)
                .map(|_| ProcessedPathNode {
                    position: Vec3::ZERO,
                    size: glam::Vec2::ZERO,
                    value: [0; 4],
                    flags: 0,
                    distance: 0.0,
                    num_links: 0,
                })
                .collect(),
            links: vec![],
        };
        let mut map = ProcessedMap {
            triggers: vec![trigger],
            paths: vec![make_path(0x0B00_0010), make_path(0x0B00_0011)],
            ..Default::default()
        };
        map.ball_track_pairs.insert(16, pair);
        map.ball_track_schedules.insert(1, schedule);

        let config = resolve_native_ball_track_alternate_config(&map, &map.triggers[0], 0).unwrap();
        assert_eq!(config.path_node_count, 5);
        assert_eq!(config.schedule_row_count, 2);
        assert_eq!(config.first_path_runtime_speed, 12.0);
        assert_eq!(config.spacing_distance, 5.0);
        assert_eq!(config.cycle_limit, 1000);

        let mut lane0 = RobotsBallTrackAlternateLaneRuntime::default();
        let request = lane0
            .begin_tick(
                false,
                config.path_node_count,
                config.schedule_row_count,
                config.cycle_limit,
                config.first_path_runtime_speed * ROBOTS_BALL_TRACK_FIXED_STEP_SECONDS,
                config.spacing_distance,
            )
            .unwrap()
            .unwrap();
        let row = native_ball_track_alternate_row_outcome(&map, config, request).unwrap();
        assert!(row.schedule_active);
        assert_eq!(row.path_hashcode, 0x0B00_0010);
        assert_eq!(row.controller_path_hashcode, 0x0B00_0010);
        assert_eq!(row.path_advance_distance, -5.0);

        let slots = [
            RobotsBallTrackSpawnSlot {
                live_handler_present: true,
                in_use: false,
                variant: 0,
            },
            RobotsBallTrackSpawnSlot {
                live_handler_present: true,
                in_use: false,
                variant: 0,
            },
        ];
        let (spawn, next_rng) = preview_native_ball_track_alternate_spawn(
            &slots,
            row,
            config.path_lane_count,
            config.first_path_runtime_speed,
            RuntimeRobotsGlobalRngState::fresh_process_startup(),
        )
        .unwrap();
        assert_eq!(spawn.rng_draws_consumed, 1);
        assert_eq!(next_rng.draws_from_anchor, 1);
        let plan = spawn.plan.unwrap();
        assert_eq!(plan.path_lane, 0);
        assert_eq!(plan.controller_path_lane, 0);
        assert_eq!(plan.path_parameter, 3.0);
        assert_eq!(plan.runtime_speed, 12.0);

        let config1 =
            resolve_native_ball_track_alternate_config(&map, &map.triggers[0], 1).unwrap();
        let mut lane1 = RobotsBallTrackAlternateLaneRuntime::default();
        let request1 = lane1
            .begin_tick(
                false,
                config1.path_node_count,
                config1.schedule_row_count,
                config1.cycle_limit,
                config1.first_path_runtime_speed * ROBOTS_BALL_TRACK_FIXED_STEP_SECONDS,
                config1.spacing_distance,
            )
            .unwrap()
            .unwrap();
        let row1 = native_ball_track_alternate_row_outcome(&map, config1, request1).unwrap();
        assert!(!row1.schedule_active);
        assert_eq!(row1.path_hashcode, 0x0B00_0011);
        assert_eq!(row1.controller_path_hashcode, 0x0B00_0010);
        let (idle, idle_rng) = preview_native_ball_track_alternate_spawn(
            &slots,
            row1,
            config1.path_lane_count,
            config1.first_path_runtime_speed,
            RuntimeRobotsGlobalRngState::fresh_process_startup(),
        )
        .unwrap();
        assert_eq!(idle.plan, None);
        assert_eq!(idle.rng_draws_consumed, 0);
        assert_eq!(idle_rng.draws_from_anchor, 0);
    }

    #[test]
    fn watchbot_focus_interaction_enters_control_and_consumes_focus_exactly_once() {
        let mut game_control = RobotsGameControlRuntime::default();
        let mut player_action = RobotsPlayerActionRuntime::default();
        let mut player_focus = NativePlayerFocusRuntimeState::default();
        let mut watchbot_owner = RobotsWatchbotOwnerRuntime::default();
        let focused_snapshot = RobotsWatchbotComponentTransitionSnapshotBits::from_f32(
            [4.0, 5.0, 6.0, 1.0],
            [0.0, 0.75, 0.0, 1.0],
        );
        assert!(watchbot_owner.service_player_owner(true, 0));
        player_focus.current_owner = Some(NativePlayerFocusOwner {
            key: 7,
            category: ROBOTS_XITEM_CATEGORY_ACTIVATION_PAD,
        });

        let enter = interact_native_watchbot_focus_transition(
            &mut game_control,
            &mut player_action,
            &mut player_focus,
            &mut watchbot_owner,
            focused_snapshot,
            None,
            Some(0x1234),
        )
        .unwrap();
        assert_eq!(game_control.mode_50f, 1);
        assert_eq!(player_focus.player_state, 0x35);
        assert_eq!(
            player_focus.current_interaction_code,
            ROBOTS_WATCHBOT_INTERACTION_CODE
        );
        assert_eq!(player_focus.current_owner, None);
        assert_eq!(enter.requested_anim_mode_uid, 0x0900_00c1);
        assert_eq!(enter.requested_watchbot_component_mode, 2);
        assert_eq!(watchbot_owner.pending_component_mode_4a4, 2);
        assert_eq!(
            watchbot_owner.pending_component_snapshot,
            Some(focused_snapshot)
        );
        assert_eq!(enter.entry_sound_uid, Some(0x1b00_003f));
        assert_eq!(
            player_action.watchbot_exit_sound_uid_4d4,
            Some(enter.exit_sound_uid_4d4)
        );

        let mut blocked_control = RobotsGameControlRuntime::default();
        let mut blocked_watchbot_owner = RobotsWatchbotOwnerRuntime::default();
        assert!(blocked_watchbot_owner.service_player_owner(true, 0));
        blocked_control.set_mode(2).unwrap();
        let mut blocked_action = RobotsPlayerActionRuntime::default();
        let mut blocked_focus = NativePlayerFocusRuntimeState::default();
        blocked_focus.current_owner = Some(NativePlayerFocusOwner {
            key: 9,
            category: ROBOTS_XITEM_CATEGORY_ACTIVATION_PAD,
        });
        assert!(interact_native_watchbot_focus_transition(
            &mut blocked_control,
            &mut blocked_action,
            &mut blocked_focus,
            &mut blocked_watchbot_owner,
            focused_snapshot,
            None,
            None,
        )
        .is_none());
        assert_eq!(blocked_control.mode_50f, 2);
        assert_eq!(
            blocked_focus.current_interaction_code,
            ROBOTS_WATCHBOT_INTERACTION_CODE
        );
        assert_eq!(blocked_focus.current_owner, None);
        assert_eq!(blocked_action.watchbot_exit_sound_uid_4d4, None);
    }

    #[test]
    fn watchbot_control_mode_edge_services_player_exit_once_and_preserves_blocked_edge() {
        let mut game_control = RobotsGameControlRuntime::default();
        let mut player_action = RobotsPlayerActionRuntime::default();
        let mut player_state = 0x0e;
        let mut player_items = RobotsPlayerItemState::default();
        assert!(player_items.seed_profile_value(0x4800_0021, 1));
        player_action.request_cutscene_action(ROBOTS_PLAYER_ACTION_SCRAP_GUN, player_state, |_| 0);
        assert_eq!(
            player_action.pending_action_uid_5e8,
            Some(ROBOTS_PLAYER_ACTION_SCRAP_GUN)
        );

        game_control.set_mode(1).unwrap();
        game_control.set_mode(0).unwrap();
        let exit = service_native_watchbot_control_exit_transition(
            &mut game_control,
            &mut player_action,
            &mut player_state,
            &player_items,
            Some(0x1af0_02c5),
        )
        .unwrap();
        assert_eq!(player_state, 2);
        assert_eq!(exit.exit_sound_uid, Some(0x1af0_02c5));
        assert_eq!(player_action.pending_action_uid_5e8, None);
        assert_eq!(
            player_action.active_action.unwrap().scrap_gun_inventory_uid,
            Some(0x4800_0021)
        );
        assert!(!game_control.watchbot_exit_pending());
        assert!(service_native_watchbot_control_exit_transition(
            &mut game_control,
            &mut player_action,
            &mut player_state,
            &player_items,
            None,
        )
        .is_none());

        game_control.set_mode(1).unwrap();
        game_control.set_mode(0).unwrap();
        player_state = 0x35;
        assert!(service_native_watchbot_control_exit_transition(
            &mut game_control,
            &mut player_action,
            &mut player_state,
            &player_items,
            None,
        )
        .is_none());
        assert!(game_control.watchbot_exit_pending());
    }

    #[test]
    fn shop_adapter_uses_processed_database_membership_and_existing_gameplay_owners() {
        use eurochef_shared::robots_runtime::{
            inventory::ROBOTS_SCRAP_UID,
            player_items::ROBOTS_PLAYER_ITEM_GATE_AGGREGATE,
            shop::{RobotsShopGroupDefinition, RobotsShopItemDefinition},
        };

        let database = RobotsShopDatabase {
            groups: vec![RobotsShopGroupDefinition::from_native_words([
                1,
                0x4800_0019,
                250,
                u32::MAX,
                0,
                u32::MAX,
                0,
                u32::MAX,
                0,
                u32::MAX,
                0,
                u32::MAX,
                0,
                u32::MAX,
                0,
                u32::MAX,
                0,
            ])
            .unwrap()],
            items: vec![RobotsShopItemDefinition::from_native_words([
                0x4800_0019,
                u32::MAX,
                0x0600_000b,
                0x0400_00c8,
                u32::MAX,
                0x4500_0168,
                0x4500_016b,
                1,
                1,
            ])
            .unwrap()],
        };
        let scrap_definition =
            RobotsInventoryDefinition::from_native_words([ROBOTS_SCRAP_UID, 0, 0, 0, 0, 0, 1000])
                .unwrap();
        let mut gameplay = RobotsScriptGameplayState::default();
        assert_eq!(
            gameplay
                .inventory
                .add(Some(scrap_definition), 500)
                .native_result_code,
            1
        );
        let mut player_items = RobotsPlayerItemState::default();

        let outcome = apply_native_shop_purchase(
            Some(&database),
            &[scrap_definition],
            &mut gameplay,
            &mut player_items,
            1,
            0,
            false,
        )
        .unwrap();
        assert!(outcome.committed);
        assert_eq!(gameplay.inventory.stored_current(ROBOTS_SCRAP_UID), 250);
        assert_eq!(player_items.stored_current(0x4800_0019), 1);
        assert_eq!(
            player_items.stored_current(ROBOTS_PLAYER_ITEM_GATE_AGGREGATE),
            1
        );
    }

    #[test]
    fn cutscene_player_action_adapter_commits_m10_scrapgun_and_preserves_ammo_priority() {
        use eurochef_shared::robots_runtime::player_action::{
            RobotsPlayerActionKind, RobotsPlayerActionRequestStatus,
        };

        let mut runtime = RobotsPlayerActionRuntime::default();
        let mut player_items = RobotsPlayerItemState::default();
        player_items.seed_profile_value(0x4800_0022, 1);
        player_items.seed_profile_value(0x4800_0023, 1);
        player_items.seed_profile_value(0x4800_0024, 1);
        apply_native_cutscene_player_action(
            &mut runtime,
            2,
            &player_items,
            RobotsCutscenePlayerAction::Action48000005,
        );
        assert_eq!(runtime.pending_action_uid_5e8, None);
        assert_eq!(
            runtime.committed_action_uid_5cc,
            ROBOTS_PLAYER_ACTION_SCRAP_GUN
        );
        assert!(matches!(
            runtime.active_action,
            Some(action)
                if action.kind == RobotsPlayerActionKind::ScrapGun
                    && action.scrap_gun_inventory_uid == Some(0x4800_0022)
        ));
        let repeated = runtime.request_cutscene_action(ROBOTS_PLAYER_ACTION_SCRAP_GUN, 2, |_| 0);
        assert_eq!(repeated.status, RobotsPlayerActionRequestStatus::Attached);
    }

    #[test]
    fn cutscene_player_action_adapter_queues_when_native_player_state_blocks_replacement() {
        let mut runtime = RobotsPlayerActionRuntime::default();
        let player_items = RobotsPlayerItemState::default();
        apply_native_cutscene_player_action(
            &mut runtime,
            0x0e,
            &player_items,
            RobotsCutscenePlayerAction::Action48000000,
        );
        assert_eq!(
            runtime.pending_action_uid_5e8,
            Some(ROBOTS_PLAYER_ACTION_48000000)
        );
        assert!(runtime.active_action.is_none());
    }

    #[test]
    fn cutscene_setpropertiesplayer_adapter_toggles_only_native_48000026_item() {
        use eurochef_shared::robots_runtime::player_items::ROBOTS_PLAYER_ITEM_CUTSCENE_PROPERTY;

        let mut player_items = RobotsPlayerItemState::default();
        apply_native_cutscene_player_property(&mut player_items, Some(1));
        assert_eq!(
            player_items.stored_current(ROBOTS_PLAYER_ITEM_CUTSCENE_PROPERTY),
            1
        );
        apply_native_cutscene_player_property(&mut player_items, Some(0));
        assert_eq!(
            player_items.stored_current(ROBOTS_PLAYER_ITEM_CUTSCENE_PROPERTY),
            0
        );
        apply_native_cutscene_player_property(&mut player_items, None);
        assert_eq!(player_items.values.len(), 0);
    }

    #[test]
    fn pickup_class_reject_preserves_trigger_manager_delay_without_rng_or_create_budget() {
        let lifecycle = RuntimeScriptTriggerLifecycleState::default();
        let pickup = RobotsPickupRuntimeState::default();
        let rng = RuntimeRobotsGlobalRngState::fresh_process_startup();
        let rejected = preview_pickup_trigger_manager_shared_rng(
            lifecycle,
            pickup,
            0,
            crate::map_runtime::RuntimeCommonTriggerLifecycleAction::DelayedActive { ticks: 3 },
            0,
            1.0,
            0x1C,
            &[Some(0)],
            Some(3),
            rng,
            true,
        )
        .expect("native TrickChip preflight rejection is a resolved no-create path");

        assert!(!rejected.lifecycle.xitem_exists);
        assert_eq!(rejected.lifecycle.delay_counter, 1);
        assert_eq!(rejected.creates_this_update, 0);
        assert_eq!(rejected.rng, rng);
        assert!(rejected.pickup.suppressed_e8);
        assert!(rejected.spawned.is_none());
    }

    #[test]
    fn pickup_creation_waits_for_shared_rng_anchor_and_commits_exactly_one_draw() {
        let lifecycle = RuntimeScriptTriggerLifecycleState::default();
        let pickup = RobotsPickupRuntimeState::default();
        let action = crate::map_runtime::RuntimeCommonTriggerLifecycleAction::ImmediateActive;

        assert!(preview_pickup_trigger_manager_shared_rng(
            lifecycle,
            pickup,
            0,
            action,
            0,
            0.0,
            0x19,
            &[Some(50)],
            None,
            RuntimeRobotsGlobalRngState::default(),
            true,
        )
        .is_none());
        assert!(!lifecycle.xitem_exists);

        let anchored = RuntimeRobotsGlobalRngState::fresh_process_startup();
        let created = preview_pickup_trigger_manager_shared_rng(
            lifecycle,
            pickup,
            0,
            action,
            0,
            0.0,
            0x19,
            &[Some(50)],
            None,
            anchored,
            true,
        )
        .expect("an exact process-global RNG anchor must allow Pickup creation");
        assert!(created.lifecycle.xitem_exists);
        assert_eq!(created.creates_this_update, 1);
        assert_eq!(created.rng.draws_from_anchor, 1);
        assert!(!created.pickup.suppressed_e8);
        let spawn = created
            .spawned
            .expect("successful CreateItem must expose its XItem payload");
        assert_eq!(spawn.layout.native_runtime_type, 0x2C);
        assert_eq!(spawn.layout.pickup_uid, 0x4700_0001);
        assert_eq!(spawn.layout.quantity, 50);
        assert!(spawn.initial_spin_radians >= 0.0);
        assert!(spawn.initial_spin_radians < std::f32::consts::TAU);
    }

    #[test]
    fn native_fluid_setup_draw_count_matches_initial_wave_loop() {
        let mut fluid = camera(0);
        fluid.ttype = ROBOTS_FLUID_TYPE;
        fluid.data[0] = Some(13);
        fluid.data[5] = Some(5);
        assert_eq!(robots_fluid_setup_draws(&fluid), 4);

        fluid.data[0] = Some(2);
        assert_eq!(robots_fluid_setup_draws(&fluid), 0);
        fluid.data[0] = Some(13);
        fluid.data[5] = Some(1);
        assert_eq!(robots_fluid_setup_draws(&fluid), 0);
    }

    #[test]
    fn native_fluid_creation_waits_for_shared_rng_anchor_transactionally() {
        let state = RuntimeScriptTriggerLifecycleState::default();
        assert!(preview_fluid_trigger_manager_shared_rng(
            state,
            0,
            crate::map_runtime::RuntimeCommonTriggerLifecycleAction::ImmediateActive,
            0,
            0.0,
            RuntimeRobotsGlobalRngState::default(),
            true,
            Some(4),
            false,
        )
        .is_none());
        assert!(!state.xitem_exists);

        let anchored = RuntimeRobotsGlobalRngState::from_observed_seed(0x955);
        let (created, creates, rng) = preview_fluid_trigger_manager_shared_rng(
            state,
            0,
            crate::map_runtime::RuntimeCommonTriggerLifecycleAction::ImmediateActive,
            0,
            0.0,
            anchored,
            true,
            Some(4),
            false,
        )
        .expect("an exact shared-RNG anchor must allow the pending Fluid creation");
        assert!(created.xitem_exists);
        assert_eq!(creates, 1);
        assert_eq!(rng.draws_from_anchor, 4);
        assert_ne!(rng.seed(), Some(0x955));
    }

    #[test]
    fn native_common_trigger_dispatch_matches_enable_disable_and_toggle_gates() {
        let mut trigger = camera(0);
        trigger.game_flags = 1;
        let mut state = NativeCommonTriggerEventState::default();

        let ignored_disable = state.dispatch(&trigger, 0x2);
        assert!(state.disabled);
        assert!(!ignored_disable.forward_to_class);
        assert!(!ignored_disable.disable_hook);

        let enabled = state.dispatch(&trigger, 0x1);
        assert!(!state.disabled);
        assert!(enabled.forward_to_class);
        assert!(enabled.enable_hook);

        let disabled = state.dispatch(&trigger, 0x2);
        assert!(state.disabled);
        assert!(!disabled.forward_to_class);
        assert!(disabled.disable_hook);

        let enabled_again = state.dispatch(&trigger, 0x1);
        assert!(enabled_again.forward_to_class);
        assert!(!state.disabled);
        state.dispatch(&trigger, 0x4);
        assert!(state.toggle_bit1);
        state.dispatch(&trigger, 0x8);
        assert!(!state.toggle_bit1);
        state.dispatch(&trigger, 0x10000);
        assert!(state.linked_initialized);
        assert_eq!(state.last_event, 0x10000);

        let mut cutscene = camera(0);
        cutscene.ttype = 19;
        cutscene.game_flags = 1 | 0x1000_0000;
        let mut cutscene_state = NativeCommonTriggerEventState::default();
        let result = cutscene_state.dispatch(&cutscene, 0x1);
        assert!(result.forward_to_class);
        assert!(result.enable_hook);
        assert!(cutscene_state.disabled);

        let direct_trigger = camera(0);
        let mut direct_state = NativeCommonTriggerEventState::default();
        direct_state.dispatch(&direct_trigger, 0x10000);
        assert!(!direct_state.disabled);
        direct_state.apply_disable_hook(&direct_trigger);
        assert!(direct_state.disabled);
        assert_eq!(direct_state.last_event, 0x10000);
    }

    #[test]
    fn native_graph_messages_use_serialized_message_block_and_default_0x101() {
        let mut trigger = camera(0);
        trigger.data[8] = Some(0x200);
        trigger.data[9] = Some(0);
        assert_eq!(runtime_trigger_deferred_link_event(&trigger, 0), 0x200);
        assert_eq!(runtime_trigger_deferred_link_event(&trigger, 1), 0x101);
        assert_eq!(runtime_trigger_deferred_link_event(&trigger, 2), 0x101);
    }

    #[test]
    fn native_counter_and_timer_match_deferred_fire_rules() {
        let mut counter = RuntimeTriggerGraphState::default();
        counter.dispatch_counter(2, ROBOTS_EVENT_ACTIVATE);
        assert_eq!(counter.counter_value, 1);
        assert!(!counter.deferred_fire);
        counter.dispatch_counter(2, ROBOTS_EVENT_ACTIVATE);
        assert_eq!(counter.counter_value, 2);
        assert!(counter.deferred_fire);
        counter.dispatch_counter(2, ROBOTS_EVENT_DEACTIVATE);
        assert_eq!(counter.counter_value, 1);
        counter.dispatch_counter(2, 0x1000);
        assert_eq!(counter.counter_value, 0);

        let mut timer = RuntimeTriggerGraphState::default();
        timer.dispatch_timer(ROBOTS_EVENT_ACTIVATE);
        for _ in 0..60 {
            timer.advance_timer_fixed(1.0);
            assert!(!timer.deferred_fire);
        }
        timer.advance_timer_fixed(1.0);
        assert!(timer.deferred_fire);
        assert!(!timer.timer_running);
        assert_eq!(timer.timer_elapsed, 0.0);
    }

    #[test]
    fn native_pattern_uses_first_mask_and_f32_fixed_tick_crossing() {
        let masks = [0x55, 0xAA];
        let mut pattern = RuntimeTriggerGraphState::default();
        pattern.initialize_pattern(true, &masks);
        assert_eq!(pattern.pattern_index, 0);
        assert_eq!(pattern.pattern_mask, 0x55);

        for _ in 0..120 {
            assert_eq!(pattern.advance_pattern_fixed(2.0, &masks), None);
        }
        assert_eq!(pattern.advance_pattern_fixed(2.0, &masks), Some(0xAA));
        assert_eq!(pattern.pattern_index, 1);
        assert_eq!(pattern.pattern_elapsed, 0.0);
    }

    #[test]
    fn native_pattern_is_a_production_runtime_event_type() {
        let mut trigger = camera(0);
        trigger.ttype = ROBOTS_PATTERN_TYPE;
        let map = ProcessedMap {
            triggers: vec![trigger.clone()],
            ..Default::default()
        };
        assert!(MapFrame::runtime_event_supported(&map, &trigger));
    }

    #[test]
    fn native_watchbot_is_a_production_runtime_event_type() {
        let mut trigger = camera(0);
        trigger.ttype = ROBOTS_WATCHBOT_TYPE;
        trigger.data[0] = Some(3);
        trigger.data[1] = Some(ROBOTS_WATCHBOT_NO_PATH_UID);
        trigger.data[2] = Some(0);
        trigger.data[3] = Some(15);
        trigger.data[4] = Some(30);
        let map = ProcessedMap {
            triggers: vec![trigger.clone()],
            ..Default::default()
        };
        assert!(MapFrame::runtime_event_supported(&map, &trigger));
    }

    #[test]
    fn native_mission_is_a_production_runtime_event_type() {
        let mut trigger = camera(0);
        trigger.ttype = ROBOTS_MISSION_TYPE;
        let map = ProcessedMap {
            triggers: vec![trigger.clone()],
            ..Default::default()
        };
        assert!(MapFrame::runtime_event_supported(&map, &trigger));
    }

    #[test]
    fn native_deferred_graph_types_are_production_runtime_event_types() {
        for ttype in [
            ROBOTS_COUNTER_TYPE,
            ROBOTS_TIMER_TYPE,
            ROBOTS_MESSAGE_RELAY_TYPE,
        ] {
            let mut trigger = camera(0);
            trigger.ttype = ttype;
            let map = ProcessedMap {
                triggers: vec![trigger],
                ..Default::default()
            };
            assert!(MapFrame::runtime_event_supported(&map, &map.triggers[0]));
        }
    }

    #[test]
    fn camera_sequence_is_a_production_runtime_event_type() {
        let mut sequence = camera(0);
        sequence.ttype = ROBOTS_CAMERA_SEQUENCE_TYPE;
        let map = ProcessedMap {
            triggers: vec![sequence],
            ..Default::default()
        };
        assert!(MapFrame::runtime_event_supported(&map, &map.triggers[0]));
    }

    #[test]
    fn camera_event_reducer_uses_native_owner_nesting_and_release_predicate() {
        let map = ProcessedMap {
            triggers: vec![camera(0x0B00_0010), camera(0x0B00_0020)],
            ..Default::default()
        };
        let mut ownership = NativeCameraOwnershipRuntime::default();

        let active = active_camera_trigger_after_event(
            &map,
            &mut ownership,
            None,
            0,
            1,
            ROBOTS_EVENT_ACTIVATE,
        );
        assert_eq!(active, Some(0));
        assert_eq!(ownership.owner_index, Some(0));
        assert_eq!(ownership.nesting_count, 1);

        let active = active_camera_trigger_after_event(
            &map,
            &mut ownership,
            active,
            1,
            1,
            ROBOTS_EVENT_DEACTIVATE,
        );
        assert_eq!(active, Some(0));
        assert_eq!(ownership.nesting_count, 1);

        let active = active_camera_trigger_after_event(
            &map,
            &mut ownership,
            active,
            0,
            1,
            ROBOTS_EVENT_ACTIVATE | ROBOTS_EVENT_DEACTIVATE,
        );
        assert_eq!(active, Some(0));
        assert_eq!(ownership.owner_index, Some(0));
        assert_eq!(ownership.nesting_count, 1);

        let active = active_camera_trigger_after_event(
            &map,
            &mut ownership,
            active,
            0,
            1,
            ROBOTS_EVENT_DEACTIVATE,
        );
        assert_eq!(active, None);
        assert_eq!(ownership, NativeCameraOwnershipRuntime::default());
    }
}
