use super::{
    runtime_script_xitem::NativeScriptXItemSpawnRequest, MapFrame, NativePickupXItemRuntime,
};
use crate::maps::{robots_pickup_visual, ProcessedMap};
use eurochef_shared::robots_runtime::{
    inventory::{RobotsInventoryDefinition, RobotsInventoryState},
    locomotion::ROBOTS_FIXED_STEP_SECONDS,
    pickup::{
        robots_generated_pickup_handler_argument, robots_generated_pickup_native_runtime_type,
        RobotsPickupGlobalSchedulerState, RobotsPickupHandlerRuntimeState,
        RobotsPickupProximityPlan, RobotsPickupSpawnPlan,
        ROBOTS_PICKUP_GENERATED_RESOURCE_PERIODIC_BIT, ROBOTS_PICKUP_LIFETIME_FLAG,
        ROBOTS_PICKUP_PERIODIC_FLAG, ROBOTS_PICKUP_SPIN_STEP_RADIANS,
    },
    player_items::RobotsPlayerItemState,
};
use glam::{Quat, Vec3};

pub(super) const ROBOTS_PICKUP_EFFECT_FILE_UID: u32 = 0x0100_0003;
pub(super) const ROBOTS_PICKUP_HIGHLIGHT_UID: u32 = 0x0400_02FB;
pub(super) const ROBOTS_PICKUP_HIGHLIGHT_GOLD_UID: u32 = 0x0400_02FC;
pub(super) const ROBOTS_PICKUP_SPOT_UID: u32 = 0x0400_02F9;
pub(super) const ROBOTS_PICKUP_SPOT_GOLD_UID: u32 = 0x0400_02FA;

/// Common producer boundary for native `XItemHandler::GeneratePickupFromHash 0x00403F20`.
/// Serialized XTrigger_Pickup instances never pass through this queue.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct NativeDynamicPickupSpawnRequest {
    pub(super) source_owner_key: u64,
    /// File that owns `resource_uid`; retained for Script/visual adapters and UE provenance.
    pub(super) resource_file_uid: u32,
    /// Explicit resource argument passed to GeneratePickupFromHash. Native copies
    /// bit 0x02000000 from this UID into Handler periodic bit 0x20.
    pub(super) resource_uid: u32,
    pub(super) pickup_uid: u32,
    /// Native XItem +0x264 producer datum. Explosion fragments pass their signed
    /// FX03 row +0x18 value here; this is not Handler +0x3CC.
    pub(super) owner_datum_264: i32,
    pub(super) position: Vec3,
    pub(super) rotation: Quat,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct NativeDynamicPickupRuntime {
    pub(super) spawn: NativeDynamicPickupSpawnRequest,
    pub(super) native_runtime_type: u32,
    pub(super) handler: RobotsPickupHandlerRuntimeState,
    pub(super) spin_radians: f32,
    /// Exact last lifetime opacity scalar written by 0x00414210. One means fully
    /// visible; the final second falls toward zero.
    pub(super) fade_scalar: f32,
    pub(super) pending_destroy: bool,
}

impl NativeDynamicPickupRuntime {
    fn from_spawn(
        spawn: NativeDynamicPickupSpawnRequest,
        global: &mut RobotsPickupGlobalSchedulerState,
    ) -> Option<Self> {
        let native_runtime_type = robots_generated_pickup_native_runtime_type(spawn.pickup_uid)?;
        let mut handler = RobotsPickupHandlerRuntimeState::new(
            spawn.pickup_uid,
            robots_generated_pickup_handler_argument(native_runtime_type),
            global,
        );
        handler.flags_3d1 |= ROBOTS_PICKUP_LIFETIME_FLAG;
        if spawn.resource_uid & ROBOTS_PICKUP_GENERATED_RESOURCE_PERIODIC_BIT != 0 {
            handler.flags_3d1 |= ROBOTS_PICKUP_PERIODIC_FLAG;
        }
        Some(Self {
            spawn,
            native_runtime_type,
            handler,
            spin_radians: 0.0,
            fade_scalar: 1.0,
            pending_destroy: false,
        })
    }

    fn owner_rotation(self) -> Quat {
        self.spawn.rotation * Quat::from_rotation_y(self.spin_radians)
    }

