use serde::Serialize;

pub const ROBOTS_HIT_BOUNDS_EPSILON: f32 = 0.001;
pub const ROBOTS_HIT_BOUNDS_FALLBACK_SQUARED_EXTENT: f32 = 2.0;

/// Query-side coarse bounds prepared by native `0x00425630` and consumed by
/// `0x004D65F0`. The fourth component is preserved because native copies a full
/// four-float block at query `+0x64..+0x70`, although the coarse reject itself
/// compares only XYZ.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsHitQueryCoarseBounds {
    pub center_xyzw: [f32; 4],
    /// Native query `+0x78`. `0x004D65F0` consumes it directly as a squared
    /// extent contribution; its producer-specific geometric meaning stays in
    /// the host adapter.
    pub squared_extent_contribution: f32,
}

/// Candidate-side values read by native `0x004D65F0` from an XItem.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsHitCandidateCoarseBounds {
    /// Native `XItem+0xD0..+0xDC`.
    pub base_center_xyzw: [f32; 4],
    /// Native `XItem+0x244..+0x250`. Applied only when `dynamic_extent > 0`.
    pub dynamic_center_offset_xyzw: [f32; 4],
    /// Native `XItem+0x254`. When positive, native contributes its square to
    /// the coarse-radius budget.
    pub dynamic_extent: f32,
}

impl RobotsHitCandidateCoarseBounds {
    pub fn effective_center_xyzw(self) -> [f32; 4] {
        if self.dynamic_extent > 0.0 {
            [
                self.base_center_xyzw[0] + self.dynamic_center_offset_xyzw[0],
                self.base_center_xyzw[1] + self.dynamic_center_offset_xyzw[1],
                self.base_center_xyzw[2] + self.dynamic_center_offset_xyzw[2],
                self.base_center_xyzw[3] + self.dynamic_center_offset_xyzw[3],
            ]
        } else {
            self.base_center_xyzw
        }
    }

    pub fn squared_extent_contribution(self) -> f32 {
        if self.dynamic_extent > 0.0 {
            self.dynamic_extent * self.dynamic_extent
        } else {
            0.0
        }
    }
}

fn normalized_squared_extent(value: f32) -> f32 {
    if value < ROBOTS_HIT_BOUNDS_EPSILON {
        ROBOTS_HIT_BOUNDS_FALLBACK_SQUARED_EXTENT
    } else {
        value
    }
}

/// Exact boolean contract of native coarse reject `0x004D65F0`.
///
/// The helper adds the candidate/query squared-extent contributions after each
/// side applies the native `<0.001 -> 2.0` fallback. A candidate is rejected
/// only when an axis squared delta or the final XYZ squared distance is strictly
/// greater than that combined value; equality remains admissible.
///
/// The axis checks are mathematically redundant with the final distance test,
/// but are kept explicit because they are part of the original cheap-reject
/// order and document what an Unreal adapter may short-circuit before invoking
/// the real narrowphase.
pub fn robots_hit_query_coarse_bounds_miss(
    query: RobotsHitQueryCoarseBounds,
    candidate: RobotsHitCandidateCoarseBounds,
) -> bool {
    let candidate_center = candidate.effective_center_xyzw();
    let combined_squared_extent =
        normalized_squared_extent(candidate.squared_extent_contribution())
            + normalized_squared_extent(query.squared_extent_contribution);

    let dx = query.center_xyzw[0] - candidate_center[0];
    let dy = query.center_xyzw[1] - candidate_center[1];
    let dz = query.center_xyzw[2] - candidate_center[2];
    let dx2 = dx * dx;
    let dy2 = dy * dy;
    let dz2 = dz * dz;

    if dx2 > combined_squared_extent
        || dy2 > combined_squared_extent
        || dz2 > combined_squared_extent
    {
        return true;
    }

    dx2 + dy2 + dz2 > combined_squared_extent
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(center: [f32; 4], squared_extent: f32) -> RobotsHitQueryCoarseBounds {
        RobotsHitQueryCoarseBounds {
            center_xyzw: center,
            squared_extent_contribution: squared_extent,
        }
    }

    fn candidate(
        center: [f32; 4],
        offset: [f32; 4],
        extent: f32,
    ) -> RobotsHitCandidateCoarseBounds {
        RobotsHitCandidateCoarseBounds {
            base_center_xyzw: center,
            dynamic_center_offset_xyzw: offset,
            dynamic_extent: extent,
        }
    }

    #[test]
    fn zero_contributions_use_native_two_point_zero_fallback_per_side() {
        let q = query([0.0, 0.0, 0.0, 1.0], 0.0);
        let c = candidate([2.0, 0.0, 0.0, 1.0], [0.0; 4], 0.0);
        // Native threshold = 2.0 + 2.0 = 4.0, and equality is admitted.
        assert!(!robots_hit_query_coarse_bounds_miss(q, c));

        let c = candidate([2.001, 0.0, 0.0, 1.0], [0.0; 4], 0.0);
        assert!(robots_hit_query_coarse_bounds_miss(q, c));
    }

    #[test]
    fn positive_dynamic_extent_offsets_center_and_contributes_extent_squared() {
        let q = query([0.0, 0.0, 0.0, 1.0], 1.0);
        let c = candidate([10.0, 0.0, 0.0, 1.0], [-8.0, 0.0, 0.0, 0.0], 2.0);
        assert_eq!(c.effective_center_xyzw(), [2.0, 0.0, 0.0, 1.0]);
        assert_eq!(c.squared_extent_contribution(), 4.0);
        assert!(!robots_hit_query_coarse_bounds_miss(q, c));
    }

    #[test]
    fn non_positive_extent_does_not_apply_dynamic_center_offset() {
        let c = candidate([1.0, 2.0, 3.0, 1.0], [100.0, 100.0, 100.0, 0.0], 0.0);
        assert_eq!(c.effective_center_xyzw(), [1.0, 2.0, 3.0, 1.0]);
        assert_eq!(c.squared_extent_contribution(), 0.0);
    }

    #[test]
    fn exact_combined_distance_boundary_is_not_a_coarse_miss() {
        let q = query([0.0, 0.0, 0.0, 1.0], 4.0);
        let c = candidate([0.0, 3.0, 0.0, 1.0], [0.0; 4], 5.0_f32.sqrt());
        // 4 + 5 = 9 and distance^2 is exactly 9.
        assert!(!robots_hit_query_coarse_bounds_miss(q, c));
    }

    #[test]
    fn fourth_component_is_preserved_but_not_used_by_native_xyz_reject() {
        let q = query([0.0, 0.0, 0.0, 999.0], 1.0);
        let c = candidate([0.0, 0.0, 0.0, -999.0], [0.0; 4], 1.0);
        assert!(!robots_hit_query_coarse_bounds_miss(q, c));
    }
}
