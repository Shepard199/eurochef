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
}
