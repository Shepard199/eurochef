/// Shared engine-neutral projection of the native XPath runtime built by
/// `0x00449A50/0x00449EA0/0x00449A90` and sampled by `0x0044A010`.
///
/// Camera mode4, Monster Transporter and WatchBot mode3 consume this same
/// natural-cubic path family. Keeping the math here avoids engine-specific
/// copies when the gameplay layer is moved to UE5.8.
pub const ROBOTS_XPATH_PARAMETER_STEP: f32 = f32::from_bits(0x3DCC_CCCD); // 0.1f
pub const ROBOTS_XPATH_DISTANCE_EPSILON: f32 = f32::from_bits(0x3A83_126F); // 0.001f
pub const ROBOTS_XPATH_INTERVAL_REFINEMENT: f32 = f32::from_bits(0x3F00_0000); // 0.5f
pub const ROBOTS_XPATH_INTERVAL_REFINEMENT_STEPS: usize = 8;

#[derive(Clone, Debug, PartialEq)]
pub struct RobotsNaturalCubicSpline3 {
    points: Vec<[f32; 3]>,
    second_derivatives: Vec<[f32; 3]>,
}

impl RobotsNaturalCubicSpline3 {
    pub fn from_points(points: Vec<[f32; 3]>) -> Option<Self> {
        if points.is_empty()
            || points
                .iter()
                .flatten()
                .any(|component| !component.is_finite())
        {
            return None;
        }

        let mut second_derivatives = vec![[0.0; 3]; points.len()];
        if points.len() > 2 {
            let interior = points.len() - 2;
            let mut c_prime = vec![0.0f32; interior];
            let mut d_prime = vec![[0.0; 3]; interior];
            for interior_index in 0..interior {
                let i = interior_index + 1;
                let rhs = mul3(
                    add3(sub3(points[i + 1], mul3(points[i], 2.0)), points[i - 1]),
                    6.0,
                );
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
                    div3(rhs, denom)
                } else {
                    div3(sub3(rhs, d_prime[interior_index - 1]), denom)
                };
            }
            for interior_index in (0..interior).rev() {
                let i = interior_index + 1;
                second_derivatives[i] = if interior_index + 1 == interior {
                    d_prime[interior_index]
                } else {
                    sub3(
                        d_prime[interior_index],
                        mul3(second_derivatives[i + 1], c_prime[interior_index]),
                    )
                };
            }
        }

        Some(Self {
            points,
            second_derivatives,
        })
    }

    pub fn points(&self) -> &[[f32; 3]] {
        &self.points
    }

    pub fn last_parameter(&self) -> f32 {
        self.points.len().saturating_sub(1) as f32
    }

    pub fn sample(&self, parameter: f32) -> [f32; 3] {
        if self.points.len() == 1 {
            return self.points[0];
        }
        let parameter = parameter.max(0.0);
        let last = self.last_parameter();
        if parameter >= last {
            return *self.points.last().unwrap_or(&[0.0; 3]);
        }

        let lower = parameter.floor() as usize;
        let upper = lower + 1;
        let b = parameter - lower as f32;
        let a = 1.0 - b;
        let cubic_a = a * a * a - a;
        let cubic_b = b * b * b - b;
        add3(
            add3(mul3(self.points[lower], a), mul3(self.points[upper], b)),
            mul3(
                add3(
                    mul3(self.second_derivatives[lower], cubic_a),
                    mul3(self.second_derivatives[upper], cubic_b),
                ),
                1.0 / 6.0,
            ),
        )
    }

    pub fn finite_difference(&self, parameter: f32, width: f32) -> [f32; 3] {
        let half = width * 0.5;
        sub3(self.sample(parameter + half), self.sample(parameter - half))
    }

    pub fn nearest_node_index(&self, point: [f32; 3]) -> usize {
        self.points
            .iter()
            .enumerate()
            .min_by(|(_, lhs), (_, rhs)| {
                distance_squared3(**lhs, point).total_cmp(&distance_squared3(**rhs, point))
            })
            .map(|(index, _)| index)
            .unwrap_or_default()
    }

    /// Engine-neutral form of native distance-to-parameter helper `0x0044A1E0`.
    /// It advances through the same spline in exact 0.1 parameter increments.
    pub fn advance_parameter_by_distance(&self, parameter: f32, distance: f32) -> f32 {
        if distance == 0.0 || self.points.len() < 2 {
            return parameter.clamp(0.0, self.last_parameter());
        }

        let direction = distance.signum();
        let mut remaining = distance.abs();
        let mut current_parameter = parameter.clamp(0.0, self.last_parameter());
        let last = self.last_parameter();
        while remaining > ROBOTS_XPATH_DISTANCE_EPSILON {
            let next_parameter = current_parameter + direction * ROBOTS_XPATH_PARAMETER_STEP;
            let current = self.sample(current_parameter);
            let next = self.sample(next_parameter);
            let segment_length = distance_squared3(current, next).sqrt();
            if segment_length > f32::EPSILON && remaining <= segment_length {
                current_parameter += direction
                    * ROBOTS_XPATH_PARAMETER_STEP
                    * (remaining / segment_length).clamp(0.0, 1.0);
                return current_parameter.clamp(0.0, last);
            }
            remaining = (remaining - segment_length).max(0.0);
            current_parameter = next_parameter;
            let following_parameter = current_parameter + direction * ROBOTS_XPATH_PARAMETER_STEP;
            if remaining <= ROBOTS_XPATH_DISTANCE_EPSILON
                || following_parameter >= last
                || following_parameter <= 0.0
            {
                break;
            }
        }
        current_parameter.clamp(0.0, last)
    }

    /// Exact control flow of native adjacent-segment nearest-parameter helper
    /// `0x0044A650`: invalid bounds collapse to the lower endpoint, then at most
    /// eight endpoint comparisons halve the losing side of the interval.
    pub fn nearest_parameter_in_interval(
        &self,
        point: [f32; 3],
        lower_index: i16,
        upper_index: i16,
    ) -> f32 {
        if self.points.is_empty() {
            return 0.0;
        }
        let count = self.points.len() as i32;
        let lower_index = if lower_index < 0 || lower_index as i32 >= count {
            0
        } else {
            lower_index
        };
        let upper_index = if upper_index < 0 || upper_index as i32 >= count {
            lower_index
        } else {
            upper_index
        };

        let mut lower = lower_index as f32;
        let mut upper = upper_index as f32;
        let mut result = 0.0;
        for _ in 0..ROBOTS_XPATH_INTERVAL_REFINEMENT_STEPS {
            let lower_distance = distance_squared3(self.sample(lower), point);
            let upper_distance = distance_squared3(self.sample(upper), point);
            if lower_distance == 0.0 || upper_distance == 0.0 {
                result = if upper_distance <= lower_distance {
                    upper
                } else {
                    lower
                };
                break;
            }
            if upper_distance <= lower_distance {
                result = upper;
                lower += ((lower - upper) * ROBOTS_XPATH_INTERVAL_REFINEMENT).abs();
            } else {
                result = lower;
                upper -= ((lower - upper) * ROBOTS_XPATH_INTERVAL_REFINEMENT).abs();
            }
        }
        result
    }
}

