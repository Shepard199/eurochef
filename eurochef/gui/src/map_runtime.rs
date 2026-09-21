use std::{collections::BTreeMap, sync::Arc};

use eurochef_edb::Hashcode;
pub(crate) use eurochef_shared::robots_runtime::mission::RobotsMissionStatus as NativeMissionStatus;
use eurochef_shared::robots_runtime::{
    ball_track::{RobotsBallTrackEventStep, RobotsBallTrackRuntimeState},
    cutscene::{
        apply_cutscene_change_level_route_toggle, apply_cutscene_message_toggle,
        initial_cutscene_finalize_route_flags, select_cutscene_script, RobotsCutsceneAudioScan,
        RobotsCutsceneEffect, RobotsCutsceneFinalizeEffect, RobotsCutsceneHandlerHostEffect,
        RobotsCutsceneHandlerHostInput, RobotsCutsceneHandlerStep, RobotsCutsceneScriptSelection,
        RobotsCutsceneStateMarkerEventResult, RobotsCutsceneStateMarkerRuntime,
        RobotsCutsceneToggle,
    },
    door::RobotsDoorRuntimeState,
    fix_switch::{RobotsFixSwitchEventStep, RobotsFixSwitchRuntimeState},
    hit_candidate_policy::{
        classify_hit_query_candidate, RobotsHitQueryCandidateContext,
        RobotsHitQueryCandidateDecision, RobotsHitQueryCandidateRejectReason,
        RobotsHitQueryCandidateView, RobotsHitQueryNarrowphaseKind,
    },
    hit_narrowphase::{execute_hit_sample_sweep, RobotsHitSampleSweepPlan, RobotsHitSweepSample},
    hit_shapes::{
        robots_hit_shapes_intersect, robots_resolve_hit_shape, RobotsHitLocalShape, RobotsHitShape,
        RobotsResolvedHitDatumPose,
    },
    npc::{RobotsNpcRuntimeState, RobotsNpcTutorialInteractionState},
    pickup::RobotsPickupRuntimeState,
    script_scheduler::{
        advance_script_scheduler, RobotsScriptNativeEventResult, RobotsScriptSchedulerState,
        RobotsScriptSchedulerStep,
    },
    trigger_links::robots_trigger_link_index,
    watchbot::RobotsWatchbotTriggerRuntime,
};
use eurochef_shared::script::UXGeoScript;
use glam::{Mat4, Quat, Vec3};

use crate::{
    entities::RobotsRaycastTriangle,
    map_frame::QueuedEntityRender,
    map_zone::robots_map_zone_indices_for_segment_by_bsp,
    maps::{
        robots_character_hit_query_raw_group, robots_trigger_path_hash,
        robots_trigger_platform_angular_velocity, robots_trigger_runtime_path_acceleration,
        robots_trigger_runtime_path_speed, ProcessedCharacterAnimationBonePose,
        ProcessedCharacterAnimationTrack, ProcessedCharacterCollisionProfile,
        ProcessedCharacterCollisionShape, ProcessedCharacterRootMotionSample,
        ProcessedCharacterVisual, ProcessedMap, ProcessedPath, ProcessedTrigger,
    },
    render::RenderStore,
};
pub(crate) fn map_trigger_link_index(link: i32, trigger_count: usize) -> Option<usize> {
    robots_trigger_link_index(link, trigger_count)
}

pub(crate) fn map_trigger_by_link(
    map: &ProcessedMap,
    link: i32,
) -> Option<(usize, &ProcessedTrigger)> {
    let index = map_trigger_link_index(link, map.triggers.len())?;
    map.triggers.get(index).map(|trigger| (index, trigger))
}

