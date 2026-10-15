use super::*;

impl MapFrame {
    /// Host-side Map + live Script line-of-sight query used by recovered AI
    /// behavior nodes. Static map geometry remains in map_runtime; dynamic Script
    /// geometry is rebuilt from the same live XItem/runtime animation state used
    /// by the renderer. Concrete brains consume only the boolean result.
    pub(super) fn runtime_map_script_line_of_sight_state(
        &self,
        map: &ProcessedMap,
        start: Vec3,
        end: Vec3,
        wall_time: f64,
    ) -> Option<(bool, usize, usize)> {
        if !crate::map_runtime::runtime_map_line_of_sight_clear(map, start, end)? {
            return Some((false, 0, 0));
        }
        self.runtime_dynamic_script_line_of_sight_state(map, start, end, wall_time)
    }

    pub(super) fn runtime_dynamic_script_line_of_sight_state(
        &self,
        map: &ProcessedMap,
        start: Vec3,
        end: Vec3,
        wall_time: f64,
    ) -> Option<(bool, usize, usize)> {
        if !self.native_script_trigger_lifecycle_valid {
            return None;
        }
        let script_start = self.script_animation_start_time.unwrap_or(wall_time);
        let script_global_time = (wall_time - script_start).max(0.0) as f32;
        let render_store = self.render_store.read();
        let mut source_count = 0usize;
        let mut entity_instance_count = 0usize;

        for (trigger_index, trigger) in map.triggers.iter().enumerate() {
            if trigger.ttype != 4
                || trigger.data.first().and_then(|value| *value).unwrap_or(0) & 0x100 == 0
            {
                continue;
            }
            let key = Self::runtime_event_key(map.hashcode, trigger_index);
            if !self
                .native_script_trigger_lifecycle
                .get(&key)
                .is_some_and(|state| state.xitem_exists)
            {
                continue;
            }
            source_count += 1;

            let visual = trigger.engine_options.visual_object?;
            if visual.base() != 0x0400_0000 {
                return None;
            }
            let visual_file =
                trigger_visual_file(self.file, visual, trigger.engine_options.visual_object_file);
            render_store.get_script(visual_file, visual)?;
            let script_time = resolved_map_script_time(
                &render_store,
                visual_file,
                visual,
                script_global_time,
                self.animate_scripts,
                self.script_playback_speed,
            );
            let rotation = Quat::from_euler(
                glam::EulerRot::ZXY,
                trigger.rotation.z,
                trigger.rotation.x,
                trigger.rotation.y,
            );
            let mut queue = Vec::new();
            render_script_without_static_animations(
                trigger.position,
                rotation,
                trigger.scale,
                visual_file,
                visual,
                script_time,
                &render_store,
                &mut |queued| queue.push(queued),
                vec![],
            );
            entity_instance_count += queue.len();
            if !crate::map_runtime::runtime_queued_entities_line_of_sight_clear(
                &queue,
                &render_store,
                start,
                end,
            )? {
                return Some((false, source_count, entity_instance_count));
            }
        }

        Some((true, source_count, entity_instance_count))
    }
    /// Build the Entity-child queue seen by native Script XItem slot16.
    /// Anim/Collision child animators are intentionally excluded because their
    /// native slot16 is the no-op 0x004E8A20.
    fn runtime_live_xitem_entity_queue(
        &self,
        map: &ProcessedMap,
        wall_time: f64,
    ) -> Option<Vec<QueuedEntityRender>> {
        let has_serialized_script_candidates = map.triggers.iter().any(|trigger| {
            trigger.ttype == 4
                && trigger.data.first().and_then(|value| *value).unwrap_or(0) & 0x100 != 0
        });
        if has_serialized_script_candidates && !self.native_script_trigger_lifecycle_valid {
            return None;
        }

        let script_start = self.script_animation_start_time.unwrap_or(wall_time);
        let script_global_time = (wall_time - script_start).max(0.0) as f32;
        let render_store = self.render_store.read();
        let mut queue = Vec::new();

        if self.native_script_trigger_lifecycle_valid {
            for (trigger_index, trigger) in map.triggers.iter().enumerate() {
                if trigger.ttype != 4
                    || trigger.data.first().and_then(|value| *value).unwrap_or(0) & 0x100 == 0
                {
                    continue;
                }
                let key = Self::runtime_event_key(map.hashcode, trigger_index);
                if !self
                    .native_script_trigger_lifecycle
                    .get(&key)
                    .is_some_and(|state| state.xitem_exists)
                {
                    continue;
                }

                let visual = trigger.engine_options.visual_object?;
                if visual.base() != 0x0400_0000 {
                    return None;
                }
                let visual_file = trigger_visual_file(
                    self.file,
                    visual,
                    trigger.engine_options.visual_object_file,
                );
                render_store.get_script(visual_file, visual)?;
                let script_time = resolved_map_script_time(
                    &render_store,
                    visual_file,
                    visual,
                    script_global_time,
                    self.animate_scripts,
                    self.script_playback_speed,
                );
                let rotation = Quat::from_euler(
                    glam::EulerRot::ZXY,
                    trigger.rotation.z,
                    trigger.rotation.x,
                    trigger.rotation.y,
                );
                render_script_without_static_animations(
                    trigger.position,
                    rotation,
                    trigger.scale,
                    visual_file,
                    visual,
                    script_time,
                    &render_store,
                    &mut |queued| queue.push(queued),
                    vec![],
                );
            }
        }

        // Generic gameplay Script factories 0x004043A0 and 0x00455C50 register
        // their transient FX XItems only with the update manager (0x004E9A1C).
        // Neither factory nor its known direct callers add XItemManager flags through
        // 0x00444240, so these transient Script XItems are not channel-0x02 blockers.

        // XTrigger_Pickup::CreateItem (0x0048A1E0) registers serialized Pickup
        // XItems with mask 0x1010, or 0x1410 for subtypes 0x2C/0x2E. Neither
        // carries channel bit 0x02 walked by 0x004ECF42, so serialized Pickup
        // Entity/Script animators are not part of this dynamic raycast pass.

        // GeneratePickupFromHash (0x00403F20) registers generated Pickup XItems
        // with mask 0x10, or 0x410 for subtype 0x2C. Neither carries channel bit
        // 0x02 walked by 0x004ECF42, so their Entity/Script animators are deliberately
        // excluded from this dynamic raycast queue even though they are renderable.

        // m08 XTrigger_BossRatchet::Activate (0x00482850) creates an XItem
        // backed by HT_Script_MagnetTruck and registers it with 0x00444240 mask
        // 0x02. Its Script therefore participates in the same live-XItem pass;
        // only Entity children are queued because Script Animation children have
        // native slot16 no-op semantics.
        for runtime in self.native_boss_ratchets.values() {
            render_store.get_script(
                ROBOTS_BOSS_RATCHET_MAGNET_TRUCK_FILE_UID,
                ROBOTS_BOSS_RATCHET_MAGNET_TRUCK_SCRIPT_UID,
            )?;
            render_script_without_static_animations(
                runtime.position,
                runtime.rotation(),
                Vec3::ONE,
                ROBOTS_BOSS_RATCHET_MAGNET_TRUCK_FILE_UID,
                ROBOTS_BOSS_RATCHET_MAGNET_TRUCK_SCRIPT_UID,
                runtime.script_time_seconds,
                &render_store,
                &mut |queued| queue.push(queued),
                vec![],
            );
        }

        // AI_Character finalizer 0x0047E6F0 performs two different registrations:
        // update-manager mask 0x05 at 0x004E9A1C and XItemManager flags 0x00009C02
        // at 0x00444240. Channel bit 0x02 is therefore present. The primary AI body
        // is EXItemAnimator_Anim (family 0x03) with no-op slot16, but ctor-proven
        // AttachEntity children are real EXItemAnimator_Entity blockers.
        for attachment in self.native_ai_entity_attachment_presentations(map) {
            if render_store
                .get_entity(attachment.file_uid, attachment.entity_uid)
                .is_none()
            {
                return None;
            }
            queue.push(QueuedEntityRender {
                entity: (attachment.file_uid, attachment.entity_uid),
                entity_alt: None,
                position: attachment.position,
                rotation: attachment.rotation,
                scale: attachment.scale,
            });
        }

        Some(queue)
    }

