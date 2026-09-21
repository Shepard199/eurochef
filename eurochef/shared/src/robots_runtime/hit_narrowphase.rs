use serde::Serialize;

use super::{hit_bounds::ROBOTS_HIT_BOUNDS_EPSILON, hit_shapes::RobotsHitShape};

pub const ROBOTS_ANIM_DATUM_HIT_AREA_UID: u32 = 0x1000_0010;
pub const ROBOTS_HIT_QUERY_ITERATE_SOURCE_SELECTOR_FLAG: u32 = 0x0000_0400;
pub const ROBOTS_HIT_QUERY_SKIP_RELATIVE_SHAPE_REFRESH_FLAG: u32 = 0x0000_0800;
pub const ROBOTS_HIT_SEGMENT_MIDPOINT_FACTOR: f32 = 0.5;

/// Native `0x00425EF0` always enumerates candidate `HT_AnimDatum_HitArea`
/// records and intersects them against one of these two source representations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsHitAreaNarrowphaseMode {
    /// `flags & 0x400 == 0`: use the query shape already prepared at query +0x04.
    PreparedQueryShape,
    /// `flags & 0x400 != 0`: enumerate source shapes using query selector +0x84.
    SourceSelectorShapes {
        selector: u32,
        /// Native calls `0x004258E0` for every source shape unless flag 0x800
        /// says the relative query shape is already stable/prepared.
        refresh_relative_query_shape: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsHitAreaNarrowphasePlan {
    pub candidate_datum_uid: u32,
    pub mode: RobotsHitAreaNarrowphaseMode,
}

impl RobotsHitAreaNarrowphasePlan {
    pub fn from_query(flags: u32, selector: u32) -> Self {
        let mode = if flags & ROBOTS_HIT_QUERY_ITERATE_SOURCE_SELECTOR_FLAG == 0 {
            RobotsHitAreaNarrowphaseMode::PreparedQueryShape
        } else {
            RobotsHitAreaNarrowphaseMode::SourceSelectorShapes {
                selector,
                refresh_relative_query_shape: flags
                    & ROBOTS_HIT_QUERY_SKIP_RELATIVE_SHAPE_REFRESH_FLAG
                    == 0,
            }
        };
        Self {
            candidate_datum_uid: ROBOTS_ANIM_DATUM_HIT_AREA_UID,
            mode,
        }
    }
}

/// Engine-neutral output of native segment builder `0x004264C0`.
/// Geometry backends can consume the same start/delta representation or map it
/// directly to a UE sweep/shape query without depending on the original memory
/// layout.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsHitSegmentPlan {
    pub start_xyzw: [f32; 4],
    pub delta_xyzw: [f32; 4],
    /// Native query +0x24, passed through unchanged by `0x004264C0`.
    pub query_scalar: f32,
    /// Native query +0x64..+0x70 coarse center.
    pub coarse_center_xyzw: [f32; 4],
    /// Native query +0x78.
    pub coarse_squared_extent_contribution: f32,
}

impl RobotsHitSegmentPlan {
    pub fn from_endpoints(
        start_xyzw: [f32; 4],
        end_xyzw: [f32; 4],
        query_scalar: f32,
        start_extent: f32,
        end_extent: f32,
    ) -> Self {
        let delta_xyzw = [
            end_xyzw[0] - start_xyzw[0],
            end_xyzw[1] - start_xyzw[1],
            end_xyzw[2] - start_xyzw[2],
            end_xyzw[3] - start_xyzw[3],
        ];
        let coarse_center_xyzw = [
            start_xyzw[0] + delta_xyzw[0] * ROBOTS_HIT_SEGMENT_MIDPOINT_FACTOR,
            start_xyzw[1] + delta_xyzw[1] * ROBOTS_HIT_SEGMENT_MIDPOINT_FACTOR,
            start_xyzw[2] + delta_xyzw[2] * ROBOTS_HIT_SEGMENT_MIDPOINT_FACTOR,
            start_xyzw[3] + delta_xyzw[3] * ROBOTS_HIT_SEGMENT_MIDPOINT_FACTOR,
        ];
        let delta_xyz_len_sq = delta_xyzw[0] * delta_xyzw[0]
            + delta_xyzw[1] * delta_xyzw[1]
            + delta_xyzw[2] * delta_xyzw[2];
        let coarse_squared_extent_contribution =
            delta_xyz_len_sq * ROBOTS_HIT_SEGMENT_MIDPOINT_FACTOR + start_extent * end_extent;

        Self {
            start_xyzw,
            delta_xyzw,
            query_scalar,
            coarse_center_xyzw,
            coarse_squared_extent_contribution,
        }
    }
}

