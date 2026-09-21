use super::{runtime_ai::horizontal_yaw, MapFrame};
use crate::{
    map_runtime::{
        runtime_map_projectile_static_world_contact_detail, RuntimeCharacterDatumWorldTransform,
        RuntimeCharacterWorldShape, RuntimeProjectileStaticWorldContact,
        RuntimeRobotsGlobalRngState,
    },
    maps::{ProcessedCharacterVisual, ProcessedMap},
};
use eurochef_shared::robots_runtime::{
    explosion::{
        robots_explosion_fragment_advance_visibility,
        robots_explosion_fragment_allocate_cadence_phase,
        robots_explosion_fragment_apply_contact_rebound, robots_explosion_fragment_ballistic_plan,
        robots_explosion_fragment_cadence_due, robots_explosion_fragment_generation_position,
        robots_explosion_fragment_hit_query_plan, robots_explosion_fragment_hit_query_speed_gate,
        robots_explosion_fragment_teardown_emits_pickup, robots_explosion_fragment_uniform_scale,
        robots_explosion_fragment_vertical_service, RobotsExplosionCollisionProfile,
        RobotsExplosionCollisionRuntimeState, RobotsExplosionDatabase, RobotsExplosionDefinition,
        RobotsExplosionFragmentBallisticPlan, RobotsExplosionFragmentDefinition,
        RobotsExplosionFragmentVerticalService, RobotsExplosionFragmentVisibilityState,
        RobotsExplosionRuntimeState, ROBOTS_EXPLOSION_FRAGMENT_ANIM_DATUM,
        ROBOTS_EXPLOSION_FRAGMENT_HIT_QUERY_FLAG, ROBOTS_EXPLOSION_FRAGMENT_HIT_QUERY_RADIUS,
        ROBOTS_EXPLOSION_FRAGMENT_MAX_HANDLER_UPDATES,
        ROBOTS_EXPLOSION_FRAGMENT_RUNTIME_PLAYER_PICKUP_LATCH,
        ROBOTS_EXPLOSION_FRAGMENT_RUNTIME_TEARDOWN_FLAG, ROBOTS_EXPLOSION_GENERATION_ANIM_DATUM,
    },
    hit_candidate_policy::RobotsHitQueryCandidateContext,
    hit_query::{robots_hit_query_allocate_serial, RobotsHitQueryState},
    hit_reaction::robots_monster_action_explosion_uid,
    hit_shapes::RobotsHitShape,
    inventory::{RobotsInventoryDefinition, RobotsInventoryState},
    pickup::{
        RobotsPickupAttractPhysicsState, ROBOTS_PICKUP_ATTRACT_COLLECT_RANGE_SQUARED,
        ROBOTS_PICKUP_ATTRACT_DEFAULT_RANGE, ROBOTS_PICKUP_ATTRACT_TARGET_OFFSET_XYZ,
    },
    projectile::RobotsProjectilePhysicsRuntimeState,
};
use glam::Vec3;

/// UE-facing spawn request for native `0x004DC510` explosion creation.
///
/// This deliberately remains the producer boundary. The shared runtime owns the
/// proven `XItemHandler_Explosion` definition/fragment scheduler; this host resolves
/// shipped FX03 resources while common HitQuery/candidate reaction and child-projectile
/// physics remain producer-neutral engine seams suitable for the UE5.8 adapter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct NativeAiExplosionSpawnRequest {
    pub(super) source_body_key: u64,
    pub(super) explosion_uid: u32,
    pub(super) position: Vec3,
}

impl NativeAiExplosionSpawnRequest {
    pub(super) const fn new(source_body_key: u64, explosion_uid: u32, position: Vec3) -> Self {
        Self {
            source_body_key,
            explosion_uid,
            position,
        }
    }
}

/// Live common `XItemHandler_Explosion` state. AI classes only create the request;
/// all definition/cursor/cadence ownership stays here so mines, missiles and bosses
/// can later feed the same runtime boundary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct NativeAiExplosionMainSpawnRequest {
    pub(super) source_body_key: u64,
    pub(super) explosion_uid: u32,
    pub(super) position: Vec3,
    /// Source XItem `HT_AnimDatum_ExplosionGeneration (0x1000002C)` resolved through
    /// the same animated-datum path as other gameplay attachment points.
    pub(super) generation_transform: Option<RuntimeCharacterDatumWorldTransform>,
    /// FX03 definition +0x04, passed by `0x004DC510` to the source XItem factory.
    pub(super) script_uid: u32,
    /// FX03 definition +0x08 companion EDB/file UID.
    pub(super) file_uid: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct NativeAiExplosionRuntime {
    pub(super) source_body_key: u64,
    pub(super) explosion_uid: u32,
    pub(super) position: Vec3,
    pub(super) definition: RobotsExplosionDefinition,
    /// Source XItem `HT_AnimDatum_ExplosionFragmentGeneration (0x1000002D)`.
    /// Shipped corpus is 37/37 sphere mode; `None` selects native +0.2 Y fallback.
    pub(super) fragment_generation_shape: Option<RuntimeCharacterWorldShape>,
    /// Exact shared query state initialized by `0x00425A70`. Geometry/candidate
    /// traversal remains a host concern; selector/flags/budget/serial are native.
    pub(super) hit_query: Option<RobotsHitQueryState>,
    /// Direct main-Script opcode13 Collision provider, if the shipped FX03 Script
    /// actually creates one. Selector 0x10000001 alone does not imply geometry.
    pub(super) collision_profile: Option<RobotsExplosionCollisionProfile>,
    /// Handler runs before attached Script animator, so this state owns the exact
    /// one-tick visibility edge between Collision creation/update and HitQuery.
    pub(super) collision_runtime: RobotsExplosionCollisionRuntimeState,
    /// Retained source yaw for common AI hit-direction selection after the original
    /// monster XItem has already entered deferred destruction.
    pub(super) source_yaw_radians: Option<f32>,
    /// Separate serial written to Handler+0x380 by `0x004DC709` after the embedded
    /// HitQuery initializer has already consumed its own +0x90 serial.
    pub(super) handler_query_serial_380: Option<u16>,
    pub(super) state: RobotsExplosionRuntimeState,
}

/// Child-XItem factory request emitted by native `0x004DCC20`. This is not a particle
/// request: FX03 sheet 1 drives a real XItem whose downstream physics uses
/// `XItemPhysics_Projectile`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct NativeAiExplosionFragmentSpawnRequest {
    pub(super) source_body_key: u64,
    pub(super) explosion_uid: u32,
    pub(super) ordinal: u8,
    pub(super) fragment_uid: u32,
    pub(super) position: Vec3,
    pub(super) definition: RobotsExplosionFragmentDefinition,
    /// Native flag 0x200 uniform XYZ child scale. Ordinary rows remain 1.0.
    pub(super) uniform_scale: f32,
    /// Native `(flags & 0x84)==4` gameplay draw after shipped pickup-table resolution.
    /// The downstream resource selector is still a separate host/render concern.
    pub(super) pickup_variant_unit_draw: Option<f32>,
    /// Exact ordinary shipped process-LCG trajectory randomization. `None` means
    /// the process seed is unknown or the unshipped directional branch was requested.
    pub(super) ballistic_plan: Option<RobotsExplosionFragmentBallisticPlan>,
    /// Exact initial `XItemPhysics_Projectile` state when the shipped ballistic branch
    /// is representable. Collision/contact remains owned by the common host layer.
    pub(super) physics: Option<RobotsProjectilePhysicsRuntimeState>,
}

