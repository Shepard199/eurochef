use serde::Serialize;

pub const ROBOTS_HIT_SEGMENT_PARALLEL_EPSILON: f32 = 0.000_01;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RobotsHitShape {
    Sphere {
        center_xyz: [f32; 3],
        radius: f32,
    },
    Capsule {
        /// Native converted-shape +0x00..+0x08.
        start_xyz: [f32; 3],
        /// Native converted-shape +0x10..+0x18: finite segment delta, not endpoint.
        delta_xyz: [f32; 3],
        /// Native converted-shape +0x20.
        radius: f32,
    },
}

/// Local AnimDatum shape semantics after parsing but before the host applies
/// the selected bone/owner transform. Native `0x004D64C0` ultimately exposes
/// only these two geometry families to the hit dispatcher.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RobotsHitLocalShape {
    Sphere { radius: f32 },
    Capsule { half_segment: f32, radius: f32 },
}

/// Engine-neutral transform result supplied by the host. The host owns matrix,
/// skeleton and quaternion math; shared runtime owns how the resolved pose and
/// local AnimDatum scalars become the final Robots hit shape.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsResolvedHitDatumPose {
    pub center_xyz: [f32; 3],
    /// World-space direction of local +Y after datum orientation and owner/bone
    /// rotation. It may be non-normalized; shared matches native/GUI
    /// `normalize_or_zero` before constructing a capsule axis.
    pub axis_y_xyz: [f32; 3],
    /// Maximum absolute component of the combined owner/bone transform scale,
    /// matching native `0x00538963` scale handling.
    pub max_abs_scale: f32,
}

fn normalized_or_zero(v: [f32; 3]) -> [f32; 3] {
    let length_sq = len_sq(v);
    if length_sq.is_finite() && length_sq > f32::EPSILON {
        let inv = length_sq.sqrt().recip();
        [v[0] * inv, v[1] * inv, v[2] * inv]
    } else {
        [0.0; 3]
    }
}

/// Shared AnimDatum-to-world-shape construction used by the current preview
/// and intended UE5.8 host adapter. Matrix decomposition remains outside this
/// function, so no renderer/engine math dependency leaks into gameplay runtime.
pub fn robots_resolve_hit_shape(
    local_shape: RobotsHitLocalShape,
    pose: RobotsResolvedHitDatumPose,
) -> RobotsHitShape {
    let scale = pose.max_abs_scale.abs();
    match local_shape {
        RobotsHitLocalShape::Sphere { radius } => RobotsHitShape::Sphere {
            center_xyz: pose.center_xyz,
            radius: radius * scale,
        },
        RobotsHitLocalShape::Capsule {
            half_segment,
            radius,
        } => {
            let axis = normalized_or_zero(pose.axis_y_xyz);
            let half_segment = half_segment * scale;
            let start_xyz = [
                pose.center_xyz[0] - axis[0] * half_segment,
                pose.center_xyz[1] - axis[1] * half_segment,
                pose.center_xyz[2] - axis[2] * half_segment,
            ];
            let delta_xyz = [
                axis[0] * half_segment * 2.0,
                axis[1] * half_segment * 2.0,
                axis[2] * half_segment * 2.0,
            ];
            RobotsHitShape::Capsule {
                start_xyz,
                delta_xyz,
                radius: radius * scale,
            }
        }
    }
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn add_scaled(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + b[0] * t, a[1] + b[1] * t, a[2] + b[2] * t]
}

fn len_sq(v: [f32; 3]) -> f32 {
    dot(v, v)
}

/// Native point-to-finite-segment squared distance helper `0x005122BD`.
pub fn robots_point_segment_distance_squared(
    point: [f32; 3],
    segment_start: [f32; 3],
    segment_delta: [f32; 3],
) -> f32 {
    let from_start = sub(point, segment_start);
    let projection = dot(from_start, segment_delta);
    let delta_len_sq = len_sq(segment_delta);
    let t = if projection <= 0.0 || delta_len_sq <= 0.0 {
        0.0
    } else if projection < delta_len_sq {
        projection / delta_len_sq
    } else {
        1.0
    };
    len_sq(sub(point, add_scaled(segment_start, segment_delta, t)))
}