/// Native byte query+0x94 used by alternate path `0x00426170`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsHitSampleSweepMode {
    /// Mode 0: each sample directly replaces query +0x04..+0x10; query +0x14
    /// receives +0x7C and the query is tested immediately.
    SamplePoint = 0,
    /// Mode 1: the first sample seeds history; later samples build consecutive
    /// segments through `0x004264C0`.
    ConsecutiveSegments = 1,
    /// Mode 2: every sample builds a segment from source object +0x140..+0x14C.
    SourceToSampleSegments = 2,
}

impl RobotsHitSampleSweepMode {
    pub fn from_raw(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::SamplePoint),
            1 => Some(Self::ConsecutiveSegments),
            2 => Some(Self::SourceToSampleSegments),
            _ => None,
        }
    }
}

pub const ROBOTS_HIT_SAMPLE_MARK_CONSUMED_ON_HIT_FLAG: u8 = 0x01;
pub const ROBOTS_HIT_SAMPLE_CONTINUE_AFTER_HIT_FLAG: u8 = 0x02;
pub const ROBOTS_HIT_SAMPLE_RADIAL_CULL_FLAG: u8 = 0x04;
pub const ROBOTS_HIT_SAMPLE_CONSUMED_VALUE: f32 = 100.0;

/// Stable host-facing configuration for native alternate narrowphase
/// `0x00426170`. The original list allocator/container stays outside shared
/// runtime; UE supplies the sample sequence while this plan preserves native
/// mode/flag semantics.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsHitSampleSweepPlan {
    pub mode: RobotsHitSampleSweepMode,
    /// Native query +0x7C, forwarded into direct point/segment preparation.
    pub query_scalar: f32,
    /// Native query +0x80. Bit2 of +0x95 enables squared-distance culling
    /// against this radius when it is greater than epsilon.
    pub radial_cull_radius: f32,
    /// Native query +0x95.
    pub flags: u8,
}

impl RobotsHitSampleSweepPlan {
    pub fn mark_consumed_on_hit(self) -> bool {
        self.flags & ROBOTS_HIT_SAMPLE_MARK_CONSUMED_ON_HIT_FLAG != 0
    }

    pub fn continue_after_hit(self) -> bool {
        self.flags & ROBOTS_HIT_SAMPLE_CONTINUE_AFTER_HIT_FLAG != 0
    }

    pub fn radial_cull_enabled(self) -> bool {
        self.flags & ROBOTS_HIT_SAMPLE_RADIAL_CULL_FLAG != 0
            && self.radial_cull_radius > ROBOTS_HIT_BOUNDS_EPSILON
    }

    /// Exact early radial sample rejection from `0x00426170`. Native marks a
    /// rejected sample's +0x04 with 100.0 before continuing.
    pub fn radial_cull_miss(self, source_xyz: [f32; 3], sample_xyz: [f32; 3]) -> bool {
        if !self.radial_cull_enabled() {
            return false;
        }
        let dx = sample_xyz[0] - source_xyz[0];
        let dy = sample_xyz[1] - source_xyz[1];
        let dz = sample_xyz[2] - source_xyz[2];
        let distance_sq = dx * dx + dy * dy + dz * dz;
        distance_sq > self.radial_cull_radius * self.radial_cull_radius
    }
}

/// Host-owned sample record consumed by native alternate query `0x00426170`.
/// The native list element stores its point at +0x0C..+0x18 and writes exactly
/// `100.0f` to +0x04 when radial culling or flag bit0 consumes that sample.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsHitSweepSample {
    pub point_xyzw: [f32; 4],
    pub marker: f32,
}

