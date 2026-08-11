use super::*;

impl MapFrame {
    fn serialized_camera_player_anchor(map: &ProcessedMap) -> Option<Vec3> {
        map.triggers
            .iter()
            .find(|trigger| trigger.ttype == 0)
            .map(|trigger| trigger.position)
    }

    fn ensure_native_camera_player_preview(&mut self, map: &ProcessedMap) {
        if self.native_camera_player_preview_map == Some(map.hashcode) {
            return;
        }

        let (fallback_position, mut direction, speed_mul) = {
            let viewer = self.viewer.lock();
            (
                viewer.camera_fly.position,
                viewer.camera_fly.front,
                viewer.camera_fly.speed_mul,
            )
        };
        if !direction.is_finite() || direction.length_squared() <= f32::EPSILON {
            direction = Vec3::Z;
        }
        let position = Self::serialized_camera_player_anchor(map).unwrap_or(fallback_position);
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
            && self.active_camera_trigger.is_some()
            && !self.textfield_focused
        {
            self.native_camera_player_preview
                .update(ui, Some(response), delta_seconds);
        }
    }

    fn native_camera_player_anchor(&mut self, map: &ProcessedMap) -> Option<Vec3> {
        self.ensure_native_camera_player_preview(map);
        if self.native_camera_live_player_preview {
            Some(self.native_camera_player_preview.position)
        } else {
            Self::serialized_camera_player_anchor(map)
                .or(Some(self.native_camera_player_preview.position))
        }
    }

    pub(super) fn sync_native_camera_viewport(&mut self, map: &ProcessedMap, time: f64) {
        let Some(trigger_index) = self
            .apply_native_camera_viewport
            .then_some(self.active_camera_trigger)
            .flatten()
        else {
            self.native_camera_runtime = None;
            self.native_camera_last_time = None;
            self.viewer.lock().clear_native_camera();
            return;
        };

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

        if runtime.boundary.is_none() && runtime.current.is_finite() {
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
    }
}