/// Dynamic Pickup/resource spawn emitted by generic fragment teardown `0x004DBB20`.
/// This deliberately does not pretend to be a serialized XTrigger_Pickup: UE/host
/// owns the spawned XItem/Actor while the request preserves native UID/quantity/position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct NativeAiExplosionFragmentPickupSpawnRequest {
    pub(super) source_body_key: u64,
    pub(super) explosion_uid: u32,
    pub(super) fragment_uid: u32,
    pub(super) pickup_uid: u32,
    pub(super) quantity: i32,
    pub(super) position: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct NativeAiExplosionFragmentRuntime {
    pub(super) source_body_key: u64,
    pub(super) explosion_uid: u32,
    pub(super) ordinal: u8,
    pub(super) fragment_uid: u32,
    pub(super) definition: RobotsExplosionFragmentDefinition,
    pub(super) uniform_scale: f32,
    pub(super) pickup_variant_unit_draw: Option<f32>,
    pub(super) physics: RobotsProjectilePhysicsRuntimeState,
    /// `0x004DB190` replaces Projectile Physics with `XItemPhysics_PickupAttract`.
    /// Keeping this as an optional replacement lane avoids inventing a parallel
    /// world-physics abstraction while preserving the exact class switch.
    pub(super) pickup_attract_physics: Option<RobotsPickupAttractPhysicsState>,
    /// Native Handler+0x3D4..+0x3DC spawn position retained for 0x004DC0F0 fall checks.
    pub(super) initial_position: Vec3,
    /// Native Handler+0x48C stagger phase allocated from process-global byte 0x007B3248.
    pub(super) cadence_phase_48c: u8,
    /// Mutable native Handler+0x3C0. Low bits originate in FX03; high runtime bits
    /// are subsequently written by contact/fade/pickup/teardown services.
    pub(super) handler_flags: u32,
    /// Native XItem+0x258/+0x25C visibility pair serviced by `0x004DBCC0`.
    pub(super) visibility: RobotsExplosionFragmentVisibilityState,
    /// Manual first global serial consumed at `0x004DD1D7` and copied to Handler+0x478.
    pub(super) handler_query_serial_478: u16,
    /// Embedded Handler+0x3E8 common HitQuery. `0x00425A70` consumes the following
    /// global serial, so the two serial allocations must remain adjacent and ordered.
    pub(super) hit_query: RobotsHitQueryState,
    /// Native Handler+0x3E4, incremented by `0x004DB6B0` after the earlier Physics pass.
    pub(super) handler_updates_completed: u32,
    /// Native Handler+0x480, incremented by `0x004DB8B0` after a usable rebound normal.
    /// The later opacity helper consumes this count, so retain it even before that lane
    /// is fully exposed to rendering.
    pub(super) physical_contact_responses_completed: u32,
    /// Mirrors the terminal `0x00443EE0` request; physical removal stays end-of-tick.
    pub(super) pending_destroy: bool,
}

impl NativeAiExplosionFragmentRuntime {
    fn from_spawn(
        request: NativeAiExplosionFragmentSpawnRequest,
        cadence_phase_48c: u8,
        handler_query_serial_478: u16,
        hit_query_serial: u16,
    ) -> Option<Self> {
        let definition = request.definition;
        Some(Self {
            source_body_key: request.source_body_key,
            explosion_uid: request.explosion_uid,
            ordinal: request.ordinal,
            fragment_uid: request.fragment_uid,
            definition,
            uniform_scale: request.uniform_scale,
            pickup_variant_unit_draw: request.pickup_variant_unit_draw,
            physics: request.physics?,
            pickup_attract_physics: None,
            initial_position: request.position,
            cadence_phase_48c,
            handler_flags: definition.flags,
            visibility: RobotsExplosionFragmentVisibilityState::default(),
            handler_query_serial_478,
            hit_query: robots_explosion_fragment_hit_query_plan(definition).instantiate(
                true,
                true,
                hit_query_serial,
            ),
            handler_updates_completed: 0,
            physical_contact_responses_completed: 0,
            pending_destroy: false,
        })
    }

    pub(super) fn position(&self) -> Vec3 {
        self.pickup_attract_physics
            .map(|physics| Vec3::from_array(physics.position_xyz))
            .unwrap_or_else(|| Vec3::from_array(self.physics.position_xyz))
    }

    fn hit_query_shape(&self) -> RuntimeCharacterWorldShape {
        RobotsHitShape::Sphere {
            center_xyz: self.position().to_array(),
            radius: ROBOTS_EXPLOSION_FRAGMENT_HIT_QUERY_RADIUS,
        }
    }

    fn should_service_hit_query(&self) -> bool {
        !self.pending_destroy
            && self.pickup_attract_physics.is_none()
            && self.handler_flags & ROBOTS_EXPLOSION_FRAGMENT_HIT_QUERY_FLAG != 0
            && robots_explosion_fragment_hit_query_speed_gate(self.physics)
    }

    fn teardown_with_pickup_position(
        &mut self,
        pickup_position: Option<Vec3>,
    ) -> Option<NativeAiExplosionFragmentPickupSpawnRequest> {
        if self.pending_destroy {
            return None;
        }
        let pickup = pickup_position
            .filter(|_| self.definition.pickup_uid != u32::MAX)
            .map(|position| NativeAiExplosionFragmentPickupSpawnRequest {
                source_body_key: self.source_body_key,
                explosion_uid: self.explosion_uid,
                fragment_uid: self.fragment_uid,
                pickup_uid: self.definition.pickup_uid,
                quantity: i32::from(self.definition.pickup_quantity()),
                position,
            });
        self.handler_flags |= ROBOTS_EXPLOSION_FRAGMENT_RUNTIME_TEARDOWN_FLAG;
        self.pending_destroy = true;
        pickup
    }

    fn generic_teardown(&mut self) -> Option<NativeAiExplosionFragmentPickupSpawnRequest> {
        let pickup_position = robots_explosion_fragment_teardown_emits_pickup(self.handler_flags)
            .then(|| self.position());
        self.teardown_with_pickup_position(pickup_position)
    }

    fn apply_static_world_contact(
        &mut self,
        contact: RuntimeProjectileStaticWorldContact,
    ) -> Option<NativeAiExplosionFragmentPickupSpawnRequest> {
        if self.pending_destroy || contact.contact_bits & 0x0001 == 0 {
            return None;
        }
        let rebound = contact.normal.map(|normal| {
            robots_explosion_fragment_apply_contact_rebound(
                self.handler_flags,
                &mut self.physics,
                normal.to_array(),
            )
        });
        if rebound.is_some_and(|step| step.rebound_applied) {
            self.physical_contact_responses_completed =
                self.physical_contact_responses_completed.saturating_add(1);
        }
        if !rebound.is_some_and(|step| step.motion_survives_contact)
            && self.handler_flags & ROBOTS_EXPLOSION_FRAGMENT_RUNTIME_PLAYER_PICKUP_LATCH == 0
        {
            return self.generic_teardown();
        }
        None
    }

    fn advance_physics_fixed(&mut self, player_position: Vec3, game_control_mode: u8) {
        if self.pending_destroy {
            return;
        }
        if let Some(physics) = self.pickup_attract_physics.as_mut() {
            let _ = physics.advance_fixed(player_position.to_array(), game_control_mode, 1.0);
            return;
        }
        self.physics.advance_fixed(1.0, false);
    }

