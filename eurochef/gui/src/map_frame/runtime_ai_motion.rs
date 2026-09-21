use crate::map_runtime::{
    runtime_character_track_root_motion_delta, RuntimeCharacterBodyState,
    RuntimeCharacterRootMotionDelta,
};
use eurochef_shared::robots_runtime::{
    events::RobotsScriptEventView,
    locomotion::ROBOTS_FIXED_STEP_SECONDS,
    script_scheduler::{
        advance_script_scheduler, RobotsScriptNativeEventResult, RobotsScriptSchedulerState,
    },
};
use glam::{EulerRot, Quat, Vec3};

/// Root-motion ownership for one gameplay-selected AnimMode.
///
/// Keep translation and rotation independent. Several Robots AI families write
/// yaw directly while still consuming animation translation, whereas directional
/// TurnOnSpot delegates rotation to animation as well.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct NativeAiRootMotionPolicy {
    translation: bool,
    rotation: bool,
}

impl NativeAiRootMotionPolicy {
    pub(super) const NONE: Self = Self {
        translation: false,
        rotation: false,
    };
    pub(super) const TRANSLATION_ONLY: Self = Self {
        translation: true,
        rotation: false,
    };
    pub(super) const FULL: Self = Self {
        translation: true,
        rotation: true,
    };

    fn enabled(self) -> bool {
        self.translation || self.rotation
    }
}

/// Host-side animation clock for a live AI XItem.
///
/// Brain/selector modules own *which* AnimMode runs. RuntimeCharacterBodyState
/// owns collision pose and owner transforms. This small seam only projects the
/// chosen AnimMode into those body facilities, so later class brains can reuse it
/// without becoming animation controllers themselves.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct NativeAiAnimationEvent {
    pub(super) event_type: u32,
    pub(super) data: Vec<u8>,
    pub(super) start: Option<i16>,
    pub(super) length: Option<u16>,
    /// Animation phase already present in the live pose when native dispatches
    /// this Event. Consumers such as CreateProjectile use it for AnimDatum queries.
    pub(super) pose_seconds: f32,
    pub(super) owner_position: Vec3,
    pub(super) owner_rotation: Quat,
}

impl NativeAiAnimationEvent {
    fn from_view(
        event: RobotsScriptEventView<'_>,
        pose_seconds: f32,
        owner_position: Vec3,
        owner_rotation: Quat,
    ) -> Self {
        Self {
            event_type: event.event_type,
            data: event.data.to_vec(),
            start: event.start,
            length: event.length,
            pose_seconds,
            owner_position,
            owner_rotation,
        }
    }

    pub(super) fn as_view(&self) -> RobotsScriptEventView<'_> {
        RobotsScriptEventView {
            event_type: self.event_type,
            data: &self.data,
            start: self.start,
            length: self.length,
        }
    }
}

#[derive(Default)]
pub(super) struct NativeAiAnimationRuntime {
    current_anim_mode: Option<u32>,
    animation_seconds: f32,
    sampled_pose_seconds: f32,
    blend_seconds: f32,
    script_scheduler: RobotsScriptSchedulerState,
    /// Native Character Physics velocity lane (+0x1B8/+0x1BC/+0x1C0) produced
    /// by animation root motion in `0x004F3E8D`. Kept here so the common AI
    /// movement-error reducer (`0x00451DF0`) can consume the previous tick's
    /// physics velocity without pretending owner displacement itself is velocity.
    character_physics_velocity: Vec3,
}

impl NativeAiAnimationRuntime {
    pub(super) fn is_mode(&self, anim_mode: u32) -> bool {
        self.current_anim_mode == Some(anim_mode)
    }

    pub(super) fn current_anim_mode(&self) -> u32 {
        self.current_anim_mode.unwrap_or(0)
    }

    pub(super) fn animation_seconds(&self) -> f32 {
        self.animation_seconds
    }