    fn effect_script_request(self, script_uid: u32) -> NativeScriptXItemSpawnRequest {
        NativeScriptXItemSpawnRequest {
            owner_key: self.spawn.source_owner_key,
            file_uid: self.spawn.resource_file_uid,
            script_uid,
            position_xyzw: [
                self.spawn.position.x,
                self.spawn.position.y,
                self.spawn.position.z,
                0.0,
            ],
            rotation: self.owner_rotation(),
        }
    }

    fn spot_effect_request(self) -> Option<NativeScriptXItemSpawnRequest> {
        (self.handler.flags_3d1 & ROBOTS_PICKUP_PERIODIC_FLAG != 0).then(|| {
            self.effect_script_request(if self.native_runtime_type == 0x2D {
                ROBOTS_PICKUP_SPOT_GOLD_UID
            } else {
                ROBOTS_PICKUP_SPOT_UID
            })
        })
    }
}

/// Concrete Player vslot +0x174 (0x004B0D50) returns 2.0. Pickup +0x34
/// squares that value before applying its own 2.5-unit broad-range floor.
const ROBOTS_PLAYER_PICKUP_INTERACTION_RANGE_SQUARED: f32 = 4.0;

fn native_pickup_source_requests_deferred_output(data: &[Option<u32>]) -> bool {
    data.get(7)
        .copied()
        .flatten()
        .is_some_and(|value| value & 1 != 0)
}

fn native_pickup_definition(
    definitions: &[RobotsInventoryDefinition],
    uid: u32,
) -> Option<RobotsInventoryDefinition> {
    definitions
        .iter()
        .copied()
        .find(|definition| definition.uid == uid)
}

/// Native 0x004DA8B0 + the following container +0x2C preflight in
/// XItemHandler_Pickup::+0x34. Player +0xB8 routes 0x47xxxxxx to the
/// ordinary inventory and 0x48xxxxxx to the Player-item flag container.
fn native_pickup_player_preflight(
    definitions: &[RobotsInventoryDefinition],
    inventory: &RobotsInventoryState,
    player_items: &RobotsPlayerItemState,
    uid: u32,
    quantity: i32,
) -> (bool, Option<u32>) {
    match uid & 0x7F00_0000 {
        0x4700_0000 => {
            let definition = native_pickup_definition(definitions, uid);
            let definition_bit0_policy =
                definition.is_some_and(|definition| definition.is_bitset());
            let preflight = (!definition_bit0_policy).then(|| {
                inventory
                    .preview_add(definition, quantity)
                    .native_result_code
            });
            (definition_bit0_policy, preflight)
        }
        0x4800_0000 => (
            false,
            Some(player_items.preflight_add_result(uid, quantity, true)),
        ),
        _ => (false, None),
    }
}

/// Concrete Player Handler +0x94 (0x004AFC90). The native implementation
/// resolves the same 0x47/0x48 container through +0xB8, chooses +0x30 or
/// +0x34 according to container +0x28, and accepts only status 1/4.
fn commit_native_pickup_to_player(
    definitions: &[RobotsInventoryDefinition],
    inventory: &mut RobotsInventoryState,
    player_items: &mut RobotsPlayerItemState,
    uid: u32,
    quantity: i32,
) -> bool {
    let result = match uid & 0x7F00_0000 {
        0x4700_0000 => {
            inventory
                .add(native_pickup_definition(definitions, uid), quantity)
                .native_result_code
        }
        0x4800_0000 => player_items.add_with_native_result(uid, quantity, true),
        _ => return false,
    };
    matches!(result, 1 | 4)
}

fn native_serialized_pickup_uses_periodic_entity_resource(
    trigger_type: u32,
    data: &[Option<u32>],
) -> bool {
    robots_pickup_visual(trigger_type, data).is_some_and(|visual| {
        // 0x0048A4C0 tests the actually selected resource class and sets Handler
        // bit0x20 only for HT_Entity. Restrict this bridge to O01 resources that
        // are exact native world assets; editor-only cross-file fallbacks (Fuel)
        // must not manufacture native Handler flags.
        visual.file == ROBOTS_PICKUP_EFFECT_FILE_UID && visual.object & 0x7F00_0000 == 0x0200_0000
    })
}