    fn service_player_pickup_tail(
        &mut self,
        player_position: Vec3,
        definition: Option<RobotsInventoryDefinition>,
        inventory: &mut RobotsInventoryState,
    ) -> bool {
        if self.pending_destroy
            || self.handler_updates_completed <= 30
            || !self.definition.uses_script_state_markers()
        {
            return false;
        }

        if self.handler_flags & ROBOTS_EXPLOSION_FRAGMENT_RUNTIME_PLAYER_PICKUP_LATCH == 0 {
            let eligible = definition.is_some_and(RobotsInventoryDefinition::is_bitset)
                || matches!(
                    inventory
                        .preview_add(definition, i32::from(self.definition.pickup_quantity()))
                        .native_result_code,
                    1 | 4
                );
            if !eligible {
                return false;
            }
            let target =
                player_position + Vec3::from_array(ROBOTS_PICKUP_ATTRACT_TARGET_OFFSET_XYZ);
            if self.position().distance_squared(target)
                > ROBOTS_PICKUP_ATTRACT_DEFAULT_RANGE * ROBOTS_PICKUP_ATTRACT_DEFAULT_RANGE
            {
                return false;
            }
            self.pickup_attract_physics = Some(RobotsPickupAttractPhysicsState::new(
                self.position().to_array(),
            ));
            self.handler_flags |= ROBOTS_EXPLOSION_FRAGMENT_RUNTIME_PLAYER_PICKUP_LATCH;
        }

        if self.handler_flags
            & eurochef_shared::robots_runtime::explosion::ROBOTS_EXPLOSION_FRAGMENT_RUNTIME_PICKUP_HANDLED_FLAG
            != 0
        {
            return false;
        }
        let Some(physics) = self.pickup_attract_physics else {
            return false;
        };
        let Some(distance_squared) = physics.target_distance_squared(player_position.to_array())
        else {
            return false;
        };
        if distance_squared >= ROBOTS_PICKUP_ATTRACT_COLLECT_RANGE_SQUARED {
            return false;
        }

        let _ = inventory.add(definition, i32::from(self.definition.pickup_quantity()));
        self.handler_flags |=
            eurochef_shared::robots_runtime::explosion::ROBOTS_EXPLOSION_FRAGMENT_RUNTIME_PICKUP_HANDLED_FLAG;
        // All three shipped FX03 pickup-fragment resources (Normal/Golden/Silver Scrap)
        // are absent from the real o01_pick UXGeoScript table, so 0x004DBF90 takes its
        // no-EXItemAnimator_Script branch: request XItem teardown, then spawn 0x040002F9.
        // The visual effect remains a presentation seam; gameplay lifetime ends here.
        self.pending_destroy = true;
        true
    }

    fn advance_handler_fixed(
        &mut self,
        engine_frame_counter: u32,
    ) -> Option<NativeAiExplosionFragmentPickupSpawnRequest> {
        if self.pending_destroy {
            return None;
        }
        self.handler_updates_completed = self.handler_updates_completed.saturating_add(1);
        if self.handler_updates_completed >= ROBOTS_EXPLOSION_FRAGMENT_MAX_HANDLER_UPDATES {
            return self.generic_teardown();
        }

        if robots_explosion_fragment_cadence_due(engine_frame_counter, self.cadence_phase_48c) {
            match robots_explosion_fragment_vertical_service(
                self.handler_flags,
                self.initial_position.to_array(),
                self.position().to_array(),
            ) {
                RobotsExplosionFragmentVerticalService::Continue => {}
                RobotsExplosionFragmentVerticalService::Teardown {
                    pickup_position_xyz,
                } => {
                    return self
                        .teardown_with_pickup_position(pickup_position_xyz.map(Vec3::from_array));
                }
            }
        }

        // Native 0x004DBCC0 runs after contact/cadence service and before the explicit
        // fragment HitQuery in this same Handler tick.
        let visibility = robots_explosion_fragment_advance_visibility(
            &mut self.handler_flags,
            self.physical_contact_responses_completed,
            &mut self.visibility,
            1.0,
        );
        if !visibility.survives {
            return self.generic_teardown();
        }
        None
    }

    fn advance_fixed(&mut self) {
        // Test/helper wrapper preserving the native Physics -> Handler order. Runtime
        // hosting calls the split methods so world contact can run between them. Frame
        // value 1 deliberately misses the default test phase 0.
        self.advance_physics_fixed(Vec3::ZERO, 0);
        let _ = self.advance_handler_fixed(1);
    }
}

fn native_explosion_hit_query_state(
    definition: RobotsExplosionDefinition,
    serial: u16,
) -> Option<RobotsHitQueryState> {
    (definition.hit_query_selector() != u32::MAX)
        .then(|| definition.hit_query_plan().instantiate(true, true, serial))
}

/// Active explosion query setup consumes two consecutive `DAT_00616DA8` serials:
/// first the embedded HitQuery+0x90 in `0x00425A70`, then Handler+0x380 in
/// `0x004DC709`. Invalid selector consumes neither.
fn native_explosion_query_serials(
    next_serial: &mut u16,
    definition: RobotsExplosionDefinition,
) -> Option<(u16, u16)> {
    if definition.hit_query_selector() == u32::MAX {
        return None;
    }
    Some((
        robots_hit_query_allocate_serial(next_serial),
        robots_hit_query_allocate_serial(next_serial),
    ))
}

fn native_explosion_main_position(
    fallback_position: Vec3,
    generation_transform: Option<RuntimeCharacterDatumWorldTransform>,
) -> Vec3 {
    generation_transform
        .map(|transform| transform.position)
        .unwrap_or(fallback_position)
}

fn native_explosion_hit_query_shape(
    runtime: &NativeAiExplosionRuntime,
) -> Option<RuntimeCharacterWorldShape> {
    let profile = runtime.collision_profile.as_ref()?;
    // Shipped FX03 opcode-13 corpus is 12/12 mode1 sphere. Keep future mode3 data
    // fail-closed until its exact Script-Collision representation is needed.
    if profile.datum_shape_mode != 1 {
        return None;
    }
    let sample = runtime.collision_runtime.handler_sample(profile)?;
    let radius = sample.shape_scalar_xyz[0];
    if !radius.is_finite() || radius <= 0.001 {
        return None;
    }
    let local_center = Vec3::from_array(sample.local_center_xyz);
    let center = runtime.position + local_center;
    Some(RobotsHitShape::Sphere {
        center_xyz: center.to_array(),
        radius,
    })
}

fn native_explosion_fragment_spawn_position(
    runtime: &NativeAiExplosionRuntime,
    gameplay_rng: &mut RuntimeRobotsGlobalRngState,
) -> Option<Vec3> {
    match runtime.fragment_generation_shape {
        Some(RobotsHitShape::Sphere { center_xyz, radius }) => {
            let draws = [
                gameplay_rng.next_unit_f32()?,
                gameplay_rng.next_unit_f32()?,
                gameplay_rng.next_unit_f32()?,
            ];
            robots_explosion_fragment_generation_position(center_xyz, radius, draws)
                .map(Vec3::from_array)
        }
        // Shipped corpus is 37/37 sphere mode. Refuse a future capsule datum until
        // its exact `0x004DC6A0 -> 0x004DCCE0` representation is proven.
        Some(RobotsHitShape::Capsule { .. }) => None,
        // Native `0x004DCE98` adds 0.2 to the parent explosion Y when the source
        // XItem has no HT_AnimDatum_ExplosionFragmentGeneration.
        None => Some(runtime.position + Vec3::new(0.0, 0.2, 0.0)),
    }
}

fn native_explosion_take_fragment_request(
    database: &RobotsExplosionDatabase,
    runtime: &mut NativeAiExplosionRuntime,
    gameplay_rng: &mut RuntimeRobotsGlobalRngState,
    process_lcg_seed: &mut Option<u32>,
) -> Option<NativeAiExplosionFragmentSpawnRequest> {
    let attempt = runtime.state.take_fragment_attempt(runtime.definition)?;
    let Some(definition) = database.fragment(attempt.fragment_uid).copied() else {
        // `0x004DCC20` already advanced Handler+0x4B8 before fragment lookup.
        // A missing/-1 row returns false with the output byte set, which arms
        // Handler+0x4E8 so the next service continues immediately at the next slot.
        runtime.state.record_fragment_factory_result(false);
        return None;
    };
    runtime.state.record_fragment_factory_result(true);

    // Preserve native `0x004DCCE0` RNG order. The two streams are process-global but
    // distinct: gameplay RNG first handles datum offset/scale/pickup variant; only
    // afterwards does DAT_007BE1E8 seed projectile scatter.
    let position = native_explosion_fragment_spawn_position(runtime, gameplay_rng)?;
    let uniform_scale = if definition.uses_random_uniform_scale() {
        robots_explosion_fragment_uniform_scale(gameplay_rng.next_unit_f32()?)?
    } else {
        1.0
    };
    let pickup_variant_unit_draw = definition
        .shipped_pickup_variant_consumes_rng()
        .then(|| gameplay_rng.next_unit_f32())
        .flatten();
    if definition.shipped_pickup_variant_consumes_rng() && pickup_variant_unit_draw.is_none() {
        return None;
    }

    let ballistic_plan =
        robots_explosion_fragment_ballistic_plan(runtime.definition, definition, process_lcg_seed);
    let physics = ballistic_plan.and_then(|plan| plan.projectile_physics(position.to_array()));
    Some(NativeAiExplosionFragmentSpawnRequest {
        source_body_key: runtime.source_body_key,
        explosion_uid: runtime.explosion_uid,
        ordinal: attempt.ordinal,
        fragment_uid: attempt.fragment_uid,
        position,
        definition,
        uniform_scale,
        pickup_variant_unit_draw,
        ballistic_plan,
        physics,
    })
}

