use std::f32::consts::FRAC_PI_2;

use super::{
    locomotion::{shortest_yaw_delta, ROBOTS_FIXED_STEP_SECONDS},
    path_spline::RobotsNaturalCubicSpline3,
};

/// `AI_FollowFlyingPath` ctor `0x0046C620` initializes node priority to 0x0F.
/// EF01 overrides it to 0x1F immediately after insertion.
pub const ROBOTS_FOLLOW_FLYING_PATH_DEFAULT_PRIORITY: u8 = 0x0f;
pub const ROBOTS_EF01_FOLLOW_FLYING_PATH_PRIORITY: u8 = 0x1f;
/// EF01 first-update `0x00463970` configures node+0x24 as
/// `3.0 / (60.0 / runtime_rate_scale)`, i.e. 3 world units per second.
pub const ROBOTS_EF01_FOLLOW_FLYING_PATH_SPEED: f32 = 3.0;
pub const ROBOTS_EF01_FOLLOW_FLYING_PATH_ENTER_ANIM_MODE: u32 = 0x0900_0003;
pub const ROBOTS_FOLLOW_FLYING_PATH_TURN_RATE_RADIANS_PER_SECOND: f32 = FRAC_PI_2;
pub const ROBOTS_FOLLOW_FLYING_PATH_TURN_COMPLETE_EPSILON: f32 = f32::from_bits(0x3a83_126f); // 0.001
pub const ROBOTS_FOLLOW_FLYING_PATH_ENDPOINT_EPSILON: f32 = f32::from_bits(0x38d1_b717); // 0.0001

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RobotsFollowFlyingPathConfig {
    pub priority: u8,
    pub speed_units_per_second: f32,
    pub enter_anim_mode: Option<u32>,
}