pub(super) fn native_pickup_xitem_from_spawn(
    trigger_type: u32,
    data: &[Option<u32>],
    game_flags: u32,
    spawn: RobotsPickupSpawnPlan,
    global: &mut RobotsPickupGlobalSchedulerState,
) -> NativePickupXItemRuntime {
    let mut handler = RobotsPickupHandlerRuntimeState::new(
        spawn.layout.handler_datum_uid,
        spawn.layout.quantity,
        global,
    );
    if native_serialized_pickup_uses_periodic_entity_resource(trigger_type, data) {
        handler.flags_3d1 |= ROBOTS_PICKUP_PERIODIC_FLAG;
    }
    // XTrigger_Pickup::CreateItem tests trigger +0x4C bit8 and clears Handler
    // +0x3D2. Preserve that class-owned cadence gate instead of folding it into
    // generic trigger proximity.
    if game_flags & 0x100 != 0 {
        handler.proximity_cadence_enabled_3d2 = false;
    }
    NativePickupXItemRuntime {
        spawn,
        handler,
        spin_radians: spawn.initial_spin_radians,
    }
}

pub(super) fn native_pickup_owner_rotation(
    serialized_rotation: Vec3,
    runtime: &NativePickupXItemRuntime,
) -> Quat {
    let base = Quat::from_euler(
        glam::EulerRot::ZXY,
        serialized_rotation.z,
        serialized_rotation.x,
        serialized_rotation.y,
    );
    base * Quat::from_rotation_y(runtime.spin_radians)
}

fn pickup_script_request(
    key: u64,
    position: Vec3,
    serialized_rotation: Vec3,
    runtime: &NativePickupXItemRuntime,
    script_uid: u32,
) -> NativeScriptXItemSpawnRequest {
    NativeScriptXItemSpawnRequest {
        owner_key: key,
        file_uid: ROBOTS_PICKUP_EFFECT_FILE_UID,
        script_uid,
        position_xyzw: [position.x, position.y, position.z, 0.0],
        rotation: native_pickup_owner_rotation(serialized_rotation, runtime),
    }
}

pub(super) fn native_pickup_spot_effect_request(
    key: u64,
    position: Vec3,
    serialized_rotation: Vec3,
    runtime: &NativePickupXItemRuntime,
) -> Option<NativeScriptXItemSpawnRequest> {
    (runtime.handler.flags_3d1 & ROBOTS_PICKUP_PERIODIC_FLAG != 0).then(|| {
        let script_uid = if runtime.spawn.layout.native_runtime_type == 0x2D {
            ROBOTS_PICKUP_SPOT_GOLD_UID
        } else {
            ROBOTS_PICKUP_SPOT_UID
        };
        pickup_script_request(key, position, serialized_rotation, runtime, script_uid)
    })
}

