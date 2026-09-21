use super::*;

impl MapFrame {
    /// Cache immutable Sweeper map bindings separately from the mutable gameplay runtime.
    /// Runtime creation is independent of shared-RNG provenance; each native phase remains
    /// fail-closed only when that phase actually needs an unavailable process-global draw/input.
    pub(super) fn sync_native_sweeper_boss_runtime_map(&mut self, map: &ProcessedMap) {
        if self.native_sweeper_boss_runtime_map == Some(map.hashcode) {
            return;
        }

        self.native_sweeper_boss_runtime_map = Some(map.hashcode);
        self.native_sweeper_boss_bindings = None;
        self.native_sweeper_boss_runtime = None;

        if map.sweeper_boss_patterns.is_none()
            || map.sweeper_eye_scripts.is_none()
            || map.sweeper_ratchet_scripts.is_none()
        {
            return;
        }
        let Some(bindings) = resolve_sweeper_boss_map_bindings(map) else {
            return;
        };

        let mut runtime = NativeSweeperBossReplayRuntime::default();
        runtime.initialize_owned_ratchet_yaw(bindings.initial_ratchet_yaw);
        runtime.post_ratchet_ai.enable_generic_handoff();
        self.native_sweeper_boss_bindings = Some(bindings);
        self.native_sweeper_boss_runtime = Some(runtime);
    }

    /// Type87 `+0x60 = 0x004CDF10` TriggerManager service. The common trigger lifecycle and
    /// create budget are owned by the outer serialized TriggerManager loop; this method owns only
    /// the controller reducer and its conditional process-global RNG draws.
    pub(super) fn advance_native_sweeper_boss_controller_service_fixed(
        &mut self,
        map: &ProcessedMap,
    ) -> bool {
        let Some(patterns) = map.sweeper_boss_patterns.as_ref() else {
            return false;
        };
        let Some(runtime) = self.native_sweeper_boss_runtime.as_mut() else {
            return false;
        };
        let player_health_state_known = self.native_sweeper_player_health_known
            && self.native_sweeper_player_current_health.is_finite()
            && self.native_sweeper_player_max_health.is_finite()
            && self.native_sweeper_player_current_health >= 0.0
            && self.native_sweeper_player_max_health > 0.0
            && self.native_sweeper_player_current_health <= self.native_sweeper_player_max_health;

        runtime
            .step_controller_phase_owned_rng(
                patterns,
                &mut self.native_global_gameplay_rng,
                NativeSweeperBossOwnedControllerPhaseInput {
                    player_health_state_known,
                    player_handler_present: player_health_state_known,
                    player_current_health: self.native_sweeper_player_current_health,
                    player_max_health: self.native_sweeper_player_max_health,
                },
            )
            .is_some()
    }