    pub(super) fn sampled_pose_seconds(&self) -> f32 {
        self.sampled_pose_seconds
    }

    pub(super) fn character_physics_velocity(&self) -> Vec3 {
        self.character_physics_velocity
    }

    /// Native behavior nodes may write the same Character Physics velocity lanes
    /// after the animation service has already run for the fixed tick. EW09's
    /// derived Attack `0x0044FAF0 -> +0x110(0.4)` is one such writer; retaining it
    /// here makes the next `0x00451DF0` movement-error prepass observe native order.
    pub(super) fn overwrite_character_physics_velocity(&mut self, velocity: Vec3) {
        self.character_physics_velocity = velocity;
    }

    pub(super) fn reset(&mut self, body: &mut RuntimeCharacterBodyState) {
        self.current_anim_mode = None;
        self.animation_seconds = 0.0;
        self.sampled_pose_seconds = 0.0;
        self.blend_seconds = 0.0;
        self.script_scheduler.reset();
        self.character_physics_velocity = Vec3::ZERO;
        body.clear_collision_animation_track();
    }

    /// Native AI `SetupIdle` marks the animation channel for reset independently
    /// from the behavior-node callback. Keep the current sampled pose until the
    /// next brain update, but invalidate playback ownership so requesting the same
    /// AnimMode again starts its AnimScript from frame zero.
    pub(super) fn setup_idle(&mut self) {
        self.current_anim_mode = None;
        self.animation_seconds = 0.0;
        self.sampled_pose_seconds = 0.0;
        self.blend_seconds = 0.0;
        self.script_scheduler.reset();
        self.character_physics_velocity = Vec3::ZERO;
    }

    pub(super) fn advance(
        &mut self,
        body: &mut RuntimeCharacterBodyState,
        requested_anim_mode: u32,
        owner_yaw_write_radians: Option<f32>,
        animation_step_seconds: f32,
        root_motion_policy: NativeAiRootMotionPolicy,
    ) -> Vec<NativeAiAnimationEvent> {
        if let Some(owner_yaw_radians) = owner_yaw_write_radians {
            let (roll_z, pitch_x, _) = body.owner_rotation.to_euler(EulerRot::ZXY);
            body.owner_rotation =
                Quat::from_euler(EulerRot::ZXY, roll_z, pitch_x, owner_yaw_radians);
        }
        let event_owner_position = body.owner_position;
        let event_owner_rotation = body.owner_rotation;

        let mode_changed = self.current_anim_mode != Some(requested_anim_mode);
        if mode_changed {
            self.animation_seconds = 0.0;
            self.sampled_pose_seconds = 0.0;
            self.blend_seconds = 0.0;
            self.script_scheduler.reset();
        }
        self.sampled_pose_seconds = self.animation_seconds;

        let track = body.animation_modes.get(&requested_anim_mode).cloned();
        let script = body
            .animation_mode_scripts
            .get(&requested_anim_mode)
            .cloned();

        self.character_physics_velocity = Vec3::ZERO;
        if let Some(track) = track {
            let delta = root_motion_policy.enabled().then(|| {
                runtime_character_track_root_motion_delta(
                    &track,
                    self.animation_seconds,
                    animation_step_seconds.max(0.0),
                    self.blend_seconds,
                )
                .map(|delta| filter_root_motion(delta, root_motion_policy))
            });

            if !body.update_collision_animation_track(
                &track,
                self.animation_seconds,
                self.blend_seconds,
                mode_changed,
            ) && mode_changed
            {
                body.clear_collision_animation_track();
            }
            if let Some(Some(delta)) = delta {
                if root_motion_policy.translation && animation_step_seconds > 0.0 {
                    self.character_physics_velocity =
                        (body.owner_rotation * delta.native_translation) / animation_step_seconds;
                }
                body.apply_local_root_motion_delta(delta);
            }
        } else if mode_changed {
            body.clear_collision_animation_track();
        }

        let mut events = Vec::new();
        if let Some(script) = script {
            let delta_frames = animation_step_seconds.max(0.0) * script.timeline_framerate();
            let _ = advance_script_scheduler(
                &script,
                &mut self.script_scheduler,
                delta_frames,
                |event| {
                    events.push(NativeAiAnimationEvent::from_view(
                        event,
                        self.animation_seconds,
                        event_owner_position,
                        event_owner_rotation,
                    ));
                    RobotsScriptNativeEventResult::Continue
                },
            );
        }

        self.current_anim_mode = Some(requested_anim_mode);
        self.animation_seconds += animation_step_seconds.max(0.0);
        self.blend_seconds += ROBOTS_FIXED_STEP_SECONDS;
        events
    }