impl MapFrame {
    /// Service serialized XItemHandler_Pickup +0x0C/+0x34 in native XItem order.
    /// Periodic presentation, owner spin and Player collection all stay in this
    /// one host. Collection commits through the already-owned 0x47 inventory or
    /// 0x48 Player-item container; no GUI-only inventory is introduced.
    pub(super) fn advance_native_pickup_xitems_fixed(
        &mut self,
        map: &ProcessedMap,
        player_position: Option<Vec3>,
    ) {
        self.native_pickup_global_scheduler.begin_fixed_update();

        // XItem order is stable; FxHashMap iteration is not. The low dword of
        // serialized runtime keys is the trigger index, which preserves native
        // TriggerManager creation order for this host.
        let mut keys = self
            .native_pickup_xitems
            .keys()
            .copied()
            .collect::<Vec<_>>();
        keys.sort_unstable();

        let player_mode = self.native_game_control_runtime.mode_50f;
        let mut script_spawns = Vec::new();
        for key in keys {
            let trigger_index = key as u32 as usize;
            let Some(trigger) = map.triggers.get(trigger_index) else {
                continue;
            };
            if !self
                .native_pickup_trigger_lifecycle
                .get(&key)
                .is_some_and(|state| state.xitem_exists)
            {
                continue;
            }

            let Some(snapshot) = self.native_pickup_xitems.get(&key).copied() else {
                continue;
            };
            let (definition_bit0_policy, preflight_result) = native_pickup_player_preflight(
                &map.inventory_definitions,
                &self.native_script_gameplay_state.inventory,
                &self.native_player_item_state,
                snapshot.handler.pickup_uid_3c8,
                snapshot.handler.quantity_3cc,
            );

            let Some(runtime) = self.native_pickup_xitems.get_mut(&key) else {
                continue;
            };

            let periodic = runtime.handler.service_periodic(
                runtime.spawn.layout.native_runtime_type,
                false, // bit0x20 is installed only for entity-backed owners.
                &mut self.native_pickup_global_scheduler,
            );
            if let Some(script_uid) = periodic.play_pickup_sound_uid {
                debug_assert!(matches!(
                    script_uid,
                    ROBOTS_PICKUP_HIGHLIGHT_UID | ROBOTS_PICKUP_HIGHLIGHT_GOLD_UID
                ));
                script_spawns.push(pickup_script_request(
                    key,
                    trigger.position,
                    trigger.rotation,
                    runtime,
                    script_uid,
                ));
            }

            // Native +0x0C performs its periodic block before testing collected bit0x02.
            // Once collected it returns immediately and +0x34 is no longer serviced.
            if runtime.handler.collected() {
                continue;
            }

            // +0x34 rotates only when owner +0x161 == 0. The exact periodic
            // branch above is entity-backed and therefore has no Script animator.
            if runtime.handler.flags_3d1 & ROBOTS_PICKUP_PERIODIC_FLAG != 0 {
                runtime.spin_radians = (runtime.spin_radians + ROBOTS_PICKUP_SPIN_STEP_RADIANS)
                    .rem_euclid(std::f32::consts::TAU);
            }

            // 0x00413C90 decrements the staggered cadence before any Player query.
            if runtime.handler.tick_cadence_gate() {
                continue;
            }
            let Some(player_position) = player_position else {
                continue;
            };
            let distance_squared = trigger.position.distance_squared(player_position);
            let proximity = runtime.handler.proximity_plan(
                distance_squared,
                ROBOTS_PLAYER_PICKUP_INTERACTION_RANGE_SQUARED,
                player_mode,
                definition_bit0_policy,
                preflight_result,
            );

            match proximity {
                RobotsPickupProximityPlan::Collect { .. } => {
                    // 0x00414100 emits the periodic spot effect before asking the
                    // candidate Player Handler +0x94 to commit the inventory change.
                    if let Some(request) = native_pickup_spot_effect_request(
                        key,
                        trigger.position,
                        trigger.rotation,
                        runtime,
                    ) {
                        script_spawns.push(request);
                    }

                    if commit_native_pickup_to_player(
                        &map.inventory_definitions,
                        &mut self.native_script_gameplay_state.inventory,
                        &mut self.native_player_item_state,
                        runtime.handler.pickup_uid_3c8,
                        runtime.handler.quantity_3cc,
                    ) {
                        runtime.handler.mark_collected();
                        let graph = self.native_trigger_graph.entry(key).or_default();
                        graph.pickup.suppressed_e8 = true;
                        // FUN_00414100 writes source XTrigger+0xE8=1 and, when
                        // serialized data[7] bit0 is set (runtime +0x88), arms
                        // common trigger bit0x100000 for the next TriggerManager pass.
                        if native_pickup_source_requests_deferred_output(&trigger.data) {
                            graph.deferred_fire = true;
                        }
                    }
                    // Native returns directly from the collect branch, including
                    // rejected final commits, so cadence remains zero for retry.
                }
                RobotsPickupProximityPlan::OutsideBroadRange
                | RobotsPickupProximityPlan::WaitForCollectRange { .. } => {
                    runtime.handler.finish_proximity_cadence(
                        distance_squared,
                        player_mode,
                        &mut self.native_pickup_global_scheduler,
                    );
                }
                RobotsPickupProximityPlan::InventoryRejected => {
                    // Native non-Scrap preflight rejection returns before cadence
                    // rescheduling and therefore retries on the next fixed update.
                }
            }
        }
        self.native_script_xitem_spawns.extend(script_spawns);
    }

    /// Materialize native GeneratePickupFromHash requests. Factory failure is
    /// fail-closed exactly like 0x00403F20: unsupported Pickup UIDs never become
    /// live XItems.
    pub(super) fn consume_native_dynamic_pickup_spawns(&mut self) {
        for spawn in std::mem::take(&mut self.native_dynamic_pickup_spawns) {
            if let Some(runtime) = NativeDynamicPickupRuntime::from_spawn(
                spawn,
                &mut self.native_pickup_global_scheduler,
            ) {
                self.native_dynamic_pickups.push(runtime);
            }
        }
    }