/// Execute the per-candidate geometry loop of native `0x00426170` without
/// importing the original list allocator into shared runtime. `source_xyzw`
/// corresponds to source XItem +0x140..+0x14C; native returns false immediately
/// when that source is absent. The callback is the existing candidate HitArea
/// narrowphase (`0x00425EF0`) in engine-neutral form.
pub fn execute_hit_sample_sweep<F>(
    plan: RobotsHitSampleSweepPlan,
    source_xyzw: Option<[f32; 4]>,
    samples: &mut [RobotsHitSweepSample],
    mut test_shape: F,
) -> bool
where
    F: FnMut(RobotsHitShape) -> bool,
{
    let Some(source_xyzw) = source_xyzw else {
        return false;
    };
    let source_xyz = [source_xyzw[0], source_xyzw[1], source_xyzw[2]];
    let mut previous_point = [0.0; 4];

    for (sample_index, sample) in samples.iter_mut().enumerate() {
        let sample_xyz = [
            sample.point_xyzw[0],
            sample.point_xyzw[1],
            sample.point_xyzw[2],
        ];
        if plan.radial_cull_miss(source_xyz, sample_xyz) {
            sample.marker = ROBOTS_HIT_SAMPLE_CONSUMED_VALUE;
            continue;
        }

        let query_shape = match plan.mode {
            RobotsHitSampleSweepMode::SamplePoint => Some(RobotsHitShape::Sphere {
                center_xyz: sample_xyz,
                radius: plan.query_scalar,
            }),
            RobotsHitSampleSweepMode::ConsecutiveSegments => {
                let start = previous_point;
                previous_point = sample.point_xyzw;
                (sample_index > 0).then_some(RobotsHitShape::Capsule {
                    start_xyz: [start[0], start[1], start[2]],
                    delta_xyz: [
                        sample_xyz[0] - start[0],
                        sample_xyz[1] - start[1],
                        sample_xyz[2] - start[2],
                    ],
                    radius: plan.query_scalar,
                })
            }
            RobotsHitSampleSweepMode::SourceToSampleSegments => Some(RobotsHitShape::Capsule {
                start_xyz: source_xyz,
                delta_xyz: [
                    sample_xyz[0] - source_xyz[0],
                    sample_xyz[1] - source_xyz[1],
                    sample_xyz[2] - source_xyz[2],
                ],
                radius: plan.query_scalar,
            }),
        };

        let Some(query_shape) = query_shape else {
            continue;
        };
        if !test_shape(query_shape) {
            continue;
        }
        if plan.mark_consumed_on_hit() {
            sample.marker = ROBOTS_HIT_SAMPLE_CONSUMED_VALUE;
        }
        if !plan.continue_after_hit() {
            return true;
        }
    }

    // Native does not accumulate a successful result when bit1 asks it to
    // continue scanning; only a hit that takes the immediate-return branch
    // makes `0x00426170` return 1.
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_area_plan_switches_only_on_native_0x400_and_0x800_bits() {
        let prepared = RobotsHitAreaNarrowphasePlan::from_query(0, 0x1000_0009);
        assert_eq!(prepared.candidate_datum_uid, ROBOTS_ANIM_DATUM_HIT_AREA_UID);
        assert_eq!(
            prepared.mode,
            RobotsHitAreaNarrowphaseMode::PreparedQueryShape
        );

        let iterating = RobotsHitAreaNarrowphasePlan::from_query(
            ROBOTS_HIT_QUERY_ITERATE_SOURCE_SELECTOR_FLAG,
            0x1000_0009,
        );
        assert_eq!(
            iterating.mode,
            RobotsHitAreaNarrowphaseMode::SourceSelectorShapes {
                selector: 0x1000_0009,
                refresh_relative_query_shape: true,
            }
        );

        let stable = RobotsHitAreaNarrowphasePlan::from_query(
            ROBOTS_HIT_QUERY_ITERATE_SOURCE_SELECTOR_FLAG
                | ROBOTS_HIT_QUERY_SKIP_RELATIVE_SHAPE_REFRESH_FLAG,
            0x1000_0009,
        );
        assert_eq!(
            stable.mode,
            RobotsHitAreaNarrowphaseMode::SourceSelectorShapes {
                selector: 0x1000_0009,
                refresh_relative_query_shape: false,
            }
        );
    }

    #[test]
    fn segment_builder_matches_native_halfway_center_and_squared_contribution() {
        let plan = RobotsHitSegmentPlan::from_endpoints(
            [0.0, 0.0, 0.0, 1.0],
            [4.0, 2.0, 0.0, 1.0],
            7.0,
            2.0,
            3.0,
        );
        assert_eq!(plan.delta_xyzw, [4.0, 2.0, 0.0, 0.0]);
        assert_eq!(plan.coarse_center_xyzw, [2.0, 1.0, 0.0, 1.0]);
        // 0.5 * (4^2 + 2^2) + 2*3 = 16.
        assert_eq!(plan.coarse_squared_extent_contribution, 16.0);
        assert_eq!(plan.query_scalar, 7.0);
    }

    #[test]
    fn sample_sweep_modes_are_fail_closed_outside_native_zero_to_two() {
        assert_eq!(
            RobotsHitSampleSweepMode::from_raw(0),
            Some(RobotsHitSampleSweepMode::SamplePoint)
        );
        assert_eq!(
            RobotsHitSampleSweepMode::from_raw(1),
            Some(RobotsHitSampleSweepMode::ConsecutiveSegments)
        );
        assert_eq!(
            RobotsHitSampleSweepMode::from_raw(2),
            Some(RobotsHitSampleSweepMode::SourceToSampleSegments)
        );
        assert_eq!(RobotsHitSampleSweepMode::from_raw(3), None);
        assert_eq!(RobotsHitSampleSweepMode::from_raw(0xFF), None);
    }

    #[test]
    fn radial_sample_cull_uses_strict_squared_distance_and_native_epsilon_gate() {
        let plan = RobotsHitSampleSweepPlan {
            mode: RobotsHitSampleSweepMode::SamplePoint,
            query_scalar: 0.0,
            radial_cull_radius: 5.0,
            flags: ROBOTS_HIT_SAMPLE_RADIAL_CULL_FLAG,
        };
        assert!(!plan.radial_cull_miss([0.0; 3], [3.0, 4.0, 0.0]));
        assert!(plan.radial_cull_miss([0.0; 3], [3.01, 4.0, 0.0]));

        let disabled = RobotsHitSampleSweepPlan {
            radial_cull_radius: ROBOTS_HIT_BOUNDS_EPSILON,
            ..plan
        };
        assert!(!disabled.radial_cull_miss([0.0; 3], [999.0, 0.0, 0.0]));
    }

    #[test]
    fn sample_flags_expose_native_mark_and_continue_semantics_independently() {
        let mark = RobotsHitSampleSweepPlan {
            mode: RobotsHitSampleSweepMode::ConsecutiveSegments,
            query_scalar: 1.0,
            radial_cull_radius: 0.0,
            flags: ROBOTS_HIT_SAMPLE_MARK_CONSUMED_ON_HIT_FLAG,
        };
        assert!(mark.mark_consumed_on_hit());
        assert!(!mark.continue_after_hit());
        assert_eq!(ROBOTS_HIT_SAMPLE_CONSUMED_VALUE, 100.0);

        let keep_scanning = RobotsHitSampleSweepPlan {
            flags: ROBOTS_HIT_SAMPLE_CONTINUE_AFTER_HIT_FLAG,
            ..mark
        };
        assert!(!keep_scanning.mark_consumed_on_hit());
        assert!(keep_scanning.continue_after_hit());
    }

    #[test]
    fn sample_point_sweep_marks_confirmed_sample_and_returns_immediately() {
        let plan = RobotsHitSampleSweepPlan {
            mode: RobotsHitSampleSweepMode::SamplePoint,
            query_scalar: 0.75,
            radial_cull_radius: 0.0,
            flags: ROBOTS_HIT_SAMPLE_MARK_CONSUMED_ON_HIT_FLAG,
        };
        let mut samples = [
            RobotsHitSweepSample {
                point_xyzw: [1.0, 2.0, 3.0, 1.0],
                marker: 7.0,
            },
            RobotsHitSweepSample {
                point_xyzw: [9.0, 9.0, 9.0, 1.0],
                marker: 8.0,
            },
        ];
        let mut tested = Vec::new();
        let hit =
            execute_hit_sample_sweep(plan, Some([0.0, 0.0, 0.0, 1.0]), &mut samples, |shape| {
                tested.push(shape);
                true
            });
        assert!(hit);
        assert_eq!(
            tested,
            vec![RobotsHitShape::Sphere {
                center_xyz: [1.0, 2.0, 3.0],
                radius: 0.75,
            }]
        );
        assert_eq!(samples[0].marker, ROBOTS_HIT_SAMPLE_CONSUMED_VALUE);
        assert_eq!(samples[1].marker, 8.0);
    }

    #[test]
    fn continue_after_hit_preserves_native_false_final_return_and_sample_side_effects() {
        let plan = RobotsHitSampleSweepPlan {
            mode: RobotsHitSampleSweepMode::SamplePoint,
            query_scalar: 1.0,
            radial_cull_radius: 0.0,
            flags: ROBOTS_HIT_SAMPLE_MARK_CONSUMED_ON_HIT_FLAG
                | ROBOTS_HIT_SAMPLE_CONTINUE_AFTER_HIT_FLAG,
        };
        let mut samples = [
            RobotsHitSweepSample {
                point_xyzw: [1.0, 0.0, 0.0, 1.0],
                marker: 0.0,
            },
            RobotsHitSweepSample {
                point_xyzw: [2.0, 0.0, 0.0, 1.0],
                marker: 0.0,
            },
        ];
        let mut calls = 0;
        let hit = execute_hit_sample_sweep(plan, Some([0.0, 0.0, 0.0, 1.0]), &mut samples, |_| {
            calls += 1;
            true
        });
        assert!(!hit);
        assert_eq!(calls, 2);
        assert!(samples
            .iter()
            .all(|sample| sample.marker == ROBOTS_HIT_SAMPLE_CONSUMED_VALUE));
    }

    #[test]
    fn consecutive_sweep_skips_radially_culled_samples_without_advancing_history() {
        let plan = RobotsHitSampleSweepPlan {
            mode: RobotsHitSampleSweepMode::ConsecutiveSegments,
            query_scalar: 0.25,
            radial_cull_radius: 5.0,
            flags: ROBOTS_HIT_SAMPLE_RADIAL_CULL_FLAG | ROBOTS_HIT_SAMPLE_CONTINUE_AFTER_HIT_FLAG,
        };
        let mut samples = [
            RobotsHitSweepSample {
                point_xyzw: [1.0, 0.0, 0.0, 1.0],
                marker: 0.0,
            },
            RobotsHitSweepSample {
                point_xyzw: [10.0, 0.0, 0.0, 1.0],
                marker: 0.0,
            },
            RobotsHitSweepSample {
                point_xyzw: [3.0, 0.0, 0.0, 1.0],
                marker: 0.0,
            },
        ];
        let mut tested = Vec::new();
        let hit =
            execute_hit_sample_sweep(plan, Some([0.0, 0.0, 0.0, 1.0]), &mut samples, |shape| {
                tested.push(shape);
                false
            });
        assert!(!hit);
        assert_eq!(samples[1].marker, ROBOTS_HIT_SAMPLE_CONSUMED_VALUE);
        assert_eq!(
            tested,
            vec![RobotsHitShape::Capsule {
                start_xyz: [1.0, 0.0, 0.0],
                delta_xyz: [2.0, 0.0, 0.0],
                radius: 0.25,
            }]
        );
    }

    #[test]
    fn source_to_sample_sweep_uses_source_endpoint_and_fails_closed_without_source() {
        let plan = RobotsHitSampleSweepPlan {
            mode: RobotsHitSampleSweepMode::SourceToSampleSegments,
            query_scalar: 0.5,
            radial_cull_radius: 0.0,
            flags: 0,
        };
        let original = RobotsHitSweepSample {
            point_xyzw: [4.0, 5.0, 6.0, 1.0],
            marker: 3.0,
        };
        let mut absent_samples = [original];
        let mut absent_calls = 0;
        assert!(!execute_hit_sample_sweep(
            plan,
            None,
            &mut absent_samples,
            |_| {
                absent_calls += 1;
                true
            },
        ));
        assert_eq!(absent_calls, 0);
        assert_eq!(absent_samples[0], original);

        let mut samples = [original];
        let mut tested = Vec::new();
        assert!(execute_hit_sample_sweep(
            plan,
            Some([1.0, 2.0, 3.0, 1.0]),
            &mut samples,
            |shape| {
                tested.push(shape);
                true
            },
        ));
        assert_eq!(
            tested,
            vec![RobotsHitShape::Capsule {
                start_xyz: [1.0, 2.0, 3.0],
                delta_xyz: [3.0, 3.0, 3.0],
                radius: 0.5,
            }]
        );
        assert_eq!(samples[0].marker, 3.0);
    }
}