impl RobotsFollowFlyingPathConfig {
    pub const fn ef01() -> Self {
        Self {
            priority: ROBOTS_EF01_FOLLOW_FLYING_PATH_PRIORITY,
            speed_units_per_second: ROBOTS_EF01_FOLLOW_FLYING_PATH_SPEED,
            enter_anim_mode: Some(ROBOTS_EF01_FOLLOW_FLYING_PATH_ENTER_ANIM_MODE),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RobotsFollowFlyingPathPhase {
    #[default]
    Moving,
    TurningAtEndpoint,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RobotsFollowFlyingPathRuntimeState {
    spline: Option<RobotsNaturalCubicSpline3>,
    looping: bool,
    source_point_count: usize,
    parameter: f32,
    reverse: bool,
    phase: RobotsFollowFlyingPathPhase,
}

impl Default for RobotsFollowFlyingPathRuntimeState {
    fn default() -> Self {
        Self {
            spline: None,
            looping: false,
            source_point_count: 0,
            parameter: 0.0,
            reverse: false,
            phase: RobotsFollowFlyingPathPhase::Moving,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RobotsFollowFlyingPathStep {
    /// Native `0x0046C760` writes the sampled spline XYZ directly to owner +0xD0.
    /// During the endpoint turn-only phase native leaves owner translation untouched.
    pub owner_position_xyz: Option<[f32; 3]>,
    pub owner_yaw_radians: f32,
    pub path_parameter: f32,
    pub reverse: bool,
    pub phase: RobotsFollowFlyingPathPhase,
}

pub const fn follow_flying_path_priority(config: RobotsFollowFlyingPathConfig) -> u8 {
    config.priority
}

impl RobotsFollowFlyingPathRuntimeState {
    /// Native setup `0x0046C660` builds the shared natural-cubic XPath spline.
    /// Path type 1 appends the first three points before `0x00449A90` finalizes
    /// second derivatives, allowing a smooth last->first loop segment.
    pub fn bind_path(&mut self, source_points: &[[f32; 3]], looping: bool) -> bool {
        if source_points.is_empty() || (looping && source_points.len() < 3) {
            self.clear_path();
            return false;
        }

        let mut spline_points = source_points.to_vec();
        if looping {
            spline_points.extend_from_slice(&source_points[..3]);
        }
        let Some(spline) = RobotsNaturalCubicSpline3::from_points(spline_points) else {
            self.clear_path();
            return false;
        };

        self.spline = Some(spline);
        self.looping = looping;
        self.source_point_count = source_points.len();
        self.parameter = 0.0;
        self.reverse = false;
        self.phase = RobotsFollowFlyingPathPhase::Moving;
        true
    }

    pub fn clear_path(&mut self) {
        *self = Self::default();
    }

    pub fn path_bound(&self) -> bool {
        self.spline.is_some()
    }

    pub fn path_parameter(&self) -> f32 {
        self.parameter
    }

    pub fn reverse(&self) -> bool {
        self.reverse
    }

    pub fn phase(&self) -> RobotsFollowFlyingPathPhase {
        self.phase
    }

    fn endpoint_parameter(&self) -> f32 {
        if self.looping {
            self.source_point_count as f32
        } else {
            self.source_point_count.saturating_sub(1) as f32
        }
    }

    fn target_yaw(&self) -> Option<f32> {
        let spline = self.spline.as_ref()?;
        // `0x00449F30(parameter, +/-1)` samples parameter +/-0.5 and subtracts.
        let tangent =
            spline.finite_difference(self.parameter, if self.reverse { -1.0 } else { 1.0 });
        if !tangent.iter().all(|value| value.is_finite())
            || tangent[0] * tangent[0] + tangent[2] * tangent[2] <= f32::EPSILON
        {
            return None;
        }
        Some(tangent[0].atan2(tangent[2]))
    }

    fn turn_toward_path(&self, current_yaw_radians: f32, runtime_rate_scale: f32) -> (f32, bool) {
        let Some(target_yaw_radians) = self.target_yaw() else {
            return (current_yaw_radians, true);
        };
        let delta = shortest_yaw_delta(current_yaw_radians, target_yaw_radians);
        let max_delta = ROBOTS_FOLLOW_FLYING_PATH_TURN_RATE_RADIANS_PER_SECOND
            * runtime_rate_scale.max(0.0)
            * ROBOTS_FIXED_STEP_SECONDS;
        let applied = delta.clamp(-max_delta, max_delta);
        (
            current_yaw_radians + applied,
            (delta - applied).abs() < ROBOTS_FOLLOW_FLYING_PATH_TURN_COMPLETE_EPSILON,
        )
    }

    /// Exact high-level phase order of `0x0046C760 -> 0x0046C910`:
    /// endpoint service first, then spline advance/sample and yaw correction only
    /// when state is moving. A completed endpoint turn can resume movement in the
    /// same fixed update.
    pub fn advance_fixed(
        &mut self,
        config: RobotsFollowFlyingPathConfig,
        current_owner_yaw_radians: f32,
        runtime_rate_scale: f32,
    ) -> Option<RobotsFollowFlyingPathStep> {
        let spline = self.spline.as_ref()?;
        let mut owner_yaw_radians = current_owner_yaw_radians;

        match self.phase {
            RobotsFollowFlyingPathPhase::Moving => {
                if !self.reverse {
                    let endpoint = self.endpoint_parameter();
                    if self.parameter > endpoint - ROBOTS_FOLLOW_FLYING_PATH_ENDPOINT_EPSILON {
                        if self.looping {
                            self.parameter = 0.0;
                        } else {
                            self.reverse = true;
                            self.phase = RobotsFollowFlyingPathPhase::TurningAtEndpoint;
                        }
                    }
                } else if self.parameter < ROBOTS_FOLLOW_FLYING_PATH_ENDPOINT_EPSILON {
                    self.reverse = false;
                    self.phase = RobotsFollowFlyingPathPhase::TurningAtEndpoint;
                }
            }
            RobotsFollowFlyingPathPhase::TurningAtEndpoint => {
                let (yaw, complete) = self.turn_toward_path(owner_yaw_radians, runtime_rate_scale);
                owner_yaw_radians = yaw;
                if complete {
                    self.phase = RobotsFollowFlyingPathPhase::Moving;
                }
            }
        }

        if self.phase == RobotsFollowFlyingPathPhase::Moving {
            let signed_distance = config.speed_units_per_second
                * runtime_rate_scale.max(0.0)
                * ROBOTS_FIXED_STEP_SECONDS
                * if self.reverse { -1.0 } else { 1.0 };
            self.parameter = spline.advance_parameter_by_distance(self.parameter, signed_distance);
            let position = spline.sample(self.parameter);
            let (yaw, _) = self.turn_toward_path(owner_yaw_radians, runtime_rate_scale);
            owner_yaw_radians = yaw;
            return Some(RobotsFollowFlyingPathStep {
                owner_position_xyz: Some(position),
                owner_yaw_radians,
                path_parameter: self.parameter,
                reverse: self.reverse,
                phase: self.phase,
            });
        }

        Some(RobotsFollowFlyingPathStep {
            owner_position_xyz: None,
            owner_yaw_radians,
            path_parameter: self.parameter,
            reverse: self.reverse,
            phase: self.phase,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ef01_config_matches_native_first_update_constants() {
        let config = RobotsFollowFlyingPathConfig::ef01();
        assert_eq!(config.priority, 0x1f);
        assert_eq!(config.speed_units_per_second.to_bits(), 3.0f32.to_bits());
        assert_eq!(config.enter_anim_mode, Some(0x0900_0003));
        assert_eq!(
            ROBOTS_FOLLOW_FLYING_PATH_TURN_RATE_RADIANS_PER_SECOND.to_bits(),
            FRAC_PI_2.to_bits()
        );
        assert_eq!(
            ROBOTS_FOLLOW_FLYING_PATH_ENDPOINT_EPSILON.to_bits(),
            0x38d1_b717
        );
    }

    #[test]
    fn straight_path_advances_exactly_point_zero_five_world_units_per_fixed_tick() {
        let mut runtime = RobotsFollowFlyingPathRuntimeState::default();
        assert!(runtime.bind_path(&[[0.0, 0.0, 0.0], [10.0, 0.0, 0.0]], false));
        let step = runtime
            .advance_fixed(RobotsFollowFlyingPathConfig::ef01(), FRAC_PI_2, 1.0)
            .unwrap();
        let position = step.owner_position_xyz.unwrap();
        assert!((position[0] - 0.05).abs() < 1.0e-5);
        assert!(position[1].abs() < 1.0e-6);
        assert!(position[2].abs() < 1.0e-6);
    }

    #[test]
    fn non_loop_endpoint_enters_turn_phase_without_moving_translation() {
        let mut runtime = RobotsFollowFlyingPathRuntimeState::default();
        assert!(runtime.bind_path(&[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]], false));
        let config = RobotsFollowFlyingPathConfig::ef01();
        for _ in 0..20 {
            let _ = runtime.advance_fixed(config, FRAC_PI_2, 1.0).unwrap();
        }
        assert!((runtime.path_parameter() - 1.0).abs() < 1.0e-6);
        let endpoint = runtime.advance_fixed(config, FRAC_PI_2, 1.0).unwrap();
        assert_eq!(endpoint.owner_position_xyz, None);
        assert!(endpoint.reverse);
        assert_eq!(
            endpoint.phase,
            RobotsFollowFlyingPathPhase::TurningAtEndpoint
        );
    }

    #[test]
    fn loop_setup_appends_three_points_and_wraps_without_reverse_phase() {
        let mut runtime = RobotsFollowFlyingPathRuntimeState::default();
        assert!(runtime.bind_path(
            &[
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
            ],
            true,
        ));
        assert_eq!(runtime.endpoint_parameter(), 4.0);
        assert!(!runtime.reverse());
        assert_eq!(runtime.phase(), RobotsFollowFlyingPathPhase::Moving);
    }
}
