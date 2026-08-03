use eurochef_edb::Hashcode;
use glam::{Quat, Vec3};

use crate::{
    map_frame::QueuedEntityRender,
    maps::{
        robots_trigger_path_hash, robots_trigger_platform_angular_velocity,
        robots_trigger_runtime_path_acceleration, robots_trigger_runtime_path_speed, ProcessedMap,
        ProcessedPath, ProcessedTrigger,
    },
    render::RenderStore,
};
pub(crate) fn map_trigger_link_index(link: i32, trigger_count: usize) -> Option<usize> {
    let index = usize::try_from(link).ok()?;
    (index < trigger_count).then_some(index)
}

pub(crate) fn map_trigger_by_link(
    map: &ProcessedMap,
    link: i32,
) -> Option<(usize, &ProcessedTrigger)> {
    let index = map_trigger_link_index(link, map.triggers.len())?;
    map.triggers.get(index).map(|trigger| (index, trigger))
}

pub(crate) fn map_trigger_runtime_path<'a>(
    map: &'a ProcessedMap,
    trigger: &ProcessedTrigger,
) -> Option<(u32, usize, &'a ProcessedPath)> {
    let path_hash = robots_trigger_path_hash(trigger.ttype, &trigger.data)?;
    map.paths
        .iter()
        .enumerate()
        .find(|(_, path)| path.hashcode == path_hash)
        .map(|(index, path)| (path_hash, index, path))
}

pub(crate) fn map_trigger_path_matches<'a>(
    map: &'a ProcessedMap,
    trigger: &ProcessedTrigger,
) -> Vec<(usize, usize, &'a ProcessedPath)> {
    trigger
        .data
        .iter()
        .enumerate()
        .filter_map(|(data_slot, value)| {
            let path_hash = value.as_ref()?;
            map.paths
                .iter()
                .enumerate()
                .find(|(_, path)| path.hashcode == *path_hash)
                .map(|(path_index, path)| (data_slot, path_index, path))
        })
        .collect()
}

pub(crate) fn runtime_path_segments(path: &ProcessedPath) -> Vec<(Vec3, Vec3)> {
    let world_node = |index: usize| {
        path.nodes
            .get(index)
            .map(|node| path.position + node.position)
    };
    if path.links.is_empty() {
        return path
            .nodes
            .windows(2)
            .map(|nodes| {
                (
                    path.position + nodes[0].position,
                    path.position + nodes[1].position,
                )
            })
            .collect();
    }

    path.links
        .iter()
        .filter_map(|(a, b)| Some((world_node(*a)?, world_node(*b)?)))
        .collect()
}

pub(crate) fn runtime_path_route(path: &ProcessedPath) -> Vec<Vec3> {
    runtime_path_route_indices(path)
        .into_iter()
        .filter_map(|index| {
            path.nodes
                .get(index)
                .map(|node| path.position + node.position)
        })
        .collect()
}

