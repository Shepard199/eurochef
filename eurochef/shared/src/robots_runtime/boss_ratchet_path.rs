use std::f32::consts::PI;

pub const ROBOTS_BOSS_RATCHET_PARAMETER_BLEND: f32 = 0.1;
/// `0x004CAF9D..0x004CB025`: sampled path pose is approached by eight percent
/// before the resulting delta is converted into Physics +0x1B8 motion.
pub const ROBOTS_BOSS_RATCHET_POSITION_BLEND: f32 = 0.08;
/// `0x004CB08B..0x004CB0AB` multiplies the per-tick displacement by 60 before
/// writing Character-Physics motion; the base 60 Hz integration cancels it.
pub const ROBOTS_BOSS_RATCHET_PHYSICS_RATE: f32 = 60.0;
pub const ROBOTS_BOSS_RATCHET_DISTANCE_STEP: f32 = 0.1;
pub const ROBOTS_BOSS_RATCHET_DISTANCE_BIAS: f32 = 12.0;
pub const ROBOTS_BOSS_RATCHET_PATH_EVENT_SWITCH: u16 = 0x09;
pub const ROBOTS_BOSS_RATCHET_PATH_EVENT_OFFSET: u16 = 0x0c;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RobotsBossRatchetPathEvent {
    pub value: [u16; 4],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RobotsBossRatchetPathStep {
    pub owner_position_xyz: [f32; 3],
    pub owner_yaw_radians: f32,
    pub tracked_player_parameter: f32,
    pub sampled_parameter: f32,
    pub distance_offset: f32,
    pub requested_path_uid: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RobotsBossRatchetPathRuntimeState {
    points: Vec<[f32; 3]>,
    node_events: Vec<RobotsBossRatchetPathEvent>,
    tracked_player_parameter: f32,
    event_node_index: i32,
    distance_offset: f32,
    target_distance_offset: f32,
}

impl Default for RobotsBossRatchetPathRuntimeState {
    fn default() -> Self {
        Self {
            points: Vec::new(),
            node_events: Vec::new(),
            tracked_player_parameter: 0.0,
            event_node_index: -1,
            distance_offset: 0.0,
            target_distance_offset: 0.0,
        }
    }
}

impl RobotsBossRatchetPathRuntimeState {
    pub fn bind_path(&mut self, points: &[[f32; 3]], node_values: &[[u16; 4]]) -> bool {
        if points.is_empty() || points.len() != node_values.len() {
            self.clear_path();
            return false;
        }
        self.points = points.to_vec();
        self.node_events = node_values
            .iter()
            .copied()
            .map(|value| RobotsBossRatchetPathEvent { value })
            .collect();
        self.tracked_player_parameter = 0.0;
        self.event_node_index = -1;
        self.distance_offset = 0.0;
        self.target_distance_offset = 0.0;
        true
    }

    pub fn clear_path(&mut self) {
        *self = Self::default();
    }

    pub fn path_bound(&self) -> bool {
        !self.points.is_empty()
    }

    pub fn tracked_player_parameter(&self) -> f32 {
        self.tracked_player_parameter
    }

    /// Host adapter for native `XPath::GetNearestPoint` (`0x0041FDF0`).
    /// BossRatchet feeds this parameter directly into `0x004CACF0`.
    pub fn nearest_player_parameter(&self, point: [f32; 3], looped: bool) -> Option<f32> {
        // Shipped m08 BossRatchet uses the default non-loop XPath. Do not silently
        // fall back to the old cubic preview when an unexpected looped path appears.
        if looped {
            return None;
        }
        Self::native_nearest_parameter(&self.points, point)
    }

    pub fn event_node_index(&self) -> i32 {
        self.event_node_index
    }

    pub fn distance_offset(&self) -> f32 {
        self.distance_offset
    }

    pub fn target_distance_offset(&self) -> f32 {
        self.target_distance_offset
    }

    fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
        [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
    }

    fn add_scaled(a: [f32; 3], delta: [f32; 3], scale: f32) -> [f32; 3] {
        [
            a[0] + delta[0] * scale,
            a[1] + delta[1] * scale,
            a[2] + delta[2] * scale,
        ]
    }

    fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }

    fn length_squared(v: [f32; 3]) -> f32 {
        Self::dot(v, v)
    }

    fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
        Self::length_squared(Self::sub(a, b)).sqrt()
    }

    /// Native default XPath sample (0x00420710): integer node plus linear fraction.
    fn native_position_at_parameter(
        points: &[[f32; 3]],
        parameter: f32,
    ) -> Option<[f32; 3]> {
        let last = points.len().checked_sub(1)?;
        let parameter = parameter.clamp(0.0, last as f32);
        let index = parameter.floor() as usize;
        let current = *points.get(index)?;
        let Some(next) = points.get(index + 1).copied() else {
            return Some(current);
        };
        Some(Self::add_scaled(
            current,
            Self::sub(next, current),
            parameter - index as f32,
        ))
    }

    /// Native default XPath tangent (0x00420600).
    fn native_tangent_at_parameter(
        points: &[[f32; 3]],
        parameter: f32,
    ) -> Option<[f32; 3]> {
        if points.is_empty() {
            return None;
        }
        if points.len() == 1 {
            return Some([0.0; 3]);
        }
        let last = points.len() - 1;
        let index = parameter.clamp(0.0, last as f32).floor() as usize;
        let (start, end) = if index < last {
            (points[index], points[index + 1])
        } else {
            (points[last - 1], points[last])
        };
        Some(Self::sub(end, start))
    }

    fn native_segment_projection(
        start: [f32; 3],
        end: [f32; 3],
        position: [f32; 3],
    ) -> (f32, f32) {
        let delta = Self::sub(end, start);
        let length_squared = Self::length_squared(delta);
        if length_squared <= f32::EPSILON {
            return (0.0, Self::length_squared(Self::sub(start, position)));
        }
        let fraction =
            (Self::dot(Self::sub(position, start), delta) / length_squared).clamp(0.0, 1.0);
        let point = Self::add_scaled(start, delta, fraction);
        (
            fraction,
            Self::length_squared(Self::sub(point, position)),
        )
    }

    /// Native XPath GetNearestPoint (0x0041FDF0).
    fn native_nearest_parameter(
        points: &[[f32; 3]],
        position: [f32; 3],
    ) -> Option<f32> {
        let mut nearest = 0usize;
        let mut nearest_distance_squared =
            Self::length_squared(Self::sub(*points.first()?, position));
        for (index, point) in points.iter().copied().enumerate().skip(1) {
            let distance_squared = Self::length_squared(Self::sub(point, position));
            if distance_squared < nearest_distance_squared {
                nearest = index;
                nearest_distance_squared = distance_squared;
            }
        }

        let previous = nearest.checked_sub(1).map(|start| {
            let (fraction, distance_squared) =
                Self::native_segment_projection(points[start], points[nearest], position);
            (start as f32 + fraction, distance_squared)
        });
        let next = points.get(nearest + 1).map(|point| {
            let (fraction, distance_squared) =
                Self::native_segment_projection(points[nearest], *point, position);
            (nearest as f32 + fraction, distance_squared)
        });

        match (previous, next) {
            (Some(previous), Some(next)) => {
                Some(if next.1 <= previous.1 { next.0 } else { previous.0 })
            }
            (Some(previous), None) => Some(previous.0),
            (None, Some(next)) => Some(next.0),
            (None, None) => Some(nearest as f32),
        }
    }

    /// Native non-loop XPath distance advance (0x00420B60).
    fn native_non_loop_advance_parameter(
        points: &[[f32; 3]],
        parameter: f32,
        distance: f32,
    ) -> Option<f32> {
        let last = points.len().checked_sub(1)?;
        let mut parameter = parameter.clamp(0.0, last as f32);
        if distance == 0.0 || last == 0 {
            return Some(parameter);
        }

        let mut remaining = distance;
        if remaining > 0.0 {
            loop {
                let index = parameter.floor() as usize;
                if index >= last {
                    return Some(last as f32);
                }
                let current = Self::native_position_at_parameter(points, parameter)?;
                let next_parameter = (index + 1) as f32;
                let next = points[index + 1];
                let segment = Self::distance(current, next);
                if remaining <= segment {
                    if segment <= f32::EPSILON {
                        return Some(next_parameter);
                    }
                    return Some(
                        parameter + (remaining / segment) * (next_parameter - parameter),
                    );
                }
                remaining -= segment;
                parameter = next_parameter;
                if remaining <= 0.001 {
                    return Some(parameter);
                }
            }
        }

        let mut current = Self::native_position_at_parameter(points, parameter)?;
        let mut target_index = parameter.floor() as usize;
        loop {
            let previous_parameter = target_index as f32;
            let previous = points[target_index];
            let segment = Self::distance(current, previous);
            if -remaining <= segment {
                if segment <= f32::EPSILON {
                    if target_index == 0 {
                        return Some(0.0);
                    }
                    target_index -= 1;
                    continue;
                }
                return Some(
                    parameter + (remaining / segment) * (parameter - previous_parameter),
                );
            }
            remaining += segment;
            parameter = previous_parameter;
            current = previous;
            if target_index == 0 {
                return Some(0.0);
            }
            if remaining >= -0.001 {
                return Some(parameter);
            }
            target_index -= 1;
        }
    }

    fn follow_parameter(current: f32, target: f32, runtime_rate_scale: f32) -> f32 {
        let alpha = runtime_rate_scale.max(0.0) * ROBOTS_BOSS_RATCHET_PARAMETER_BLEND;
        if alpha >= 1.0 {
            target
        } else {
            current + (target - current) * alpha
        }
    }

    fn approach_distance(current: f32, target: f32, runtime_rate_scale: f32) -> f32 {
        let step = runtime_rate_scale.max(0.0) * ROBOTS_BOSS_RATCHET_DISTANCE_STEP;
        if target - current > step {
            current + step
        } else if target - current < -step {
            current - step
        } else {
            current
        }
    }

    fn service_event(&mut self, index: usize) -> Option<u32> {
        let event = self.node_events.get(index).copied()?;
        match event.value[0] {
            ROBOTS_BOSS_RATCHET_PATH_EVENT_SWITCH => {
                let path_index = u32::from(event.value[1]);
                (path_index != 0).then_some(0x0b00_0000 | path_index)
            }
            ROBOTS_BOSS_RATCHET_PATH_EVENT_OFFSET => {
                let raw = f32::from(event.value[1]);
                let signed = if event.value[2] > 0 { -raw } else { raw };
                self.target_distance_offset = signed + ROBOTS_BOSS_RATCHET_DISTANCE_BIAS;
                None
            }
            _ => None,
        }
    }

    /// Engine-neutral m08 BossRatchet path service (`0x004CACF0`).
    /// `nearest_player_parameter` is the exact native path vslot+0x10 result;
    /// the host owns that spatial projection rather than guessing it here.
    pub fn advance_fixed(
        &mut self,
        nearest_player_parameter: f32,
        runtime_rate_scale: f32,
    ) -> Option<RobotsBossRatchetPathStep> {
        if !nearest_player_parameter.is_finite() || !runtime_rate_scale.is_finite() {
            return None;
        }
        if self.points.is_empty() {
            return None;
        }

        self.tracked_player_parameter = Self::follow_parameter(
            self.tracked_player_parameter,
            nearest_player_parameter,
            runtime_rate_scale,
        );

        let max_index = self.node_events.len().saturating_sub(1) as i32;
        let floor_index = (self.tracked_player_parameter.floor().max(0.0) as i32).min(max_index);
        let mut requested_path_uid = None;
        while self.event_node_index != floor_index {
            if self.event_node_index > floor_index {
                self.event_node_index = floor_index;
            } else {
                self.event_node_index += 1;
            }
            if self.event_node_index >= 0 {
                requested_path_uid =
                    self.service_event(self.event_node_index as usize).or(requested_path_uid);
            }
        }

        self.distance_offset = Self::approach_distance(
            self.distance_offset,
            self.target_distance_offset,
            runtime_rate_scale,
        );

        let sampled_parameter = Self::native_non_loop_advance_parameter(
            &self.points,
            self.tracked_player_parameter,
            self.distance_offset,
        )?;
        let owner_position_xyz =
            Self::native_position_at_parameter(&self.points, sampled_parameter)?;
        let tangent = Self::native_tangent_at_parameter(&self.points, sampled_parameter)?;
        let owner_yaw_radians = tangent[0].atan2(tangent[2]) + PI;

        Some(RobotsBossRatchetPathStep {
            owner_position_xyz,
            owner_yaw_radians,
            tracked_player_parameter: self.tracked_player_parameter,
            sampled_parameter,
            distance_offset: self.distance_offset,
            requested_path_uid,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bind(values: &[[u16; 4]]) -> RobotsBossRatchetPathRuntimeState {
        let mut state = RobotsBossRatchetPathRuntimeState::default();
        let points: Vec<_> = (0..values.len())
            .map(|i| [i as f32 * 10.0, 0.0, 0.0])
            .collect();
        assert!(state.bind_path(&points, values));
        state
    }

    #[test]
    fn ctor_defaults_match_004cab30() {
        let state = RobotsBossRatchetPathRuntimeState::default();
        assert_eq!(state.event_node_index(), -1);
        assert_eq!(state.tracked_player_parameter().to_bits(), 0.0f32.to_bits());
        assert_eq!(state.distance_offset().to_bits(), 0.0f32.to_bits());
        assert_eq!(state.target_distance_offset().to_bits(), 0.0f32.to_bits());
    }

    #[test]
    fn player_parameter_follows_native_ten_percent_helper() {
        let mut state = bind(&[[0; 4], [0; 4], [0; 4]]);
        let step = state.advance_fixed(2.0, 1.0).unwrap();
        assert!((step.tracked_player_parameter - 0.2).abs() < 1.0e-6);
        let step = state.advance_fixed(2.0, 1.0).unwrap();
        assert!((step.tracked_player_parameter - 0.38).abs() < 1.0e-6);
    }

    #[test]
    fn event_0c_sets_signed_target_plus_twelve_and_distance_uses_native_dead_zone() {
        let mut state = bind(&[[12, 10, 1, 0], [0; 4]]);
        let step = state.advance_fixed(0.0, 1.0).unwrap();
        assert_eq!(state.event_node_index(), 0);
        assert_eq!(state.target_distance_offset().to_bits(), 2.0f32.to_bits());
        assert!((step.distance_offset - 0.1).abs() < 1.0e-6);

        state.distance_offset = 1.95;
        let step = state.advance_fixed(0.0, 1.0).unwrap();
        assert_eq!(step.distance_offset.to_bits(), 1.95f32.to_bits());
    }

    #[test]
    fn event_cursor_catches_up_forward_and_resets_directly_when_player_moves_backward() {
        let mut state = bind(&[
            [12, 10, 1, 0],
            [12, 3, 1, 0],
            [12, 7, 0, 0],
            [0; 4],
        ]);
        state.tracked_player_parameter = 2.0;
        state.event_node_index = -1;
        let _ = state.advance_fixed(2.0, 0.0).unwrap();
        assert_eq!(state.event_node_index(), 2);
        assert_eq!(state.target_distance_offset().to_bits(), 19.0f32.to_bits());

        state.tracked_player_parameter = 0.0;
        let _ = state.advance_fixed(0.0, 0.0).unwrap();
        assert_eq!(state.event_node_index(), 0);
        assert_eq!(state.target_distance_offset().to_bits(), 2.0f32.to_bits());
    }

    #[test]
    fn native_xpath_sampling_is_piecewise_linear_on_corners() {
        let mut state = RobotsBossRatchetPathRuntimeState::default();
        let points = [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [10.0, 0.0, 10.0]];
        let values = [[0; 4]; 3];
        assert!(state.bind_path(&points, &values));

        assert_eq!(
            RobotsBossRatchetPathRuntimeState::native_position_at_parameter(&state.points, 0.5),
            Some([5.0, 0.0, 0.0])
        );
        assert_eq!(
            RobotsBossRatchetPathRuntimeState::native_position_at_parameter(&state.points, 1.5),
            Some([10.0, 0.0, 5.0])
        );
        assert_eq!(
            RobotsBossRatchetPathRuntimeState::native_tangent_at_parameter(&state.points, 1.5),
            Some([0.0, 0.0, 10.0])
        );
        let nearest =
            RobotsBossRatchetPathRuntimeState::native_nearest_parameter(&state.points, [9.0, 0.0, 4.0])
                .expect("nearest parameter");
        assert!((nearest - 1.4).abs() < 1.0e-6);
    }

    #[test]
    fn event_09_reports_native_path_uid_without_guessing_host_rebind_order() {
        let mut state = bind(&[[9, 0x1f, 0, 0], [0; 4]]);
        let step = state.advance_fixed(0.0, 1.0).unwrap();
        assert_eq!(step.requested_path_uid, Some(0x0b00_001f));
    }
}
