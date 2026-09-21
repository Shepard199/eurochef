use serde::Serialize;

use super::{
    locomotion::{shortest_yaw_delta, ROBOTS_FIXED_STEP_SECONDS},
    path_spline::{RobotsNaturalCubicSpline3, ROBOTS_XPATH_PARAMETER_STEP},
    watchbot_component::{
        service_watchbot_component_recovery, watchbot_component_stop_predicate,
        RobotsWatchbotComponentRecoveryContact, RobotsWatchbotComponentStopInput,
        ROBOTS_WATCHBOT_COMPONENT_ACCELERATION_SCALE, ROBOTS_WATCHBOT_COMPONENT_EPSILON,
        ROBOTS_WATCHBOT_COMPONENT_MAX_HORIZONTAL_SPEED, ROBOTS_WATCHBOT_COMPONENT_MOVING_DAMPING,
        ROBOTS_WATCHBOT_COMPONENT_NEGATIVE_EPSILON,
        ROBOTS_WATCHBOT_COMPONENT_RECOVERY_INPUT_MAGNITUDE,
        ROBOTS_WATCHBOT_COMPONENT_RECOVERY_LATCH_UPDATES,
        ROBOTS_WATCHBOT_COMPONENT_STEERING_ROLL_SCALE,
        ROBOTS_WATCHBOT_COMPONENT_STOP_BLOCKED_ANIM_MODES,
        ROBOTS_WATCHBOT_COMPONENT_YAW_INPUT_SCALE,
    },
};

pub const ROBOTS_WATCHBOT_MODE3_STATE_IDLE: u32 = 1;
pub const ROBOTS_WATCHBOT_MODE3_STATE_DRIVE: u32 = 3;
pub const ROBOTS_WATCHBOT_MODE3_STATE_RECOVERY: u32 = 8;

pub const ROBOTS_WATCHBOT_MODE3_EPSILON: f32 = ROBOTS_WATCHBOT_COMPONENT_EPSILON;
pub const ROBOTS_WATCHBOT_MODE3_NEGATIVE_EPSILON: f32 = ROBOTS_WATCHBOT_COMPONENT_NEGATIVE_EPSILON;
pub const ROBOTS_WATCHBOT_MODE3_MAX_HORIZONTAL_SPEED: f32 =
    ROBOTS_WATCHBOT_COMPONENT_MAX_HORIZONTAL_SPEED;
pub const ROBOTS_WATCHBOT_MODE3_STEERING_ROLL_SCALE: f32 =
    ROBOTS_WATCHBOT_COMPONENT_STEERING_ROLL_SCALE;
pub const ROBOTS_WATCHBOT_MODE3_YAW_INPUT_SCALE: f32 = ROBOTS_WATCHBOT_COMPONENT_YAW_INPUT_SCALE;
pub const ROBOTS_WATCHBOT_MODE3_ACCELERATION_SCALE: f32 =
    ROBOTS_WATCHBOT_COMPONENT_ACCELERATION_SCALE;
pub const ROBOTS_WATCHBOT_MODE3_MOVING_DAMPING: f32 = ROBOTS_WATCHBOT_COMPONENT_MOVING_DAMPING;
pub const ROBOTS_WATCHBOT_MODE3_IDLE_DAMPING: f32 = f32::from_bits(0x3F4C_CCCD); // 0.8
pub const ROBOTS_WATCHBOT_MODE3_PATH_STEP_RADIUS_SQUARED: f32 = 1.0;
pub const ROBOTS_WATCHBOT_MODE3_RECOVERY_INPUT_MAGNITUDE: f32 =
    ROBOTS_WATCHBOT_COMPONENT_RECOVERY_INPUT_MAGNITUDE;
pub const ROBOTS_WATCHBOT_MODE3_RECOVERY_LATCH_UPDATES: u8 =
    ROBOTS_WATCHBOT_COMPONENT_RECOVERY_LATCH_UPDATES;
