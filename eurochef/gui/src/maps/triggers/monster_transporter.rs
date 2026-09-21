use glam::{EulerRot, Quat, Vec3};

use crate::maps::ProcessedPath;

use super::{camera::NativeMode4Spline, float};

pub const TYPE: u32 = 73;
pub const DEFAULT_PATH_SPEED: f32 = 3.0;
pub const PATH_SPEED_EPSILON: f32 = 0.001;
const FIXED_HZ: f32 = 60.0;
const LOOP_EPSILON: f32 = 0.001;
const TURN_LIMIT_RADIANS: f32 = std::f32::consts::PI / 240.0;
const NODE_EVENT_RADIUS_SQUARED: f32 = 0.05;

pub fn primary_path_hash(data: &[Option<u32>]) -> Option<u32> {
    data.get(1).copied().flatten()
}

pub fn path_speed(data: &[Option<u32>]) -> Option<f32> {
    let speed = float(data, 2)?;
    Some(if speed < PATH_SPEED_EPSILON {
        DEFAULT_PATH_SPEED
    } else {
        speed
    })
}

pub fn secondary_path_hash(data: &[Option<u32>]) -> Option<u32> {
    data.get(4).copied().flatten()
}

/// `XItemHandler_Transporter::0x00469450` compares runtime trigger `+0xE4`
/// (successful spawned-monster count) against trigger `+0x78`, which is the
/// serialized data3 field for this class.
#[allow(dead_code)]
pub fn total_spawn_limit(data: &[Option<u32>]) -> Option<u32> {
    data.get(3).copied().flatten()
}

/// The same native spawn gate compares handler `+0x538` live carried-monster
/// count against trigger `+0x80`, serialized as data5.
#[allow(dead_code)]
pub fn active_spawn_limit(data: &[Option<u32>]) -> Option<u32> {
    data.get(5).copied().flatten()
}

fn wrapped_delta(target: f32, current: f32) -> f32 {
    let mut delta = (target - current) % std::f32::consts::TAU;
    if delta > std::f32::consts::PI {
        delta -= std::f32::consts::TAU;
    } else if delta < -std::f32::consts::PI {
        delta += std::f32::consts::TAU;
    }
    delta
}

