use eurochef_edb::map::EXGeoBspNode;
use glam::Vec3;

fn robots_map_zone_index_from_bsp(
    zone_count: usize,
    point: Vec3,
    mut node_at: impl FnMut(usize) -> Option<([f32; 4], [i16; 2])>,
) -> Option<usize> {
    if zone_count == 0 {
        return None;
    }

    let mut node_index = 0usize;
    for _ in 0..=0x1_0000 {
        let (plane, children) = node_at(node_index)?;
        let distance = point.x * plane[0] + point.y * plane[1] + point.z * plane[2] + plane[3];
        let child = children[usize::from(distance < 0.0)];
        if child > 0 {
            node_index = child as usize;
            continue;
        }
        let zone_index = usize::try_from(-(child as i32)).ok()?;
        return (zone_index < zone_count).then_some(zone_index);
    }
    None
}

/// Exact Robots.exe MapZone selector reconstructed from `0x0055578F`.
/// The game traverses `EXGeoMap.bsp_tree`; positive children are BSP node
/// indices and non-positive children encode the final zone as `-child`.
pub fn robots_map_zone_index_by_bsp(
    bsp_nodes: &[EXGeoBspNode],
    zone_count: usize,
    point: Vec3,
) -> Option<usize> {
    robots_map_zone_index_from_bsp(zone_count, point, |index| {
        bsp_nodes.get(index).map(|node| (node.pos, node.nodes))
    })
}

/// AABB helper retained for diagnostics and legacy corpus checks only.
/// `0x004E9E71` is a loaded map/submap selector, not a MapZone selector.
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
    fn native_bsp_selector_follows_plane_side_and_leaf_encoding() {
        let nodes = [([1.0, 0.0, 0.0, 0.0], [-2, -1])];
        assert_eq!(
            robots_map_zone_index_from_bsp(3, Vec3::X, |index| nodes.get(index).copied()),
            Some(2)
        );
        assert_eq!(
            robots_map_zone_index_from_bsp(3, -Vec3::X, |index| nodes.get(index).copied()),
            Some(1)
        );
    }

    #[test]
    fn native_bsp_selector_traverses_positive_child_and_leaf_zero() {
        let nodes = [
            ([1.0, 0.0, 0.0, 0.0], [1, 0]),
            ([0.0, 0.0, 1.0, 0.0], [-3, -2]),
        ];
        assert_eq!(
            robots_map_zone_index_from_bsp(4, Vec3::new(1.0, 0.0, 1.0), |index| {
                nodes.get(index).copied()
            }),
            Some(3)
        );
        assert_eq!(
            robots_map_zone_index_from_bsp(4, -Vec3::X, |index| nodes.get(index).copied()),
            Some(0)
        );
    }

    #[test]
    fn native_bsp_selector_rejects_invalid_node_or_leaf() {
        let bad_node = [([1.0, 0.0, 0.0, 0.0], [7, -1])];
        assert_eq!(
            robots_map_zone_index_from_bsp(2, Vec3::X, |index| bad_node.get(index).copied()),
            None
        );
        let bad_leaf = [([1.0, 0.0, 0.0, 0.0], [-3, -1])];
        assert_eq!(
            robots_map_zone_index_from_bsp(2, Vec3::X, |index| bad_leaf.get(index).copied()),
            None
        );
    }

    #[test]
    fn diagnostic_bounds_selector_uses_first_serialized_match_not_smallest_volume() {
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
