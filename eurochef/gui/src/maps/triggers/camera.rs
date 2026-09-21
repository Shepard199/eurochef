use eurochef_shared::robots_runtime::path_spline::RobotsNaturalCubicSpline3;
use glam::Vec3;

use crate::maps::{ProcessedMap, ProcessedPath, ProcessedTrigger};

pub const TYPE: u32 = 1;
pub const MARKER_TYPE: u32 = 20;
pub const VALUES_TYPE: u32 = 35;
pub const SEQUENCE_TYPE: u32 = 85;

const NATIVE_TESTED_FLAG_MASK: u32 = 0x0000_03F7;
const MODE4_OPTION_FLAG_MASK: u32 = 0x0000_0057;
const NATIVE_FIXED_HZ: f32 = 60.0;
const NATIVE_FIXED_SECONDS: f32 = 1.0 / NATIVE_FIXED_HZ;
pub const NATIVE_FADE_IDLE_STATE: u8 = 0;
pub const NATIVE_FADE_IN_STATE: u8 = 1;
pub const NATIVE_FADE_OUT_STATE: u8 = 2;
pub const NATIVE_FADE_DEFAULT_UPDATES: u32 = 30;
pub const NATIVE_FADE_LOW_THRESHOLD: f32 = 0.001;
pub const NATIVE_FADE_HIGH_THRESHOLD: f32 = 0.999;
const MODE4_DEFAULT_DISTANCE_SCALE: f32 = 6.4;
const MODE4_TARGET_Y_OFFSET: f32 = 1.3;
const MODE4_DISTANCE_MULTIPLIER: f32 = 5.0;
const MODE4_PARAMETER_STEP: f32 = 0.1;
const MODE4_BANK_SAMPLE_COUNT: usize = 16;
const MODE4_BANK_SCALE: f32 = -1200.0;
const MODE4_SPECIAL_MAP_HASHCODE: u32 = 0x0100_0073;
const MODE4_SPECIAL_PATH_HASHCODE: u32 = 0x0B00_0001;
const MODE4_SPECIAL_TARGET_Y_OFFSET: f32 = 0.23;
const PLAYER_MODE1_PITCH_DEGREES: f32 = 11.0;
const PLAYER_MODE1_TARGET_Y_OFFSET: f32 = 1.3;
const PLAYER_MODE1_INTERPOLATION_RATE: f32 = 0.1;
const PLAYER_MODE1_DISTANCE: f32 = 6.4;
const PLAYER_MODE1_FOV_DEGREES: f32 = 45.0;
const PLAYER_TRACKER_UPWARD_LAG: f32 = 1.275;
const PLAYER_MODE1_CONTACT_ANGLE_STEP_RADIANS: f32 = 0.383_972_44;
const PLAYER_MODE1_CONTACT_DISTANCE_MARGIN: f32 = 0.75;
const PLAYER_MODE1_SIDE_EYE_OFFSET: f32 = 0.375;
const PLAYER_MODE1_SIDE_TARGET_SCALE: f32 = 0.8;
const PLAYER_MODE1_PLAYER_EXTENT: f32 = 0.4;
const PLAYER_MODE1_SIDE_VERTICAL_OFFSET: f32 = 1.0;
const PLAYER_MODE1_MOVEMENT_PROBE_THRESHOLD_SQ: f32 = 0.1;
const PLAYER_MODE1_TRACKER_MOVEMENT_THRESHOLD_SQ: f32 = 1.0e-5;
const PLAYER_MODE1_RECOVERY_ANGLE_EPSILON: f32 = 0.0001;
const PLAYER_MODE1_RECOVERY_NORMAL_STEP_DEGREES: f32 = 10.0;
const PLAYER_MODE1_RECOVERY_NORMAL_ACCEL_DEGREES: f32 = 5.0;
const PLAYER_MODE1_RECOVERY_VERTICAL_STEP_DEGREES: f32 = 45.0;
const PLAYER_MODE1_RECOVERY_VERTICAL_EYE_RAISE: f32 = 0.4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeCameraViewportPose {
    pub position: Vec3,
    pub target: Vec3,
    pub vertical_fov_degrees: f32,
    pub roll_degrees: f32,
}

impl NativeCameraViewportPose {
    pub fn is_finite(self) -> bool {
        self.position.is_finite()
            && self.target.is_finite()
            && self.vertical_fov_degrees.is_finite()
            && self.roll_degrees.is_finite()
    }
}

fn player_mode1_pose_from_target(
    target: Vec3,
    heading_radians: f32,
    pitch_radians: f32,
    distance: f32,
) -> NativeCameraViewportPose {
    let horizontal_distance = pitch_radians.cos() * distance;
    let yaw = heading_radians + std::f32::consts::PI;
    NativeCameraViewportPose {
        position: Vec3::new(
            target.x + yaw.sin() * horizontal_distance,
            target.y - pitch_radians.sin() * distance,
            target.z + yaw.cos() * horizontal_distance,
        ),
        target,
        vertical_fov_degrees: PLAYER_MODE1_FOV_DEGREES,
        roll_degrees: 0.0,
    }
}

