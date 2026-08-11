use glam::Vec3;

use crate::maps::{ProcessedMap, ProcessedPath, ProcessedTrigger};

pub const TYPE: u32 = 1;
pub const MARKER_TYPE: u32 = 20;

const NATIVE_TESTED_FLAG_MASK: u32 = 0x0000_03F7;
const MODE4_OPTION_FLAG_MASK: u32 = 0x0000_0057;
const NATIVE_FIXED_HZ: f32 = 60.0;
const MODE4_DEFAULT_DISTANCE_SCALE: f32 = 6.4;
const MODE4_TARGET_Y_OFFSET: f32 = 1.3;
const MODE4_DISTANCE_MULTIPLIER: f32 = 5.0;
const MODE4_PARAMETER_STEP: f32 = 0.1;
const MODE4_DISTANCE_EPSILON: f32 = 0.001;
const MODE4_BANK_SAMPLE_COUNT: usize = 16;
const MODE4_BANK_SCALE: f32 = -1200.0;
const MODE4_SPECIAL_MAP_HASHCODE: u32 = 0x0100_0073;
const MODE4_SPECIAL_PATH_HASHCODE: u32 = 0x0B00_0001;
const MODE4_SPECIAL_TARGET_Y_OFFSET: f32 = 0.23;

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
struct NativeMode4Spline {
    points: Vec<Vec3>,
    second_derivatives: Vec<Vec3>,
}

impl NativeMode4Spline {
    fn from_path(path: &ProcessedPath) -> Option<Self> {
        if path.path_type != 0 || path.flags & 0x2000_0000 == 0 || path.nodes.is_empty() {
            return None;
        }

        let points = path
            .nodes
            .iter()
            .map(|node| node.position)
            .collect::<Vec<_>>();
        let mut second_derivatives = vec![Vec3::ZERO; points.len()];
        if points.len() > 2 {
            let interior = points.len() - 2;
            let mut c_prime = vec![0.0f32; interior];
            let mut d_prime = vec![Vec3::ZERO; interior];
            for interior_index in 0..interior {
                let i = interior_index + 1;
                let rhs = (points[i + 1] - points[i] * 2.0 + points[i - 1]) * 6.0;
                let denom = if interior_index == 0 {
                    4.0
                } else {
                    4.0 - c_prime[interior_index - 1]
                };
                c_prime[interior_index] = if interior_index + 1 == interior {
                    0.0
                } else {
                    1.0 / denom
                };
                d_prime[interior_index] = if interior_index == 0 {
                    rhs / denom
                } else {
                    (rhs - d_prime[interior_index - 1]) / denom
                };
            }
            for interior_index in (0..interior).rev() {
                let i = interior_index + 1;
                second_derivatives[i] = if interior_index + 1 == interior {
                    d_prime[interior_index]
                } else {
                    d_prime[interior_index] - second_derivatives[i + 1] * c_prime[interior_index]
                };
            }
        }

        Some(Self {
            points,
            second_derivatives,
        })
    }

    fn last_parameter(&self) -> f32 {
        self.points.len().saturating_sub(1) as f32
    }

    fn sample(&self, parameter: f32) -> Vec3 {
        if self.points.len() == 1 {
            return self.points[0];
        }
        let parameter = parameter.max(0.0);
        let last = self.last_parameter();
        if parameter >= last {
            return *self.points.last().unwrap_or(&Vec3::ZERO);
        }
        let lower = parameter.floor() as usize;
        let upper = lower + 1;
        let b = parameter - lower as f32;
        let a = 1.0 - b;
        let cubic_a = a * a * a - a;
        let cubic_b = b * b * b - b;
        self.points[lower] * a
            + self.points[upper] * b
            + (self.second_derivatives[lower] * cubic_a + self.second_derivatives[upper] * cubic_b)
                * (1.0 / 6.0)
    }

    fn finite_difference(&self, parameter: f32, width: f32) -> Vec3 {
        let half = width * 0.5;
        self.sample(parameter + half) - self.sample(parameter - half)
    }

    fn nearest_node_index(&self, point: Vec3) -> usize {
        self.points
            .iter()
            .enumerate()
            .min_by(|(_, lhs), (_, rhs)| {
                lhs.distance_squared(point)
                    .total_cmp(&rhs.distance_squared(point))
            })
            .map(|(index, _)| index)
            .unwrap_or_default()
    }

    fn advance_parameter_by_distance(&self, parameter: f32, distance: f32) -> f32 {
        if distance == 0.0 || self.points.len() < 2 {
            return parameter.clamp(0.0, self.last_parameter());
        }
        let direction = distance.signum();
        let mut remaining = distance.abs();
        let mut current_parameter = parameter.clamp(0.0, self.last_parameter());
        let last = self.last_parameter();
        while remaining > MODE4_DISTANCE_EPSILON {
            let next_parameter = current_parameter + direction * MODE4_PARAMETER_STEP;
            if next_parameter <= 0.0 || next_parameter >= last {
                return next_parameter.clamp(0.0, last);
            }
            let current = self.sample(current_parameter);
            let next = self.sample(next_parameter);
            let segment_length = current.distance(next);
            if segment_length <= f32::EPSILON {
                current_parameter = next_parameter;
                continue;
            }
            if remaining <= segment_length {
                current_parameter +=
                    direction * MODE4_PARAMETER_STEP * (remaining / segment_length).clamp(0.0, 1.0);
                return current_parameter.clamp(0.0, last);
            }
            remaining -= segment_length;
            current_parameter = next_parameter;
        }
        current_parameter.clamp(0.0, last)
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
}