fn tangent_angles(tangent: Vec3) -> Option<(f32, f32)> {
    if !tangent.is_finite() || tangent.length_squared() <= f32::EPSILON {
        return None;
    }
    let horizontal = (tangent.x * tangent.x + tangent.z * tangent.z).sqrt();
    let yaw = tangent.x.atan2(tangent.z);
    let pitch = horizontal.atan2(tangent.y) - std::f32::consts::FRAC_PI_2;
    Some((pitch, yaw))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MonsterTransporterRouteBounds {
    pub center: Vec3,
    pub radius: f32,
}

pub(crate) fn route_bounds(primary: &ProcessedPath) -> Option<MonsterTransporterRouteBounds> {
    let mut nodes = primary.nodes.iter();
    let first = nodes.next()?.position;
    if !first.is_finite() {
        return None;
    }
    let mut min = first;
    let mut max = first;
    for node in nodes {
        let point = node.position;
        if !point.is_finite() {
            return None;
        }
        min = min.min(point);
        max = max.max(point);
    }
    let center = (min + max) * 0.5;
    Some(MonsterTransporterRouteBounds {
        center,
        radius: (max - min).length() * 0.5,
    })
}

pub(crate) fn route_distance_squared(primary: &ProcessedPath, point: Vec3) -> Option<f32> {
    let bounds = route_bounds(primary)?;
    let xz = Vec3::new(point.x - bounds.center.x, 0.0, point.z - bounds.center.z);
    let distance = xz.length().max(1.0) - bounds.radius;
    Some(distance * distance)
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeMonsterTransporterRuntime {
    spline: NativeMode4Spline,
    event_nodes: Vec<(Vec3, [u16; 4])>,
    event_node_index: usize,
    parameter: f32,
    loop_parameter: f32,
    speed: f32,
    pitch: f32,
    yaw: f32,
    pub position: Vec3,
    pub rotation: Quat,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeMonsterTransporterPathEvent {
    /// Native path event type 0x0D. `count` is node value[1] and calls
    /// XItemHandler_Transporter::0x00469450 exactly that many times.
    SpawnAttempts { count: u16 },
    /// Native path event type 0x0B dispatches common mask 0x1000 to the creator trigger.
    ResetReload,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct NativeMonsterTransporterFixedStep {
    pub events: Vec<NativeMonsterTransporterPathEvent>,
}

impl NativeMonsterTransporterRuntime {
    pub(crate) fn new(
        primary: &ProcessedPath,
        secondary: Option<&ProcessedPath>,
        speed: f32,
    ) -> Option<Self> {
        if primary.nodes.len() < 3 {
            return None;
        }
        let secondary_nodes = secondary.map_or(0, |path| path.nodes.len());
        let mut points = Vec::with_capacity(secondary_nodes + primary.nodes.len() + 3);
        if let Some(secondary) = secondary {
            points.extend(secondary.nodes.iter().map(|node| node.position));
        }
        points.extend(primary.nodes.iter().map(|node| node.position));
        points.extend(primary.nodes.iter().take(3).map(|node| node.position));
        let spline = NativeMode4Spline::from_points(points)?;
        let parameter = 0.0;
        let position = spline.sample(parameter);
        let tangent = spline.finite_difference(parameter, 1.0);
        let (pitch, yaw) = tangent_angles(tangent).unwrap_or((0.0, 0.0));
        let rotation = Quat::from_euler(EulerRot::ZXY, 0.0, pitch, yaw);
        Some(Self {
            spline,
            event_nodes: primary
                .nodes
                .iter()
                .map(|node| (node.position, node.value))
                .collect(),
            event_node_index: 0,
            parameter,
            loop_parameter: secondary_nodes as f32 + 1.0,
            speed,
            pitch,
            yaw,
            position,
            rotation,
        })
    }

    pub(crate) fn parameter(&self) -> f32 {
        self.parameter
    }

    pub(crate) fn advance_fixed_tick(&mut self) -> NativeMonsterTransporterFixedStep {
        if self.parameter >= self.spline.last_parameter() - LOOP_EPSILON {
            self.parameter = self.loop_parameter;
        }
        self.position = self.spline.sample(self.parameter);

        // XItemHandler_Transporter::0x00469020 samples the current route position first,
        // then 0x00469350 checks the current primary-path node against radius^2 0.05.
        // The node cursor advances/wraps whenever that radius is reached, even for a node
        // whose event type is zero.
        let mut step = NativeMonsterTransporterFixedStep::default();
        if let Some((node_position, value)) = self.event_nodes.get(self.event_node_index).copied() {
            if self.position.distance_squared(node_position) < NODE_EVENT_RADIUS_SQUARED {
                match value[0] {
                    0x0D if value[1] != 0 => step
                        .events
                        .push(NativeMonsterTransporterPathEvent::SpawnAttempts { count: value[1] }),
                    0x0B => step
                        .events
                        .push(NativeMonsterTransporterPathEvent::ResetReload),
                    _ => {}
                }
                self.event_node_index += 1;
                if self.event_node_index >= self.event_nodes.len() {
                    self.event_node_index = 0;
                }
            }
        }

        if let Some((target_pitch, target_yaw)) =
            tangent_angles(self.spline.finite_difference(self.parameter, 1.0))
        {
            self.pitch += wrapped_delta(target_pitch, self.pitch)
                .clamp(-TURN_LIMIT_RADIANS, TURN_LIMIT_RADIANS);
            self.yaw +=
                wrapped_delta(target_yaw, self.yaw).clamp(-TURN_LIMIT_RADIANS, TURN_LIMIT_RADIANS);
            self.rotation = Quat::from_euler(EulerRot::ZXY, 0.0, self.pitch, self.yaw);
        }
        self.parameter = self
            .spline
            .advance_parameter_by_distance(self.parameter, self.speed / FIXED_HZ);
        step
    }
}

#[cfg(test)]
mod tests {
    use glam::{Vec2, Vec3};

    use crate::maps::{ProcessedPath, ProcessedPathNode};

    use super::*;

    fn path(hashcode: u32, points: &[[f32; 3]]) -> ProcessedPath {
        ProcessedPath {
            hashcode,
            position: Vec3::ZERO,
            flags: 0x2000_0000,
            path_type: 0,
            nodes: points
                .iter()
                .map(|point| ProcessedPathNode {
                    position: Vec3::from(*point),
                    size: Vec2::ZERO,
                    value: [0; 4],
                    flags: 0,
                    distance: 0.0,
                    num_links: 0,
                })
                .collect(),
            links: Vec::new(),
        }
    }

    #[test]
    fn transporter_speed_uses_serialized_data2_with_native_fallback() {
        let mut data = vec![None; 5];
        data[2] = Some(0.0f32.to_bits());
        assert_eq!(path_speed(&data), Some(3.0));
        data[2] = Some(0.0005f32.to_bits());
        assert_eq!(path_speed(&data), Some(3.0));
        data[2] = Some(4.25f32.to_bits());
        assert_eq!(path_speed(&data), Some(4.25));
    }

    #[test]
    fn transporter_route_bounds_match_native_aabb_center_and_half_3d_diagonal() {
        let primary = path(1, &[[0.0, 0.0, 0.0], [4.0, 6.0, 8.0], [2.0, 1.0, 3.0]]);
        let bounds = route_bounds(&primary).unwrap();
        assert!((bounds.center - Vec3::new(2.0, 3.0, 4.0)).length() < 1.0e-6);
        assert!((bounds.radius - 116.0f32.sqrt() * 0.5).abs() < 1.0e-6);
    }

    #[test]
    fn transporter_path_node_events_match_native_type_and_repeat_count() {
        let mut primary = path(1, &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]);
        primary.nodes[0].value = [0x0D, 3, 0, 0];
        primary.nodes[1].value = [0x0B, 0, 0, 0];
        let mut runtime = NativeMonsterTransporterRuntime::new(&primary, None, 3.0)
            .expect("native transporter events");

        let spawn = runtime.advance_fixed_tick();
        assert_eq!(
            spawn.events,
            vec![NativeMonsterTransporterPathEvent::SpawnAttempts { count: 3 }]
        );
        assert_eq!(runtime.event_node_index, 1);

        runtime.parameter = 1.0;
        let reset = runtime.advance_fixed_tick();
        assert_eq!(
            reset.events,
            vec![NativeMonsterTransporterPathEvent::ResetReload]
        );
        assert_eq!(runtime.event_node_index, 2);
    }

    #[test]
    fn transporter_event_cursor_advances_across_nodes_without_events() {
        let primary = path(1, &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]);
        let mut runtime = NativeMonsterTransporterRuntime::new(&primary, None, 3.0)
            .expect("native transporter cursor");
        assert!(runtime.advance_fixed_tick().events.is_empty());
        assert_eq!(runtime.event_node_index, 1);
    }

    #[test]
    fn transporter_composite_spline_uses_secondary_prefix_and_primary_three_point_tail() {
        let primary = path(
            1,
            &[
                [10.0, 0.0, 0.0],
                [11.0, 0.0, 0.0],
                [12.0, 0.0, 0.0],
                [13.0, 0.0, 0.0],
            ],
        );
        let secondary = path(2, &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]]);
        let mut runtime = NativeMonsterTransporterRuntime::new(&primary, Some(&secondary), 3.0)
            .expect("native composite transporter route");
        assert_eq!(runtime.parameter(), 0.0);
        assert!((runtime.position - secondary.nodes[0].position).length() < 1.0e-6);
        assert!((runtime.spline.sample(2.0) - primary.nodes[0].position).length() < 1.0e-6);
        assert!((runtime.spline.sample(6.0) - primary.nodes[0].position).length() < 1.0e-6);
        assert!((runtime.spline.sample(8.0) - primary.nodes[2].position).length() < 1.0e-6);

        runtime.parameter = runtime.spline.last_parameter();
        runtime.advance_fixed_tick();
        assert!((runtime.position - runtime.spline.sample(3.0)).length() < 1.0e-6);
        assert!(runtime.parameter() > 3.0);

        // Native service reset 0x0047FE20 -> 0x0044BBB0 destroys a live
        // Transporter XItem and reloads serialized class state via 0x0047F910.
        // Recreating the editor runtime must therefore restart the route at 0.
        let restarted = NativeMonsterTransporterRuntime::new(&primary, Some(&secondary), 3.0)
            .expect("reset transporter route");
        assert_eq!(restarted.parameter(), 0.0);
        assert!((restarted.position - secondary.nodes[0].position).length() < 1.0e-6);
    }
}