/// Finite-segment squared distance matching the native `0x0050F231` contract.
/// The original uses determinant threshold `0x005F426C ~= 1e-5` to select its
/// near-parallel branch. We preserve that threshold while exposing only the
/// distance result required by capsule/capsule intersection.
pub fn robots_segment_segment_distance_squared(
    a_start: [f32; 3],
    a_delta: [f32; 3],
    b_start: [f32; 3],
    b_delta: [f32; 3],
) -> f32 {
    let r = sub(a_start, b_start);
    let a = dot(a_delta, a_delta);
    let e = dot(b_delta, b_delta);

    if a <= 0.0 && e <= 0.0 {
        return len_sq(r);
    }
    if a <= 0.0 {
        let t = if e > 0.0 {
            (dot(b_delta, r) / e).clamp(0.0, 1.0)
        } else {
            0.0
        };
        return len_sq(sub(a_start, add_scaled(b_start, b_delta, t)));
    }
    if e <= 0.0 {
        let s = (-dot(a_delta, r) / a).clamp(0.0, 1.0);
        return len_sq(sub(add_scaled(a_start, a_delta, s), b_start));
    }

    let b = dot(a_delta, b_delta);
    let c = dot(a_delta, r);
    let f = dot(b_delta, r);
    let determinant = a * e - b * b;

    let mut s = if determinant.abs() >= ROBOTS_HIT_SEGMENT_PARALLEL_EPSILON {
        ((b * f - c * e) / determinant).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mut t = (b * s + f) / e;

    if t < 0.0 {
        t = 0.0;
        s = (-c / a).clamp(0.0, 1.0);
    } else if t > 1.0 {
        t = 1.0;
        s = ((b - c) / a).clamp(0.0, 1.0);
    }

    let a_point = add_scaled(a_start, a_delta, s);
    let b_point = add_scaled(b_start, b_delta, t);
    len_sq(sub(a_point, b_point))
}

/// Engine-neutral transcription of dispatcher `0x004D6C70` and helpers
/// `0x00556C65 / 0x00556CAE / 0x00556CF4`.
///
/// All three native pairings use inclusive contact: equality with the summed
/// radii still counts as a hit.
pub fn robots_hit_shapes_intersect(a: RobotsHitShape, b: RobotsHitShape) -> bool {
    match (a, b) {
        (
            RobotsHitShape::Sphere {
                center_xyz: a_center,
                radius: a_radius,
            },
            RobotsHitShape::Sphere {
                center_xyz: b_center,
                radius: b_radius,
            },
        ) => {
            let radius = a_radius + b_radius;
            len_sq(sub(a_center, b_center)) <= radius * radius
        }
        (
            RobotsHitShape::Sphere { center_xyz, radius },
            RobotsHitShape::Capsule {
                start_xyz,
                delta_xyz,
                radius: capsule_radius,
            },
        )
        | (
            RobotsHitShape::Capsule {
                start_xyz,
                delta_xyz,
                radius: capsule_radius,
            },
            RobotsHitShape::Sphere { center_xyz, radius },
        ) => {
            let combined = radius + capsule_radius;
            robots_point_segment_distance_squared(center_xyz, start_xyz, delta_xyz)
                <= combined * combined
        }
        (
            RobotsHitShape::Capsule {
                start_xyz: a_start,
                delta_xyz: a_delta,
                radius: a_radius,
            },
            RobotsHitShape::Capsule {
                start_xyz: b_start,
                delta_xyz: b_delta,
                radius: b_radius,
            },
        ) => {
            let combined = a_radius + b_radius;
            robots_segment_segment_distance_squared(a_start, a_delta, b_start, b_delta)
                <= combined * combined
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolved_pose_builds_shared_sphere_and_capsule_with_max_scale() {
        let sphere = robots_resolve_hit_shape(
            RobotsHitLocalShape::Sphere { radius: 0.5 },
            RobotsResolvedHitDatumPose {
                center_xyz: [12.0, 1.0, 0.0],
                axis_y_xyz: [0.0, 1.0, 0.0],
                max_abs_scale: 2.0,
            },
        );
        assert_eq!(
            sphere,
            RobotsHitShape::Sphere {
                center_xyz: [12.0, 1.0, 0.0],
                radius: 1.0,
            }
        );

        let capsule = robots_resolve_hit_shape(
            RobotsHitLocalShape::Capsule {
                half_segment: 0.45,
                radius: 0.40,
            },
            RobotsResolvedHitDatumPose {
                center_xyz: [10.0, 20.85, 30.0],
                axis_y_xyz: [0.0, 2.0, 0.0],
                max_abs_scale: 1.0,
            },
        );
        assert_eq!(
            capsule,
            RobotsHitShape::Capsule {
                start_xyz: [10.0, 20.4, 30.0],
                delta_xyz: [0.0, 0.9, 0.0],
                radius: 0.40,
            }
        );
    }

    #[test]
    fn sphere_sphere_contact_is_inclusive_like_native_00556c65() {
        let a = RobotsHitShape::Sphere {
            center_xyz: [0.0, 0.0, 0.0],
            radius: 1.0,
        };
        let b = RobotsHitShape::Sphere {
            center_xyz: [2.0, 0.0, 0.0],
            radius: 1.0,
        };
        assert!(robots_hit_shapes_intersect(a, b));
        let b = RobotsHitShape::Sphere {
            center_xyz: [2.001, 0.0, 0.0],
            radius: 1.0,
        };
        assert!(!robots_hit_shapes_intersect(a, b));
    }

    #[test]
    fn point_segment_distance_clamps_to_finite_capsule_axis() {
        assert_eq!(
            robots_point_segment_distance_squared(
                [3.0, 1.0, 0.0],
                [0.0, 0.0, 0.0],
                [2.0, 0.0, 0.0],
            ),
            2.0,
        );
        assert_eq!(
            robots_point_segment_distance_squared(
                [-1.0, 0.0, 0.0],
                [0.0, 0.0, 0.0],
                [2.0, 0.0, 0.0],
            ),
            1.0,
        );
    }

    #[test]
    fn sphere_capsule_uses_point_segment_distance_and_sum_of_radii() {
        let sphere = RobotsHitShape::Sphere {
            center_xyz: [1.0, 2.0, 0.0],
            radius: 1.0,
        };
        let capsule = RobotsHitShape::Capsule {
            start_xyz: [0.0, 0.0, 0.0],
            delta_xyz: [2.0, 0.0, 0.0],
            radius: 1.0,
        };
        assert!(robots_hit_shapes_intersect(sphere, capsule));

        let sphere = RobotsHitShape::Sphere {
            center_xyz: [1.0, 2.01, 0.0],
            radius: 1.0,
        };
        assert!(!robots_hit_shapes_intersect(sphere, capsule));
    }

    #[test]
    fn capsule_capsule_crossing_segments_hit_even_with_zero_radii() {
        let a = RobotsHitShape::Capsule {
            start_xyz: [-1.0, 0.0, 0.0],
            delta_xyz: [2.0, 0.0, 0.0],
            radius: 0.0,
        };
        let b = RobotsHitShape::Capsule {
            start_xyz: [0.0, -1.0, 0.0],
            delta_xyz: [0.0, 2.0, 0.0],
            radius: 0.0,
        };
        assert!(robots_hit_shapes_intersect(a, b));
    }

    #[test]
    fn parallel_capsules_use_native_near_parallel_threshold_path() {
        let a = RobotsHitShape::Capsule {
            start_xyz: [0.0, 0.0, 0.0],
            delta_xyz: [10.0, 0.0, 0.0],
            radius: 0.4,
        };
        let b = RobotsHitShape::Capsule {
            start_xyz: [0.0, 1.0, 0.0],
            delta_xyz: [10.0, 0.000_000_01, 0.0],
            radius: 0.6,
        };
        assert!(robots_hit_shapes_intersect(a, b));
    }
}
