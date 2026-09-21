use serde::Serialize;

pub const ROBOTS_MONSTER_NAV_RAY_START_Y_OFFSET: f32 = 1.0;
pub const ROBOTS_MONSTER_NAV_RAY_DIRECTION_Y: f32 = -1.0;
pub const ROBOTS_MONSTER_NAV_RAY_MAX_T: f32 = 2.0;
pub const ROBOTS_MONSTER_NAV_DETERMINANT_EPSILON: f32 = 1.0e-10;
pub const ROBOTS_MONSTER_NAV_SEGMENT_EPSILON: f32 = f32::from_bits(0x3a83_126f);
/// Handler+0x5F0 seeded by common Monster ctor `0x004515DD`.
pub const ROBOTS_MONSTER_NAV_BOUNDARY_PROBE_RADIUS: f32 = 0.75;
pub const ROBOTS_MONSTER_NAV_DIAGONAL: f32 = f32::from_bits(0x3f35_04e6);
/// Exact global direction table initialized by `0x00456960..0x00456A96` and
/// consumed in this order by common Monster pre-pass `0x00452220`.
pub const ROBOTS_MONSTER_NAV_BOUNDARY_PROBE_DIRECTIONS: [[f32; 3]; 8] = [
    [0.0, 0.0, 1.0],
    [
        ROBOTS_MONSTER_NAV_DIAGONAL,
        0.0,
        ROBOTS_MONSTER_NAV_DIAGONAL,
    ],
    [1.0, 0.0, 0.0],
    [
        -ROBOTS_MONSTER_NAV_DIAGONAL,
        0.0,
        ROBOTS_MONSTER_NAV_DIAGONAL,
    ],
    [0.0, 0.0, -1.0],
    [
        ROBOTS_MONSTER_NAV_DIAGONAL,
        0.0,
        -ROBOTS_MONSTER_NAV_DIAGONAL,
    ],
    [-1.0, 0.0, 0.0],
    [
        -ROBOTS_MONSTER_NAV_DIAGONAL,
        0.0,
        -ROBOTS_MONSTER_NAV_DIAGONAL,
    ],
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsMonsterNavGroup {
    pub start_face: u32,
    pub face_count: u32,
    /// Native `0x0046FA10` returns this high byte from the first group DWORD.
    pub flags0: u8,
    pub flags1: u16,
}

#[derive(Clone, Copy)]
pub struct RobotsMonsterNavMeshView<'a> {
    pub vertices: &'a [[f32; 3]],
    pub faces: &'a [[u32; 3]],
    pub adjacency: &'a [[Option<u32>; 3]],
    pub groups: &'a [RobotsMonsterNavGroup],
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RobotsMonsterNavigationRuntimeState {
    /// Host-stable ordinal replacing native Handler+0x61C pointer identity.
    pub region_ordinal: Option<usize>,
    /// Native Handler+0x614.
    pub face_index: Option<u32>,
    /// Native Handler+0x610, resolved through `0x0046FA10`.
    pub group_flags0: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsMonsterNavDirectSegmentResult {
    pub reached_target: bool,
    /// Exact target when reachable, otherwise the first blocked nav-edge
    /// intersection returned by `0x00470280 -> 0x00471160`.
    pub end_xyz: [f32; 3],
    pub face_index: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsMonsterNavBoundaryCorrectionResult {
    pub owner_xyz: [f32; 3],
    /// Handler+0x5C0/+0x5C4/+0x5C8 projection accumulated by `0x00452220`.
    pub correction_xyz: [f32; 3],
    /// Native Handler+0x5FC..+0x603: true when the corresponding radial probe
    /// was stopped by a nav boundary/group boundary.
    pub blocked_probes: [bool; 8],
    pub face_index: u32,
    pub floor_clamped: bool,
}

impl RobotsMonsterNavigationRuntimeState {
    /// Native `0x00451F40` fallback path: choose the first navigation region whose
    /// vertical face test accepts the current owner position.
    pub fn initialize(
        &mut self,
        regions: &[RobotsMonsterNavMeshView<'_>],
        owner_xyz: [f32; 3],
    ) -> bool {
        for (region_ordinal, region) in regions.iter().copied().enumerate() {
            if let Some(face_index) = region.find_face(owner_xyz) {
                self.region_ordinal = Some(region_ordinal);
                self.face_index = Some(face_index);
                self.group_flags0 = region.group_flags0_for_face(face_index);
                return true;
            }
        }
        *self = Self::default();
        false
    }

    /// Native `0x00451FC0`: once Handler+0x61C owns a region, face maintenance
    /// stays inside that region. It does not opportunistically jump to another region.
    pub fn maintain(
        &mut self,
        regions: &[RobotsMonsterNavMeshView<'_>],
        owner_xyz: [f32; 3],
    ) -> bool {
        let Some(region_ordinal) = self.region_ordinal else {
            return self.initialize(regions, owner_xyz);
        };
        let Some(region) = regions.get(region_ordinal).copied() else {
            *self = Self::default();
            return false;
        };
        let Some(face_index) = region.update_face(self.face_index, owner_xyz) else {
            self.face_index = None;
            self.group_flags0 = None;
            return false;
        };
        self.face_index = Some(face_index);
        self.group_flags0 = region.group_flags0_for_face(face_index);
        true
    }
}

impl<'a> RobotsMonsterNavMeshView<'a> {
    fn face_vertices(self, face_index: u32) -> Option<[[f32; 3]; 3]> {
        let face = *self.faces.get(face_index as usize)?;
        Some([
            *self.vertices.get(face[0] as usize)?,
            *self.vertices.get(face[1] as usize)?,
            *self.vertices.get(face[2] as usize)?,
        ])
    }

    /// Native `0x00470CA0` used by monster navigation. It casts a vertical ray
    /// from `owner_y + 1` in direction `(0,-1,0)` and accepts only `0 < t < 2`.
    /// Keeping the vertical window is important on stacked/sloped navigation.
    pub fn face_contains_owner_position(self, face_index: u32, owner_xyz: [f32; 3]) -> bool {
        let Some([a, b, c]) = self.face_vertices(face_index) else {
            return false;
        };
        let origin = [
            owner_xyz[0],
            owner_xyz[1] + ROBOTS_MONSTER_NAV_RAY_START_Y_OFFSET,
            owner_xyz[2],
        ];
        let direction = [0.0, ROBOTS_MONSTER_NAV_RAY_DIRECTION_Y, 0.0];
        ray_triangle_t(origin, direction, a, b, c)
            .is_some_and(|t| t > 0.0 && t < ROBOTS_MONSTER_NAV_RAY_MAX_T)
    }

    /// Native `0x0046F140` fallback: linear face search in the selected NavMesh region.
    pub fn find_face(self, owner_xyz: [f32; 3]) -> Option<u32> {
        (0..self.faces.len() as u32)
            .find(|face| self.face_contains_owner_position(*face, owner_xyz))
    }

    /// Native `0x0046F240 -> 0x0046F330 -> 0x0046F140` maintenance order:
    /// keep current face when valid, otherwise try its three adjacent faces, then
    /// fall back to a full region search.
    pub fn update_face(self, current_face: Option<u32>, owner_xyz: [f32; 3]) -> Option<u32> {
        if let Some(current_face) = current_face {
            if self.face_contains_owner_position(current_face, owner_xyz) {
                return Some(current_face);
            }
            if let Some(neighbors) = self.adjacency.get(current_face as usize) {
                for neighbor in neighbors.iter().flatten().copied() {
                    if self.face_contains_owner_position(neighbor, owner_xyz) {
                        return Some(neighbor);
                    }
                }
            }
        }
        self.find_face(owner_xyz)
    }

    /// Native `0x0046FA10`: map a face to the high-byte group/category value
    /// stored in the first group DWORD. This is the value cached in Handler+0x610.
    pub fn group_flags0_for_face(self, face_index: u32) -> Option<u8> {
        self.groups.iter().find_map(|group| {
            let end = group.start_face.saturating_add(group.face_count);
            (group.start_face <= face_index && face_index < end).then_some(group.flags0)
        })
    }

    /// Native NavMesh graph costs (`0x00550110` / `0x005501DD`) fall back to the
    /// geometric centre of each face when no precomputed centre/cost tables exist.
    pub fn face_center(self, face_index: u32) -> Option<[f32; 3]> {
        let [a, b, c] = self.face_vertices(face_index)?;
        Some([
            (a[0] + b[0] + c[0]) / 3.0,
            (a[1] + b[1] + c[1]) / 3.0,
            (a[2] + b[2] + c[2]) / 3.0,
        ])
    }

    fn group_ordinal_for_face(self, face_index: u32) -> Option<usize> {
        self.groups.iter().position(|group| {
            let end = group.start_face.saturating_add(group.face_count);
            group.start_face <= face_index && face_index < end
        })
    }

    /// Native `0x00470150` waypoint geometry: when a route advances from one
    /// face to the next, steering aims at the midpoint of the shared edge.
    pub fn shared_edge_midpoint(self, face_a: u32, face_b: u32) -> Option<[f32; 3]> {
        let a = *self.faces.get(face_a as usize)?;
        let b = *self.faces.get(face_b as usize)?;
        let mut shared = [u32::MAX; 2];
        let mut count = 0usize;
        for vertex in a {
            if b.contains(&vertex) && count < 2 {
                shared[count] = vertex;
                count += 1;
            }
        }
        if count != 2 {
            return None;
        }
        let p0 = *self.vertices.get(shared[0] as usize)?;
        let p1 = *self.vertices.get(shared[1] as usize)?;
        Some([
            (p0[0] + p1[0]) * 0.5,
            (p0[1] + p1[1]) * 0.5,
            (p0[2] + p1[2]) * 0.5,
        ])
    }

    /// Native route waypoint policy from `0x0046DB60`: interior route points are
    /// shared-edge midpoints, while the final route face uses its centre.
    pub fn route_waypoint(self, route: &[u32], route_index: usize) -> Option<[f32; 3]> {
        let face = *route.get(route_index)?;
        match route.get(route_index + 1).copied() {
            Some(next) => self.shared_edge_midpoint(face, next),
            None => self.face_center(face),
        }
    }

    /// Engine-neutral face walk for native `0x0046F050 -> 0x00471160`.
    /// Besides the success bit, native returns the exact end point: the target
    /// when reachable or the first blocked edge intersection otherwise.
    pub fn trace_direct_segment(
        self,
        owner_xyz: [f32; 3],
        target_xyz: [f32; 3],
        start_face: u32,
        allow_cross_group: bool,
    ) -> Option<RobotsMonsterNavDirectSegmentResult> {
        if start_face as usize >= self.faces.len()
            || !owner_xyz.iter().all(|value| value.is_finite())
            || !target_xyz.iter().all(|value| value.is_finite())
        {
            return None;
        }

        let start_group = (!allow_cross_group)
            .then(|| self.group_ordinal_for_face(start_face))
            .flatten();
        if !allow_cross_group && !self.groups.is_empty() && start_group.is_none() {
            return None;
        }

        let mut current_face = start_face;
        let mut previous_face = None::<u32>;
        let mut segment_start = owner_xyz;
        for _ in 0..=self.faces.len() {
            let face = self.faces.get(current_face as usize).copied()?;
            let edges = [[face[0], face[1]], [face[1], face[2]], [face[2], face[0]]];
            let mut best = None::<(f32, [f32; 3], [u32; 2])>;
            for edge in edges {
                if previous_face.is_some_and(|previous| {
                    self.faces
                        .get(previous as usize)
                        .is_some_and(|previous_face| {
                            previous_face.contains(&edge[0]) && previous_face.contains(&edge[1])
                        })
                }) {
                    continue;
                }
                let Some(a) = self.vertices.get(edge[0] as usize).copied() else {
                    continue;
                };
                let Some(b) = self.vertices.get(edge[1] as usize).copied() else {
                    continue;
                };
                let Some(hit) = segment_intersection_xz(segment_start, target_xyz, a, b) else {
                    continue;
                };
                let dx = hit[0] - segment_start[0];
                let dz = hit[2] - segment_start[2];
                let distance_squared = dx * dx + dz * dz;
                if distance_squared
                    <= ROBOTS_MONSTER_NAV_SEGMENT_EPSILON * ROBOTS_MONSTER_NAV_SEGMENT_EPSILON
                {
                    continue;
                }
                if best.is_none_or(|(best_distance, _, _)| distance_squared < best_distance) {
                    best = Some((distance_squared, hit, edge));
                }
            }

            let Some((_, hit, crossed_edge)) = best else {
                return Some(RobotsMonsterNavDirectSegmentResult {
                    reached_target: true,
                    end_xyz: target_xyz,
                    face_index: current_face,
                });
            };
            let next_face = self
                .adjacency
                .get(current_face as usize)
                .and_then(|neighbors| {
                    neighbors.iter().flatten().copied().find(|candidate| {
                        self.faces
                            .get(*candidate as usize)
                            .is_some_and(|candidate_face| {
                                candidate_face.contains(&crossed_edge[0])
                                    && candidate_face.contains(&crossed_edge[1])
                            })
                    })
                });
            let Some(next_face) = next_face else {
                return Some(RobotsMonsterNavDirectSegmentResult {
                    reached_target: false,
                    end_xyz: hit,
                    face_index: current_face,
                });
            };
            if !allow_cross_group
                && !self.groups.is_empty()
                && self.group_ordinal_for_face(next_face) != start_group
            {
                return Some(RobotsMonsterNavDirectSegmentResult {
                    reached_target: false,
                    end_xyz: hit,
                    face_index: current_face,
                });
            }

            previous_face = Some(current_face);
            current_face = next_face;
            segment_start = hit;
        }
        None
    }

    /// Compatibility predicate for callers that only need native's success bit.
    pub fn direct_segment_reaches_target(
        self,
        owner_xyz: [f32; 3],
        target_xyz: [f32; 3],
        start_face: u32,
        allow_cross_group: bool,
    ) -> bool {
        self.trace_direct_segment(owner_xyz, target_xyz, start_face, allow_cross_group)
            .is_some_and(|trace| trace.reached_target)
    }

    /// Exact current-face surface projection used by `0x0046F2B0 -> 0x004712F0`.
    /// Native casts from owner.y+1 down by two units and returns the intersected Y.
    pub fn face_surface_y(self, face_index: u32, owner_xyz: [f32; 3]) -> Option<f32> {
        let [a, b, c] = self.face_vertices(face_index)?;
        let origin = [
            owner_xyz[0],
            owner_xyz[1] + ROBOTS_MONSTER_NAV_RAY_START_Y_OFFSET,
            owner_xyz[2],
        ];
        let direction = [0.0, ROBOTS_MONSTER_NAV_RAY_DIRECTION_Y, 0.0];
        let t = ray_triangle_t(origin, direction, a, b, c)?;
        (t > 0.0 && t < ROBOTS_MONSTER_NAV_RAY_MAX_T).then_some(origin[1] + direction[1] * t)
    }

    /// Shared eight-direction radial boundary probe used by common Monster
    /// pre-pass `0x00452220` and EW07 Dodgem `AI_BounceNavMesh 0x0046D3E0`.
    /// This helper deliberately does not floor-clamp; callers own that separate
    /// native step. Each blocked probe moves the owner immediately, so later
    /// probes observe earlier corrections exactly like the executable.
    pub fn constrain_owner_radially(
        self,
        owner_xyz: [f32; 3],
        start_face: u32,
        probe_radius: f32,
        allow_cross_group: bool,
    ) -> Option<RobotsMonsterNavBoundaryCorrectionResult> {
        if !probe_radius.is_finite() || probe_radius < 0.0 {
            return None;
        }
        let mut owner = owner_xyz;
        let mut correction = [0.0; 3];
        let mut blocked_probes = [false; 8];
        let mut current_face = start_face;

        for (probe_index, direction) in ROBOTS_MONSTER_NAV_BOUNDARY_PROBE_DIRECTIONS
            .iter()
            .copied()
            .enumerate()
        {
            let target = [
                owner[0] + direction[0] * probe_radius,
                owner[1] + direction[1] * probe_radius,
                owner[2] + direction[2] * probe_radius,
            ];
            let trace =
                self.trace_direct_segment(owner, target, current_face, allow_cross_group)?;
            if trace.reached_target {
                continue;
            }

            blocked_probes[probe_index] = true;
            let delta = [
                trace.end_xyz[0] - target[0],
                trace.end_xyz[1] - target[1],
                trace.end_xyz[2] - target[2],
            ];
            for axis in 0..3 {
                correction[axis] += delta[axis];
                owner[axis] += delta[axis];
            }
            current_face = self.update_face(Some(current_face), owner)?;
        }

        Some(RobotsMonsterNavBoundaryCorrectionResult {
            owner_xyz: owner,
            correction_xyz: correction,
            blocked_probes,
            face_index: current_face,
            floor_clamped: false,
        })
    }

    /// Common Monster pre-pass `0x00452220`: keep the owner's 0.75-unit radial
    /// footprint inside the current NavMesh, then perform the separate
    /// `0x0046F2B0` current-face floor clamp at the tail.
    pub fn constrain_owner_to_navmesh(
        self,
        owner_xyz: [f32; 3],
        start_face: u32,
        probe_radius: f32,
        allow_cross_group: bool,
    ) -> Option<RobotsMonsterNavBoundaryCorrectionResult> {
        let mut result =
            self.constrain_owner_radially(owner_xyz, start_face, probe_radius, allow_cross_group)?;
        if let Some(floor_y) = self.face_surface_y(result.face_index, result.owner_xyz) {
            if result.owner_xyz[1] < floor_y {
                result.owner_xyz[1] = floor_y;
                result.floor_clamped = true;
            }
        }
        Some(result)
    }

    /// Engine-neutral form of the native A* core `0x00550EEB` used by
    /// `0x0046ECA0`. Neighbor order is preserved exactly (three face links),
    /// edge cost and heuristic are Euclidean face-centre distance, matching the
    /// native fallback when shipped precomputed tables are absent.
    pub fn find_face_route(self, start_face: u32, target_face: u32) -> Option<Vec<u32>> {
        let face_count = self.faces.len();
        let start = start_face as usize;
        let target = target_face as usize;
        if start >= face_count || target >= face_count {
            return None;
        }
        if start == target {
            return Some(vec![start_face]);
        }

        let mut open = vec![start_face];
        let mut closed = vec![false; face_count];
        let mut g = vec![f32::INFINITY; face_count];
        let mut parent = vec![None::<u32>; face_count];
        g[start] = 0.0;

        while !open.is_empty() {
            let mut best_open = 0usize;
            let mut best_f = f32::INFINITY;
            for (open_index, face) in open.iter().copied().enumerate() {
                let h = face_distance(self, face, target_face)?;
                let f = g[face as usize] + h;
                if f < best_f {
                    best_f = f;
                    best_open = open_index;
                }
            }

            let current = open.remove(best_open);
            if current == target_face {
                let mut route = vec![current];
                let mut cursor = current;
                while let Some(previous) = parent[cursor as usize] {
                    route.push(previous);
                    cursor = previous;
                }
                route.reverse();
                return Some(route);
            }
            closed[current as usize] = true;

            for neighbor in self.adjacency[current as usize].iter().flatten().copied() {
                let neighbor_index = neighbor as usize;
                if neighbor_index >= face_count || closed[neighbor_index] {
                    continue;
                }
                let tentative = g[current as usize] + face_distance(self, current, neighbor)?;
                if tentative >= g[neighbor_index] {
                    continue;
                }
                parent[neighbor_index] = Some(current);
                g[neighbor_index] = tentative;
                if !open.contains(&neighbor) {
                    open.push(neighbor);
                }
            }
        }
        None
    }

    /// Exact destination sampling used by NPC flag2 behavior `0x0046EDF0`.
    /// Native scans for the first NavMesh group whose high-byte category matches
    /// Handler+0x610, performs exactly five gameplay-global RNG draws inside that
    /// group's face range, keeps the sampled face with the greatest squared 3D
    /// distance from the current face centre, then feeds the winner into the
    /// shared A* pathfinder `0x00550EEB`.
    ///
    /// RNG ownership remains outside this module. The five already-consumed
    /// draws are supplied by the host so all global consumers keep one ordering
    /// boundary.
    pub fn sampled_farthest_face_route(
        self,
        current_face: u32,
        group_flags0: u8,
        draws: [u32; 5],
    ) -> Option<Vec<u32>> {
        let group = self
            .groups
            .iter()
            .copied()
            .find(|group| group.flags0 == group_flags0)?;
        if group.face_count == 0 {
            return None;
        }
        let current_center = self.face_center(current_face)?;
        let mut selected = None::<(u32, f32)>;
        for draw in draws {
            let candidate = group.start_face + draw % group.face_count;
            if candidate == current_face {
                continue;
            }
            let candidate_center = self.face_center(candidate)?;
            let dx = candidate_center[0] - current_center[0];
            let dy = candidate_center[1] - current_center[1];
            let dz = candidate_center[2] - current_center[2];
            let distance_squared = dx * dx + dy * dy + dz * dz;
            if selected.is_none_or(|(_, best_distance)| distance_squared > best_distance) {
                selected = Some((candidate, distance_squared));
            }
        }
        self.find_face_route(current_face, selected?.0)
    }
}

fn face_distance(nav: RobotsMonsterNavMeshView<'_>, a: u32, b: u32) -> Option<f32> {
    let a = nav.face_center(a)?;
    let b = nav.face_center(b)?;
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let dz = b[2] - a[2];
    Some((dx * dx + dy * dy + dz * dz).sqrt())
}

fn segment_intersection_xz(
    start: [f32; 3],
    end: [f32; 3],
    edge_start: [f32; 3],
    edge_end: [f32; 3],
) -> Option<[f32; 3]> {
    let dx = end[0] - start[0];
    let dz = end[2] - start[2];
    let ex = edge_end[0] - edge_start[0];
    let ez = edge_end[2] - edge_start[2];
    let denominator = dx * ez - ex * dz;
    if denominator.abs() < ROBOTS_MONSTER_NAV_SEGMENT_EPSILON {
        return None;
    }
    let inverse = denominator.recip();
    let t = ((start[2] - edge_start[2]) * ex - (start[0] - edge_start[0]) * ez) * inverse;
    let u = ((start[2] - edge_start[2]) * dx - (start[0] - edge_start[0]) * dz) * inverse;
    if !(0.0..=1.0).contains(&t) || !(0.0..=1.0).contains(&u) {
        return None;
    }
    Some([
        start[0] + dx * t,
        start[1] + (end[1] - start[1]) * t,
        start[2] + dz * t,
    ])
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn ray_triangle_t(
    origin: [f32; 3],
    direction: [f32; 3],
    a: [f32; 3],
    b: [f32; 3],
    c: [f32; 3],
) -> Option<f32> {
    let edge1 = sub(b, a);
    let edge2 = sub(c, a);
    let pvec = cross(direction, edge2);
    let determinant = dot(edge1, pvec);
    if determinant.abs() < ROBOTS_MONSTER_NAV_DETERMINANT_EPSILON {
        return None;
    }
    let inverse_determinant = determinant.recip();
    let tvec = sub(origin, a);
    let u = dot(tvec, pvec) * inverse_determinant;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let qvec = cross(tvec, edge1);
    let v = dot(direction, qvec) * inverse_determinant;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    Some(dot(edge2, qvec) * inverse_determinant)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mesh<'a>(
        vertices: &'a [[f32; 3]],
        faces: &'a [[u32; 3]],
        adjacency: &'a [[Option<u32>; 3]],
        groups: &'a [RobotsMonsterNavGroup],
    ) -> RobotsMonsterNavMeshView<'a> {
        RobotsMonsterNavMeshView {
            vertices,
            faces,
            adjacency,
            groups,
        }
    }

    #[test]
    fn vertical_face_window_matches_native_one_up_two_down_contract() {
        let vertices = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]];
        let faces = [[0, 1, 2]];
        let adjacency = [[None, None, None]];
        let nav = mesh(&vertices, &faces, &adjacency, &[]);
        assert!(nav.face_contains_owner_position(0, [0.25, 0.0, 0.25]));
        assert!(!nav.face_contains_owner_position(0, [0.25, 2.0, 0.25]));
        assert!(!nav.face_contains_owner_position(0, [2.0, 0.0, 2.0]));
    }

    #[test]
    fn current_face_uses_adjacent_face_before_full_search() {
        let vertices = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
        ];
        let faces = [[0, 1, 2], [1, 3, 2]];
        let adjacency = [[Some(1), None, None], [Some(0), None, None]];
        let nav = mesh(&vertices, &faces, &adjacency, &[]);
        assert_eq!(nav.update_face(Some(0), [0.8, 0.0, 0.8]), Some(1));
    }

    #[test]
    fn face_group_returns_native_high_byte_category_not_group_ordinal() {
        let groups = [
            RobotsMonsterNavGroup {
                start_face: 0,
                face_count: 3,
                flags0: 7,
                flags1: 0,
            },
            RobotsMonsterNavGroup {
                start_face: 3,
                face_count: 2,
                flags0: 11,
                flags1: 2,
            },
        ];
        let nav = mesh(&[], &[], &[], &groups);
        assert_eq!(nav.group_flags0_for_face(0), Some(7));
        assert_eq!(nav.group_flags0_for_face(4), Some(11));
        assert_eq!(nav.group_flags0_for_face(5), None);
    }

    #[test]
    fn navigation_owner_keeps_selected_region_and_updates_face_group() {
        let vertices_a = [[10.0, 0.0, 10.0], [11.0, 0.0, 10.0], [10.0, 0.0, 11.0]];
        let faces_a = [[0, 1, 2]];
        let adjacency_a = [[None, None, None]];
        let groups_a = [RobotsMonsterNavGroup {
            start_face: 0,
            face_count: 1,
            flags0: 3,
            flags1: 0,
        }];
        let vertices_b = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
        ];
        let faces_b = [[0, 1, 2], [1, 3, 2]];
        let adjacency_b = [[Some(1), None, None], [Some(0), None, None]];
        let groups_b = [RobotsMonsterNavGroup {
            start_face: 0,
            face_count: 2,
            flags0: 9,
            flags1: 0,
        }];
        let regions = [
            mesh(&vertices_a, &faces_a, &adjacency_a, &groups_a),
            mesh(&vertices_b, &faces_b, &adjacency_b, &groups_b),
        ];
        let mut state = RobotsMonsterNavigationRuntimeState::default();
        assert!(state.initialize(&regions, [0.2, 0.0, 0.2]));
        assert_eq!(state.region_ordinal, Some(1));
        assert_eq!(state.face_index, Some(0));
        assert_eq!(state.group_flags0, Some(9));
        assert!(state.maintain(&regions, [0.8, 0.0, 0.8]));
        assert_eq!(state.region_ordinal, Some(1));
        assert_eq!(state.face_index, Some(1));
    }

    #[test]
    fn face_route_matches_native_astar_face_center_fallback() {
        let vertices = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [2.0, 0.0, 0.0],
            [2.0, 0.0, 1.0],
        ];
        let faces = [[0, 1, 2], [1, 3, 2], [1, 4, 3], [4, 5, 3]];
        let adjacency = [
            [Some(1), None, None],
            [Some(0), Some(2), None],
            [Some(1), Some(3), None],
            [Some(2), None, None],
        ];
        let nav = mesh(&vertices, &faces, &adjacency, &[]);
        assert_eq!(nav.find_face_route(0, 3), Some(vec![0, 1, 2, 3]));
        assert_eq!(nav.find_face_route(2, 2), Some(vec![2]));
        assert_eq!(nav.find_face_route(0, 99), None);
    }

    #[test]
    fn sampled_farthest_route_consumes_candidates_inside_matching_group() {
        let vertices = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [2.0, 0.0, 0.0],
            [2.0, 0.0, 1.0],
        ];
        let faces = [[0, 1, 2], [1, 3, 2], [1, 4, 3], [4, 5, 3]];
        let adjacency = [
            [Some(1), None, None],
            [Some(0), Some(2), None],
            [Some(1), Some(3), None],
            [Some(2), None, None],
        ];
        let groups = [RobotsMonsterNavGroup {
            start_face: 0,
            face_count: 4,
            flags0: 7,
            flags1: 0,
        }];
        let nav = mesh(&vertices, &faces, &adjacency, &groups);
        assert_eq!(
            nav.sampled_farthest_face_route(0, 7, [1, 2, 3, 3, 1]),
            Some(vec![0, 1, 2, 3])
        );
        assert_eq!(nav.sampled_farthest_face_route(0, 7, [0; 5]), None);
        assert_eq!(nav.sampled_farthest_face_route(0, 9, [1, 2, 3, 3, 1]), None);
    }

    #[test]
    fn route_waypoint_uses_shared_edge_then_final_face_center() {
        let vertices = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
        ];
        let faces = [[0, 1, 2], [1, 3, 2]];
        let adjacency = [[Some(1), None, None], [Some(0), None, None]];
        let nav = mesh(&vertices, &faces, &adjacency, &[]);
        assert_eq!(nav.route_waypoint(&[0, 1], 0), Some([0.5, 0.0, 0.5]));
        let final_center = nav.route_waypoint(&[0, 1], 1).expect("final centre");
        assert!((final_center[0] - 2.0 / 3.0).abs() < 1.0e-6);
        assert!((final_center[2] - 2.0 / 3.0).abs() < 1.0e-6);
    }

    #[test]
    fn direct_segment_walk_requires_connected_faces_and_honors_group_gate() {
        let vertices = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
        ];
        let faces = [[0, 1, 2], [1, 3, 2]];
        let connected = [[Some(1), None, None], [Some(0), None, None]];
        let groups = [
            RobotsMonsterNavGroup {
                start_face: 0,
                face_count: 1,
                flags0: 1,
                flags1: 0,
            },
            RobotsMonsterNavGroup {
                start_face: 1,
                face_count: 1,
                flags0: 2,
                flags1: 0,
            },
        ];
        let nav = mesh(&vertices, &faces, &connected, &groups);
        assert!(nav.direct_segment_reaches_target([0.2, 0.0, 0.2], [0.8, 0.0, 0.8], 0, true,));
        assert!(!nav.direct_segment_reaches_target([0.2, 0.0, 0.2], [0.8, 0.0, 0.8], 0, false,));

        let blocked = [[None, None, None], [None, None, None]];
        let nav = mesh(&vertices, &faces, &blocked, &[]);
        assert!(!nav.direct_segment_reaches_target([0.2, 0.0, 0.2], [0.8, 0.0, 0.8], 0, true,));
        let trace = nav
            .trace_direct_segment([0.2, 0.0, 0.2], [0.8, 0.0, 0.8], 0, true)
            .expect("blocked trace");
        assert!(!trace.reached_target);
        assert!((trace.end_xyz[0] - 0.5).abs() < 1.0e-6);
        assert!((trace.end_xyz[2] - 0.5).abs() < 1.0e-6);
    }

    #[test]
    fn common_monster_boundary_probe_table_matches_native_initialiser_bits() {
        let bits = ROBOTS_MONSTER_NAV_BOUNDARY_PROBE_DIRECTIONS
            .map(|direction| direction.map(f32::to_bits));
        assert_eq!(
            ROBOTS_MONSTER_NAV_BOUNDARY_PROBE_RADIUS.to_bits(),
            0x3f40_0000
        );
        assert_eq!(ROBOTS_MONSTER_NAV_DIAGONAL.to_bits(), 0x3f35_04e6);
        assert_eq!(
            bits,
            [
                [0, 0, 0x3f80_0000],
                [0x3f35_04e6, 0, 0x3f35_04e6],
                [0x3f80_0000, 0, 0],
                [0xbf35_04e6, 0, 0x3f35_04e6],
                [0, 0, 0xbf80_0000],
                [0x3f35_04e6, 0, 0xbf35_04e6],
                [0xbf80_0000, 0, 0],
                [0xbf35_04e6, 0, 0xbf35_04e6],
            ]
        );
    }

    #[test]
    fn common_monster_nav_constraint_accumulates_blocked_probe_correction_then_clamps_floor() {
        let vertices = [
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [0.0, 0.0, 2.0],
            [2.0, 0.0, 2.0],
        ];
        let faces = [[0, 1, 2], [1, 3, 2]];
        let adjacency = [[Some(1), None, None], [Some(0), None, None]];
        let nav = mesh(&vertices, &faces, &adjacency, &[]);
        let result = nav
            .constrain_owner_to_navmesh(
                [0.4, -0.5, 1.0],
                0,
                ROBOTS_MONSTER_NAV_BOUNDARY_PROBE_RADIUS,
                true,
            )
            .expect("native common monster nav constraint");

        assert!((result.owner_xyz[0] - 0.75).abs() < 1.0e-5);
        assert!((result.owner_xyz[1] - 0.0).abs() < 1.0e-6);
        let diagonal_pullback =
            ROBOTS_MONSTER_NAV_BOUNDARY_PROBE_RADIUS * ROBOTS_MONSTER_NAV_DIAGONAL - 0.4;
        assert!((result.owner_xyz[2] - (1.0 - diagonal_pullback)).abs() < 1.0e-5);
        assert!((result.correction_xyz[0] - 0.35).abs() < 1.0e-5);
        assert!(result.correction_xyz[1].abs() < 1.0e-6);
        assert!((result.correction_xyz[2] + diagonal_pullback).abs() < 1.0e-5);
        assert!(result.blocked_probes[3]);
        assert!(result.blocked_probes[6]);
        assert!(result.floor_clamped);
        assert_eq!(result.face_index, 0);
    }
}
