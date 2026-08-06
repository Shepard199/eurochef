use glam::Vec3;

/// Exact Robots.exe MapZone selector reconstructed from:
///
/// - `0x004E9E71`: skips zone 0, returns the first matching zone in serialized
///   order, and falls back to zone 0 when no later zone matches.
/// - `0x0051CE55`: inclusive AABB containment with tolerance `0.0` at this call
///   site (`min <= point <= max` on all three axes).
pub fn robots_map_zone_index_by_bounds(
    zone_count: usize,
    point: Vec3,
    mut bounds_at: impl FnMut(usize) -> (Vec3, Vec3),
) -> Option<usize> {
    if zone_count == 0 {
        return None;
    }

    (1..zone_count)
        .find(|index| {
            let (bounds_min, bounds_max) = bounds_at(*index);
            robots_map_zone_contains(bounds_min, bounds_max, point)
        })
        .or(Some(0))
}

pub fn robots_map_zone_contains(bounds_min: Vec3, bounds_max: Vec3, point: Vec3) -> bool {
    point.cmpge(bounds_min).all() && point.cmple(bounds_max).all()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_selector_uses_first_serialized_match_not_smallest_volume() {
        let bounds = [
            (Vec3::splat(-100.0), Vec3::splat(100.0)),
            (Vec3::splat(-10.0), Vec3::splat(10.0)),
            (Vec3::splat(-1.0), Vec3::splat(1.0)),
        ];

        assert_eq!(
            robots_map_zone_index_by_bounds(bounds.len(), Vec3::ZERO, |index| bounds[index]),
            Some(1)
        );
    }

    #[test]
    fn native_selector_uses_zone_zero_as_unconditional_fallback() {
        let bounds = [
            (Vec3::splat(-1.0), Vec3::splat(1.0)),
            (Vec3::splat(10.0), Vec3::splat(12.0)),
        ];

        assert_eq!(
            robots_map_zone_index_by_bounds(bounds.len(), Vec3::splat(50.0), |index| bounds[index]),
            Some(0)
        );
        assert_eq!(
            robots_map_zone_index_by_bounds(0, Vec3::ZERO, |_| unreachable!()),
            None
        );
    }

    #[test]
    fn native_selector_does_not_test_zone_zero_before_later_zones() {
        let bounds = [
            (Vec3::splat(-10.0), Vec3::splat(10.0)),
            (Vec3::splat(-1.0), Vec3::splat(1.0)),
        ];

        assert_eq!(
            robots_map_zone_index_by_bounds(bounds.len(), Vec3::ZERO, |index| bounds[index]),
            Some(1)
        );
    }

    #[test]
    fn native_aabb_test_is_inclusive_on_both_edges() {
        let bounds_min = Vec3::new(-1.0, -2.0, -3.0);
        let bounds_max = Vec3::new(4.0, 5.0, 6.0);

        assert!(robots_map_zone_contains(bounds_min, bounds_max, bounds_min));
        assert!(robots_map_zone_contains(bounds_min, bounds_max, bounds_max));
        assert!(!robots_map_zone_contains(
            bounds_min,
            bounds_max,
            Vec3::new(4.0001, 5.0, 6.0)
        ));
    }
}
