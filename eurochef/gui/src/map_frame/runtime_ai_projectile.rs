use super::{
    runtime_ai::{horizontal_yaw, NativeAiTransientSoundRequest},
    runtime_ai_explosion::NativeAiExplosionSpawnRequest,
    runtime_script_xitem::NativeScriptXItemSpawnRequest,
    MapFrame,
};
use crate::{
    map_runtime::{
        runtime_character_animation_datum_world_transform,
        runtime_map_projectile_static_world_contact, runtime_world_shape_for_collision_profile,
        RuntimeCharacterHitCandidateResult, RuntimeCharacterWorldShape,
    },
    maps::{ProcessedCharacterVisual, ProcessedMap},
};
use eurochef_shared::robots_runtime::{
    ai_character::RobotsAiHitReactionKind,
    hit_candidate_policy::{
        classify_hit_query_candidate, RobotsHitQueryCandidateContext,
        RobotsHitQueryCandidateDecision, RobotsHitQueryCandidateView,
        RobotsHitQueryNarrowphaseKind, ROBOTS_HIT_QUERY_RAW_GROUP2,
    },
    hit_query::robots_hit_query_allocate_serial,
    hit_reaction::{
        robots_apply_dodgem_hit_reaction, robots_apply_ef01_mine_hit_reaction,
        robots_apply_roller_bot_hit_reaction, robots_apply_turret_bot_hit_reaction,
        robots_apply_turret_hit_reaction, RobotsAcceptedHitReactionInput, RobotsAiHitReactionState,
    },
    hit_reaction_player::{
        robots_player_base_hit_gate, RobotsPlayerBaseHitGateInput, RobotsPlayerHitResult,
        RobotsPlayerHitRuntimeState,
    },
    hit_shapes::{robots_hit_shapes_intersect, RobotsHitShape},
    projectile::{
        RobotsCreateProjectileRequest, RobotsMissileDefinition, RobotsProjectileHandlerKind,
        RobotsProjectileRuntimeState,
    },
};
use glam::{Mat4, Quat, Vec3};

#[cfg(test)]
use eurochef_shared::robots_runtime::hit_reaction::ROBOTS_TURRET_HIT_SOUND_UID;

