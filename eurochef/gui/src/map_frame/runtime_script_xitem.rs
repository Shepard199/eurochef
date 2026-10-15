use super::{
    runtime_pickup::{NativeDynamicPickupSpawnRequest, ROBOTS_PICKUP_EFFECT_FILE_UID},
    MapFrame,
};
use crate::{maps::ProcessedMap, render::RenderStore};
use eurochef_shared::{
    robots_runtime::{
        character_effects::{
            apply_ai_character_attachment_event, RobotsCharacterAttachmentRuntimeState,
        },
        events::{
            RobotsHandlerEventReturnPolicy, RobotsHandlerScriptCommandFamily,
            RobotsHandlerScriptCommandSemantic,
        },
        locomotion::ROBOTS_FIXED_STEP_SECONDS,
        message_presentation::build_cutscene_show_message_record,
        pickup::{
            robots_generated_pickup_native_runtime_type, robots_generated_pickup_resource_uid,
        },
        script_host::execute_recovered_handler_script_command,
        script_scheduler::{
            advance_script_scheduler, RobotsScriptNativeEventResult, RobotsScriptSchedulerState,
        },
    },
    script::UXGeoScript,
};

/// Common factory request for a gameplay-created `XItemHandler_Script`.
///
/// This type lives with the common Script-XItem host rather than inside AI:
/// Piranha, projectiles, explosion fragments, pickups and future gameplay
/// producers all converge on the same native owner class before presentation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct NativeScriptXItemSpawnRequest {
    pub(super) owner_key: u64,
    pub(super) file_uid: u32,
    pub(super) script_uid: u32,
    pub(super) position_xyzw: [f32; 4],
    /// Native spawned Script XItem rotation lane `+0xE0..+0xEC`.
    /// Producers without inherited orientation use identity explicitly.
    pub(super) rotation: glam::Quat,
}

/// Live runtime ownership for gameplay-created `XItemHandler_Script` instances.
///
/// Native `0x00455C50` / generic `0x004043A0` create a separate Script XItem,
/// attach the resolved Script resource and register it with the XItem manager.
/// Keep this host class-agnostic so Piranha splashes, FootDust, Pickup effects and
/// later projectile/effect factories can share the same lifecycle instead of
/// growing class-specific presentation runtimes.
#[derive(Clone, Debug)]
pub(super) struct NativeTransientScriptXItemRuntime {
    /// Provenance only. The spawned XItem has its own lifetime after creation.
    pub(super) source_owner_key: u64,
    pub(super) file_uid: u32,
    pub(super) script_uid: u32,
    /// Native XItem transform lane `+0xD0..+0xDC` copied by the factory.
    pub(super) position_xyzw: [f32; 4],
    /// Native XItem orientation lane `+0xE0..+0xEC`.
    pub(super) rotation: glam::Quat,
    /// Shared EXItemAnimator_Script command/event stream contract.
    pub(super) script_scheduler: RobotsScriptSchedulerState,
    /// Common Script-owner presentation attachments. The same native virtuals
    /// back AI_Character and generic XItemHandler_Script owners.
    pub(super) character_attachments: RobotsCharacterAttachmentRuntimeState,
    /// Visual/controller time is not the same thing as command-stream EOF.
    /// Opcode18 stops dispatch, while Entity/Particle commands remain alive
    /// through their serialized Script length.
    pub(super) visual_frame: f32,
}

impl NativeTransientScriptXItemRuntime {
    fn from_request(request: NativeScriptXItemSpawnRequest) -> Self {
        Self {
            source_owner_key: request.owner_key,
            file_uid: request.file_uid,
            script_uid: request.script_uid,
            position_xyzw: request.position_xyzw,
            rotation: request.rotation,
            script_scheduler: RobotsScriptSchedulerState::default(),
            character_attachments: RobotsCharacterAttachmentRuntimeState::default(),
            visual_frame: 0.0,
        }
    }

    pub(super) fn script_time_seconds(&self, script: &UXGeoScript) -> f32 {
        script.time_at_frame(self.visual_frame)
    }

    fn fixed_tick_delta_frames(script: &UXGeoScript) -> f32 {
        script.timeline_framerate() * ROBOTS_FIXED_STEP_SECONDS
    }

    fn advance_visual_lifetime(&mut self, script: &UXGeoScript) -> bool {
        self.visual_frame += Self::fixed_tick_delta_frames(script);
        self.visual_frame < script.length as f32
    }
}

fn materialize_script_spawn_requests(
    requests: Vec<NativeScriptXItemSpawnRequest>,
    render_store: &RenderStore,
) -> Vec<NativeTransientScriptXItemRuntime> {
    requests
        .into_iter()
        // Native resource lookup/factory failure aborts the XItem create. Do not
        // keep a zombie request when the owning EDB/Script is unavailable.
        .filter(|request| {
            render_store
                .get_script(request.file_uid, request.script_uid)
                .is_some()
        })
        .map(NativeTransientScriptXItemRuntime::from_request)
        .collect()
}

