use super::*;

/// A serialized XTrigger_Player location is only a spawn *definition*.
/// Gameplay target ownership begins when the Player XItem is materialized.
fn native_created_player_pose(state: Option<RuntimePlayerState>) -> Option<(Vec3, f32)> {
    let state = state?;
    let direction = state.rotation * Vec3::Z;
    Some((state.position, direction.x.atan2(direction.z)))
}

impl MapFrame {
    pub(super) fn serialized_camera_player_pose(
        map: &ProcessedMap,
        selected_trigger: Option<usize>,
    ) -> Option<(Vec3, f32)> {
        // An EDB can carry many Player destinations (m03_hub1 has 16).
        // Never pretend the first one is the automatic level entry. Explicit
        // editor selection or a genuinely unique Player is safe to preview.
        let trigger = selected_trigger
            .and_then(|index| map.triggers.get(index))
            .filter(|trigger| trigger.ttype == ROBOTS_TRIGGER_PLAYER)
            .or_else(|| {
                let mut players = map.triggers.iter()
                    .filter(|trigger| trigger.ttype == ROBOTS_TRIGGER_PLAYER);
                let only_player = players.next()?;
                players.next().is_none().then_some(only_player)
            })?;
        let mode = trigger.data.get(1).and_then(|value| *value).unwrap_or(0);
        Some((
            crate::map_runtime::runtime_player_spawn_position(trigger.position, mode),
            trigger.rotation.y,
        ))
    }

    fn ensure_native_camera_player_preview(&mut self, map: &ProcessedMap) {
        if self.native_camera_player_preview_map == Some(map.hashcode) {
            return;
        }

        let (fallback_position, fallback_direction, speed_mul) = {
            let viewer = self.viewer.lock();
            (
                viewer.camera_fly.position,
                viewer.camera_fly.front,
                viewer.camera_fly.speed_mul,
            )
        };
        let (position, mut direction) =
            Self::serialized_camera_player_pose(map, self.selected_trigger)
                .map(|(position, heading)| (position, Vec3::new(heading.sin(), 0.0, heading.cos())))
            .unwrap_or((fallback_position, fallback_direction));
        if !direction.is_finite() || direction.length_squared() <= f32::EPSILON {
            direction = Vec3::Z;
        }
        self.native_camera_player_preview
            .set_pose_from_direction(position, direction);
        self.native_camera_player_preview.speed_mul = speed_mul;
        self.native_camera_player_preview_map = Some(map.hashcode);
        self.native_camera_player_preview_last_time = None;
    }

    pub(super) fn reset_native_camera_player_preview(&mut self, map: &ProcessedMap) {
        self.native_camera_player_preview_map = None;
        self.ensure_native_camera_player_preview(map);
    }

    pub(super) fn update_native_camera_player_preview(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        map: &ProcessedMap,
        time: f64,
    ) {
        self.ensure_native_camera_player_preview(map);
        let delta_seconds = self
            .native_camera_player_preview_last_time
            .replace(time)
            .map(|last| (time - last).max(0.0) as f32)
            .unwrap_or_default();

        if self.native_camera_live_player_preview
            && self.apply_native_camera_viewport
            && !self.textfield_focused
        {
            self.native_camera_player_preview
                .update(ui, Some(response), delta_seconds);
        }
    }

    /// Only a class-created XTrigger_Player XItem is a gameplay target. The
    /// editor Camera proxy and serialized spawn are *not* Player Physics; they
    /// must never trigger native proximity events or make enemies attack.
    pub(super) fn native_gameplay_player_pose(&self) -> Option<(Vec3, f32)> {
        native_created_player_pose(self.native_runtime_player_state)
    }