fn add3(lhs: [f32; 3], rhs: [f32; 3]) -> [f32; 3] {
    [lhs[0] + rhs[0], lhs[1] + rhs[1], lhs[2] + rhs[2]]
}

fn sub3(lhs: [f32; 3], rhs: [f32; 3]) -> [f32; 3] {
    [lhs[0] - rhs[0], lhs[1] - rhs[1], lhs[2] - rhs[2]]
}

fn mul3(value: [f32; 3], scalar: f32) -> [f32; 3] {
    [value[0] * scalar, value[1] * scalar, value[2] * scalar]
}

fn div3(value: [f32; 3], scalar: f32) -> [f32; 3] {
    [value[0] / scalar, value[1] / scalar, value[2] / scalar]
}

fn distance_squared3(lhs: [f32; 3], rhs: [f32; 3]) -> f32 {
    let dx = lhs[0] - rhs[0];
    let dy = lhs[1] - rhs[1];
    let dz = lhs[2] - rhs[2];
    dx * dx + dy * dy + dz * dz
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linear() -> RobotsNaturalCubicSpline3 {
        RobotsNaturalCubicSpline3::from_points(vec![
            [0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [20.0, 0.0, 0.0],
        ])
        .unwrap()
    }

    #[test]
    fn natural_cubic_sampling_matches_linear_three_point_path() {
        let spline = linear();
        assert_eq!(spline.sample(0.5), [5.0, 0.0, 0.0]);
        assert_eq!(spline.sample(1.5), [15.0, 0.0, 0.0]);
    }

    #[test]
    fn native_distance_parameter_walk_uses_point_one_steps() {
        let spline = linear();
        assert!((spline.advance_parameter_by_distance(1.0, 5.0) - 1.5).abs() < 1.0e-5);
        assert!((spline.advance_parameter_by_distance(2.0, -5.0) - 1.5).abs() < 1.0e-5);
    }

    #[test]
    fn native_distance_parameter_walk_interpolates_before_endpoint_clamp() {
        let spline = linear();
        let forward = spline.advance_parameter_by_distance(1.95, 0.1);
        let reverse = spline.advance_parameter_by_distance(0.05, -0.1);
        assert!((forward - 1.97).abs() < 1.0e-5);
        assert!((reverse - 0.03).abs() < 1.0e-5);
    }

    #[test]
    fn native_adjacent_interval_search_halves_the_losing_endpoint_eight_times() {
        let spline = linear();
        let parameter = spline.nearest_parameter_in_interval([13.0, 0.0, 0.0], 1, 2);
        assert!((parameter - 1.296_875).abs() < 1.0e-5);
        assert_eq!(
            spline.nearest_parameter_in_interval([10.0, 0.0, 0.0], 1, 2),
            1.0
        );
    }
}
