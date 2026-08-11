use super::*;

impl MapFrame {
    fn native_camera_player_anchor(map: &ProcessedMap) -> Option<Vec3> {
        map.triggers
            .iter()
            .find(|trigger| trigger.ttype == 0)
            .map(|trigger| trigger.position)
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
            let player_anchor = Self::native_camera_player_anchor(map).unwrap_or(position);
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
        let player_anchor = Self::native_camera_player_anchor(map).unwrap_or(runtime.player_anchor);
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