    /// XItemManager slice after TriggerManager has completed. This is deliberately separate from
    /// the controller service so unrelated TriggerManager users of `0x007BE1E4` (notably Fluid)
    /// remain interleaved in native serialized order before Eye/Transporter/Ratchet work.
    pub(super) fn advance_native_sweeper_boss_xitem_fixed(
        &mut self,
        map: &ProcessedMap,
        player_position: Option<Vec3>,
        transporter_steps: Option<&FxHashMap<u64, NativeMonsterTransporterFixedStep>>,
    ) -> bool {
        let player_position = player_position;
        let (Some(patterns), Some(eye_scripts), Some(ratchet_scripts)) = (
            map.sweeper_boss_patterns.as_ref(),
            map.sweeper_eye_scripts.as_ref(),
            map.sweeper_ratchet_scripts.as_ref(),
        ) else {
            return true;
        };
        let Some(bindings) = self.native_sweeper_boss_bindings.as_ref() else {
            return true;
        };

        let controller_key =
            Self::runtime_event_key(map.hashcode, bindings.controller_trigger_index);
        let zone_visual_active = map
            .triggers
            .get(bindings.controller_trigger_index)
            .and_then(|trigger| map.native_zone_index(trigger.position))
            .and_then(|zone_index| self.native_zone_runtime.get(zone_index))
            .is_some_and(|zone| zone.activated && zone.visual_depth > 0);
        let controller_xitem_exists = self
            .native_sweeper_boss_controller_lifecycle
            .get_mut(&controller_key)
            .map(|state| {
                state.advance_xitem(zone_visual_active);
                state.xitem_exists
            })
            .unwrap_or(false);
        if !controller_xitem_exists {
            return true;
        }
        let Some(player_position) = player_position else {
            self.native_global_gameplay_rng.invalidate();
            return false;
        };

        let transporter_key =
            Self::runtime_event_key(map.hashcode, bindings.transporter_trigger_index);
        let mut transporter_events = Vec::new();
        let transporter_events_known = transporter_steps.is_some();
        let linked_transporter_step =
            transporter_steps.and_then(|steps| steps.get(&transporter_key));
        let transporter_owner_position = self
            .native_monster_transporters
            .get(&transporter_key)
            .map(|runtime| {
                [
                    runtime.position.x,
                    runtime.position.y,
                    runtime.position.z,
                    1.0,
                ]
            });
        let transporter_spawn_position = self
            .native_monster_transporters
            .get(&transporter_key)
            .map(|runtime| {
                let local = Vec3::from_array(bindings.transporter_spawn_position_local);
                let world = runtime.position + runtime.rotation * local;
                [world.x, world.y, world.z, 1.0]
            });
        let mut transporter_spawn_attempts = 0u16;
        if let Some(step) = linked_transporter_step {
            for event in &step.events {
                match event {
                    NativeMonsterTransporterPathEvent::ResetReload => {
                        transporter_events.push(NativeSweeperBossTransporterEvent::ResetReload);
                    }
                    NativeMonsterTransporterPathEvent::SpawnAttempts { count } => {
                        transporter_spawn_attempts =
                            transporter_spawn_attempts.saturating_add(*count);
                    }
                }
            }
        }
        let Some(runtime) = self.native_sweeper_boss_runtime.as_mut() else {
            return true;
        };

        let player_position = [player_position.x, player_position.y, player_position.z, 1.0];
        let health_pickup_valid_hit = runtime.health_pickup.as_ref().and_then(|pickup| {
            sweeper_health_pickup_player_contact_guaranteed_miss(
                player_position,
                pickup.spawn_plan.position,
            )
            .then_some(false)
        });
        let input = NativeSweeperBossOwnedXItemPhaseInput {
            roller_path_graph: Some(&bindings.roller_path_graph),
            transporter_events_known,
            transporter_events: &transporter_events,
            transporter_spawn: Some(&bindings.transporter_spawn),
            transporter_spawn_attempts,
            transporter_owner_position,
            transporter_spawn_position,
            health_pickup_valid_hit,
            ratchet_missile_hand_local: map.sweeper_ratchet_missile_hand_local,
            player_position,
            anchors: bindings.ratchet_anchors,
            eye_spawn_sources: bindings.eye_spawn_sources,
        };
        let Some(step) = runtime.step_xitem_phase_owned_rng(
            patterns,
            ratchet_scripts,
            eye_scripts,
            &mut self.native_global_gameplay_rng,
            input,
        ) else {
            self.native_global_gameplay_rng.invalidate();
            return false;
        };
        let player_health_snapshot_stale = step
            .health_pickup
            .is_some_and(|health| health.inventory_add_requested);
        if player_health_snapshot_stale {
            self.native_sweeper_player_health_known = false;
        }

        true
    }

    /// Native fixed-frame tail `0x00444E0A` consumes one shared draw only when the
    /// per-frame Pickup candidate count is nonzero. We currently own a proof of zero,
    /// not the complete Pickup manager: shipped static ingress must be absent and no
    /// boss-side dynamic producer may have appeared since runtime reset.
    pub(super) fn advance_native_sweeper_boss_post_xitem_pickup_tail_fixed(
        &mut self,
        map: &ProcessedMap,
    ) {
        let Some(current_dynamic_risk) = self
            .native_sweeper_boss_runtime
            .as_ref()
            .map(|runtime| runtime.post_xitem_pickup_dynamic_risk())
        else {
            return;
        };
        if self.native_global_gameplay_rng.seed().is_some()
            && (!map.sweeper_post_xitem_pickup_static_zero || current_dynamic_risk)
        {
            self.native_global_gameplay_rng.invalidate();
        }
    }
}