    /// Advance a non-primary native animation-state slot (for example EB13 slot9).
    /// These slots own their own AnimScript timeline and events, but they do not
    /// replace the character body's primary collision/root-motion AnimMode.
    pub(super) fn advance_auxiliary_script_channel(
        &mut self,
        body: &RuntimeCharacterBodyState,
        requested_anim_mode: u32,
        animation_step_seconds: f32,
    ) -> Vec<NativeAiAnimationEvent> {
        let event_owner_position = body.owner_position;
        let event_owner_rotation = body.owner_rotation;
        let mode_changed = self.current_anim_mode != Some(requested_anim_mode);
        if mode_changed {
            self.animation_seconds = 0.0;
            self.sampled_pose_seconds = 0.0;
            self.blend_seconds = 0.0;
            self.script_scheduler.reset();
        }
        self.sampled_pose_seconds = self.animation_seconds;
        self.character_physics_velocity = Vec3::ZERO;

        let mut events = Vec::new();
        if let Some(script) = body
            .animation_mode_scripts
            .get(&requested_anim_mode)
            .cloned()
        {
            let delta_frames = animation_step_seconds.max(0.0) * script.timeline_framerate();
            let _ = advance_script_scheduler(
                &script,
                &mut self.script_scheduler,
                delta_frames,
                |event| {
                    events.push(NativeAiAnimationEvent::from_view(
                        event,
                        self.animation_seconds,
                        event_owner_position,
                        event_owner_rotation,
                    ));
                    RobotsScriptNativeEventResult::Continue
                },
            );
        }

        self.current_anim_mode = Some(requested_anim_mode);
        self.animation_seconds += animation_step_seconds.max(0.0);
        self.blend_seconds += ROBOTS_FIXED_STEP_SECONDS;
        events
    }
}

fn filter_root_motion(
    delta: RuntimeCharacterRootMotionDelta,
    policy: NativeAiRootMotionPolicy,
) -> RuntimeCharacterRootMotionDelta {
    RuntimeCharacterRootMotionDelta {
        native_translation: if policy.translation {
            delta.native_translation
        } else {
            Vec3::ZERO
        },
        native_rotation: if policy.rotation {
            delta.native_rotation
        } else {
            Quat::IDENTITY
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_motion_policy_keeps_translation_and_rotation_ownership_separate() {
        let delta = RuntimeCharacterRootMotionDelta {
            native_translation: Vec3::new(1.0, 2.0, 3.0),
            native_rotation: Quat::from_rotation_y(0.5),
        };

        let translation_only =
            filter_root_motion(delta, NativeAiRootMotionPolicy::TRANSLATION_ONLY);
        assert_eq!(
            translation_only.native_translation,
            delta.native_translation
        );
        assert_eq!(translation_only.native_rotation, Quat::IDENTITY);

        let none = filter_root_motion(delta, NativeAiRootMotionPolicy::NONE);
        assert_eq!(none.native_translation, Vec3::ZERO);
        assert_eq!(none.native_rotation, Quat::IDENTITY);

        let full = filter_root_motion(delta, NativeAiRootMotionPolicy::FULL);
        assert_eq!(full, delta);
    }
}