    /// Service gameplay-created XItemHandler_Pickup owners. These share the
    /// process-global Pickup cadence/ordinal state with serialized map Pickups,
    /// but have no source XTrigger lifecycle to forge.
    pub(super) fn advance_native_dynamic_pickups_fixed(
        &mut self,
        map: &ProcessedMap,
        player_position: Option<Vec3>,
    ) {
        let player_mode = self.native_game_control_runtime.mode_50f;
        let mut retained = Vec::with_capacity(self.native_dynamic_pickups.len());
        let mut script_spawns = Vec::new();

        for mut runtime in std::mem::take(&mut self.native_dynamic_pickups) {
            if runtime.pending_destroy {
                continue;
            }

            let periodic = runtime.handler.service_periodic(
                runtime.native_runtime_type,
                false,
                &mut self.native_pickup_global_scheduler,
            );
            if let Some(script_uid) = periodic.play_pickup_sound_uid {
                debug_assert!(matches!(
                    script_uid,
                    ROBOTS_PICKUP_HIGHLIGHT_UID | ROBOTS_PICKUP_HIGHLIGHT_GOLD_UID
                ));
                script_spawns.push(runtime.effect_script_request(script_uid));
            }

            // +0x0C checks collected before +0x34 is entered.
            if runtime.handler.collected() {
                retained.push(runtime);
                continue;
            }

            // GeneratePickupFromHash explicitly seeds owner +0x161 = 0, so the
            // -2 degree owner rotation is active for this class from its first
            // +0x34 service. Lifetime bit0x01 is also installed by the factory.
            let lifetime = runtime
                .handler
                .advance_lifetime(ROBOTS_FIXED_STEP_SECONDS, true);
            runtime.spin_radians = (runtime.spin_radians + lifetime.spin_delta_radians)
                .rem_euclid(std::f32::consts::TAU);
            if let Some(fade_scalar) = lifetime.fade_scalar {
                runtime.fade_scalar = fade_scalar;
            }

            if lifetime.collect_on_expiry {
                // 0x00414210 calls 0x00414100(handler, 0, 0): no Player
                // inventory mutation occurs. Periodic owners request teardown
                // and spawn their spot Script before Handler bit0x02 is set.
                if let Some(request) = runtime.spot_effect_request() {
                    script_spawns.push(request);
                    runtime.pending_destroy = true;
                }
                runtime.handler.mark_collected();
                if !runtime.pending_destroy {
                    retained.push(runtime);
                }
                continue;
            }
            if lifetime.destroy_on_expiry {
                // Handler +0x90 teardown. Physical ownership ends at this host
                // boundary; there is no serialized trigger lifecycle to retain.
                continue;
            }

            if runtime.handler.tick_cadence_gate() {
                retained.push(runtime);
                continue;
            }
            let Some(player_position) = player_position else {
                retained.push(runtime);
                continue;
            };
            let distance_squared = runtime.spawn.position.distance_squared(player_position);
            let (definition_bit0_policy, preflight_result) = native_pickup_player_preflight(
                &map.inventory_definitions,
                &self.native_script_gameplay_state.inventory,
                &self.native_player_item_state,
                runtime.handler.pickup_uid_3c8,
                runtime.handler.quantity_3cc,
            );
            let proximity = runtime.handler.proximity_plan(
                distance_squared,
                ROBOTS_PLAYER_PICKUP_INTERACTION_RANGE_SQUARED,
                player_mode,
                definition_bit0_policy,
                preflight_result,
            );

            match proximity {
                RobotsPickupProximityPlan::Collect { .. } => {
                    if let Some(request) = runtime.spot_effect_request() {
                        script_spawns.push(request);
                        // Periodic 0x00414100 requests owner teardown before
                        // the final Player +0x94 inventory commit.
                        runtime.pending_destroy = true;
                    }
                    if commit_native_pickup_to_player(
                        &map.inventory_definitions,
                        &mut self.native_script_gameplay_state.inventory,
                        &mut self.native_player_item_state,
                        runtime.handler.pickup_uid_3c8,
                        runtime.handler.quantity_3cc,
                    ) {
                        runtime.handler.mark_collected();
                    }
                }
                RobotsPickupProximityPlan::OutsideBroadRange
                | RobotsPickupProximityPlan::WaitForCollectRange { .. } => {
                    runtime.handler.finish_proximity_cadence(
                        distance_squared,
                        player_mode,
                        &mut self.native_pickup_global_scheduler,
                    );
                }
                RobotsPickupProximityPlan::InventoryRejected => {}
            }

            if !runtime.pending_destroy {
                retained.push(runtime);
            }
        }

        self.native_dynamic_pickups = retained;
        self.native_script_xitem_spawns.extend(script_spawns);
    }