pub const ROBOTS_WATCHBOT_MODE3_STOP_BLOCKED_ANIM_MODES: [u32; 3] =
    ROBOTS_WATCHBOT_COMPONENT_STOP_BLOCKED_ANIM_MODES;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsWatchbotMode3ControlInput {
    /// Component +0x40 and +0x48. Native input setup keeps the X/Z steering pair
    /// separate from magnitude at +0x60.
    pub direction_xz: [f32; 2],
    /// Component +0x60.
    pub magnitude: f32,
}

impl Default for RobotsWatchbotMode3ControlInput {
    fn default() -> Self {
        Self {
            direction_xz: [0.0; 2],
            magnitude: 0.0,
        }
    }
}

pub type RobotsWatchbotMode3StopInput = RobotsWatchbotComponentStopInput;
pub type RobotsWatchbotMode3RecoveryContact = RobotsWatchbotComponentRecoveryContact;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsWatchbotMode3FixedInput {
    pub owner_position_xyz: [f32; 3],
    /// Native XItem Euler lane +0xE0/+0xE4/+0xE8/+0xEC.
    pub owner_rotation_euler4: [f32; 4],
    /// Player/controller heading consumed from Handler +0xB30.
    pub controller_yaw_radians: f32,
    /// Native DAT_00620034. Standard fixed-rate runtime is 1.0.
    pub runtime_rate_scale: f32,
    pub stop: RobotsWatchbotMode3StopInput,
    /// Only consumed while internal state == 8.
    pub recovery_contact: Option<RobotsWatchbotMode3RecoveryContact>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsWatchbotMode3PrePhysicsStep {
    /// State transition requested before locomotion. Native state3 still executes
    /// vslot +0x7C in the same update after requesting state1.
    pub requested_internal_state: Option<u32>,
    pub owner_rotation_euler4: [f32; 4],
    /// Component +0x30..+0x3C after acceleration+damping. The host applies this
    /// through the existing physics/motion owner, matching native vslot +0x20.
    pub velocity_xyzw: [f32; 4],
    pub path_parameter: f32,
    pub sampled_path_position_xyz: [f32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsWatchbotMode3PostPhysicsStep {
    /// Component +0xF8 recomputed after host physics has applied velocity.
    pub path_step: f32,
}

/// Engine-neutral runtime for XItemHandler_WatchBot mode3 component
/// (ctor `0x00496860`, vtable `0x005ECF50`).
///
/// It intentionally does not own collision or XItem transforms. Native vslot
/// +0x7C calls the XItem motion vslot +0x20 mid-function, then reads the resulting
/// post-physics position before recomputing +0xF8. The split pre/post API keeps
/// that ownership boundary exact for MapFrame today and UE5.8 later.
#[derive(Debug, Clone, PartialEq)]
pub struct RobotsWatchbotMode3Runtime {
    spline: Option<RobotsNaturalCubicSpline3>,
    pub internal_state: u32,
    pub velocity_xyzw_30: [f32; 4],
    pub control_input: RobotsWatchbotMode3ControlInput,
    /// Component +0x64. Normal driving does not consume it in vslot +0x7C, but
    /// recovery vslot +0x88 writes it and other component paths can observe it.
    pub control_yaw_64: f32,
    /// Component +0x70. Base ctor `0x00493470` initializes this to 1.0; state8
    /// doubles input magnitude whenever it remains positive.
    pub state8_boost_70: f32,
    pub recovery_latched_94: bool,
    pub recovery_updates_95: u8,
    pub path_parameter_f4: f32,
    pub path_step_f8: f32,
}

impl Default for RobotsWatchbotMode3Runtime {
    fn default() -> Self {
        Self {
            spline: None,
            internal_state: ROBOTS_WATCHBOT_MODE3_STATE_IDLE,
            velocity_xyzw_30: [0.0; 4],
            control_input: RobotsWatchbotMode3ControlInput::default(),
            control_yaw_64: 0.0,
            state8_boost_70: 1.0,
            recovery_latched_94: false,
            recovery_updates_95: 0,
            path_parameter_f4: 0.0,
            path_step_f8: 0.0,
        }
    }
}

impl RobotsWatchbotMode3Runtime {
    pub fn path_bound(&self) -> bool {
        self.spline.is_some()
    }

    pub fn clear_path(&mut self) {
        self.spline = None;
        self.path_parameter_f4 = 0.0;
        self.path_step_f8 = 0.0;
    }

    /// Mode3 vslot +0x38 / `0x00496A30` path setup.
    ///
    /// Native chooses traversal direction from the nearest source node: first
    /// half keeps source order, second half reverses it. Start +0xF4 is then the
    /// nearest interior control-node index in [1, count-1), not a fractional
    /// nearest-spline projection.
    pub fn bind_path(
        &mut self,
        source_points: &[[f32; 3]],
        owner_position_xyz: [f32; 3],
        current_velocity_xyzw: [f32; 4],
        runtime_rate_scale: f32,
    ) -> bool {
        let Some(source) = RobotsNaturalCubicSpline3::from_points(source_points.to_vec()) else {
            self.clear_path();
            return false;
        };

        let nearest_source = source.nearest_node_index(owner_position_xyz);
        let mut ordered = source_points.to_vec();
        if nearest_source >= ordered.len() / 2 {
            ordered.reverse();
        }
        let Some(spline) = RobotsNaturalCubicSpline3::from_points(ordered) else {
            self.clear_path();
            return false;
        };

        let count = spline.points().len();
        self.path_parameter_f4 = nearest_interior_node_parameter(&spline, owner_position_xyz);
        self.path_step_f8 =
            runtime_rate_scale * ROBOTS_FIXED_STEP_SECONDS * length4(current_velocity_xyzw);
        self.velocity_xyzw_30 = current_velocity_xyzw;
        self.spline = Some(spline);
        self.internal_state = if count > 0 {
            ROBOTS_WATCHBOT_MODE3_STATE_DRIVE
        } else {
            ROBOTS_WATCHBOT_MODE3_STATE_IDLE
        };
        true
    }

    pub fn set_control_input(&mut self, input: RobotsWatchbotMode3ControlInput) {
        self.control_input = input;
        self.control_yaw_64 = input.direction_xz[0].atan2(input.direction_xz[1]);
    }

    pub fn set_internal_state(&mut self, state: u32) {
        self.internal_state = state;
    }

    fn stop_predicate(&self, input: RobotsWatchbotMode3StopInput) -> bool {
        watchbot_component_stop_predicate(self.velocity_xyzw_30, input)
    }

    fn mode3_stop_ready(&self, input: RobotsWatchbotMode3StopInput) -> bool {
        self.stop_predicate(input)
            && self.path_step_f8 < ROBOTS_WATCHBOT_MODE3_EPSILON
            && self.path_step_f8 > ROBOTS_WATCHBOT_MODE3_NEGATIVE_EPSILON
    }

    fn service_state8_recovery(&mut self, contact: Option<RobotsWatchbotMode3RecoveryContact>) {
        service_watchbot_component_recovery(
            &mut self.recovery_latched_94,
            &mut self.recovery_updates_95,
            &mut self.control_input.magnitude,
            &mut self.control_yaw_64,
            contact,
        );
    }

    /// First half of mode3 vslot +0x7C / `0x00496C60`, up to the native motion
    /// vslot +0x20 call. The host must apply returned velocity/rotation before
    /// calling `finish_after_physics()` with the resulting XItem position.
    pub fn fixed_pre_physics(
        &mut self,
        input: RobotsWatchbotMode3FixedInput,
    ) -> Option<RobotsWatchbotMode3PrePhysicsStep> {
        if self.spline.is_none() {
            return None;
        }
        let mut requested_internal_state = None;

        match self.internal_state {
            ROBOTS_WATCHBOT_MODE3_STATE_DRIVE => {
                if self.mode3_stop_ready(input.stop) {
                    self.internal_state = ROBOTS_WATCHBOT_MODE3_STATE_IDLE;
                    requested_internal_state = Some(ROBOTS_WATCHBOT_MODE3_STATE_IDLE);
                }
            }
            ROBOTS_WATCHBOT_MODE3_STATE_RECOVERY => {
                self.service_state8_recovery(input.recovery_contact);
            }
            _ => {}
        }

        let spline = self.spline.as_ref().expect("checked above");
        let mut effective_input_magnitude = self.control_input.magnitude;
        if self.internal_state == ROBOTS_WATCHBOT_MODE3_STATE_RECOVERY && self.state8_boost_70 > 0.0
        {
            effective_input_magnitude += effective_input_magnitude;
        }

        let owner_yaw = input.owner_rotation_euler4[1];
        let velocity_yaw = self.velocity_xyzw_30[0].atan2(self.velocity_xyzw_30[2]);
        let yaw_error = shortest_yaw_delta(owner_yaw, velocity_yaw);
        let horizontal_speed = (self.velocity_xyzw_30[0] * self.velocity_xyzw_30[0]
            + self.velocity_xyzw_30[2] * self.velocity_xyzw_30[2])
            .sqrt()
            .clamp(0.0, ROBOTS_WATCHBOT_MODE3_MAX_HORIZONTAL_SPEED);

        let mut owner_rotation_euler4 = input.owner_rotation_euler4;
        owner_rotation_euler4[1] = owner_yaw
            + yaw_error
                * effective_input_magnitude
                * ROBOTS_WATCHBOT_MODE3_YAW_INPUT_SCALE
                * input.runtime_rate_scale;
        owner_rotation_euler4[2] =
            -(horizontal_speed * ROBOTS_WATCHBOT_MODE3_STEERING_ROLL_SCALE * yaw_error);

        self.path_parameter_f4 = spline
            .advance_parameter_by_distance(self.path_parameter_f4, self.path_step_f8)
            .clamp(0.0, spline.last_parameter());
        let mut sampled = spline.sample(self.path_parameter_f4);

        let dx = sampled[0] - input.owner_position_xyz[0];
        let dy = sampled[1] - input.owner_position_xyz[1];
        let dz = sampled[2] - input.owner_position_xyz[2];
        let target_yaw = dx.atan2(dz);
        let horizontal_distance = (dx * dx + dz * dz).sqrt();
        let target_pitch = dy.atan2(horizontal_distance);
        let acceleration = effective_input_magnitude * ROBOTS_WATCHBOT_MODE3_ACCELERATION_SCALE;

        self.velocity_xyzw_30[0] += target_yaw.sin() * acceleration;
        self.velocity_xyzw_30[1] += target_pitch.sin() * acceleration;
        self.velocity_xyzw_30[2] += target_yaw.cos() * acceleration;

        let damping = if effective_input_magnitude >= ROBOTS_WATCHBOT_MODE3_EPSILON {
            ROBOTS_WATCHBOT_MODE3_MOVING_DAMPING
        } else {
            ROBOTS_WATCHBOT_MODE3_IDLE_DAMPING
        };
        for component in &mut self.velocity_xyzw_30 {
            *component *= damping;
        }

        if effective_input_magnitude < ROBOTS_WATCHBOT_MODE3_EPSILON {
            let lower = self.path_parameter_f4.trunc() as i16;
            self.path_parameter_f4 = spline.nearest_parameter_in_interval(
                input.owner_position_xyz,
                lower,
                lower.saturating_add(1),
            );
            sampled = spline.sample(self.path_parameter_f4);
        }

        if self.internal_state == ROBOTS_WATCHBOT_MODE3_STATE_RECOVERY && self.recovery_latched_94 {
            self.internal_state = ROBOTS_WATCHBOT_MODE3_STATE_DRIVE;
            requested_internal_state = Some(ROBOTS_WATCHBOT_MODE3_STATE_DRIVE);
        }

        Some(RobotsWatchbotMode3PrePhysicsStep {
            requested_internal_state,
            owner_rotation_euler4,
            velocity_xyzw: self.velocity_xyzw_30,
            path_parameter: self.path_parameter_f4,
            sampled_path_position_xyz: sampled,
        })
    }

    /// Native tail after component vslot +0x20 has applied motion. `0x00496C60`
    /// reads the resulting owner position here, then recomputes +0xF8 from spline
    /// tangent, Player/controller yaw and the retained X/Z input direction.
    pub fn finish_after_physics(
        &mut self,
        post_owner_position_xyz: [f32; 3],
        controller_yaw_radians: f32,
    ) -> Option<RobotsWatchbotMode3PostPhysicsStep> {
        let spline = self.spline.as_ref()?;
        let sampled = spline.sample(self.path_parameter_f4);
        let dx = sampled[0] - post_owner_position_xyz[0];
        let dz = sampled[2] - post_owner_position_xyz[2];
        if dx * dx + dz * dz <= ROBOTS_WATCHBOT_MODE3_PATH_STEP_RADIUS_SQUARED {
            let tangent =
                spline.finite_difference(self.path_parameter_f4, ROBOTS_XPATH_PARAMETER_STEP);
            let tangent_yaw = tangent[0].atan2(tangent[2]);
            let delta = shortest_yaw_delta(tangent_yaw, controller_yaw_radians);
            self.path_step_f8 = (delta.sin() * self.control_input.direction_xz[0]
                + delta.cos() * self.control_input.direction_xz[1])
                .clamp(-1.0, 1.0);
        } else {
            self.path_step_f8 = 0.0;
        }
        Some(RobotsWatchbotMode3PostPhysicsStep {
            path_step: self.path_step_f8,
        })
    }
}

fn nearest_interior_node_parameter(spline: &RobotsNaturalCubicSpline3, point: [f32; 3]) -> f32 {
    let points = spline.points();
    let mut best_index: i16 = -1;
    let mut best_distance = 999_999.0f32;
    for index in 1..points.len().saturating_sub(1) {
        let dx = point[0] - points[index][0];
        let dy = point[1] - points[index][1];
        let dz = point[2] - points[index][2];
        let distance = dx * dx + dy * dy + dz * dz;
        if distance < best_distance {
            best_distance = distance;
            best_index = index as i16;
        }
    }
    best_index as f32
}

fn length4(value: [f32; 4]) -> f32 {
    (value[0] * value[0] + value[1] * value[1] + value[2] * value[2] + value[3] * value[3]).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn straight_z_points() -> Vec<[f32; 3]> {
        vec![
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 10.0],
            [0.0, 0.0, 20.0],
            [0.0, 0.0, 30.0],
        ]
    }

    fn fixed_input(owner_position_xyz: [f32; 3]) -> RobotsWatchbotMode3FixedInput {
        RobotsWatchbotMode3FixedInput {
            owner_position_xyz,
            owner_rotation_euler4: [0.0; 4],
            controller_yaw_radians: 0.0,
            runtime_rate_scale: 1.0,
            stop: RobotsWatchbotMode3StopInput {
                owner_position_xyz,
                handler_target_xz: [999.0, 999.0],
                current_anim_mode_uid: 0x0900_0003,
            },
            recovery_contact: None,
        }
    }

    #[test]
    fn bind_reverses_second_half_source_and_starts_on_nearest_interior_control_node() {
        let points = straight_z_points();
        let mut runtime = RobotsWatchbotMode3Runtime::default();
        assert!(runtime.bind_path(&points, [0.0, 0.0, 29.0], [0.0; 4], 1.0));
        assert_eq!(runtime.internal_state, ROBOTS_WATCHBOT_MODE3_STATE_DRIVE);
        // Source reverses to 30,20,10,0 and native interior scan is indices 1..3.
        assert_eq!(runtime.path_parameter_f4, 1.0);
        assert_eq!(runtime.path_step_f8, 0.0);
    }

    #[test]
    fn state3_stop_request_still_executes_same_tick_locomotion() {
        let points = straight_z_points();
        let mut runtime = RobotsWatchbotMode3Runtime::default();
        assert!(runtime.bind_path(&points, [0.0, 0.0, 10.0], [0.0; 4], 1.0));
        runtime.set_control_input(RobotsWatchbotMode3ControlInput {
            direction_xz: [0.0, 1.0],
            magnitude: 1.0,
        });
        runtime.path_step_f8 = 0.0;
        let mut input = fixed_input([0.0, 0.0, 10.0]);
        input.stop.handler_target_xz = [0.0, 10.0];
        let step = runtime.fixed_pre_physics(input).unwrap();
        assert_eq!(
            step.requested_internal_state,
            Some(ROBOTS_WATCHBOT_MODE3_STATE_IDLE)
        );
        assert_eq!(runtime.internal_state, ROBOTS_WATCHBOT_MODE3_STATE_IDLE);
        // Locomotion still ran after state1 request and accelerated toward sampled path.
        assert!(step.velocity_xyzw[2] > 0.0);
    }

    #[test]
    fn state8_contact_latches_recovery_then_returns_to_state3_after_locomotion() {
        let points = straight_z_points();
        let mut runtime = RobotsWatchbotMode3Runtime::default();
        assert!(runtime.bind_path(&points, [0.0, 0.0, 10.0], [0.0; 4], 1.0));
        runtime.set_internal_state(ROBOTS_WATCHBOT_MODE3_STATE_RECOVERY);
        runtime.set_control_input(RobotsWatchbotMode3ControlInput {
            direction_xz: [0.0, 1.0],
            magnitude: 0.1,
        });
        let mut input = fixed_input([0.0, 0.0, 10.0]);
        input.recovery_contact = Some(RobotsWatchbotMode3RecoveryContact {
            special_contact: true,
            recovery_yaw_radians: 0.75,
        });
        let step = runtime.fixed_pre_physics(input).unwrap();
        assert!(runtime.recovery_latched_94);
        assert_eq!(
            runtime.control_input.magnitude,
            ROBOTS_WATCHBOT_MODE3_RECOVERY_INPUT_MAGNITUDE
        );
        assert_eq!(runtime.control_yaw_64, 0.75);
        assert_eq!(
            step.requested_internal_state,
            Some(ROBOTS_WATCHBOT_MODE3_STATE_DRIVE)
        );
        assert_eq!(runtime.internal_state, ROBOTS_WATCHBOT_MODE3_STATE_DRIVE);
        // State8 doubles the native 0.5 recovery magnitude to 1.0 before 0.25 accel,
        // then moving damping 0.95 is applied.
        assert!((step.velocity_xyzw[2] - 0.2375).abs() < 1.0e-6);
    }

    #[test]
    fn post_physics_tail_projects_input_onto_path_tangent_and_controller_heading() {
        let points = straight_z_points();
        let mut runtime = RobotsWatchbotMode3Runtime::default();
        assert!(runtime.bind_path(&points, [0.0, 0.0, 10.0], [0.0; 4], 1.0));
        runtime.path_parameter_f4 = 1.0;
        runtime.set_control_input(RobotsWatchbotMode3ControlInput {
            direction_xz: [0.0, 1.0],
            magnitude: 1.0,
        });
        let step = runtime.finish_after_physics([0.0, 0.0, 10.0], 0.0).unwrap();
        assert!((step.path_step - 1.0).abs() < 1.0e-6);

        let far = runtime.finish_after_physics([2.0, 0.0, 10.0], 0.0).unwrap();
        assert_eq!(far.path_step, 0.0);
    }

    #[test]
    fn stop_predicate_is_strict_and_blocks_three_native_animation_modes() {
        let mut runtime = RobotsWatchbotMode3Runtime::default();
        runtime.velocity_xyzw_30 = [0.0; 4];
        runtime.path_step_f8 = 0.0;
        let base = RobotsWatchbotMode3StopInput {
            owner_position_xyz: [1.0, 0.0, 2.0],
            handler_target_xz: [1.0, 2.0],
            current_anim_mode_uid: 0x0900_0003,
        };
        assert!(runtime.mode3_stop_ready(base));
        for mode in ROBOTS_WATCHBOT_MODE3_STOP_BLOCKED_ANIM_MODES {
            assert!(!runtime.mode3_stop_ready(RobotsWatchbotMode3StopInput {
                current_anim_mode_uid: mode,
                ..base
            }));
        }
        runtime.path_step_f8 = ROBOTS_WATCHBOT_MODE3_EPSILON;
        assert!(!runtime.mode3_stop_ready(base));
        runtime.path_step_f8 = ROBOTS_WATCHBOT_MODE3_NEGATIVE_EPSILON;
        assert!(!runtime.mode3_stop_ready(base));
    }
}
