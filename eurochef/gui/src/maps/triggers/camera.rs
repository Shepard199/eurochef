use glam::Vec3;

use crate::maps::{ProcessedMap, ProcessedTrigger};

pub const TYPE: u32 = 1;
pub const MARKER_TYPE: u32 = 20;

const NATIVE_TESTED_FLAG_MASK: u32 = 0x0000_03F7;
const MODE4_OPTION_FLAG_MASK: u32 = 0x0000_0057;

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
    pub mode3_override_player_y: bool,
    pub mode3_override_player_xz: bool,
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
        mode3_override_player_y: mode == 3 && flags & 0x100 != 0,
        mode3_override_player_xz: mode == 3 && flags & 0x200 != 0,
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
    fn mode_three_plan_preserves_player_axis_override_bits() {
        let camera = trigger(TYPE, Vec3::ZERO, camera_data(3, 0x300), vec![]);
        let map = ProcessedMap {
            triggers: vec![camera],
            ..Default::default()
        };
        let plan = controller_plan(&map, 0).unwrap();
        assert!(plan.mode3_override_player_y);
        assert!(plan.mode3_override_player_xz);
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