pub(crate) fn runtime_path_route_indices(path: &ProcessedPath) -> Vec<usize> {
    let world_nodes = || (0..path.nodes.len()).collect::<Vec<_>>();
    if path.nodes.len() < 2 || path.links.is_empty() {
        return world_nodes();
    }

    let valid_links = path
        .links
        .iter()
        .copied()
        .filter(|(start, end)| *start < path.nodes.len() && *end < path.nodes.len())
        .collect::<Vec<_>>();
    if valid_links.is_empty() {
        return world_nodes();
    }

    let mut incoming = vec![0usize; path.nodes.len()];
    for (_, end) in &valid_links {
        incoming[*end] += 1;
    }
    let start = valid_links
        .iter()
        .find_map(|(start, _)| (incoming[*start] == 0).then_some(*start))
        .unwrap_or(valid_links[0].0);

    let mut used = vec![false; valid_links.len()];
    let mut route = vec![start];
    let mut current = start;
    loop {
        let Some((edge_index, (_, end))) = valid_links
            .iter()
            .enumerate()
            .find(|(index, (edge_start, _))| !used[*index] && *edge_start == current)
        else {
            break;
        };
        used[edge_index] = true;
        current = *end;
        route.push(current);
    }

    if route.len() >= 2 && used.iter().all(|used| *used) {
        route
    } else {
        world_nodes()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuntimePathNodeEvent {
    DeactivateSelf,
    DispatchLinked { event_mask: u32, link_mask: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RuntimePathNodeDispatch {
    pub(crate) node_index: usize,
    pub(crate) event: RuntimePathNodeEvent,
}

fn path_node_event(node: &crate::maps::ProcessedPathNode) -> Option<RuntimePathNodeEvent> {
    match node.value[0] {
        4 => Some(RuntimePathNodeEvent::DeactivateSelf),
        8 => Some(RuntimePathNodeEvent::DispatchLinked {
            event_mask: node.value[1] as u32,
            link_mask: node.value[2] as u8,
        }),
        _ => None,
    }
}

fn periodic_crossing_count(from: f32, to: f32, phase: f32, period: f32) -> usize {
    if period <= f32::EPSILON || (to - from).abs() <= f32::EPSILON {
        return 0;
    }
    let low = from.min(to);
    let high = from.max(to);
    let first = ((low - phase) / period).floor() as i64 - 1;
    let last = ((high - phase) / period).ceil() as i64 + 1;
    (first..=last)
        .map(|k| phase + k as f32 * period)
        .filter(|value| {
            if to > from {
                *value > from + f32::EPSILON && *value <= to + f32::EPSILON
            } else {
                *value < from - f32::EPSILON && *value >= to - f32::EPSILON
            }
        })
        .count()
}

pub(crate) fn runtime_path_node_dispatches_between(
    map: &ProcessedMap,
    trigger: &ProcessedTrigger,
    from_traveled: f32,
    to_traveled: f32,
) -> Vec<RuntimePathNodeDispatch> {
    let Some((_, _, path)) = map_trigger_runtime_path(map, trigger) else {
        return Vec::new();
    };
    let route_indices = runtime_path_route_indices(path);
    if route_indices.len() < 2 {
        return Vec::new();
    }
    let route = route_indices
        .iter()
        .filter_map(|index| {
            path.nodes
                .get(*index)
                .map(|node| path.position + node.position)
        })
        .collect::<Vec<_>>();
    if route.len() != route_indices.len() {
        return Vec::new();
    }

    let looping = trigger.ttype == 80 && path.flags & 0x6000_0000 == 0x6000_0000;
    let segments = runtime_path_segments_for_motion(&route, looping);
    let total_length = segments
        .iter()
        .map(|(start, end)| start.distance(*end))
        .sum::<f32>();
    if total_length <= f32::EPSILON {
        return Vec::new();
    }
    let (start_phase, _) = closest_route_phase(&segments, trigger.position);
    let from = start_phase + from_traveled;
    let to = start_phase + to_traveled;

    let mut node_phases = Vec::with_capacity(route.len());
    let mut node_phase = 0.0f32;
    node_phases.push(0.0);
    for nodes in route.windows(2) {
        node_phase += nodes[0].distance(nodes[1]);
        node_phases.push(node_phase);
    }

    let mut dispatches = Vec::new();
    for (route_position, node_index) in route_indices.iter().copied().enumerate() {
        let Some(node) = path.nodes.get(node_index) else {
            continue;
        };
        let Some(event) = path_node_event(node) else {
            continue;
        };
        let phase = node_phases[route_position];
        let count = if looping {
            periodic_crossing_count(from, to, phase, total_length)
        } else {
            let cycle = total_length * 2.0;
            let mut count = periodic_crossing_count(from, to, phase, cycle);
            let reverse_phase = cycle - phase;
            if (reverse_phase - phase).abs() > f32::EPSILON {
                count += periodic_crossing_count(from, to, reverse_phase, cycle);
            }
            count
        };
        dispatches.extend((0..count).map(|_| RuntimePathNodeDispatch { node_index, event }));
    }
    dispatches
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RuntimePathSample {
    pub(crate) position: Vec3,
    pub(crate) tangent: Vec3,
}

pub(crate) fn runtime_path_segments_for_motion(route: &[Vec3], looping: bool) -> Vec<(Vec3, Vec3)> {
    let mut segments = route
        .windows(2)
        .map(|nodes| (nodes[0], nodes[1]))
        .collect::<Vec<_>>();
    if looping {
        if let (Some(first), Some(last)) = (route.first().copied(), route.last().copied()) {
            if first.distance_squared(last) > f32::EPSILON {
                segments.push((last, first));
            }
        }
    }
    segments
}

pub(crate) fn closest_route_phase(segments: &[(Vec3, Vec3)], position: Vec3) -> (f32, Vec3) {
    let mut cumulative = 0.0;
    let mut best_distance_squared = f32::INFINITY;
    let mut best_phase = 0.0;
    let mut best_point = position;

    for (start, end) in segments {
        let delta = *end - *start;
        let length_squared = delta.length_squared();
        if length_squared <= f32::EPSILON {
            continue;
        }
        let segment_length = length_squared.sqrt();
        let fraction = ((position - *start).dot(delta) / length_squared).clamp(0.0, 1.0);
        let point = start.lerp(*end, fraction);
        let distance_squared = point.distance_squared(position);
        if distance_squared < best_distance_squared {
            best_distance_squared = distance_squared;
            best_phase = cumulative + fraction * segment_length;
            best_point = point;
        }
        cumulative += segment_length;
    }

    (best_phase, position - best_point)
}

pub(crate) const ROBOTS_EVENT_ACTIVATE: u32 = 0x0000_0100;
pub(crate) const ROBOTS_EVENT_DEACTIVATE: u32 = 0x0000_0200;
pub(crate) const ROBOTS_PLATFORM_RETRIGGER_REVERSE_FLAG: u32 = 0x0000_0200;

#[derive(Clone, Copy, Debug)]
pub(crate) struct RuntimeEventPreviewState {
    pub(crate) active: bool,
    pub(crate) elapsed_seconds: f32,
    pub(crate) direction: f32,
    pub(crate) distance_offset: f32,
    pub(crate) last_wall_time: Option<f64>,
    pub(crate) last_event: Option<u32>,
    pub(crate) last_node_index: Option<usize>,
    pub(crate) last_node_opcode: Option<u16>,
    vehicle_steering_angle: f32,
    vehicle_previous_heading: f32,
    vehicle_steering_wall_accumulator: f32,
    vehicle_steering_motion_time: f32,
    platform_contact_linear_velocity: Vec3,
    platform_contact_previous_position: Option<Vec3>,
    platform_contact_wall_accumulator: f32,
    platform_contact_motion_time: f32,
}

impl Default for RuntimeEventPreviewState {
    fn default() -> Self {
        Self {
            active: false,
            elapsed_seconds: 0.0,
            direction: 1.0,
            distance_offset: 0.0,
            last_wall_time: None,
            last_event: None,
            last_node_index: None,
            last_node_opcode: None,
            vehicle_steering_angle: 0.0,
            // XItemHandler_Vehicle constructor at 0x0041819B writes -1000.0
            // to handler+0x308 before the first fixed-step update replaces it.
            vehicle_previous_heading: -1000.0,
            vehicle_steering_wall_accumulator: 0.0,
            vehicle_steering_motion_time: 0.0,
            platform_contact_linear_velocity: Vec3::ZERO,
            platform_contact_previous_position: None,
            platform_contact_wall_accumulator: 0.0,
            platform_contact_motion_time: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RuntimeEventPreviewSnapshot {
    pub(crate) active: bool,
    pub(crate) elapsed_seconds: f32,
    pub(crate) path_distance: f32,
    pub(crate) direction_reversed: bool,
    pub(crate) last_event: Option<u32>,
    pub(crate) last_node_index: Option<usize>,
    pub(crate) last_node_opcode: Option<u16>,
    pub(crate) vehicle_steering_angle: Option<f32>,
    pub(crate) platform_contact_linear_velocity: Option<Vec3>,
}

impl RuntimeEventPreviewState {
    pub(crate) fn reset(&mut self, wall_time: f64) {
        *self = Self {
            last_wall_time: Some(wall_time),
            ..Default::default()
        };
    }

    pub(crate) fn advance(&mut self, wall_time: f64) {
        if let Some(previous) = self.last_wall_time {
            if self.active {
                self.elapsed_seconds += (wall_time - previous).max(0.0) as f32;
            }
        }
        self.last_wall_time = Some(wall_time);
    }

    pub(crate) fn hold(&mut self, wall_time: f64) {
        self.last_wall_time = Some(wall_time);
        self.platform_contact_linear_velocity = Vec3::ZERO;
    }

    pub(crate) fn advance_runtime(
        &mut self,
        map: &ProcessedMap,
        trigger: &ProcessedTrigger,
        wall_time: f64,
        speed_scale: f32,
    ) {
        let delta_seconds = self
            .last_wall_time
            .map(|previous| (wall_time - previous).max(0.0) as f32)
            .unwrap_or_default();
        if matches!(trigger.ttype, 8 | 37 | 80) {
            self.advance_platform_contact_carry(map, trigger, delta_seconds, speed_scale);
        }
        if trigger.ttype == 80 {
            self.advance_vehicle_steering(map, trigger, delta_seconds, speed_scale);
        }
        self.advance(wall_time);
    }

    fn path_distance_at_elapsed(
        &self,
        trigger: &ProcessedTrigger,
        elapsed_seconds: f32,
        speed_scale: f32,
    ) -> f32 {
        let speed_scale = speed_scale.max(0.0);
        let Some(maximum_speed) = robots_trigger_runtime_path_speed(trigger.ttype, &trigger.data)
        else {
            return 0.0;
        };
        let acceleration = robots_trigger_runtime_path_acceleration(trigger.ttype, &trigger.data)
            .unwrap_or_default();
        let raw = runtime_path_travel_distance(
            elapsed_seconds,
            maximum_speed * speed_scale,
            acceleration,
            speed_scale,
        );
        self.distance_offset + self.direction * raw
    }

    fn advance_platform_contact_carry(
        &mut self,
        map: &ProcessedMap,
        trigger: &ProcessedTrigger,
        delta_seconds: f32,
        speed_scale: f32,
    ) {
        const FIXED_STEP: f32 = 1.0 / 60.0;
        const FIXED_HZ: f32 = 60.0;
        const MAX_STEPS_PER_ADVANCE: usize = 200_000;

        let sample_position = |distance: f32| {
            runtime_path_preview_sample_at_distance(map, trigger, distance)
                .map(|sample| sample.position)
                .unwrap_or(trigger.position)
        };
        if self.platform_contact_previous_position.is_none() {
            let distance = self.path_distance_at_elapsed(
                trigger,
                self.platform_contact_motion_time,
                speed_scale,
            );
            self.platform_contact_previous_position = Some(sample_position(distance));
        }

        self.platform_contact_wall_accumulator += delta_seconds.max(0.0);
        let steps =
            ((self.platform_contact_wall_accumulator / FIXED_STEP) + 1.0e-4).floor() as usize;
        let steps = steps.min(MAX_STEPS_PER_ADVANCE);
        self.platform_contact_wall_accumulator =
            (self.platform_contact_wall_accumulator - steps as f32 * FIXED_STEP).max(0.0);

        for _ in 0..steps {
            if self.active {
                self.platform_contact_motion_time += FIXED_STEP;
            }
            let distance = self.path_distance_at_elapsed(
                trigger,
                self.platform_contact_motion_time,
                speed_scale,
            );
            let current_position = sample_position(distance);
            let previous_position = self
                .platform_contact_previous_position
                .unwrap_or(current_position);
            // XItemPhysics_Platform::Contact at 0x0041DFB9..0x0041E2B7
            // converts the platform's one-tick displacement into per-second carry
            // velocity with the exact fixed 60-Hz multiplier.
            self.platform_contact_linear_velocity =
                (current_position - previous_position) * FIXED_HZ;
            self.platform_contact_previous_position = Some(current_position);
        }
    }

    fn advance_vehicle_steering(
        &mut self,
        map: &ProcessedMap,
        trigger: &ProcessedTrigger,
        delta_seconds: f32,
        speed_scale: f32,
    ) {
        const FIXED_STEP: f32 = 1.0 / 60.0;
        const MAX_STEPS_PER_ADVANCE: usize = 200_000;

        self.vehicle_steering_wall_accumulator += delta_seconds.max(0.0);
        // Wall time arrives as f64 but the recovered runtime fields are f32. Keep an
        // exact 60 Hz boundary from occasionally becoming 2.99999 frames after the cast.
        let steps =
            ((self.vehicle_steering_wall_accumulator / FIXED_STEP) + 1.0e-4).floor() as usize;
        let steps = steps.min(MAX_STEPS_PER_ADVANCE);
        self.vehicle_steering_wall_accumulator =
            (self.vehicle_steering_wall_accumulator - steps as f32 * FIXED_STEP).max(0.0);

        for _ in 0..steps {
            if self.active {
                self.vehicle_steering_motion_time += FIXED_STEP;
            }
            let Some(current_heading) = vehicle_heading_at_time(
                map,
                trigger,
                self.vehicle_steering_motion_time,
                speed_scale,
            ) else {
                continue;
            };
            let target = wrap_radians(current_heading - self.vehicle_previous_heading) * 10.0;
            self.vehicle_steering_angle +=
                (target - self.vehicle_steering_angle) * ROBOTS_VEHICLE_STEERING_SMOOTHING;
            self.vehicle_previous_heading = current_heading;
        }
    }

    fn raw_path_distance(&self, trigger: &ProcessedTrigger, speed_scale: f32) -> f32 {
        let speed_scale = speed_scale.max(0.0);
        let Some(maximum_speed) = robots_trigger_runtime_path_speed(trigger.ttype, &trigger.data)
        else {
            return 0.0;
        };
        let acceleration = robots_trigger_runtime_path_acceleration(trigger.ttype, &trigger.data)
            .unwrap_or_default();
        runtime_path_travel_distance(
            self.elapsed_seconds,
            maximum_speed * speed_scale,
            acceleration,
            speed_scale,
        )
    }

    pub(crate) fn dispatch(
        &mut self,
        trigger: &ProcessedTrigger,
        event_mask: u32,
        wall_time: f64,
        speed_scale: f32,
    ) {
        self.advance(wall_time);
        self.last_event = Some(event_mask);

        // Native handlers test activation first. A combined 0x300 mask therefore
        // behaves as activation rather than an immediate start/stop pair.
        if event_mask & ROBOTS_EVENT_ACTIVATE != 0 {
            if self.active {
                let retrigger_reverses =
                    trigger.ttype == 8
                        && trigger.data.get(7).copied().flatten().is_some_and(|flags| {
                            flags & ROBOTS_PLATFORM_RETRIGGER_REVERSE_FLAG != 0
                        });
                if retrigger_reverses {
                    let raw = self.raw_path_distance(trigger, speed_scale);
                    let current = self.distance_offset + self.direction * raw;
                    self.direction = -self.direction;
                    self.distance_offset = current - self.direction * raw;
                }
            } else {
                self.active = true;
            }
        } else if event_mask & ROBOTS_EVENT_DEACTIVATE != 0 {
            self.active = false;
        }
        self.last_wall_time = Some(wall_time);
    }

    pub(crate) fn record_node_dispatch(&mut self, node_index: usize, opcode: u16) {
        self.last_node_index = Some(node_index);
        self.last_node_opcode = Some(opcode);
    }

    pub(crate) fn snapshot(
        &self,
        trigger: &ProcessedTrigger,
        speed_scale: f32,
    ) -> RuntimeEventPreviewSnapshot {
        RuntimeEventPreviewSnapshot {
            active: self.active,
            elapsed_seconds: self.elapsed_seconds,
            path_distance: self.distance_offset
                + self.direction * self.raw_path_distance(trigger, speed_scale),
            direction_reversed: self.direction < 0.0,
            last_event: self.last_event,
            last_node_index: self.last_node_index,
            last_node_opcode: self.last_node_opcode,
            vehicle_steering_angle: (trigger.ttype == 80).then_some(self.vehicle_steering_angle),
            platform_contact_linear_velocity: matches!(trigger.ttype, 8 | 37 | 80)
                .then_some(self.platform_contact_linear_velocity),
        }
    }
}

pub(crate) fn runtime_path_travel_distance(
    elapsed_seconds: f32,
    maximum_speed: f32,
    acceleration_factor: f32,
    default_initial_speed: f32,
) -> f32 {
    const FIXED_HZ: f32 = 60.0;

    let frame_position = elapsed_seconds.max(0.0) * FIXED_HZ;
    let full_frames = frame_position.floor();
    let partial_frame = frame_position - full_frames;

    // Runtime update at 0x004230C9..0x004230F2:
    // current += (maximum - current) * acceleration.
    // A serialized non-zero acceleration starts current speed at zero. Without one,
    // setup initializes current speed to 1.0 and uses an effective factor of 1.0.
    let has_serialized_acceleration = acceleration_factor.abs() > f32::EPSILON;
    let effective_acceleration = if has_serialized_acceleration {
        acceleration_factor
    } else {
        1.0
    };
    let initial_speed = if has_serialized_acceleration {
        0.0
    } else {
        default_initial_speed
    };
    let remaining = 1.0 - effective_acceleration;
    let remaining_power = remaining.powi(full_frames as i32);
    let geometric_sum = (1.0 - remaining_power) / effective_acceleration;
    let full_frame_speed_sum =
        full_frames * maximum_speed + (initial_speed - maximum_speed) * geometric_sum;
    let current_speed = maximum_speed + (initial_speed - maximum_speed) * remaining_power;

    (full_frame_speed_sum + current_speed * partial_frame) / FIXED_HZ
}

const ROBOTS_VEHICLE_PASSIVE_WHEEL: Hashcode = 0x0200_017B;
const ROBOTS_VEHICLE_DRIVE_WHEEL: Hashcode = 0x0200_017A;
const ROBOTS_VEHICLE_STEERING_SMOOTHING: f32 = 0.1;

fn wrap_radians(angle: f32) -> f32 {
    (angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

fn vehicle_heading_at_time(
    map: &ProcessedMap,
    trigger: &ProcessedTrigger,
    elapsed_seconds: f32,
    speed_scale: f32,
) -> Option<f32> {
    let speed_scale = speed_scale.max(0.0);
    let maximum_speed =
        robots_trigger_runtime_path_speed(trigger.ttype, &trigger.data)? * speed_scale;
    let acceleration =
        robots_trigger_runtime_path_acceleration(trigger.ttype, &trigger.data).unwrap_or_default();
    let traveled =
        runtime_path_travel_distance(elapsed_seconds, maximum_speed, acceleration, speed_scale);
    runtime_path_preview_sample_at_distance(map, trigger, traveled)
        .and_then(|sample| robots_vehicle_yaw_from_tangent(sample.tangent))
}

pub(crate) fn robots_vehicle_steering_wheel_angle(
    map: &ProcessedMap,
    trigger: &ProcessedTrigger,
    elapsed_seconds: f32,
    speed_scale: f32,
) -> Option<f32> {
    if trigger.ttype != 80 {
        return None;
    }
    let mut state = RuntimeEventPreviewState {
        active: true,
        ..Default::default()
    };
    state.advance_vehicle_steering(map, trigger, elapsed_seconds.max(0.0), speed_scale);
    Some(state.vehicle_steering_angle)
}

pub(crate) fn robots_vehicle_wheel_roll_angle_unwrapped(
    elapsed_seconds: f32,
    maximum_speed: f32,
    acceleration_factor: f32,
    default_initial_speed: f32,
) -> f32 {
    const FIXED_HZ: f32 = 60.0;
    const MAX_EXACT_STEPS: usize = 200_000;

    let frame_position = elapsed_seconds.max(0.0) * FIXED_HZ;
    let full_frames = frame_position.floor() as usize;
    let partial_frame = frame_position - full_frames as f32;
    let has_serialized_acceleration = acceleration_factor.abs() > f32::EPSILON;
    let effective_acceleration = if has_serialized_acceleration {
        acceleration_factor
    } else {
        1.0
    };
    let mut current_speed = if has_serialized_acceleration {
        0.0
    } else {
        default_initial_speed
    };
    let mut angle = 0.0f32;
    let exact_steps = full_frames.min(MAX_EXACT_STEPS);
    for _ in 0..exact_steps {
        angle -= 2.0 * (current_speed * 0.02).clamp(-1.0, 1.0).asin();
        current_speed += (maximum_speed - current_speed) * effective_acceleration;
    }

    if full_frames > exact_steps {
        let remaining = full_frames - exact_steps;
        angle -= remaining as f32 * 2.0 * (maximum_speed * 0.02).clamp(-1.0, 1.0).asin();
        current_speed = maximum_speed;
    }
    angle -= partial_frame * 2.0 * (current_speed * 0.02).clamp(-1.0, 1.0).asin();
    angle
}

pub(crate) fn robots_vehicle_wheel_roll_angle(
    elapsed_seconds: f32,
    maximum_speed: f32,
    acceleration_factor: f32,
    default_initial_speed: f32,
) -> f32 {
    robots_vehicle_wheel_roll_angle_unwrapped(
        elapsed_seconds,
        maximum_speed,
        acceleration_factor,
        default_initial_speed,
    )
    .rem_euclid(std::f32::consts::TAU)
}

pub(crate) fn apply_vehicle_wheel_roll_angle(
    queue: &mut [QueuedEntityRender],
    render_store: &RenderStore,
    angle: f32,
) {
    let roll = Quat::from_rotation_x(angle.rem_euclid(std::f32::consts::TAU));
    for queued in queue {
        if queued.entity_alt.is_some() {
            continue;
        }
        if render_store.resolve_entity_hashcode(queued.entity.0, queued.entity.1)
            == Some(ROBOTS_VEHICLE_PASSIVE_WHEEL)
        {
            queued.rotation *= roll;
        }
    }
}

pub(crate) fn apply_vehicle_wheel_roll(
    queue: &mut [QueuedEntityRender],
    render_store: &RenderStore,
    elapsed_seconds: f32,
    maximum_speed: f32,
    acceleration_factor: f32,
    default_initial_speed: f32,
) {
    let angle = robots_vehicle_wheel_roll_angle(
        elapsed_seconds,
        maximum_speed,
        acceleration_factor,
        default_initial_speed,
    );
    // 0x0200017B is thin on local X and the runtime wheel record applies the
    // accumulated +0x0C angle after the assembly controller transform.
    apply_vehicle_wheel_roll_angle(queue, render_store, angle);
}

pub(crate) fn apply_vehicle_steering_wheel_angle(
    queue: &mut [QueuedEntityRender],
    render_store: &RenderStore,
    angle: f32,
) {
    let steering = Quat::from_rotation_y(angle);
    for queued in queue {
        if queued.entity_alt.is_some() {
            continue;
        }
        let resolved = render_store.resolve_entity_hashcode(queued.entity.0, queued.entity.1);
        if matches!(
            resolved,
            Some(ROBOTS_VEHICLE_DRIVE_WHEEL) | Some(ROBOTS_VEHICLE_PASSIVE_WHEEL)
        ) {
            // Both native wheel-update branches call 0x00419010, smooth
            // record+0x08 and compose Euler (0, steering, 0). Mode 0 passive
            // road wheels do this after their record+0x0C local-X roll; mode 1
            // drive/cab wheels apply steering without the road-wheel roll.
            queued.rotation *= steering;
        }
    }
}

pub(crate) fn sample_route(
    segments: &[(Vec3, Vec3)],
    mut distance: f32,
    reverse_tangent: bool,
    root_offset: Vec3,
) -> Option<RuntimePathSample> {
    for (start, end) in segments {
        let segment_length = start.distance(*end);
        if segment_length <= f32::EPSILON {
            continue;
        }
        if distance <= segment_length {
            let mut tangent = (*end - *start) / segment_length;
            if reverse_tangent {
                tangent = -tangent;
            }
            return Some(RuntimePathSample {
                position: start.lerp(*end, distance / segment_length) + root_offset,
                tangent,
            });
        }
        distance -= segment_length;
    }

    let (start, end) = segments.last().copied()?;
    let mut tangent = (end - start).normalize_or_zero();
    if reverse_tangent {
        tangent = -tangent;
    }
    Some(RuntimePathSample {
        position: end + root_offset,
        tangent,
    })
}

pub(crate) fn runtime_path_preview_sample_at_distance(
    map: &ProcessedMap,
    trigger: &ProcessedTrigger,
    traveled: f32,
) -> Option<RuntimePathSample> {
    let (_, _, path) = map_trigger_runtime_path(map, trigger)?;
    let route = runtime_path_route(path);
    if route.len() < 2 {
        return None;
    }

    // Shipped Vehicle paths carry 0x60000000 and are consumed by
    // XPathController_Vehicle as continuous traffic routes. Platform/Lift routes
    // retain the controller's end-to-end reversal preview.
    let looping = trigger.ttype == 80 && path.flags & 0x6000_0000 == 0x6000_0000;
    let segments = runtime_path_segments_for_motion(&route, looping);
    let total_length = segments
        .iter()
        .map(|(start, end)| start.distance(*end))
        .sum::<f32>();
    if total_length <= f32::EPSILON {
        return None;
    }

    let (start_phase, root_offset) = closest_route_phase(&segments, trigger.position);
    if looping {
        let distance = (start_phase + traveled).rem_euclid(total_length);
        sample_route(&segments, distance, traveled < 0.0, root_offset)
    } else {
        let cycle_length = total_length * 2.0;
        let phase = (start_phase + traveled).rem_euclid(cycle_length);
        let (distance, reverse_tangent) = if phase > total_length {
            (cycle_length - phase, true)
        } else {
            (phase, false)
        };
        sample_route(&segments, distance, reverse_tangent, root_offset)
    }
}

pub(crate) fn runtime_path_preview_sample(
    map: &ProcessedMap,
    trigger: &ProcessedTrigger,
    time: f32,
    animate: bool,
    speed_scale: f32,
) -> Option<RuntimePathSample> {
    if !animate || speed_scale <= f32::EPSILON {
        return runtime_path_preview_sample_at_distance(map, trigger, 0.0);
    }
    let speed_scale = speed_scale.max(0.0);
    let maximum_speed =
        robots_trigger_runtime_path_speed(trigger.ttype, &trigger.data)? * speed_scale;
    let acceleration =
        robots_trigger_runtime_path_acceleration(trigger.ttype, &trigger.data).unwrap_or_default();
    let traveled = runtime_path_travel_distance(time, maximum_speed, acceleration, speed_scale);
    runtime_path_preview_sample_at_distance(map, trigger, traveled)
}

pub(crate) fn runtime_path_preview_position(
    map: &ProcessedMap,
    trigger: &ProcessedTrigger,
    time: f32,
    animate: bool,
    speed_scale: f32,
) -> Vec3 {
    runtime_path_preview_sample(map, trigger, time, animate, speed_scale)
        .map(|sample| sample.position)
        .unwrap_or(trigger.position)
}

pub(crate) fn runtime_path_preview_position_with_event(
    map: &ProcessedMap,
    trigger: &ProcessedTrigger,
    time: f32,
    animate: bool,
    speed_scale: f32,
    event_snapshot: Option<RuntimeEventPreviewSnapshot>,
) -> Vec3 {
    if let Some(snapshot) = event_snapshot {
        return runtime_path_preview_sample_at_distance(map, trigger, snapshot.path_distance)
            .map(|sample| sample.position)
            .unwrap_or(trigger.position);
    }
    runtime_path_preview_position(map, trigger, time, animate, speed_scale)
}

pub(crate) fn runtime_platform_contact_linear_velocity(
    map: &ProcessedMap,
    trigger: &ProcessedTrigger,
    time: f32,
    animate: bool,
    speed_scale: f32,
) -> Option<Vec3> {
    if !matches!(trigger.ttype, 8 | 37 | 80) {
        return None;
    }
    if !animate {
        return Some(Vec3::ZERO);
    }
    const FIXED_STEP: f32 = 1.0 / 60.0;
    const FIXED_HZ: f32 = 60.0;
    let current_time = time.max(0.0);
    let previous_time = (current_time - FIXED_STEP).max(0.0);
    let current = runtime_path_preview_position(map, trigger, current_time, true, speed_scale);
    let previous = runtime_path_preview_position(map, trigger, previous_time, true, speed_scale);
    Some((current - previous) * FIXED_HZ)
}

pub(crate) fn trigger_base_rotation(trigger: &ProcessedTrigger) -> Quat {
    Quat::from_euler(
        glam::EulerRot::ZXY,
        trigger.rotation.z,
        trigger.rotation.x,
        trigger.rotation.y,
    )
}

pub(crate) fn runtime_platform_preview_rotation(
    trigger: &ProcessedTrigger,
    time: f32,
    animate: bool,
    speed_scale: f32,
) -> Quat {
    let base_rotation = trigger_base_rotation(trigger);
    if !animate {
        return base_rotation;
    }
    let Some(degrees_per_second) =
        robots_trigger_platform_angular_velocity(trigger.ttype, &trigger.data)
    else {
        return base_rotation;
    };

    let elapsed_degrees = degrees_per_second * time * speed_scale.max(0.0);
    let delta_rotation = Quat::from_euler(
        glam::EulerRot::ZXY,
        elapsed_degrees.z.to_radians(),
        elapsed_degrees.x.to_radians(),
        elapsed_degrees.y.to_radians(),
    );

    // XPathController_Platform writes the angular velocity directly to the physics body,
    // so the recovered vector is treated as world-axis rotation applied over the base pose.
    delta_rotation * base_rotation
}

pub(crate) fn robots_vehicle_yaw_from_tangent(tangent: Vec3) -> Option<f32> {
    let tangent_xz = Vec3::new(tangent.x, 0.0, tangent.z);
    (tangent_xz.length_squared() > f32::EPSILON)
        // XPathController_Vehicle::update at 0x00424C69 computes
        // atan2(-tangent.x, -tangent.z). Robots vehicle models face -Z.
        .then(|| (-tangent_xz.x).atan2(-tangent_xz.z))
}

pub(crate) fn runtime_trigger_preview_rotation_with_event(
    map: &ProcessedMap,
    trigger: &ProcessedTrigger,
    time: f32,
    animate: bool,
    path_speed_scale: f32,
    platform_rotation_speed_scale: f32,
    event_snapshot: Option<RuntimeEventPreviewSnapshot>,
) -> Quat {
    if let Some(snapshot) = event_snapshot {
        if trigger.ttype == 80 {
            if let Some(yaw) =
                runtime_path_preview_sample_at_distance(map, trigger, snapshot.path_distance)
                    .and_then(|sample| robots_vehicle_yaw_from_tangent(sample.tangent))
            {
                return Quat::from_euler(
                    glam::EulerRot::ZXY,
                    trigger.rotation.z,
                    trigger.rotation.x,
                    yaw,
                );
            }
        }
        return runtime_platform_preview_rotation(
            trigger,
            snapshot.elapsed_seconds,
            animate,
            platform_rotation_speed_scale,
        );
    }
    runtime_trigger_preview_rotation(
        map,
        trigger,
        time,
        animate,
        path_speed_scale,
        platform_rotation_speed_scale,
    )
}

pub(crate) fn runtime_trigger_preview_rotation(
    map: &ProcessedMap,
    trigger: &ProcessedTrigger,
    time: f32,
    animate: bool,
    path_speed_scale: f32,
    platform_rotation_speed_scale: f32,
) -> Quat {
    if !animate {
        return trigger_base_rotation(trigger);
    }

    if trigger.ttype == 80 {
        if let Some(yaw) =
            runtime_path_preview_sample(map, trigger, time, animate, path_speed_scale)
                .and_then(|sample| robots_vehicle_yaw_from_tangent(sample.tangent))
        {
            return Quat::from_euler(
                glam::EulerRot::ZXY,
                trigger.rotation.z,
                trigger.rotation.x,
                yaw,
            );
        }
    }

    runtime_platform_preview_rotation(trigger, time, animate, platform_rotation_speed_scale)
}