impl MapFrame {
    /// Materialize gameplay-created Script XItems after their producer XItem has
    /// run. A newly-created XItem begins at Script frame 0 and is first serviced
    /// on the following fixed XItem tick, matching manager insertion ownership
    /// without retroactively advancing an object created mid-tick.
    pub(super) fn consume_native_script_xitem_spawns(&mut self) {
        if self.native_script_xitem_spawns.is_empty() {
            return;
        }
        let requests = std::mem::take(&mut self.native_script_xitem_spawns);
        let created = {
            let render_store = self.render_store.read();
            materialize_script_spawn_requests(requests, &render_store)
        };
        self.native_transient_script_xitems.extend(created);
    }

    /// Advance the common transient `XItemHandler_Script` host once per native
    /// fixed XItem tick. Script presentation is sampled later by the GUI/UE
    /// adapter from this same scheduler frame.
    pub(super) fn advance_native_transient_script_xitems_fixed(
        &mut self,
        map: &ProcessedMap,
        wall_time: f64,
    ) {
        if self.native_transient_script_xitems.is_empty() {
            return;
        }

        // Clone only the small Script descriptors needed by this tick so the
        // RenderStore lock never overlaps mutable gameplay state.
        let scripts: Vec<Option<UXGeoScript>> = {
            let render_store = self.render_store.read();
            self.native_transient_script_xitems
                .iter()
                .map(|runtime| {
                    render_store
                        .get_script(runtime.file_uid, runtime.script_uid)
                        .cloned()
                })
                .collect()
        };

        let gameplay_state = &mut self.native_script_gameplay_state;
        let game_control_mode = self.native_game_control_runtime.mode_50f;
        let current_file_uid = map.hashcode;
        let mut pickup_spawns = Vec::new();
        let mut sound_stops = Vec::new();
        let mut message_records = Vec::new();
        let mut camera_shakes = Vec::new();
        let mut keep = Vec::with_capacity(self.native_transient_script_xitems.len());
        for (mut runtime, script) in self
            .native_transient_script_xitems
            .drain(..)
            .zip(scripts.into_iter())
        {
            let Some(script) = script else {
                // Native resource ownership cannot survive a missing resource.
                continue;
            };

            let mut exit_script_requested = false;
            if !runtime.script_scheduler.finished {
                let delta_frames =
                    NativeTransientScriptXItemRuntime::fixed_tick_delta_frames(&script);
                let character_attachments = &mut runtime.character_attachments;
                let _step = advance_script_scheduler(
                    &script,
                    &mut runtime.script_scheduler,
                    delta_frames,
                    |event| {
                        let Some(execution) = execute_recovered_handler_script_command(
                            RobotsHandlerScriptCommandFamily::ScriptLifecycle,
                            event,
                            &map.inventory_definitions,
                            &map.mission_definitions,
                            gameplay_state,
                            false,
                        ) else {
                            return RobotsScriptNativeEventResult::Hold;
                        };

                        if matches!(
                            execution.plan.semantic,
                            RobotsHandlerScriptCommandSemantic::ExitScript
                        ) {
                            exit_script_requested = true;
                            return RobotsScriptNativeEventResult::Continue;
                        }

                        // Common generic Script owner virtuals +0xD4/+0xD8/+0xDC
                        // are the same attachment implementations used by AI_Character.
                        // Capture presentation ownership now; GUI/UE renders it later.
                        let _ = apply_ai_character_attachment_event(character_attachments, event);

                        if let RobotsHandlerScriptCommandSemantic::GeneratePickup {
                            pickup: Some(pickup_uid),
                            quantity: Some(quantity),
                        } = execution.plan.semantic
                        {
                            // Generic 0x00402EF0 calls vslot +0x9C exactly once.
                            // `quantity` is converted through __ftol and becomes
                            // owner +0x264; it is not a spawn count.
                            let owner_datum_264 = quantity.trunc() as i32;
                            if owner_datum_264 < 0 {
                                return RobotsScriptNativeEventResult::Continue;
                            }
                            if let Some(native_runtime_type) =
                                robots_generated_pickup_native_runtime_type(pickup_uid)
                            {
                                // The common generated families live in O01_Pickups.
                                // HealthReplenish's normal-mode registration resource
                                // is the shipped 0x04000022 Script; 0x004DA710 only
                                // replaces it in GameControl modes 2/3/4.
                                let fallback_resource_uid = match pickup_uid {
                                    0x4700_0003 | 0x4700_0023 => 0x0400_0022,
                                    _ => pickup_uid,
                                };
                                let resource_uid = robots_generated_pickup_resource_uid(
                                    fallback_resource_uid,
                                    owner_datum_264,
                                    native_runtime_type,
                                    game_control_mode,
                                    current_file_uid,
                                );
                                pickup_spawns.push(NativeDynamicPickupSpawnRequest {
                                    source_owner_key: runtime.source_owner_key,
                                    resource_file_uid: ROBOTS_PICKUP_EFFECT_FILE_UID,
                                    resource_uid,
                                    pickup_uid,
                                    owner_datum_264,
                                    position: glam::Vec3::new(
                                        runtime.position_xyzw[0],
                                        runtime.position_xyzw[1],
                                        runtime.position_xyzw[2],
                                    ),
                                    rotation: runtime.rotation,
                                });
                            }
                            return RobotsScriptNativeEventResult::Continue;
                        }

                        if let RobotsHandlerScriptCommandSemantic::SoundStop {
                            sound: Some(sound_uid),
                        } = execution.plan.semantic
                        {
                            // 0x1600003B -> native sound lookup -> object +0x10 kill bit.
                            // Defer the actual adapter mutation until after the Script-XItem
                            // drain so audio ownership stays outside shared scheduling state.
                            sound_stops.push(sound_uid);
                            return RobotsScriptNativeEventResult::Continue;
                        }

                        if let RobotsHandlerScriptCommandSemantic::ShowMessage {
                            message: Some(message_uid),
                            value: Some(parameter_08),
                        } = execution.plan.semantic
                        {
                            let use_script_length =
                                event.native_arg_byte(4).is_some_and(|flags| flags & 1 == 0);
                            if let (Some(command_length), Some(message)) = (
                                event.length,
                                map.text_groups.message_definition(message_uid),
                            ) {
                                if let Some(record) = build_cutscene_show_message_record(
                                    message,
                                    (wall_time * 60.0) as f32,
                                    script.framerate,
                                    command_length,
                                    use_script_length,
                                    parameter_08.trunc() as i32,
                                    event.native_arg_word(3).unwrap_or(u32::MAX) as i32,
                                    event.native_arg_word(4).unwrap_or(u32::MAX) as i32,
                                ) {
                                    message_records.push(record);
                                }
                            }
                            return RobotsScriptNativeEventResult::Continue;
                        }

                        if let RobotsHandlerScriptCommandSemantic::ShakeCamera {
                            arg0: Some(magnitude),
                            arg1: Some(duration_updates),
                            flags: Some(flags),
                        } = execution.plan.semantic
                        {
                            // Generic 0x00402EF0 uses the same 0x00433970 camera-shake
                            // service as Cutscene. __ftol converts both float arguments;
                            // command flags bit0 selects the player channel and bit1 the
                            // primary camera channel. Native Player +0x2EA bit3 is not
                            // currently represented by the Maps player state, so that
                            // suppression remains an explicit adapter boundary rather than
                            // inventing an unrelated flag.
                            camera_shakes.push((
                                magnitude.trunc() as i32,
                                duration_updates.trunc() as i32,
                                flags & 2 != 0,
                                flags & 1 != 0,
                            ));
                            return RobotsScriptNativeEventResult::Continue;
                        }

                        // Generic XItemHandler_Script vslot +0x60 is 0x004037F0.
                        // Plain Wait 0x1600000B has no external predicate there and
                        // returns native 1 immediately. Keeping it forever was a host
                        // reconstruction bug, not original behavior.
                        if matches!(
                            execution.plan.semantic,
                            RobotsHandlerScriptCommandSemantic::Wait
                        ) {
                            return RobotsScriptNativeEventResult::Continue;
                        }

                        match execution.plan.return_policy {
                            RobotsHandlerEventReturnPolicy::Zero => {
                                RobotsScriptNativeEventResult::Continue
                            }
                            RobotsHandlerEventReturnPolicy::One
                            | RobotsHandlerEventReturnPolicy::StateDependent
                            | RobotsHandlerEventReturnPolicy::HostResult
                            | RobotsHandlerEventReturnPolicy::Delegate => {
                                // Host/state-dependent commands stay on the Event
                                // until their exact external condition is implemented.
                                RobotsScriptNativeEventResult::Hold
                            }
                        }
                    },
                );
            }

            // Opcode18 is command-stream EOF, not XItem teardown. Natural
            // effect lifetime follows the Script animator's serialized length.
            // An explicit ExitScript remains an early lifecycle exit.
            if exit_script_requested {
                continue;
            }
            if runtime.advance_visual_lifetime(&script) {
                keep.push(runtime);
            }
        }
        self.native_transient_script_xitems = keep;
        self.native_dynamic_pickup_spawns.extend(pickup_spawns);
        for record in message_records {
            self.native_message_presentation.enqueue(record);
        }
        for (magnitude, duration_updates, primary_channel, player_channel) in camera_shakes {
            self.native_camera_shake.request(
                magnitude,
                duration_updates,
                primary_channel,
                player_channel,
            );
        }
        if !sound_stops.is_empty() {
            let mut preview = self.sound_preview.lock();
            for sound_uid in sound_stops {
                preview.stop_sound_uid(sound_uid);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eurochef_shared::script::{UXGeoScriptCommand, UXGeoScriptCommandData};

    fn script(hashcode: u32, commands: Vec<UXGeoScriptCommand>) -> UXGeoScript {
        UXGeoScript {
            hashcode,
            framerate: 30.0,
            length: 30,
            num_threads: 1,
            commands,
            serialized_controller_count: 0,
            controller_record_metadata: vec![],
            controllers: vec![],
            controller_group_indices: vec![],
            controller_groups: vec![],
        }
    }

    #[test]
    fn materialization_requires_the_exact_script_resource_and_preserves_transform() {
        let mut render_store = RenderStore::new();
        render_store.insert_script(0x0100_0066, script(0x0400_029C, vec![]));

        let accepted = NativeScriptXItemSpawnRequest {
            owner_key: 0x1234,
            file_uid: 0x0100_0066,
            script_uid: 0x0400_029C,
            position_xyzw: [1.0, 2.0, 3.0, 0.0],
            rotation: glam::Quat::from_rotation_y(0.75),
        };
        let missing = NativeScriptXItemSpawnRequest {
            script_uid: 0x0400_029D,
            ..accepted
        };

        let live = materialize_script_spawn_requests(vec![accepted, missing], &render_store);
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].source_owner_key, 0x1234);
        assert_eq!(live[0].file_uid, 0x0100_0066);
        assert_eq!(live[0].script_uid, 0x0400_029C);
        assert_eq!(live[0].position_xyzw, [1.0, 2.0, 3.0, 0.0]);
        assert_eq!(live[0].rotation, accepted.rotation);
        assert_eq!(live[0].script_scheduler.frame, 0.0);
        assert_eq!(live[0].visual_frame, 0.0);
    }

    #[test]
    fn transient_script_uses_shared_scheduler_and_terminates_on_opcode18() {
        let script = script(
            0x0400_029C,
            vec![
                UXGeoScriptCommand {
                    opcode: 1,
                    start: 0,
                    length: 1,
                    controller_header_index: 0,
                    controller_index: 0,
                    parent_controller_index: 0xff,
                    data: UXGeoScriptCommandData::Unknown {
                        cmd: 1,
                        data: vec![],
                    },
                },
                UXGeoScriptCommand {
                    opcode: 18,
                    start: 1,
                    length: 0,
                    controller_header_index: 0,
                    controller_index: 0,
                    parent_controller_index: 0xff,
                    data: UXGeoScriptCommandData::Unknown {
                        cmd: 18,
                        data: vec![],
                    },
                },
            ],
        );
        let mut scheduler = RobotsScriptSchedulerState::default();
        let delta_frames = NativeTransientScriptXItemRuntime::fixed_tick_delta_frames(&script);
        assert_eq!(delta_frames, 0.5);

        let first = advance_script_scheduler(&script, &mut scheduler, delta_frames, |_| {
            RobotsScriptNativeEventResult::Continue
        });
        assert!(!first.terminated);
        assert_eq!(scheduler.frame, 0.5);

        let second = advance_script_scheduler(&script, &mut scheduler, delta_frames, |_| {
            RobotsScriptNativeEventResult::Continue
        });
        assert!(!second.terminated);
        assert_eq!(scheduler.frame, 1.0);

        let third = advance_script_scheduler(&script, &mut scheduler, delta_frames, |_| {
            RobotsScriptNativeEventResult::Continue
        });
        assert!(third.terminated);
        assert!(scheduler.finished);

        // Command EOF does not end visual ownership. A 30-frame Script remains
        // renderable for exactly one second at 30 FPS on the fixed 60 Hz engine
        // timeline and is removed only when frame 30 is reached.
        let mut runtime =
            NativeTransientScriptXItemRuntime::from_request(NativeScriptXItemSpawnRequest {
                owner_key: 1,
                file_uid: 0x0100_0066,
                script_uid: 0x0400_029C,
                position_xyzw: [0.0; 4],
                rotation: glam::Quat::IDENTITY,
            });
        runtime.script_scheduler = scheduler;
        for tick in 1..60 {
            assert!(runtime.advance_visual_lifetime(&script));
            assert_eq!(runtime.visual_frame, tick as f32 * 0.5);
        }
        assert!(!runtime.advance_visual_lifetime(&script));
        assert_eq!(runtime.visual_frame, 30.0);
    }
}
