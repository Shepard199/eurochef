use super::*;

impl MapFrame {
    fn runtime_event_key(map_hash: u32, trigger_index: usize) -> u64 {
        ((map_hash as u64) << 32) | trigger_index as u64
    }

    fn runtime_event_supported(map: &ProcessedMap, trigger: &ProcessedTrigger) -> bool {
        robots_trigger_runtime_path_speed(trigger.ttype, &trigger.data).is_some()
            && map_trigger_runtime_path(map, trigger).is_some()
    }

    pub(super) fn runtime_event_snapshots(
        &mut self,
        map: &ProcessedMap,
        wall_time: f64,
    ) -> Vec<Option<RuntimeEventPreviewSnapshot>> {
        if !self.native_runtime_event_gate {
            return vec![None; map.triggers.len()];
        }

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
        let Some(trigger) = map.triggers.get(trigger_index) else {
            return;
        };
        if !Self::runtime_event_supported(map, trigger) {
            return;
        }
        self.native_runtime_event_gate = true;
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

    pub(super) fn reset_runtime_event(
        &mut self,
        map: &ProcessedMap,
        trigger_index: usize,
        wall_time: f64,
    ) {
        if map.triggers.get(trigger_index).is_none() {
            return;
        }
        self.runtime_event_states
            .entry(Self::runtime_event_key(map.hashcode, trigger_index))
            .or_default()
            .reset(wall_time);
    }

    pub(super) fn reset_all_runtime_events(&mut self) {
        self.runtime_event_states.clear();
        self.runtime_motion_start_time = None;
    }
}