/// Resolve the common Monster action (`0x004550A0`) explosion resource from the
/// exact Handler+0x628/+0x630/+0x634 state materialized from MonsterDatabase.
pub(super) fn native_common_monster_explosion_uid(
    visual: &ProcessedCharacterVisual,
) -> Option<u32> {
    let uid = robots_monster_action_explosion_uid(
        visual.handler_flags_628,
        visual.explosion_uid_634?,
        visual.explosion_uid_630?,
    );
    (uid != 0).then_some(uid)
}

impl MapFrame {
    pub(super) fn queue_native_ai_explosion(
        &mut self,
        source_body_key: u64,
        explosion_uid: u32,
        position: Vec3,
    ) {
        self.native_ai_explosion_spawns
            .push(NativeAiExplosionSpawnRequest::new(
                source_body_key,
                explosion_uid,
                position,
            ));
    }

    pub(super) fn queue_native_common_monster_explosion(
        &mut self,
        source_body_key: u64,
        visual: &ProcessedCharacterVisual,
        position: Vec3,
    ) -> bool {
        let Some(explosion_uid) = native_common_monster_explosion_uid(visual) else {
            return false;
        };
        self.queue_native_ai_explosion(source_body_key, explosion_uid, position);
        true
    }

    fn commit_native_ai_explosion_fragment_spawn(
        &mut self,
        fragment: NativeAiExplosionFragmentSpawnRequest,
    ) {
        self.native_ai_explosion_fragment_spawns.push(fragment);
        // Native 0x004DD1D7 consumes the Handler+0x478 serial first; the nested
        // 0x00425A70 then consumes the immediately following embedded-query serial.
        let cadence_phase_48c = robots_explosion_fragment_allocate_cadence_phase(
            &mut self.native_ai_explosion_fragment_phase_counter,
        );
        let handler_query_serial_478 =
            robots_hit_query_allocate_serial(&mut self.native_hit_query_next_serial);
        let hit_query_serial =
            robots_hit_query_allocate_serial(&mut self.native_hit_query_next_serial);
        if let Some(live) = NativeAiExplosionFragmentRuntime::from_spawn(
            fragment,
            cadence_phase_48c,
            handler_query_serial_478,
            hit_query_serial,
        ) {
            self.native_ai_explosion_fragments.push(live);
        }
    }

    /// Materialize queued `0x004DC510` requests after the AI/contact producer phase
    /// but before deferred owner destruction. `0x004DC6A0` performs the first
    /// `0x004DCC20` fragment attempt immediately during init, so this does too.
    pub(super) fn consume_native_ai_explosion_spawns(&mut self, map: &ProcessedMap) {
        let requests = std::mem::take(&mut self.native_ai_explosion_spawns);
        let Some(database) = map.explosion_database.as_ref() else {
            return;
        };

        for request in requests {
            let Some(definition) = database.definition(request.explosion_uid).copied() else {
                continue;
            };
            let generation_transform = self.runtime_ai_animation_datum_world_transform_by_key(
                map,
                request.source_body_key,
                ROBOTS_EXPLOSION_GENERATION_ANIM_DATUM,
            );
            let fragment_generation_shape = self.runtime_ai_animation_datum_world_shape_by_key(
                map,
                request.source_body_key,
                ROBOTS_EXPLOSION_FRAGMENT_ANIM_DATUM,
            );
            self.native_ai_explosion_main_spawns
                .push(NativeAiExplosionMainSpawnRequest {
                    source_body_key: request.source_body_key,
                    explosion_uid: request.explosion_uid,
                    position: request.position,
                    generation_transform,
                    script_uid: definition.main_script_uid,
                    file_uid: definition.main_file_uid,
                });
            let (hit_query, handler_query_serial_380) = match native_explosion_query_serials(
                &mut self.native_hit_query_next_serial,
                definition,
            ) {
                Some((hit_query_serial, handler_query_serial_380)) => (
                    native_explosion_hit_query_state(definition, hit_query_serial),
                    Some(handler_query_serial_380),
                ),
                None => (None, None),
            };
            let source_yaw_radians = self
                .runtime_character_bodies
                .get(&request.source_body_key)
                .map(|body| horizontal_yaw(body.owner_rotation));
            // `0x004DC510` seeds the generic factory position from owner +0xD0..+0xDC,
            // then replaces that vec4 with HT_AnimDatum_ExplosionGeneration when present.
            // The companion orientation vec4 passed to the factory is explicitly zeroed.
            let main_position =
                native_explosion_main_position(request.position, generation_transform);
            let mut runtime = NativeAiExplosionRuntime {
                source_body_key: request.source_body_key,
                explosion_uid: request.explosion_uid,
                position: main_position,
                definition,
                fragment_generation_shape,
                hit_query,
                collision_profile: database.collision_profile(request.explosion_uid).cloned(),
                collision_runtime: RobotsExplosionCollisionRuntimeState::default(),
                source_yaw_radians,
                handler_query_serial_380,
                state: RobotsExplosionRuntimeState::default(),
            };
            let use_native_gameplay_rng = self.native_script_trigger_lifecycle_valid
                && self.native_global_gameplay_rng.seed().is_some();
            let use_native_process_rng = self.native_script_trigger_lifecycle_valid
                && self.native_process_lcg_seed.is_some();
            let fragment = {
                let gameplay_rng = if use_native_gameplay_rng {
                    &mut self.native_global_gameplay_rng
                } else {
                    &mut self.native_ai_preview_gameplay_rng
                };
                let process_lcg_seed = if use_native_process_rng {
                    &mut self.native_process_lcg_seed
                } else {
                    &mut self.native_ai_preview_process_lcg_seed
                };
                native_explosion_take_fragment_request(
                    database,
                    &mut runtime,
                    gameplay_rng,
                    process_lcg_seed,
                )
            };
            if let Some(fragment) = fragment {
                self.commit_native_ai_explosion_fragment_spawn(fragment);
            }
            self.native_ai_explosions.push(runtime);
        }
    }