#[allow(dead_code)]
pub(crate) const ROBOTS_GLOBAL_RNG_FRESH_PROCESS_SEED: u32 = 0x955;
#[allow(dead_code)]
pub(crate) const ROBOTS_GLOBAL_RNG_FLOAT_SCALE: f32 = f32::from_bits(0x3000_0000);

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeRobotsGlobalRngProvenance {
    /// A standalone map/editor session has no proof of the executable-global
    /// `0x007BE1E4` seed after prior process-wide consumers advanced it.
    UnknownSession,
    /// Exact startup state installed by the process initializer at 0x00509C34.
    FreshProcessStartup,
    /// Seed captured/provided at a known native runtime boundary.
    ObservedSeed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeRobotsGlobalRngState {
    seed: Option<u32>,
    pub(crate) provenance: RuntimeRobotsGlobalRngProvenance,
    pub(crate) draws_from_anchor: u64,
}

impl Default for RuntimeRobotsGlobalRngState {
    fn default() -> Self {
        Self {
            seed: None,
            provenance: RuntimeRobotsGlobalRngProvenance::UnknownSession,
            draws_from_anchor: 0,
        }
    }
}

#[allow(dead_code)]
impl RuntimeRobotsGlobalRngState {
    pub(crate) fn fresh_process_startup() -> Self {
        Self {
            seed: Some(ROBOTS_GLOBAL_RNG_FRESH_PROCESS_SEED),
            provenance: RuntimeRobotsGlobalRngProvenance::FreshProcessStartup,
            draws_from_anchor: 0,
        }
    }

    pub(crate) fn from_observed_seed(seed: u32) -> Self {
        Self {
            seed: Some(seed),
            provenance: RuntimeRobotsGlobalRngProvenance::ObservedSeed,
            draws_from_anchor: 0,
        }
    }

    pub(crate) fn seed(&self) -> Option<u32> {
        self.seed
    }

    pub(crate) fn invalidate(&mut self) {
        *self = Self::default();
    }

    /// Exact Robots.exe `FUN_00509C48(uint *seed)`. One public draw performs two
    /// wrapping LCG updates against the same seed cell and returns their mixed high words.
    pub(crate) fn next_u32(&mut self) -> Option<u32> {
        let seed = self.seed?;
        let first = seed.wrapping_mul(0x343FD).wrapping_add(0x269EC3);
        let second = first.wrapping_mul(0x343FD).wrapping_add(0x269EC3);
        self.seed = Some(second);
        self.draws_from_anchor = self.draws_from_anchor.wrapping_add(1);
        Some((second >> 16) | (first & 0xFFFF_0000))
    }

    /// Exact `FUN_00509C6E`: `(FUN_00509C48(seed) >> 1) * 2^-31`.
    pub(crate) fn next_unit_f32(&mut self) -> Option<f32> {
        let draw = self.next_u32()?;
        Some(((draw >> 1) as f32) * ROBOTS_GLOBAL_RNG_FLOAT_SCALE)
    }
}

pub(crate) const ROBOTS_TRIGGER_PLAYER: u32 = 0;
pub(crate) const ROBOTS_PLAYER_SPECIAL_SPAWN_Y_OFFSET: f32 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RuntimePlayerState {
    pub(crate) trigger_index: usize,
    pub(crate) mode: u32,
    pub(crate) position: Vec3,
    pub(crate) rotation: Quat,
}

pub(crate) fn runtime_player_spawn_position(position: Vec3, mode: u32) -> Vec3 {
    let mut position = position;
    if matches!(mode, 1 | 2 | 3) {
        position.y += ROBOTS_PLAYER_SPECIAL_SPAWN_Y_OFFSET;
    }
    position
}

pub(crate) fn runtime_player_trigger_state(
    trigger_index: usize,
    trigger: &ProcessedTrigger,
) -> Option<RuntimePlayerState> {
    (trigger.ttype == ROBOTS_TRIGGER_PLAYER).then(|| {
        let mode = trigger.data.get(1).and_then(|value| *value).unwrap_or(0);
        RuntimePlayerState {
            trigger_index,
            mode,
            position: runtime_player_spawn_position(trigger.position, mode),
            rotation: Quat::from_euler(
                glam::EulerRot::ZXY,
                trigger.rotation[2],
                trigger.rotation[0],
                trigger.rotation[1],
            ),
        }
    })
}

pub(crate) fn runtime_player_spawn_state(map: &ProcessedMap) -> Option<RuntimePlayerState> {
    let (trigger_index, trigger) = map
        .triggers
        .iter()
        .enumerate()
        .find(|(_, trigger)| trigger.ttype == ROBOTS_TRIGGER_PLAYER)?;
    runtime_player_trigger_state(trigger_index, trigger)
}

pub(crate) fn runtime_minebot_move_gate(
    minebot_position: Vec3,
    player_position: Vec3,
    was_active: bool,
) -> bool {
    let threshold = if was_active { 16.0 } else { 12.0 };
    minebot_position.distance_squared(player_position) < threshold * threshold
}

pub(crate) const ROBOTS_TRIGGER_NEAR0_RADIUS_SQUARED: f32 = 100.0;
pub(crate) const ROBOTS_TRIGGER_NEAR1_RADIUS_SQUARED: f32 = 400.0;
pub(crate) const ROBOTS_TRIGGER_NEAR2_RADIUS_SQUARED: f32 = 900.0;
pub(crate) const ROBOTS_TRIGGER_CREATE_BUDGET_PER_UPDATE: u32 = 30;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeTriggerDistanceEvent {
    Event0,
    Event1,
    Event2,
    Event3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeCommonTriggerLifecycleAction {
    ImmediateActive,
    DelayedActive { ticks: u8 },
    InactiveState2,
    InactiveState3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RuntimeScriptTriggerLifecycleState {
    pub(crate) xitem_exists: bool,
    pub(crate) xitem_state: u8,
    pub(crate) delay_counter: u8,
    pub(crate) current_opacity: f32,
    pub(crate) target_opacity: f32,
}

impl Default for RuntimeScriptTriggerLifecycleState {
    fn default() -> Self {
        Self {
            xitem_exists: false,
            xitem_state: 0,
            delay_counter: 0,
            current_opacity: 0.0,
            target_opacity: 1.0,
        }
    }
}

impl RuntimeScriptTriggerLifecycleState {
    fn destroy_xitem(&mut self) {
        self.xitem_exists = false;
        self.xitem_state = 0;
        self.current_opacity = 0.0;
        self.target_opacity = 1.0;
    }

    fn ensure_xitem(
        &mut self,
        proximity_factor: f32,
        creates_this_update: &mut u32,
        create_allowed: bool,
    ) {
        if !create_allowed
            || self.xitem_exists
            || *creates_this_update >= ROBOTS_TRIGGER_CREATE_BUDGET_PER_UPDATE
        {
            return;
        }
        // XItem ctor 0x00443BD0 initializes +0x258=1 and +0x25C=0.
        self.xitem_exists = true;
        self.xitem_state = 0;
        self.target_opacity = 1.0;
        self.current_opacity = 0.0;
        *creates_this_update += 1;
        // Trigger finalizer 0x0047D890 forces current opacity to 1 only when
        // the per-update proximity factor written at 0x0044C10A is < 0.75.
        if proximity_factor < 0.75 {
            self.current_opacity = 1.0;
        }
    }

    pub(crate) fn cleanup_zone_failed(&mut self) {
        // 0x0044BE10 passes cleanup(1): this bypasses opacity retention.
        if self.xitem_exists {
            self.destroy_xitem();
        }
    }

    pub(crate) fn would_attempt_create(
        &self,
        action: RuntimeCommonTriggerLifecycleAction,
        creates_this_update: u32,
    ) -> bool {
        matches!(
            action,
            RuntimeCommonTriggerLifecycleAction::ImmediateActive
                | RuntimeCommonTriggerLifecycleAction::DelayedActive { .. }
        ) && !self.xitem_exists
            && creates_this_update < ROBOTS_TRIGGER_CREATE_BUDGET_PER_UPDATE
    }

    pub(crate) fn advance_trigger_manager(
        &mut self,
        action: RuntimeCommonTriggerLifecycleAction,
        flags: u32,
        proximity_factor: f32,
        creates_this_update: &mut u32,
    ) {
        self.advance_trigger_manager_with_create_gate(
            action,
            flags,
            proximity_factor,
            creates_this_update,
            true,
        );
    }

    pub(crate) fn advance_trigger_manager_with_create_gate(
        &mut self,
        action: RuntimeCommonTriggerLifecycleAction,
        flags: u32,
        proximity_factor: f32,
        creates_this_update: &mut u32,
        create_allowed: bool,
    ) {
        match action {
            RuntimeCommonTriggerLifecycleAction::ImmediateActive => {
                self.ensure_xitem(proximity_factor, creates_this_update, create_allowed);
                if self.xitem_exists {
                    self.xitem_state = 0;
                }
            }
            RuntimeCommonTriggerLifecycleAction::DelayedActive { ticks } => {
                if flags & 0x0000_0100 != 0 {
                    self.ensure_xitem(proximity_factor, creates_this_update, create_allowed);
                    if self.xitem_exists {
                        self.xitem_state = 0;
                    }
                    return;
                }
                self.ensure_xitem(proximity_factor, creates_this_update, create_allowed);
                if self.xitem_exists {
                    self.xitem_state = 1;
                }
                // 0x0044D150 increments trigger+0xE0 even when the create
                // budget prevented XItem creation or the class CreateItem returned null.
                self.delay_counter = self.delay_counter.wrapping_add(1);
                if self.delay_counter >= ticks {
                    self.delay_counter = 0;
                }
            }
            RuntimeCommonTriggerLifecycleAction::InactiveState2
            | RuntimeCommonTriggerLifecycleAction::InactiveState3 => {
                if !self.xitem_exists {
                    return;
                }
                self.xitem_state = match action {
                    RuntimeCommonTriggerLifecycleAction::InactiveState2 => 2,
                    RuntimeCommonTriggerLifecycleAction::InactiveState3 => 3,
                    _ => unreachable!(),
                };
                // Runtime loader sets trigger bit26 (0x04000000). cleanup(0)
                // at 0x0047D840 therefore retains the XItem while +0x25C>0,
                // returning before bit29 is set. Once opacity reaches zero the
                // next TriggerManager tick destroys it.
                if self.current_opacity <= 0.0 {
                    self.destroy_xitem();
                }
            }
        }
    }

    pub(crate) fn advance_xitem(&mut self, zone_visual_active: bool) {
        if !self.xitem_exists {
            return;
        }
        self.target_opacity = match self.xitem_state {
            0 | 1 => 1.0,
            2 if zone_visual_active => 1.0,
            2 | 3 => 0.0,
            _ => self.target_opacity,
        };
        self.current_opacity =
            runtime_xitem_opacity_fixed_update(self.current_opacity, self.target_opacity);
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct NativeCutsceneRuntimeState {
    pub(crate) lifecycle: RuntimeScriptTriggerLifecycleState,
    /// `XTrigger_Cutscene +0xE4`: -1 is represented by `None`. NPC activation
    /// writes this before event 0x101; CreateItem consumes a non-null override
    /// into the live Cutscene handler and resets the trigger field to -1.
    pub(crate) pending_alternate_uid: Option<u32>,
    pub(crate) pending_activation: bool,
    pub(crate) active_alternate_uid: Option<u32>,
    /// Exact `0x00409730` Script selection captured when the Cutscene XItem is
    /// created. `handler+0x3B8` overrides the default trigger Script UID but
    /// does not change the selected owner EDB/file.
    pub(crate) active_script: Option<RobotsCutsceneScriptSelection>,
    /// Host-resolved timeline position for the selected Script. `None` means
    /// the selected native resource is not currently available in RenderStore;
    /// runtime effect execution must fail closed in that case.
    pub(crate) active_script_time_seconds: Option<f32>,
    /// Native `EXItemAnimator_Script` event/control scheduler. It owns the
    /// serialized frame and command cursor instead of advancing a decorative
    /// Cutscene timer independently from opcode11 dispatch.
    pub(crate) script_scheduler: RobotsScriptSchedulerState,
    /// Creator `XTrigger_Cutscene +0x40`: persisted Script time in seconds. The
    /// base XTrigger ctor initializes 0.0; vslot +0x70 (`0x00487F30`) copies
    /// live Handler `+0x3A0` here before trigger Save.
    pub(crate) creator_saved_time_bits_40: u32,
    /// Native Cutscene StateMarker table/latches (`+0x398/+0x39C`,
    /// `+0x16B0`, `+0x16B8..+0x16BC`). This stays separate from the generic
    /// Script scheduler because the same held Event is revisited across Handler ticks.
    pub(crate) state_marker_runtime: RobotsCutsceneStateMarkerRuntime,
    /// Due Cutscene-specific host effects emitted by opcode11 commands reached
    /// during the most recent XItem update. The GUI/UE adapter consumes these;
    /// shared gameplay remains engine-neutral.
    pub(crate) pending_effects: Vec<RobotsCutsceneEffect>,
    /// Cutscene Handler lifecycle effects that are not opcode11 payload effects.
    /// Fade requests are consumed by the GUI host immediately; FinalizeCutscene
    /// remains explicit for the later full 0x004075C0 host adapter.
    pub(crate) pending_handler_effects: Vec<RobotsCutsceneHandlerHostEffect>,
    /// Finalizer-relevant subset of native creator `XTrigger_Cutscene +0xE8`.
    /// Bit1 routes finalization to linked ChangeLevel; bit2 routes to linked Cutscene.
    pub(crate) creator_runtime_flags_e8: u8,
    pub(crate) has_change_level_link: bool,
    pub(crate) creator_routes_initialized: bool,
    /// Cutscene Handler-local fields consumed by `0x004075C0`.
    pub(crate) message_enabled_16be: bool,
    pub(crate) saved_display_mask_16b4: u32,
    pub(crate) pending_swap_character_16bd: bool,
    pub(crate) audio_active_16c3: bool,
    pub(crate) music_audio_present_16c4: bool,
    pub(crate) script_audio_scan_initialized: bool,
    /// Typed `0x004075C0` host effects from the most recent finalization. Keep
    /// them after routed release so the GUI/UE boundary does not silently drop
    /// native side effects that are intentionally implemented outside this reducer.
    pub(crate) pending_finalize_effects: Vec<RobotsCutsceneFinalizeEffect>,
    /// `XTrigger_NPC::ActivateCutscene` can overwrite the linked Cutscene trigger
    /// transform from the current live NPC XItem according to NPC flags 0x100/0x200.
    pub(crate) position_override: Option<Vec3>,
    pub(crate) rotation_override: Option<Quat>,
    pub(crate) activation_requests: u32,
}

impl NativeCutsceneRuntimeState {
    pub(crate) fn prepare_npc_activation(
        &mut self,
        alternate_uid: Option<u32>,
        position_override: Option<Vec3>,
        rotation_override: Option<Quat>,
    ) {
        self.pending_alternate_uid = alternate_uid;
        self.pending_activation = true;
        self.position_override = position_override;
        self.rotation_override = rotation_override;
        self.activation_requests = self.activation_requests.wrapping_add(1);
    }

    pub(crate) fn ensure_finalize_routes_initialized(
        &mut self,
        has_change_level_link: bool,
        has_cutscene_link: bool,
        suppress_linked_cutscene: bool,
    ) {
        if self.creator_routes_initialized {
            return;
        }
        self.has_change_level_link = has_change_level_link;
        self.creator_runtime_flags_e8 = initial_cutscene_finalize_route_flags(
            has_change_level_link,
            has_cutscene_link,
            suppress_linked_cutscene,
        );
        self.creator_routes_initialized = true;
    }

    pub(crate) fn apply_change_level_route_toggle(&mut self, toggle: RobotsCutsceneToggle) {
        self.creator_runtime_flags_e8 = apply_cutscene_change_level_route_toggle(
            self.creator_runtime_flags_e8,
            toggle,
            self.has_change_level_link,
        );
    }

    pub(crate) fn apply_message_toggle(
        &mut self,
        toggle: RobotsCutsceneToggle,
        creator_data0: u32,
    ) {
        self.message_enabled_16be =
            apply_cutscene_message_toggle(self.message_enabled_16be, toggle, creator_data0);
    }

    pub(crate) fn apply_swap_character_mode(&mut self, mode: Option<i32>) {
        if mode == Some(1) {
            self.pending_swap_character_16bd = true;
        }
    }

    pub(crate) fn cleanup_zone_failed(&mut self) {
        self.lifecycle.cleanup_zone_failed();
        if !self.lifecycle.xitem_exists {
            self.active_alternate_uid = None;
            self.active_script = None;
            self.active_script_time_seconds = None;
            self.script_scheduler.reset();
            self.state_marker_runtime.reset();
            self.pending_effects.clear();
            self.pending_handler_effects.clear();
        }
    }

    pub(crate) fn advance_trigger_manager(
        &mut self,
        action: RuntimeCommonTriggerLifecycleAction,
        flags: u32,
        proximity_factor: f32,
        creates_this_update: &mut u32,
    ) -> bool {
        let existed_before = self.lifecycle.xitem_exists;
        self.lifecycle.advance_trigger_manager(
            action,
            flags,
            proximity_factor,
            creates_this_update,
        );
        let created_now = !existed_before && self.lifecycle.xitem_exists;
        if created_now {
            self.script_scheduler.reset();
            self.state_marker_runtime.reset();
            self.pending_effects.clear();
            self.pending_handler_effects.clear();
            self.pending_finalize_effects.clear();
            self.message_enabled_16be = true;
            self.saved_display_mask_16b4 = 0;
            self.pending_swap_character_16bd = false;
            self.audio_active_16c3 = false;
            self.music_audio_present_16c4 = false;
            self.script_audio_scan_initialized = false;
            if self.pending_activation {
                self.active_alternate_uid = self.pending_alternate_uid.take();
                self.pending_activation = false;
            } else {
                self.active_alternate_uid = None;
            }
        }
        created_now
    }

    pub(crate) fn bind_script_on_create(
        &mut self,
        current_file_uid: u32,
        explicit_file_uid: Option<u32>,
        default_script_uid: Option<u32>,
    ) {
        self.active_script = select_cutscene_script(
            current_file_uid,
            explicit_file_uid,
            default_script_uid,
            self.active_alternate_uid,
        );
        self.active_script_time_seconds = None;
        self.script_scheduler.reset();
        self.state_marker_runtime.reset();
        self.pending_effects.clear();
        self.pending_handler_effects.clear();
        self.audio_active_16c3 = false;
        self.music_audio_present_16c4 = false;
        self.script_audio_scan_initialized = false;
    }

    pub(crate) fn bind_script_audio_scan(&mut self, scan: RobotsCutsceneAudioScan) {
        if !self.lifecycle.xitem_exists {
            return;
        }
        self.audio_active_16c3 = scan.streamed_sfx_present_16c3;
        self.music_audio_present_16c4 = scan.music_present_16c4;
        self.script_audio_scan_initialized = true;
    }

    pub(crate) fn advance_cutscene_handler(
        &mut self,
        input: RobotsCutsceneHandlerHostInput,
    ) -> RobotsCutsceneHandlerStep {
        if !self.lifecycle.xitem_exists {
            return RobotsCutsceneHandlerStep::default();
        }
        self.state_marker_runtime.advance_handler(input)
    }

    pub(crate) fn record_finalize_effects(&mut self, effects: Vec<RobotsCutsceneFinalizeEffect>) {
        self.pending_finalize_effects = effects;
    }

    pub(crate) fn complete_ordinary_finalize(&mut self) {
        if self.lifecycle.xitem_exists {
            self.state_marker_runtime.complete_ordinary_finalize();
            self.audio_active_16c3 = false;
            self.saved_display_mask_16b4 = 0;
            self.pending_swap_character_16bd = false;
        }
    }

    pub(crate) fn release_after_finalize(&mut self) {
        self.lifecycle.destroy_xitem();
        self.active_alternate_uid = None;
        self.active_script = None;
        self.active_script_time_seconds = None;
        self.script_scheduler.reset();
        self.state_marker_runtime.reset();
        self.pending_effects.clear();
        self.pending_handler_effects.clear();
        self.message_enabled_16be = false;
        self.saved_display_mask_16b4 = 0;
        self.pending_alternate_uid = None;
        self.pending_activation = false;
        self.pending_swap_character_16bd = false;
        self.audio_active_16c3 = false;
        self.music_audio_present_16c4 = false;
        self.script_audio_scan_initialized = false;
    }

    pub(crate) fn advance_script<F>(
        &mut self,
        script: &UXGeoScript,
        mut event_handler: F,
    ) -> RobotsScriptSchedulerStep
    where
        F: FnMut(
            eurochef_shared::robots_runtime::events::RobotsScriptEventView<'_>,
        ) -> RobotsScriptNativeEventResult,
    {
        if !self.lifecycle.xitem_exists {
            return RobotsScriptSchedulerStep::default();
        }
        self.state_marker_runtime.bind_script(script);
        let creator_saved_time_bits_40 = self.creator_saved_time_bits_40;
        let state_marker_runtime = &mut self.state_marker_runtime;
        advance_script_scheduler(script, &mut self.script_scheduler, 1.0, |event| {
            if event.event_type == eurochef_shared::robots_runtime::events::event_type::STATE_MARKER
            {
                let Some(start) = event.start else {
                    return RobotsScriptNativeEventResult::Hold;
                };
                return match state_marker_runtime.on_state_marker(start) {
                    RobotsCutsceneStateMarkerEventResult::Continue => {
                        RobotsScriptNativeEventResult::Continue
                    }
                    RobotsCutsceneStateMarkerEventResult::Hold => {
                        RobotsScriptNativeEventResult::Hold
                    }
                };
            }
            if event.event_type
                == eurochef_shared::robots_runtime::events::event_type::SET_ALTERNATE_STATE
            {
                return state_marker_runtime
                    .set_alternate_state_seek_frame_bits(creator_saved_time_bits_40)
                    .map_or(RobotsScriptNativeEventResult::Continue, |frame_bits| {
                        RobotsScriptNativeEventResult::SeekToFrameBits { frame_bits }
                    });
            }
            event_handler(event)
        })
    }

    pub(crate) fn advance_xitem(&mut self, zone_visual_active: bool) {
        let existed_before = self.lifecycle.xitem_exists;
        self.lifecycle.advance_xitem(zone_visual_active);
        if existed_before && !self.lifecycle.xitem_exists {
            self.active_alternate_uid = None;
            self.active_script = None;
            self.active_script_time_seconds = None;
            self.script_scheduler.reset();
            self.state_marker_runtime.reset();
            self.pending_effects.clear();
            self.pending_handler_effects.clear();
        }
    }
}

pub(crate) const ROBOTS_AI_HIT_FATAL_FADE_SECONDS: f32 = 0.1;
pub(crate) const ROBOTS_AI_HIT_FATAL_FIXED_STEP_SECONDS: f32 = 1.0 / 60.0;
pub(crate) const ROBOTS_AI_HIT_FATAL_DESTROY_EPSILON: f32 = 0.001;

/// Exact owned subset of the common `AI_HitFatal` behavior installed by
/// `0x00457E30` with vtable `0x005E240C`.
///
/// `0x00457F70` recognizes the normal fatal-hit edge (`+0x608 != 0 &&
/// +0x62E == 0`). The behavior event slot `0x00458150` handles
/// `HT_ScriptEvents_MonsterExplosion (0x16000039)` and raises local `+0x4C`
/// only after the explosion action succeeds. `0x00458060` then fades the
/// XItem from 0.1 seconds and finally requests handler `+0x12C`; the common AI
/// implementation `0x00453640 -> 0x00443EE0` queues deferred XItem destruction.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RuntimeNativeAiHitFatalState {
    pub(crate) monster_explosion_completed: bool,
    pub(crate) fade_remaining: f32,
}

impl Default for RuntimeNativeAiHitFatalState {
    fn default() -> Self {
        Self {
            monster_explosion_completed: false,
            fade_remaining: ROBOTS_AI_HIT_FATAL_FADE_SECONDS,
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct RuntimeNativeAiHitFatalPlan {
    pub(crate) opacity_write: Option<f32>,
    pub(crate) request_destroy: bool,
    pub(crate) fade_remaining: f32,
}

#[allow(dead_code)]
impl RuntimeNativeAiHitFatalState {
    pub(crate) fn normal_fatal_hit_condition(hit_latch: bool, hit_points: u8) -> bool {
        hit_latch && hit_points == 0
    }

    /// Mirrors `0x00458150`: the death fade is armed only if the native
    /// MonsterExplosion action returned success.
    pub(crate) fn apply_monster_explosion_event(&mut self, action_succeeded: bool) -> bool {
        if !action_succeeded || self.monster_explosion_completed {
            return false;
        }
        self.monster_explosion_completed = true;
        true
    }

    /// Pure owned part of `0x00458060`. Hosts apply the resulting +0x12C request
    /// to their own live-XItem registry/lifecycle instead of duplicating fade logic.
    /// `below_floor_cleanup` is the separately proven `+0x5C` fast path raised
    /// by `0x00457FE0`; the normal fatal-hit path waits for MonsterExplosion and
    /// performs the 0.1-second fade first.
    pub(crate) fn advance_fixed_tick_plan(
        &mut self,
        below_floor_cleanup: bool,
    ) -> RuntimeNativeAiHitFatalPlan {
        if below_floor_cleanup {
            return RuntimeNativeAiHitFatalPlan {
                opacity_write: None,
                request_destroy: true,
                fade_remaining: self.fade_remaining,
            };
        }

        if !self.monster_explosion_completed {
            return RuntimeNativeAiHitFatalPlan {
                opacity_write: None,
                request_destroy: false,
                fade_remaining: self.fade_remaining,
            };
        }

        let opacity_write = (self.fade_remaining < ROBOTS_AI_HIT_FATAL_FADE_SECONDS)
            .then(|| (self.fade_remaining / ROBOTS_AI_HIT_FATAL_FADE_SECONDS).clamp(0.0, 1.0));

        if self.fade_remaining >= ROBOTS_AI_HIT_FATAL_DESTROY_EPSILON {
            self.fade_remaining -= ROBOTS_AI_HIT_FATAL_FIXED_STEP_SECONDS;
        }

        RuntimeNativeAiHitFatalPlan {
            opacity_write,
            request_destroy: self.fade_remaining < ROBOTS_AI_HIT_FATAL_DESTROY_EPSILON,
            fade_remaining: self.fade_remaining,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct RuntimeCameraBit0TriggerState {
    pub(crate) lifecycle: RuntimeScriptTriggerLifecycleState,
    /// Native EXItemAnimator_Script +0x104, expressed in serialized Script frames.
    pub(crate) script_frame: f32,
}

impl RuntimeCameraBit0TriggerState {
    pub(crate) fn cleanup_zone_failed(&mut self) {
        self.lifecycle.cleanup_zone_failed();
        if !self.lifecycle.xitem_exists {
            self.script_frame = 0.0;
        }
    }

    pub(crate) fn advance_trigger_manager(
        &mut self,
        action: RuntimeCommonTriggerLifecycleAction,
        flags: u32,
        proximity_factor: f32,
        creates_this_update: &mut u32,
    ) {
        let existed = self.lifecycle.xitem_exists;
        self.lifecycle.advance_trigger_manager(
            action,
            flags,
            proximity_factor,
            creates_this_update,
        );
        if !existed && self.lifecycle.xitem_exists {
            self.script_frame = 0.0;
        } else if !self.lifecycle.xitem_exists {
            self.script_frame = 0.0;
        }
    }

    pub(crate) fn advance_xitem_before_camera(&mut self, zone_visual_active: bool) {
        self.lifecycle.advance_xitem(zone_visual_active);
        if self.lifecycle.xitem_exists {
            // EXItemAnimator base ctor 0x004E8C0D sets speed +0x100=1.0;
            // Script slot +0x10 0x004FA5A8 adds DAT_00620050(1.0)*speed.
            // BossSewerCanon priority 0x14 sorts before Camera owner 0x64.
            self.script_frame += 1.0;
        } else {
            self.script_frame = 0.0;
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct NativeCommonTriggerEventResult {
    pub(crate) forward_to_class: bool,
    pub(crate) enable_hook: bool,
    pub(crate) disable_hook: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct NativeCommonTriggerEventState {
    pub(crate) initialized: bool,
    pub(crate) disabled: bool,
    pub(crate) toggle_bit1: bool,
    pub(crate) linked_initialized: bool,
    pub(crate) last_event: u32,
}

impl NativeCommonTriggerEventState {
    fn ensure_initialized(&mut self, trigger: &ProcessedTrigger) {
        if self.initialized {
            return;
        }
        self.initialized = true;
        self.disabled = trigger.game_flags & 1 != 0;
        self.toggle_bit1 = trigger.game_flags & 2 != 0;
    }

    pub(crate) fn apply_disable_hook(&mut self, trigger: &ProcessedTrigger) {
        self.ensure_initialized(trigger);
        self.disabled = true;
    }

    pub(crate) fn dispatch(
        &mut self,
        trigger: &ProcessedTrigger,
        event_mask: u32,
    ) -> NativeCommonTriggerEventResult {
        self.ensure_initialized(trigger);
        self.last_event = event_mask;
        if event_mask & 0x10000 != 0 {
            self.linked_initialized = true;
        }

        // 0x0044C3BD..0x0044C3D9: event 4/8 toggles runtime bit1
        // according to its current state, not by simply assigning from the mask.
        if self.toggle_bit1 {
            if event_mask & 8 != 0 {
                self.toggle_bit1 = false;
            }
        } else if event_mask & 4 != 0 {
            self.toggle_bit1 = true;
        }

        if self.disabled {
            if event_mask & 1 == 0 {
                return NativeCommonTriggerEventResult::default();
            }
            // Base +0x1C clears disabled bit0. Cutscene overrides that hook and
            // deliberately keeps the latch set when runtime/game bit28 is present.
            // ProcessedTrigger.ttype is the serialized type code: Cutscene is 19.
            if trigger.ttype != 19 || trigger.game_flags & 0x1000_0000 == 0 {
                self.disabled = false;
            }
            return NativeCommonTriggerEventResult {
                forward_to_class: true,
                enable_hook: true,
                disable_hook: false,
            };
        }

        if event_mask & 2 != 0 {
            self.disabled = true;
            return NativeCommonTriggerEventResult {
                forward_to_class: false,
                enable_hook: false,
                disable_hook: true,
            };
        }

        NativeCommonTriggerEventResult {
            forward_to_class: true,
            enable_hook: false,
            disable_hook: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum NativeMissionOwnerRelation {
    #[default]
    None,
    SelfTrigger,
    OtherTrigger,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum NativeMissionOwnerAction {
    #[default]
    Keep,
    SetSelf,
    Clear,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct NativeMissionDispatchOutcome {
    pub(crate) owner_action: NativeMissionOwnerAction,
    pub(crate) status_update: Option<NativeMissionStatus>,
    pub(crate) output_slot: Option<usize>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct NativeMissionTickOutcome {
    pub(crate) dispatch: NativeMissionDispatchOutcome,
    pub(crate) clock_activate: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct RuntimeTriggerGraphState {
    pub(crate) counter_value: u32,
    pub(crate) timer_running: bool,
    pub(crate) timer_elapsed: f32,
    pub(crate) deferred_fire: bool,
    pub(crate) active: bool,
    pub(crate) initialized: bool,
    pub(crate) pattern_index: usize,
    pub(crate) pattern_elapsed: f32,
    pub(crate) pattern_mask: u8,
    pub(crate) door: RobotsDoorRuntimeState,
    pub(crate) fix_switch: RobotsFixSwitchRuntimeState,
    pub(crate) binary_state_e4: Option<u8>,
    pub(crate) tutorial_interaction: RobotsNpcTutorialInteractionState,
    pub(crate) tutorial_repeat_ea: u8,
    pub(crate) npc: RobotsNpcRuntimeState,
    pub(crate) watchbot: RobotsWatchbotTriggerRuntime,
    pub(crate) ball_track: RobotsBallTrackRuntimeState,
    pub(crate) pickup: RobotsPickupRuntimeState,
    pub(crate) display_message_uid: Option<u32>,
    pub(crate) display_message_duration_raw: Option<u32>,
    pub(crate) display_message_requests: u32,
    pub(crate) owned_xitem_active_e4: Option<u8>,
    pub(crate) clock_active_e4: bool,
    pub(crate) mission_initialized: bool,
    pub(crate) mission_timer_e4: f32,
    pub(crate) mission_repeat_e8: i32,
    pub(crate) mission_active_ec: bool,
    pub(crate) mission_hud_ed: bool,
    pub(crate) mission_hud_ee: bool,
}

impl RuntimeTriggerGraphState {
    pub(crate) fn dispatch_clock(&mut self, event_mask: u32) {
        // XTrigger_Clock +0x6C = 0x0048ACE0. Class bits are independent and
        // ordered: 0x100 sets +E4=1, then 0x200 clears +E4=0, so 0x300 ends off.
        if event_mask & 0x100 != 0 {
            self.clock_active_e4 = true;
        }
        if event_mask & 0x200 != 0 {
            self.clock_active_e4 = false;
        }
    }

    fn initialize_mission_state(&mut self) {
        if self.mission_initialized {
            return;
        }
        // XTrigger_Mission ctor 0x0048CB30 seeds +E4 with 0x3A83126F
        // (0.001f), +E8=0 and +EC/+ED/+EE=0.
        self.mission_initialized = true;
        self.mission_timer_e4 = f32::from_bits(0x3A83_126F);
        self.mission_repeat_e8 = 0;
        self.mission_active_ec = false;
        self.mission_hud_ed = false;
        self.mission_hud_ee = false;
    }

    fn start_mission(&mut self, base_timer: f32, repeat_count: i32) {
        // XTrigger_Mission start helper 0x0048F340. Maps has no native
        // mission-HUD object, so it follows the helper's no-object branch and
        // treats the HUD latches as immediately acknowledged.
        self.mission_active_ec = true;
        if base_timer != 0.0 {
            self.mission_timer_e4 = base_timer;
        }
        self.mission_repeat_e8 = if repeat_count < 1 {
            0
        } else {
            repeat_count - 1
        };
        self.mission_hud_ed = true;
        self.mission_hud_ee = true;
    }

    fn merge_mission_outcomes(
        prefix: NativeMissionDispatchOutcome,
        mut final_outcome: NativeMissionDispatchOutcome,
    ) -> NativeMissionDispatchOutcome {
        if final_outcome.owner_action == NativeMissionOwnerAction::Keep {
            final_outcome.owner_action = prefix.owner_action;
        }
        if final_outcome.status_update.is_none() {
            final_outcome.status_update = prefix.status_update;
        }
        final_outcome
    }

    fn stop_and_finalize_mission(
        &mut self,
        flags: u32,
        has_status_manager: bool,
    ) -> NativeMissionDispatchOutcome {
        // 0x0048F2C0 clears the local active/HUD latches. It clears the global
        // current-Mission pointer unless serialized flag 0x2000 requests that
        // ownership be retained.
        self.mission_active_ec = false;
        self.mission_hud_ed = false;
        self.mission_hud_ee = false;
        let owner_action = if flags & 0x2000 == 0 {
            NativeMissionOwnerAction::Clear
        } else {
            NativeMissionOwnerAction::Keep
        };

        // 0x0048F070 selects link 7 for fail/non-positive time and link 6 for
        // completion/positive time, then makes E4 positive again. Mission
        // manager state changes are conditional on the same serialized flags.
        if self.mission_timer_e4 <= 0.0 {
            let status_update = (has_status_manager && ((flags >> 8) as u8 as i8) >= 0)
                .then_some(NativeMissionStatus::Failed);
            self.mission_timer_e4 = self.mission_timer_e4.abs();
            NativeMissionDispatchOutcome {
                owner_action,
                status_update,
                output_slot: Some(7),
            }
        } else {
            let status_update = (has_status_manager && flags & 0x4000 == 0)
                .then_some(NativeMissionStatus::Completed);
            self.mission_timer_e4 = self.mission_timer_e4.abs();
            NativeMissionDispatchOutcome {
                owner_action,
                status_update,
                output_slot: Some(6),
            }
        }
    }

    pub(crate) fn dispatch_mission(
        &mut self,
        base_timer: f32,
        time_add: f32,
        repeat_count: i32,
        flags: u32,
        event_mask: u32,
        mission_status: Option<NativeMissionStatus>,
        owner_relation: NativeMissionOwnerRelation,
    ) -> NativeMissionDispatchOutcome {
        self.initialize_mission_state();
        let has_status_manager = mission_status.is_some();
        let mut outcome = NativeMissionDispatchOutcome::default();

        if event_mask & 0x100 != 0 {
            // 0x0048F110 gives an already-owned Mission absolute precedence.
            // Both the foreign-owner and self-owner branches return before the
            // independent 0x200 test, which matters for combined 0x300 events.
            match owner_relation {
                NativeMissionOwnerRelation::OtherTrigger => return outcome,
                NativeMissionOwnerRelation::SelfTrigger => {
                    self.mission_repeat_e8 -= 1;
                    if time_add > 0.0 {
                        self.mission_timer_e4 += time_add;
                    }
                    return outcome;
                }
                NativeMissionOwnerRelation::None => {}
            }

            match mission_status {
                Some(NativeMissionStatus::Completed) => return outcome,
                Some(NativeMissionStatus::Active) => {
                    if !self.mission_active_ec {
                        self.start_mission(base_timer, repeat_count);
                        outcome.owner_action = NativeMissionOwnerAction::SetSelf;
                        if repeat_count != 0 {
                            return outcome;
                        }
                    }
                    self.mission_repeat_e8 -= 1;
                    if time_add > 0.0 {
                        self.mission_timer_e4 += time_add;
                    }
                    if self.mission_repeat_e8 >= 0 {
                        return outcome;
                    }
                    let final_outcome = self.stop_and_finalize_mission(flags, has_status_manager);
                    return Self::merge_mission_outcomes(outcome, final_outcome);
                }
                Some(NativeMissionStatus::Inactive | NativeMissionStatus::Failed) => {
                    outcome.status_update = Some(NativeMissionStatus::Active);
                }
                None => {}
            }

            if !self.mission_active_ec {
                self.start_mission(base_timer, repeat_count);
                outcome.owner_action = NativeMissionOwnerAction::SetSelf;
            } else {
                let final_outcome = self.stop_and_finalize_mission(flags, has_status_manager);
                return Self::merge_mission_outcomes(outcome, final_outcome);
            }
        }

        if event_mask & 0x200 == 0 {
            return outcome;
        }

        // The 0x200 path negates E4 before the common stop/finalize path.
        self.mission_timer_e4 = -self.mission_timer_e4;
        let final_outcome = self.stop_and_finalize_mission(flags, has_status_manager);
        Self::merge_mission_outcomes(outcome, final_outcome)
    }

    pub(crate) fn advance_mission_fixed(
        &mut self,
        base_timer: f32,
        flags: u32,
        hud_mode_top: Option<u32>,
        player_special_pause: bool,
        objective_condition_met: bool,
        has_status_manager: bool,
    ) -> NativeMissionTickOutcome {
        self.initialize_mission_state();
        let mut outcome = NativeMissionTickOutcome::default();
        if !self.mission_active_ec || !self.mission_hud_ee {
            return outcome;
        }

        if matches!(hud_mode_top, Some(1 | 2 | 3)) {
            if base_timer == 0.0 || flags & 0x400 == 0 {
                return outcome;
            }
        } else if player_special_pause {
            // Native Player handler state byte +0x6DE == 0x1D pauses Mission
            // ticking outside modal UI states.
            return outcome;
        }

        if base_timer != 0.0 {
            let previous = self.mission_timer_e4;
            self.mission_timer_e4 -= f32::from_bits(0x3C88_8889); // exact 1/60
            self.mission_timer_e4 = self.mission_timer_e4.clamp(0.0, f32::MAX);
            outcome.clock_activate = previous == base_timer;
        }

        if flags & 0x200 != 0 && objective_condition_met {
            self.mission_repeat_e8 = -1;
        }
        if self.mission_timer_e4 <= 0.0 || self.mission_repeat_e8 < 0 {
            outcome.dispatch = self.stop_and_finalize_mission(flags, has_status_manager);
        }
        outcome
    }

    pub(crate) fn dispatch_counter(&mut self, threshold: u32, event_mask: u32) {
        if event_mask & 0x100 != 0 {
            self.counter_value = self.counter_value.wrapping_add(1);
            if self.counter_value == threshold {
                self.deferred_fire = true;
            }
        }
        if event_mask & 0x200 != 0 && self.counter_value != 0 {
            self.counter_value -= 1;
        }
        if event_mask & 0x1000 != 0 {
            self.counter_value = 0;
        }
    }

    pub(crate) fn dispatch_timer(&mut self, event_mask: u32) {
        if event_mask & 0x100 != 0 && !self.timer_running {
            self.timer_running = true;
        }
        if event_mask & 0x1000 != 0 && self.timer_running {
            self.timer_running = false;
            self.timer_elapsed = 0.0;
        }
    }

    pub(crate) fn advance_timer_fixed(&mut self, duration: f32) {
        if !self.timer_running {
            return;
        }
        self.timer_elapsed += 1.0 / 60.0;
        if self.timer_elapsed >= duration {
            self.timer_running = false;
            self.timer_elapsed = 0.0;
            self.deferred_fire = true;
        }
    }

    pub(crate) fn initialize_pattern(&mut self, active: bool, masks: &[u8]) {
        self.active = active;
        self.initialized = true;
        self.pattern_index = 0;
        self.pattern_elapsed = 0.0;
        self.pattern_mask = masks.first().copied().unwrap_or_default();
    }

    pub(crate) fn advance_pattern_fixed(&mut self, interval: f32, masks: &[u8]) -> Option<u8> {
        if !self.active || masks.is_empty() {
            return None;
        }
        self.pattern_elapsed += 1.0 / 60.0;
        if self.pattern_elapsed < interval {
            return None;
        }
        self.pattern_elapsed = 0.0;
        self.pattern_index += 1;
        if self.pattern_index >= masks.len() {
            self.pattern_index = 0;
        }
        self.pattern_mask = masks[self.pattern_index];
        Some(self.pattern_mask)
    }

    pub(crate) fn dispatch_door(&mut self, serialized_data0: u32, event_mask: u32) {
        self.door
            .dispatch_trigger_event(serialized_data0, event_mask);
    }

    pub(crate) fn dispatch_fix_switch(
        &mut self,
        serialized_data0: u32,
        event_mask: u32,
        owned_handler_present: bool,
    ) -> RobotsFixSwitchEventStep {
        self.fix_switch
            .dispatch_trigger_event(serialized_data0, event_mask, owned_handler_present)
    }

    pub(crate) fn dispatch_binary_active(&mut self, event_mask: u32) {
        // XTrigger_Interact/XTrigger_Light +0x6C = 0x00489BB0.
        // Save/load can seed +E4 before Maps sees an event, so keep it unknown
        // until a class event determines the value. Activate is tested first
        // and deactivate independently, therefore 0x300 ends at zero.
        if event_mask & 0x100 != 0 {
            self.binary_state_e4 = Some(1);
        }
        if event_mask & 0x200 != 0 {
            self.binary_state_e4 = Some(0);
        }
    }

    pub(crate) fn dispatch_npc(&mut self, event_mask: u32) {
        // XTrigger_NPC +0x6C = 0x0047F190. Runtime setup 0x0047F000
        // initializes +0x10C/+0x10D to zero; event 0x200 sets +0x10C.
        if event_mask & 0x200 != 0 {
            self.npc.latch_e10c = true;
        }
    }

    pub(crate) fn dispatch_ball_track(
        &mut self,
        initial_created: bool,
        two_phase_stop: bool,
        event_mask: u32,
    ) -> RobotsBallTrackEventStep {
        self.ball_track
            .dispatch_event(initial_created, two_phase_stop, event_mask)
    }

    pub(crate) fn dispatch_pickup(&mut self, event_mask: u32) {
        // XTrigger_Pickup +0x6C = 0x0048A160. Service bit 0x1000 does not
        // force-spawn the pickup; shared state owns only the persisted E8 latch.
        self.pickup.dispatch_event(event_mask);
    }

    pub(crate) fn dispatch_display_message(
        &mut self,
        text_uid: u32,
        duration_raw: u32,
        event_mask: u32,
        hud_mode_top: Option<u32>,
    ) {
        // XTrigger_DisplayMessage +0x6C = 0x0048C850. Class semantics are a
        // one-way 0x100 request into the HUD/message queue, suppressed while
        // the shared HUD/GameWnd mode stack top is 1, 2 or 3. Native computes
        // the queue duration as data[1] * DAT_005DD72C / 0x00433F90(...);
        // both operands are exactly 60, so the numeric duration is data[1].
        if event_mask & 0x100 == 0
            || matches!(hud_mode_top, Some(1 | 2 | 3))
            || matches!(text_uid, 0 | u32::MAX | 0x4500_0000)
        {
            return;
        }
        self.display_message_uid = Some(text_uid);
        self.display_message_duration_raw = Some(duration_raw);
        self.display_message_requests = self.display_message_requests.wrapping_add(1);
    }

    pub(crate) fn dispatch_owned_xitem_binary(
        &mut self,
        initial_e4: u8,
        xitem_exists: bool,
        event_mask: u32,
    ) {
        // SlideUnder/AlertIcon share +0x6C = 0x004833D0. Native load
        // 0x00483360 copies SlideUnder data[2] (+0x74) to +E4 and 0x00483730
        // copies AlertIcon data[3] (+0x78) to +E4. Class events mutate +E4
        // only while trigger+0x68 owns an XItem.
        if self.owned_xitem_active_e4.is_none() {
            self.owned_xitem_active_e4 = Some(initial_e4);
        }
        if !xitem_exists {
            return;
        }
        if event_mask & 0x100 != 0 && self.owned_xitem_active_e4 == Some(0) {
            self.owned_xitem_active_e4 = Some(1);
        }
        if event_mask & 0x200 != 0 && self.owned_xitem_active_e4 == Some(1) {
            self.owned_xitem_active_e4 = Some(0);
        }
    }

    pub(crate) fn dispatch_tutorial(&mut self, event_mask: u32) {
        // XTrigger_Tutorial +0x6C = 0x00484D80.
        if event_mask & 0x100 == 0 {
            return;
        }
        if !self.tutorial_interaction.latch_e8 {
            self.tutorial_interaction.latch_e8 = true;
        } else {
            self.tutorial_repeat_ea = 1;
        }
    }
}

pub(crate) fn runtime_trigger_link_message(trigger: &ProcessedTrigger, slot: usize) -> u32 {
    trigger.data.get(8 + slot).copied().flatten().unwrap_or(0)
}

pub(crate) fn runtime_trigger_deferred_link_event(trigger: &ProcessedTrigger, slot: usize) -> u32 {
    let message = runtime_trigger_link_message(trigger, slot);
    if message == 0 {
        0x101
    } else {
        message
    }
}

pub(crate) fn runtime_xitem_opacity_fixed_update(current: f32, target: f32) -> f32 {
    if current == target {
        return current;
    }
    let difference = target - current;
    let mut next = if target <= 0.0 {
        // 0x0044402B..0x00444039: ordinary decay to zero.
        current + difference * 0.1
    } else {
        // 0x00444043..0x004440A5: native eased rise toward one.
        let delta = if difference < 0.0 {
            (difference + 1.05) * -0.1
        } else {
            (1.05 - difference) * 0.1
        };
        let candidate = current + delta;
        if candidate > target - 0.05 {
            target
        } else {
            candidate
        }
    };
    if next >= 0.999 {
        next = 1.0;
    }
    if next < 0.001 {
        next = 0.0;
    }
    next.clamp(0.0, 1.0)
}

pub(crate) fn runtime_trigger_normal_proximity_factor(distance_squared: f32) -> f32 {
    if distance_squared <= ROBOTS_TRIGGER_NEAR1_RADIUS_SQUARED {
        0.0
    } else if distance_squared <= ROBOTS_TRIGGER_NEAR2_RADIUS_SQUARED {
        ((distance_squared - ROBOTS_TRIGGER_NEAR1_RADIUS_SQUARED) * 0.002).clamp(0.0, 1.0)
    } else {
        1.0
    }
}

pub(crate) fn runtime_trigger_distance_event(
    distance_squared: f32,
    flags: u32,
) -> Option<RuntimeTriggerDistanceEvent> {
    if flags & 0x0008_0000 != 0 {
        return None;
    }
    let event = if distance_squared <= ROBOTS_TRIGGER_NEAR0_RADIUS_SQUARED {
        RuntimeTriggerDistanceEvent::Event0
    } else if distance_squared <= ROBOTS_TRIGGER_NEAR1_RADIUS_SQUARED {
        RuntimeTriggerDistanceEvent::Event1
    } else if distance_squared <= ROBOTS_TRIGGER_NEAR2_RADIUS_SQUARED || flags & 0x0800_0000 != 0 {
        RuntimeTriggerDistanceEvent::Event2
    } else {
        RuntimeTriggerDistanceEvent::Event3
    };
    Some(event)
}

pub(crate) fn runtime_common_trigger_lifecycle_action(
    event: RuntimeTriggerDistanceEvent,
    zone_visual_active: bool,
    flags: u32,
) -> RuntimeCommonTriggerLifecycleAction {
    let bit1 = flags & 0x0000_0002 != 0;
    match event {
        RuntimeTriggerDistanceEvent::Event0 => {
            if zone_visual_active {
                RuntimeCommonTriggerLifecycleAction::ImmediateActive
            } else if bit1 {
                RuntimeCommonTriggerLifecycleAction::DelayedActive { ticks: 10 }
            } else {
                RuntimeCommonTriggerLifecycleAction::InactiveState2
            }
        }
        RuntimeTriggerDistanceEvent::Event1 => {
            if zone_visual_active || bit1 {
                RuntimeCommonTriggerLifecycleAction::DelayedActive { ticks: 30 }
            } else {
                RuntimeCommonTriggerLifecycleAction::InactiveState2
            }
        }
        RuntimeTriggerDistanceEvent::Event2 => {
            if zone_visual_active || bit1 {
                RuntimeCommonTriggerLifecycleAction::DelayedActive { ticks: 60 }
            } else {
                RuntimeCommonTriggerLifecycleAction::InactiveState2
            }
        }
        RuntimeTriggerDistanceEvent::Event3 => {
            if bit1 {
                RuntimeCommonTriggerLifecycleAction::DelayedActive { ticks: 60 }
            } else {
                RuntimeCommonTriggerLifecycleAction::InactiveState3
            }
        }
    }
}

pub(crate) type RuntimeMineBotMoveAttackWinner =
    eurochef_shared::robots_runtime::minebot::RobotsMineBotMoveAttackWinner;

/// GUI compatibility wrapper around the engine-neutral MineBot Attack gate.
pub(crate) fn runtime_minebot_attack_gate(
    minebot_position: Vec3,
    player_position: Vec3,
    target_visible: bool,
) -> bool {
    eurochef_shared::robots_runtime::minebot::minebot_attack_gate(
        minebot_position.to_array(),
        player_position.to_array(),
        target_visible,
    )
}

/// GUI compatibility wrapper around the engine-neutral local selector reducer.
pub(crate) fn runtime_minebot_move_attack_winner(
    move_eligible: bool,
    attack_eligible: bool,
) -> RuntimeMineBotMoveAttackWinner {
    eurochef_shared::robots_runtime::minebot::minebot_move_attack_winner(
        move_eligible,
        attack_eligible,
    )
}

#[derive(Clone, Copy, Debug)]
struct RuntimeRobotsRaycastHit {
    t: f32,
    face_mask: u16,
    normal: Vec3,
}

/// Mirrors the static `EXGeoEntity::DoRayCast` face loop at Robots.exe
/// `0x005182B4`. `delta` is the complete segment delta and `max_t` is normally
/// 1.0 for a fresh query. Faces are two-sided. Native include/exclude masks are
/// matched against the complete fourth u16 in each serialized 10-byte face.
fn runtime_robots_mesh_raycast_nearest_hit_filtered<F>(
    triangles: &[RobotsRaycastTriangle],
    start: Vec3,
    delta: Vec3,
    max_t: f32,
    mut accepts_face: F,
) -> Option<RuntimeRobotsRaycastHit>
where
    F: FnMut(u16) -> bool,
{
    let mut nearest = max_t;
    let mut nearest_hit = None;

    for triangle in triangles {
        if !accepts_face(triangle.face_mask) {
            continue;
        }

        let [v0, v1, v2] = triangle.positions;
        let raw_normal = (v1 - v0).cross(v2 - v0);
        let normal_length = raw_normal.length();
        let (normal, plane_d) = if normal_length == 0.0 {
            (Vec3::X, 0.0)
        } else {
            let normal = raw_normal / normal_length;
            (normal, -normal.dot(v0))
        };

        let denominator = normal.dot(delta);
        if denominator == 0.0 {
            continue;
        }
        let t = -(plane_d + normal.dot(start)) / denominator;
        if t < 0.0 || t >= nearest {
            continue;
        }

        let point = start + delta * t;
        if (v0 - v2).cross(normal).dot(point - v0) > 0.0
            || (v1 - v0).cross(normal).dot(point - v1) > 0.0
            || (v2 - v1).cross(normal).dot(point - v2) > 0.0
        {
            continue;
        }

        nearest = t;
        nearest_hit = Some(RuntimeRobotsRaycastHit {
            t,
            face_mask: triangle.face_mask,
            normal,
        });
    }

    nearest_hit
}

fn runtime_robots_mesh_raycast_nearest_hit(
    triangles: &[RobotsRaycastTriangle],
    start: Vec3,
    delta: Vec3,
    max_t: f32,
) -> Option<(f32, u16)> {
    runtime_robots_mesh_raycast_nearest_hit_filtered(triangles, start, delta, max_t, |_| true)
        .map(|hit| (hit.t, hit.face_mask))
}

pub(crate) fn runtime_robots_mesh_raycast_nearest_t(
    triangles: &[RobotsRaycastTriangle],
    start: Vec3,
    delta: Vec3,
    include_mask: u32,
    exclude_mask: u32,
    max_t: f32,
) -> Option<f32> {
    runtime_robots_mesh_raycast_nearest_hit_filtered(triangles, start, delta, max_t, |face_mask| {
        let face_mask = u32::from(face_mask);
        (include_mask == 0 || face_mask & include_mask != 0)
            && (exclude_mask == 0 || face_mask & exclude_mask == 0)
    })
    .map(|hit| hit.t)
}

fn runtime_closest_point_on_segment(point: Vec3, start: Vec3, end: Vec3) -> Vec3 {
    let delta = end - start;
    let length_squared = delta.length_squared();
    if length_squared <= f32::EPSILON {
        return start;
    }
    let t = ((point - start).dot(delta) / length_squared).clamp(0.0, 1.0);
    start + delta * t
}

/// Closest point used by the native variant-0 Physics sphere/triangle distance path.
fn runtime_closest_point_on_triangle(point: Vec3, triangle: &RobotsRaycastTriangle) -> Vec3 {
    let [a, b, c] = triangle.positions;
    let ab = b - a;
    let ac = c - a;
    if ab.cross(ac).length_squared() <= f32::EPSILON {
        let ab_point = runtime_closest_point_on_segment(point, a, b);
        let bc_point = runtime_closest_point_on_segment(point, b, c);
        let ca_point = runtime_closest_point_on_segment(point, c, a);
        return [ab_point, bc_point, ca_point]
            .into_iter()
            .min_by(|lhs, rhs| {
                point
                    .distance_squared(*lhs)
                    .total_cmp(&point.distance_squared(*rhs))
            })
            .unwrap_or(a);
    }

    let ap = point - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = point - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return a + ab * v;
    }
    let cp = point - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return a + ac * w;
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let edge = c - b;
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return b + edge * w;
    }
    let inverse = (va + vb + vc).recip();
    let v = vb * inverse;
    let w = vc * inverse;
    a + ab * v + ac * w
}

/// Squared point/triangle distance used by the native variant-0 Physics shape path.
/// Robots.exe `0x004EEF15 -> 0x0051253B` computes the same quantity, takes sqrt,
/// then accepts the sphere contact while `radius - distance >= 0`.
fn runtime_point_triangle_distance_squared(point: Vec3, triangle: &RobotsRaycastTriangle) -> f32 {
    point.distance_squared(runtime_closest_point_on_triangle(point, triangle))
}

/// Native `0x0041AA00` material admission for ordinary generic Projectile XItems.
/// Their generic factory raw group is zero. Face bit 0x04 takes the immediate
/// terminal-contact lane before the category exclusions below.
fn runtime_projectile_raw_group0_face_accepts_contact(face_mask: u16) -> bool {
    if face_mask & 0x0004 != 0 {
        return true;
    }
    let category = face_mask & 0x0078;
    face_mask & 0x0080 == 0 && category != 0x0030 && category != 0x0020
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RuntimeProjectileStaticWorldContact {
    /// Native Physics+0x0A contact bits required by downstream Handler consumers.
    pub(crate) contact_bits: u16,
    /// Normal from the variant-0 sphere/triangle narrowphase. `None` means contact
    /// exists but multiple non-coplanar contacts make a single native rebound normal
    /// ambiguous in this bounded host adapter.
    pub(crate) normal: Option<Vec3>,
}

fn runtime_projectile_sphere_contact_from_triangles(
    triangles: &[RobotsRaycastTriangle],
    center: Vec3,
    radius: f32,
) -> Option<RuntimeProjectileStaticWorldContact> {
    let radius_squared = radius * radius;
    let mut contact_bits = 0u16;
    let mut resolved_normal = None::<Vec3>;
    let mut normal_ambiguous = false;

    for triangle in triangles {
        if !runtime_projectile_raw_group0_face_accepts_contact(triangle.face_mask)
            || runtime_point_triangle_distance_squared(center, triangle) > radius_squared
        {
            continue;
        }

        // `0x0041AA1C..0x0041AA32`: face bit 0x04 takes the immediate lane and
        // writes Physics+0x0A |= 0x06 without entering the ordinary normal path.
        if triangle.face_mask & 0x0004 != 0 {
            contact_bits |= 0x0006;
            continue;
        }

        // Ordinary accepted static contact supplies bit0 consumed by
        // XExplosionFragment::0x004DB8B0 and a contact normal at Physics+0xBC..+0xC8.
        contact_bits |= 0x0001;
        let closest = runtime_closest_point_on_triangle(center, triangle);
        let delta = center - closest;
        let normal = if delta.length_squared() > f32::EPSILON {
            delta.normalize()
        } else {
            let [a, b, c] = triangle.positions;
            (b - a).cross(c - a).normalize_or_zero()
        };
        if normal == Vec3::ZERO {
            normal_ambiguous = true;
            continue;
        }
        if let Some(previous) = resolved_normal {
            // Reflection is invariant under n -> -n. Keep one normal for coplanar
            // triangle seams; fail closed for a real corner/multi-plane contact.
            if previous.dot(normal).abs() < 0.999 {
                normal_ambiguous = true;
            }
        } else {
            resolved_normal = Some(normal);
        }
    }

    (contact_bits != 0).then_some(RuntimeProjectileStaticWorldContact {
        contact_bits,
        normal: (!normal_ambiguous).then_some(resolved_normal).flatten(),
    })
}

fn runtime_projectile_sphere_hits_triangles(
    triangles: &[RobotsRaycastTriangle],
    center: Vec3,
    radius: f32,
) -> bool {
    runtime_projectile_sphere_contact_from_triangles(triangles, center, radius).is_some()
}

/// Static-world half of the native generic Physics contact pass used by
/// `XItemPhysics_Projectile`. Variant-0 radius comes from Physics+0x1D8/+0x10;
/// `0x004EEF15 -> 0x0051253B` performs a discrete sphere/triangle overlap, while
/// the separate ProjectileRayCast subclass owns swept/raycast behavior.
///
/// Outer `None` means required zone/placement collision geometry is unresolved;
/// callers fail closed rather than inventing a contact. Dynamic XItem-vs-XItem
/// contacts remain a separate host lane.
pub(crate) fn runtime_map_projectile_static_world_contact_detail(
    map: &ProcessedMap,
    center: Vec3,
    radius: f32,
) -> Option<Option<RuntimeProjectileStaticWorldContact>> {
    if !center.is_finite() || !radius.is_finite() || radius < 0.0 {
        return Some(None);
    }
    let Some(zone_index) = map.native_zone_index(center) else {
        return Some(None);
    };
    let Some(Some(zone_triangles)) = map.zone_raycast_triangles.get(zone_index) else {
        return None;
    };
    let mut contact =
        runtime_projectile_sphere_contact_from_triangles(zone_triangles, center, radius);

    let Some(zone) = map.zones.get(zone_index) else {
        return None;
    };
    let Some(info) = zone.unk20.as_ref() else {
        return Some(contact);
    };
    let Some(tree) = info.data_ref().spatial_tree.as_ref() else {
        return Some(contact);
    };
    let mut placement_indices = Vec::new();
    tree.collect_flagged_placement_indices(0x08, &mut placement_indices);
    for placement_index in placement_indices {
        let Some(placement) = map.placements.get(placement_index as usize) else {
            return None;
        };
        if placement.engine_flags & 0x08 == 0 {
            continue;
        }
        let Some(Some(triangles)) = map
            .placement_raycast_triangles
            .get(placement_index as usize)
        else {
            return None;
        };
        let Some(next) =
            runtime_projectile_sphere_contact_from_triangles(triangles, center, radius)
        else {
            continue;
        };
        contact = Some(match contact {
            None => next,
            Some(previous) => {
                let normal = match (previous.normal, next.normal) {
                    (Some(lhs), Some(rhs)) if lhs.dot(rhs).abs() >= 0.999 => Some(lhs),
                    (Some(_), Some(_)) => None,
                    _ => None,
                };
                RuntimeProjectileStaticWorldContact {
                    contact_bits: previous.contact_bits | next.contact_bits,
                    normal,
                }
            }
        });
    }
    Some(contact)
}

pub(crate) fn runtime_map_projectile_static_world_contact(
    map: &ProcessedMap,
    center: Vec3,
    radius: f32,
) -> Option<bool> {
    runtime_map_projectile_static_world_contact_detail(map, center, radius)
        .map(|contact| contact.is_some())
}

const ROBOTS_AI_ENV_QUERY_Y_BIAS: f32 = 1.0;
const ROBOTS_AI_ENV_QUERY_HALF_HEIGHT: f32 = 50.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct RuntimeAiEnvironmentFloorContact {
    pub(crate) point: Vec3,
    pub(crate) normal: Vec3,
}

fn runtime_robots_ai_environment_floor_nearest_hit(
    triangles: &[RobotsRaycastTriangle],
    start: Vec3,
    delta: Vec3,
    max_t: f32,
) -> Option<RuntimeRobotsRaycastHit> {
    runtime_robots_mesh_raycast_nearest_hit_filtered(triangles, start, delta, max_t, |face_mask| {
        face_mask & 1 == 0
    })
}

/// Common AI environment reference used by `0x00405320 -> 0x004F0A08 ->
/// 0x004ED004`. Native initializes the query point at ownerY+1 with bounds
/// ownerY-50..ownerY+50. The geometry contributor `0x00517E3A` writes the floor
/// lane (Handler+0x1C0 / Handler+0x2C0 bit0) only for faces whose fourth-u16
/// metadata bit0 is clear. `None` means required static geometry is unresolved;
/// `Some(None)` means native bit0 remains clear because no floor face was found.
/// The detailed hit is also reused by EQ04's CharacterPhysics floor projection so
/// the selector and physical contact consume one proven Map/environment query.
pub(crate) fn runtime_map_ai_environment_floor_contact(
    map: &ProcessedMap,
    owner_position: Vec3,
) -> Option<Option<RuntimeAiEnvironmentFloorContact>> {
    let start = owner_position + Vec3::Y * ROBOTS_AI_ENV_QUERY_Y_BIAS;
    let end_y = owner_position.y - ROBOTS_AI_ENV_QUERY_HALF_HEIGHT;
    let delta = Vec3::new(0.0, end_y - start.y, 0.0);

    let Some(zone_index) = map.native_zone_index(start) else {
        return Some(None);
    };
    let Some(Some(zone_triangles)) = map.zone_raycast_triangles.get(zone_index) else {
        return None;
    };

    let mut nearest = 1.0f32;
    let mut nearest_hit = None;
    if let Some(hit) =
        runtime_robots_ai_environment_floor_nearest_hit(zone_triangles, start, delta, nearest)
    {
        nearest = hit.t;
        nearest_hit = Some(hit);
    }

    let Some(zone) = map.zones.get(zone_index) else {
        return None;
    };
    if let Some(info) = zone.unk20.as_ref() {
        if let Some(tree) = info.data_ref().spatial_tree.as_ref() {
            let mut placement_indices = Vec::new();
            tree.collect_flagged_placement_indices(0x08, &mut placement_indices);
            for placement_index in placement_indices {
                let Some(placement) = map.placements.get(placement_index as usize) else {
                    return None;
                };
                if placement.engine_flags & 0x08 == 0 {
                    continue;
                }
                let Some(Some(triangles)) = map
                    .placement_raycast_triangles
                    .get(placement_index as usize)
                else {
                    return None;
                };
                if let Some(hit) = runtime_robots_ai_environment_floor_nearest_hit(
                    triangles, start, delta, nearest,
                ) {
                    nearest = hit.t;
                    nearest_hit = Some(hit);
                }
            }
        }
    }

    Some(nearest_hit.map(|hit| RuntimeAiEnvironmentFloorContact {
        point: start + delta * hit.t,
        normal: hit.normal,
    }))
}

pub(crate) fn runtime_map_ai_environment_floor_y(
    map: &ProcessedMap,
    owner_position: Vec3,
) -> Option<Option<f32>> {
    runtime_map_ai_environment_floor_contact(map, owner_position)
        .map(|contact| contact.map(|contact| contact.point.y))
}

/// Bounded CharacterPhysics floor-plane adapter for the EQ04 vertical-fall lane.
/// The native generic contact solver (`0x0041A3A0 -> 0x004F0414`) projects a body
/// out of accepted contact planes; this keeps exactly that geometric operation for
/// the already-proven environment floor face. It is deliberately not advertised as
/// a replacement for the complete multi-face world-contact solver.
pub(crate) fn runtime_character_floor_contact_projection(
    shape: RuntimeCharacterWorldShape,
    contact: RuntimeAiEnvironmentFloorContact,
) -> Option<Vec3> {
    let shape_center = match shape {
        RuntimeCharacterWorldShape::Sphere { center_xyz, .. } => Vec3::from_array(center_xyz),
        RuntimeCharacterWorldShape::Capsule {
            start_xyz,
            delta_xyz,
            ..
        } => Vec3::from_array(start_xyz) + Vec3::from_array(delta_xyz) * 0.5,
    };
    let mut normal = contact.normal.normalize_or_zero();
    if normal == Vec3::ZERO {
        return None;
    }
    if normal.dot(shape_center - contact.point) < 0.0 {
        normal = -normal;
    }

    let support_distance = match shape {
        RuntimeCharacterWorldShape::Sphere { center_xyz, radius } => {
            normal.dot(Vec3::from_array(center_xyz) - contact.point) - radius
        }
        RuntimeCharacterWorldShape::Capsule {
            start_xyz,
            delta_xyz,
            radius,
        } => {
            let start = Vec3::from_array(start_xyz);
            let end = start + Vec3::from_array(delta_xyz);
            normal
                .dot(start - contact.point)
                .min(normal.dot(end - contact.point))
                - radius
        }
    };
    (support_distance < 0.0).then_some(normal * -support_distance)
}

/// Native Camera mode-1 contact acceptance from Robots.exe 0x00478EF0.
/// 0x0041AD50 + 0x004F6244 + 0x004F68A9 prove these bits are the fourth
/// u16 (`surface_metadata`) of the serialized 10-byte face record.
pub(crate) fn runtime_robots_camera_contact_face_accepted(face_mask: u16) -> bool {
    let face_mask = u32::from(face_mask);
    face_mask & 0x78 != 0x48 && face_mask & 0x4000 == 0 && face_mask & 0x78 != 0x20
}

#[cfg(test)]
pub(crate) fn runtime_robots_camera_contact_nearest_t(
    triangles: &[RobotsRaycastTriangle],
    start: Vec3,
    delta: Vec3,
    max_t: f32,
) -> Option<f32> {
    let (t, face_mask) = runtime_robots_mesh_raycast_nearest_hit(triangles, start, delta, max_t)?;
    runtime_robots_camera_contact_face_accepted(face_mask).then_some(t)
}

/// Runs the native Robots face test against the world-space Entity instances
/// produced by the live AnimScript traversal. Script slot16 reaches attached
/// Entity animators rather than the script resource itself, so the caller must
/// pass the active `QueuedEntityRender` set for the current script time.
pub(crate) fn runtime_queued_entities_line_of_sight_clear(
    queue: &[QueuedEntityRender],
    render_store: &RenderStore,
    start: Vec3,
    end: Vec3,
) -> Option<bool> {
    let delta = end - start;
    if delta.length_squared() == 0.0 {
        return Some(true);
    }

    for queued in queue {
        if queued.entity_alt.is_some() {
            return None;
        }
        let entity = render_store.get_entity(queued.entity.0, queued.entity.1)?;
        let triangles = entity.robots_raycast_triangles();
        if triangles.is_empty() {
            continue;
        }

        let transform =
            Mat4::from_scale_rotation_translation(queued.scale, queued.rotation, queued.position);
        let world_triangles = triangles
            .iter()
            .map(|triangle| RobotsRaycastTriangle {
                positions: triangle
                    .positions
                    .map(|position| transform.transform_point3(position)),
                face_mask: triangle.face_mask,
                trailing_raw: triangle.trailing_raw,
            })
            .collect::<Vec<_>>();
        if runtime_robots_mesh_raycast_nearest_t(&world_triangles, start, delta, 0, 0, 1.0)
            .is_some()
        {
            return Some(false);
        }
    }

    Some(true)
}

/// Native Camera mask-bit0 XItem fallback (`0x00444B50 -> 0x004E8460`).
/// The dynamic pass selects the nearest raw Entity face first and applies the
/// same Camera material predicate only to that winning face. Animation/Collision
/// child animators are intentionally absent from the supplied queue because their
/// native slot16 is the no-op `0x004E8A20`.
pub(crate) fn runtime_queued_entities_camera_contact_nearest_t(
    queue: &[QueuedEntityRender],
    render_store: &RenderStore,
    start: Vec3,
    end: Vec3,
) -> Option<Option<f32>> {
    let delta = end - start;
    if delta.length_squared() == 0.0 {
        return Some(None);
    }

    let mut nearest = 1.0f32;
    let mut nearest_mask = 0u16;
    let mut hit = false;
    for queued in queue {
        if queued.entity_alt.is_some() {
            return None;
        }
        let entity = render_store.get_entity(queued.entity.0, queued.entity.1)?;
        let triangles = entity.robots_raycast_triangles();
        if triangles.is_empty() {
            continue;
        }
        let transform =
            Mat4::from_scale_rotation_translation(queued.scale, queued.rotation, queued.position);
        for triangle in triangles {
            let world = RobotsRaycastTriangle {
                positions: triangle
                    .positions
                    .map(|position| transform.transform_point3(position)),
                face_mask: triangle.face_mask,
                trailing_raw: triangle.trailing_raw,
            };
            if let Some((t, face_mask)) = runtime_robots_mesh_raycast_nearest_hit(
                std::slice::from_ref(&world),
                start,
                delta,
                nearest,
            ) {
                nearest = t;
                nearest_mask = face_mask;
                hit = true;
            }
        }
    }

    Some((hit && runtime_robots_camera_contact_face_accepted(nearest_mask)).then_some(nearest))
}

/// Camera mode-1 static Map stage from Robots.exe `0x00478E30 -> 0x004ECE9B`.
/// Geometry selects the nearest raw hit first; the Camera material predicate is
/// applied only to that winning face. Outer `None` means required crossed
/// zone/placement geometry is unresolved. `Some(Some(t))` is an accepted static
/// hit. `Some(None)` means the static stage produced no accepted hit and native
/// code would continue with the separate mask-bit0 XItem fallback `0x00444B50`.
pub(crate) fn runtime_map_camera_contact_nearest_t(
    map: &ProcessedMap,
    start: Vec3,
    end: Vec3,
) -> Option<Option<f32>> {
    let delta = end - start;
    if delta.length_squared() == 0.0 {
        return Some(None);
    }

    let mut nearest = 1.0f32;
    let mut nearest_mask = 0u16;
    let mut hit = false;
    for zone_index in
        robots_map_zone_indices_for_segment_by_bsp(&map.bsp_nodes, map.zones.len(), start, end)
    {
        let Some(Some(zone_triangles)) = map.zone_raycast_triangles.get(zone_index) else {
            return None;
        };
        if let Some((t, face_mask)) =
            runtime_robots_mesh_raycast_nearest_hit(zone_triangles, start, delta, nearest)
        {
            nearest = t;
            nearest_mask = face_mask;
            hit = true;
        }

        let Some(zone) = map.zones.get(zone_index) else {
            return None;
        };
        let Some(info) = zone.unk20.as_ref() else {
            continue;
        };
        let Some(tree) = info.data_ref().spatial_tree.as_ref() else {
            continue;
        };
        let mut placement_indices = Vec::new();
        tree.collect_flagged_placement_indices(0x08, &mut placement_indices);
        for placement_index in placement_indices {
            let Some(placement) = map.placements.get(placement_index as usize) else {
                return None;
            };
            if placement.engine_flags & 0x08 == 0 {
                continue;
            }
            let Some(Some(triangles)) = map
                .placement_raycast_triangles
                .get(placement_index as usize)
            else {
                return None;
            };
            if let Some((t, face_mask)) =
                runtime_robots_mesh_raycast_nearest_hit(triangles, start, delta, nearest)
            {
                nearest = t;
                nearest_mask = face_mask;
                hit = true;
            }
        }
    }

    Some((hit && runtime_robots_camera_contact_face_accepted(nearest_mask)).then_some(nearest))
}

/// Mirrors the static-geometry portion of Robots.exe Map slot16 LOS:
/// `0x005557D6` enumerates every BSP zone crossed by the segment, each zone root
/// gets `EXGeoEntity::DoRayCast`, then `0x00553006` contributes only placement
/// leaves whose low-byte flags contain bit 0x08. The placement leaf flag is
/// byte-identical to `EXGeoPlacement.engine_flags` on the real Robots corpus.
pub(crate) fn runtime_map_line_of_sight_clear(
    map: &ProcessedMap,
    start: Vec3,
    end: Vec3,
) -> Option<bool> {
    let delta = end - start;
    if delta.length_squared() == 0.0 {
        return Some(true);
    }

    for zone_index in
        robots_map_zone_indices_for_segment_by_bsp(&map.bsp_nodes, map.zones.len(), start, end)
    {
        let Some(Some(zone_triangles)) = map.zone_raycast_triangles.get(zone_index) else {
            return None;
        };
        if runtime_robots_mesh_raycast_nearest_t(zone_triangles, start, delta, 0, 0, 1.0).is_some()
        {
            return Some(false);
        }

        let Some(zone) = map.zones.get(zone_index) else {
            return None;
        };
        let Some(info) = zone.unk20.as_ref() else {
            continue;
        };
        let Some(tree) = info.data_ref().spatial_tree.as_ref() else {
            continue;
        };
        let mut placement_indices = Vec::new();
        tree.collect_flagged_placement_indices(0x08, &mut placement_indices);
        for placement_index in placement_indices {
            let Some(placement) = map.placements.get(placement_index as usize) else {
                return None;
            };
            if placement.engine_flags & 0x08 == 0 {
                continue;
            }
            let Some(Some(triangles)) = map
                .placement_raycast_triangles
                .get(placement_index as usize)
            else {
                return None;
            };
            if runtime_robots_mesh_raycast_nearest_t(triangles, start, delta, 0, 0, 1.0).is_some() {
                return Some(false);
            }
        }
    }

    Some(true)
}

pub(crate) fn map_trigger_runtime_path<'a>(
    map: &'a ProcessedMap,
    trigger: &ProcessedTrigger,
) -> Option<(u32, usize, &'a ProcessedPath)> {
    let path_hash = robots_trigger_path_hash(trigger.ttype, &trigger.data)?;
    map.paths
        .iter()
        .enumerate()
        .find(|(_, path)| path.hashcode == path_hash)
        .map(|(index, path)| (path_hash, index, path))
}

pub(crate) fn map_trigger_path_matches<'a>(
    map: &'a ProcessedMap,
    trigger: &ProcessedTrigger,
) -> Vec<(usize, usize, &'a ProcessedPath)> {
    trigger
        .data
        .iter()
        .enumerate()
        .filter_map(|(data_slot, value)| {
            let path_hash = value.as_ref()?;
            map.paths
                .iter()
                .enumerate()
                .find(|(_, path)| path.hashcode == *path_hash)
                .map(|(path_index, path)| (data_slot, path_index, path))
        })
        .collect()
}

pub(crate) fn runtime_path_segments(path: &ProcessedPath) -> Vec<(Vec3, Vec3)> {
    let world_node = |index: usize| {
        path.nodes
            .get(index)
            .map(|node| path.position + node.position)
    };
    if path.links.is_empty() {
        return path
            .nodes
            .windows(2)
            .map(|nodes| {
                (
                    path.position + nodes[0].position,
                    path.position + nodes[1].position,
                )
            })
            .collect();
    }

    path.links
        .iter()
        .filter_map(|(a, b)| Some((world_node(*a)?, world_node(*b)?)))
        .collect()
}

pub(crate) fn runtime_path_route(path: &ProcessedPath) -> Vec<Vec3> {
    runtime_path_route_indices(path)
        .into_iter()
        .filter_map(|index| {
            path.nodes
                .get(index)
                .map(|node| path.position + node.position)
        })
        .collect()
}

pub(crate) fn runtime_path_route_indices(path: &ProcessedPath) -> Vec<usize> {
    let world_nodes = || (0..path.nodes.len()).collect::<Vec<_>>();
    if path.nodes.len() < 2 || path.links.is_empty() {
        return world_nodes();
    }

    let valid_links = path
        .links
        .iter()
        .copied()
        .filter(|(start, end)| *start < path.nodes.len() && *end < path.nodes.len())
        .collect::<Vec<_>>();
    if valid_links.is_empty() {
        return world_nodes();
    }

    let mut incoming = vec![0usize; path.nodes.len()];
    for (_, end) in &valid_links {
        incoming[*end] += 1;
    }
    let start = valid_links
        .iter()
        .find_map(|(start, _)| (incoming[*start] == 0).then_some(*start))
        .unwrap_or(valid_links[0].0);

    let mut used = vec![false; valid_links.len()];
    let mut route = vec![start];
    let mut current = start;
    loop {
        let Some((edge_index, (_, end))) = valid_links
            .iter()
            .enumerate()
            .find(|(index, (edge_start, _))| !used[*index] && *edge_start == current)
        else {
            break;
        };
        used[edge_index] = true;
        current = *end;
        route.push(current);
    }

    if route.len() >= 2 && used.iter().all(|used| *used) {
        route
    } else {
        world_nodes()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuntimePathNodeEvent {
    DeactivateSelf,
    DispatchLinked { event_mask: u32, link_mask: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RuntimePathNodeDispatch {
    pub(crate) node_index: usize,
    pub(crate) event: RuntimePathNodeEvent,
}

fn path_node_event(node: &crate::maps::ProcessedPathNode) -> Option<RuntimePathNodeEvent> {
    match node.value[0] {
        4 => Some(RuntimePathNodeEvent::DeactivateSelf),
        8 => Some(RuntimePathNodeEvent::DispatchLinked {
            event_mask: node.value[1] as u32,
            link_mask: node.value[2] as u8,
        }),
        _ => None,
    }
}

fn periodic_crossing_count(from: f32, to: f32, phase: f32, period: f32) -> usize {
    if period <= f32::EPSILON || (to - from).abs() <= f32::EPSILON {
        return 0;
    }
    let low = from.min(to);
    let high = from.max(to);
    let first = ((low - phase) / period).floor() as i64 - 1;
    let last = ((high - phase) / period).ceil() as i64 + 1;
    (first..=last)
        .map(|k| phase + k as f32 * period)
        .filter(|value| {
            if to > from {
                *value > from + f32::EPSILON && *value <= to + f32::EPSILON
            } else {
                *value < from - f32::EPSILON && *value >= to - f32::EPSILON
            }
        })
        .count()
}

pub(crate) fn runtime_path_node_dispatches_between(
    map: &ProcessedMap,
    trigger: &ProcessedTrigger,
    from_traveled: f32,
    to_traveled: f32,
) -> Vec<RuntimePathNodeDispatch> {
    let Some((_, _, path)) = map_trigger_runtime_path(map, trigger) else {
        return Vec::new();
    };
    let route_indices = runtime_path_route_indices(path);
    if route_indices.len() < 2 {
        return Vec::new();
    }
    let route = route_indices
        .iter()
        .filter_map(|index| {
            path.nodes
                .get(*index)
                .map(|node| path.position + node.position)
        })
        .collect::<Vec<_>>();
    if route.len() != route_indices.len() {
        return Vec::new();
    }

    let looping = trigger.ttype == 80 && path.flags & 0x6000_0000 == 0x6000_0000;
    let segments = runtime_path_segments_for_motion(&route, looping);
    let total_length = segments
        .iter()
        .map(|(start, end)| start.distance(*end))
        .sum::<f32>();
    if total_length <= f32::EPSILON {
        return Vec::new();
    }
    let (start_phase, _) = closest_route_phase(&segments, trigger.position);
    let from = start_phase + from_traveled;
    let to = start_phase + to_traveled;

    let mut node_phases = Vec::with_capacity(route.len());
    let mut node_phase = 0.0f32;
    node_phases.push(0.0);
    for nodes in route.windows(2) {
        node_phase += nodes[0].distance(nodes[1]);
        node_phases.push(node_phase);
    }

    let mut dispatches = Vec::new();
    for (route_position, node_index) in route_indices.iter().copied().enumerate() {
        let Some(node) = path.nodes.get(node_index) else {
            continue;
        };
        let Some(event) = path_node_event(node) else {
            continue;
        };
        let phase = node_phases[route_position];
        let count = if looping {
            periodic_crossing_count(from, to, phase, total_length)
        } else {
            let cycle = total_length * 2.0;
            let mut count = periodic_crossing_count(from, to, phase, cycle);
            let reverse_phase = cycle - phase;
            if (reverse_phase - phase).abs() > f32::EPSILON {
                count += periodic_crossing_count(from, to, reverse_phase, cycle);
            }
            count
        };
        dispatches.extend((0..count).map(|_| RuntimePathNodeDispatch { node_index, event }));
    }
    dispatches
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RuntimePathSample {
    pub(crate) position: Vec3,
    pub(crate) tangent: Vec3,
}

pub(crate) fn runtime_path_segments_for_motion(route: &[Vec3], looping: bool) -> Vec<(Vec3, Vec3)> {
    let mut segments = route
        .windows(2)
        .map(|nodes| (nodes[0], nodes[1]))
        .collect::<Vec<_>>();
    if looping {
        if let (Some(first), Some(last)) = (route.first().copied(), route.last().copied()) {
            if first.distance_squared(last) > f32::EPSILON {
                segments.push((last, first));
            }
        }
    }
    segments
}

pub(crate) fn closest_route_phase(segments: &[(Vec3, Vec3)], position: Vec3) -> (f32, Vec3) {
    let mut cumulative = 0.0;
    let mut best_distance_squared = f32::INFINITY;
    let mut best_phase = 0.0;
    let mut best_point = position;

    for (start, end) in segments {
        let delta = *end - *start;
        let length_squared = delta.length_squared();
        if length_squared <= f32::EPSILON {
            continue;
        }
        let segment_length = length_squared.sqrt();
        let fraction = ((position - *start).dot(delta) / length_squared).clamp(0.0, 1.0);
        let point = start.lerp(*end, fraction);
        let distance_squared = point.distance_squared(position);
        if distance_squared < best_distance_squared {
            best_distance_squared = distance_squared;
            best_phase = cumulative + fraction * segment_length;
            best_point = point;
        }
        cumulative += segment_length;
    }

    (best_phase, position - best_point)
}

pub(crate) const ROBOTS_EVENT_ACTIVATE: u32 = 0x0000_0100;
pub(crate) const ROBOTS_EVENT_DEACTIVATE: u32 = 0x0000_0200;
pub(crate) const ROBOTS_PLATFORM_RETRIGGER_REVERSE_FLAG: u32 = 0x0000_0200;
/// Native manager registration mask required for the Physics slot4
/// contact/world-query phase (`0x004E7F46 -> manager+0x88[0] -> 0x00444C20`).
pub(crate) const ROBOTS_GAMEPLAY_COLLISION_REGISTRATION_MASK: u32 = 0x0000_0001;

/// Native map-collision primitive after the serialized AnimDatum has been
/// transformed first by the selected AnimSkin bone matrix (`0x00500569`) and
/// then by the owning animator/XItem transform (`0x004F4911 -> 0x004E5C6F ->
/// 0x00538963`). Capsule representation matches `0x00539A63`: segment start +
/// Shared Robots world-hit shape. The GUI owns only matrix/bone resolution;
/// final sphere/capsule construction lives in `eurochef-shared` for UE5.8 reuse.
pub(crate) type RuntimeCharacterWorldShape = RobotsHitShape;

/// Host-side candidate gate for a runtime-created character body. A decoded
/// AnimSkin body exists as immutable map metadata before TriggerManager creates
/// its XItem, so body presence alone must never make it hittable. Registration
/// bit0 and concrete lifecycle ownership are both required before exposing the
/// current animated world shape to the common hit-query host.
pub(crate) fn runtime_live_character_collision_shape(
    body: Option<&RuntimeCharacterBodyState>,
    xitem_exists: bool,
) -> Option<RuntimeCharacterWorldShape> {
    if !xitem_exists {
        return None;
    }
    let body = body?;
    if body.registration_mask & ROBOTS_GAMEPLAY_COLLISION_REGISTRATION_MASK == 0 {
        return None;
    }
    Some(body.current_world_shape())
}

/// Geometry supplied by the query owner after native source/sample resolution.
/// Candidate HitArea geometry stays on the character body; source geometry stays
/// outside it, matching the original XItem/query ownership split.
pub(crate) enum RuntimeCharacterHitQueryGeometry<'a> {
    PreparedShape(RuntimeCharacterWorldShape),
    SampleSweep {
        plan: RobotsHitSampleSweepPlan,
        source_xyzw: Option<[f32; 4]>,
        samples: &'a mut [RobotsHitSweepSample],
    },
}

/// Result of the GUI/UE host adapter for one live character candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeCharacterHitCandidateResult {
    Reject(RobotsHitQueryCandidateRejectReason),
    AnimDatumHitArea { hit: bool },
    AlternateSampleSweep { hit: bool },
    NarrowphaseInputMismatch,
}

fn runtime_classify_live_character_hit_candidate(
    body: &RuntimeCharacterBodyState,
    xitem_exists: bool,
    context: RobotsHitQueryCandidateContext,
    is_source: bool,
    is_secondary_source: bool,
    coarse_bounds_miss: bool,
) -> RobotsHitQueryCandidateDecision {
    let present =
        xitem_exists && body.registration_mask & ROBOTS_GAMEPLAY_COLLISION_REGISTRATION_MASK != 0;
    classify_hit_query_candidate(
        context,
        RobotsHitQueryCandidateView {
            present,
            is_source,
            is_secondary_source,
            raw_group: body.raw_hit_query_group,
            last_hit_query_serial: body.last_hit_query_serial,
            coarse_bounds_miss,
        },
    )
}

/// Apply native candidate admission followed by the exact selected narrowphase.
/// Both paths query `HT_AnimDatum_HitArea`, never MapCollisionCapsule. Only a
/// narrowphase return of true commits the current 16-bit serial to Handler+0x37C.
pub(crate) fn runtime_test_live_character_hit_candidate(
    body: Option<&mut RuntimeCharacterBodyState>,
    xitem_exists: bool,
    context: RobotsHitQueryCandidateContext,
    geometry: RuntimeCharacterHitQueryGeometry<'_>,
    is_source: bool,
    is_secondary_source: bool,
    coarse_bounds_miss: bool,
) -> RuntimeCharacterHitCandidateResult {
    let Some(body) = body else {
        return RuntimeCharacterHitCandidateResult::Reject(
            RobotsHitQueryCandidateRejectReason::MissingCandidate,
        );
    };
    let decision = runtime_classify_live_character_hit_candidate(
        body,
        xitem_exists,
        context,
        is_source,
        is_secondary_source,
        coarse_bounds_miss,
    );
    match decision {
        RobotsHitQueryCandidateDecision::Reject(reason) => {
            RuntimeCharacterHitCandidateResult::Reject(reason)
        }
        RobotsHitQueryCandidateDecision::Test(RobotsHitQueryNarrowphaseKind::AnimDatumHitArea) => {
            let RuntimeCharacterHitQueryGeometry::PreparedShape(query_shape) = geometry else {
                return RuntimeCharacterHitCandidateResult::NarrowphaseInputMismatch;
            };
            let hit = body
                .current_hit_area_world_shape()
                .is_some_and(|candidate_shape| {
                    robots_hit_shapes_intersect(query_shape, candidate_shape)
                });
            if hit {
                body.last_hit_query_serial = context.query_serial;
            }
            RuntimeCharacterHitCandidateResult::AnimDatumHitArea { hit }
        }
        RobotsHitQueryCandidateDecision::Test(RobotsHitQueryNarrowphaseKind::Alternate) => {
            let RuntimeCharacterHitQueryGeometry::SampleSweep {
                plan,
                source_xyzw,
                samples,
            } = geometry
            else {
                return RuntimeCharacterHitCandidateResult::NarrowphaseInputMismatch;
            };
            let candidate_shape = body.current_hit_area_world_shape();
            let hit = execute_hit_sample_sweep(plan, source_xyzw, samples, |query_shape| {
                candidate_shape
                    .is_some_and(|candidate| robots_hit_shapes_intersect(query_shape, candidate))
            });
            if hit {
                body.last_hit_query_serial = context.query_serial;
            }
            RuntimeCharacterHitCandidateResult::AlternateSampleSweep { hit }
        }
    }
}

/// Apply the exact proven transform order for `HT_AnimDatum_MapCollisionCapsule`.
/// `bone_matrix` must be the current matrix selected by AnimDatum +0x30. Native
/// `0x00538963` scales shape scalars by the maximum transform scale while using
/// the rotation component for the capsule axis; owner pose is applied after the
/// bone-local transform, matching `0x004F4911`.
fn runtime_normalized_quat_or_identity(quat: Quat) -> Quat {
    let length_squared = quat.length_squared();
    if length_squared.is_finite() && length_squared > f32::EPSILON {
        quat * length_squared.sqrt().recip()
    } else {
        Quat::IDENTITY
    }
}

/// Robots' native AI animator runs at 60 fixed updates per second. The proven
/// Default->Move contribution uses a weight-rate parameter of 5; native
/// `0x0054F8A6` turns that into `1/5`, and `0x0054F853` integrates the weight
/// once per fixed update. The incoming node therefore reaches full weight in
/// exactly five fixed updates.
#[allow(dead_code)]
pub(crate) const ROBOTS_CHARACTER_MOVE_BLEND_FIXED_TICKS: f32 = 5.0;
#[allow(dead_code)]
pub(crate) const ROBOTS_CHARACTER_ANIMATION_FIXED_HZ: f32 = 60.0;
#[allow(dead_code)]
pub(crate) const ROBOTS_CHARACTER_MOVE_BLEND_SECONDS: f32 =
    ROBOTS_CHARACTER_MOVE_BLEND_FIXED_TICKS / ROBOTS_CHARACTER_ANIMATION_FIXED_HZ;

fn runtime_character_track_local_poses(
    track: &ProcessedCharacterAnimationTrack,
    seconds: f32,
) -> Option<Vec<ProcessedCharacterAnimationBonePose>> {
    let chain_len = track.bone_chain.len();
    if track.frame_count == 0
        || chain_len == 0
        || track.poses.len() != track.frame_count.saturating_mul(chain_len)
    {
        return None;
    }

    let frame_position = (seconds.max(0.0) * f32::from(track.clip_rate)) % track.frame_count as f32;
    let frame0 = frame_position.floor() as usize;
    let frame1 = (frame0 + 1) % track.frame_count;
    let fraction = frame_position - frame0 as f32;
    let mut poses = Vec::with_capacity(chain_len);
    for chain_index in 0..chain_len {
        let pose0 = track.poses[frame0 * chain_len + chain_index];
        let pose1 = track.poses[frame1 * chain_len + chain_index];
        let position = pose0.position.lerp(pose1.position, fraction);
        let mut next_rotation = pose1.rotation;
        if pose0.rotation.dot(next_rotation) < 0.0 {
            next_rotation = -next_rotation;
        }
        let rotation =
            runtime_normalized_quat_or_identity(pose0.rotation.slerp(next_rotation, fraction));
        poses.push(ProcessedCharacterAnimationBonePose { position, rotation });
    }
    Some(poses)
}

fn runtime_character_track_root_motion_sample(
    track: &ProcessedCharacterAnimationTrack,
    seconds: f32,
) -> Option<ProcessedCharacterRootMotionSample> {
    if track.frame_count == 0 || track.root_motion_samples.len() != track.frame_count {
        return None;
    }

    let frame_position = (seconds.max(0.0) * f32::from(track.clip_rate)) % track.frame_count as f32;
    let frame0 = frame_position.floor() as usize;
    let frame1 = (frame0 + 1) % track.frame_count;
    let fraction = frame_position - frame0 as f32;
    let sample0 = track.root_motion_samples[frame0];
    let sample1 = track.root_motion_samples[frame1];
    let position = sample0.position.lerp(sample1.position, fraction);
    let mut next_rotation = sample1.rotation;
    if sample0.rotation.dot(next_rotation) < 0.0 {
        next_rotation = -next_rotation;
    }
    let rotation =
        runtime_normalized_quat_or_identity(sample0.rotation.slerp(next_rotation, fraction));
    Some(ProcessedCharacterRootMotionSample { position, rotation })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RuntimeCharacterRootMotionDelta {
    pub(crate) native_translation: Vec3,
    pub(crate) native_rotation: Quat,
}

/// Family-4 root-motion delta consumed by `0x004F3E8D` mode 3. Native samples
/// current and next animation phases, subtracts translation and composes
/// `next * inverse(current)` for rotation. Transition alpha uses the serialized
/// AnimSet +0x06 tick count recovered through `0x004F2A67 -> 0x0054F8A6`.
pub(crate) fn runtime_character_track_root_motion_delta(
    track: &ProcessedCharacterAnimationTrack,
    seconds: f32,
    fixed_step_seconds: f32,
    transition_elapsed_seconds: f32,
) -> Option<RuntimeCharacterRootMotionDelta> {
    let current = runtime_character_track_root_motion_sample(track, seconds)?;
    let next = runtime_character_track_root_motion_sample(track, seconds + fixed_step_seconds)?;
    let alpha = if track.transition_fixed_ticks == 0 {
        1.0
    } else {
        let transition_seconds =
            track.transition_fixed_ticks as f32 / ROBOTS_CHARACTER_ANIMATION_FIXED_HZ;
        (transition_elapsed_seconds.max(0.0) / transition_seconds).clamp(0.0, 1.0)
    };
    let native_translation = (next.position - current.position) * alpha;
    let native_delta =
        runtime_normalized_quat_or_identity(next.rotation * current.rotation.conjugate());
    let native_rotation =
        runtime_normalized_quat_or_identity(Quat::IDENTITY.slerp(native_delta, alpha));
    Some(RuntimeCharacterRootMotionDelta {
        native_translation,
        native_rotation,
    })
}

fn runtime_character_chain_matrix(poses: &[ProcessedCharacterAnimationBonePose]) -> Mat4 {
    poses.iter().fold(Mat4::IDENTITY, |global, pose| {
        global * Mat4::from_rotation_translation(pose.rotation, pose.position)
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RuntimeCharacterDatumWorldTransform {
    pub(crate) position: Vec3,
    pub(crate) rotation: Quat,
}

/// Resolve one gameplay AnimDatum through the exact sampled bone chain belonging
/// to the currently selected AnimMode. The character body remains the sole owner
/// of the XItem world transform; character metadata owns immutable datum/track data.
pub(crate) fn runtime_character_animation_datum_world_transform(
    owner_position: Vec3,
    owner_rotation: Quat,
    owner_scale: Vec3,
    visual: &ProcessedCharacterVisual,
    anim_mode: Hashcode,
    datum_hashcode: Hashcode,
    animation_seconds: f32,
) -> Option<RuntimeCharacterDatumWorldTransform> {
    let datum = visual.anim_datums.get(&datum_hashcode)?;
    let track = visual
        .animation_mode_datum_tracks
        .get(&(anim_mode, datum_hashcode))?;
    if track.animskin != datum.animskin {
        return None;
    }
    let poses = runtime_character_track_local_poses(track, animation_seconds)?;
    let bone_matrix = runtime_character_chain_matrix(&poses);
    let owner_matrix =
        Mat4::from_scale_rotation_translation(owner_scale, owner_rotation, owner_position);
    let world_from_bone = owner_matrix * bone_matrix;
    let (_, bone_rotation, _) = world_from_bone.to_scale_rotation_translation();
    let local_rotation =
        runtime_normalized_quat_or_identity(Quat::from_array(datum.local_orientation));
    Some(RuntimeCharacterDatumWorldTransform {
        position: world_from_bone.transform_point3(datum.local_center),
        rotation: runtime_normalized_quat_or_identity(bone_rotation * local_rotation),
    })
}

/// Resolve a searchable animated AnimDatum as the same native world-space hit
/// shape used by the common HitCheck query. This is the shape counterpart of
/// `runtime_character_animation_datum_world_transform`; point-only consumers keep
/// using the transform helper while melee/hazard queries reuse this geometry path.
pub(crate) fn runtime_character_animation_datum_world_shape(
    owner_position: Vec3,
    owner_rotation: Quat,
    owner_scale: Vec3,
    visual: &ProcessedCharacterVisual,
    anim_mode: Hashcode,
    datum_hashcode: Hashcode,
    animation_seconds: f32,
) -> Option<RuntimeCharacterWorldShape> {
    let datum = visual.anim_datums.get(&datum_hashcode)?;
    let track = visual
        .animation_mode_datum_tracks
        .get(&(anim_mode, datum_hashcode))?;
    if track.animskin != datum.animskin {
        return None;
    }
    let poses = runtime_character_track_local_poses(track, animation_seconds)?;
    let profile = ProcessedCharacterCollisionProfile {
        animskin: datum.animskin,
        shape: datum.shape,
        local_center: datum.local_center,
        local_orientation: datum.local_orientation,
        transform_selector: datum.transform_selector,
    };
    Some(runtime_world_shape_for_collision_profile(
        owner_position,
        owner_rotation,
        owner_scale,
        &profile,
        runtime_character_chain_matrix(&poses),
    ))
}

fn runtime_character_blended_local_poses(
    outgoing: &[ProcessedCharacterAnimationBonePose],
    incoming: &[ProcessedCharacterAnimationBonePose],
    incoming_weight: f32,
) -> Option<Vec<ProcessedCharacterAnimationBonePose>> {
    if outgoing.len() != incoming.len() || outgoing.is_empty() {
        return None;
    }
    let alpha = incoming_weight.clamp(0.0, 1.0);
    Some(
        outgoing
            .iter()
            .zip(incoming)
            .map(|(outgoing, incoming)| {
                let position = outgoing.position.lerp(incoming.position, alpha);
                let mut incoming_rotation = incoming.rotation;
                if outgoing.rotation.dot(incoming_rotation) < 0.0 {
                    incoming_rotation = -incoming_rotation;
                }
                let rotation = runtime_normalized_quat_or_identity(
                    outgoing.rotation.slerp(incoming_rotation, alpha),
                );
                ProcessedCharacterAnimationBonePose { position, rotation }
            })
            .collect(),
    )
}

#[allow(dead_code)]
fn runtime_character_blended_chain_matrix(
    outgoing: &[ProcessedCharacterAnimationBonePose],
    incoming: &[ProcessedCharacterAnimationBonePose],
    incoming_weight: f32,
) -> Option<Mat4> {
    runtime_character_blended_local_poses(outgoing, incoming, incoming_weight)
        .map(|poses| runtime_character_chain_matrix(&poses))
}

pub(crate) fn runtime_world_shape_for_collision_profile(
    owner_position: Vec3,
    owner_rotation: Quat,
    owner_scale: Vec3,
    profile: &ProcessedCharacterCollisionProfile,
    bone_matrix: Mat4,
) -> RuntimeCharacterWorldShape {
    let owner_matrix =
        Mat4::from_scale_rotation_translation(owner_scale, owner_rotation, owner_position);
    let world_from_datum = owner_matrix * bone_matrix;
    let (scale, transform_rotation, _) = world_from_datum.to_scale_rotation_translation();
    let max_scale = scale.abs().max_element();
    let center = world_from_datum.transform_point3(profile.local_center);
    let local_orientation =
        runtime_normalized_quat_or_identity(Quat::from_array(profile.local_orientation));
    let world_orientation =
        runtime_normalized_quat_or_identity(transform_rotation * local_orientation);

    let local_shape = match profile.shape {
        ProcessedCharacterCollisionShape::Sphere { radius } => {
            RobotsHitLocalShape::Sphere { radius }
        }
        ProcessedCharacterCollisionShape::Capsule {
            half_segment,
            radius,
        } => RobotsHitLocalShape::Capsule {
            half_segment,
            radius,
        },
    };
    let axis_y = (world_orientation * Vec3::Y).normalize_or_zero();
    robots_resolve_hit_shape(
        local_shape,
        RobotsResolvedHitDatumPose {
            center_xyz: center.to_array(),
            axis_y_xyz: axis_y.to_array(),
            max_abs_scale: max_scale,
        },
    )
}

pub(crate) fn runtime_character_world_shape_from_bone_matrix(
    body: &RuntimeCharacterBodyState,
    bone_matrix: Mat4,
) -> RuntimeCharacterWorldShape {
    runtime_world_shape_for_collision_profile(
        body.owner_position,
        body.owner_rotation,
        body.native_transform_scale_xyz(),
        &body.collision,
        bone_matrix,
    )
}

/// Minimal editor-side equivalent of the native XItem owner + Physics body
/// bootstrap. It intentionally preserves the owner pose and AnimSkin-local
/// collision datum separately: `HT_AnimDatum_MapCollisionCapsule +0x30`
/// selects an animated bone transform in `0x00500569`, so a world-space shape
/// must not be invented from the camera proxy or from trigger root pose alone.
#[derive(Clone, Debug)]
pub(crate) struct RuntimeCharacterBodyState {
    pub(crate) registration_mask: u32,
    /// Native `XItem+0x260`, copied by `0x0047E740` from the serialized-trigger
    /// class table at `0x0061F380`. All shipped AI-character families are group 1.
    pub(crate) raw_hit_query_group: i32,
    /// Native Hittable query serial at Handler+0x37C. The embedded Character
    /// query record ctor (`0x004254F0` at Handler+0x2EC) seeds this to 0xFFFF.
    pub(crate) last_hit_query_serial: u16,
    pub(crate) owner_position: Vec3,
    pub(crate) owner_rotation: Quat,
    /// Native XItem +0xF0..+0xFC transform-scale vector. XItem ctor seeds all
    /// four components to 1.0; class first-update hooks such as JailBotLarge
    /// `0x00462BE0 -> 0x00455E60(2.0)` multiply them once.
    pub(crate) native_transform_scale: [f32; 4],
    pub(crate) collision: ProcessedCharacterCollisionProfile,
    /// Candidate-side HT_AnimDatum_HitArea used by common hit queries. This must
    /// never be substituted with MapCollisionCapsule merely because both are shapes.
    pub(crate) hit_area: Option<ProcessedCharacterCollisionProfile>,
    pub(crate) initial_animation: Option<Arc<ProcessedCharacterAnimationTrack>>,
    /// Data-driven catalog of gameplay-selectable AnimMode tracks. Concrete
    /// brains own mode selection; this body only owns pose/root-motion projection.
    pub(crate) animation_modes: BTreeMap<Hashcode, Arc<ProcessedCharacterAnimationTrack>>,
    /// Native AnimScript timelines associated with gameplay-selectable AnimModes.
    /// The body stores immutable decoded data; execution state lives in the AI host.
    pub(crate) animation_mode_scripts: BTreeMap<Hashcode, Arc<UXGeoScript>>,
    /// Predecoded native `HT_AnimMode_Move` result, if this character EDB
    /// exposes that mode through an unambiguous Default transition. Diagnostic
    /// only until the gameplay state-machine ingress that activates Move is replayed.
    pub(crate) move_animation: Option<Arc<ProcessedCharacterAnimationTrack>>,
    /// Predecoded directional root-motion clips used by AI vslot +0x118 when
    /// Handler+0x628 bit0x4 delegates turning to animation instead of directly
    /// writing XItem+0xE4.
    pub(crate) turn_on_spot_l_animation: Option<Arc<ProcessedCharacterAnimationTrack>>,
    pub(crate) turn_on_spot_r_animation: Option<Arc<ProcessedCharacterAnimationTrack>>,
    /// Elapsed seconds since the current map runtime preview started. This drives
    /// only the native-proven freshly-created layer-0 Idle_Attack state; later AI
    /// transitions remain a separate gameplay boundary.
    pub(crate) initial_animation_seconds: f32,
    /// Current AnimSkin-local pose cache for a gameplay-selected animation. This
    /// is the editor-side projection of the native per-bone SkinAnim/current-matrix
    /// cache, not another animation controller. AnimMode/timing ownership stays in
    /// the concrete gameplay runtime (NPC/AI/etc.).
    active_collision_local_poses: Option<Vec<ProcessedCharacterAnimationBonePose>>,
    /// Bone identity for `active_collision_local_poses`. Length equality alone is
    /// insufficient for native per-bone blending; the root-to-selector chain must
    /// match exactly across an animation transition.
    active_collision_bone_chain: Option<Vec<usize>>,
    /// Outgoing local-pose snapshot captured on an actual AnimMode edge. Native
    /// blends each bone before rebuilding matrices, so keeping local poses avoids
    /// the incorrect shortcut of interpolating final Mat4 values.
    collision_transition_snapshot: Option<Vec<ProcessedCharacterAnimationBonePose>>,
}

impl RuntimeCharacterBodyState {
    pub(crate) fn native_transform_scale_xyz(&self) -> Vec3 {
        Vec3::new(
            self.native_transform_scale[0],
            self.native_transform_scale[1],
            self.native_transform_scale[2],
        )
    }

    pub(crate) fn apply_native_uniform_transform_scale(&mut self, scale: f32) {
        for component in &mut self.native_transform_scale {
            *component *= scale;
        }
    }

    pub(crate) fn from_visual(
        visual: &ProcessedCharacterVisual,
        raw_hit_query_group: i32,
        owner_position: Vec3,
        owner_rotation: Quat,
    ) -> Option<Self> {
        let collision = visual.collision?;
        Some(Self {
            registration_mask: ROBOTS_GAMEPLAY_COLLISION_REGISTRATION_MASK,
            raw_hit_query_group,
            last_hit_query_serial: u16::MAX,
            owner_position,
            owner_rotation,
            native_transform_scale: [1.0; 4],
            collision,
            hit_area: visual.hit_area,
            initial_animation: visual.initial_animation.clone(),
            animation_modes: visual.animation_modes.clone(),
            animation_mode_scripts: visual.animation_mode_scripts.clone(),
            move_animation: visual.move_animation.clone(),
            turn_on_spot_l_animation: visual.turn_on_spot_l_animation.clone(),
            turn_on_spot_r_animation: visual.turn_on_spot_r_animation.clone(),
            initial_animation_seconds: 0.0,
            active_collision_local_poses: None,
            active_collision_bone_chain: None,
            collision_transition_snapshot: None,
        })
    }

    pub(crate) fn from_trigger(trigger: &ProcessedTrigger) -> Option<Self> {
        let raw_hit_query_group = robots_character_hit_query_raw_group(trigger.ttype)?;
        let visual = trigger.character_visual.as_ref()?;
        // Maps render/runtime already reconstructs Robots trigger orientation
        // in this exact ZXY order. Keep body ownership consistent with that
        // proven scene convention while leaving the AnimDatum bone transform
        // as a separate later stage.
        let owner_rotation = Quat::from_euler(
            glam::EulerRot::ZXY,
            trigger.rotation[2],
            trigger.rotation[0],
            trigger.rotation[1],
        );
        Self::from_visual(
            visual,
            raw_hit_query_group,
            trigger.position,
            owner_rotation,
        )
    }

    pub(crate) fn sync_initial_animation(&mut self, trigger: &ProcessedTrigger, seconds: f32) {
        self.initial_animation = trigger
            .character_visual
            .as_ref()
            .and_then(|visual| visual.initial_animation.clone());
        self.animation_modes = trigger
            .character_visual
            .as_ref()
            .map(|visual| visual.animation_modes.clone())
            .unwrap_or_default();
        self.animation_mode_scripts = trigger
            .character_visual
            .as_ref()
            .map(|visual| visual.animation_mode_scripts.clone())
            .unwrap_or_default();
        self.move_animation = trigger
            .character_visual
            .as_ref()
            .and_then(|visual| visual.move_animation.clone());
        self.turn_on_spot_l_animation = trigger
            .character_visual
            .as_ref()
            .and_then(|visual| visual.turn_on_spot_l_animation.clone());
        self.turn_on_spot_r_animation = trigger
            .character_visual
            .as_ref()
            .and_then(|visual| visual.turn_on_spot_r_animation.clone());
        self.initial_animation_seconds = seconds.max(0.0);
    }

    pub(crate) fn initial_animation_bone_matrix(&self) -> Mat4 {
        let Some(track) = self.initial_animation.as_ref() else {
            return Mat4::IDENTITY;
        };

        // Fresh native node flags are 0x3, so layer-0 Idle_Attack loops. Native
        // 0x0050C644 converts the serialized +0x0C byte to rate/60 per fixed tick,
        // base node ctor 0x0054F801 seeds node+0x2C=1.0, and Robots advances at
        // 60 fixed ticks/s. Converting that normalized clock back to wall seconds
        // cancels the /60, leaving frame_position = seconds * serialized rate.
        runtime_character_track_local_poses(track, self.initial_animation_seconds)
            .map(|poses| runtime_character_chain_matrix(&poses))
            .unwrap_or(Mat4::IDENTITY)
    }

    fn current_collision_pose_snapshot(
        &self,
    ) -> Option<(Vec<ProcessedCharacterAnimationBonePose>, Vec<usize>)> {
        if let (Some(poses), Some(chain)) = (
            self.active_collision_local_poses.as_ref(),
            self.active_collision_bone_chain.as_ref(),
        ) {
            return Some((poses.clone(), chain.clone()));
        }
        let track = self.initial_animation.as_ref()?;
        let poses = runtime_character_track_local_poses(track, self.initial_animation_seconds)?;
        Some((poses, track.bone_chain.clone()))
    }

    /// Update the collision-side AnimSkin pose cache from an animation selected
    /// by an existing gameplay owner. The gameplay runtime owns AnimMode and
    /// clocks; this body owns only the per-bone pose cache consumed by hit shapes.
    pub(crate) fn update_collision_animation_track(
        &mut self,
        track: &ProcessedCharacterAnimationTrack,
        track_seconds: f32,
        blend_elapsed_seconds: f32,
        start_transition: bool,
    ) -> bool {
        let Some(incoming) = runtime_character_track_local_poses(track, track_seconds) else {
            return false;
        };

        if start_transition {
            let Some((snapshot, snapshot_chain)) = self.current_collision_pose_snapshot() else {
                return false;
            };
            if snapshot_chain != track.bone_chain || snapshot.len() != incoming.len() {
                return false;
            }
            self.collision_transition_snapshot = Some(snapshot);
        } else if self
            .active_collision_bone_chain
            .as_ref()
            .is_some_and(|chain| chain != &track.bone_chain)
        {
            return false;
        }

        let next = if let Some(snapshot) = self.collision_transition_snapshot.as_ref() {
            let alpha = if track.transition_fixed_ticks == 0 {
                1.0
            } else {
                let transition_seconds =
                    track.transition_fixed_ticks as f32 / ROBOTS_CHARACTER_ANIMATION_FIXED_HZ;
                (blend_elapsed_seconds.max(0.0) / transition_seconds).clamp(0.0, 1.0)
            };
            let Some(blended) = runtime_character_blended_local_poses(snapshot, &incoming, alpha)
            else {
                return false;
            };
            if alpha >= 1.0 {
                self.collision_transition_snapshot = None;
            }
            blended
        } else {
            incoming
        };
        self.active_collision_local_poses = Some(next);
        self.active_collision_bone_chain = Some(track.bone_chain.clone());
        true
    }

    pub(crate) fn clear_collision_animation_track(&mut self) {
        self.active_collision_local_poses = None;
        self.active_collision_bone_chain = None;
        self.collision_transition_snapshot = None;
    }

    pub(crate) fn current_collision_bone_matrix(&self) -> Mat4 {
        self.active_collision_local_poses
            .as_ref()
            .map(|poses| runtime_character_chain_matrix(poses))
            .unwrap_or_else(|| self.initial_animation_bone_matrix())
    }

    /// Reproduce the proven native layer-0 snapshot->Move transition after a
    /// gameplay caller has already requested `HT_AnimMode_Move`.
    ///
    /// This function deliberately does not decide *when* Move should start.
    /// Native AI owns that decision (for example MineBot's target-distance
    /// action). It only reproduces the animation-side result: the outgoing pose
    /// is snapshotted at transition ingress, Move starts at weight 0, its weight
    /// rises linearly by 0.2 per fixed tick, and once it reaches 1 the layer
    /// cleanup removes the snapshot.
    #[allow(dead_code)]
    pub(crate) fn move_transition_bone_matrix(
        &self,
        outgoing_initial_seconds: f32,
        move_seconds: f32,
        blend_elapsed_seconds: f32,
    ) -> Mat4 {
        let (Some(outgoing_track), Some(move_track)) = (
            self.initial_animation.as_ref(),
            self.move_animation.as_ref(),
        ) else {
            return self.initial_animation_bone_matrix();
        };
        if outgoing_track.bone_chain != move_track.bone_chain {
            return self.initial_animation_bone_matrix();
        }
        let Some(outgoing) =
            runtime_character_track_local_poses(outgoing_track, outgoing_initial_seconds)
        else {
            return Mat4::IDENTITY;
        };
        let Some(incoming) = runtime_character_track_local_poses(move_track, move_seconds) else {
            return runtime_character_chain_matrix(&outgoing);
        };
        let incoming_weight =
            (blend_elapsed_seconds.max(0.0) / ROBOTS_CHARACTER_MOVE_BLEND_SECONDS).min(1.0);
        runtime_character_blended_chain_matrix(&outgoing, &incoming, incoming_weight)
            .unwrap_or_else(|| runtime_character_chain_matrix(&outgoing))
    }

    /// Apply one native local root-motion delta to the live XItem owner pose.
    /// `0x00405450` rotates translation by the current owner orientation and
    /// routes the animation delta through Physics; `0x00419400` commits the
    /// resulting position/orientation. Keep that ownership here rather than in
    /// NPC behavior code so other AI families can reuse the same contract.
    pub(crate) fn apply_local_root_motion_delta(&mut self, delta: RuntimeCharacterRootMotionDelta) {
        let owner_rotation = self.owner_rotation;
        self.owner_position += owner_rotation * delta.native_translation;
        self.owner_rotation =
            runtime_normalized_quat_or_identity(owner_rotation * delta.native_rotation);
    }

    pub(crate) fn current_world_shape(&self) -> RuntimeCharacterWorldShape {
        runtime_character_world_shape_from_bone_matrix(self, self.current_collision_bone_matrix())
    }

    pub(crate) fn current_hit_area_world_shape(&self) -> Option<RuntimeCharacterWorldShape> {
        let hit_area = self.hit_area.as_ref()?;
        if hit_area.animskin != self.collision.animskin
            || hit_area.transform_selector != self.collision.transform_selector
        {
            return None;
        }
        Some(runtime_world_shape_for_collision_profile(
            self.owner_position,
            self.owner_rotation,
            self.native_transform_scale_xyz(),
            hit_area,
            self.current_collision_bone_matrix(),
        ))
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RuntimeEventPreviewState {
    pub(crate) active: bool,
    pub(crate) elapsed_seconds: f32,
    pub(crate) direction: f32,
    pub(crate) distance_offset: f32,
    pub(crate) last_wall_time: Option<f64>,
    pub(crate) last_event: Option<u32>,
    pub(crate) last_node_index: Option<usize>,
    pub(crate) last_node_opcode: Option<u16>,
    vehicle_steering_angle: f32,
    vehicle_previous_heading: f32,
    vehicle_steering_wall_accumulator: f32,
    vehicle_steering_motion_time: f32,
    platform_contact_linear_velocity: Vec3,
    platform_contact_previous_position: Option<Vec3>,
    platform_contact_wall_accumulator: f32,
    platform_contact_motion_time: f32,
}

impl Default for RuntimeEventPreviewState {
    fn default() -> Self {
        Self {
            active: false,
            elapsed_seconds: 0.0,
            direction: 1.0,
            distance_offset: 0.0,
            last_wall_time: None,
            last_event: None,
            last_node_index: None,
            last_node_opcode: None,
            vehicle_steering_angle: 0.0,
            // XItemHandler_Vehicle constructor at 0x0041819B writes -1000.0
            // to handler+0x308 before the first fixed-step update replaces it.
            vehicle_previous_heading: -1000.0,
            vehicle_steering_wall_accumulator: 0.0,
            vehicle_steering_motion_time: 0.0,
            platform_contact_linear_velocity: Vec3::ZERO,
            platform_contact_previous_position: None,
            platform_contact_wall_accumulator: 0.0,
            platform_contact_motion_time: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RuntimeEventPreviewSnapshot {
    pub(crate) active: bool,
    pub(crate) elapsed_seconds: f32,
    pub(crate) path_distance: f32,
    pub(crate) direction_reversed: bool,
    pub(crate) last_event: Option<u32>,
    pub(crate) last_node_index: Option<usize>,
    pub(crate) last_node_opcode: Option<u16>,
    pub(crate) vehicle_steering_angle: Option<f32>,
    pub(crate) platform_contact_linear_velocity: Option<Vec3>,
}

impl RuntimeEventPreviewState {
    pub(crate) fn reset(&mut self, wall_time: f64) {
        *self = Self {
            last_wall_time: Some(wall_time),
            ..Default::default()
        };
    }

    pub(crate) fn advance(&mut self, wall_time: f64) {
        if let Some(previous) = self.last_wall_time {
            if self.active {
                self.elapsed_seconds += (wall_time - previous).max(0.0) as f32;
            }
        }
        self.last_wall_time = Some(wall_time);
    }

    pub(crate) fn hold(&mut self, wall_time: f64) {
        self.last_wall_time = Some(wall_time);
        self.platform_contact_linear_velocity = Vec3::ZERO;
    }

    pub(crate) fn advance_runtime(
        &mut self,
        map: &ProcessedMap,
        trigger: &ProcessedTrigger,
        wall_time: f64,
        speed_scale: f32,
    ) {
        let delta_seconds = self
            .last_wall_time
            .map(|previous| (wall_time - previous).max(0.0) as f32)
            .unwrap_or_default();
        if matches!(trigger.ttype, 8 | 37 | 80) {
            self.advance_platform_contact_carry(map, trigger, delta_seconds, speed_scale);
        }
        if trigger.ttype == 80 {
            self.advance_vehicle_steering(map, trigger, delta_seconds, speed_scale);
        }
        self.advance(wall_time);
    }

    fn path_distance_at_elapsed(
        &self,
        trigger: &ProcessedTrigger,
        elapsed_seconds: f32,
        speed_scale: f32,
    ) -> f32 {
        let speed_scale = speed_scale.max(0.0);
        let Some(maximum_speed) = robots_trigger_runtime_path_speed(trigger.ttype, &trigger.data)
        else {
            return 0.0;
        };
        let acceleration = robots_trigger_runtime_path_acceleration(trigger.ttype, &trigger.data)
            .unwrap_or_default();
        let raw = runtime_path_travel_distance(
            elapsed_seconds,
            maximum_speed * speed_scale,
            acceleration,
            speed_scale,
        );
        self.distance_offset + self.direction * raw
    }

    fn advance_platform_contact_carry(
        &mut self,
        map: &ProcessedMap,
        trigger: &ProcessedTrigger,
        delta_seconds: f32,
        speed_scale: f32,
    ) {
        const FIXED_STEP: f32 = 1.0 / 60.0;
        const FIXED_HZ: f32 = 60.0;
        const MAX_STEPS_PER_ADVANCE: usize = 200_000;

        let sample_position = |distance: f32| {
            runtime_path_preview_sample_at_distance(map, trigger, distance)
                .map(|sample| sample.position)
                .unwrap_or(trigger.position)
        };
        if self.platform_contact_previous_position.is_none() {
            let distance = self.path_distance_at_elapsed(
                trigger,
                self.platform_contact_motion_time,
                speed_scale,
            );
            self.platform_contact_previous_position = Some(sample_position(distance));
        }

        self.platform_contact_wall_accumulator += delta_seconds.max(0.0);
        let steps =
            ((self.platform_contact_wall_accumulator / FIXED_STEP) + 1.0e-4).floor() as usize;
        let steps = steps.min(MAX_STEPS_PER_ADVANCE);
        self.platform_contact_wall_accumulator =
            (self.platform_contact_wall_accumulator - steps as f32 * FIXED_STEP).max(0.0);

        for _ in 0..steps {
            if self.active {
                self.platform_contact_motion_time += FIXED_STEP;
            }
            let distance = self.path_distance_at_elapsed(
                trigger,
                self.platform_contact_motion_time,
                speed_scale,
            );
            let current_position = sample_position(distance);
            let previous_position = self
                .platform_contact_previous_position
                .unwrap_or(current_position);
            // XItemPhysics_Platform::Contact at 0x0041DFB9..0x0041E2B7
            // converts the platform's one-tick displacement into per-second carry
            // velocity with the exact fixed 60-Hz multiplier.
            self.platform_contact_linear_velocity =
                (current_position - previous_position) * FIXED_HZ;
            self.platform_contact_previous_position = Some(current_position);
        }
    }

    fn advance_vehicle_steering(
        &mut self,
        map: &ProcessedMap,
        trigger: &ProcessedTrigger,
        delta_seconds: f32,
        speed_scale: f32,
    ) {
        const FIXED_STEP: f32 = 1.0 / 60.0;
        const MAX_STEPS_PER_ADVANCE: usize = 200_000;

        self.vehicle_steering_wall_accumulator += delta_seconds.max(0.0);
        // Wall time arrives as f64 but the recovered runtime fields are f32. Keep an
        // exact 60 Hz boundary from occasionally becoming 2.99999 frames after the cast.
        let steps =
            ((self.vehicle_steering_wall_accumulator / FIXED_STEP) + 1.0e-4).floor() as usize;
        let steps = steps.min(MAX_STEPS_PER_ADVANCE);
        self.vehicle_steering_wall_accumulator =
            (self.vehicle_steering_wall_accumulator - steps as f32 * FIXED_STEP).max(0.0);

        for _ in 0..steps {
            if self.active {
                self.vehicle_steering_motion_time += FIXED_STEP;
            }
            let Some(current_heading) = vehicle_heading_at_time(
                map,
                trigger,
                self.vehicle_steering_motion_time,
                speed_scale,
            ) else {
                continue;
            };
            let target = wrap_radians(current_heading - self.vehicle_previous_heading) * 10.0;
            self.vehicle_steering_angle +=
                (target - self.vehicle_steering_angle) * ROBOTS_VEHICLE_STEERING_SMOOTHING;
            self.vehicle_previous_heading = current_heading;
        }
    }

    fn raw_path_distance(&self, trigger: &ProcessedTrigger, speed_scale: f32) -> f32 {
        let speed_scale = speed_scale.max(0.0);
        let Some(maximum_speed) = robots_trigger_runtime_path_speed(trigger.ttype, &trigger.data)
        else {
            return 0.0;
        };
        let acceleration = robots_trigger_runtime_path_acceleration(trigger.ttype, &trigger.data)
            .unwrap_or_default();
        runtime_path_travel_distance(
            self.elapsed_seconds,
            maximum_speed * speed_scale,
            acceleration,
            speed_scale,
        )
    }

    pub(crate) fn dispatch(
        &mut self,
        trigger: &ProcessedTrigger,
        event_mask: u32,
        wall_time: f64,
        speed_scale: f32,
    ) {
        self.advance(wall_time);
        self.last_event = Some(event_mask);

        // Native handlers test activation first. A combined 0x300 mask therefore
        // behaves as activation rather than an immediate start/stop pair.
        if event_mask & ROBOTS_EVENT_ACTIVATE != 0 {
            if self.active {
                let retrigger_reverses =
                    trigger.ttype == 8
                        && trigger.data.get(7).copied().flatten().is_some_and(|flags| {
                            flags & ROBOTS_PLATFORM_RETRIGGER_REVERSE_FLAG != 0
                        });
                if retrigger_reverses {
                    let raw = self.raw_path_distance(trigger, speed_scale);
                    let current = self.distance_offset + self.direction * raw;
                    self.direction = -self.direction;
                    self.distance_offset = current - self.direction * raw;
                }
            } else {
                self.active = true;
            }
        } else if event_mask & ROBOTS_EVENT_DEACTIVATE != 0 {
            self.active = false;
        }
        self.last_wall_time = Some(wall_time);
    }

    pub(crate) fn record_node_dispatch(&mut self, node_index: usize, opcode: u16) {
        self.last_node_index = Some(node_index);
        self.last_node_opcode = Some(opcode);
    }

    pub(crate) fn snapshot(
        &self,
        trigger: &ProcessedTrigger,
        speed_scale: f32,
    ) -> RuntimeEventPreviewSnapshot {
        RuntimeEventPreviewSnapshot {
            active: self.active,
            elapsed_seconds: self.elapsed_seconds,
            path_distance: self.distance_offset
                + self.direction * self.raw_path_distance(trigger, speed_scale),
            direction_reversed: self.direction < 0.0,
            last_event: self.last_event,
            last_node_index: self.last_node_index,
            last_node_opcode: self.last_node_opcode,
            vehicle_steering_angle: (trigger.ttype == 80).then_some(self.vehicle_steering_angle),
            platform_contact_linear_velocity: matches!(trigger.ttype, 8 | 37 | 80)
                .then_some(self.platform_contact_linear_velocity),
        }
    }
}

pub(crate) fn runtime_path_travel_distance(
    elapsed_seconds: f32,
    maximum_speed: f32,
    acceleration_factor: f32,
    default_initial_speed: f32,
) -> f32 {
    const FIXED_HZ: f32 = 60.0;

    let frame_position = elapsed_seconds.max(0.0) * FIXED_HZ;
    let full_frames = frame_position.floor();
    let partial_frame = frame_position - full_frames;

    // Runtime update at 0x004230C9..0x004230F2:
    // current += (maximum - current) * acceleration.
    // A serialized non-zero acceleration starts current speed at zero. Without one,
    // setup initializes current speed to 1.0 and uses an effective factor of 1.0.
    let has_serialized_acceleration = acceleration_factor.abs() > f32::EPSILON;
    let effective_acceleration = if has_serialized_acceleration {
        acceleration_factor
    } else {
        1.0
    };
    let initial_speed = if has_serialized_acceleration {
        0.0
    } else {
        default_initial_speed
    };
    let remaining = 1.0 - effective_acceleration;
    let remaining_power = remaining.powi(full_frames as i32);
    let geometric_sum = (1.0 - remaining_power) / effective_acceleration;
    let full_frame_speed_sum =
        full_frames * maximum_speed + (initial_speed - maximum_speed) * geometric_sum;
    let current_speed = maximum_speed + (initial_speed - maximum_speed) * remaining_power;

    (full_frame_speed_sum + current_speed * partial_frame) / FIXED_HZ
}

const ROBOTS_VEHICLE_PASSIVE_WHEEL: Hashcode = 0x0200_017B;
const ROBOTS_VEHICLE_DRIVE_WHEEL: Hashcode = 0x0200_017A;
const ROBOTS_VEHICLE_STEERING_SMOOTHING: f32 = 0.1;

fn wrap_radians(angle: f32) -> f32 {
    (angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

fn vehicle_heading_at_time(
    map: &ProcessedMap,
    trigger: &ProcessedTrigger,
    elapsed_seconds: f32,
    speed_scale: f32,
) -> Option<f32> {
    let speed_scale = speed_scale.max(0.0);
    let maximum_speed =
        robots_trigger_runtime_path_speed(trigger.ttype, &trigger.data)? * speed_scale;
    let acceleration =
        robots_trigger_runtime_path_acceleration(trigger.ttype, &trigger.data).unwrap_or_default();
    let traveled =
        runtime_path_travel_distance(elapsed_seconds, maximum_speed, acceleration, speed_scale);
    runtime_path_preview_sample_at_distance(map, trigger, traveled)
        .and_then(|sample| robots_vehicle_yaw_from_tangent(sample.tangent))
}

pub(crate) fn robots_vehicle_steering_wheel_angle(
    map: &ProcessedMap,
    trigger: &ProcessedTrigger,
    elapsed_seconds: f32,
    speed_scale: f32,
) -> Option<f32> {
    if trigger.ttype != 80 {
        return None;
    }
    let mut state = RuntimeEventPreviewState {
        active: true,
        ..Default::default()
    };
    state.advance_vehicle_steering(map, trigger, elapsed_seconds.max(0.0), speed_scale);
    Some(state.vehicle_steering_angle)
}

pub(crate) fn robots_vehicle_wheel_roll_angle_unwrapped(
    elapsed_seconds: f32,
    maximum_speed: f32,
    acceleration_factor: f32,
    default_initial_speed: f32,
) -> f32 {
    const FIXED_HZ: f32 = 60.0;
    const MAX_EXACT_STEPS: usize = 200_000;

    let frame_position = elapsed_seconds.max(0.0) * FIXED_HZ;
    let full_frames = frame_position.floor() as usize;
    let partial_frame = frame_position - full_frames as f32;
    let has_serialized_acceleration = acceleration_factor.abs() > f32::EPSILON;
    let effective_acceleration = if has_serialized_acceleration {
        acceleration_factor
    } else {
        1.0
    };
    let mut current_speed = if has_serialized_acceleration {
        0.0
    } else {
        default_initial_speed
    };
    let mut angle = 0.0f32;
    let exact_steps = full_frames.min(MAX_EXACT_STEPS);
    for _ in 0..exact_steps {
        angle -= 2.0 * (current_speed * 0.02).clamp(-1.0, 1.0).asin();
        current_speed += (maximum_speed - current_speed) * effective_acceleration;
    }

    if full_frames > exact_steps {
        let remaining = full_frames - exact_steps;
        angle -= remaining as f32 * 2.0 * (maximum_speed * 0.02).clamp(-1.0, 1.0).asin();
        current_speed = maximum_speed;
    }
    angle -= partial_frame * 2.0 * (current_speed * 0.02).clamp(-1.0, 1.0).asin();
    angle
}

pub(crate) fn robots_vehicle_wheel_roll_angle(
    elapsed_seconds: f32,
    maximum_speed: f32,
    acceleration_factor: f32,
    default_initial_speed: f32,
) -> f32 {
    robots_vehicle_wheel_roll_angle_unwrapped(
        elapsed_seconds,
        maximum_speed,
        acceleration_factor,
        default_initial_speed,
    )
    .rem_euclid(std::f32::consts::TAU)
}

pub(crate) fn apply_vehicle_wheel_roll_angle(
    queue: &mut [QueuedEntityRender],
    render_store: &RenderStore,
    angle: f32,
) {
    let roll = Quat::from_rotation_x(angle.rem_euclid(std::f32::consts::TAU));
    for queued in queue {
        if queued.entity_alt.is_some() {
            continue;
        }
        if render_store.resolve_entity_hashcode(queued.entity.0, queued.entity.1)
            == Some(ROBOTS_VEHICLE_PASSIVE_WHEEL)
        {
            queued.rotation *= roll;
        }
    }
}

pub(crate) fn apply_vehicle_wheel_roll(
    queue: &mut [QueuedEntityRender],
    render_store: &RenderStore,
    elapsed_seconds: f32,
    maximum_speed: f32,
    acceleration_factor: f32,
    default_initial_speed: f32,
) {
    let angle = robots_vehicle_wheel_roll_angle(
        elapsed_seconds,
        maximum_speed,
        acceleration_factor,
        default_initial_speed,
    );
    // 0x0200017B is thin on local X and the runtime wheel record applies the
    // accumulated +0x0C angle after the assembly controller transform.
    apply_vehicle_wheel_roll_angle(queue, render_store, angle);
}

pub(crate) fn apply_vehicle_steering_wheel_angle(
    queue: &mut [QueuedEntityRender],
    render_store: &RenderStore,
    angle: f32,
) {
    let steering = Quat::from_rotation_y(angle);
    for queued in queue {
        if queued.entity_alt.is_some() {
            continue;
        }
        let resolved = render_store.resolve_entity_hashcode(queued.entity.0, queued.entity.1);
        if matches!(
            resolved,
            Some(ROBOTS_VEHICLE_DRIVE_WHEEL) | Some(ROBOTS_VEHICLE_PASSIVE_WHEEL)
        ) {
            // Both native wheel-update branches call 0x00419010, smooth
            // record+0x08 and compose Euler (0, steering, 0). Mode 0 passive
            // road wheels do this after their record+0x0C local-X roll; mode 1
            // drive/cab wheels apply steering without the road-wheel roll.
            queued.rotation *= steering;
        }
    }
}

pub(crate) fn sample_route(
    segments: &[(Vec3, Vec3)],
    mut distance: f32,
    reverse_tangent: bool,
    root_offset: Vec3,
) -> Option<RuntimePathSample> {
    for (start, end) in segments {
        let segment_length = start.distance(*end);
        if segment_length <= f32::EPSILON {
            continue;
        }
        if distance <= segment_length {
            let mut tangent = (*end - *start) / segment_length;
            if reverse_tangent {
                tangent = -tangent;
            }
            return Some(RuntimePathSample {
                position: start.lerp(*end, distance / segment_length) + root_offset,
                tangent,
            });
        }
        distance -= segment_length;
    }

    let (start, end) = segments.last().copied()?;
    let mut tangent = (end - start).normalize_or_zero();
    if reverse_tangent {
        tangent = -tangent;
    }
    Some(RuntimePathSample {
        position: end + root_offset,
        tangent,
    })
}

pub(crate) fn runtime_path_preview_sample_at_distance(
    map: &ProcessedMap,
    trigger: &ProcessedTrigger,
    traveled: f32,
) -> Option<RuntimePathSample> {
    let (_, _, path) = map_trigger_runtime_path(map, trigger)?;
    let route = runtime_path_route(path);
    if route.len() < 2 {
        return None;
    }

    // Shipped Vehicle paths carry 0x60000000 and are consumed by
    // XPathController_Vehicle as continuous traffic routes. Platform/Lift routes
    // retain the controller's end-to-end reversal preview.
    let looping = trigger.ttype == 80 && path.flags & 0x6000_0000 == 0x6000_0000;
    let segments = runtime_path_segments_for_motion(&route, looping);
    let total_length = segments
        .iter()
        .map(|(start, end)| start.distance(*end))
        .sum::<f32>();
    if total_length <= f32::EPSILON {
        return None;
    }

    let (start_phase, root_offset) = closest_route_phase(&segments, trigger.position);
    if looping {
        let distance = (start_phase + traveled).rem_euclid(total_length);
        sample_route(&segments, distance, traveled < 0.0, root_offset)
    } else {
        let cycle_length = total_length * 2.0;
        let phase = (start_phase + traveled).rem_euclid(cycle_length);
        let (distance, reverse_tangent) = if phase > total_length {
            (cycle_length - phase, true)
        } else {
            (phase, false)
        };
        sample_route(&segments, distance, reverse_tangent, root_offset)
    }
}

pub(crate) fn runtime_path_preview_sample(
    map: &ProcessedMap,
    trigger: &ProcessedTrigger,
    time: f32,
    animate: bool,
    speed_scale: f32,
) -> Option<RuntimePathSample> {
    if !animate || speed_scale <= f32::EPSILON {
        return runtime_path_preview_sample_at_distance(map, trigger, 0.0);
    }
    let speed_scale = speed_scale.max(0.0);
    let maximum_speed =
        robots_trigger_runtime_path_speed(trigger.ttype, &trigger.data)? * speed_scale;
    let acceleration =
        robots_trigger_runtime_path_acceleration(trigger.ttype, &trigger.data).unwrap_or_default();
    let traveled = runtime_path_travel_distance(time, maximum_speed, acceleration, speed_scale);
    runtime_path_preview_sample_at_distance(map, trigger, traveled)
}

pub(crate) fn runtime_path_preview_position(
    map: &ProcessedMap,
    trigger: &ProcessedTrigger,
    time: f32,
    animate: bool,
    speed_scale: f32,
) -> Vec3 {
    runtime_path_preview_sample(map, trigger, time, animate, speed_scale)
        .map(|sample| sample.position)
        .unwrap_or(trigger.position)
}

pub(crate) fn runtime_path_preview_position_with_event(
    map: &ProcessedMap,
    trigger: &ProcessedTrigger,
    time: f32,
    animate: bool,
    speed_scale: f32,
    event_snapshot: Option<RuntimeEventPreviewSnapshot>,
) -> Vec3 {
    if let Some(snapshot) = event_snapshot {
        return runtime_path_preview_sample_at_distance(map, trigger, snapshot.path_distance)
            .map(|sample| sample.position)
            .unwrap_or(trigger.position);
    }
    runtime_path_preview_position(map, trigger, time, animate, speed_scale)
}

pub(crate) fn runtime_platform_contact_linear_velocity(
    map: &ProcessedMap,
    trigger: &ProcessedTrigger,
    time: f32,
    animate: bool,
    speed_scale: f32,
) -> Option<Vec3> {
    if !matches!(trigger.ttype, 8 | 37 | 80) {
        return None;
    }
    if !animate {
        return Some(Vec3::ZERO);
    }
    const FIXED_STEP: f32 = 1.0 / 60.0;
    const FIXED_HZ: f32 = 60.0;
    let current_time = time.max(0.0);
    let previous_time = (current_time - FIXED_STEP).max(0.0);
    let current = runtime_path_preview_position(map, trigger, current_time, true, speed_scale);
    let previous = runtime_path_preview_position(map, trigger, previous_time, true, speed_scale);
    Some((current - previous) * FIXED_HZ)
}

#[allow(dead_code)] // Kept dormant until Maps owns a real gameplay peer-body registration state.
pub(crate) fn runtime_platform_one_tick_angular_delta(
    degrees_per_second: Vec3,
    fixed_step_count: u8,
) -> Vec3 {
    degrees_per_second * (fixed_step_count as f32 * (1.0 / 60.0)).to_radians()
}

#[allow(dead_code)] // Do not bind this to the editor player proxy; it requires native peer-body state.
pub(crate) fn runtime_platform_peer_body_carry_velocity(
    linear_velocity: Vec3,
    platform_origin: Vec3,
    peer_origin: Vec3,
    one_tick_euler_radians: Vec3,
    frame_step_scalar: f32,
) -> Vec3 {
    if frame_step_scalar.abs() <= f32::EPSILON {
        return linear_velocity;
    }

    // XItemPhysics_Platform::ContactPointVelocityTransfer (0x0041DCF0):
    // the peer XItem pose origin is rotated around the platform-owner origin.
    // The native finite rotation is Ry * Rx * Rz, not an omega-cross-radius
    // approximation and not a collision-manifold contact point.
    let angles = one_tick_euler_radians * frame_step_scalar;
    let (sin_x, cos_x) = angles.x.sin_cos();
    let (sin_y, cos_y) = angles.y.sin_cos();
    let (sin_z, cos_z) = angles.z.sin_cos();
    let relative = peer_origin - platform_origin;
    let rotated = Vec3::new(
        (cos_y * cos_z + sin_z * sin_y * sin_x) * relative.x
            + (cos_z * sin_y * sin_x - sin_z * cos_y) * relative.y
            + (sin_y * cos_x) * relative.z,
        (sin_z * cos_x) * relative.x + (cos_z * cos_x) * relative.y - sin_x * relative.z,
        (sin_z * cos_y * sin_x - cos_z * sin_y) * relative.x
            + (cos_y * cos_z * sin_x + sin_z * sin_y) * relative.y
            + (cos_x * cos_y) * relative.z,
    );

    linear_velocity + (rotated - relative) * (60.0 / frame_step_scalar)
}

#[allow(dead_code)] // Native gameplay spheres only; never substitute the editor player/camera proxy.
pub(crate) fn runtime_sphere_sphere_impulse_velocity(
    self_velocity: Vec3,
    peer_velocity: Vec3,
    contact_normal: Vec3,
    self_mass: f32,
    peer_mass: f32,
    self_restitution: f32,
    peer_restitution: f32,
) -> Option<(Vec3, Vec3)> {
    if !self_velocity.is_finite()
        || !peer_velocity.is_finite()
        || !contact_normal.is_finite()
        || !self_mass.is_finite()
        || !peer_mass.is_finite()
        || !self_restitution.is_finite()
        || !peer_restitution.is_finite()
        || self_mass <= f32::EPSILON
        || peer_mass <= f32::EPSILON
    {
        return None;
    }

    // EXItemPhysicsSphere::sphere/sphere contact response (0x0050BD80).
    // Serialized/raw +0xF0 is restitution e; initializer 0x00509DBC adds 1.0,
    // so the runtime normal-response multiplier is (1 + e). Native averages
    // both bodies' multipliers, then solves J using 1/(1/m1 + 1/m2).
    let closing_normal_velocity = (self_velocity - peer_velocity).dot(contact_normal);
    if closing_normal_velocity > 0.0 {
        return Some((self_velocity, peer_velocity));
    }
    let one_plus_restitution = ((1.0 + self_restitution) + (1.0 + peer_restitution)) * 0.5;
    let effective_mass = 1.0 / (1.0 / self_mass + 1.0 / peer_mass);
    let impulse =
        contact_normal * (one_plus_restitution * closing_normal_velocity * effective_mass);
    Some((
        self_velocity - impulse / self_mass,
        peer_velocity + impulse / peer_mass,
    ))
}

pub(crate) fn trigger_base_rotation(trigger: &ProcessedTrigger) -> Quat {
    Quat::from_euler(
        glam::EulerRot::ZXY,
        trigger.rotation.z,
        trigger.rotation.x,
        trigger.rotation.y,
    )
}

fn compose_platform_local_rotation(
    base_rotation: Quat,
    degrees_per_second: Vec3,
    time: f32,
    speed_scale: f32,
) -> Quat {
    let elapsed_degrees = degrees_per_second * time.max(0.0) * speed_scale.max(0.0);
    let delta_rotation = Quat::from_euler(
        glam::EulerRot::ZXY,
        elapsed_degrees.z.to_radians(),
        elapsed_degrees.x.to_radians(),
        elapsed_degrees.y.to_radians(),
    );

    // XPathController_Platform serializes angular velocity in the platform body's
    // local XYZ axes. The fixed-step physics path integrates that delta into the
    // existing body quaternion; it is not an absolute world-space Euler pose.
    (base_rotation * delta_rotation).normalize()
}

pub(crate) fn runtime_platform_preview_rotation(
    trigger: &ProcessedTrigger,
    time: f32,
    animate: bool,
    speed_scale: f32,
) -> Quat {
    let base_rotation = trigger_base_rotation(trigger);
    if !animate {
        return base_rotation;
    }
    let Some(degrees_per_second) =
        robots_trigger_platform_angular_velocity(trigger.ttype, &trigger.data)
    else {
        return base_rotation;
    };

    compose_platform_local_rotation(base_rotation, degrees_per_second, time, speed_scale)
}

pub(crate) fn robots_vehicle_yaw_from_tangent(tangent: Vec3) -> Option<f32> {
    let tangent_xz = Vec3::new(tangent.x, 0.0, tangent.z);
    (tangent_xz.length_squared() > f32::EPSILON)
        // XPathController_Vehicle::update at 0x00424C69 computes
        // atan2(-tangent.x, -tangent.z). Robots vehicle models face -Z.
        .then(|| (-tangent_xz.x).atan2(-tangent_xz.z))
}

pub(crate) fn runtime_trigger_preview_rotation_with_event(
    map: &ProcessedMap,
    trigger: &ProcessedTrigger,
    time: f32,
    animate: bool,
    path_speed_scale: f32,
    platform_rotation_speed_scale: f32,
    event_snapshot: Option<RuntimeEventPreviewSnapshot>,
) -> Quat {
    if let Some(snapshot) = event_snapshot {
        if trigger.ttype == 80 {
            if let Some(yaw) =
                runtime_path_preview_sample_at_distance(map, trigger, snapshot.path_distance)
                    .and_then(|sample| robots_vehicle_yaw_from_tangent(sample.tangent))
            {
                return Quat::from_euler(
                    glam::EulerRot::ZXY,
                    trigger.rotation.z,
                    trigger.rotation.x,
                    yaw,
                );
            }
        }
        return runtime_platform_preview_rotation(
            trigger,
            snapshot.elapsed_seconds,
            animate,
            platform_rotation_speed_scale,
        );
    }
    runtime_trigger_preview_rotation(
        map,
        trigger,
        time,
        animate,
        path_speed_scale,
        platform_rotation_speed_scale,
    )
}

pub(crate) fn runtime_trigger_preview_rotation(
    map: &ProcessedMap,
    trigger: &ProcessedTrigger,
    time: f32,
    animate: bool,
    path_speed_scale: f32,
    platform_rotation_speed_scale: f32,
) -> Quat {
    if !animate {
        return trigger_base_rotation(trigger);
    }

    if trigger.ttype == 80 {
        if let Some(yaw) =
            runtime_path_preview_sample(map, trigger, time, animate, path_speed_scale)
                .and_then(|sample| robots_vehicle_yaw_from_tangent(sample.tangent))
        {
            return Quat::from_euler(
                glam::EulerRot::ZXY,
                trigger.rotation.z,
                trigger.rotation.x,
                yaw,
            );
        }
    }

    runtime_platform_preview_rotation(trigger, time, animate, platform_rotation_speed_scale)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eurochef_shared::robots_runtime::hit_narrowphase::{
        RobotsHitSampleSweepMode, ROBOTS_HIT_SAMPLE_MARK_CONSUMED_ON_HIT_FLAG,
    };

    #[test]
    fn global_rng_default_is_unknown_and_cannot_invent_a_map_local_seed() {
        let mut rng = RuntimeRobotsGlobalRngState::default();
        assert_eq!(
            rng.provenance,
            RuntimeRobotsGlobalRngProvenance::UnknownSession
        );
        assert_eq!(rng.seed(), None);
        assert_eq!(rng.next_u32(), None);
        assert_eq!(rng.draws_from_anchor, 0);
    }

    #[test]
    fn global_rng_fresh_process_matches_native_two_step_lcg_sequence() {
        let mut rng = RuntimeRobotsGlobalRngState::fresh_process_startup();
        assert_eq!(rng.seed(), Some(0x955));
        assert_eq!(rng.next_u32(), Some(0x1EA0_6E9E));
        assert_eq!(rng.seed(), Some(0x6E9E_6A77));
        assert_eq!(rng.next_u32(), Some(0x07F7_B91D));
        assert_eq!(rng.seed(), Some(0xB91D_A4A9));
        assert_eq!(rng.next_u32(), Some(0xA285_5273));
        assert_eq!(rng.next_u32(), Some(0x962B_C8B2));
        assert_eq!(rng.next_u32(), Some(0x0F7F_D99E));
        assert_eq!(rng.seed(), Some(0xD99E_B29F));
        assert_eq!(rng.next_u32(), Some(0xB299_08D0));
        assert_eq!(rng.seed(), Some(0x08D0_6E11));
        assert_eq!(rng.draws_from_anchor, 6);
    }

    #[test]
    fn global_rng_float_wrapper_uses_native_two_to_minus_31_scale() {
        let mut rng = RuntimeRobotsGlobalRngState::fresh_process_startup();
        let unit = rng.next_unit_f32().expect("fresh process RNG draw");
        let expected = ((0x1EA0_6E9Eu32 >> 1) as f32) * f32::from_bits(0x3000_0000);
        assert_eq!(unit.to_bits(), expected.to_bits());
        assert!(unit >= 0.0 && unit < 1.0);
    }

    #[test]
    fn global_rng_observed_seed_is_a_distinct_runtime_anchor() {
        let mut rng = RuntimeRobotsGlobalRngState::from_observed_seed(0x1234_5678);
        assert_eq!(
            rng.provenance,
            RuntimeRobotsGlobalRngProvenance::ObservedSeed
        );
        assert_eq!(rng.seed(), Some(0x1234_5678));
        assert!(rng.next_u32().is_some());
        assert_eq!(rng.draws_from_anchor, 1);
        rng.invalidate();
        assert_eq!(
            rng.provenance,
            RuntimeRobotsGlobalRngProvenance::UnknownSession
        );
        assert_eq!(rng.seed(), None);
        assert_eq!(rng.draws_from_anchor, 0);
    }

    #[test]
    fn player_spawn_modes_match_native_trigger_create_switch() {
        let base = Vec3::new(10.0, 20.0, 30.0);
        for mode in [0, 4, 5, 6] {
            assert_eq!(runtime_player_spawn_position(base, mode), base);
        }
        for mode in [1, 2, 3] {
            assert_eq!(
                runtime_player_spawn_position(base, mode),
                Vec3::new(10.0, 22.0, 30.0)
            );
        }
    }

    #[test]
    fn door_event_tracks_native_request_latches_without_inventing_open_state() {
        let mut state = RuntimeTriggerGraphState::default();
        state.dispatch_door(1, 0);
        assert!(state.door.initialized);
        assert_eq!(state.door.state_e4, 0);
        assert_eq!(state.door.request_e5, 1);
        assert_eq!(state.door.request_ed, 0);

        state.door.request_e5 = 0;
        state.dispatch_door(1, 0x400);
        assert_eq!(state.door.state_e4, 0);
        assert_eq!(state.door.request_e5, 1);
        assert_eq!(state.door.request_ed, 0);

        state.door.state_e4 = 1;
        state.door.request_e5 = 0;
        state.dispatch_door(1, 0x400);
        assert_eq!(state.door.state_e4, 1);
        assert_eq!(state.door.request_e5, 0);
        assert_eq!(state.door.request_ed, 1);

        state.dispatch_door(1, 0x800);
        assert_eq!(state.door.state_e4, 1);
        assert_eq!(state.door.request_e5, 0);
        assert_eq!(state.door.request_ed, 0);
    }

    #[test]
    fn fix_switch_service_event_matches_native_reset_latch() {
        let mut state = RuntimeTriggerGraphState::default();
        let ignored = state.dispatch_fix_switch(1, 0x101, true);
        assert!(!ignored.mark_owned_handler_progress_dirty);
        assert!(!state.fix_switch.initialized);
        assert_eq!(state.fix_switch.state_e4, 0);
        assert_eq!(state.fix_switch.active_e8, 0);

        state.fix_switch.timer_ec = 3.0;
        let reset = state.dispatch_fix_switch(1, 0x1000, true);
        assert!(reset.mark_owned_handler_progress_dirty);
        assert!(state.fix_switch.initialized);
        assert_eq!(state.fix_switch.state_e4, 1);
        assert_eq!(state.fix_switch.active_e8, 1);
        assert_eq!(state.fix_switch.timer_ec, 0.0);
    }

    #[test]
    fn interact_and_light_share_native_binary_e4_event_state() {
        let mut state = RuntimeTriggerGraphState::default();
        assert_eq!(state.binary_state_e4, None);
        state.dispatch_binary_active(0x100);
        assert_eq!(state.binary_state_e4, Some(1));
        state.dispatch_binary_active(0x100);
        assert_eq!(state.binary_state_e4, Some(1));
        state.dispatch_binary_active(0x200);
        assert_eq!(state.binary_state_e4, Some(0));

        state.dispatch_binary_active(0x300);
        assert_eq!(state.binary_state_e4, Some(0));
    }

    #[test]
    fn npc_deactivate_event_sets_only_the_proven_runtime_latch() {
        let mut state = RuntimeTriggerGraphState::default();
        state.dispatch_npc(0x1);
        assert!(!state.npc.latch_e10c);
        state.dispatch_npc(0x200);
        assert!(state.npc.latch_e10c);
    }

    #[test]
    fn tutorial_activation_uses_first_and_repeat_native_latches() {
        let mut state = RuntimeTriggerGraphState::default();
        state.dispatch_tutorial(0x1);
        assert!(!state.tutorial_interaction.latch_e8);
        assert_eq!(state.tutorial_repeat_ea, 0);

        state.dispatch_tutorial(0x100);
        assert!(state.tutorial_interaction.latch_e8);
        assert_eq!(state.tutorial_repeat_ea, 0);

        state.dispatch_tutorial(0x100);
        assert!(state.tutorial_interaction.latch_e8);
        assert_eq!(state.tutorial_repeat_ea, 1);
    }

    #[test]
    fn pickup_service_event_rearms_native_eligibility_latch_only() {
        let mut state = RuntimeTriggerGraphState::default();
        assert!(!state.pickup.suppressed_e8);
        state.dispatch_pickup(0x1);
        assert!(!state.pickup.suppressed_e8);

        state.pickup.suppressed_e8 = true;
        state.dispatch_pickup(0x1000);
        assert!(!state.pickup.suppressed_e8);

        state.pickup.suppressed_e8 = true;
        state.dispatch_pickup(0x1001);
        assert!(!state.pickup.suppressed_e8);
    }

    #[test]
    fn owned_xitem_binary_event_requires_current_native_ownership() {
        let mut state = RuntimeTriggerGraphState::default();
        state.dispatch_owned_xitem_binary(0, false, 0x100);
        assert_eq!(state.owned_xitem_active_e4, Some(0));

        state.dispatch_owned_xitem_binary(0, true, 0x100);
        assert_eq!(state.owned_xitem_active_e4, Some(1));

        state.dispatch_owned_xitem_binary(0, false, 0x200);
        assert_eq!(state.owned_xitem_active_e4, Some(1));

        state.dispatch_owned_xitem_binary(0, true, 0x200);
        assert_eq!(state.owned_xitem_active_e4, Some(0));

        let mut loaded_active = RuntimeTriggerGraphState::default();
        loaded_active.dispatch_owned_xitem_binary(1, false, 0);
        assert_eq!(loaded_active.owned_xitem_active_e4, Some(1));
        loaded_active.dispatch_owned_xitem_binary(1, true, 0x300);
        assert_eq!(
            loaded_active.owned_xitem_active_e4,
            Some(0),
            "native handler tests 0x100 first and then independently tests 0x200"
        );
    }

    #[test]
    fn clock_event_matches_native_ordered_binary_latch() {
        let mut state = RuntimeTriggerGraphState::default();
        state.dispatch_clock(0x100);
        assert!(state.clock_active_e4);
        state.dispatch_clock(0x200);
        assert!(!state.clock_active_e4);
        state.dispatch_clock(0x300);
        assert!(!state.clock_active_e4);
    }

    #[test]
    fn mission_event_matches_native_owner_status_and_finalize_order() {
        let mut state = RuntimeTriggerGraphState::default();
        let started = state.dispatch_mission(
            10.0,
            2.0,
            3,
            0,
            0x100,
            Some(NativeMissionStatus::Inactive),
            NativeMissionOwnerRelation::None,
        );
        assert_eq!(started.owner_action, NativeMissionOwnerAction::SetSelf);
        assert_eq!(started.status_update, Some(NativeMissionStatus::Active));
        assert_eq!(started.output_slot, None);
        assert!(state.mission_active_ec);
        assert_eq!(state.mission_timer_e4, 10.0);
        assert_eq!(state.mission_repeat_e8, 2);

        let self_owned_combined = state.dispatch_mission(
            10.0,
            2.0,
            3,
            0,
            0x300,
            Some(NativeMissionStatus::Active),
            NativeMissionOwnerRelation::SelfTrigger,
        );
        assert_eq!(self_owned_combined, NativeMissionDispatchOutcome::default());
        assert!(state.mission_active_ec);
        assert_eq!(state.mission_timer_e4, 12.0);
        assert_eq!(state.mission_repeat_e8, 1);

        let mut foreign = RuntimeTriggerGraphState::default();
        let ignored = foreign.dispatch_mission(
            5.0,
            1.0,
            1,
            0,
            0x300,
            Some(NativeMissionStatus::Inactive),
            NativeMissionOwnerRelation::OtherTrigger,
        );
        assert_eq!(ignored, NativeMissionDispatchOutcome::default());
        assert!(!foreign.mission_active_ec);
        assert_eq!(foreign.mission_timer_e4, f32::from_bits(0x3A83_126F));

        let mut completed = RuntimeTriggerGraphState::default();
        let ignored_completed = completed.dispatch_mission(
            5.0,
            0.0,
            1,
            0,
            0x100,
            Some(NativeMissionStatus::Completed),
            NativeMissionOwnerRelation::None,
        );
        assert_eq!(ignored_completed, NativeMissionDispatchOutcome::default());
        assert!(!completed.mission_active_ec);
    }

    #[test]
    fn mission_event_matches_native_fail_complete_and_retained_owner_paths() {
        let mut failed = RuntimeTriggerGraphState::default();
        let failed_outcome = failed.dispatch_mission(
            0.0,
            0.0,
            0,
            0,
            0x200,
            Some(NativeMissionStatus::Inactive),
            NativeMissionOwnerRelation::None,
        );
        assert_eq!(failed_outcome.owner_action, NativeMissionOwnerAction::Clear);
        assert_eq!(
            failed_outcome.status_update,
            Some(NativeMissionStatus::Failed)
        );
        assert_eq!(failed_outcome.output_slot, Some(7));
        assert_eq!(failed.mission_timer_e4, f32::from_bits(0x3A83_126F));

        let mut active_status = RuntimeTriggerGraphState::default();
        let completed_outcome = active_status.dispatch_mission(
            5.0,
            0.0,
            0,
            0,
            0x100,
            Some(NativeMissionStatus::Active),
            NativeMissionOwnerRelation::None,
        );
        assert_eq!(
            completed_outcome.owner_action,
            NativeMissionOwnerAction::Clear
        );
        assert_eq!(
            completed_outcome.status_update,
            Some(NativeMissionStatus::Completed)
        );
        assert_eq!(completed_outcome.output_slot, Some(6));
        assert_eq!(active_status.mission_timer_e4, 5.0);
        assert!(!active_status.mission_active_ec);

        let mut retained = RuntimeTriggerGraphState::default();
        let retained_outcome = retained.dispatch_mission(
            5.0,
            0.0,
            1,
            0xA000,
            0x300,
            Some(NativeMissionStatus::Inactive),
            NativeMissionOwnerRelation::None,
        );
        assert_eq!(
            retained_outcome.owner_action,
            NativeMissionOwnerAction::SetSelf
        );
        assert_eq!(
            retained_outcome.status_update,
            Some(NativeMissionStatus::Active)
        );
        assert_eq!(retained_outcome.output_slot, Some(7));
        assert_eq!(retained.mission_timer_e4, 5.0);
        assert!(!retained.mission_active_ec);
    }

    #[test]
    fn mission_fixed_tick_matches_native_timer_pause_clock_and_finalize_rules() {
        let mut timed = RuntimeTriggerGraphState::default();
        timed.dispatch_mission(
            1.0,
            0.0,
            1,
            0,
            0x100,
            Some(NativeMissionStatus::Inactive),
            NativeMissionOwnerRelation::None,
        );
        let first = timed.advance_mission_fixed(1.0, 0, None, false, false, true);
        assert!(first.clock_activate);
        assert_eq!(first.dispatch.output_slot, None);
        assert_eq!(timed.mission_timer_e4, 1.0 - f32::from_bits(0x3C88_8889));

        let before_pause = timed.mission_timer_e4;
        let modal_pause = timed.advance_mission_fixed(1.0, 0, Some(1), false, false, true);
        assert_eq!(modal_pause, NativeMissionTickOutcome::default());
        assert_eq!(timed.mission_timer_e4, before_pause);
        let player_pause = timed.advance_mission_fixed(1.0, 0, None, true, false, true);
        assert_eq!(player_pause, NativeMissionTickOutcome::default());
        assert_eq!(timed.mission_timer_e4, before_pause);

        let modal_allowed = timed.advance_mission_fixed(1.0, 0x400, Some(2), false, false, true);
        assert!(!modal_allowed.clock_activate);
        assert!(timed.mission_timer_e4 < before_pause);

        let mut expires = RuntimeTriggerGraphState::default();
        expires.dispatch_mission(
            f32::from_bits(0x3C08_8889),
            0.0,
            1,
            0,
            0x100,
            Some(NativeMissionStatus::Inactive),
            NativeMissionOwnerRelation::None,
        );
        let expired =
            expires.advance_mission_fixed(f32::from_bits(0x3C08_8889), 0, None, false, false, true);
        assert_eq!(expires.mission_timer_e4, 0.0);
        assert_eq!(expired.dispatch.output_slot, Some(7));
        assert_eq!(
            expired.dispatch.status_update,
            Some(NativeMissionStatus::Failed)
        );

        let mut objective = RuntimeTriggerGraphState::default();
        objective.dispatch_mission(
            0.0,
            0.0,
            1,
            0x200,
            0x100,
            Some(NativeMissionStatus::Inactive),
            NativeMissionOwnerRelation::None,
        );
        let completed = objective.advance_mission_fixed(0.0, 0x200, None, false, true, true);
        assert_eq!(completed.dispatch.output_slot, Some(6));
        assert_eq!(
            completed.dispatch.status_update,
            Some(NativeMissionStatus::Completed)
        );
    }

    #[test]
    fn display_message_event_preserves_native_text_uid_and_raw_duration() {
        let mut state = RuntimeTriggerGraphState::default();
        state.dispatch_display_message(0x4500_0350, 300, 0x1, None);
        assert_eq!(state.display_message_uid, None);
        assert_eq!(state.display_message_requests, 0);

        state.dispatch_display_message(0x4500_0350, 300, 0x100, None);
        assert_eq!(state.display_message_uid, Some(0x4500_0350));
        assert_eq!(state.display_message_duration_raw, Some(300));
        assert_eq!(state.display_message_requests, 1);

        state.dispatch_display_message(0x4500_0136, 0, 0x101, None);
        assert_eq!(state.display_message_uid, Some(0x4500_0136));
        assert_eq!(state.display_message_duration_raw, Some(0));
        assert_eq!(state.display_message_requests, 2);

        for modal_top in [1, 2, 3] {
            state.dispatch_display_message(0x4500_0350, 300, 0x100, Some(modal_top));
        }
        assert_eq!(
            state.display_message_requests, 2,
            "native HUD/GameWnd modal states 1/2/3 suppress DisplayMessage queueing"
        );
    }

    #[test]
    fn ball_track_event_matches_native_create_and_two_phase_stop() {
        let mut shipped = RuntimeTriggerGraphState::default();
        let create = shipped.dispatch_ball_track(false, false, 0x100);
        assert!(create.create_owned_track);
        assert!(shipped.ball_track.owned_track_created);
        assert!(!shipped.ball_track.spawn_stopped);
        let ignored = shipped.dispatch_ball_track(false, false, 0x200);
        assert_eq!(ignored, RobotsBallTrackEventStep::default());
        assert!(shipped.ball_track.owned_track_created);

        let mut two_phase = RuntimeTriggerGraphState::default();
        two_phase.dispatch_ball_track(false, true, 0x100);
        let stop = two_phase.dispatch_ball_track(false, true, 0x200);
        assert!(stop.stop_spawning);
        assert!(two_phase.ball_track.owned_track_created);
        assert!(two_phase.ball_track.spawn_stopped);
        let cleanup = two_phase.dispatch_ball_track(false, true, 0x200);
        assert!(cleanup.cleanup_owned_track);
        assert!(!two_phase.ball_track.owned_track_created);
        assert!(!two_phase.ball_track.spawn_stopped);

        let mut restored = RuntimeTriggerGraphState::default();
        let no_action = restored.dispatch_ball_track(true, true, 0);
        assert_eq!(no_action, RobotsBallTrackEventStep::default());
        assert!(restored.ball_track.owned_track_created);
    }

    #[test]
    fn trigger_manager_normal_distance_bands_are_squared_10_20_30() {
        assert_eq!(
            runtime_trigger_distance_event(100.0, 0),
            Some(RuntimeTriggerDistanceEvent::Event0)
        );
        assert_eq!(
            runtime_trigger_distance_event(100.001, 0),
            Some(RuntimeTriggerDistanceEvent::Event1)
        );
        assert_eq!(
            runtime_trigger_distance_event(400.0, 0),
            Some(RuntimeTriggerDistanceEvent::Event1)
        );
        assert_eq!(
            runtime_trigger_distance_event(400.001, 0),
            Some(RuntimeTriggerDistanceEvent::Event2)
        );
        assert_eq!(
            runtime_trigger_distance_event(900.0, 0),
            Some(RuntimeTriggerDistanceEvent::Event2)
        );
        assert_eq!(
            runtime_trigger_distance_event(900.001, 0),
            Some(RuntimeTriggerDistanceEvent::Event3)
        );
        assert_eq!(
            runtime_trigger_distance_event(5000.0, 0x0800_0000),
            Some(RuntimeTriggerDistanceEvent::Event2)
        );
        assert_eq!(runtime_trigger_distance_event(1.0, 0x0008_0000), None);
        assert_eq!(ROBOTS_TRIGGER_CREATE_BUDGET_PER_UPDATE, 30);
    }

    #[test]
    fn script_trigger_lifecycle_tracks_native_create_delay_fade_and_cleanup() {
        let mut state = RuntimeScriptTriggerLifecycleState::default();
        let mut creates = 0;
        state.advance_trigger_manager(
            RuntimeCommonTriggerLifecycleAction::DelayedActive { ticks: 30 },
            0,
            0.0,
            &mut creates,
        );
        assert!(state.xitem_exists);
        assert_eq!(state.xitem_state, 1);
        assert_eq!(state.delay_counter, 1);
        assert_eq!(state.current_opacity, 1.0);
        assert_eq!(creates, 1);

        for _ in 1..30 {
            state.advance_trigger_manager(
                RuntimeCommonTriggerLifecycleAction::DelayedActive { ticks: 30 },
                0,
                0.0,
                &mut creates,
            );
        }
        assert_eq!(state.delay_counter, 0);
        assert_eq!(creates, 1);

        state.advance_trigger_manager(
            RuntimeCommonTriggerLifecycleAction::InactiveState3,
            0,
            1.0,
            &mut creates,
        );
        assert!(state.xitem_exists);
        assert_eq!(state.xitem_state, 3);
        state.advance_xitem(false);
        assert!((state.current_opacity - 0.9).abs() < f32::EPSILON);

        for _ in 0..80 {
            state.advance_xitem(false);
            if state.current_opacity == 0.0 {
                break;
            }
        }
        assert_eq!(state.current_opacity, 0.0);
        state.advance_trigger_manager(
            RuntimeCommonTriggerLifecycleAction::InactiveState3,
            0,
            1.0,
            &mut creates,
        );
        assert!(!state.xitem_exists);

        creates = ROBOTS_TRIGGER_CREATE_BUDGET_PER_UPDATE;
        state.advance_trigger_manager(
            RuntimeCommonTriggerLifecycleAction::DelayedActive { ticks: 60 },
            0,
            0.0,
            &mut creates,
        );
        assert!(!state.xitem_exists);
        assert_eq!(state.delay_counter, 1);
    }

    #[test]
    fn cutscene_runtime_consumes_pending_alternate_on_create_and_resolves_host_script() {
        let mut state = NativeCutsceneRuntimeState::default();
        state.prepare_npc_activation(
            Some(0x0400_1234),
            Some(Vec3::new(1.0, 2.0, 3.0)),
            Some(Quat::from_rotation_y(0.5)),
        );
        assert!(state.pending_activation);
        assert_eq!(state.pending_alternate_uid, Some(0x0400_1234));
        assert_eq!(state.active_alternate_uid, None);

        let mut creates = ROBOTS_TRIGGER_CREATE_BUDGET_PER_UPDATE;
        assert!(!state.advance_trigger_manager(
            RuntimeCommonTriggerLifecycleAction::ImmediateActive,
            0,
            0.0,
            &mut creates,
        ));
        assert!(!state.lifecycle.xitem_exists);
        assert!(state.pending_activation);
        assert_eq!(state.pending_alternate_uid, Some(0x0400_1234));

        creates = 0;
        assert!(state.advance_trigger_manager(
            RuntimeCommonTriggerLifecycleAction::ImmediateActive,
            0,
            0.0,
            &mut creates,
        ));
        state.bind_script_on_create(0x0100_00BB, Some(0x0100_00C9), Some(0x0400_028E));
        assert!(state.lifecycle.xitem_exists);
        assert!(!state.pending_activation);
        assert_eq!(state.pending_alternate_uid, None);
        assert_eq!(state.active_alternate_uid, Some(0x0400_1234));
        assert_eq!(
            state.active_script,
            Some(RobotsCutsceneScriptSelection {
                owner_file_uid: 0x0100_00C9,
                default_script_uid: Some(0x0400_028E),
                selected_script_uid: 0x0400_1234,
                alternate_applied: true,
            })
        );
        assert_eq!(state.position_override, Some(Vec3::new(1.0, 2.0, 3.0)));
        assert_eq!(state.activation_requests, 1);

        let mut render_store = RenderStore::new();
        for (hashcode, framerate) in [(0x0400_028E, 60.0), (0x0400_1234, 30.0)] {
            render_store.insert_script(
                0x0100_00C9,
                eurochef_shared::script::UXGeoScript {
                    hashcode,
                    framerate,
                    length: 120,
                    num_threads: 1,
                    commands: vec![eurochef_shared::script::UXGeoScriptCommand {
                        opcode: 11,
                        start: 1,
                        length: 1,
                        controller_header_index: 0,
                        controller_index: 0,
                        parent_controller_index: 0xff,
                        data: eurochef_shared::script::UXGeoScriptCommandData::Event {
                            event_type: 0x1600_0045,
                            data: vec![0; 8],
                        },
                    }],
                    serialized_controller_count: 0,
                    controller_record_metadata: vec![],
                    controllers: vec![],
                    controller_group_indices: vec![],
                    controller_groups: vec![],
                },
            );
        }
        assert_eq!(state.script_scheduler.frame, 0.0);
        state.advance_xitem(true);
        let selected = state.active_script.unwrap();
        let script = render_store
            .get_script(selected.owner_file_uid, selected.selected_script_uid)
            .unwrap()
            .clone();
        let step = state.advance_script(&script, |_| RobotsScriptNativeEventResult::Continue);
        assert_eq!(step.dispatched_events, 0);
        assert_eq!(state.script_scheduler.frame, 1.0);
        state.active_script_time_seconds = Some(script.time_at_frame(state.script_scheduler.frame));
        assert_eq!(state.active_script_time_seconds, Some(1.0 / 30.0));
    }

    #[test]
    fn cutscene_finalize_routes_are_trigger_lifetime_state_and_setproperties_toggles_only_changelevel(
    ) {
        let mut state = NativeCutsceneRuntimeState::default();
        state.ensure_finalize_routes_initialized(true, true, false);
        assert!(state.creator_routes_initialized);
        assert!(state.has_change_level_link);
        assert_eq!(state.creator_runtime_flags_e8, 0x06);

        state.apply_change_level_route_toggle(RobotsCutsceneToggle::Disabled);
        assert_eq!(state.creator_runtime_flags_e8, 0x04);

        // Trigger init is not an XItem create hook. Re-entering the lazy GUI
        // initialization path must therefore preserve the runtime toggle.
        state.ensure_finalize_routes_initialized(true, true, false);
        assert_eq!(state.creator_runtime_flags_e8, 0x04);

        let mut creates = 0;
        assert!(state.advance_trigger_manager(
            RuntimeCommonTriggerLifecycleAction::ImmediateActive,
            0,
            0.0,
            &mut creates,
        ));
        state.bind_script_on_create(0x0100_00BB, None, Some(0x0400_028E));
        assert_eq!(state.creator_runtime_flags_e8, 0x04);
        assert!(state.message_enabled_16be);
        assert!(!state.pending_swap_character_16bd);

        state.apply_change_level_route_toggle(RobotsCutsceneToggle::Enabled);
        assert_eq!(state.creator_runtime_flags_e8, 0x06);

        state.apply_message_toggle(RobotsCutsceneToggle::Enabled, 0);
        assert!(state.message_enabled_16be);
        state.apply_message_toggle(RobotsCutsceneToggle::Disabled, 0);
        assert!(!state.message_enabled_16be);
        state.apply_message_toggle(RobotsCutsceneToggle::Enabled, 1);
        assert!(!state.message_enabled_16be);

        state.apply_swap_character_mode(Some(0));
        assert!(!state.pending_swap_character_16bd);
        state.apply_swap_character_mode(Some(1));
        assert!(state.pending_swap_character_16bd);

        state.bind_script_audio_scan(RobotsCutsceneAudioScan {
            streamed_sfx_present_16c3: true,
            music_present_16c4: true,
        });
        assert!(state.script_audio_scan_initialized);
        assert!(state.audio_active_16c3);
        assert!(state.music_audio_present_16c4);
        state.record_finalize_effects(vec![
            RobotsCutsceneFinalizeEffect::ResetCreatorActivationOverride,
        ]);
        state.release_after_finalize();
        assert!(!state.lifecycle.xitem_exists);
        assert!(!state.audio_active_16c3);
        assert!(!state.music_audio_present_16c4);
        assert!(!state.script_audio_scan_initialized);
        assert_eq!(
            state.pending_finalize_effects,
            vec![RobotsCutsceneFinalizeEffect::ResetCreatorActivationOverride]
        );
    }

    #[test]
    fn cutscene_script_scheduler_emits_only_due_effects_and_holds_state_marker() {
        use eurochef_shared::robots_runtime::{
            cutscene::plan_cutscene_effect,
            events::{
                event_type, resolve_recovered_handler_script_command,
                RobotsHandlerEventReturnPolicy, RobotsHandlerScriptCommandFamily,
            },
        };

        let event = |start: i16, event_type: u32, args: &[u32]| {
            let mut data = 0u32.to_le_bytes().to_vec();
            for arg in args {
                data.extend_from_slice(&arg.to_le_bytes());
            }
            eurochef_shared::script::UXGeoScriptCommand {
                opcode: 11,
                start,
                length: 1,
                controller_header_index: 0,
                controller_index: 0,
                parent_controller_index: 0xff,
                data: eurochef_shared::script::UXGeoScriptCommandData::Event { event_type, data },
            }
        };
        let script = eurochef_shared::script::UXGeoScript {
            hashcode: 0x0400_028E,
            framerate: 30.0,
            length: 8,
            num_threads: 1,
            commands: vec![
                event(0, event_type::PERFORM_ACTION_CUTSCENE, &[4]),
                event(2, event_type::SHOW_MESSAGE, &[0x4500_034E, 0, 0, u32::MAX]),
                event(3, event_type::STATE_MARKER, &[u32::MAX]),
                eurochef_shared::script::UXGeoScriptCommand {
                    opcode: 18,
                    start: 0,
                    length: 0,
                    controller_header_index: 0,
                    controller_index: 0,
                    parent_controller_index: 0,
                    data: eurochef_shared::script::UXGeoScriptCommandData::Unknown {
                        cmd: 18,
                        data: vec![],
                    },
                },
            ],
            serialized_controller_count: 0,
            controller_record_metadata: vec![],
            controllers: vec![],
            controller_group_indices: vec![],
            controller_groups: vec![],
        };
        let mut state = NativeCutsceneRuntimeState::default();
        state.lifecycle.xitem_exists = true;

        let run_tick = |state: &mut NativeCutsceneRuntimeState| {
            let mut effects = Vec::new();
            let step = state.advance_script(&script, |event| {
                let plan = resolve_recovered_handler_script_command(
                    RobotsHandlerScriptCommandFamily::Cutscene,
                    event,
                )
                .unwrap();
                if let Some(effect) = plan_cutscene_effect(&plan.semantic) {
                    effects.push(effect);
                }
                match plan.return_policy {
                    RobotsHandlerEventReturnPolicy::Zero => RobotsScriptNativeEventResult::Continue,
                    RobotsHandlerEventReturnPolicy::One
                    | RobotsHandlerEventReturnPolicy::StateDependent
                    | RobotsHandlerEventReturnPolicy::HostResult
                    | RobotsHandlerEventReturnPolicy::Delegate => {
                        RobotsScriptNativeEventResult::Hold
                    }
                }
            });
            (step, effects)
        };

        let (first, effects) = run_tick(&mut state);
        assert_eq!(first.dispatched_events, 1);
        assert_eq!(effects.len(), 1);
        assert!(matches!(
            effects[0],
            RobotsCutsceneEffect::PerformActionCutscene { .. }
        ));
        assert_eq!(state.script_scheduler.frame, 1.0);

        let (second, effects) = run_tick(&mut state);
        assert_eq!(second.dispatched_events, 0);
        assert!(effects.is_empty());
        assert_eq!(state.script_scheduler.frame, 2.0);

        let (third, effects) = run_tick(&mut state);
        assert_eq!(third.dispatched_events, 1);
        assert!(matches!(
            effects[0],
            RobotsCutsceneEffect::ShowMessage { .. }
        ));
        assert_eq!(state.script_scheduler.frame, 3.0);

        let (fourth, effects) = run_tick(&mut state);
        assert!(fourth.held_on_event);
        assert_eq!(state.script_scheduler.frame, 3.0);
        assert!(effects.is_empty());
        assert_eq!(state.state_marker_runtime.handler_state_16b0, 1);
        let held_cursor = state.script_scheduler.cursor;

        let fade_request = state.advance_cutscene_handler(RobotsCutsceneHandlerHostInput {
            trigger_flags: 0,
            fade_available: true,
            fade_low_reached: false,
            fade_high_reached: true,
            creator_script_present: true,
        });
        assert_eq!(
            fade_request.effect,
            Some(RobotsCutsceneHandlerHostEffect::RequestFadeOut {
                duration_updates: 30,
            })
        );
        let (fifth, effects) = run_tick(&mut state);
        assert!(fifth.held_on_event);
        assert!(effects.is_empty());
        assert_eq!(state.script_scheduler.cursor, held_cursor);

        let blocked = state.advance_cutscene_handler(RobotsCutsceneHandlerHostInput {
            trigger_flags: 0,
            fade_available: true,
            fade_low_reached: false,
            fade_high_reached: false,
            creator_script_present: true,
        });
        assert!(blocked.blocked_on_fade);
        let finalized = state.advance_cutscene_handler(RobotsCutsceneHandlerHostInput {
            trigger_flags: 0,
            fade_available: true,
            fade_low_reached: true,
            fade_high_reached: false,
            creator_script_present: true,
        });
        assert_eq!(
            finalized.effect,
            Some(RobotsCutsceneHandlerHostEffect::FinalizeCutscene)
        );
        assert_eq!(state.state_marker_runtime.handler_state_16b0, 3);
        assert!(!state.state_marker_runtime.teardown_complete_16ba);
        state.complete_ordinary_finalize();
        let (released, effects) = run_tick(&mut state);
        assert!(released.terminated);
        assert!(effects.is_empty());
        assert!(state.state_marker_runtime.script_complete_16b8);
    }

    #[test]
    fn cutscene_set_alternate_state_is_internal_timeline_seek_not_a_host_effect() {
        use eurochef_shared::robots_runtime::{
            cutscene::plan_cutscene_effect,
            events::{
                event_type, resolve_recovered_handler_script_command,
                RobotsHandlerScriptCommandFamily,
            },
        };

        let event = |start: i16, event_type: u32, args: &[u32]| {
            let mut data = 0u32.to_le_bytes().to_vec();
            for arg in args {
                data.extend_from_slice(&arg.to_le_bytes());
            }
            eurochef_shared::script::UXGeoScriptCommand {
                opcode: 11,
                start,
                length: 1,
                controller_header_index: 0,
                controller_index: 0,
                parent_controller_index: 0xff,
                data: eurochef_shared::script::UXGeoScriptCommandData::Event { event_type, data },
            }
        };
        let script = eurochef_shared::script::UXGeoScript {
            hashcode: 0x0400_028E,
            framerate: 30.0,
            length: 8,
            num_threads: 1,
            commands: vec![
                event(0, event_type::SET_ALTERNATE_STATE, &[]),
                event(0, event_type::PERFORM_ACTION_CUTSCENE, &[3]),
                event(4, event_type::SHOW_MESSAGE, &[0x4500_034E, 0, 0, u32::MAX]),
            ],
            serialized_controller_count: 0,
            controller_record_metadata: vec![],
            controllers: vec![],
            controller_group_indices: vec![],
            controller_groups: vec![],
        };

        let run = |state: &mut NativeCutsceneRuntimeState| {
            let mut effects = Vec::new();
            let step = state.advance_script(&script, |event| {
                let plan = resolve_recovered_handler_script_command(
                    RobotsHandlerScriptCommandFamily::Cutscene,
                    event,
                )
                .unwrap();
                if let Some(effect) = plan_cutscene_effect(&plan.semantic) {
                    effects.push(effect);
                }
                RobotsScriptNativeEventResult::Continue
            });
            (step, effects)
        };

        let mut fresh = NativeCutsceneRuntimeState::default();
        fresh.lifecycle.xitem_exists = true;
        let (fresh_step, fresh_effects) = run(&mut fresh);
        assert_eq!(fresh_step.dispatched_events, 2);
        assert_eq!(fresh.script_scheduler.frame, 1.0);
        assert_eq!(fresh_effects.len(), 1);
        assert!(matches!(
            fresh_effects[0],
            RobotsCutsceneEffect::PerformActionCutscene {
                raw_mode: Some(3),
                ..
            }
        ));

        let mut resumed = NativeCutsceneRuntimeState::default();
        resumed.lifecycle.xitem_exists = true;
        resumed.creator_saved_time_bits_40 = (3.0f32 / 30.0).to_bits();
        let (resumed_step, resumed_effects) = run(&mut resumed);
        assert_eq!(resumed_step.dispatched_events, 1);
        assert_eq!(resumed.script_scheduler.frame, 4.0);
        assert_eq!(resumed.script_scheduler.cursor, 2);
        assert!(resumed_effects.is_empty());
    }

    #[test]
    fn native_ai_hit_fatal_waits_for_monster_explosion_then_requests_destroy_on_sixth_fade_tick() {
        assert!(RuntimeNativeAiHitFatalState::normal_fatal_hit_condition(
            true, 0
        ));
        assert!(!RuntimeNativeAiHitFatalState::normal_fatal_hit_condition(
            false, 0
        ));
        assert!(!RuntimeNativeAiHitFatalState::normal_fatal_hit_condition(
            true, 1
        ));

        let mut death = RuntimeNativeAiHitFatalState::default();
        let before_explosion = death.advance_fixed_tick_plan(false);
        assert_eq!(before_explosion.opacity_write, None);
        assert!(!before_explosion.request_destroy);

        assert!(!death.apply_monster_explosion_event(false));
        assert!(!death.monster_explosion_completed);
        assert!(death.apply_monster_explosion_event(true));
        assert!(!death.apply_monster_explosion_event(true));

        let mut destroy_tick = None;
        for tick in 1..=6 {
            let step = death.advance_fixed_tick_plan(false);
            if tick == 1 {
                assert_eq!(step.opacity_write, None);
            } else {
                assert!(step.opacity_write.is_some());
            }
            if step.request_destroy {
                destroy_tick = Some(tick);
            }
            if tick < 6 {
                assert!(!step.request_destroy);
            }
        }
        assert_eq!(destroy_tick, Some(6));
        assert!(death.fade_remaining < ROBOTS_AI_HIT_FATAL_DESTROY_EPSILON);

        let mut below_floor = RuntimeNativeAiHitFatalState::default();
        let immediate = below_floor.advance_fixed_tick_plan(true);
        assert!(immediate.request_destroy);
        assert_eq!(immediate.opacity_write, None);
    }

    #[test]
    fn camera_bit0_trigger_ticks_script_before_camera_and_resets_on_destroy() {
        let mut state = RuntimeCameraBit0TriggerState::default();
        let mut creates = 0;
        state.advance_trigger_manager(
            RuntimeCommonTriggerLifecycleAction::ImmediateActive,
            0,
            0.0,
            &mut creates,
        );
        assert!(state.lifecycle.xitem_exists);
        assert_eq!(state.script_frame, 0.0);
        state.advance_xitem_before_camera(true);
        assert_eq!(state.script_frame, 1.0);
        state.advance_xitem_before_camera(true);
        assert_eq!(state.script_frame, 2.0);
        state.cleanup_zone_failed();
        assert!(!state.lifecycle.xitem_exists);
        assert_eq!(state.script_frame, 0.0);
    }

    #[test]
    fn script_trigger_creation_uses_native_proximity_factor_and_opacity_curve() {
        assert_eq!(runtime_trigger_normal_proximity_factor(400.0), 0.0);
        assert!((runtime_trigger_normal_proximity_factor(650.0) - 0.5).abs() < f32::EPSILON);
        assert_eq!(runtime_trigger_normal_proximity_factor(900.0), 1.0);

        let mut near = RuntimeScriptTriggerLifecycleState::default();
        let mut creates = 0;
        near.advance_trigger_manager(
            RuntimeCommonTriggerLifecycleAction::ImmediateActive,
            0,
            0.5,
            &mut creates,
        );
        assert_eq!(near.current_opacity, 1.0);

        let mut far = RuntimeScriptTriggerLifecycleState::default();
        far.advance_trigger_manager(
            RuntimeCommonTriggerLifecycleAction::DelayedActive { ticks: 60 },
            0,
            0.8,
            &mut creates,
        );
        assert_eq!(far.current_opacity, 0.0);
        far.advance_xitem(true);
        assert!((far.current_opacity - 0.005).abs() < 1.0e-6);
    }

    #[test]
    fn common_trigger_event_routing_matches_native_zone_and_bit1_rules() {
        assert_eq!(
            runtime_common_trigger_lifecycle_action(RuntimeTriggerDistanceEvent::Event0, true, 0),
            RuntimeCommonTriggerLifecycleAction::ImmediateActive
        );
        assert_eq!(
            runtime_common_trigger_lifecycle_action(RuntimeTriggerDistanceEvent::Event0, false, 0),
            RuntimeCommonTriggerLifecycleAction::InactiveState2
        );
        assert_eq!(
            runtime_common_trigger_lifecycle_action(RuntimeTriggerDistanceEvent::Event0, false, 2),
            RuntimeCommonTriggerLifecycleAction::DelayedActive { ticks: 10 }
        );
        assert_eq!(
            runtime_common_trigger_lifecycle_action(RuntimeTriggerDistanceEvent::Event1, true, 0),
            RuntimeCommonTriggerLifecycleAction::DelayedActive { ticks: 30 }
        );
        assert_eq!(
            runtime_common_trigger_lifecycle_action(RuntimeTriggerDistanceEvent::Event2, true, 0),
            RuntimeCommonTriggerLifecycleAction::DelayedActive { ticks: 60 }
        );
        assert_eq!(
            runtime_common_trigger_lifecycle_action(RuntimeTriggerDistanceEvent::Event3, true, 0),
            RuntimeCommonTriggerLifecycleAction::InactiveState3
        );
        assert_eq!(
            runtime_common_trigger_lifecycle_action(RuntimeTriggerDistanceEvent::Event3, false, 2),
            RuntimeCommonTriggerLifecycleAction::DelayedActive { ticks: 60 }
        );
    }

    #[test]
    fn minebot_move_gate_uses_native_12_16_hysteresis() {
        let minebot = Vec3::ZERO;
        assert!(runtime_minebot_move_gate(
            minebot,
            Vec3::new(11.999, 0.0, 0.0),
            false
        ));
        assert!(!runtime_minebot_move_gate(
            minebot,
            Vec3::new(12.0, 0.0, 0.0),
            false
        ));
        assert!(runtime_minebot_move_gate(
            minebot,
            Vec3::new(15.999, 0.0, 0.0),
            true
        ));
        assert!(!runtime_minebot_move_gate(
            minebot,
            Vec3::new(16.0, 0.0, 0.0),
            true
        ));
    }

    #[test]
    fn minebot_attack_gate_uses_native_5_19_ring_visibility_and_vertical_limit() {
        let minebot = Vec3::ZERO;
        assert!(!runtime_minebot_attack_gate(
            minebot,
            Vec3::new(6.0, 0.0, 0.0),
            false
        ));
        assert!(!runtime_minebot_attack_gate(
            minebot,
            Vec3::new(4.999, 0.0, 0.0),
            true
        ));
        assert!(runtime_minebot_attack_gate(
            minebot,
            Vec3::new(5.0, 0.0, 0.0),
            true
        ));
        assert!(runtime_minebot_attack_gate(
            minebot,
            Vec3::new(19.0, 0.0, 0.0),
            true
        ));
        assert!(!runtime_minebot_attack_gate(
            minebot,
            Vec3::new(19.001, 0.0, 0.0),
            true
        ));
        assert!(!runtime_minebot_attack_gate(
            minebot,
            Vec3::new(5.0, 1000.001, 0.0),
            true
        ));
    }

    #[test]
    fn minebot_attack_priority_preempts_move_locally() {
        assert_eq!(
            runtime_minebot_move_attack_winner(true, true),
            RuntimeMineBotMoveAttackWinner::Attack
        );
        assert_eq!(
            runtime_minebot_move_attack_winner(true, false),
            RuntimeMineBotMoveAttackWinner::Move
        );
        assert_eq!(
            runtime_minebot_move_attack_winner(false, false),
            RuntimeMineBotMoveAttackWinner::Neither
        );
    }

    #[test]
    fn ai_environment_floor_query_keeps_native_bounds_and_face_bit0_filter() {
        assert_eq!(ROBOTS_AI_ENV_QUERY_Y_BIAS.to_bits(), 1.0f32.to_bits());
        assert_eq!(ROBOTS_AI_ENV_QUERY_HALF_HEIGHT.to_bits(), 50.0f32.to_bits());

        let floor = |y: f32, face_mask: u16| RobotsRaycastTriangle {
            positions: [
                Vec3::new(-2.0, y, -2.0),
                Vec3::new(0.0, y, 2.0),
                Vec3::new(2.0, y, -2.0),
            ],
            face_mask,
            trailing_raw: 0,
        };
        let start = Vec3::new(0.0, 11.0, 0.0);
        let delta = Vec3::new(0.0, -51.0, 0.0);
        let blocked_by_native_bit0 = floor(5.0, 1);
        let accepted_floor = floor(0.0, 0);

        let hit = runtime_robots_ai_environment_floor_nearest_hit(
            &[blocked_by_native_bit0, accepted_floor],
            start,
            delta,
            1.0,
        )
        .expect("bit0-clear floor should be selected");
        assert!((hit.t - 11.0 / 51.0).abs() < 1.0e-6);
        assert!((start.y + delta.y * hit.t).abs() < 1.0e-6);
        assert!(runtime_robots_ai_environment_floor_nearest_hit(
            &[blocked_by_native_bit0],
            start,
            delta,
            1.0,
        )
        .is_none());
    }

    #[test]
    fn character_floor_projection_uses_world_shape_support_and_two_sided_plane() {
        let contact = RuntimeAiEnvironmentFloorContact {
            point: Vec3::ZERO,
            normal: -Vec3::Y,
        };
        let sphere = RuntimeCharacterWorldShape::Sphere {
            center_xyz: [0.0, 0.25, 0.0],
            radius: 0.5,
        };
        let correction = runtime_character_floor_contact_projection(sphere, contact)
            .expect("penetrating sphere should be projected above the floor");
        assert!((correction - Vec3::new(0.0, 0.25, 0.0)).length() < 1.0e-6);

        let capsule = RuntimeCharacterWorldShape::Capsule {
            start_xyz: [0.0, 0.2, 0.0],
            delta_xyz: [0.0, 1.0, 0.0],
            radius: 0.3,
        };
        let correction = runtime_character_floor_contact_projection(capsule, contact)
            .expect("capsule support point should be projected above the floor");
        assert!((correction - Vec3::new(0.0, 0.1, 0.0)).length() < 1.0e-6);
    }

    #[test]
    fn robots_mesh_raycast_matches_native_two_sided_segment_and_masks() {
        let triangle = RobotsRaycastTriangle {
            positions: [
                Vec3::new(-1.0, -1.0, 0.0),
                Vec3::new(1.0, -1.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
            ],
            face_mask: 0,
            trailing_raw: 0,
        };
        assert_eq!(
            runtime_robots_mesh_raycast_nearest_t(
                &[triangle],
                Vec3::new(0.0, 0.0, -1.0),
                Vec3::new(0.0, 0.0, 2.0),
                0,
                0,
                1.0,
            ),
            Some(0.5)
        );
        assert_eq!(
            runtime_robots_mesh_raycast_nearest_t(
                &[triangle],
                Vec3::new(0.0, 0.0, 1.0),
                Vec3::new(0.0, 0.0, -2.0),
                0,
                0,
                1.0,
            ),
            Some(0.5)
        );
        assert_eq!(
            runtime_robots_mesh_raycast_nearest_t(
                &[triangle],
                Vec3::new(0.0, 0.0, -1.0),
                Vec3::new(0.0, 0.0, 1.0),
                0,
                0,
                1.0,
            ),
            None,
            "native t < max_t excludes a face exactly at the segment endpoint"
        );

        let masked = RobotsRaycastTriangle {
            face_mask: 0x40,
            ..triangle
        };
        assert!(runtime_robots_mesh_raycast_nearest_t(
            &[masked],
            Vec3::new(0.0, 0.0, -1.0),
            Vec3::new(0.0, 0.0, 2.0),
            0x40,
            0,
            1.0,
        )
        .is_some());
        assert!(runtime_robots_mesh_raycast_nearest_t(
            &[masked],
            Vec3::new(0.0, 0.0, -1.0),
            Vec3::new(0.0, 0.0, 2.0),
            0x20,
            0,
            1.0,
        )
        .is_none());
        assert!(runtime_robots_mesh_raycast_nearest_t(
            &[masked],
            Vec3::new(0.0, 0.0, -1.0),
            Vec3::new(0.0, 0.0, 2.0),
            0,
            0x40,
            1.0,
        )
        .is_none());
    }

    #[test]
    fn projectile_variant0_sphere_contact_matches_native_distance_and_material_gates() {
        let triangle = RobotsRaycastTriangle {
            positions: [
                Vec3::new(-1.0, 0.0, -1.0),
                Vec3::new(1.0, 0.0, -1.0),
                Vec3::new(0.0, 0.0, 1.0),
            ],
            face_mask: 0,
            trailing_raw: 0,
        };
        assert!(runtime_projectile_sphere_hits_triangles(
            &[triangle],
            Vec3::new(0.0, 0.2, 0.0),
            0.2,
        ));
        let ordinary_contact = runtime_projectile_sphere_contact_from_triangles(
            &[triangle],
            Vec3::new(0.0, 0.1, 0.0),
            0.2,
        )
        .expect("ordinary static contact");
        assert_eq!(ordinary_contact.contact_bits, 0x0001);
        assert!(ordinary_contact
            .normal
            .is_some_and(|normal| normal.dot(Vec3::Y).abs() > 0.999));
        assert!(!runtime_projectile_sphere_hits_triangles(
            &[triangle],
            Vec3::new(0.0, 0.2001, 0.0),
            0.2,
        ));
        assert!(runtime_projectile_sphere_hits_triangles(
            &[triangle],
            Vec3::new(1.09, 0.0, -1.0),
            0.1,
        ));

        let rejected_category = RobotsRaycastTriangle {
            face_mask: 0x30,
            ..triangle
        };
        assert!(!runtime_projectile_sphere_hits_triangles(
            &[rejected_category],
            Vec3::ZERO,
            0.2,
        ));
        let immediate_terminal = RobotsRaycastTriangle {
            face_mask: 0x34,
            ..triangle
        };
        assert!(runtime_projectile_sphere_hits_triangles(
            &[immediate_terminal],
            Vec3::ZERO,
            0.2,
        ));
        let immediate_contact = runtime_projectile_sphere_contact_from_triangles(
            &[immediate_terminal],
            Vec3::ZERO,
            0.2,
        )
        .expect("immediate bit-0x04 contact");
        assert_eq!(immediate_contact.contact_bits, 0x0006);
        assert_eq!(immediate_contact.normal, None);
        let high_bit_rejected = RobotsRaycastTriangle {
            face_mask: 0x80,
            ..triangle
        };
        assert!(!runtime_projectile_sphere_hits_triangles(
            &[high_bit_rejected],
            Vec3::ZERO,
            0.2,
        ));

        let degenerate = RobotsRaycastTriangle {
            positions: [Vec3::ZERO, Vec3::X, Vec3::X * 2.0],
            face_mask: 0,
            trailing_raw: 0,
        };
        assert!(
            (runtime_point_triangle_distance_squared(Vec3::new(0.5, 1.0, 0.0), &degenerate) - 1.0)
                .abs()
                < 1.0e-6
        );
    }

    #[test]
    fn camera_contact_filter_matches_native_surface_metadata_gate() {
        assert!(runtime_robots_camera_contact_face_accepted(0x0000));
        assert!(runtime_robots_camera_contact_face_accepted(0x0040));
        assert!(!runtime_robots_camera_contact_face_accepted(0x0048));
        assert!(!runtime_robots_camera_contact_face_accepted(0x0020));
        assert!(!runtime_robots_camera_contact_face_accepted(0x4000));
        assert!(!runtime_robots_camera_contact_face_accepted(0x4040));

        let ignored = RobotsRaycastTriangle {
            positions: [
                Vec3::new(-1.0, -1.0, -0.5),
                Vec3::new(1.0, -1.0, -0.5),
                Vec3::new(0.0, 1.0, -0.5),
            ],
            face_mask: 0x48,
            trailing_raw: 0,
        };
        let accepted = RobotsRaycastTriangle {
            positions: [
                Vec3::new(-1.0, -1.0, 0.5),
                Vec3::new(1.0, -1.0, 0.5),
                Vec3::new(0.0, 1.0, 0.5),
            ],
            face_mask: 0,
            trailing_raw: 0,
        };
        assert_eq!(
            runtime_robots_camera_contact_nearest_t(
                &[ignored, accepted],
                Vec3::new(0.0, 0.0, -1.0),
                Vec3::new(0.0, 0.0, 2.0),
                1.0,
            ),
            None,
            "native Camera filters the nearest raw hit after Map raycast; it does not skip through an ignored face to a farther accepted face"
        );
        assert_eq!(
            runtime_robots_camera_contact_nearest_t(
                &[accepted],
                Vec3::new(0.0, 0.0, -1.0),
                Vec3::new(0.0, 0.0, 2.0),
                1.0,
            ),
            Some(0.75)
        );
    }

    #[test]
    fn queued_entity_los_uses_script_instance_world_transform() {
        let triangle = RobotsRaycastTriangle {
            positions: [
                Vec3::new(-1.0, -1.0, 0.0),
                Vec3::new(1.0, -1.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
            ],
            face_mask: 0,
            trailing_raw: 0,
        };
        let mut renderer = crate::render::entity::EntityRenderer::new(
            0x0100_0001,
            eurochef_edb::versions::Platform::Pc,
        );
        renderer.set_robots_raycast_triangles_for_test(vec![triangle]);
        let mut store = RenderStore::new();
        store.insert_entity(0x0100_0001, 0x0200_0001, 0, renderer);

        let blocking = QueuedEntityRender {
            entity: (0x0100_0001, 0x0200_0001),
            entity_alt: None,
            position: Vec3::new(0.0, 0.0, 5.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        };
        assert_eq!(
            runtime_queued_entities_line_of_sight_clear(
                &[blocking.clone()],
                &store,
                Vec3::ZERO,
                Vec3::new(0.0, 0.0, 10.0),
            ),
            Some(false)
        );

        let clear = QueuedEntityRender {
            position: Vec3::new(5.0, 0.0, 5.0),
            ..blocking
        };
        assert_eq!(
            runtime_queued_entities_line_of_sight_clear(
                &[clear],
                &store,
                Vec3::ZERO,
                Vec3::new(0.0, 0.0, 10.0),
            ),
            Some(true)
        );
    }

    #[test]
    fn camera_dynamic_xitem_filters_only_the_global_nearest_raw_face() {
        let triangle = |face_mask| RobotsRaycastTriangle {
            positions: [
                Vec3::new(-1.0, -1.0, 0.0),
                Vec3::new(1.0, -1.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
            ],
            face_mask,
            trailing_raw: 0,
        };
        let mut ignored_renderer = crate::render::entity::EntityRenderer::new(
            0x0100_0001,
            eurochef_edb::versions::Platform::Pc,
        );
        ignored_renderer.set_robots_raycast_triangles_for_test(vec![triangle(0x48)]);
        let mut accepted_renderer = crate::render::entity::EntityRenderer::new(
            0x0100_0001,
            eurochef_edb::versions::Platform::Pc,
        );
        accepted_renderer.set_robots_raycast_triangles_for_test(vec![triangle(0)]);
        let mut store = RenderStore::new();
        store.insert_entity(0x0100_0001, 0x0200_0001, 0, ignored_renderer);
        store.insert_entity(0x0100_0001, 0x0200_0002, 1, accepted_renderer);

        let far = QueuedEntityRender {
            entity: (0x0100_0001, 0x0200_0002),
            entity_alt: None,
            position: Vec3::new(0.0, 0.0, 7.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        };
        let near_ignored = QueuedEntityRender {
            entity: (0x0100_0001, 0x0200_0001),
            entity_alt: None,
            position: Vec3::new(0.0, 0.0, 3.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        };
        assert_eq!(
            runtime_queued_entities_camera_contact_nearest_t(
                &[far.clone(), near_ignored],
                &store,
                Vec3::ZERO,
                Vec3::new(0.0, 0.0, 10.0),
            ),
            Some(None),
            "native dynamic pass rejects the nearest raw winner instead of seeing through it"
        );
        assert_eq!(
            runtime_queued_entities_camera_contact_nearest_t(
                &[far],
                &store,
                Vec3::ZERO,
                Vec3::new(0.0, 0.0, 10.0),
            ),
            Some(Some(0.7))
        );
    }

    #[test]
    fn sphere_impulse_swaps_equal_mass_velocities_at_full_restitution() {
        let (self_after, peer_after) = runtime_sphere_sphere_impulse_velocity(
            Vec3::new(-1.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::X,
            1.0,
            1.0,
            1.0,
            1.0,
        )
        .expect("valid native sphere parameters");
        assert!(self_after.distance(Vec3::new(1.0, 0.0, 0.0)) < 1.0e-6);
        assert!(peer_after.distance(Vec3::new(-1.0, 0.0, 0.0)) < 1.0e-6);
    }

    #[test]
    fn sphere_impulse_stops_equal_mass_head_on_motion_at_zero_restitution() {
        let (self_after, peer_after) = runtime_sphere_sphere_impulse_velocity(
            Vec3::new(-1.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::X,
            1.0,
            1.0,
            0.0,
            0.0,
        )
        .expect("valid native sphere parameters");
        assert!(self_after.length() < 1.0e-6);
        assert!(peer_after.length() < 1.0e-6);
    }

    #[test]
    fn sphere_impulse_does_not_react_when_bodies_are_separating() {
        let self_velocity = Vec3::new(1.0, 0.0, 0.0);
        let peer_velocity = Vec3::new(-1.0, 0.0, 0.0);
        let (self_after, peer_after) = runtime_sphere_sphere_impulse_velocity(
            self_velocity,
            peer_velocity,
            Vec3::X,
            2.0,
            3.0,
            0.4,
            0.2,
        )
        .expect("valid native sphere parameters");
        assert_eq!(self_after, self_velocity);
        assert_eq!(peer_after, peer_velocity);
    }

    #[test]
    fn character_move_transition_uses_native_five_tick_direct_alpha_ramp() {
        let make_track = |animation, position| {
            Arc::new(ProcessedCharacterAnimationTrack {
                animation,
                animskin: 0x0D00_0001,
                clip_rate: 60,
                transition_fixed_ticks: 5,
                frame_count: 1,
                root_motion_samples: vec![ProcessedCharacterRootMotionSample {
                    position: Vec3::ZERO,
                    rotation: Quat::IDENTITY,
                }],
                bone_chain: vec![0],
                poses: vec![ProcessedCharacterAnimationBonePose {
                    position,
                    rotation: Quat::IDENTITY,
                }],
            })
        };
        let body = RuntimeCharacterBodyState {
            registration_mask: ROBOTS_GAMEPLAY_COLLISION_REGISTRATION_MASK,
            raw_hit_query_group: 1,
            last_hit_query_serial: u16::MAX,
            owner_position: Vec3::ZERO,
            owner_rotation: Quat::IDENTITY,
            native_transform_scale: [1.0; 4],
            collision: ProcessedCharacterCollisionProfile {
                animskin: 0x0D00_0001,
                shape: ProcessedCharacterCollisionShape::Sphere { radius: 0.5 },
                local_center: Vec3::ZERO,
                local_orientation: Quat::IDENTITY.to_array(),
                transform_selector: 0,
            },
            hit_area: None,
            initial_animation: Some(make_track(0x0300_000E, Vec3::ZERO)),
            animation_modes: BTreeMap::new(),
            animation_mode_scripts: BTreeMap::new(),
            move_animation: Some(make_track(0x8300_0016, Vec3::new(10.0, 0.0, 0.0))),
            turn_on_spot_l_animation: None,
            turn_on_spot_r_animation: None,
            initial_animation_seconds: 0.0,
            active_collision_local_poses: None,
            active_collision_bone_chain: None,
            collision_transition_snapshot: None,
        };

        assert!((ROBOTS_CHARACTER_MOVE_BLEND_SECONDS - 5.0 / 60.0).abs() < f32::EPSILON);
        let start = body.move_transition_bone_matrix(0.0, 0.0, 0.0);
        let half =
            body.move_transition_bone_matrix(0.0, 0.0, ROBOTS_CHARACTER_MOVE_BLEND_SECONDS * 0.5);
        let complete =
            body.move_transition_bone_matrix(0.0, 0.0, ROBOTS_CHARACTER_MOVE_BLEND_SECONDS);
        let after_cleanup =
            body.move_transition_bone_matrix(0.0, 0.0, ROBOTS_CHARACTER_MOVE_BLEND_SECONDS * 2.0);
        assert!(start.transform_point3(Vec3::ZERO).distance(Vec3::ZERO) < 1.0e-6);
        assert!(
            half.transform_point3(Vec3::ZERO)
                .distance(Vec3::new(5.0, 0.0, 0.0))
                < 1.0e-6
        );
        assert!(
            complete
                .transform_point3(Vec3::ZERO)
                .distance(Vec3::new(10.0, 0.0, 0.0))
                < 1.0e-6
        );
        assert!(
            after_cleanup
                .transform_point3(Vec3::ZERO)
                .distance(Vec3::new(10.0, 0.0, 0.0))
                < 1.0e-6
        );
    }

    #[test]
    fn character_collision_pose_tracks_gameplay_animation_and_snapshots_mode_switch() {
        let make_track = |animation, x| {
            Arc::new(ProcessedCharacterAnimationTrack {
                animation,
                animskin: 0x0D00_0001,
                clip_rate: 60,
                transition_fixed_ticks: 5,
                frame_count: 1,
                root_motion_samples: vec![ProcessedCharacterRootMotionSample {
                    position: Vec3::ZERO,
                    rotation: Quat::IDENTITY,
                }],
                bone_chain: vec![0],
                poses: vec![ProcessedCharacterAnimationBonePose {
                    position: Vec3::new(x, 0.0, 0.0),
                    rotation: Quat::IDENTITY,
                }],
            })
        };
        let initial = make_track(0x0300_000E, 0.0);
        let turn_l = make_track(0x8300_0006, 10.0);
        let turn_r = make_track(0x8300_0007, -10.0);
        let mut body = RuntimeCharacterBodyState {
            registration_mask: ROBOTS_GAMEPLAY_COLLISION_REGISTRATION_MASK,
            raw_hit_query_group: 1,
            last_hit_query_serial: u16::MAX,
            owner_position: Vec3::ZERO,
            owner_rotation: Quat::IDENTITY,
            native_transform_scale: [1.0; 4],
            collision: ProcessedCharacterCollisionProfile {
                animskin: 0x0D00_0001,
                shape: ProcessedCharacterCollisionShape::Sphere { radius: 0.5 },
                local_center: Vec3::ZERO,
                local_orientation: Quat::IDENTITY.to_array(),
                transform_selector: 0,
            },
            hit_area: None,
            initial_animation: Some(initial),
            animation_modes: BTreeMap::new(),
            animation_mode_scripts: BTreeMap::new(),
            move_animation: None,
            turn_on_spot_l_animation: Some(turn_l.clone()),
            turn_on_spot_r_animation: Some(turn_r.clone()),
            initial_animation_seconds: 0.0,
            active_collision_local_poses: None,
            active_collision_bone_chain: None,
            collision_transition_snapshot: None,
        };

        assert!(body.update_collision_animation_track(&turn_l, 0.0, 0.0, true));
        assert!(body
            .active_collision_local_poses
            .as_ref()
            .is_some_and(|poses| poses[0].position.x.abs() < 1.0e-6));
        assert!(body.update_collision_animation_track(
            &turn_l,
            0.0,
            2.5 / ROBOTS_CHARACTER_ANIMATION_FIXED_HZ,
            false,
        ));
        assert!(body
            .active_collision_local_poses
            .as_ref()
            .is_some_and(|poses| (poses[0].position.x - 5.0).abs() < 1.0e-6));

        // A direction change snapshots the current blended local pose. At alpha 0
        // the new right-turn node therefore starts from x=5, not from Idle x=0.
        assert!(body.update_collision_animation_track(&turn_r, 0.0, 0.0, true));
        assert!(body
            .active_collision_local_poses
            .as_ref()
            .is_some_and(|poses| (poses[0].position.x - 5.0).abs() < 1.0e-6));
        assert!(body.update_collision_animation_track(
            &turn_r,
            0.0,
            2.5 / ROBOTS_CHARACTER_ANIMATION_FIXED_HZ,
            false,
        ));
        let RuntimeCharacterWorldShape::Sphere { center_xyz, .. } = body.current_world_shape()
        else {
            panic!("synthetic collision profile must stay spherical");
        };
        assert!((Vec3::from_array(center_xyz).x + 2.5).abs() < 1.0e-6);
        assert!(runtime_live_character_collision_shape(Some(&body), false).is_none());
        let Some(RuntimeCharacterWorldShape::Sphere { center_xyz, .. }) =
            runtime_live_character_collision_shape(Some(&body), true)
        else {
            panic!("live registered body must expose its animated sphere");
        };
        assert!((Vec3::from_array(center_xyz).x + 2.5).abs() < 1.0e-6);
        body.registration_mask = 0;
        assert!(runtime_live_character_collision_shape(Some(&body), true).is_none());
        body.registration_mask = ROBOTS_GAMEPLAY_COLLISION_REGISTRATION_MASK;

        body.clear_collision_animation_track();
        let RuntimeCharacterWorldShape::Sphere { center_xyz, .. } = body.current_world_shape()
        else {
            panic!("synthetic collision profile must stay spherical");
        };
        assert!(Vec3::from_array(center_xyz).length() < 1.0e-6);
    }

    #[test]
    fn character_hit_candidate_commits_serial_only_after_confirmed_ordinary_hit() {
        let mut body = RuntimeCharacterBodyState {
            registration_mask: ROBOTS_GAMEPLAY_COLLISION_REGISTRATION_MASK,
            raw_hit_query_group: 1,
            last_hit_query_serial: u16::MAX,
            owner_position: Vec3::ZERO,
            owner_rotation: Quat::IDENTITY,
            native_transform_scale: [1.0; 4],
            collision: ProcessedCharacterCollisionProfile {
                animskin: 0x0D00_0001,
                shape: ProcessedCharacterCollisionShape::Sphere { radius: 0.1 },
                local_center: Vec3::new(100.0, 0.0, 0.0),
                local_orientation: Quat::IDENTITY.to_array(),
                transform_selector: 0,
            },
            hit_area: Some(ProcessedCharacterCollisionProfile {
                animskin: 0x0D00_0001,
                shape: ProcessedCharacterCollisionShape::Sphere { radius: 1.0 },
                local_center: Vec3::ZERO,
                local_orientation: Quat::IDENTITY.to_array(),
                transform_selector: 0,
            }),
            initial_animation: None,
            animation_modes: BTreeMap::new(),
            animation_mode_scripts: BTreeMap::new(),
            move_animation: None,
            turn_on_spot_l_animation: None,
            turn_on_spot_r_animation: None,
            initial_animation_seconds: 0.0,
            active_collision_local_poses: None,
            active_collision_bone_chain: None,
            collision_transition_snapshot: None,
        };
        let touching_query = RuntimeCharacterWorldShape::Sphere {
            center_xyz: [1.5, 0.0, 0.0],
            radius: 0.5,
        };
        let context = RobotsHitQueryCandidateContext {
            flags: 0,
            query_serial: 0x1234,
            source_raw_group: None,
            secondary_source_raw_group: None,
        };

        assert_eq!(
            runtime_test_live_character_hit_candidate(
                Some(&mut body),
                false,
                context,
                RuntimeCharacterHitQueryGeometry::PreparedShape(touching_query),
                false,
                false,
                false,
            ),
            RuntimeCharacterHitCandidateResult::Reject(
                RobotsHitQueryCandidateRejectReason::MissingCandidate
            )
        );
        assert_eq!(body.last_hit_query_serial, u16::MAX);

        assert_eq!(
            runtime_test_live_character_hit_candidate(
                Some(&mut body),
                true,
                context,
                RuntimeCharacterHitQueryGeometry::PreparedShape(touching_query),
                false,
                false,
                false,
            ),
            RuntimeCharacterHitCandidateResult::AnimDatumHitArea { hit: true }
        );
        assert_eq!(body.last_hit_query_serial, 0x1234);
        assert_eq!(
            runtime_test_live_character_hit_candidate(
                Some(&mut body),
                true,
                context,
                RuntimeCharacterHitQueryGeometry::PreparedShape(touching_query),
                false,
                false,
                false,
            ),
            RuntimeCharacterHitCandidateResult::Reject(
                RobotsHitQueryCandidateRejectReason::AlreadyHitByQuerySerial
            )
        );

        let miss_context = RobotsHitQueryCandidateContext {
            query_serial: 0x1235,
            ..context
        };
        assert_eq!(
            runtime_test_live_character_hit_candidate(
                Some(&mut body),
                true,
                miss_context,
                RuntimeCharacterHitQueryGeometry::PreparedShape(
                    RuntimeCharacterWorldShape::Sphere {
                        center_xyz: [10.0, 0.0, 0.0],
                        radius: 0.5,
                    }
                ),
                false,
                false,
                false,
            ),
            RuntimeCharacterHitCandidateResult::AnimDatumHitArea { hit: false }
        );
        assert_eq!(body.last_hit_query_serial, 0x1234);

        let alternate_context = RobotsHitQueryCandidateContext {
            flags: 0x200,
            query_serial: 0x1236,
            ..context
        };
        let mut sweep_samples = [RobotsHitSweepSample {
            point_xyzw: [1.25, 0.0, 0.0, 1.0],
            marker: 0.0,
        }];
        assert_eq!(
            runtime_test_live_character_hit_candidate(
                Some(&mut body),
                true,
                alternate_context,
                RuntimeCharacterHitQueryGeometry::SampleSweep {
                    plan: RobotsHitSampleSweepPlan {
                        mode: RobotsHitSampleSweepMode::SamplePoint,
                        query_scalar: 0.5,
                        radial_cull_radius: 0.0,
                        flags: ROBOTS_HIT_SAMPLE_MARK_CONSUMED_ON_HIT_FLAG,
                    },
                    source_xyzw: Some([0.0, 0.0, 0.0, 1.0]),
                    samples: &mut sweep_samples,
                },
                false,
                false,
                false,
            ),
            RuntimeCharacterHitCandidateResult::AlternateSampleSweep { hit: true }
        );
        assert_eq!(sweep_samples[0].marker, 100.0);
        assert_eq!(body.last_hit_query_serial, 0x1236);

        let group1_peer_context = RobotsHitQueryCandidateContext {
            query_serial: 0x1237,
            source_raw_group: Some(1),
            ..context
        };
        assert_eq!(
            runtime_test_live_character_hit_candidate(
                Some(&mut body),
                true,
                group1_peer_context,
                RuntimeCharacterHitQueryGeometry::PreparedShape(touching_query),
                false,
                false,
                false,
            ),
            RuntimeCharacterHitCandidateResult::Reject(
                RobotsHitQueryCandidateRejectReason::RawGroup1Peer
            )
        );
        assert_eq!(body.last_hit_query_serial, 0x1236);
    }

    #[test]
    fn character_root_motion_delta_uses_current_next_and_serialized_transition_ticks() {
        let track = ProcessedCharacterAnimationTrack {
            animation: 0x8300_0007,
            animskin: 0x0D00_0001,
            clip_rate: 60,
            transition_fixed_ticks: 5,
            frame_count: 2,
            root_motion_samples: vec![
                ProcessedCharacterRootMotionSample {
                    position: Vec3::ZERO,
                    rotation: Quat::IDENTITY,
                },
                ProcessedCharacterRootMotionSample {
                    position: Vec3::new(10.0, 0.0, 0.0),
                    rotation: Quat::from_rotation_y(0.5),
                },
            ],
            bone_chain: vec![0],
            poses: vec![
                ProcessedCharacterAnimationBonePose {
                    position: Vec3::ZERO,
                    rotation: Quat::IDENTITY,
                },
                ProcessedCharacterAnimationBonePose {
                    position: Vec3::ZERO,
                    rotation: Quat::IDENTITY,
                },
            ],
        };

        let delta = runtime_character_track_root_motion_delta(
            &track,
            0.0,
            1.0 / ROBOTS_CHARACTER_ANIMATION_FIXED_HZ,
            2.5 / ROBOTS_CHARACTER_ANIMATION_FIXED_HZ,
        )
        .expect("synthetic native root-motion delta");
        assert!(delta.native_translation.distance(Vec3::new(5.0, 0.0, 0.0)) < 1.0e-6);
        let (_, angle) = delta.native_rotation.to_axis_angle();
        assert!((angle - 0.25).abs() < 1.0e-5);
    }

    #[test]
    fn character_root_motion_apply_uses_live_owner_local_space() {
        let mut body = RuntimeCharacterBodyState {
            registration_mask: ROBOTS_GAMEPLAY_COLLISION_REGISTRATION_MASK,
            raw_hit_query_group: 1,
            last_hit_query_serial: u16::MAX,
            owner_position: Vec3::new(10.0, 0.0, 20.0),
            owner_rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            native_transform_scale: [1.0; 4],
            collision: ProcessedCharacterCollisionProfile {
                animskin: 0x0D00_0001,
                shape: ProcessedCharacterCollisionShape::Sphere { radius: 0.5 },
                local_center: Vec3::ZERO,
                local_orientation: Quat::IDENTITY.to_array(),
                transform_selector: 0,
            },
            hit_area: None,
            initial_animation: None,
            animation_modes: BTreeMap::new(),
            animation_mode_scripts: BTreeMap::new(),
            move_animation: None,
            turn_on_spot_l_animation: None,
            turn_on_spot_r_animation: None,
            initial_animation_seconds: 0.0,
            active_collision_local_poses: None,
            active_collision_bone_chain: None,
            collision_transition_snapshot: None,
        };
        body.apply_local_root_motion_delta(RuntimeCharacterRootMotionDelta {
            native_translation: Vec3::new(0.0, 0.0, 2.0),
            native_rotation: Quat::from_rotation_y(0.25),
        });
        assert!(body.owner_position.distance(Vec3::new(12.0, 0.0, 20.0)) < 1.0e-5);
        let forward = body.owner_rotation * Vec3::Z;
        let yaw = forward.x.atan2(forward.z);
        assert!((yaw - (std::f32::consts::FRAC_PI_2 + 0.25)).abs() < 1.0e-5);
    }

    #[test]
    fn character_world_capsule_matches_rodney_identity_bone_pose() {
        let body = RuntimeCharacterBodyState {
            registration_mask: ROBOTS_GAMEPLAY_COLLISION_REGISTRATION_MASK,
            raw_hit_query_group: 1,
            last_hit_query_serial: u16::MAX,
            owner_position: Vec3::new(10.0, 20.0, 30.0),
            owner_rotation: Quat::IDENTITY,
            native_transform_scale: [1.0; 4],
            collision: ProcessedCharacterCollisionProfile {
                animskin: 0x0D00_0001,
                shape: ProcessedCharacterCollisionShape::Capsule {
                    half_segment: 0.45,
                    radius: 0.40,
                },
                local_center: Vec3::new(0.0, 0.85, 0.0),
                local_orientation: Quat::IDENTITY.to_array(),
                transform_selector: 0,
            },
            hit_area: None,
            initial_animation: None,
            animation_modes: BTreeMap::new(),
            animation_mode_scripts: BTreeMap::new(),
            move_animation: None,
            turn_on_spot_l_animation: None,
            turn_on_spot_r_animation: None,
            initial_animation_seconds: 0.0,
            active_collision_local_poses: None,
            active_collision_bone_chain: None,
            collision_transition_snapshot: None,
        };
        let RuntimeCharacterWorldShape::Capsule {
            start_xyz,
            delta_xyz,
            radius,
        } = runtime_character_world_shape_from_bone_matrix(&body, Mat4::IDENTITY)
        else {
            panic!("Rodney MapCollision must remain a capsule");
        };
        assert!(Vec3::from_array(start_xyz).distance(Vec3::new(10.0, 20.4, 30.0)) < 1.0e-6);
        assert!(Vec3::from_array(delta_xyz).distance(Vec3::new(0.0, 0.9, 0.0)) < 1.0e-6);
        assert!((radius - 0.40).abs() < 1.0e-6);
    }

    #[test]
    fn character_world_shape_applies_bone_then_owner_and_max_scale() {
        let body = RuntimeCharacterBodyState {
            registration_mask: ROBOTS_GAMEPLAY_COLLISION_REGISTRATION_MASK,
            raw_hit_query_group: 1,
            last_hit_query_serial: u16::MAX,
            owner_position: Vec3::new(10.0, 0.0, 0.0),
            owner_rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            native_transform_scale: [2.0; 4],
            collision: ProcessedCharacterCollisionProfile {
                animskin: 0x0D00_0001,
                shape: ProcessedCharacterCollisionShape::Sphere { radius: 0.5 },
                local_center: Vec3::new(0.0, 1.0, 0.0),
                local_orientation: Quat::IDENTITY.to_array(),
                transform_selector: 0,
            },
            hit_area: None,
            initial_animation: None,
            animation_modes: BTreeMap::new(),
            animation_mode_scripts: BTreeMap::new(),
            move_animation: None,
            turn_on_spot_l_animation: None,
            turn_on_spot_r_animation: None,
            initial_animation_seconds: 0.0,
            active_collision_local_poses: None,
            active_collision_bone_chain: None,
            collision_transition_snapshot: None,
        };
        let bone = Mat4::from_scale_rotation_translation(
            Vec3::new(2.0, 1.0, 0.5),
            Quat::IDENTITY,
            Vec3::new(0.0, 0.0, 2.0),
        );
        let RuntimeCharacterWorldShape::Sphere { center_xyz, radius } =
            runtime_character_world_shape_from_bone_matrix(&body, bone)
        else {
            panic!("sphere profile must remain a sphere");
        };
        assert!(Vec3::from_array(center_xyz).distance(Vec3::new(14.0, 2.0, 0.0)) < 1.0e-5);
        assert!((radius - 2.0).abs() < 1.0e-6);
    }

    #[test]
    fn platform_one_tick_angular_delta_matches_native_fixed_60hz_units() {
        let delta = runtime_platform_one_tick_angular_delta(Vec3::new(60.0, -120.0, 30.0), 1);
        let expected = Vec3::new(1.0, -2.0, 0.5) * std::f32::consts::PI / 180.0;
        assert!(
            delta.distance(expected) < 1.0e-7,
            "delta={delta:?} expected={expected:?}"
        );
    }

    #[test]
    fn platform_peer_body_carry_uses_native_finite_rotation_order() {
        let linear = Vec3::new(2.0, -3.0, 4.0);
        let platform_origin = Vec3::new(10.0, 20.0, -5.0);
        let relative = Vec3::new(1.25, -0.75, 2.5);
        let peer_origin = platform_origin + relative;
        let angles = Vec3::new(0.31, -0.22, 0.47);
        let rotated = Quat::from_rotation_y(angles.y)
            * (Quat::from_rotation_x(angles.x) * (Quat::from_rotation_z(angles.z) * relative));
        let expected = linear + (rotated - relative) * 60.0;
        let actual = runtime_platform_peer_body_carry_velocity(
            linear,
            platform_origin,
            peer_origin,
            angles,
            1.0,
        );
        assert!(
            actual.distance(expected) < 1.0e-4,
            "actual={actual:?} expected={expected:?}"
        );
    }

    #[test]
    fn platform_peer_body_carry_does_not_invent_rotation_without_a_peer_step() {
        let linear = Vec3::new(1.0, 2.0, 3.0);
        let actual = runtime_platform_peer_body_carry_velocity(
            linear,
            Vec3::new(5.0, 6.0, 7.0),
            Vec3::new(8.0, 9.0, 10.0),
            Vec3::ZERO,
            1.0,
        );
        assert!(actual.distance(linear) < 1.0e-6);
    }

    #[test]
    fn platform_spin_preserves_the_body_local_rotation_axis() {
        let base = Quat::from_rotation_y(0.977_145_73);
        let rotated = compose_platform_local_rotation(base, Vec3::new(0.0, 0.0, 15.0), 7.0, 1.0);

        let expected_axis = base * Vec3::Z;
        let actual_axis = rotated * Vec3::Z;
        assert!(
            expected_axis.distance(actual_axis) < 1.0e-5,
            "expected_axis={expected_axis:?} actual_axis={actual_axis:?}"
        );
    }

    #[test]
    fn platform_spin_keeps_the_serialized_direction_and_speed() {
        let base = Quat::from_rotation_y(0.523_666_9);
        let forward = compose_platform_local_rotation(base, Vec3::new(0.0, 0.0, 20.0), 1.0, 1.0);
        let reverse = compose_platform_local_rotation(base, Vec3::new(0.0, 0.0, -20.0), 1.0, 1.0);
        let local_x = base * Vec3::X;
        let expected_forward = base * (Quat::from_rotation_z(20.0_f32.to_radians()) * Vec3::X);
        let expected_reverse = base * (Quat::from_rotation_z(-20.0_f32.to_radians()) * Vec3::X);

        assert!(forward.mul_vec3(Vec3::X).distance(expected_forward) < 1.0e-5);
        assert!(reverse.mul_vec3(Vec3::X).distance(expected_reverse) < 1.0e-5);
        assert!(forward.mul_vec3(Vec3::X).distance(local_x) > 0.1);
        assert!(reverse.mul_vec3(Vec3::X).distance(local_x) > 0.1);
    }
}