    /// Camera-only source. Serialized spawn and the editor's live Camera proxy
    /// can preview native Camera controllers before the real Player exists,
    /// without fabricating an XItem for TriggerManager and AI.
    fn native_camera_player_pose(&mut self, map: &ProcessedMap) -> Option<(Vec3, f32)> {
        self.ensure_native_camera_player_preview(map);
        let fallback = self
            .native_gameplay_player_pose()
            .or_else(|| Self::serialized_camera_player_pose(map, self.selected_trigger))
            .unwrap_or_else(|| {
                let mut front = self.native_camera_player_preview.front;
                front.y = 0.0;
                (self.native_camera_player_preview.position, front.x.atan2(front.z))
            });
        // In gameplay the Camera tracks the same actual Player XItem as
        // TriggerManager and AI. Never let an editor camera proxy override a
        // live Player merely because the preview checkbox defaults to true.
        if (self.simulate_ai && self.native_runtime_player_state.is_some())
            || !self.native_camera_live_player_preview
        {
            return Some(fallback);
        }
        let mut horizontal_front = self.native_camera_player_preview.front;
        horizontal_front.y = 0.0;
        if !horizontal_front.is_finite() || horizontal_front.length_squared() <= f32::EPSILON {
            return Some((self.native_camera_player_preview.position, fallback.1));
        }
        horizontal_front = horizontal_front.normalize();
        Some((
            self.native_camera_player_preview.position,
            horizontal_front.x.atan2(horizontal_front.z),
        ))
    }

    fn native_camera_player_anchor(&mut self, map: &ProcessedMap) -> Option<Vec3> {
        self.native_camera_player_pose(map)
            .map(|(position, _)| position)
            .or_else(|| {
                self.ensure_native_camera_player_preview(map);
                Some(self.native_camera_player_preview.position)
            })
    }