    /// Common `XItemPhysics_Projectile::Update` followed by the later
    /// `XExplosionFragment::Update` Handler pass. A fragment born in the current tick
    /// cannot receive a retroactive Physics/Handler service.
    pub(super) fn advance_native_ai_explosion_fragments_fixed(
        &mut self,
        map: &ProcessedMap,
        player_position: Vec3,
    ) {
        let mut fragments = std::mem::take(&mut self.native_ai_explosion_fragments);
        let mut pickup_spawns = Vec::new();
        let engine_frame_counter = self.native_ai_engine_frame_counter;
        let game_control_mode = self.native_game_control_runtime.mode_50f;
        for fragment in &mut fragments {
            // Native scheduler order is current Physics -> common world contact -> Handler.
            // Once 0x004DBEF0 swaps in XItemPhysics_PickupAttract, Projectile integration
            // and its static-world contact pass stop running for this fragment.
            fragment.advance_physics_fixed(player_position, game_control_mode);
            if fragment.pickup_attract_physics.is_none() {
                if let Some(Some(contact)) = runtime_map_projectile_static_world_contact_detail(
                    map,
                    fragment.position(),
                    ROBOTS_EXPLOSION_FRAGMENT_HIT_QUERY_RADIUS,
                ) {
                    if let Some(pickup) = fragment.apply_static_world_contact(contact) {
                        pickup_spawns.push(pickup);
                    }
                    if fragment.pending_destroy {
                        continue;
                    }
                }
            }

            if let Some(pickup) = fragment.advance_handler_fixed(engine_frame_counter) {
                pickup_spawns.push(pickup);
            }
            if fragment.should_service_hit_query() {
                let context = RobotsHitQueryCandidateContext {
                    flags: fragment.hit_query.flags,
                    query_serial: fragment.hit_query.serial,
                    // Generic XItem factory 0x00403DB0 initializes XItem+0x260 to zero.
                    source_raw_group: Some(0),
                    // 0x004DD23F passes parent Explosion Handler+0x4B4 as query secondary
                    // source; AI-character creators use native raw group1.
                    secondary_source_raw_group: Some(1),
                };
                let shape = fragment.hit_query_shape();
                let hit_player = self.native_hit_shape_hits_player(map, shape, context);
                let hit_ai = self.native_hit_shape_hits_live_ai(
                    map,
                    shape,
                    context,
                    Some(fragment.source_body_key),
                    None,
                );
                if hit_player || hit_ai {
                    // Native +0xE4 dispatch is XExplosionFragment::0x004DBCB0, a direct
                    // tail-jump into generic teardown 0x004DBB20.
                    fragment.hit_query.hit_present = true;
                    if let Some(pickup) = fragment.generic_teardown() {
                        pickup_spawns.push(pickup);
                    }
                }
            }

            // Native 0x004DB6B0 services 0x004DBEF0/0x004DBF90 after the explicit
            // fragment HitQuery on this same staggered frame lane. Ordinary pickup
            // fragments have no raw bit 0x8, so this must not be hidden behind the
            // HitQuery branch above.
            if !fragment.pending_destroy
                && robots_explosion_fragment_cadence_due(
                    engine_frame_counter,
                    fragment.cadence_phase_48c,
                )
            {
                let definition = map
                    .inventory_definitions
                    .iter()
                    .copied()
                    .find(|definition| definition.uid == fragment.definition.pickup_uid);
                let _ = fragment.service_player_pickup_tail(
                    player_position,
                    definition,
                    &mut self.native_script_gameplay_state.inventory,
                );
            }
        }
        self.native_ai_explosion_fragments = fragments;
        self.native_ai_explosion_fragment_pickup_spawns
            .extend(pickup_spawns);
    }

    /// Native fragment teardown uses the same deferred XItem destruction model as other
    /// handlers. Keep the marked body alive through the represented tick and unlink it
    /// only in the end-of-tick flush.
    pub(super) fn flush_native_ai_explosion_fragment_destroy_fixed(&mut self) {
        self.native_ai_explosion_fragments
            .retain(|fragment| !fragment.pending_destroy);
    }