    /// Native 0x00444E0A: after all Pickup candidates registered this frame,
    /// choose the ordinal for the next frame with exactly one shared gameplay RNG
    /// draw. Unknown RNG provenance remains fail-closed.
    pub(super) fn finish_native_pickup_global_scheduler_fixed(
        &mut self,
        shared_rng_order_known: bool,
    ) -> bool {
        let candidate_count = self
            .native_pickup_global_scheduler
            .periodic_candidate_count_b8;
        let random_u32 = if candidate_count == 0 {
            None
        } else if shared_rng_order_known {
            self.native_global_gameplay_rng.next_u32()
        } else {
            None
        };
        self.native_pickup_global_scheduler
            .finish_fixed_update(random_u32)
            || candidate_count == 0
    }

    pub(super) fn native_pickup_runtime_rotation(
        &self,
        map: &ProcessedMap,
        trigger_index: usize,
    ) -> Option<Quat> {
        let trigger = map.triggers.get(trigger_index)?;
        let key = Self::runtime_event_key(map.hashcode, trigger_index);
        let runtime = self.native_pickup_xitems.get(&key)?;
        Some(native_pickup_owner_rotation(trigger.rotation, runtime))
    }

    pub(super) fn native_pickup_runtime_live(&self, map_hash: u32, trigger_index: usize) -> bool {
        let key = Self::runtime_event_key(map_hash, trigger_index);
        self.native_pickup_trigger_lifecycle
            .get(&key)
            .is_some_and(|state| state.xitem_exists)
            && self
                .native_pickup_xitems
                .get(&key)
                .is_some_and(|runtime| !runtime.handler.collected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eurochef_shared::robots_runtime::pickup::{
        RobotsPickupSpawnLayout, ROBOTS_PICKUP_DEFAULT_REGISTRATION_FLAGS,
    };

    fn spawn(native_runtime_type: u32, spin: f32) -> RobotsPickupSpawnPlan {
        RobotsPickupSpawnPlan {
            layout: RobotsPickupSpawnLayout {
                serialized_trigger_type: if native_runtime_type == 0x2D {
                    0x1B
                } else {
                    0x19
                },
                native_runtime_type,
                pickup_uid: if native_runtime_type == 0x2D {
                    0x4700_0002
                } else {
                    0x4700_0001
                },
                handler_datum_uid: if native_runtime_type == 0x2D {
                    0x4700_0002
                } else {
                    0x4700_0001
                },
                quantity: 1,
                registration_flags: ROBOTS_PICKUP_DEFAULT_REGISTRATION_FLAGS,
            },
            initial_spin_radians: spin,
        }
    }

    #[test]
    fn serialized_entity_pickup_installs_periodic_flag_and_exact_initial_spin() {
        let mut global = RobotsPickupGlobalSchedulerState::default();
        let runtime =
            native_pickup_xitem_from_spawn(0x19, &[Some(50)], 0, spawn(0x2C, 1.25), &mut global);
        assert_ne!(runtime.handler.flags_3d1 & ROBOTS_PICKUP_PERIODIC_FLAG, 0);
        assert_eq!(runtime.spin_radians, 1.25);
    }

    #[test]
    fn periodic_service_uses_global_ordinal_and_native_highlight_selector() {
        let mut global = RobotsPickupGlobalSchedulerState::default();
        global.selected_periodic_ordinal_bc = 0;
        global.pickup_sound_cooldown_c0 = 0;

        let mut scrap = RobotsPickupHandlerRuntimeState::new(0x4700_0001, 1, &mut global);
        scrap.flags_3d1 |= ROBOTS_PICKUP_PERIODIC_FLAG;
        scrap.periodic_sound_cooldown_3d3 = 0;
        global.begin_fixed_update();
        let ordinary = scrap.service_periodic(0x2C, false, &mut global);
        assert_eq!(ordinary.candidate_ordinal, Some(0));
        assert_eq!(
            ordinary.play_pickup_sound_uid,
            Some(ROBOTS_PICKUP_HIGHLIGHT_UID)
        );
        assert_eq!(global.periodic_candidate_count_b8, 1);

        // The next frame selects the same first ordinal, but the Golden Scrap
        // runtime type chooses the alternate native highlight Script.
        global.selected_periodic_ordinal_bc = 0;
        global.pickup_sound_cooldown_c0 = 0;
        let mut gold = RobotsPickupHandlerRuntimeState::new(0x4700_0002, 1, &mut global);
        gold.flags_3d1 |= ROBOTS_PICKUP_PERIODIC_FLAG;
        gold.periodic_sound_cooldown_3d3 = 0;
        global.begin_fixed_update();
        let golden = gold.service_periodic(0x2D, false, &mut global);
        assert_eq!(golden.candidate_ordinal, Some(0));
        assert_eq!(
            golden.play_pickup_sound_uid,
            Some(ROBOTS_PICKUP_HIGHLIGHT_GOLD_UID)
        );
        assert_eq!(global.periodic_candidate_count_b8, 1);
    }

    #[test]
    fn pickup_effect_selector_preserves_gold_runtime_type_and_owner_transform() {
        let mut global = RobotsPickupGlobalSchedulerState::default();
        let runtime =
            native_pickup_xitem_from_spawn(0x1B, &[Some(1)], 0, spawn(0x2D, 0.5), &mut global);
        let position = Vec3::new(1.0, 2.0, 3.0);
        let rotation = Vec3::ZERO;
        let request = native_pickup_spot_effect_request(7, position, rotation, &runtime)
            .expect("entity-backed pickup owns spot effect");
        assert_eq!(request.file_uid, ROBOTS_PICKUP_EFFECT_FILE_UID);
        assert_eq!(request.script_uid, ROBOTS_PICKUP_SPOT_GOLD_UID);
        assert_eq!(request.position_xyzw, [1.0, 2.0, 3.0, 0.0]);
        assert_eq!(
            request.rotation,
            native_pickup_owner_rotation(rotation, &runtime)
        );
    }

    #[test]
    fn player_pickup_commit_routes_47_inventory_and_48_player_items() {
        let definitions = [RobotsInventoryDefinition {
            uid: 0x4700_0001,
            words_04_to_14: [0; 5],
            target_word_18: 0,
        }];
        let mut inventory = RobotsInventoryState::default();
        let mut player_items = RobotsPlayerItemState::default();

        let (policy, preflight) =
            native_pickup_player_preflight(&definitions, &inventory, &player_items, 0x4700_0001, 5);
        assert!(!policy);
        assert_eq!(preflight, Some(1));
        assert!(commit_native_pickup_to_player(
            &definitions,
            &mut inventory,
            &mut player_items,
            0x4700_0001,
            5,
        ));
        assert_eq!(inventory.stored_current(0x4700_0001), 5);

        let (policy, preflight) =
            native_pickup_player_preflight(&definitions, &inventory, &player_items, 0x4800_000E, 1);
        assert!(!policy);
        assert_eq!(preflight, Some(4));
        assert!(commit_native_pickup_to_player(
            &definitions,
            &mut inventory,
            &mut player_items,
            0x4800_000E,
            1,
        ));
        assert!(player_items.is_enabled(0x4800_000E));
    }

    #[test]
    fn definition_bit0_uses_native_policy_path_and_bitset_commit() {
        let definitions = [RobotsInventoryDefinition {
            uid: 0x4700_0010,
            words_04_to_14: [0, 0, 0, 0, 1],
            target_word_18: 8,
        }];
        let mut inventory = RobotsInventoryState::default();
        let mut player_items = RobotsPlayerItemState::default();

        let (policy, preflight) =
            native_pickup_player_preflight(&definitions, &inventory, &player_items, 0x4700_0010, 2);
        assert!(policy);
        assert_eq!(preflight, None);
        assert!(commit_native_pickup_to_player(
            &definitions,
            &mut inventory,
            &mut player_items,
            0x4700_0010,
            2,
        ));
        assert_eq!(inventory.stored_current(0x4700_0010), 1 << 2);
    }

    #[test]
    fn dynamic_pickup_factory_installs_native_lifetime_argument_and_periodic_resource_bit() {
        let mut global = RobotsPickupGlobalSchedulerState::default();
        let scrap = NativeDynamicPickupRuntime::from_spawn(
            NativeDynamicPickupSpawnRequest {
                source_owner_key: 9,
                resource_file_uid: 0x0100_0003,
                resource_uid: 0x0400_0024,
                pickup_uid: 0x4700_0001,
                owner_datum_264: 3,
                position: Vec3::new(1.0, 2.0, 3.0),
                rotation: Quat::IDENTITY,
            },
            &mut global,
        )
        .expect("normal generated Scrap is accepted");
        assert_eq!(scrap.native_runtime_type, 0x2C);
        assert_eq!(scrap.handler.quantity_3cc, 0x400);
        assert_ne!(scrap.handler.flags_3d1 & ROBOTS_PICKUP_LIFETIME_FLAG, 0);
        assert_eq!(scrap.handler.flags_3d1 & ROBOTS_PICKUP_PERIODIC_FLAG, 0);

        let gold = NativeDynamicPickupRuntime::from_spawn(
            NativeDynamicPickupSpawnRequest {
                source_owner_key: 10,
                resource_file_uid: ROBOTS_PICKUP_EFFECT_FILE_UID,
                resource_uid: 0x0200_0001,
                pickup_uid: 0x4700_0002,
                owner_datum_264: 1,
                position: Vec3::ZERO,
                rotation: Quat::IDENTITY,
            },
            &mut global,
        )
        .expect("generated Golden Scrap is accepted");
        assert_eq!(gold.native_runtime_type, 0x2D);
        assert_eq!(gold.handler.quantity_3cc, 0);
        assert_ne!(gold.handler.flags_3d1 & ROBOTS_PICKUP_PERIODIC_FLAG, 0);
        assert_eq!(
            gold.spot_effect_request()
                .expect("periodic gold spot")
                .script_uid,
            ROBOTS_PICKUP_SPOT_GOLD_UID
        );

        assert!(NativeDynamicPickupRuntime::from_spawn(
            NativeDynamicPickupSpawnRequest {
                source_owner_key: 11,
                resource_file_uid: 0x0100_0003,
                resource_uid: 0x0400_0024,
                pickup_uid: 0x4700_002B,
                owner_datum_264: 1,
                position: Vec3::ZERO,
                rotation: Quat::IDENTITY,
            },
            &mut global,
        )
        .is_none());
    }

    #[test]
    fn dynamic_pickup_lifetime_reaches_native_autocollect_after_five_seconds() {
        let mut global = RobotsPickupGlobalSchedulerState::default();
        let mut runtime = NativeDynamicPickupRuntime::from_spawn(
            NativeDynamicPickupSpawnRequest {
                source_owner_key: 12,
                resource_file_uid: 0x0100_0003,
                resource_uid: 0x0400_0024,
                pickup_uid: 0x4700_0001,
                owner_datum_264: 1,
                position: Vec3::ZERO,
                rotation: Quat::IDENTITY,
            },
            &mut global,
        )
        .unwrap();

        let mut expired = false;
        for _ in 0..=301 {
            let step = runtime
                .handler
                .advance_lifetime(ROBOTS_FIXED_STEP_SECONDS, true);
            runtime.spin_radians += step.spin_delta_radians;
            if let Some(fade) = step.fade_scalar {
                runtime.fade_scalar = fade;
            }
            if step.collect_on_expiry {
                expired = true;
                runtime.handler.mark_collected();
                break;
            }
        }
        assert!(expired);
        assert!(runtime.handler.collected());
        assert_eq!(runtime.fade_scalar, 0.0);
    }

    #[test]
    fn pickup_source_deferred_output_is_exactly_data7_bit0() {
        let mut data = vec![None; 8];
        assert!(!native_pickup_source_requests_deferred_output(&data));
        data[7] = Some(2);
        assert!(!native_pickup_source_requests_deferred_output(&data));
        data[7] = Some(1);
        assert!(native_pickup_source_requests_deferred_output(&data));
        data[7] = Some(3);
        assert!(native_pickup_source_requests_deferred_output(&data));
    }
}
