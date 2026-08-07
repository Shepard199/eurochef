use glam::Vec3;

use crate::maps::{ProcessedMap, ProcessedTrigger};

pub const TYPE: u32 = 1;
pub const MARKER_TYPE: u32 = 20;

const NATIVE_TESTED_FLAG_MASK: u32 = 0x0000_03F7;
const MODE4_OPTION_FLAG_MASK: u32 = 0x0000_0057;
const NATIVE_FIXED_HZ: f32 = 60.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeCameraViewportPose {
    pub position: Vec3,
    pub target: Vec3,
    pub vertical_fov_degrees: f32,
}

impl NativeCameraViewportPose {
    pub fn is_finite(self) -> bool {
        self.position.is_finite()
            && self.target.is_finite()
            && self.vertical_fov_degrees.is_finite()
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
                "mode 4 path binding is decoded, but native path traversal is unresolved"
            }
            Self::UnknownMode(_) => "unknown native Camera mode",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeCameraViewportRuntime {
    pub trigger_index: usize,
    pub mode: u32,
    pub current: NativeCameraViewportPose,
    pub desired: NativeCameraViewportPose,
    pub interpolation_rate: f32,
    pub interpolating: bool,
    pub player_anchor: Vec3,
    pub boundary: Option<NativeCameraViewportBoundary>,
}

impl NativeCameraViewportRuntime {
    pub fn advance(&mut self, delta_seconds: f32) {
        if !self.interpolating || self.boundary.is_some() {
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

        if self
            .current
            .position
            .distance_squared(self.desired.position)
            <= 1.0e-8
            && self.current.target.distance_squared(self.desired.target) <= 1.0e-8
            && (self.current.vertical_fov_degrees - self.desired.vertical_fov_degrees).abs()
                <= 1.0e-4
        {
            self.current = self.desired;
        }
    }

    pub fn is_transitioning(self) -> bool {
        self.interpolating
            && self.boundary.is_none()
            && (self.current.position != self.desired.position
                || self.current.target != self.desired.target
                || self.current.vertical_fov_degrees != self.desired.vertical_fov_degrees)
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
        4 => (0.08, 0.0, 45.0),
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
    plan: NativeCameraControllerPlan,
    current: NativeCameraViewportPose,
    player_anchor: Vec3,
) -> NativeCameraViewportRuntime {
    let (interpolation_rate, player_target_y_offset, vertical_fov_degrees) =
        native_mode_profile(plan.mode).unwrap_or((0.0, 0.0, current.vertical_fov_degrees));
    let mut desired = current;
    desired.vertical_fov_degrees = vertical_fov_degrees;

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
        4 => Some(NativeCameraViewportBoundary::Mode4PathTraversalUnresolved),
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eurochef_edb::map::EXGeoTriggerEngineOptions;

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
        let mut runtime = viewport_runtime(controller_plan(&map, 0).unwrap(), current, Vec3::ZERO);

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
        let runtime = viewport_runtime(
            controller_plan(&map, 0).unwrap(),
            current_pose(),
            player_anchor,
        );

        assert_eq!(runtime.current.position, Vec3::new(3.0, 5.0, 7.0));
        assert_eq!(runtime.current.target, Vec3::new(100.0, 3.3, -20.0));
        assert_eq!(runtime.current.vertical_fov_degrees, 60.0);
        assert_eq!(runtime.interpolation_rate, 0.08);
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
        let runtime = viewport_runtime(controller_plan(&map, 0).unwrap(), current, Vec3::ZERO);

        assert_eq!(runtime.current.position, current.position);
        assert_eq!(runtime.current.target, Vec3::new(0.0, 1.3, 0.0));
    }

    #[test]
    fn mode_four_viewport_remains_diagnostic_until_path_traversal_is_proven() {
        let mut data = camera_data(4, 1);
        data[1] = Some(0x0B00_0042);
        let camera = trigger(TYPE, Vec3::ZERO, data, vec![]);
        let map = ProcessedMap {
            triggers: vec![camera],
            ..Default::default()
        };
        let current = current_pose();
        let runtime = viewport_runtime(controller_plan(&map, 0).unwrap(), current, Vec3::ZERO);

        assert_eq!(runtime.current, current);
        assert_eq!(
            runtime.boundary,
            Some(NativeCameraViewportBoundary::Mode4PathTraversalUnresolved)
        );
        assert!(!runtime.is_transitioning());
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