    /// Replay the native Explosion Handler phase (`0x004DCBA0`) followed by the
    /// attached main-Script animator. Fragment cadence and HitQuery therefore see the
    /// Collision datum from the previous Script tick, exactly like `XItem+0x28`.
    pub(super) fn advance_native_ai_explosions_fixed(&mut self, map: &ProcessedMap) {
        let Some(database) = map.explosion_database.as_ref() else {
            return;
        };
        let use_native_gameplay_rng = self.native_script_trigger_lifecycle_valid
            && self.native_global_gameplay_rng.seed().is_some();
        let use_native_process_rng =
            self.native_script_trigger_lifecycle_valid && self.native_process_lcg_seed.is_some();
        let mut emitted = Vec::new();
        let mut runtimes = std::mem::take(&mut self.native_ai_explosions);

        for runtime in &mut runtimes {
            // Native 0x004DCBA0 services the fragment sequence before the embedded
            // HitQuery. Keep both global RNG streams transactionally host-owned.
            if runtime.state.advance_service() {
                let fragment = {
                    let gameplay_rng = if use_native_gameplay_rng {
                        &mut self.native_global_gameplay_rng
                    } else {
                        &mut self.native_ai_preview_gameplay_rng
                    };
                    let process_lcg_seed = if use_native_process_rng {
                        &mut self.native_process_lcg_seed
                    } else {
                        &mut self.native_ai_preview_process_lcg_seed
                    };
                    native_explosion_take_fragment_request(
                        database,
                        runtime,
                        gameplay_rng,
                        process_lcg_seed,
                    )
                };
                if let Some(fragment) = fragment {
                    emitted.push(fragment);
                }
            }

            let query_shape = native_explosion_hit_query_shape(runtime);
            if let Some(mut query) = runtime.hit_query {
                let context = RobotsHitQueryCandidateContext {
                    flags: query.flags,
                    query_serial: query.serial,
                    source_raw_group: None,
                    // `0x004DC6A0 -> 0x00425A70` keeps the creator/source AI XItem as
                    // the secondary source. Shipped AI character bodies are raw group 1.
                    secondary_source_raw_group: Some(1),
                };
                let scan_hit = if let Some(query_shape) = query_shape {
                    let player_hit = self.native_hit_shape_hits_player(map, query_shape, context);
                    let ai_hit = self.native_hit_shape_hits_live_ai(
                        map,
                        query_shape,
                        context,
                        Some(runtime.source_body_key),
                        runtime.source_yaw_radians,
                    );
                    player_hit || ai_hit
                } else {
                    false
                };
                // Explosion requests FLT_MAX budget, so the standard 60-rate budget
                // decrement keeps the query active while its source XItem exists.
                let _ = query.step(query_shape.is_some(), scan_hit, 0, 1.0);
                runtime.hit_query = Some(query);
            }

            // `XItem+0x28 = 0x004E8188`: Handler above, attached Script animator after.
            // A newly spawned priority-0x14 Explosion cannot run in the already-passed
            // lane of its priority-0x32 Monster spawn tick; first Handler therefore sees
            // no opcode-13 child, then this creates/applies Script frame 0 for next tick.
            if let Some(profile) = runtime.collision_profile.as_ref() {
                runtime.collision_runtime.advance_animator(profile);
            }
        }
        self.native_ai_explosions = runtimes;

        for fragment in emitted {
            self.commit_native_ai_explosion_fragment_spawn(fragment);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eurochef_shared::robots_runtime::{
        explosion::{
            RobotsExplosionCollisionProfile, RobotsExplosionFragmentDefinition,
            ROBOTS_EXPLOSION_FRAGMENT_ROW_WORDS,
        },
        hit_reaction::ROBOTS_MONSTER_ACTION_ALT_EXPLOSION_FLAG,
    };

    fn visual_with_explosions(flags: u32) -> ProcessedCharacterVisual {
        ProcessedCharacterVisual {
            file: 0,
            script: 0,
            runtime_type: 5,
            config_index: 0,
            handler_class:
                eurochef_shared::robots_runtime::ai_character::RobotsAiHandlerClass::MonsterBase,
            handler_flags_628: flags,
            initial_health: None,
            attacker_priority_638: None,
            explosion_uid_634: Some(0x5200_003A),
            explosion_uid_630: Some(0x5200_002C),
            magnetic_mass: None,
            pickup_drop_count: None,
            collision: None,
            hit_area: None,
            initial_animation: None,
            animation_modes: Default::default(),
            animation_mode_scripts: Default::default(),
            anim_datums: Default::default(),
            animation_mode_datum_tracks: Default::default(),
            move_animation: None,
            turn_on_spot_l_animation: None,
            turn_on_spot_r_animation: None,
        }
    }

    #[test]
    fn common_monster_explosion_uid_uses_exact_handler_flag_lane() {
        assert_eq!(
            native_common_monster_explosion_uid(&visual_with_explosions(0)),
            Some(0x5200_003A)
        );
        assert_eq!(
            native_common_monster_explosion_uid(&visual_with_explosions(
                ROBOTS_MONSTER_ACTION_ALT_EXPLOSION_FLAG,
            )),
            Some(0x5200_002C)
        );
    }

    #[test]
    fn explosion_hit_query_uses_definition_plus_40_selector_and_plus_38_flags() {
        let mut words = [0u32; 17];
        words[16] = u32::MAX;
        assert!(native_explosion_hit_query_state(
            RobotsExplosionDefinition::from_native_words(words),
            17,
        )
        .is_none());

        words[14] = 0x0000_0200;
        words[16] = 0x1000_0001;
        let definition = RobotsExplosionDefinition::from_native_words(words);
        let state = native_explosion_hit_query_state(definition, 17).unwrap();
        assert_eq!(state.selector, 0x1000_0001);
        assert_eq!(state.flags, 0x0000_0200);
        assert_eq!(state.remaining_budget, f32::MAX);
        assert_eq!(state.serial, 17);
        assert!(state.source_present);
        assert!(state.secondary_source_present);

        let mut next_serial = 17;
        assert_eq!(
            native_explosion_query_serials(&mut next_serial, definition),
            Some((17, 18))
        );
        assert_eq!(next_serial, 19);
        let mut disabled = definition.raw_words();
        disabled[16] = u32::MAX;
        assert_eq!(
            native_explosion_query_serials(
                &mut next_serial,
                RobotsExplosionDefinition::from_native_words(disabled),
            ),
            None
        );
        assert_eq!(next_serial, 19);
    }

    #[test]
    fn explosion_generation_datum_replaces_factory_fallback_position() {
        let fallback = Vec3::new(1.0, 2.0, 3.0);
        assert_eq!(native_explosion_main_position(fallback, None), fallback);
        let generated = RuntimeCharacterDatumWorldTransform {
            position: Vec3::new(4.0, 5.0, 6.0),
            rotation: glam::Quat::from_rotation_y(1.0),
        };
        assert_eq!(
            native_explosion_main_position(fallback, Some(generated)),
            generated.position
        );
    }

    #[test]
    fn explosion_collision_is_invisible_to_first_handler_then_visible_after_script_frame0() {
        let profile = RobotsExplosionCollisionProfile {
            explosion_selector: 0x5200_003A,
            script_uid: 0x0400_01C5,
            command_start: 0,
            command_length: 52,
            datum_shape_mode: 1,
            serialized_shape_scalar_bits: [0.5_f32.to_bits(), 0, 0],
            position_keys: Vec::new(),
            scale_keys: Vec::new(),
        };
        let definition = RobotsExplosionDefinition::from_native_words([0; 17]);
        let mut runtime = NativeAiExplosionRuntime {
            source_body_key: 7,
            explosion_uid: profile.explosion_selector,
            position: Vec3::new(10.0, 20.0, 30.0),
            definition,
            fragment_generation_shape: None,
            hit_query: None,
            collision_profile: Some(profile),
            collision_runtime: RobotsExplosionCollisionRuntimeState::default(),
            source_yaw_radians: None,
            handler_query_serial_380: None,
            state: RobotsExplosionRuntimeState::default(),
        };
        assert_eq!(native_explosion_hit_query_shape(&runtime), None);
        runtime
            .collision_runtime
            .advance_animator(runtime.collision_profile.as_ref().unwrap());
        assert_eq!(
            native_explosion_hit_query_shape(&runtime),
            Some(RobotsHitShape::Sphere {
                center_xyz: [10.0, 20.0, 30.0],
                radius: 0.5,
            })
        );
    }

    #[test]
    fn fragment_static_contact_uses_native_bit0_rebound_and_terminal_policy() {
        let mut words = [u32::MAX;
            eurochef_shared::robots_runtime::explosion::ROBOTS_EXPLOSION_FRAGMENT_ROW_WORDS];
        words[0] = 0x5300_0037;
        words[7] =
            eurochef_shared::robots_runtime::explosion::ROBOTS_EXPLOSION_FRAGMENT_REBOUND_FLAG;
        let definition = RobotsExplosionFragmentDefinition::from_native_words(words);
        let spawn = |velocity_x: f32| NativeAiExplosionFragmentSpawnRequest {
            source_body_key: 7,
            explosion_uid: 0x5200_003A,
            ordinal: 0,
            fragment_uid: definition.selector,
            position: Vec3::ZERO,
            definition,
            uniform_scale: 1.0,
            pickup_variant_unit_draw: None,
            ballistic_plan: None,
            physics: Some(RobotsProjectilePhysicsRuntimeState {
                position_xyz: [0.0; 3],
                linear_velocity_xyz: [velocity_x, 0.0, 0.0],
                gravity_velocity_y: 0.0,
                gravity_acceleration: 9.8,
                updates_completed: 0,
            }),
        };

        let mut runtime =
            NativeAiExplosionFragmentRuntime::from_spawn(spawn(2.0), 1, 1, 2).unwrap();
        runtime.apply_static_world_contact(RuntimeProjectileStaticWorldContact {
            contact_bits: 0x0001,
            normal: Some(Vec3::X),
        });
        assert!(!runtime.pending_destroy);
        assert_eq!(runtime.physical_contact_responses_completed, 1);
        assert!((runtime.physics.linear_velocity_xyz[0] + 1.96).abs() < 1.0e-6);

        let mut slow = NativeAiExplosionFragmentRuntime::from_spawn(spawn(0.5), 2, 3, 4).unwrap();
        slow.apply_static_world_contact(RuntimeProjectileStaticWorldContact {
            contact_bits: 0x0001,
            normal: Some(Vec3::X),
        });
        assert!(slow.pending_destroy);
        assert_eq!(slow.physical_contact_responses_completed, 1);

        let mut immediate =
            NativeAiExplosionFragmentRuntime::from_spawn(spawn(2.0), 3, 5, 6).unwrap();
        immediate.apply_static_world_contact(RuntimeProjectileStaticWorldContact {
            contact_bits: 0x0006,
            normal: None,
        });
        assert!(!immediate.pending_destroy);
        assert_eq!(immediate.physical_contact_responses_completed, 0);

        let mut ambiguous =
            NativeAiExplosionFragmentRuntime::from_spawn(spawn(2.0), 4, 7, 8).unwrap();
        ambiguous.apply_static_world_contact(RuntimeProjectileStaticWorldContact {
            contact_bits: 0x0001,
            normal: None,
        });
        assert!(ambiguous.pending_destroy);
        assert_eq!(ambiguous.physical_contact_responses_completed, 0);

        let mut latched =
            NativeAiExplosionFragmentRuntime::from_spawn(spawn(0.5), 5, 9, 10).unwrap();
        latched.handler_flags |= ROBOTS_EXPLOSION_FRAGMENT_RUNTIME_PLAYER_PICKUP_LATCH;
        assert_eq!(
            latched.apply_static_world_contact(RuntimeProjectileStaticWorldContact {
                contact_bits: 0x0001,
                normal: Some(Vec3::X),
            }),
            None
        );
        assert!(!latched.pending_destroy);
        assert_eq!(latched.physical_contact_responses_completed, 1);
    }

    #[test]
    fn fragment_generic_teardown_emits_dynamic_pickup_once_and_honors_handled_latch() {
        let mut words = [u32::MAX;
            eurochef_shared::robots_runtime::explosion::ROBOTS_EXPLOSION_FRAGMENT_ROW_WORDS];
        words[0] = 0x5300_0001;
        words[5] = 0x4700_0001;
        words[6] = 3;
        words[7] = eurochef_shared::robots_runtime::explosion::ROBOTS_EXPLOSION_FRAGMENT_SCRIPT_STATE_MARKER_FLAG;
        let definition = RobotsExplosionFragmentDefinition::from_native_words(words);
        let spawn = NativeAiExplosionFragmentSpawnRequest {
            source_body_key: 0x1234,
            explosion_uid: 0x5200_003A,
            ordinal: 2,
            fragment_uid: definition.selector,
            position: Vec3::new(1.0, 2.0, 3.0),
            definition,
            uniform_scale: 1.0,
            pickup_variant_unit_draw: Some(0.25),
            ballistic_plan: None,
            physics: Some(RobotsProjectilePhysicsRuntimeState {
                position_xyz: [1.0, 2.0, 3.0],
                linear_velocity_xyz: [0.0; 3],
                gravity_velocity_y: 0.0,
                gravity_acceleration: 9.8,
                updates_completed: 0,
            }),
        };
        let mut runtime = NativeAiExplosionFragmentRuntime::from_spawn(spawn, 6, 11, 12).unwrap();
        let pickup = runtime.generic_teardown().expect("dynamic pickup spawn");
        assert_eq!(pickup.source_body_key, 0x1234);
        assert_eq!(pickup.explosion_uid, 0x5200_003A);
        assert_eq!(pickup.fragment_uid, 0x5300_0001);
        assert_eq!(pickup.pickup_uid, 0x4700_0001);
        assert_eq!(pickup.quantity, 3);
        assert_eq!(pickup.position, Vec3::new(1.0, 2.0, 3.0));
        assert!(runtime.pending_destroy);
        assert_ne!(
            runtime.handler_flags & ROBOTS_EXPLOSION_FRAGMENT_RUNTIME_TEARDOWN_FLAG,
            0
        );
        assert_eq!(runtime.generic_teardown(), None);

        let mut handled = NativeAiExplosionFragmentRuntime::from_spawn(spawn, 7, 13, 14).unwrap();
        handled.handler_flags |= eurochef_shared::robots_runtime::explosion::ROBOTS_EXPLOSION_FRAGMENT_RUNTIME_PICKUP_HANDLED_FLAG;
        assert_eq!(handled.generic_teardown(), None);
        assert!(handled.pending_destroy);
    }

    #[test]
    fn fragment_pickup_tail_switches_to_attract_then_commits_inventory() {
        let definition = RobotsExplosionFragmentDefinition::from_native_words([
            0x5300_0001,
            0,
            0,
            u32::MAX,
            u32::MAX,
            0x4700_0001,
            3,
            eurochef_shared::robots_runtime::explosion::ROBOTS_EXPLOSION_FRAGMENT_SCRIPT_STATE_MARKER_FLAG,
        ]);
        let spawn = NativeAiExplosionFragmentSpawnRequest {
            source_body_key: 7,
            explosion_uid: 0x5200_003A,
            ordinal: 0,
            fragment_uid: definition.selector,
            position: Vec3::new(1.0, 0.5, 0.0),
            definition,
            uniform_scale: 1.0,
            pickup_variant_unit_draw: Some(0.25),
            ballistic_plan: None,
            physics: Some(RobotsProjectilePhysicsRuntimeState {
                position_xyz: [1.0, 0.5, 0.0],
                linear_velocity_xyz: [0.0; 3],
                gravity_velocity_y: 0.0,
                gravity_acceleration: 9.8,
                updates_completed: 0,
            }),
        };
        let mut runtime = NativeAiExplosionFragmentRuntime::from_spawn(spawn, 1, 1, 2).unwrap();
        runtime.handler_updates_completed = 31;
        let inventory_definition =
            RobotsInventoryDefinition::from_native_words([0x4700_0001, 0, 0, 0, 0, 0, 1000]);
        let mut inventory = RobotsInventoryState::default();

        assert!(!runtime.service_player_pickup_tail(
            Vec3::ZERO,
            inventory_definition,
            &mut inventory,
        ));
        assert_ne!(
            runtime.handler_flags & ROBOTS_EXPLOSION_FRAGMENT_RUNTIME_PLAYER_PICKUP_LATCH,
            0
        );
        assert!(runtime.pickup_attract_physics.is_some());
        assert_eq!(inventory.stored_current(0x4700_0001), 0);

        runtime
            .pickup_attract_physics
            .as_mut()
            .unwrap()
            .position_xyz = [0.0, 0.5, 0.0];
        assert!(runtime.service_player_pickup_tail(
            Vec3::ZERO,
            inventory_definition,
            &mut inventory,
        ));
        assert_eq!(inventory.stored_current(0x4700_0001), 3);
        assert_ne!(
            runtime.handler_flags
                & eurochef_shared::robots_runtime::explosion::ROBOTS_EXPLOSION_FRAGMENT_RUNTIME_PICKUP_HANDLED_FLAG,
            0
        );
        assert!(runtime.pending_destroy);
        assert!(!runtime.service_player_pickup_tail(
            Vec3::ZERO,
            inventory_definition,
            &mut inventory,
        ));
        assert_eq!(inventory.stored_current(0x4700_0001), 3);
    }

    #[test]
    fn fragment_handler_fade_runs_before_hitquery_and_uses_generic_teardown() {
        let mut words = [u32::MAX;
            eurochef_shared::robots_runtime::explosion::ROBOTS_EXPLOSION_FRAGMENT_ROW_WORDS];
        words[0] = 0x5300_0037;
        words[7] =
            eurochef_shared::robots_runtime::explosion::ROBOTS_EXPLOSION_FRAGMENT_REBOUND_FLAG
                | ROBOTS_EXPLOSION_FRAGMENT_HIT_QUERY_FLAG;
        let definition = RobotsExplosionFragmentDefinition::from_native_words(words);
        let spawn = NativeAiExplosionFragmentSpawnRequest {
            source_body_key: 1,
            explosion_uid: 0x5200_003A,
            ordinal: 0,
            fragment_uid: definition.selector,
            position: Vec3::ZERO,
            definition,
            uniform_scale: 1.0,
            pickup_variant_unit_draw: None,
            ballistic_plan: None,
            physics: Some(RobotsProjectilePhysicsRuntimeState {
                position_xyz: [0.0; 3],
                linear_velocity_xyz: [2.0, 0.0, 0.0],
                gravity_velocity_y: 0.0,
                gravity_acceleration: 9.8,
                updates_completed: 0,
            }),
        };
        let mut runtime = NativeAiExplosionFragmentRuntime::from_spawn(spawn, 8, 15, 16).unwrap();
        runtime.physical_contact_responses_completed = 1;
        runtime.visibility = RobotsExplosionFragmentVisibilityState {
            target_258: 0.0,
            current_25c: 0.0,
        };
        assert_eq!(runtime.advance_handler_fixed(1), None);
        assert!(runtime.pending_destroy);
        assert!(!runtime.should_service_hit_query());
    }

    #[test]
    fn fragment_staggered_vertical_service_runs_only_on_matching_engine_frame() {
        let mut words = [u32::MAX;
            eurochef_shared::robots_runtime::explosion::ROBOTS_EXPLOSION_FRAGMENT_ROW_WORDS];
        words[0] = 0x5300_0001;
        words[5] = 0x4700_0001;
        words[6] = 3;
        words[7] = eurochef_shared::robots_runtime::explosion::ROBOTS_EXPLOSION_FRAGMENT_SCRIPT_STATE_MARKER_FLAG
            | eurochef_shared::robots_runtime::explosion::ROBOTS_EXPLOSION_FRAGMENT_DROP_PICKUP_ON_FALL_FLAG;
        let definition = RobotsExplosionFragmentDefinition::from_native_words(words);
        let spawn = NativeAiExplosionFragmentSpawnRequest {
            source_body_key: 0x1234,
            explosion_uid: 0x5200_003A,
            ordinal: 0,
            fragment_uid: definition.selector,
            position: Vec3::new(1.0, 5.0, 3.0),
            definition,
            uniform_scale: 1.0,
            pickup_variant_unit_draw: None,
            ballistic_plan: None,
            physics: Some(RobotsProjectilePhysicsRuntimeState {
                position_xyz: [1.0, 5.0, 3.0],
                linear_velocity_xyz: [0.0; 3],
                gravity_velocity_y: 0.0,
                gravity_acceleration: 0.0,
                updates_completed: 0,
            }),
        };

        let mut not_due = NativeAiExplosionFragmentRuntime::from_spawn(spawn, 1, 20, 21).unwrap();
        not_due.physics.position_xyz = [9.0, 2.9, 7.0];
        assert_eq!(not_due.advance_handler_fixed(2), None);
        assert!(!not_due.pending_destroy);

        let mut due = NativeAiExplosionFragmentRuntime::from_spawn(spawn, 1, 22, 23).unwrap();
        due.physics.position_xyz = [9.0, 2.9, 7.0];
        let pickup = due
            .advance_handler_fixed(31)
            .expect("staggered fall pickup");
        assert!(due.pending_destroy);
        assert_eq!(pickup.pickup_uid, 0x4700_0001);
        assert_eq!(pickup.quantity, 3);
        assert_eq!(pickup.position, Vec3::new(1.0, 5.0, 3.0));
    }

    #[test]
    fn fragment_handler_marks_deferred_destroy_exactly_on_update_1200() {
        let definition = RobotsExplosionFragmentDefinition::from_native_words([
            0x5300_0001,
            0,
            0,
            0,
            0,
            u32::MAX,
            0,
            0,
        ]);
        let mut runtime = NativeAiExplosionFragmentRuntime {
            source_body_key: 7,
            explosion_uid: 0x5200_003A,
            ordinal: 0,
            fragment_uid: definition.selector,
            definition,
            uniform_scale: 1.0,
            pickup_variant_unit_draw: None,
            physics: RobotsProjectilePhysicsRuntimeState {
                position_xyz: [0.0, 0.0, 0.0],
                linear_velocity_xyz: [0.0, 0.0, 0.0],
                gravity_velocity_y: 0.0,
                gravity_acceleration: 0.0,
                updates_completed: 0,
            },
            pickup_attract_physics: None,
            initial_position: Vec3::ZERO,
            cadence_phase_48c: 0,
            handler_flags: definition.flags,
            visibility: RobotsExplosionFragmentVisibilityState::default(),
            handler_query_serial_478: 0x1234,
            hit_query: robots_explosion_fragment_hit_query_plan(definition)
                .instantiate(true, true, 0x1235),
            handler_updates_completed: 0,
            physical_contact_responses_completed: 0,
            pending_destroy: false,
        };
        for _ in 0..(ROBOTS_EXPLOSION_FRAGMENT_MAX_HANDLER_UPDATES - 1) {
            runtime.advance_fixed();
        }
        assert_eq!(
            runtime.handler_updates_completed,
            ROBOTS_EXPLOSION_FRAGMENT_MAX_HANDLER_UPDATES - 1
        );
        assert!(!runtime.pending_destroy);
        runtime.advance_fixed();
        assert_eq!(
            runtime.handler_updates_completed,
            ROBOTS_EXPLOSION_FRAGMENT_MAX_HANDLER_UPDATES
        );
        assert!(runtime.pending_destroy);
    }

    #[test]
    fn fragment_spawn_preserves_gameplay_then_process_rng_order() {
        let mut definition_words = [u32::MAX; 17];
        definition_words[0] = 0x5200_003A;
        definition_words[3] = 0x5300_0001;
        definition_words[13] = 2.0_f32.to_bits();
        let definition = RobotsExplosionDefinition::from_native_words(definition_words);
        let mut fragment_words = [u32::MAX; ROBOTS_EXPLOSION_FRAGMENT_ROW_WORDS];
        fragment_words[0] = 0x5300_0001;
        fragment_words[5] = 0x4700_0001;
        fragment_words[7] = 0x0000_0204;
        let fragment = RobotsExplosionFragmentDefinition::from_native_words(fragment_words);
        let database = RobotsExplosionDatabase {
            definitions: vec![definition],
            fragments: vec![fragment],
            collision_profiles: Vec::new(),
        };
        let mut runtime = NativeAiExplosionRuntime {
            source_body_key: 7,
            explosion_uid: definition.selector,
            position: Vec3::new(1.0, 2.0, 3.0),
            definition,
            fragment_generation_shape: Some(RobotsHitShape::Sphere {
                center_xyz: [10.0, 20.0, 30.0],
                radius: 2.0,
            }),
            hit_query: None,
            collision_profile: None,
            collision_runtime: RobotsExplosionCollisionRuntimeState::default(),
            source_yaw_radians: None,
            handler_query_serial_380: None,
            state: RobotsExplosionRuntimeState::default(),
        };
        let mut gameplay_rng = RuntimeRobotsGlobalRngState::fresh_process_startup();
        let mut process_seed = Some(1);
        let request = native_explosion_take_fragment_request(
            &database,
            &mut runtime,
            &mut gameplay_rng,
            &mut process_seed,
        )
        .unwrap();
        assert_eq!(gameplay_rng.draws_from_anchor, 5);
        assert_eq!(process_seed, Some(0x8116_017E));
        assert_ne!(request.position, Vec3::new(10.0, 20.0, 30.0));
        assert!((0.4..=1.0).contains(&request.uniform_scale));
        assert!(request.pickup_variant_unit_draw.is_some());
        assert!(request.physics.is_some());
    }

    #[test]
    fn fragment_factory_request_uses_fx03_sheet1_and_missing_row_arms_immediate_continue() {
        let mut definition_words = [u32::MAX; 17];
        definition_words[0] = 0x5200_003A;
        definition_words[3] = 0x5300_0001;
        definition_words[4] = u32::MAX;
        definition_words[13] = 2.0_f32.to_bits();
        let definition = RobotsExplosionDefinition::from_native_words(definition_words);
        let fragment = RobotsExplosionFragmentDefinition::from_native_words(std::array::from_fn::<
            _,
            ROBOTS_EXPLOSION_FRAGMENT_ROW_WORDS,
            _,
        >(|index| {
            if index == 0 {
                0x5300_0001
            } else {
                index as u32
            }
        }));
        let database = RobotsExplosionDatabase {
            definitions: vec![definition],
            fragments: vec![fragment],
            collision_profiles: Vec::new(),
        };
        let mut runtime = NativeAiExplosionRuntime {
            source_body_key: 7,
            explosion_uid: definition.selector,
            position: Vec3::new(1.0, 2.0, 3.0),
            definition,
            fragment_generation_shape: None,
            hit_query: None,
            collision_profile: None,
            collision_runtime: RobotsExplosionCollisionRuntimeState::default(),
            source_yaw_radians: None,
            handler_query_serial_380: None,
            state: RobotsExplosionRuntimeState::default(),
        };

        let mut gameplay_rng = RuntimeRobotsGlobalRngState::fresh_process_startup();
        let gameplay_seed_before = gameplay_rng.seed();
        let mut process_seed = Some(1);
        let first = native_explosion_take_fragment_request(
            &database,
            &mut runtime,
            &mut gameplay_rng,
            &mut process_seed,
        )
        .unwrap();
        assert_eq!(first.ordinal, 0);
        assert_eq!(first.fragment_uid, 0x5300_0001);
        assert_eq!(first.position, Vec3::new(1.0, 2.2, 3.0));
        assert_eq!(first.uniform_scale, 1.0);
        assert_eq!(first.pickup_variant_unit_draw, None);
        assert!(first.ballistic_plan.is_some());
        assert!(first.physics.is_some());
        assert_eq!(gameplay_rng.seed(), gameplay_seed_before);
        assert_eq!(process_seed, Some(0x0CF0_6D60));
        assert!(!runtime.state.continue_next_service);

        let gameplay_seed_after_first = gameplay_rng.seed();
        let process_seed_after_first = process_seed;
        assert!(native_explosion_take_fragment_request(
            &database,
            &mut runtime,
            &mut gameplay_rng,
            &mut process_seed,
        )
        .is_none());
        assert_eq!(gameplay_rng.seed(), gameplay_seed_after_first);
        assert_eq!(process_seed, process_seed_after_first);
        assert!(runtime.state.continue_next_service);
        assert!(runtime.state.advance_service());
        assert_eq!(runtime.state.next_fragment_ordinal, 2);
    }
}