    pub(super) fn runtime_dynamic_xitem_nearest_raw_hit(
        &self,
        map: &ProcessedMap,
        start: Vec3,
        end: Vec3,
        wall_time: f64,
        max_t: f32,
    ) -> Option<Option<crate::map_runtime::RuntimeRobotsRaycastHit>> {
        let queue = self.runtime_live_xitem_entity_queue(map, wall_time)?;
        let render_store = self.render_store.read();
        crate::map_runtime::runtime_queued_entities_nearest_raw_hit(
            &queue,
            &render_store,
            start,
            end,
            max_t,
        )
    }

    /// Full host-visible WatchBot recovery query matching native 0x00494260:
    /// static Map/placement pass first, then live XItems using the same query
    /// fraction. Only after the global nearest raw face is known is category 0x20
    /// accepted by 0x00495A10.
    pub(super) fn runtime_watchbot_recovery_contact_state(
        &self,
        map: &ProcessedMap,
        owner_position: Vec3,
        owner_rotation_euler4: [f32; 4],
        controller_yaw_radians: f32,
        local_velocity_xyzw: [f32; 4],
        wall_time: f64,
    ) -> Option<Option<
        eurochef_shared::robots_runtime::watchbot_component::RobotsWatchbotComponentRecoveryContact,
    >> {
        let Some(probe) = crate::map_runtime::runtime_watchbot_recovery_probe(
            owner_position,
            owner_rotation_euler4,
            controller_yaw_radians,
            local_velocity_xyzw,
        ) else {
            return Some(None);
        };

        let static_hit = crate::map_runtime::runtime_map_static_nearest_raw_hit(
            map,
            probe.start,
            probe.end,
        )?;
        let static_limit = static_hit.map_or(1.0, |hit| hit.t);
        let dynamic_hit = self.runtime_dynamic_xitem_nearest_raw_hit(
            map,
            probe.start,
            probe.end,
            wall_time,
            static_limit,
        )?;
        Some(crate::map_runtime::runtime_watchbot_recovery_contact_from_hits(
            probe,
            static_hit,
            dynamic_hit,
        ))
    }

}