    fn native_camera_bit0_collision_queue(
        &self,
        map: &ProcessedMap,
    ) -> Option<Vec<QueuedEntityRender>> {
        const BOSS_SEWER_CANON_TYPE: u32 = 76;
        let render_store = self.render_store.read();
        let mut queue = Vec::new();
        for (trigger_index, trigger) in map.triggers.iter().enumerate() {
            if trigger.ttype != BOSS_SEWER_CANON_TYPE
                || trigger.data.first().copied().flatten().unwrap_or_default() < 1
            {
                continue;
            }
            let key = Self::runtime_event_key(map.hashcode, trigger_index);
            let Some(state) = self.native_camera_bit0_trigger_lifecycle.get(&key) else {
                continue;
            };
            if !state.lifecycle.xitem_exists {
                continue;
            }

            let visual = trigger.engine_options.visual_object?;
            if visual.base() != 0x0400_0000 {
                return None;
            }
            let visual_file =
                trigger_visual_file(self.file, visual, trigger.engine_options.visual_object_file);
            let script = render_store.get_script(visual_file, visual)?;
            let script_time = script.time_at_frame(state.script_frame);
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
        Some(queue)
    }

    pub(super) fn sync_native_camera_viewport(&mut self, map: &ProcessedMap, time: f64) {
        self.native_camera_viewpoint_valid = false;
        if !self.apply_native_camera_viewport {
            self.native_camera_runtime = None;
            self.native_default_camera_runtime = None;
            self.native_camera_last_time = None;
            self.viewer.lock().clear_native_camera();
            return;
        }

        if let Some(trigger_index) = self.active_camera_trigger {
            self.native_default_camera_runtime = None;
            let Some(plan) = robots_camera_controller_plan(map, trigger_index) else {
                self.native_camera_runtime = None;
                self.native_camera_last_time = None;
                self.viewer.lock().clear_native_camera();
                return;
            };

            let live_player_anchor = self.native_camera_player_anchor(map);
            let needs_activation = self
                .native_camera_runtime
                .as_ref()
                .is_none_or(|runtime| runtime.trigger_index != trigger_index);

            if needs_activation {
                let mut viewer = self.viewer.lock();
                let camera = viewer.camera_mut();
                let position = camera.position();
                let mut direction = camera.rotation() * Vec3::Z;
                if !direction.is_finite() || direction.length_squared() <= f32::EPSILON {
                    direction = Vec3::Z;
                } else {
                    direction = direction.normalize();
                }
                let current = NativeCameraViewportPose {
                    position,
                    target: position + direction,
                    vertical_fov_degrees: camera
                        .vertical_fov_radians()
                        .map(f32::to_degrees)
                        .unwrap_or(90.0),
                    roll_degrees: 0.0,
                };
                let player_anchor = live_player_anchor.unwrap_or(position);
                self.native_camera_runtime = Some(robots_camera_viewport_runtime(
                    map,
                    plan,
                    current,
                    player_anchor,
                ));
                self.native_camera_last_time = Some(time);
            }

            let delta_seconds = self
                .native_camera_last_time
                .replace(time)
                .map(|last| (time - last).max(0.0) as f32)
                .unwrap_or_default();
            let mut viewer = self.viewer.lock();
            let Some(runtime) = self.native_camera_runtime.as_mut() else {
                viewer.clear_native_camera();
                return;
            };
            let player_anchor = live_player_anchor.unwrap_or(runtime.player_anchor);
            runtime.update_dynamic_pose(player_anchor);
            runtime.advance(delta_seconds);
            self.native_camera_viewpoint_valid =
                runtime.boundary.is_none() && runtime.current.is_finite();
            if self.native_camera_viewpoint_valid {
                viewer.set_native_camera(
                    NativeViewCamera::new(
                        runtime.current.position,
                        runtime.current.target,
                        runtime.current.vertical_fov_degrees,
                    )
                    .with_roll(runtime.current.roll_degrees),
                );
            } else {
                viewer.clear_native_camera();
            }
            return;
        }

        let previous_trigger_pose = self
            .native_camera_runtime
            .take()
            .map(|runtime| runtime.current);
        let Some((player_position, player_heading)) = self.native_camera_player_pose(map) else {
            self.native_default_camera_runtime = None;
            self.native_camera_last_time = None;
            self.viewer.lock().clear_native_camera();
            return;
        };
        if self.native_default_camera_runtime.is_none() {
            self.native_default_camera_runtime = Some(NativeDefaultPlayerCameraRuntime::new(
                player_position,
                player_heading,
                previous_trigger_pose,
            ));
            self.native_camera_last_time = Some(time);
        }
        let delta_seconds = self
            .native_camera_last_time
            .replace(time)
            .map(|last| (time - last).max(0.0) as f32)
            .unwrap_or_default();
        let bit0_collision_queue = self.native_camera_bit0_collision_queue(map);
        let render_store = Arc::clone(&self.render_store);
        let Some(runtime) = self.native_default_camera_runtime.as_mut() else {
            self.viewer.lock().clear_native_camera();
            return;
        };
        let contacts_resolved = runtime.update_player_pose_with_contacts(
            player_position,
            player_heading,
            |start, end| {
                let static_hit =
                    crate::map_runtime::runtime_map_camera_contact_nearest_t(map, start, end)?;
                if static_hit.is_some() {
                    return Some(static_hit);
                }
                let queue = bit0_collision_queue.as_ref()?;
                let render_store = render_store.read();
                crate::map_runtime::runtime_queued_entities_camera_contact_nearest_t(
                    queue,
                    &render_store,
                    start,
                    end,
                )
            },
        );
        if contacts_resolved {
            runtime.advance(delta_seconds);
        }
        let pose = runtime.current;

        // Ordinary mode 1 now mirrors the native two-stage contact query:
        // static Map/placements first, then shipped mask-bit0 BossSewerCanon
        // Script-XItems when static geometry is clear or rejected by material.
        // Gameplay-created transient bit0 Script-XItems remain outside Maps until
        // their owning gameplay factories (for example projectiles) are replayed.
        self.native_camera_viewpoint_valid = contacts_resolved && pose.is_finite();
        let mut viewer = self.viewer.lock();
        if self.native_camera_viewpoint_valid {
            viewer.set_native_camera(
                NativeViewCamera::new(pose.position, pose.target, pose.vertical_fov_degrees)
                    .with_roll(pose.roll_degrees),
            );
        } else {
            viewer.clear_native_camera();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn created_player_is_required_for_gameplay_target_pose() {
        // The serialized spawn, free Camera and editor live position are not
        // native Player XItems and must never create proximity/attack targets.
        assert_eq!(native_created_player_pose(None), None);
        let owner = RuntimePlayerState {
            trigger_index: 7,
            mode: 0,
            position: Vec3::new(10.0, 2.0, -4.0),
            rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        };
        let (position, yaw) = native_created_player_pose(Some(owner)).unwrap();
        assert_eq!(position, owner.position);
        assert!((yaw - std::f32::consts::FRAC_PI_2).abs() < 1.0e-5);
    }
}