/// Host-resolved creation contract for a projectile emitted by an AI AnimScript.
/// Shared code owns Event/D04 semantics; this GUI adapter supplies the live animated
/// datum and target transforms. Physics ownership starts after this plan is accepted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct NativeAiProjectileSpawnPlan {
    pub(super) source_body_key: u64,
    pub(super) request: RobotsCreateProjectileRequest,
    pub(super) definition: RobotsMissileDefinition,
    pub(super) handler_kind: RobotsProjectileHandlerKind,
    pub(super) registration_mask: u32,
    pub(super) query_serial: u16,
    pub(super) launch_position: Vec3,
    pub(super) launch_rotation: Quat,
    pub(super) target_position: Vec3,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct NativeAiProjectileRuntime {
    pub(super) source_body_key: u64,
    pub(super) request: RobotsCreateProjectileRequest,
    pub(super) definition: RobotsMissileDefinition,
    pub(super) handler_kind: RobotsProjectileHandlerKind,
    pub(super) registration_mask: u32,
    pub(super) query_serial: u16,
    pub(super) launch_rotation: Quat,
    pub(super) target_position: Vec3,
    pub(super) state: RobotsProjectileRuntimeState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NativeAiProjectileTerminalReason {
    HitQuery,
    PhysicalContact,
    LifetimeExpired,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct NativeAiProjectileTerminalEvent {
    pub(super) source_body_key: u64,
    pub(super) missile_hashcode: u32,
    pub(super) handler_kind: RobotsProjectileHandlerKind,
    pub(super) position: Vec3,
    pub(super) rotation: Quat,
    pub(super) terminal_file_uid: u32,
    pub(super) primary_terminal_resource: u32,
    pub(super) reason: NativeAiProjectileTerminalReason,
    /// Base Projectile lifetime expiry enters +0xD4. The eventual explosion/resource
    /// dispatch stays a separate host seam rather than being silently fabricated here.
    pub(super) requests_native_terminal_path: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum NativeAiProjectileTerminalSpawn {
    Explosion(NativeAiExplosionSpawnRequest),
    Script(NativeScriptXItemSpawnRequest),
}

/// Native `XItemHandler_Projectile::GenerateExplosion 0x004152A0`.
/// `0x52xxxxxx` resources enter the common Explosion factory; every other
/// valid resource creates a generic Script XItem in Handler+0x384's file.
fn native_ai_projectile_terminal_spawn(
    event: NativeAiProjectileTerminalEvent,
) -> Option<NativeAiProjectileTerminalSpawn> {
    if !event.requests_native_terminal_path || event.primary_terminal_resource == u32::MAX {
        return None;
    }
    if event.primary_terminal_resource & 0x7F00_0000 == 0x5200_0000 {
        return Some(NativeAiProjectileTerminalSpawn::Explosion(
            NativeAiExplosionSpawnRequest::projectile_terminal(
                event.source_body_key,
                event.primary_terminal_resource,
                event.position,
                horizontal_yaw(event.rotation),
            ),
        ));
    }
    Some(NativeAiProjectileTerminalSpawn::Script(
        NativeScriptXItemSpawnRequest {
            owner_key: event.source_body_key,
            file_uid: event.terminal_file_uid,
            script_uid: event.primary_terminal_resource,
            position_xyzw: [event.position.x, event.position.y, event.position.z, 0.0],
            rotation: event.rotation,
        },
    ))
}

pub(super) fn resolve_ai_projectile_spawn_plan(
    map: &ProcessedMap,
    source_body_key: u64,
    owner_position: Vec3,
    owner_rotation: Quat,
    owner_scale: Vec3,
    visual: &ProcessedCharacterVisual,
    anim_mode: u32,
    request: RobotsCreateProjectileRequest,
    query_serial: u16,
    pose_seconds: f32,
    target_position: Vec3,
) -> Option<NativeAiProjectileSpawnPlan> {
    let definition = *map
        .missile_database
        .as_ref()?
        .definition_by_index(request.missile_index)?;
    let launch = runtime_character_animation_datum_world_transform(
        owner_position,
        owner_rotation,
        owner_scale,
        visual,
        anim_mode,
        request.launch_datum,
        pose_seconds,
    )?;
    Some(NativeAiProjectileSpawnPlan {
        source_body_key,
        request,
        definition,
        handler_kind: definition.handler_kind(),
        registration_mask: definition.registration_mask(),
        query_serial,
        launch_position: launch.position,
        launch_rotation: launch.rotation,
        target_position,
    })
}

impl NativeAiProjectileRuntime {
    fn from_spawn_plan(plan: NativeAiProjectileSpawnPlan) -> Option<Self> {
        let mut direction = plan.target_position - plan.launch_position;
        if !direction.is_finite() || direction.length_squared() <= f32::EPSILON {
            direction = plan.launch_rotation * Vec3::Z;
        }
        let state = RobotsProjectileRuntimeState::from_launch(
            plan.definition,
            plan.request,
            plan.launch_position.to_array(),
            direction.to_array(),
        )?;
        Some(Self {
            source_body_key: plan.source_body_key,
            request: plan.request,
            definition: plan.definition,
            handler_kind: plan.handler_kind,
            registration_mask: plan.registration_mask,
            query_serial: plan.query_serial,
            launch_rotation: plan.launch_rotation,
            target_position: plan.target_position,
            state,
        })
    }

    pub(super) fn position(&self) -> Vec3 {
        Vec3::from_array(self.state.physics.position_xyz)
    }

    fn hit_query_shape(&self) -> Option<RuntimeCharacterWorldShape> {
        let radius = self.definition.hit_query_radius();
        (radius.is_finite() && radius >= 0.0).then(|| RobotsHitShape::Sphere {
            center_xyz: self.state.physics.position_xyz,
            radius,
        })
    }
}

fn apply_confirmed_native_player_query_hit(
    runtime: &mut RobotsPlayerHitRuntimeState,
    last_hit_query_serial: &mut u16,
    context: RobotsHitQueryCandidateContext,
    top_game_state: u32,
    game_control_mode_50f: u8,
    player_state_6de: u8,
) -> RobotsPlayerHitResult {
    *last_hit_query_serial = context.query_serial;
    let base_hit_gate = robots_player_base_hit_gate(RobotsPlayerBaseHitGateInput {
        top_game_state,
        game_control_mode_50f,
        raw_gate_7b3021: false,
        context_7b2c80: None,
        field_460_nonzero: false,
        player_state_6de,
        field_6e8_nonzero: false,
    });
    runtime.apply_hit(base_hit_gate, context.flags, player_state_6de)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct NativeAiCandidateReactionOutcome {
    reaction_accepted: bool,
    clear_roller_motion: bool,
    request_natural_death: bool,
    transient_sound_uid: Option<u32>,
}

fn apply_native_ai_candidate_reaction(
    reaction_kind: Option<RobotsAiHitReactionKind>,
    hit_state: &mut RobotsAiHitReactionState,
    input: RobotsAcceptedHitReactionInput,
    owner_pending_destroy: bool,
) -> NativeAiCandidateReactionOutcome {
    match reaction_kind {
        Some(RobotsAiHitReactionKind::Common) => NativeAiCandidateReactionOutcome {
            reaction_accepted: hit_state.apply_hit(input, false).reaction_accepted,
            ..Default::default()
        },
        Some(RobotsAiHitReactionKind::RollerBot) => {
            let reaction = robots_apply_roller_bot_hit_reaction(hit_state, input, false);
            NativeAiCandidateReactionOutcome {
                reaction_accepted: reaction.base.reaction_accepted,
                clear_roller_motion: reaction.clear_four_runtime_words,
                ..Default::default()
            }
        }
        Some(RobotsAiHitReactionKind::Ef01Mine) => {
            let reaction =
                robots_apply_ef01_mine_hit_reaction(hit_state, input, owner_pending_destroy, false);
            NativeAiCandidateReactionOutcome {
                reaction_accepted: reaction.base.reaction_accepted,
                request_natural_death: reaction.request_monster_action_dispatch_0_1,
                ..Default::default()
            }
        }
        Some(RobotsAiHitReactionKind::TurretFamily) => {
            let reaction = robots_apply_turret_hit_reaction(hit_state, input, false);
            NativeAiCandidateReactionOutcome {
                reaction_accepted: reaction.base.reaction_accepted,
                transient_sound_uid: reaction.transient_sound_uid,
                ..Default::default()
            }
        }
        Some(RobotsAiHitReactionKind::Dodgem) => {
            let reaction = robots_apply_dodgem_hit_reaction(hit_state, input, false);
            NativeAiCandidateReactionOutcome {
                reaction_accepted: reaction.base.reaction_accepted,
                ..Default::default()
            }
        }
        Some(RobotsAiHitReactionKind::TurretBot) => {
            let reaction = robots_apply_turret_bot_hit_reaction(hit_state, input, false);
            NativeAiCandidateReactionOutcome {
                reaction_accepted: reaction.base.reaction_accepted,
                ..Default::default()
            }
        }
        None => NativeAiCandidateReactionOutcome::default(),
    }
}

impl MapFrame {
    pub(super) fn allocate_native_hit_query_serial(&mut self) -> u16 {
        robots_hit_query_allocate_serial(&mut self.native_hit_query_next_serial)
    }

    pub(super) fn spawn_native_ai_projectile(&mut self, plan: NativeAiProjectileSpawnPlan) {
        if let Some(runtime) = NativeAiProjectileRuntime::from_spawn_plan(plan) {
            self.native_ai_projectiles.push(runtime);
        }
    }

    /// Producer-neutral native HitQuery candidate traversal. Projectile, Explosion and
    /// later melee/script producers feed the same admission, serial-commit and concrete
    /// Handler +0xC8 reaction path instead of cloning AI-family damage logic.
    pub(super) fn native_hit_shape_hits_live_ai(
        &mut self,
        map: &ProcessedMap,
        query_shape: RuntimeCharacterWorldShape,
        context: RobotsHitQueryCandidateContext,
        secondary_source_body_key: Option<u64>,
        source_yaw_radians: Option<f32>,
    ) -> bool {
        let mut any_hit = false;
        let candidates = self.runtime_ai_hit_candidates(map);
        for candidate in candidates {
            let candidate_key = candidate.key;
            let is_secondary_source = secondary_source_body_key == Some(candidate_key);
            let result = self.test_runtime_ai_hit_candidate_by_key(
                candidate_key,
                context,
                query_shape,
                false,
                is_secondary_source,
                false,
            );
            if matches!(
                result,
                RuntimeCharacterHitCandidateResult::AnimDatumHitArea { hit: true }
            ) {
                any_hit = true;
                let reaction_kind = Some(candidate.handler_class.hit_reaction_kind());
                let reaction_input = RobotsAcceptedHitReactionInput {
                    query_flags: context.flags,
                    query_serial: context.query_serial,
                    hit_metadata: 0,
                    source_is_candidate_owner: false,
                    secondary_source_is_candidate_owner: is_secondary_source,
                };
                let owner_pending_destroy =
                    self.native_ai_deferred_destroy.is_pending(candidate_key);
                let reaction = self
                    .native_ai_hit_reactions
                    .get_mut(&candidate_key)
                    .map(|hit_state| {
                        apply_native_ai_candidate_reaction(
                            reaction_kind,
                            hit_state,
                            reaction_input,
                            owner_pending_destroy,
                        )
                    })
                    .unwrap_or_default();
                if let Some(sound_uid) = reaction.transient_sound_uid {
                    self.native_ai_transient_sounds
                        .push(NativeAiTransientSoundRequest {
                            owner_key: candidate_key,
                            sound_uid,
                        });
                }
                if reaction.clear_roller_motion {
                    if let Some(runtime) = self.native_rollerbot_runtime.get_mut(&candidate_key) {
                        runtime.clear_retained_motion();
                    }
                }
                if reaction.request_natural_death {
                    if let Some(position) = self
                        .runtime_character_bodies
                        .get(&candidate_key)
                        .map(|body| body.owner_position)
                    {
                        if let Some(explosion_uid) = candidate.common_monster_explosion_uid {
                            self.queue_native_ai_explosion(candidate_key, explosion_uid, position);
                            self.request_native_ai_natural_death(
                                map.hashcode,
                                candidate_key,
                                position,
                            );
                        }
                    }
                }
                if reaction.reaction_accepted {
                    if let Some(source_yaw_radians) = source_yaw_radians {
                        self.native_ai_last_hit_source_yaw
                            .insert(candidate_key, source_yaw_radians);
                    }
                    if let Some(source_position) = secondary_source_body_key
                        .and_then(|source_key| self.runtime_character_bodies.get(&source_key))
                        .map(|body| body.owner_position)
                    {
                        self.native_ai_last_hit_source_position
                            .insert(candidate_key, source_position);
                    }
                }
            }
        }
        any_hit
    }

    fn native_ai_projectile_hits_live_ai(
        &mut self,
        map: &ProcessedMap,
        projectile: &NativeAiProjectileRuntime,
    ) -> bool {
        let Some(query_shape) = projectile.hit_query_shape() else {
            return false;
        };
        let context = RobotsHitQueryCandidateContext {
            flags: projectile.registration_mask,
            query_serial: projectile.query_serial,
            source_raw_group: None,
            secondary_source_raw_group: Some(1),
        };
        let source_yaw_radians = self
            .runtime_character_bodies
            .get(&projectile.source_body_key)
            .map(|body| horizontal_yaw(body.owner_rotation));
        self.native_hit_shape_hits_live_ai(
            map,
            query_shape,
            context,
            Some(projectile.source_body_key),
            source_yaw_radians,
        )
    }

    /// Common Player candidate adapter for native HitCheck/projectile queries.
    /// Candidate policy, Rodney HitArea geometry and Handler+0x37C serial commit
    /// belong here rather than in each producer-specific AI brain.
    pub(super) fn native_hit_shape_hits_player(
        &mut self,
        map: &ProcessedMap,
        query_shape: RuntimeCharacterWorldShape,
        context: RobotsHitQueryCandidateContext,
    ) -> bool {
        let Some(player) = self.native_runtime_player_state else {
            return false;
        };
        let Some(hit_area) = map.player_hit_area.as_ref() else {
            return false;
        };
        let decision = classify_hit_query_candidate(
            context,
            RobotsHitQueryCandidateView {
                present: true,
                is_source: false,
                is_secondary_source: false,
                raw_group: ROBOTS_HIT_QUERY_RAW_GROUP2,
                last_hit_query_serial: self.native_runtime_player_last_hit_query_serial,
                coarse_bounds_miss: false,
            },
        );
        if decision
            != RobotsHitQueryCandidateDecision::Test(
                RobotsHitQueryNarrowphaseKind::AnimDatumHitArea,
            )
        {
            return false;
        }
        let player_shape = runtime_world_shape_for_collision_profile(
            player.position,
            player.rotation,
            Vec3::ONE,
            hit_area,
            Mat4::IDENTITY,
        );
        let hit = robots_hit_shapes_intersect(query_shape, player_shape);
        if hit {
            // Native 0x00425C70 commits Player Handler+0x37C even when the
            // subsequent Player +0xC8 reaction callback rejects the hit.
            let top_game_state = self
                .native_cutscene_host_runtime
                .game_state_stack
                .last()
                .copied()
                .unwrap_or_default();
            let player_state = self.native_player_focus_runtime.player_state;
            let _ = apply_confirmed_native_player_query_hit(
                &mut self.native_player_hit_runtime,
                &mut self.native_runtime_player_last_hit_query_serial,
                context,
                top_game_state,
                self.native_game_control_runtime.mode_50f,
                player_state,
            );
        }
        hit
    }

    fn native_ai_projectile_hits_player(
        &mut self,
        map: &ProcessedMap,
        projectile: &NativeAiProjectileRuntime,
    ) -> bool {
        let Some(query_shape) = projectile.hit_query_shape() else {
            return false;
        };
        self.native_hit_shape_hits_player(
            map,
            query_shape,
            RobotsHitQueryCandidateContext {
                flags: projectile.registration_mask,
                query_serial: projectile.query_serial,
                source_raw_group: None,
                secondary_source_raw_group: Some(1),
            },
        )
    }

    fn native_ai_projectile_terminal_event(
        projectile: &NativeAiProjectileRuntime,
        reason: NativeAiProjectileTerminalReason,
    ) -> NativeAiProjectileTerminalEvent {
        NativeAiProjectileTerminalEvent {
            source_body_key: projectile.source_body_key,
            missile_hashcode: projectile.definition.selector,
            handler_kind: projectile.handler_kind,
            position: projectile.position(),
            rotation: projectile.launch_rotation,
            terminal_file_uid: projectile.definition.factory_resource_pair()[0],
            primary_terminal_resource: projectile.definition.primary_terminal_resource(),
            reason,
            requests_native_terminal_path: true,
        }
    }

    /// Drain common Projectile `+0xD8` terminal generation into the already-owned
    /// Explosion / transient Script-XItem factories.
    pub(super) fn consume_native_ai_projectile_terminal_events(&mut self) {
        let events = std::mem::take(&mut self.native_ai_projectile_terminal_events);
        for event in events {
            match native_ai_projectile_terminal_spawn(event) {
                Some(NativeAiProjectileTerminalSpawn::Explosion(request)) => {
                    self.native_ai_explosion_spawns.push(request);
                }
                Some(NativeAiProjectileTerminalSpawn::Script(request)) => {
                    self.native_script_xitem_spawns.push(request);
                }
                None => {}
            }
        }
    }

    /// Projectile XItems are serviced before AI-character priority 0x32 in the
    /// represented fixed slice. A projectile created by an AI AnimScript therefore
    /// starts updating on the following fixed tick, never retroactively in its
    /// creation tick.
    pub(super) fn advance_native_ai_projectiles_fixed(&mut self, map: &ProcessedMap) {
        let projectiles = std::mem::take(&mut self.native_ai_projectiles);
        let mut survivors = Vec::with_capacity(projectiles.len());
        let mut terminal_events = Vec::new();
        for mut projectile in projectiles {
            // Homing adds class-specific steering before common Projectile Physics.
            // Keep it fail-closed until that handler path is recovered; the current
            // EQ02 production path creates XItemHandler_Mine, not HomingProjectile.
            if projectile.handler_kind == RobotsProjectileHandlerKind::Homing {
                survivors.push(projectile);
                continue;
            }

            // XItemHandler_Projectile::Update runs its embedded common hit-query
            // before physical-contact and lifetime tails. GenericMine suppresses the
            // separate +0xCC world-contact terminal hook but inherits this +0xD0 hit.
            let hit_live_ai = self.native_ai_projectile_hits_live_ai(map, &projectile);
            let hit_player = self.native_ai_projectile_hits_player(map, &projectile);
            if hit_live_ai || hit_player {
                terminal_events.push(Self::native_ai_projectile_terminal_event(
                    &projectile,
                    NativeAiProjectileTerminalReason::HitQuery,
                ));
                continue;
            }

            // Native `0x004150E0` checks Physics+0x0A after the explicit HitQuery and
            // before the lifetime tail. Generic Projectile XItems use the variant-0
            // Physics sphere seeded from missile row +0x1C; Mine replaces +0xCC with
            // a no-op and therefore keeps running through the same physical contact.
            let physical_contact = runtime_map_projectile_static_world_contact(
                map,
                projectile.position(),
                projectile.definition.hit_query_radius(),
            )
            .unwrap_or(false);
            if physical_contact
                && !projectile
                    .handler_kind
                    .suppresses_physical_contact_terminal_path()
            {
                terminal_events.push(Self::native_ai_projectile_terminal_event(
                    &projectile,
                    NativeAiProjectileTerminalReason::PhysicalContact,
                ));
                continue;
            }

            let handler_step = projectile.state.advance_handler_fixed(1.0);
            if handler_step.lifetime_expired {
                terminal_events.push(Self::native_ai_projectile_terminal_event(
                    &projectile,
                    NativeAiProjectileTerminalReason::LifetimeExpired,
                ));
                continue;
            }

            // The shared Projectile fixed integrator remains independent from world
            // geometry. Contact impulse/correction is a separate common Physics seam;
            // terminal projectiles do not reach this point, while stationary Mine is
            // already exact with zero speed/gravity.
            projectile.state.advance_physics_fixed(1.0, false);
            survivors.push(projectile);
        }
        self.native_ai_projectiles = survivors;
        self.native_ai_projectile_terminal_events
            .extend(terminal_events);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eurochef_shared::robots_runtime::hit_candidate_policy::ROBOTS_HIT_QUERY_RAW_GROUP1;

    fn generic_mine_plan() -> NativeAiProjectileSpawnPlan {
        let definition = RobotsMissileDefinition::from_native_words([
            0x5700_0008,
            0x0100_0038,
            0x0200_000E,
            0x0400_004C,
            0x1AF0_033F,
            0.0_f32.to_bits(),
            5.0_f32.to_bits(),
            0.2_f32.to_bits(),
            0x8,
            0,
            0,
            0,
            0,
            0,
        ]);
        NativeAiProjectileSpawnPlan {
            source_body_key: 7,
            request: RobotsCreateProjectileRequest {
                missile_index: 3,
                launch_datum: 0x1000_0011,
                source_override: u32::MAX,
                launch_scalar_bits: None,
            },
            definition,
            handler_kind: RobotsProjectileHandlerKind::Mine,
            registration_mask: 0x2000,
            query_serial: 0x1234,
            launch_position: Vec3::new(1.0, 2.0, 3.0),
            launch_rotation: Quat::IDENTITY,
            target_position: Vec3::new(10.0, 2.0, 3.0),
        }
    }

    #[test]
    fn projectile_terminal_routes_script_and_explosion_into_common_factories() {
        let rotation = Quat::from_rotation_y(0.75);
        let script_event = NativeAiProjectileTerminalEvent {
            source_body_key: 7,
            missile_hashcode: 0x5700_0008,
            handler_kind: RobotsProjectileHandlerKind::Mine,
            position: Vec3::new(4.0, 5.0, 6.0),
            rotation,
            terminal_file_uid: 0x0100_0038,
            primary_terminal_resource: 0x0400_004C,
            reason: NativeAiProjectileTerminalReason::LifetimeExpired,
            requests_native_terminal_path: true,
        };
        let Some(NativeAiProjectileTerminalSpawn::Script(script)) =
            native_ai_projectile_terminal_spawn(script_event)
        else {
            panic!("generic terminal resource must create a Script XItem");
        };
        assert_eq!(script.owner_key, 7);
        assert_eq!(script.file_uid, 0x0100_0038);
        assert_eq!(script.script_uid, 0x0400_004C);
        assert_eq!(script.position_xyzw, [4.0, 5.0, 6.0, 0.0]);
        assert_eq!(script.rotation, rotation);

        let explosion_event = NativeAiProjectileTerminalEvent {
            primary_terminal_resource: 0x5200_0006,
            ..script_event
        };
        let Some(NativeAiProjectileTerminalSpawn::Explosion(explosion)) =
            native_ai_projectile_terminal_spawn(explosion_event)
        else {
            panic!("HT_Explosion terminal resource must enter the common Explosion host");
        };
        assert_eq!(explosion.explosion_uid, 0x5200_0006);
        assert_eq!(explosion.position, Vec3::new(4.0, 5.0, 6.0));
        assert!(!explosion.resolve_source_character_datums);
        assert!(explosion.source_yaw_radians.is_some());

        let disabled = NativeAiProjectileTerminalEvent {
            requests_native_terminal_path: false,
            ..script_event
        };
        assert!(native_ai_projectile_terminal_spawn(disabled).is_none());

        let none = NativeAiProjectileTerminalEvent {
            primary_terminal_resource: u32::MAX,
            ..script_event
        };
        assert!(native_ai_projectile_terminal_spawn(none).is_none());
    }

    #[test]
    fn confirmed_player_query_hit_commits_serial_applies_twenty_hp_and_rearms_window() {
        let context = RobotsHitQueryCandidateContext {
            flags: 0,
            query_serial: 0x1234,
            source_raw_group: Some(ROBOTS_HIT_QUERY_RAW_GROUP1),
            secondary_source_raw_group: None,
        };
        let mut runtime = RobotsPlayerHitRuntimeState::default();
        let mut last_serial = u16::MAX;

        let result = apply_confirmed_native_player_query_hit(
            &mut runtime,
            &mut last_serial,
            context,
            0,
            0,
            2,
        );
        assert_eq!(last_serial, 0x1234);
        assert!(result.reaction_accepted);
        assert_eq!(runtime.current_health, 80.0);
        assert_eq!(runtime.reaction_window, 5.0);

        assert_eq!(
            classify_hit_query_candidate(
                context,
                RobotsHitQueryCandidateView {
                    present: true,
                    is_source: false,
                    is_secondary_source: false,
                    raw_group: ROBOTS_HIT_QUERY_RAW_GROUP2,
                    last_hit_query_serial: last_serial,
                    coarse_bounds_miss: false,
                },
            ),
            RobotsHitQueryCandidateDecision::Reject(
                eurochef_shared::robots_runtime::hit_candidate_policy::RobotsHitQueryCandidateRejectReason::AlreadyHitByQuerySerial,
            )
        );
    }

    #[test]
    fn ai_candidate_reaction_dispatch_keeps_roller_zero_health_side_effect() {
        let input = RobotsAcceptedHitReactionInput {
            query_flags: 0,
            query_serial: 7,
            hit_metadata: 0,
            source_is_candidate_owner: false,
            secondary_source_is_candidate_owner: false,
        };
        let mut common = RobotsAiHitReactionState {
            query_flags_snapshot: 0,
            query_serial_snapshot: u16::MAX,
            hit_metadata_snapshot: 0,
            last_query_serial: u16::MAX,
            capability_flags: 0,
            health: 2,
            got_hit_latch: false,
            turretbot_zero_health_latch_60c: false,
            owner_category: 5,
        };
        assert_eq!(
            apply_native_ai_candidate_reaction(
                Some(RobotsAiHitReactionKind::Common),
                &mut common,
                input,
                false,
            ),
            NativeAiCandidateReactionOutcome {
                reaction_accepted: true,
                ..Default::default()
            }
        );
        assert_eq!(common.health, 1);

        let mut roller = RobotsAiHitReactionState {
            health: 1,
            ..common
        };
        roller.last_query_serial = u16::MAX;
        assert_eq!(
            apply_native_ai_candidate_reaction(
                Some(RobotsAiHitReactionKind::RollerBot),
                &mut roller,
                RobotsAcceptedHitReactionInput {
                    query_serial: 8,
                    ..input
                },
                false,
            ),
            NativeAiCandidateReactionOutcome {
                reaction_accepted: true,
                clear_roller_motion: true,
                ..Default::default()
            }
        );
        assert_eq!(roller.health, 0);
    }

    #[test]
    fn ai_candidate_reaction_dispatch_routes_dodgem_force_zero_wrapper() {
        let mut dodgem = RobotsAiHitReactionState {
            query_flags_snapshot: 0,
            query_serial_snapshot: u16::MAX,
            hit_metadata_snapshot: 0,
            last_query_serial: u16::MAX,
            capability_flags: 0,
            health: 3,
            got_hit_latch: false,
            turretbot_zero_health_latch_60c: false,
            owner_category: 5,
        };
        let input = RobotsAcceptedHitReactionInput {
            query_flags: 0x10,
            query_serial: 0x2a,
            hit_metadata: 0,
            source_is_candidate_owner: false,
            secondary_source_is_candidate_owner: false,
        };

        assert_eq!(
            apply_native_ai_candidate_reaction(
                Some(RobotsAiHitReactionKind::Dodgem),
                &mut dodgem,
                input,
                false,
            ),
            NativeAiCandidateReactionOutcome {
                reaction_accepted: true,
                ..Default::default()
            }
        );
        assert_eq!(dodgem.health, 0);
        assert_eq!(dodgem.query_serial_snapshot, 0x2a);
        assert_eq!(dodgem.last_query_serial, 0x2a);
        assert!(dodgem.got_hit_latch);
    }

    #[test]
    fn ai_candidate_reaction_dispatch_routes_turret_family_through_derived_wrapper() {
        let mut turret = RobotsAiHitReactionState {
            query_flags_snapshot: 0,
            query_serial_snapshot: u16::MAX,
            hit_metadata_snapshot: 0,
            last_query_serial: u16::MAX,
            capability_flags: 0,
            health: 2,
            got_hit_latch: false,
            turretbot_zero_health_latch_60c: false,
            owner_category: 5,
        };
        let input = RobotsAcceptedHitReactionInput {
            query_flags: 0,
            query_serial: 0x33,
            hit_metadata: 0,
            source_is_candidate_owner: false,
            secondary_source_is_candidate_owner: false,
        };
        assert_eq!(
            apply_native_ai_candidate_reaction(
                Some(RobotsAiHitReactionKind::TurretFamily),
                &mut turret,
                input,
                false,
            ),
            NativeAiCandidateReactionOutcome {
                reaction_accepted: true,
                transient_sound_uid: Some(ROBOTS_TURRET_HIT_SOUND_UID),
                ..Default::default()
            }
        );
        assert_eq!(turret.health, 1);
        assert!(turret.got_hit_latch);

        turret.last_query_serial = u16::MAX;
        turret.got_hit_latch = false;
        turret.health = 2;
        assert_eq!(
            apply_native_ai_candidate_reaction(
                Some(RobotsAiHitReactionKind::TurretFamily),
                &mut turret,
                RobotsAcceptedHitReactionInput {
                    query_flags: 0x0040_0000,
                    query_serial: 0x34,
                    ..input
                },
                false,
            ),
            NativeAiCandidateReactionOutcome {
                reaction_accepted: true,
                transient_sound_uid: Some(ROBOTS_TURRET_HIT_SOUND_UID),
                ..Default::default()
            }
        );
        assert_eq!(turret.health, 0);
        assert!(turret.got_hit_latch);
    }

    #[test]
    fn ai_candidate_reaction_dispatch_routes_turretbot_and_persists_60c_on_rejected_force_zero() {
        let mut turretbot = RobotsAiHitReactionState {
            query_flags_snapshot: 0,
            query_serial_snapshot: u16::MAX,
            hit_metadata_snapshot: 0,
            last_query_serial: 0x40,
            capability_flags: 0,
            health: 3,
            got_hit_latch: false,
            turretbot_zero_health_latch_60c: false,
            owner_category: 5,
        };
        let outcome = apply_native_ai_candidate_reaction(
            Some(RobotsAiHitReactionKind::TurretBot),
            &mut turretbot,
            RobotsAcceptedHitReactionInput {
                query_flags: 0x0040_0000,
                query_serial: 0x40,
                hit_metadata: 0,
                source_is_candidate_owner: false,
                secondary_source_is_candidate_owner: false,
            },
            false,
        );

        assert_eq!(outcome, NativeAiCandidateReactionOutcome::default());
        assert_eq!(turretbot.health, 0);
        assert!(!turretbot.got_hit_latch);
        assert!(turretbot.turretbot_zero_health_latch_60c);
    }

    #[test]
    fn ai_candidate_reaction_dispatch_routes_ef01_alternate_hit_to_deferred_death_request() {
        let mut mine = RobotsAiHitReactionState {
            query_flags_snapshot: 0,
            query_serial_snapshot: u16::MAX,
            hit_metadata_snapshot: 0,
            last_query_serial: u16::MAX,
            capability_flags: 0,
            health: 3,
            got_hit_latch: false,
            turretbot_zero_health_latch_60c: false,
            owner_category: 5,
        };
        let input = RobotsAcceptedHitReactionInput {
            query_flags: 0,
            query_serial: 0x44,
            hit_metadata: 0,
            source_is_candidate_owner: false,
            secondary_source_is_candidate_owner: false,
        };

        assert_eq!(
            apply_native_ai_candidate_reaction(
                Some(RobotsAiHitReactionKind::Ef01Mine),
                &mut mine,
                input,
                false,
            ),
            NativeAiCandidateReactionOutcome {
                reaction_accepted: true,
                request_natural_death: true,
                ..Default::default()
            }
        );
        assert_eq!(mine.health, 3);
        assert_eq!(mine.last_query_serial, u16::MAX);

        assert_eq!(
            apply_native_ai_candidate_reaction(
                Some(RobotsAiHitReactionKind::Ef01Mine),
                &mut mine,
                RobotsAcceptedHitReactionInput {
                    query_serial: 0x45,
                    ..input
                },
                true,
            ),
            NativeAiCandidateReactionOutcome::default()
        );
    }

    #[test]
    fn generic_mine_runtime_is_stationary_and_data_limited_to_five_seconds() {
        let mut runtime = NativeAiProjectileRuntime::from_spawn_plan(generic_mine_plan()).unwrap();
        let initial = runtime.position();
        runtime.state.advance_handler_fixed(1.0);
        runtime.state.advance_physics_fixed(1.0, false);
        assert_eq!(runtime.position(), initial);
        assert_eq!(runtime.state.physics.linear_velocity_xyz, [0.0; 3]);
        assert_eq!(runtime.state.physics.gravity_acceleration.to_bits(), 0);
        assert!(runtime.state.handler.lifetime_seconds < 5.0);
    }
}