pub fn default_player_mode1_pose(
    player_focus: Vec3,
    player_heading_radians: f32,
) -> NativeCameraViewportPose {
    player_mode1_pose_from_target(
        player_focus + Vec3::Y * PLAYER_MODE1_TARGET_Y_OFFSET,
        player_heading_radians,
        PLAYER_MODE1_PITCH_DEGREES.to_radians(),
        PLAYER_MODE1_DISTANCE,
    )
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeMode1ActivationContactResolution {
    pub pose: NativeCameraViewportPose,
    pub heading_radians: f32,
    pub distance: f32,
    pub used_distance_fallback: bool,
}

/// Exact control-flow/math of the mode-1 activation wall search at 0x004768B0.
///
/// `accepted_contact_t` must return the native accepted-contact closest fraction
/// in [0,1], or `None` when the segment is clear. The callback deliberately owns
/// the source/material predicate: Robots filters the hit source through
/// 0x00478E30/0x00478EF0, and that resource-level gate is not yet represented by
/// EuroChef's static Map triangles.
#[allow(dead_code)]
pub fn resolve_player_mode1_activation_contacts<F>(
    player_focus: Vec3,
    player_heading_radians: f32,
    mut accepted_contact_t: F,
) -> NativeMode1ActivationContactResolution
where
    F: FnMut(Vec3, Vec3) -> Option<f32>,
{
    let target = player_focus + Vec3::Y * PLAYER_MODE1_TARGET_Y_OFFSET;
    let pitch = PLAYER_MODE1_PITCH_DEGREES.to_radians();
    let mut offset = 0.0f32;
    let mut best_t = 0.0f32;
    let mut best_heading = player_heading_radians;

    loop {
        let heading = player_heading_radians + offset;
        let pose = player_mode1_pose_from_target(target, heading, pitch, PLAYER_MODE1_DISTANCE);
        match accepted_contact_t(target, pose.position) {
            None => {
                return NativeMode1ActivationContactResolution {
                    pose,
                    heading_radians: heading,
                    distance: PLAYER_MODE1_DISTANCE,
                    used_distance_fallback: false,
                };
            }
            Some(t) => {
                if best_t < t {
                    best_t = t;
                    best_heading = heading;
                }
            }
        }

        // Literal 0x00476A65..0x00476A89 update: negate the previous offset,
        // then grow its magnitude by 22 degrees on that side. This yields
        // 0,+22,-44,+66,-88,... rather than a symmetric +/-22,+/-44 sweep.
        offset = -offset;
        if offset >= 0.0 {
            offset += PLAYER_MODE1_CONTACT_ANGLE_STEP_RADIANS;
        } else {
            offset -= PLAYER_MODE1_CONTACT_ANGLE_STEP_RADIANS;
        }
        if offset >= std::f32::consts::PI {
            break;
        }
    }

    let distance = if best_t > 0.0 {
        PLAYER_MODE1_DISTANCE * best_t - PLAYER_MODE1_CONTACT_DISTANCE_MARGIN
    } else {
        PLAYER_MODE1_DISTANCE
    };
    NativeMode1ActivationContactResolution {
        pose: player_mode1_pose_from_target(target, best_heading, pitch, distance),
        heading_radians: best_heading,
        distance,
        used_distance_fallback: best_t > 0.0,
    }
}

#[allow(dead_code)]
pub fn player_mode1_single_side_contact_correction(
    eye: Vec3,
    target: Vec3,
    contact_t: f32,
    side_sign: f32,
) -> Vec3 {
    let mut view = target - eye;
    view.y = 0.0;
    if !view.is_finite() || view.length_squared() <= f32::EPSILON {
        return eye;
    }
    let view = view.normalize();
    let right = Vec3::new(-view.z, 0.0, view.x);
    let magnitude = (1.0 - contact_t) * PLAYER_MODE1_SIDE_EYE_OFFSET;
    eye + right * magnitude * side_sign.signum()
}

#[derive(Clone, Debug, PartialEq)]
struct NativeMode1ContactRuntime {
    initialized: bool,
    probe_positive_side: bool,
    positive_hit: bool,
    negative_hit: bool,
    positive_t: f32,
    negative_t: f32,
    movement_hit: bool,
    recovery_active: bool,
    recovery_vertical: bool,
    recovery_angle_radians: f32,
    recovery_step_degrees: f32,
    recovery_accel_degrees: f32,
    recovery_counter: u32,
}

impl Default for NativeMode1ContactRuntime {
    fn default() -> Self {
        Self {
            initialized: false,
            probe_positive_side: false,
            positive_hit: false,
            negative_hit: false,
            positive_t: 0.0,
            negative_t: 0.0,
            movement_hit: false,
            recovery_active: false,
            recovery_vertical: false,
            recovery_angle_radians: 0.0,
            recovery_step_degrees: 0.0,
            recovery_accel_degrees: 0.0,
            recovery_counter: 0,
        }
    }
}

fn player_mode1_heading_from_eye_target(eye: Vec3, target: Vec3) -> Option<f32> {
    let delta = target - eye;
    let horizontal = Vec3::new(delta.x, 0.0, delta.z);
    (horizontal.is_finite() && horizontal.length_squared() > f32::EPSILON)
        .then(|| horizontal.x.atan2(horizontal.z))
}

fn player_mode1_rotate_eye_about_target(eye: Vec3, target: Vec3, angle_radians: f32) -> Vec3 {
    let offset = eye - target;
    let (sin_angle, cos_angle) = angle_radians.sin_cos();
    Vec3::new(
        target.x + cos_angle * offset.x + sin_angle * offset.z,
        eye.y,
        target.z - sin_angle * offset.x + cos_angle * offset.z,
    )
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeDefaultPlayerCameraRuntime {
    pub current: NativeCameraViewportPose,
    pub desired: NativeCameraViewportPose,
    pub player_focus: Vec3,
    pub player_heading_radians: f32,
    player_position: Vec3,
    camera_heading_radians: f32,
    camera_distance: f32,
    activation_heading_lock: bool,
    contact: NativeMode1ContactRuntime,
}

impl NativeDefaultPlayerCameraRuntime {
    pub fn new(
        player_position: Vec3,
        player_heading_radians: f32,
        previous_camera: Option<NativeCameraViewportPose>,
    ) -> Self {
        let desired = default_player_mode1_pose(player_position, player_heading_radians);
        let current = previous_camera
            .filter(|pose| pose.is_finite())
            .unwrap_or(desired);
        Self {
            current,
            desired,
            player_focus: player_position,
            player_heading_radians,
            player_position,
            camera_heading_radians: player_heading_radians,
            camera_distance: PLAYER_MODE1_DISTANCE,
            activation_heading_lock: false,
            contact: NativeMode1ContactRuntime::default(),
        }
    }

    fn update_player_tracker(
        &mut self,
        player_position: Vec3,
        player_heading_radians: f32,
    ) -> Vec3 {
        let movement = player_position - self.player_position;
        self.player_position = player_position;
        self.player_focus.x = player_position.x;
        self.player_focus.z = player_position.z;
        let delta_y = player_position.y - self.player_focus.y;
        if delta_y < 0.0 {
            self.player_focus.y = player_position.y;
        } else if delta_y > PLAYER_TRACKER_UPWARD_LAG {
            self.player_focus.y = player_position.y - PLAYER_TRACKER_UPWARD_LAG;
        }
        self.player_heading_radians = player_heading_radians;
        movement
    }

    #[cfg(test)]
    pub fn update_player_pose(&mut self, player_position: Vec3, player_heading_radians: f32) {
        let movement = self.update_player_tracker(player_position, player_heading_radians);
        let target = self.player_focus + Vec3::Y * PLAYER_MODE1_TARGET_Y_OFFSET;
        if movement.length_squared() > PLAYER_MODE1_TRACKER_MOVEMENT_THRESHOLD_SQ {
            self.activation_heading_lock = false;
        }
        if !self.activation_heading_lock {
            self.camera_heading_radians =
                player_mode1_heading_from_eye_target(self.desired.position, target)
                    .unwrap_or(self.camera_heading_radians);
        }
        self.desired = player_mode1_pose_from_target(
            target,
            self.camera_heading_radians,
            PLAYER_MODE1_PITCH_DEGREES.to_radians(),
            self.camera_distance,
        );
    }

    /// Reproduces the ordinary zero-input mode-1 static Map contact path:
    /// activation sweep 0x004768B0, per-frame probes 0x004775D0, correction
    /// 0x00477C10 and the normal recovery search 0x00477FC0. The callback uses
    /// `None` for unresolved geometry, `Some(None)` for clear, and
    /// `Some(Some(t))` for the nearest native-accepted contact fraction.
    pub fn update_player_pose_with_contacts<F>(
        &mut self,
        player_position: Vec3,
        player_heading_radians: f32,
        mut accepted_contact_t: F,
    ) -> bool
    where
        F: FnMut(Vec3, Vec3) -> Option<Option<f32>>,
    {
        let snapshot = self.clone();
        let movement = self.update_player_tracker(player_position, player_heading_radians);
        let target = self.player_focus + Vec3::Y * PLAYER_MODE1_TARGET_Y_OFFSET;
        let moved = movement.length_squared() > PLAYER_MODE1_TRACKER_MOVEMENT_THRESHOLD_SQ;
        if moved {
            self.activation_heading_lock = false;
        }

        if !self.contact.initialized {
            let mut unresolved = false;
            let activation = resolve_player_mode1_activation_contacts(
                self.player_focus,
                player_heading_radians,
                |start, end| match accepted_contact_t(start, end) {
                    Some(hit) => hit,
                    None => {
                        unresolved = true;
                        Some(0.0)
                    }
                },
            );
            if unresolved {
                *self = snapshot;
                return false;
            }
            self.desired = activation.pose;
            self.camera_heading_radians = activation.heading_radians;
            self.camera_distance = activation.distance;
            self.activation_heading_lock = true;
            self.contact.initialized = true;
        } else {
            if !self.activation_heading_lock {
                self.camera_heading_radians =
                    player_mode1_heading_from_eye_target(self.desired.position, target)
                        .unwrap_or(self.camera_heading_radians);
            }
            self.desired = player_mode1_pose_from_target(
                target,
                self.camera_heading_radians,
                PLAYER_MODE1_PITCH_DEGREES.to_radians(),
                self.camera_distance,
            );
        }

        self.contact.movement_hit = false;
        if self
            .current
            .position
            .distance_squared(self.desired.position)
            > PLAYER_MODE1_MOVEMENT_PROBE_THRESHOLD_SQ
        {
            match accepted_contact_t(self.current.position, self.desired.position) {
                Some(hit) => self.contact.movement_hit = hit.is_some(),
                None => {
                    *self = snapshot;
                    return false;
                }
            }
        }

        let mut horizontal_view = self.desired.target - self.desired.position;
        horizontal_view.y = 0.0;
        if horizontal_view.is_finite() && horizontal_view.length_squared() > f32::EPSILON {
            let view = horizontal_view.normalize();
            let right = Vec3::new(-view.z, 0.0, view.x);
            let side = if self.contact.probe_positive_side {
                1.0
            } else {
                -1.0
            };
            let mut eye_probe =
                self.desired.position + right * (PLAYER_MODE1_SIDE_EYE_OFFSET * side);
            let mut target_probe = self.desired.target
                + right * (PLAYER_MODE1_PLAYER_EXTENT * PLAYER_MODE1_SIDE_TARGET_SCALE * side);
            eye_probe.y -= PLAYER_MODE1_SIDE_VERTICAL_OFFSET;
            target_probe.y -= PLAYER_MODE1_SIDE_VERTICAL_OFFSET;
            let hit = match accepted_contact_t(target_probe, eye_probe) {
                Some(hit) => hit,
                None => {
                    *self = snapshot;
                    return false;
                }
            };
            if self.contact.probe_positive_side {
                self.contact.positive_hit = hit.is_some();
                self.contact.positive_t = hit.unwrap_or_default();
            } else {
                self.contact.negative_hit = hit.is_some();
                self.contact.negative_t = hit.unwrap_or_default();
            }
            self.contact.probe_positive_side = !self.contact.probe_positive_side;
        }

        if self.contact.positive_hit || self.contact.negative_hit || self.contact.movement_hit {
            self.activation_heading_lock = false;
        }

        if (!self.contact.positive_hit || !self.contact.negative_hit) && !self.contact.movement_hit
        {
            if self.contact.recovery_active
                && !self.contact.positive_hit
                && !self.contact.negative_hit
            {
                self.contact.recovery_active = false;
                self.contact.recovery_vertical = false;
            }
            if self.contact.positive_hit {
                self.desired.position = player_mode1_single_side_contact_correction(
                    self.desired.position,
                    self.desired.target,
                    self.contact.positive_t,
                    -1.0,
                );
            } else if self.contact.negative_hit {
                self.desired.position = player_mode1_single_side_contact_correction(
                    self.desired.position,
                    self.desired.target,
                    self.contact.negative_t,
                    1.0,
                );
            }
        } else if !self.contact.recovery_active {
            self.contact.recovery_active = true;
            self.contact.recovery_vertical =
                movement.x * movement.x + movement.z * movement.z < movement.y * movement.y;
            if self.contact.recovery_vertical {
                self.contact.recovery_step_degrees = PLAYER_MODE1_RECOVERY_VERTICAL_STEP_DEGREES;
                self.contact.recovery_accel_degrees = 0.0;
            } else {
                self.contact.recovery_step_degrees = PLAYER_MODE1_RECOVERY_NORMAL_STEP_DEGREES;
                self.contact.recovery_accel_degrees = PLAYER_MODE1_RECOVERY_NORMAL_ACCEL_DEGREES;
            }
            self.contact.recovery_angle_radians = if self.contact.recovery_angle_radians < 0.0 {
                PLAYER_MODE1_RECOVERY_ANGLE_EPSILON
            } else {
                -PLAYER_MODE1_RECOVERY_ANGLE_EPSILON
            };
            self.contact.recovery_counter = 1;
        }

        if self.contact.recovery_active {
            let attempts = if self.contact.recovery_vertical { 4 } else { 1 };
            for _ in 0..attempts {
                if self.contact.recovery_angle_radians.abs() > std::f32::consts::PI {
                    break;
                }
                let saved_angle = self.contact.recovery_angle_radians;
                let saved_step = self.contact.recovery_step_degrees;
                let saved_counter = self.contact.recovery_counter;

                self.contact.recovery_angle_radians = -self.contact.recovery_angle_radians;
                self.contact.recovery_counter += 1;
                if self.contact.recovery_counter == 2 {
                    let step = self.contact.recovery_step_degrees.to_radians();
                    if self.contact.recovery_angle_radians <= 0.0 {
                        self.contact.recovery_angle_radians -= step;
                    } else {
                        self.contact.recovery_angle_radians += step;
                    }
                    self.contact.recovery_counter = 0;
                    self.contact.recovery_step_degrees += self.contact.recovery_accel_degrees;
                }

                let mut candidate = player_mode1_rotate_eye_about_target(
                    self.desired.position,
                    self.desired.target,
                    self.contact.recovery_angle_radians,
                );
                if self.contact.recovery_vertical {
                    let horizontal = Vec3::new(
                        self.desired.position.x - self.desired.target.x,
                        0.0,
                        self.desired.position.z - self.desired.target.z,
                    )
                    .length();
                    candidate.y += horizontal * PLAYER_MODE1_RECOVERY_VERTICAL_EYE_RAISE;
                }
                let hit = match accepted_contact_t(self.desired.target, candidate) {
                    Some(hit) => hit,
                    None => {
                        *self = snapshot;
                        return false;
                    }
                };
                if hit.is_none() {
                    self.contact.recovery_angle_radians = saved_angle;
                    self.contact.recovery_step_degrees = saved_step;
                    self.contact.recovery_counter = saved_counter;
                    self.desired.position = candidate;
                    break;
                }
            }
        }

        self.camera_heading_radians =
            player_mode1_heading_from_eye_target(self.desired.position, self.desired.target)
                .unwrap_or(self.camera_heading_radians);
        true
    }

    pub fn is_transitioning(&self) -> bool {
        self.current
            .position
            .distance_squared(self.desired.position)
            > 1.0e-8
            || self.current.target.distance_squared(self.desired.target) > 1.0e-8
            || (self.current.vertical_fov_degrees - self.desired.vertical_fov_degrees).abs()
                > 1.0e-4
            || (self.current.roll_degrees - self.desired.roll_degrees).abs() > 1.0e-4
    }

    pub fn advance(&mut self, delta_seconds: f32) {
        let factor = (PLAYER_MODE1_INTERPOLATION_RATE * delta_seconds.max(0.0) * NATIVE_FIXED_HZ)
            .clamp(0.0, 1.0);
        if factor <= 0.0 {
            return;
        }
        if factor >= 1.0 {
            self.current = self.desired;
            return;
        }
        self.current.position = self.current.position.lerp(self.desired.position, factor);
        self.current.target = self.current.target.lerp(self.desired.target, factor);
        self.current.vertical_fov_degrees +=
            (self.desired.vertical_fov_degrees - self.current.vertical_fov_degrees) * factor;
        self.current.roll_degrees +=
            (self.desired.roll_degrees - self.current.roll_degrees) * factor;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeCameraViewportBoundary {
    MissingLinkedMarker,
    Mode1ControllerStateUnresolved,
    Mode2ControllerStateUnresolved,
    Mode4PathTraversalUnresolved,
    UnknownMode(u32),
}

impl NativeCameraViewportBoundary {
    pub fn description(self) -> &'static str {
        match self {
            Self::MissingLinkedMarker => "linked Camera Marker is missing",
            Self::Mode1ControllerStateUnresolved => {
                "mode 1 yaw is decoded, but its gameplay controller position is unresolved"
            }
            Self::Mode2ControllerStateUnresolved => {
                "mode 2 gameplay controller position/target is unresolved"
            }
            Self::Mode4PathTraversalUnresolved => {
                "mode 4 path is missing or is not the shipped XPath_Spline class"
            }
            Self::UnknownMode(_) => "unknown native Camera mode",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeMode4Spline {
    points: Vec<Vec3>,
    shared: RobotsNaturalCubicSpline3,
}

impl NativeMode4Spline {
    pub(crate) fn from_path(path: &ProcessedPath) -> Option<Self> {
        if path.path_type != 0 || path.flags & 0x2000_0000 == 0 || path.nodes.is_empty() {
            return None;
        }

        Self::from_points(path.nodes.iter().map(|node| node.position).collect())
    }

    pub(crate) fn from_points(points: Vec<Vec3>) -> Option<Self> {
        let shared = RobotsNaturalCubicSpline3::from_points(
            points.iter().map(|point| point.to_array()).collect(),
        )?;
        Some(Self { points, shared })
    }

    pub(crate) fn last_parameter(&self) -> f32 {
        self.shared.last_parameter()
    }

    pub(crate) fn sample(&self, parameter: f32) -> Vec3 {
        Vec3::from_array(self.shared.sample(parameter))
    }

    pub(crate) fn finite_difference(&self, parameter: f32, width: f32) -> Vec3 {
        Vec3::from_array(self.shared.finite_difference(parameter, width))
    }

    fn nearest_node_index(&self, point: Vec3) -> usize {
        self.shared.nearest_node_index(point.to_array())
    }

    pub(crate) fn advance_parameter_by_distance(&self, parameter: f32, distance: f32) -> f32 {
        self.shared
            .advance_parameter_by_distance(parameter, distance)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct NativeMode4Runtime {
    spline: NativeMode4Spline,
    map_hashcode: u32,
    path_hashcode: u32,
    parameter: f32,
    cached_point: Vec3,
    direction_scalar: f32,
    distance_scale: f32,
    tangent_target: bool,
    bank_enabled: bool,
    fixed_start_tangent: bool,
    last_target_yaw: f32,
    bank_samples: [f32; MODE4_BANK_SAMPLE_COUNT],
    bank_cursor: usize,
    bank_sum: f32,
}

impl NativeMode4Runtime {
    fn activation_direction_scalar(
        spline: &NativeMode4Spline,
        player_anchor: Vec3,
        current: NativeCameraViewportPose,
    ) -> f32 {
        let index = spline.nearest_node_index(player_anchor);
        let (from, to) = if index + 1 < spline.points.len() {
            (spline.points[index], spline.points[index + 1])
        } else if index > 0 {
            (spline.points[index - 1], spline.points[index])
        } else {
            return 1.0;
        };
        let path_direction = (to - from).normalize_or_zero();
        let camera_direction = (current.target - current.position).normalize_or_zero();
        if path_direction.dot(camera_direction) <= 0.0 {
            1.0
        } else {
            -1.0
        }
    }

    fn new(
        map: &ProcessedMap,
        path: &ProcessedPath,
        plan: NativeCameraControllerPlan,
        current: NativeCameraViewportPose,
        player_anchor: Vec3,
    ) -> Option<Self> {
        let spline = NativeMode4Spline::from_path(path)?;
        let nearest = spline.nearest_node_index(player_anchor);
        let direction_scalar = if plan.mode4_option_flags & 0x02 != 0 {
            Self::activation_direction_scalar(&spline, player_anchor, current)
        } else {
            -plan
                .mode4_data7
                .unwrap_or_default()
                .round_ties_even()
                .clamp(i16::MIN as f32, i16::MAX as f32)
        };
        let distance_scale = plan
            .mode4_data6
            .filter(|value| *value > 0.0)
            .unwrap_or(MODE4_DEFAULT_DISTANCE_SCALE);
        let parameter = nearest as f32;
        let cached_point = spline.sample(parameter);
        Some(Self {
            spline,
            map_hashcode: map.hashcode,
            path_hashcode: path.hashcode,
            parameter,
            cached_point,
            direction_scalar,
            distance_scale,
            tangent_target: plan.mode4_option_flags & 0x10 != 0,
            bank_enabled: plan.mode4_option_flags & 0x04 != 0,
            fixed_start_tangent: plan.mode4_option_flags & 0x40 != 0,
            last_target_yaw: 0.0,
            bank_samples: [0.0; MODE4_BANK_SAMPLE_COUNT],
            bank_cursor: 0,
            bank_sum: 0.0,
        })
    }

    fn solve_position(&mut self, player_anchor: Vec3, previous_desired: Vec3) -> Vec3 {
        if self.direction_scalar == 0.0 {
            let tangent = self
                .spline
                .finite_difference(self.parameter, MODE4_PARAMETER_STEP)
                * (1.0 / MODE4_PARAMETER_STEP);
            let tangent_len_sq = tangent.length_squared();
            if tangent_len_sq > f32::EPSILON {
                self.parameter += (player_anchor - self.cached_point).dot(tangent) / tangent_len_sq;
            }
            self.parameter = self.parameter.clamp(0.0, self.spline.last_parameter());
            self.cached_point = self.spline.sample(self.parameter);
            return self.cached_point;
        }

        let tangent_parameter = if self.fixed_start_tangent {
            0.0
        } else {
            self.parameter
        };
        let tangent = self
            .spline
            .finite_difference(tangent_parameter, 0.25)
            .normalize_or_zero();
        if tangent.length_squared() <= f32::EPSILON {
            return self.spline.sample(self.parameter);
        }
        let plane_normal = -tangent;
        let actual_distance = plane_normal.dot(player_anchor - previous_desired);
        let wanted_distance =
            MODE4_DISTANCE_MULTIPLIER * self.distance_scale * self.direction_scalar;
        self.parameter = self
            .spline
            .advance_parameter_by_distance(self.parameter, wanted_distance - actual_distance);
        self.cached_point = self.spline.sample(self.parameter);
        self.cached_point
    }

    fn solve_target(&self, player_anchor: Vec3, desired_position: Vec3) -> Vec3 {
        if self.map_hashcode == MODE4_SPECIAL_MAP_HASHCODE
            && self.path_hashcode == MODE4_SPECIAL_PATH_HASHCODE
        {
            return Vec3::new(
                desired_position.x,
                desired_position.y - MODE4_SPECIAL_TARGET_Y_OFFSET,
                desired_position.z + 1.0,
            );
        }
        if self.tangent_target {
            let tangent = self
                .spline
                .finite_difference(self.parameter, 1.0)
                .normalize_or_zero();
            let distance = -MODE4_DISTANCE_MULTIPLIER * self.distance_scale * self.direction_scalar;
            return desired_position + tangent * distance;
        }
        player_anchor + Vec3::Y * MODE4_TARGET_Y_OFFSET
    }

    fn wrapped_angle_delta(previous: f32, current: f32) -> f32 {
        let mut delta = current - previous;
        while delta > std::f32::consts::PI {
            delta -= std::f32::consts::TAU;
        }
        while delta < -std::f32::consts::PI {
            delta += std::f32::consts::TAU;
        }
        delta
    }

    fn update_bank(&mut self, desired_position: Vec3, desired_target: Vec3) -> f32 {
        if !self.bank_enabled {
            return 0.0;
        }
        let delta = desired_target - desired_position;
        let yaw = delta.x.atan2(delta.z);
        let sample = Self::wrapped_angle_delta(self.last_target_yaw, yaw) * MODE4_BANK_SCALE;
        self.last_target_yaw = yaw;
        self.bank_sum -= self.bank_samples[self.bank_cursor];
        self.bank_samples[self.bank_cursor] = sample;
        self.bank_sum += sample;
        self.bank_cursor = (self.bank_cursor + 1) % MODE4_BANK_SAMPLE_COUNT;
        self.bank_sum / MODE4_BANK_SAMPLE_COUNT as f32
    }

    fn activate_pose(
        &mut self,
        current: NativeCameraViewportPose,
        player_anchor: Vec3,
    ) -> NativeCameraViewportPose {
        let mut position = self.solve_position(player_anchor, current.position);
        for _ in 0..10 {
            let previous = position;
            position = self.solve_position(player_anchor, position);
            if previous.distance_squared(position) <= 0.01 {
                break;
            }
        }
        position = self.solve_position(player_anchor, position);
        let target = self.solve_target(player_anchor, position);
        let roll_degrees = self.update_bank(position, target);
        NativeCameraViewportPose {
            position,
            target,
            vertical_fov_degrees: 45.0,
            roll_degrees,
        }
    }

    fn update_pose(
        &mut self,
        previous_desired: NativeCameraViewportPose,
        player_anchor: Vec3,
    ) -> NativeCameraViewportPose {
        let position = self.solve_position(player_anchor, previous_desired.position);
        let target = self.solve_target(player_anchor, position);
        let roll_degrees = self.update_bank(position, target);
        NativeCameraViewportPose {
            position,
            target,
            vertical_fov_degrees: 45.0,
            roll_degrees,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeCameraViewportRuntime {
    pub trigger_index: usize,
    pub mode: u32,
    pub current: NativeCameraViewportPose,
    pub desired: NativeCameraViewportPose,
    pub interpolation_rate: f32,
    pub interpolating: bool,
    pub player_anchor: Vec3,
    pub boundary: Option<NativeCameraViewportBoundary>,
    mode4: Option<NativeMode4Runtime>,
}

impl NativeCameraViewportRuntime {
    pub fn advance(&mut self, delta_seconds: f32) {
        if (!self.interpolating && self.mode != 4) || self.boundary.is_some() {
            return;
        }

        // Robots.exe FUN_00474030 multiplies the active mode profile rate by
        // DAT_00620034, the engine's fixed-60-Hz frame-step scalar. It snaps
        // only when that product reaches one or more; otherwise every lane is
        // the same current -> desired linear interpolation.
        let factor = self.interpolation_rate * delta_seconds.max(0.0) * NATIVE_FIXED_HZ;
        if factor >= 1.0 {
            self.current = self.desired;
            return;
        }
        if factor <= 0.0 {
            return;
        }

        self.current.position = self.current.position.lerp(self.desired.position, factor);
        self.current.target = self.current.target.lerp(self.desired.target, factor);
        self.current.vertical_fov_degrees +=
            (self.desired.vertical_fov_degrees - self.current.vertical_fov_degrees) * factor;
        self.current.roll_degrees +=
            (self.desired.roll_degrees - self.current.roll_degrees) * factor;

        if self
            .current
            .position
            .distance_squared(self.desired.position)
            <= 1.0e-8
            && self.current.target.distance_squared(self.desired.target) <= 1.0e-8
            && (self.current.vertical_fov_degrees - self.desired.vertical_fov_degrees).abs()
                <= 1.0e-4
            && (self.current.roll_degrees - self.desired.roll_degrees).abs() <= 1.0e-4
        {
            self.current = self.desired;
        }
    }

    pub fn update_dynamic_pose(&mut self, player_anchor: Vec3) {
        self.player_anchor = player_anchor;
        if self.boundary.is_some() {
            return;
        }
        match self.mode {
            3 => {
                // Mode-3 vfunc 0x00478CD0 reads the live player XYZ every time it
                // builds the desired target, adds the profile Y offset and calls
                // SetTarget. SetTarget copies desired -> current when the native
                // controller has interpolation disabled.
                self.desired.target = player_anchor + Vec3::Y * 1.3;
                if !self.interpolating {
                    self.current.target = self.desired.target;
                }
            }
            4 => {
                if let Some(mode4) = self.mode4.as_mut() {
                    self.desired = mode4.update_pose(self.desired, player_anchor);
                }
            }
            _ => {}
        }
    }

    pub fn mode4_parameter(&self) -> Option<f32> {
        self.mode4.as_ref().map(|runtime| runtime.parameter)
    }

    pub fn mode4_direction_scalar(&self) -> Option<f32> {
        self.mode4.as_ref().map(|runtime| runtime.direction_scalar)
    }

    pub fn is_transitioning(&self) -> bool {
        (self.interpolating || self.mode == 4)
            && self.boundary.is_none()
            && (self.current.position != self.desired.position
                || self.current.target != self.desired.target
                || self.current.vertical_fov_degrees != self.desired.vertical_fov_degrees
                || self.current.roll_degrees != self.desired.roll_degrees)
    }
}

fn native_mode_profile(mode: u32) -> Option<(f32, f32, f32)> {
    // Camera-controller configuration records constructed by FUN_00471B60.
    // Tuple: (interpolation rate, target Y offset, vertical FOV degrees).
    Some(match mode {
        0 => (0.04, 0.0, 45.0),
        1 => (0.10, 0.0, 45.0),
        2 => (0.075, 0.0, 45.0),
        3 => (0.08, 1.3, 60.0),
        4 => (0.08, MODE4_TARGET_Y_OFFSET, 45.0),
        _ => return None,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeCameraSetupKind {
    Mode0DualPoint,
    Mode1MarkerYaw,
    Mode2Generic,
    Mode3MarkerPosition,
    Mode4Path,
    Unknown(u32),
}

impl NativeCameraSetupKind {
    pub fn description(self) -> &'static str {
        match self {
            Self::Mode0DualPoint => "mode 0: marker/camera point setup",
            Self::Mode1MarkerYaw => "mode 1: marker-derived yaw setup",
            Self::Mode2Generic => "mode 2: generic controller setup",
            Self::Mode3MarkerPosition => "mode 3: marker-position setup",
            Self::Mode4Path => "mode 4: path-controller setup",
            Self::Unknown(_) => "unknown mode: generic controller setup",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeCameraControllerPlan {
    pub trigger_index: usize,
    pub mode: u32,
    pub setup_kind: NativeCameraSetupKind,
    pub camera_position: Vec3,
    pub flags: u32,
    pub native_tested_flags: u32,
    pub controller_data3_raw: u32,
    pub scaled_data4: Option<f32>,
    pub scaled_data5: Option<f32>,
    pub linked_marker_index: Option<usize>,
    pub linked_marker_position: Option<Vec3>,
    pub mode1_yaw_radians: Option<f32>,
    pub mode3_preserve_current_camera_y: bool,
    pub mode3_preserve_current_camera_xz: bool,
    pub path_hashcode: Option<u32>,
    pub mode4_data6: Option<f32>,
    pub mode4_data7: Option<f32>,
    pub mode4_option_flags: u32,
    pub controller_flag_0x1: bool,
    pub controller_flag_0x20: bool,
    pub lifecycle_flag_0x80: bool,
}

pub fn path_hash(data: &[Option<u32>]) -> Option<u32> {
    data.get(1).copied().flatten()
}

pub fn mode(data: &[Option<u32>]) -> Option<u32> {
    data.first().copied().flatten()
}

pub fn scaled_data4(data: &[Option<u32>]) -> Option<f32> {
    data.get(4)
        .copied()
        .flatten()
        .map(|value| value as i32 as f32 * 0.1)
}

pub fn scaled_data5(data: &[Option<u32>]) -> Option<f32> {
    data.get(5)
        .copied()
        .flatten()
        .map(|value| value as i32 as f32 * 0.1)
}

pub fn flags(data: &[Option<u32>]) -> Option<u32> {
    data.get(2).copied().flatten()
}

fn data_float(data: &[Option<u32>], slot: usize) -> Option<f32> {
    let value = f32::from_bits(data.get(slot).copied().flatten()?);
    value.is_finite().then_some(value)
}

fn linked_marker<'a>(
    map: &'a ProcessedMap,
    camera: &ProcessedTrigger,
) -> Option<(usize, &'a ProcessedTrigger)> {
    camera
        .links
        .iter()
        .take(8)
        .filter_map(|link| usize::try_from(*link).ok())
        .filter_map(|index| map.triggers.get(index).map(|trigger| (index, trigger)))
        .find(|(_, trigger)| trigger.ttype == MARKER_TYPE)
}

fn first_linked_trigger_of_type(
    map: &ProcessedMap,
    trigger: &ProcessedTrigger,
    trigger_type: u32,
) -> Option<usize> {
    trigger
        .links
        .iter()
        .take(8)
        .filter_map(|link| usize::try_from(*link).ok())
        .find(|index| {
            map.triggers
                .get(*index)
                .is_some_and(|linked| linked.ttype == trigger_type)
        })
}

pub fn native_camera_owner_proxy_index(map: &ProcessedMap, camera_index: usize) -> Option<usize> {
    let camera = map.triggers.get(camera_index)?;
    if camera.ttype != TYPE {
        return None;
    }
    match mode(&camera.data)? {
        0 | 3 => first_linked_trigger_of_type(map, camera, MARKER_TYPE),
        _ => Some(camera_index),
    }
}

pub fn native_camera_directly_owns_controller(
    map: &ProcessedMap,
    camera_index: usize,
    current_mode: u32,
    current_owner_index: Option<usize>,
    current_path_hashcode: Option<u32>,
) -> bool {
    let Some(camera) = map.triggers.get(camera_index) else {
        return false;
    };
    if camera.ttype != TYPE {
        return false;
    }
    let Some(camera_mode) = mode(&camera.data) else {
        return false;
    };
    if camera_mode != current_mode {
        return false;
    }
    match camera_mode {
        0 | 3 => current_owner_index == first_linked_trigger_of_type(map, camera, MARKER_TYPE),
        4 => {
            let expected = path_hash(&camera.data).unwrap_or(0x0B00_0000);
            current_path_hashcode.unwrap_or(0x0B00_0000) == expected
        }
        _ => current_owner_index == Some(camera_index),
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NativeCameraOwnershipRuntime {
    pub owner_index: Option<usize>,
    pub nesting_count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeCameraOwnershipChange {
    Acquired {
        owner_index: usize,
        nesting_count: u32,
    },
    Released {
        owner_index: Option<usize>,
        nesting_count: u32,
    },
}

impl NativeCameraOwnershipRuntime {
    pub fn activate(
        &mut self,
        map: &ProcessedMap,
        camera_index: usize,
    ) -> Option<NativeCameraOwnershipChange> {
        let owner_index = native_camera_owner_proxy_index(map, camera_index)?;
        self.owner_index = Some(owner_index);
        self.nesting_count = self.nesting_count.saturating_add(1);
        Some(NativeCameraOwnershipChange::Acquired {
            owner_index,
            nesting_count: self.nesting_count,
        })
    }

    pub fn release(
        &mut self,
        map: &ProcessedMap,
        camera_index: usize,
        current_mode: u32,
        current_path_hashcode: Option<u32>,
    ) -> Option<NativeCameraOwnershipChange> {
        if !native_camera_directly_owns_controller(
            map,
            camera_index,
            current_mode,
            self.owner_index,
            current_path_hashcode,
        ) {
            return None;
        }
        self.nesting_count = self.nesting_count.saturating_sub(1);
        if self.nesting_count == 0 {
            self.owner_index = None;
        }
        Some(NativeCameraOwnershipChange::Released {
            owner_index: self.owner_index,
            nesting_count: self.nesting_count,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeGameWndFadeRuntime {
    pub state: u8,
    pub duration_updates: u32,
    pub progress: f32,
}

impl Default for NativeGameWndFadeRuntime {
    fn default() -> Self {
        Self {
            state: NATIVE_FADE_IDLE_STATE,
            duration_updates: NATIVE_FADE_DEFAULT_UPDATES,
            progress: 1.0,
        }
    }
}

impl NativeGameWndFadeRuntime {
    pub fn request(&mut self, state: u8, duration_updates: u32) {
        self.state = state;
        self.duration_updates = if duration_updates == 0 {
            NATIVE_FADE_DEFAULT_UPDATES
        } else {
            duration_updates
        };
    }

    pub fn fixed_update(&mut self) {
        let step = 1.0 / self.duration_updates.max(1) as f32;
        match self.state {
            NATIVE_FADE_IDLE_STATE => self.progress = 1.0,
            NATIVE_FADE_IN_STATE => {
                self.progress = (self.progress + step).min(1.0);
                if self.progress > NATIVE_FADE_HIGH_THRESHOLD {
                    self.progress = 1.0;
                    self.state = NATIVE_FADE_IDLE_STATE;
                }
            }
            NATIVE_FADE_OUT_STATE => {
                self.progress = (self.progress - step).max(0.0);
            }
            _ => {}
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NativeCameraShakeRuntime {
    pub magnitude: i32,
    pub remaining_updates: u32,
    pub primary_channel: bool,
    pub player_channel: bool,
    pub request_count: u32,
}

impl NativeCameraShakeRuntime {
    pub fn request(
        &mut self,
        magnitude: i32,
        duration_updates: i32,
        primary_channel: bool,
        player_channel: bool,
    ) -> bool {
        // Native 0x00433970 rejects zero low-byte magnitude and zero duration.
        if magnitude as u8 == 0 || duration_updates <= 0 {
            return false;
        }
        self.magnitude = magnitude;
        self.remaining_updates = duration_updates as u32;
        self.primary_channel = primary_channel;
        self.player_channel = player_channel;
        self.request_count = self.request_count.wrapping_add(1);
        true
    }

    pub fn fixed_update(&mut self) {
        self.remaining_updates = self.remaining_updates.saturating_sub(1);
    }

    #[cfg(test)]
    pub fn active(&self) -> bool {
        self.remaining_updates != 0 && (self.primary_channel || self.player_channel)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NativeCameraFadeTransitionRuntime {
    pub fade: NativeGameWndFadeRuntime,
    pub controller_transition_pending: bool,
}

impl NativeCameraFadeTransitionRuntime {
    pub fn set_camera_mode(&mut self, transition_flag: u32) {
        if transition_flag == 1 {
            self.controller_transition_pending = true;
            self.fade
                .request(NATIVE_FADE_OUT_STATE, NATIVE_FADE_DEFAULT_UPDATES);
        }
    }

    pub fn fixed_update_fade(&mut self) {
        self.fade.fixed_update();
    }

    pub fn fixed_update_camera(&mut self) {
        if self.controller_transition_pending
            && self.fade.state == NATIVE_FADE_OUT_STATE
            && self.fade.progress < NATIVE_FADE_LOW_THRESHOLD
        {
            self.controller_transition_pending = false;
            self.fade
                .request(NATIVE_FADE_IN_STATE, NATIVE_FADE_DEFAULT_UPDATES);
        }
    }

    #[cfg(test)]
    pub fn fixed_update(&mut self) {
        self.fixed_update_fade();
        self.fixed_update_camera();
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeCameraSequenceEmission {
    pub sequence_index: usize,
    pub slot: usize,
    pub target_trigger_index: usize,
    pub duration_seconds: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeCameraSequenceRuntime {
    pub sequence_index: usize,
    pub state: u32,
    pub cursor: u8,
    pub remaining_seconds: f32,
}

fn native_camera_sequence_emission(
    map: &ProcessedMap,
    sequence_index: usize,
    slot: usize,
) -> Option<NativeCameraSequenceEmission> {
    if slot >= 6 {
        return None;
    }
    let sequence = map.triggers.get(sequence_index)?;
    if sequence.ttype != SEQUENCE_TYPE {
        return None;
    }
    let target_trigger_index = usize::try_from(*sequence.links.get(slot)?).ok()?;
    map.triggers.get(target_trigger_index)?;
    let duration_seconds = f32::from_bits(sequence.data.get(slot).copied().flatten()?);
    duration_seconds
        .is_finite()
        .then_some(NativeCameraSequenceEmission {
            sequence_index,
            slot,
            target_trigger_index,
            duration_seconds,
        })
}

impl NativeCameraSequenceRuntime {
    pub fn idle(sequence_index: usize) -> Self {
        Self {
            sequence_index,
            state: 0,
            cursor: 0,
            remaining_seconds: 0.0,
        }
    }

    pub fn start(
        map: &ProcessedMap,
        sequence_index: usize,
    ) -> Option<(Self, NativeCameraSequenceEmission)> {
        let emission = native_camera_sequence_emission(map, sequence_index, 0)?;
        Some((
            Self {
                sequence_index,
                state: 1,
                cursor: 0,
                remaining_seconds: emission.duration_seconds,
            },
            emission,
        ))
    }

    pub fn auto_start_if_player_near(
        &mut self,
        map: &ProcessedMap,
        player_position: Vec3,
    ) -> Option<NativeCameraSequenceEmission> {
        if self.state != 0 {
            return None;
        }
        let sequence = map.triggers.get(self.sequence_index)?;
        if sequence.ttype != SEQUENCE_TYPE {
            return None;
        }
        let radius_raw = sequence.data.get(6).copied().flatten()? as i32 as f32;
        let radius_squared = radius_raw * radius_raw * f32::from_bits(0x3C23_D70B);
        if player_position.distance_squared(sequence.position) >= radius_squared {
            return None;
        }
        let (runtime, emission) = Self::start(map, self.sequence_index)?;
        *self = runtime;
        Some(emission)
    }

    pub fn fixed_update(
        &mut self,
        map: &ProcessedMap,
        native_fade_state: u8,
    ) -> Option<NativeCameraSequenceEmission> {
        match self.state {
            1 if matches!(
                native_fade_state,
                NATIVE_FADE_IDLE_STATE | NATIVE_FADE_IN_STATE
            ) =>
            {
                self.state = 2;
                None
            }
            2 => {
                self.remaining_seconds -= NATIVE_FIXED_SECONDS;
                if self.remaining_seconds < 0.0 {
                    self.cursor = self.cursor.saturating_add(1);
                    if let Some(emission) = native_camera_sequence_emission(
                        map,
                        self.sequence_index,
                        usize::from(self.cursor),
                    ) {
                        self.remaining_seconds = emission.duration_seconds;
                        Some(emission)
                    } else {
                        let waits_for_fade_in = map
                            .triggers
                            .get(self.sequence_index)
                            .and_then(|sequence| sequence.data.get(7).copied().flatten())
                            .is_some_and(|flags| flags & 1 != 0);
                        self.state = if waits_for_fade_in { 3 } else { 0 };
                        None
                    }
                } else {
                    None
                }
            }
            3 if native_fade_state == NATIVE_FADE_IN_STATE => {
                self.state = 0;
                None
            }
            _ => None,
        }
    }
}

pub fn controller_plan(
    map: &ProcessedMap,
    trigger_index: usize,
) -> Option<NativeCameraControllerPlan> {
    let camera = map.triggers.get(trigger_index)?;
    if camera.ttype != TYPE {
        return None;
    }

    let mode = mode(&camera.data)?;
    let flags = flags(&camera.data).unwrap_or_default();
    let marker = linked_marker(map, camera);
    let marker_position = marker.map(|(_, marker)| marker.position);
    // Mode 1 at 0x00481029..0x0048104E computes
    // atan2(camera.x-marker.x, camera.z-marker.z) + 2*pi.
    let mode1_yaw_radians = if mode == 1 {
        marker_position.map(|position| {
            let delta = camera.position - position;
            delta.x.atan2(delta.z) + std::f32::consts::TAU
        })
    } else {
        None
    };

    Some(NativeCameraControllerPlan {
        trigger_index,
        mode,
        setup_kind: match mode {
            0 => NativeCameraSetupKind::Mode0DualPoint,
            1 => NativeCameraSetupKind::Mode1MarkerYaw,
            2 => NativeCameraSetupKind::Mode2Generic,
            3 => NativeCameraSetupKind::Mode3MarkerPosition,
            4 => NativeCameraSetupKind::Mode4Path,
            value => NativeCameraSetupKind::Unknown(value),
        },
        camera_position: camera.position,
        flags,
        native_tested_flags: flags & NATIVE_TESTED_FLAG_MASK,
        controller_data3_raw: camera.data.get(3).copied().flatten().unwrap_or_default(),
        scaled_data4: scaled_data4(&camera.data),
        scaled_data5: scaled_data5(&camera.data),
        linked_marker_index: marker.map(|(index, _)| index),
        linked_marker_position: marker_position,
        mode1_yaw_radians,
        mode3_preserve_current_camera_y: mode == 3 && flags & 0x100 != 0,
        mode3_preserve_current_camera_xz: mode == 3 && flags & 0x200 != 0,
        path_hashcode: (mode == 4)
            .then(|| path_hash(&camera.data))
            .flatten()
            .filter(|hashcode| !matches!(*hashcode, 0 | u32::MAX | 0x0B00_0000)),
        mode4_data6: (mode == 4).then(|| data_float(&camera.data, 6)).flatten(),
        mode4_data7: (mode == 4).then(|| data_float(&camera.data, 7)).flatten(),
        mode4_option_flags: if mode == 4 {
            flags & MODE4_OPTION_FLAG_MASK
        } else {
            0
        },
        controller_flag_0x1: flags & 0x1 != 0,
        controller_flag_0x20: flags & 0x20 != 0,
        lifecycle_flag_0x80: flags & 0x80 != 0,
    })
}

pub fn viewport_runtime(
    map: &ProcessedMap,
    plan: NativeCameraControllerPlan,
    current: NativeCameraViewportPose,
    player_anchor: Vec3,
) -> NativeCameraViewportRuntime {
    let (interpolation_rate, player_target_y_offset, vertical_fov_degrees) =
        native_mode_profile(plan.mode).unwrap_or((0.0, 0.0, current.vertical_fov_degrees));
    let mut desired = current;
    desired.vertical_fov_degrees = vertical_fov_degrees;
    let mut mode4 = None;

    let boundary = match plan.mode {
        0 => match plan.linked_marker_position {
            Some(marker) => {
                desired.position = marker;
                desired.target = plan.camera_position;
                None
            }
            None => Some(NativeCameraViewportBoundary::MissingLinkedMarker),
        },
        1 => Some(NativeCameraViewportBoundary::Mode1ControllerStateUnresolved),
        2 => Some(NativeCameraViewportBoundary::Mode2ControllerStateUnresolved),
        3 => match plan.linked_marker_position {
            Some(marker) => {
                desired.position = marker;
                if plan.mode3_preserve_current_camera_y {
                    desired.position.y = current.position.y;
                }
                if plan.mode3_preserve_current_camera_xz {
                    desired.position.x = current.position.x;
                    desired.position.z = current.position.z;
                }
                desired.target = player_anchor + Vec3::Y * player_target_y_offset;
                None
            }
            None => Some(NativeCameraViewportBoundary::MissingLinkedMarker),
        },
        4 => {
            let native_runtime = plan
                .path_hashcode
                .and_then(|hashcode| map.paths.iter().find(|path| path.hashcode == hashcode))
                .and_then(|path| NativeMode4Runtime::new(map, path, plan, current, player_anchor));
            match native_runtime {
                Some(mut runtime) => {
                    desired = runtime.activate_pose(current, player_anchor);
                    mode4 = Some(runtime);
                    None
                }
                None => Some(NativeCameraViewportBoundary::Mode4PathTraversalUnresolved),
            }
        }
        value => Some(NativeCameraViewportBoundary::UnknownMode(value)),
    };

    let interpolating = plan.controller_flag_0x1;
    NativeCameraViewportRuntime {
        trigger_index: plan.trigger_index,
        mode: plan.mode,
        current: if interpolating || boundary.is_some() {
            current
        } else {
            desired
        },
        desired,
        interpolation_rate,
        interpolating,
        player_anchor,
        boundary,
        mode4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::maps::ProcessedPathNode;
    use eurochef_edb::map::EXGeoTriggerEngineOptions;
    use glam::Vec2;

    fn trigger(
        trigger_type: u32,
        position: Vec3,
        data: Vec<Option<u32>>,
        links: Vec<i32>,
    ) -> ProcessedTrigger {
        ProcessedTrigger {
            file_offset: 0,
            link_ref: -1,
            type_index: 0,
            ttype: trigger_type,
            tsubtype: None,
            debug: 0,
            game_flags: 0,
            trig_flags: 0,
            position,
            rotation: Vec3::ZERO,
            scale: Vec3::ONE,
            data,
            links,
            engine_options: EXGeoTriggerEngineOptions::default(),
            trigger_script: None,
            character_visual: None,
            incoming_links: vec![],
        }
    }

    fn camera_data(mode: u32, flags: u32) -> Vec<Option<u32>> {
        let mut data = vec![None; 16];
        data[0] = Some(mode);
        data[2] = Some(flags);
        data
    }

    fn sequence_data(durations: &[f32]) -> Vec<Option<u32>> {
        let mut data = vec![None; 16];
        for (slot, duration) in durations.iter().copied().take(6).enumerate() {
            data[slot] = Some(duration.to_bits());
        }
        data
    }

    fn current_pose() -> NativeCameraViewportPose {
        NativeCameraViewportPose {
            position: Vec3::new(20.0, 8.0, -4.0),
            target: Vec3::new(21.0, 8.0, -4.0),
            vertical_fov_degrees: 90.0,
            roll_degrees: 0.0,
        }
    }

    fn spline_path(hashcode: u32, points: &[Vec3]) -> ProcessedPath {
        ProcessedPath {
            hashcode,
            position: Vec3::ZERO,
            flags: 0x2000_0000,
            path_type: 0,
            nodes: points
                .iter()
                .copied()
                .map(|position| ProcessedPathNode {
                    position,
                    size: Vec2::ZERO,
                    value: [0; 4],
                    flags: 0,
                    distance: 0.0,
                    num_links: 0,
                })
                .collect(),
            links: vec![],
        }
    }

    #[test]
    fn default_player_mode1_pose_matches_native_profile_and_spherical_solver() {
        let player = Vec3::new(10.0, 2.0, 30.0);
        let forward_z = default_player_mode1_pose(player, 0.0);
        assert!((forward_z.target - Vec3::new(10.0, 3.3, 30.0)).length() < 1.0e-5);
        assert!((forward_z.position.x - 10.0).abs() < 1.0e-5);
        assert!((forward_z.position.y - 2.078_822_4).abs() < 1.0e-5);
        assert!((forward_z.position.z - 23.717_587).abs() < 1.0e-5);
        assert_eq!(forward_z.vertical_fov_degrees, 45.0);
        assert_eq!(forward_z.roll_degrees, 0.0);

        let forward_x = default_player_mode1_pose(player, std::f32::consts::FRAC_PI_2);
        assert!((forward_x.position.x - 3.717_586).abs() < 1.0e-5);
        assert!((forward_x.position.z - 30.0).abs() < 1.0e-5);
    }

    #[test]
    fn default_player_mode1_runtime_tracks_native_player_focus_and_fixed_step() {
        let mut runtime = NativeDefaultPlayerCameraRuntime::new(Vec3::ZERO, 0.0, None);
        runtime.update_player_pose(Vec3::new(2.0, 0.5, 3.0), 0.0);
        assert_eq!(runtime.player_focus, Vec3::new(2.0, 0.0, 3.0));

        runtime.update_player_pose(Vec3::new(2.0, 2.0, 3.0), 0.0);
        assert!((runtime.player_focus.y - 0.725).abs() < 1.0e-6);
        runtime.update_player_pose(Vec3::new(2.0, -1.0, 3.0), 0.0);
        assert_eq!(runtime.player_focus.y, -1.0);

        let before = runtime.current;
        runtime.advance(1.0 / 60.0);
        assert!(runtime.is_transitioning());
        let expected = before.position.lerp(runtime.desired.position, 0.1);
        assert!(runtime.current.position.distance(expected) < 1.0e-5);
    }

    #[test]
    fn mode1_activation_contact_search_matches_native_signed_22_degree_sweep() {
        let mut headings = Vec::new();
        let result = resolve_player_mode1_activation_contacts(Vec3::ZERO, 0.0, |target, eye| {
            let raw = (eye.x - target.x).atan2(eye.z - target.z) - std::f32::consts::PI;
            headings.push(raw.sin().atan2(raw.cos()));
            (headings.len() != 3).then_some(0.25)
        });

        assert_eq!(headings.len(), 3);
        assert!(headings[0].abs() < 1.0e-5);
        assert!((headings[1] - 22.0_f32.to_radians()).abs() < 1.0e-5);
        assert!((headings[2] + 44.0_f32.to_radians()).abs() < 1.0e-5);
        assert!((result.heading_radians + 44.0_f32.to_radians()).abs() < 1.0e-5);
        assert_eq!(result.distance, PLAYER_MODE1_DISTANCE);
        assert!(!result.used_distance_fallback);
    }

    #[test]
    fn mode1_activation_contact_search_uses_best_fraction_distance_fallback() {
        let mut probe = 0usize;
        let result = resolve_player_mode1_activation_contacts(Vec3::ZERO, 0.0, |_target, _eye| {
            let t = if probe == 1 { 0.8 } else { 0.2 };
            probe += 1;
            Some(t)
        });

        assert_eq!(probe, 9);
        assert!((result.heading_radians - 22.0_f32.to_radians()).abs() < 1.0e-5);
        assert!((result.distance - (6.4 * 0.8 - 0.75)).abs() < 1.0e-6);
        assert!(result.used_distance_fallback);
    }

    #[test]
    fn mode1_single_side_contact_correction_matches_native_lateral_nudge() {
        let eye = Vec3::new(0.0, 0.0, -6.0);
        let target = Vec3::ZERO;
        let corrected = player_mode1_single_side_contact_correction(eye, target, 0.5, 1.0);
        assert!((corrected - Vec3::new(-0.1875, 0.0, -6.0)).length() < 1.0e-6);
        let opposite = player_mode1_single_side_contact_correction(eye, target, 0.5, -1.0);
        assert!((opposite - Vec3::new(0.1875, 0.0, -6.0)).length() < 1.0e-6);
    }

    #[test]
    fn mode1_runtime_alternates_native_side_probes_with_rodney_extent() {
        let mut runtime = NativeDefaultPlayerCameraRuntime::new(Vec3::ZERO, 0.0, None);
        let mut probes = Vec::new();
        assert!(
            runtime.update_player_pose_with_contacts(Vec3::ZERO, 0.0, |start, end| {
                probes.push((start, end));
                Some(None)
            })
        );
        assert_eq!(probes.len(), 2, "activation + first alternating side probe");
        let (negative_start, negative_end) = probes[1];
        assert!((negative_start.x - 0.32).abs() < 1.0e-5);
        assert!((negative_end.x - 0.375).abs() < 1.0e-5);
        assert!((negative_start.y - 0.3).abs() < 1.0e-5);
        assert!((negative_end.y - (runtime.desired.position.y - 1.0)).abs() < 1.0e-5);

        probes.clear();
        assert!(
            runtime.update_player_pose_with_contacts(Vec3::ZERO, 0.0, |start, end| {
                probes.push((start, end));
                Some(None)
            })
        );
        assert_eq!(
            probes.len(),
            1,
            "second frame probes only the opposite side"
        );
        let (positive_start, positive_end) = probes[0];
        assert!((positive_start.x + 0.32).abs() < 1.0e-5);
        assert!((positive_end.x + 0.375).abs() < 1.0e-5);
    }

    #[test]
    fn mode1_runtime_heading_lock_ignores_turn_in_place_and_releases_on_translation() {
        let mut runtime = NativeDefaultPlayerCameraRuntime::new(Vec3::ZERO, 0.0, None);
        assert!(runtime
            .update_player_pose_with_contacts(Vec3::ZERO, 0.0, |_start, _end| { Some(None) }));
        let locked_heading = runtime.camera_heading_radians;
        let locked_eye = runtime.desired.position;

        assert!(runtime.update_player_pose_with_contacts(
            Vec3::ZERO,
            std::f32::consts::FRAC_PI_2,
            |_start, _end| Some(None),
        ));
        assert!((runtime.camera_heading_radians - locked_heading).abs() < 1.0e-6);
        assert!(runtime.desired.position.distance(locked_eye) < 1.0e-6);

        let moved = Vec3::new(1.0, 0.0, 0.0);
        let moved_target = moved + Vec3::Y * PLAYER_MODE1_TARGET_Y_OFFSET;
        let expected_heading =
            player_mode1_heading_from_eye_target(runtime.desired.position, moved_target).unwrap();
        assert!(runtime.update_player_pose_with_contacts(
            moved,
            std::f32::consts::FRAC_PI_2,
            |_start, _end| Some(None),
        ));
        assert!((runtime.camera_heading_radians - expected_heading).abs() < 1.0e-5);
        assert!((runtime.camera_heading_radians - std::f32::consts::FRAC_PI_2).abs() > 0.1);
    }

    #[test]
    fn mode1_runtime_blocked_contacts_advance_native_recovery_search_state() {
        let mut runtime = NativeDefaultPlayerCameraRuntime::new(Vec3::ZERO, 0.0, None);
        assert!(runtime
            .update_player_pose_with_contacts(Vec3::ZERO, 0.0, |_start, _end| { Some(Some(0.5)) }));
        assert!(runtime.contact.recovery_active);
        assert!(!runtime.contact.recovery_vertical);
        assert!(
            (runtime.contact.recovery_angle_radians
                - (10.0_f32.to_radians() + PLAYER_MODE1_RECOVERY_ANGLE_EPSILON))
                .abs()
                < 1.0e-5
        );
        assert!((runtime.contact.recovery_step_degrees - 15.0).abs() < 1.0e-6);
        assert_eq!(runtime.contact.recovery_counter, 0);

        assert!(runtime
            .update_player_pose_with_contacts(Vec3::ZERO, 0.0, |_start, _end| { Some(Some(0.5)) }));
        assert!(
            (runtime.contact.recovery_angle_radians
                + (10.0_f32.to_radians() + PLAYER_MODE1_RECOVERY_ANGLE_EPSILON))
                .abs()
                < 1.0e-5
        );
        assert_eq!(runtime.contact.recovery_counter, 1);
    }

    #[test]
    fn mode1_runtime_rolls_back_contact_state_when_map_geometry_is_unresolved() {
        let mut runtime = NativeDefaultPlayerCameraRuntime::new(Vec3::ZERO, 0.0, None);
        let before = runtime.clone();
        assert!(!runtime.update_player_pose_with_contacts(Vec3::ZERO, 0.0, |_start, _end| None));
        assert_eq!(runtime, before);
    }

    #[test]
    fn controller_plan_resolves_the_first_marker_in_the_native_eight_link_window() {
        let camera = trigger(
            TYPE,
            Vec3::new(10.0, 2.0, 4.0),
            camera_data(0, 0x21),
            vec![2, 1, -1, -1, -1, -1, -1, -1, 3],
        );
        let wrong_type = trigger(35, Vec3::ZERO, vec![None; 16], vec![]);
        let marker = trigger(
            MARKER_TYPE,
            Vec3::new(3.0, 5.0, 7.0),
            vec![None; 16],
            vec![],
        );
        let outside_window_marker = trigger(
            MARKER_TYPE,
            Vec3::new(100.0, 100.0, 100.0),
            vec![None; 16],
            vec![],
        );
        let map = ProcessedMap {
            triggers: vec![camera, wrong_type, marker, outside_window_marker],
            ..Default::default()
        };

        let plan = controller_plan(&map, 0).unwrap();
        assert_eq!(plan.setup_kind, NativeCameraSetupKind::Mode0DualPoint);
        assert_eq!(plan.linked_marker_index, Some(2));
        assert_eq!(plan.linked_marker_position, Some(Vec3::new(3.0, 5.0, 7.0)));
        assert!(plan.controller_flag_0x1);
        assert!(plan.controller_flag_0x20);
    }

    #[test]
    fn mode_zero_uses_linked_marker_as_controller_proxy() {
        let camera = trigger(TYPE, Vec3::ZERO, camera_data(0, 0), vec![1]);
        let marker = trigger(MARKER_TYPE, Vec3::ZERO, vec![None; 16], vec![]);
        let map = ProcessedMap {
            triggers: vec![camera, marker],
            ..Default::default()
        };
        assert_eq!(native_camera_owner_proxy_index(&map, 0), Some(1));
        assert!(native_camera_directly_owns_controller(
            &map,
            0,
            0,
            Some(1),
            None
        ));
    }

    #[test]
    fn ownership_runtime_tracks_marker_proxy_and_native_nesting() {
        let camera = trigger(TYPE, Vec3::ZERO, camera_data(0, 0), vec![1]);
        let marker = trigger(MARKER_TYPE, Vec3::ZERO, vec![None; 16], vec![]);
        let map = ProcessedMap {
            triggers: vec![camera, marker],
            ..Default::default()
        };
        let mut runtime = NativeCameraOwnershipRuntime::default();

        assert_eq!(
            runtime.activate(&map, 0),
            Some(NativeCameraOwnershipChange::Acquired {
                owner_index: 1,
                nesting_count: 1,
            })
        );
        assert_eq!(
            runtime.activate(&map, 0),
            Some(NativeCameraOwnershipChange::Acquired {
                owner_index: 1,
                nesting_count: 2,
            })
        );
        assert_eq!(
            runtime.release(&map, 0, 0, None),
            Some(NativeCameraOwnershipChange::Released {
                owner_index: Some(1),
                nesting_count: 1,
            })
        );
        assert_eq!(
            runtime.release(&map, 0, 0, None),
            Some(NativeCameraOwnershipChange::Released {
                owner_index: None,
                nesting_count: 0,
            })
        );
    }

    #[test]
    fn ownership_runtime_rejects_non_owner_release() {
        let mut data = camera_data(4, 0);
        data[1] = Some(0x0B00_0012);
        let map = ProcessedMap {
            triggers: vec![trigger(TYPE, Vec3::ZERO, data, vec![])],
            ..Default::default()
        };
        let mut runtime = NativeCameraOwnershipRuntime::default();
        runtime.activate(&map, 0).unwrap();

        assert_eq!(runtime.release(&map, 0, 4, Some(0x0B00_0013)), None);
        assert_eq!(runtime.owner_index, Some(0));
        assert_eq!(runtime.nesting_count, 1);
        assert!(runtime.release(&map, 0, 4, Some(0x0B00_0012)).is_some());
        assert_eq!(runtime, NativeCameraOwnershipRuntime::default());
    }

    #[test]
    fn mode_one_plan_matches_native_marker_yaw_plus_tau() {
        let camera = trigger(TYPE, Vec3::new(1.0, 0.0, 0.0), camera_data(1, 1), vec![1]);
        let marker = trigger(MARKER_TYPE, Vec3::ZERO, vec![None; 16], vec![]);
        let map = ProcessedMap {
            triggers: vec![camera, marker],
            ..Default::default()
        };

        let yaw = controller_plan(&map, 0).unwrap().mode1_yaw_radians.unwrap();
        assert!((yaw - (std::f32::consts::TAU + std::f32::consts::FRAC_PI_2)).abs() < 0.0001);
    }

    #[test]
    fn mode_four_path_match_is_exact() {
        let mut data = camera_data(4, 0);
        data[1] = Some(0x0B00_0012);
        let camera = trigger(TYPE, Vec3::ZERO, data, vec![]);
        let map = ProcessedMap {
            triggers: vec![camera],
            ..Default::default()
        };
        assert!(native_camera_directly_owns_controller(
            &map,
            0,
            4,
            None,
            Some(0x0B00_0012)
        ));
        assert!(!native_camera_directly_owns_controller(
            &map,
            0,
            4,
            None,
            Some(0x0B00_0013)
        ));
    }

    #[test]
    fn mode_three_plan_preserves_current_camera_axis_bits() {
        let camera = trigger(TYPE, Vec3::ZERO, camera_data(3, 0x300), vec![]);
        let map = ProcessedMap {
            triggers: vec![camera],
            ..Default::default()
        };
        let plan = controller_plan(&map, 0).unwrap();
        assert!(plan.mode3_preserve_current_camera_y);
        assert!(plan.mode3_preserve_current_camera_xz);
    }

    #[test]
    fn sequence_idle_proximity_uses_native_data6_tenths_scale() {
        let mut data = sequence_data(&[0.25]);
        data[6] = Some(100);
        let map = ProcessedMap {
            triggers: vec![
                trigger(SEQUENCE_TYPE, Vec3::ZERO, data, vec![1]),
                trigger(TYPE, Vec3::ZERO, camera_data(0, 0), vec![]),
            ],
            ..Default::default()
        };

        let mut outside = NativeCameraSequenceRuntime::idle(0);
        assert_eq!(
            outside.auto_start_if_player_near(&map, Vec3::new(10.01, 0.0, 0.0)),
            None
        );
        assert_eq!(outside.state, 0);

        let mut inside = NativeCameraSequenceRuntime::idle(0);
        let emission = inside
            .auto_start_if_player_near(&map, Vec3::new(9.99, 0.0, 0.0))
            .unwrap();
        assert_eq!(emission.target_trigger_index, 1);
        assert_eq!(inside.state, 1);
        assert_eq!(inside.cursor, 0);
        assert!((inside.remaining_seconds - 0.25).abs() < f32::EPSILON);
    }

    #[test]
    fn sequence_uses_seconds_and_advances_to_the_next_link() {
        let sequence = trigger(
            SEQUENCE_TYPE,
            Vec3::ZERO,
            sequence_data(&[0.5, 0.25]),
            vec![1, 2],
        );
        let first_camera = trigger(TYPE, Vec3::ZERO, camera_data(0, 0), vec![]);
        let second_camera = trigger(TYPE, Vec3::ZERO, camera_data(1, 0), vec![]);
        let map = ProcessedMap {
            triggers: vec![sequence, first_camera, second_camera],
            ..Default::default()
        };

        let (mut runtime, first) = NativeCameraSequenceRuntime::start(&map, 0).unwrap();
        assert_eq!(runtime.state, 1);
        assert_eq!(first.target_trigger_index, 1);
        assert!((first.duration_seconds - 0.5).abs() < f32::EPSILON);
        assert_eq!(runtime.fixed_update(&map, 0), None);
        assert_eq!(runtime.state, 2);

        let mut next = None;
        for _ in 0..31 {
            next = runtime.fixed_update(&map, 0);
            if next.is_some() {
                break;
            }
        }
        let next = next.unwrap();
        assert_eq!(next.slot, 1);
        assert_eq!(next.target_trigger_index, 2);
        assert!((next.duration_seconds - 0.25).abs() < f32::EPSILON);
    }

    #[test]
    fn sequence_completion_enters_native_fade_wait_only_when_data7_bit0_is_set() {
        let mut no_wait_data = sequence_data(&[0.0]);
        no_wait_data[7] = Some(0);
        let no_wait_map = ProcessedMap {
            triggers: vec![
                trigger(SEQUENCE_TYPE, Vec3::ZERO, no_wait_data, vec![1]),
                trigger(TYPE, Vec3::ZERO, camera_data(0, 0), vec![]),
            ],
            ..Default::default()
        };
        let (mut no_wait, _) = NativeCameraSequenceRuntime::start(&no_wait_map, 0).unwrap();
        assert_eq!(
            no_wait.fixed_update(&no_wait_map, NATIVE_FADE_IDLE_STATE),
            None
        );
        assert_eq!(no_wait.state, 2);
        assert_eq!(
            no_wait.fixed_update(&no_wait_map, NATIVE_FADE_IDLE_STATE),
            None
        );
        assert_eq!(no_wait.state, 0);

        let mut wait_data = sequence_data(&[0.0]);
        wait_data[7] = Some(1);
        let wait_map = ProcessedMap {
            triggers: vec![
                trigger(SEQUENCE_TYPE, Vec3::ZERO, wait_data, vec![1]),
                trigger(TYPE, Vec3::ZERO, camera_data(0, 0), vec![]),
            ],
            ..Default::default()
        };
        let (mut wait, _) = NativeCameraSequenceRuntime::start(&wait_map, 0).unwrap();
        assert_eq!(wait.fixed_update(&wait_map, NATIVE_FADE_IDLE_STATE), None);
        assert_eq!(wait.state, 2);
        assert_eq!(wait.fixed_update(&wait_map, NATIVE_FADE_IDLE_STATE), None);
        assert_eq!(wait.state, 3);
        assert_eq!(wait.fixed_update(&wait_map, NATIVE_FADE_IN_STATE), None);
        assert_eq!(wait.state, 0);
    }

    #[test]
    fn native_camera_fade_transition_matches_set_camera_mode_and_update_handshake() {
        let mut runtime = NativeCameraFadeTransitionRuntime::default();
        runtime.set_camera_mode(1);
        assert_eq!(runtime.fade.state, NATIVE_FADE_OUT_STATE);
        assert_eq!(runtime.fade.duration_updates, 30);
        assert!(runtime.controller_transition_pending);

        for _ in 0..29 {
            runtime.fixed_update();
            assert_eq!(runtime.fade.state, NATIVE_FADE_OUT_STATE);
            assert!(runtime.controller_transition_pending);
        }
        runtime.fixed_update();
        assert_eq!(runtime.fade.state, NATIVE_FADE_IN_STATE);
        assert!(!runtime.controller_transition_pending);
        assert!(runtime.fade.progress < NATIVE_FADE_LOW_THRESHOLD);

        for _ in 0..30 {
            runtime.fixed_update();
        }
        assert_eq!(runtime.fade.state, NATIVE_FADE_IDLE_STATE);
        assert_eq!(runtime.fade.progress, 1.0);
    }

    #[test]
    fn native_camera_fade_transition_ignores_non_transitioning_set_camera_mode() {
        let mut runtime = NativeCameraFadeTransitionRuntime::default();
        runtime.set_camera_mode(0);
        assert_eq!(runtime, NativeCameraFadeTransitionRuntime::default());
    }

    #[test]
    fn sequence_waits_on_native_fade_state() {
        let sequence = trigger(SEQUENCE_TYPE, Vec3::ZERO, sequence_data(&[0.25]), vec![1]);
        let camera = trigger(TYPE, Vec3::ZERO, camera_data(0, 0), vec![]);
        let map = ProcessedMap {
            triggers: vec![sequence, camera],
            ..Default::default()
        };

        let (mut runtime, _) = NativeCameraSequenceRuntime::start(&map, 0).unwrap();
        assert_eq!(runtime.fixed_update(&map, NATIVE_FADE_OUT_STATE), None);
        assert_eq!(runtime.state, 1);
        assert_eq!(runtime.fixed_update(&map, NATIVE_FADE_IN_STATE), None);
        assert_eq!(runtime.state, 2);

        runtime.state = 3;
        assert_eq!(runtime.fixed_update(&map, NATIVE_FADE_IDLE_STATE), None);
        assert_eq!(runtime.state, 3);
        assert_eq!(runtime.fixed_update(&map, NATIVE_FADE_IN_STATE), None);
        assert_eq!(runtime.state, 0);
    }

    #[test]
    fn native_frame_order_keeps_sequence_before_camera_fade_handshake() {
        let mut data = sequence_data(&[0.0]);
        data[7] = Some(1);
        let map = ProcessedMap {
            triggers: vec![
                trigger(SEQUENCE_TYPE, Vec3::ZERO, data, vec![1]),
                trigger(TYPE, Vec3::ZERO, camera_data(0, 0), vec![]),
            ],
            ..Default::default()
        };
        let (mut sequence, _) = NativeCameraSequenceRuntime::start(&map, 0).unwrap();
        sequence.state = 3;

        let mut camera = NativeCameraFadeTransitionRuntime::default();
        camera.set_camera_mode(1);
        camera.fade.progress = NATIVE_FADE_LOW_THRESHOLD + 0.001;

        camera.fixed_update_fade();
        assert_eq!(camera.fade.state, NATIVE_FADE_OUT_STATE);
        assert!(camera.fade.progress < NATIVE_FADE_LOW_THRESHOLD);

        assert_eq!(sequence.fixed_update(&map, camera.fade.state), None);
        assert_eq!(sequence.state, 3);

        camera.fixed_update_camera();
        assert_eq!(camera.fade.state, NATIVE_FADE_IN_STATE);
        assert_eq!(sequence.state, 3);

        camera.fixed_update_fade();
        assert_eq!(sequence.fixed_update(&map, camera.fade.state), None);
        assert_eq!(sequence.state, 0);
    }

    #[test]
    fn mode_zero_viewport_snaps_to_marker_and_camera_with_native_vfov() {
        let camera = trigger(TYPE, Vec3::new(10.0, 2.0, 4.0), camera_data(0, 0), vec![1]);
        let marker = trigger(
            MARKER_TYPE,
            Vec3::new(3.0, 5.0, 7.0),
            vec![None; 16],
            vec![],
        );
        let map = ProcessedMap {
            triggers: vec![camera, marker],
            ..Default::default()
        };

        let runtime = viewport_runtime(
            &map,
            controller_plan(&map, 0).unwrap(),
            current_pose(),
            Vec3::ZERO,
        );
        assert_eq!(runtime.current.position, Vec3::new(3.0, 5.0, 7.0));
        assert_eq!(runtime.current.target, Vec3::new(10.0, 2.0, 4.0));
        assert_eq!(runtime.current.vertical_fov_degrees, 45.0);
        assert_eq!(runtime.interpolation_rate, 0.04);
        assert!(!runtime.interpolating);
        assert_eq!(runtime.boundary, None);
    }

    #[test]
    fn mode_zero_viewport_uses_native_fixed_sixty_hz_interpolation() {
        let camera = trigger(TYPE, Vec3::new(10.0, 2.0, 4.0), camera_data(0, 1), vec![1]);
        let marker = trigger(
            MARKER_TYPE,
            Vec3::new(3.0, 5.0, 7.0),
            vec![None; 16],
            vec![],
        );
        let map = ProcessedMap {
            triggers: vec![camera, marker],
            ..Default::default()
        };
        let current = current_pose();
        let mut runtime =
            viewport_runtime(&map, controller_plan(&map, 0).unwrap(), current, Vec3::ZERO);

        runtime.advance(1.0 / 60.0);
        assert!(
            runtime
                .current
                .position
                .distance(current.position.lerp(runtime.desired.position, 0.04))
                < 1.0e-6
        );
        assert!(
            runtime
                .current
                .target
                .distance(current.target.lerp(runtime.desired.target, 0.04))
                < 1.0e-6
        );
        assert!((runtime.current.vertical_fov_degrees - 88.2).abs() < 1.0e-5);
        assert!(runtime.is_transitioning());
    }

    #[test]
    fn mode_three_viewport_targets_player_anchor_plus_native_height() {
        let camera = trigger(TYPE, Vec3::ZERO, camera_data(3, 0), vec![1]);
        let marker = trigger(
            MARKER_TYPE,
            Vec3::new(3.0, 5.0, 7.0),
            vec![None; 16],
            vec![],
        );
        let map = ProcessedMap {
            triggers: vec![camera, marker],
            ..Default::default()
        };
        let player_anchor = Vec3::new(100.0, 2.0, -20.0);
        let mut runtime = viewport_runtime(
            &map,
            controller_plan(&map, 0).unwrap(),
            current_pose(),
            player_anchor,
        );

        assert_eq!(runtime.current.position, Vec3::new(3.0, 5.0, 7.0));
        assert_eq!(runtime.current.target, Vec3::new(100.0, 3.3, -20.0));
        assert_eq!(runtime.current.vertical_fov_degrees, 60.0);
        assert_eq!(runtime.interpolation_rate, 0.08);

        let moved_player = Vec3::new(104.0, 4.0, -18.0);
        runtime.update_dynamic_pose(moved_player);
        assert_eq!(runtime.player_anchor, moved_player);
        assert_eq!(runtime.desired.target, Vec3::new(104.0, 5.3, -18.0));
        assert_eq!(runtime.current.target, runtime.desired.target);
    }

    #[test]
    fn mode_three_viewport_preserves_requested_current_camera_axes() {
        let camera = trigger(TYPE, Vec3::ZERO, camera_data(3, 0x300), vec![1]);
        let marker = trigger(
            MARKER_TYPE,
            Vec3::new(3.0, 5.0, 7.0),
            vec![None; 16],
            vec![],
        );
        let map = ProcessedMap {
            triggers: vec![camera, marker],
            ..Default::default()
        };
        let current = current_pose();
        let runtime =
            viewport_runtime(&map, controller_plan(&map, 0).unwrap(), current, Vec3::ZERO);

        assert_eq!(runtime.current.position, current.position);
        assert_eq!(runtime.current.target, Vec3::new(0.0, 1.3, 0.0));
    }

    #[test]
    fn mode_four_missing_path_keeps_the_native_viewport_disabled() {
        let mut data = camera_data(4, 1);
        data[1] = Some(0x0B00_0042);
        let camera = trigger(TYPE, Vec3::ZERO, data, vec![]);
        let map = ProcessedMap {
            triggers: vec![camera],
            ..Default::default()
        };
        let current = current_pose();
        let runtime =
            viewport_runtime(&map, controller_plan(&map, 0).unwrap(), current, Vec3::ZERO);

        assert_eq!(runtime.current, current);
        assert_eq!(
            runtime.boundary,
            Some(NativeCameraViewportBoundary::Mode4PathTraversalUnresolved)
        );
        assert!(!runtime.is_transitioning());
    }

    #[test]
    fn mode_four_xpath_spline_matches_native_natural_cubic_sampling() {
        let path = spline_path(
            0x0B00_0042,
            &[
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 1.0, 0.0),
                Vec3::new(2.0, 0.0, 0.0),
            ],
        );
        let spline = NativeMode4Spline::from_path(&path).unwrap();
        let sample = spline.sample(0.5);
        assert!((sample.x - 0.5).abs() < 1.0e-6);
        assert!((sample.y - 0.6875).abs() < 1.0e-6);
        assert_eq!(sample.z, 0.0);
    }

    #[test]
    fn mode_four_distance_solver_uses_native_point_one_parameter_steps() {
        let path = spline_path(
            0x0B00_0042,
            &[
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(10.0, 0.0, 0.0),
                Vec3::new(20.0, 0.0, 0.0),
                Vec3::new(30.0, 0.0, 0.0),
            ],
        );
        let spline = NativeMode4Spline::from_path(&path).unwrap();
        assert!((spline.advance_parameter_by_distance(1.0, 5.0) - 1.5).abs() < 1.0e-5);
        assert!((spline.advance_parameter_by_distance(2.0, -5.0) - 1.5).abs() < 1.0e-5);
    }

    #[test]
    fn mode_four_chase_activation_holds_native_distance_and_tangent_target() {
        let mut data = camera_data(4, 0x10);
        data[1] = Some(0x0B00_0042);
        data[6] = Some(MODE4_DEFAULT_DISTANCE_SCALE.to_bits());
        data[7] = Some(1.0f32.to_bits());
        let camera = trigger(TYPE, Vec3::ZERO, data, vec![]);
        let map = ProcessedMap {
            paths: vec![spline_path(
                0x0B00_0042,
                &[
                    Vec3::new(0.0, 0.0, 0.0),
                    Vec3::new(32.0, 0.0, 0.0),
                    Vec3::new(64.0, 0.0, 0.0),
                ],
            )],
            triggers: vec![camera],
            ..Default::default()
        };
        let player_anchor = Vec3::new(32.0, 0.0, 0.0);
        let runtime = viewport_runtime(
            &map,
            controller_plan(&map, 0).unwrap(),
            current_pose(),
            player_anchor,
        );

        assert_eq!(runtime.boundary, None);
        assert_eq!(runtime.mode4_direction_scalar(), Some(-1.0));
        assert!(runtime.desired.position.distance(Vec3::ZERO) < 0.01);
        assert!(runtime.desired.target.distance(player_anchor) < 0.01);
        assert!((runtime.mode4_parameter().unwrap_or_default() - 0.0).abs() < 0.001);
        assert_eq!(runtime.current, runtime.desired);
        assert_eq!(runtime.current.vertical_fov_degrees, 45.0);
    }

    #[test]
    fn mode_four_recomputes_desired_pose_each_frame_before_native_smoothing() {
        let mut data = camera_data(4, 0x10);
        data[1] = Some(0x0B00_0042);
        data[6] = Some(MODE4_DEFAULT_DISTANCE_SCALE.to_bits());
        data[7] = Some(1.0f32.to_bits());
        let camera = trigger(TYPE, Vec3::ZERO, data, vec![]);
        let map = ProcessedMap {
            paths: vec![spline_path(
                0x0B00_0042,
                &[
                    Vec3::new(0.0, 0.0, 0.0),
                    Vec3::new(32.0, 0.0, 0.0),
                    Vec3::new(64.0, 0.0, 0.0),
                ],
            )],
            triggers: vec![camera],
            ..Default::default()
        };
        let mut runtime = viewport_runtime(
            &map,
            controller_plan(&map, 0).unwrap(),
            current_pose(),
            Vec3::new(32.0, 0.0, 0.0),
        );
        runtime.update_dynamic_pose(Vec3::new(48.0, 0.0, 0.0));
        assert!((runtime.desired.position.x - 16.0).abs() < 0.02);
        assert!((runtime.desired.target.x - 48.0).abs() < 0.02);

        runtime.advance(1.0 / 60.0);
        assert!((runtime.current.position.x - 1.28).abs() < 0.02);
        assert!(runtime.is_transitioning());
    }

    #[test]
    fn mode_four_flag_two_derives_direction_from_path_vs_camera_look() {
        let mut data = camera_data(4, 0x02);
        data[1] = Some(0x0B00_0042);
        let camera = trigger(TYPE, Vec3::ZERO, data, vec![]);
        let map = ProcessedMap {
            paths: vec![spline_path(
                0x0B00_0042,
                &[
                    Vec3::new(0.0, 0.0, 0.0),
                    Vec3::new(32.0, 0.0, 0.0),
                    Vec3::new(64.0, 0.0, 0.0),
                ],
            )],
            triggers: vec![camera],
            ..Default::default()
        };
        let runtime = viewport_runtime(
            &map,
            controller_plan(&map, 0).unwrap(),
            current_pose(),
            Vec3::new(32.0, 0.0, 0.0),
        );
        assert_eq!(runtime.mode4_direction_scalar(), Some(-1.0));
    }

    #[test]
    fn mode_four_bank_uses_native_sixteen_sample_angle_filter() {
        let mut data = camera_data(4, 0x14);
        data[1] = Some(0x0B00_0042);
        data[6] = Some(MODE4_DEFAULT_DISTANCE_SCALE.to_bits());
        data[7] = Some(1.0f32.to_bits());
        let camera = trigger(TYPE, Vec3::ZERO, data, vec![]);
        let map = ProcessedMap {
            paths: vec![spline_path(
                0x0B00_0042,
                &[
                    Vec3::new(0.0, 0.0, 0.0),
                    Vec3::new(32.0, 0.0, 0.0),
                    Vec3::new(64.0, 0.0, 0.0),
                ],
            )],
            triggers: vec![camera],
            ..Default::default()
        };
        let runtime = viewport_runtime(
            &map,
            controller_plan(&map, 0).unwrap(),
            current_pose(),
            Vec3::new(32.0, 0.0, 0.0),
        );
        let expected =
            std::f32::consts::FRAC_PI_2 * MODE4_BANK_SCALE / MODE4_BANK_SAMPLE_COUNT as f32;
        assert!((runtime.desired.roll_degrees - expected).abs() < 0.001);
    }

    #[test]
    fn mode_four_special_map_path_pair_uses_native_fixed_target_offset() {
        let mut data = camera_data(4, 0);
        data[1] = Some(MODE4_SPECIAL_PATH_HASHCODE);
        let camera = trigger(TYPE, Vec3::ZERO, data, vec![]);
        let map = ProcessedMap {
            hashcode: MODE4_SPECIAL_MAP_HASHCODE,
            paths: vec![spline_path(
                MODE4_SPECIAL_PATH_HASHCODE,
                &[
                    Vec3::new(0.0, 2.0, 0.0),
                    Vec3::new(10.0, 2.0, 0.0),
                    Vec3::new(20.0, 2.0, 0.0),
                ],
            )],
            triggers: vec![camera],
            ..Default::default()
        };
        let runtime = viewport_runtime(
            &map,
            controller_plan(&map, 0).unwrap(),
            current_pose(),
            Vec3::new(10.0, 2.0, 0.0),
        );
        assert_eq!(runtime.boundary, None);
        assert!((runtime.desired.target.x - runtime.desired.position.x).abs() < 1.0e-6);
        assert!((runtime.desired.target.y - (runtime.desired.position.y - 0.23)).abs() < 1.0e-6);
        assert!((runtime.desired.target.z - (runtime.desired.position.z + 1.0)).abs() < 1.0e-6);
    }

    #[test]
    fn mode_four_plan_preserves_path_and_float_payloads_without_naming_them() {
        let mut data = camera_data(4, 0x57);
        data[1] = Some(0x0B00_0042);
        data[3] = Some(7);
        data[4] = Some(30);
        data[5] = Some(40);
        data[6] = Some(1.25f32.to_bits());
        data[7] = Some(0.5f32.to_bits());
        let camera = trigger(TYPE, Vec3::ZERO, data, vec![]);
        let map = ProcessedMap {
            triggers: vec![camera],
            ..Default::default()
        };
        let plan = controller_plan(&map, 0).unwrap();
        assert_eq!(plan.setup_kind, NativeCameraSetupKind::Mode4Path);
        assert_eq!(plan.path_hashcode, Some(0x0B00_0042));
        assert_eq!(plan.controller_data3_raw, 7);
        assert_eq!(plan.scaled_data4, Some(3.0));
        assert_eq!(plan.scaled_data5, Some(4.0));
        assert_eq!(plan.mode4_data6, Some(1.25));
        assert_eq!(plan.mode4_data7, Some(0.5));
        assert_eq!(plan.mode4_option_flags, 0x57);
    }

    #[test]
    fn cutscene_camera_shake_request_lives_for_exact_native_update_count() {
        let mut runtime = NativeCameraShakeRuntime::default();
        assert!(runtime.request(10, 20, true, false));
        assert_eq!(runtime.magnitude, 10);
        assert_eq!(runtime.remaining_updates, 20);
        assert!(runtime.active());
        assert_eq!(runtime.request_count, 1);
        for _ in 0..19 {
            runtime.fixed_update();
        }
        assert_eq!(runtime.remaining_updates, 1);
        assert!(runtime.active());
        runtime.fixed_update();
        assert_eq!(runtime.remaining_updates, 0);
        assert!(!runtime.active());
        assert!(!runtime.request(0, 20, true, false));
        assert!(!runtime.request(10, 0, true, false));
    }
}
