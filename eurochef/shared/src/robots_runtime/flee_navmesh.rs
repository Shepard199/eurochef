use serde::Serialize;

use super::monster_navigation::RobotsMonsterNavMeshView;

pub const ROBOTS_FLEE_NAVMESH_PRIORITY: u8 = 0x50;
pub const ROBOTS_FLEE_NAVMESH_ROUTE_SAMPLE_COUNT: usize = 5;
pub const ROBOTS_FLEE_NAVMESH_WAYPOINT_DISTANCE_SQUARED: f32 = 2.25;
pub const ROBOTS_FLEE_NAVMESH_TARGET_LOCOMOTION_SCALAR: f32 = 1.0;
pub const ROBOTS_FLEE_NAVMESH_TURN_RATE_RADIANS_PER_SECOND: f32 = std::f32::consts::PI;
pub const ROBOTS_FLEE_NAVMESH_FALLBACK_ANIM_MODE: u32 = 0x0900_0004;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsFleeNavMeshConfig {
    pub enter_radius: f32,
    pub retain_radius: f32,
}

impl RobotsFleeNavMeshConfig {
    /// EB07 MineBot builder `0x0045ECF0 -> AI_FleeNavMesh::Setup 0x00458CE0`.
    pub const fn eb07_minebot() -> Self {
        Self {
            enter_radius: 12.0,
            retain_radius: 16.0,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct RobotsFleeNavMeshRuntimeState {
    /// Native node vector `+0x2C..+0x38`, populated by `0x0046EDF0`.
    pub route: Vec<u32>,
    /// Native node +0x3C.
    pub route_index: usize,
    /// Native node +0x40..+0x48. Steering consumes XYZ only.
    pub waypoint_xyz: Option<[f32; 3]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsFleeNavMeshAction {
    RequestIdleAttack,
    SteerToWaypoint {
        target_yaw_radians: f32,
        target_locomotion_scalar: f32,
        turn_rate_radians_per_second: f32,
    },
}

/// Native priority callback `0x0046A5B0`. The common behavior active byte chooses
/// the 12/16 full-XYZ hysteresis edge; a valid Monster NavMesh region is mandatory.
pub fn flee_navmesh_priority(
    config: RobotsFleeNavMeshConfig,
    was_active: bool,
    nav_region_available: bool,
    owner_position_xyz: [f32; 3],
    target_position_xyz: Option<[f32; 3]>,
) -> u8 {
    if !nav_region_available {
        return 1;
    }
    let Some(target) = target_position_xyz else {
        return 1;
    };
    if !owner_position_xyz.iter().all(|value| value.is_finite())
        || !target.iter().all(|value| value.is_finite())
    {
        return 1;
    }
    let dx = owner_position_xyz[0] - target[0];
    let dy = owner_position_xyz[1] - target[1];
    let dz = owner_position_xyz[2] - target[2];
    let distance_squared = dx * dx + dy * dy + dz * dz;
    let eligible = if was_active {
        distance_squared < config.retain_radius * config.retain_radius
    } else {
        distance_squared <= config.enter_radius * config.enter_radius
    };
    if eligible {
        ROBOTS_FLEE_NAVMESH_PRIORITY
    } else {
        1
    }
}

pub fn flee_navmesh_enter_needs_rng(state: &RobotsFleeNavMeshRuntimeState) -> bool {
    state.route.is_empty()
}

fn rebuild_flee_route(
    state: &mut RobotsFleeNavMeshRuntimeState,
    nav: RobotsMonsterNavMeshView<'_>,
    current_face: u32,
    group_flags0: u8,
    draws: [u32; ROBOTS_FLEE_NAVMESH_ROUTE_SAMPLE_COUNT],
) {
    // `0x0046A7C0` resets the cursor before `0x0046EDF0`. The native float 10.0
    // passed to 0x0046EDF0 is provably unread by that function; route selection is
    // exactly five global-RNG samples followed by the shared A* face route.
    state.route_index = 0;
    if let Some(route) = nav.sampled_farthest_face_route(current_face, group_flags0, draws) {
        state.route = route;
    }
    if state.route.len() > 1 {
        state.waypoint_xyz = nav.shared_edge_midpoint(state.route[0], state.route[1]);
    } else {
        state.waypoint_xyz = None;
    }
}

/// Native enter callback `0x0046A790` only builds a route when no retained route
/// exists. An editor host without the process-global RNG anchor fails closed before
/// mutating state rather than manufacturing the five draws native always has.
pub fn enter_flee_navmesh(
    state: &mut RobotsFleeNavMeshRuntimeState,
    nav: RobotsMonsterNavMeshView<'_>,
    current_face: u32,
    group_flags0: u8,
    draws: Option<[u32; ROBOTS_FLEE_NAVMESH_ROUTE_SAMPLE_COUNT]>,
) -> bool {
    if !state.route.is_empty() {
        return true;
    }
    let Some(draws) = draws else {
        return false;
    };
    rebuild_flee_route(state, nav, current_face, group_flags0, draws);
    true
}

/// `0x0046A6B0` rebuilds immediately after reaching the final routed edge. Hosts
/// use this preflight to acquire all five global draws transactionally.
pub fn flee_navmesh_step_needs_rng(
    state: &RobotsFleeNavMeshRuntimeState,
    owner_position_xyz: [f32; 3],
) -> bool {
    if state.route.len() < 2 {
        return false;
    }
    let Some(waypoint) = state.waypoint_xyz else {
        return false;
    };
    let dx = owner_position_xyz[0] - waypoint[0];
    let dy = owner_position_xyz[1] - waypoint[1];
    let dz = owner_position_xyz[2] - waypoint[2];
    let reached = dx * dx + dy * dy + dz * dz < ROBOTS_FLEE_NAVMESH_WAYPOINT_DISTANCE_SQUARED;
    reached && state.route_index.saturating_add(1) >= state.route.len().saturating_sub(1)
}

/// Native execute `0x0046A6B0`. Route traversal uses shared-edge midpoints; at the
/// final edge it immediately rebuilds another sampled-farthest route. A route with
/// fewer than two faces requests AnimMode04 instead of inventing direct movement.
pub fn step_flee_navmesh(
    state: &mut RobotsFleeNavMeshRuntimeState,
    nav: RobotsMonsterNavMeshView<'_>,
    owner_position_xyz: [f32; 3],
    current_face: u32,
    group_flags0: u8,
    rebuild_draws: Option<[u32; ROBOTS_FLEE_NAVMESH_ROUTE_SAMPLE_COUNT]>,
) -> Option<RobotsFleeNavMeshAction> {
    if state.route.len() < 2 {
        return Some(RobotsFleeNavMeshAction::RequestIdleAttack);
    }
    let waypoint = state.waypoint_xyz?;
    let dx = owner_position_xyz[0] - waypoint[0];
    let dy = owner_position_xyz[1] - waypoint[1];
    let dz = owner_position_xyz[2] - waypoint[2];
    if dx * dx + dy * dy + dz * dz < ROBOTS_FLEE_NAVMESH_WAYPOINT_DISTANCE_SQUARED {
        let next_index = state.route_index.saturating_add(1);
        if next_index < state.route.len().saturating_sub(1) {
            state.route_index = next_index;
            state.waypoint_xyz = nav.shared_edge_midpoint(
                state.route[state.route_index],
                state.route[state.route_index + 1],
            );
        } else {
            let draws = rebuild_draws?;
            rebuild_flee_route(state, nav, current_face, group_flags0, draws);
        }
    }

    let waypoint = state.waypoint_xyz?;
    let target_dx = waypoint[0] - owner_position_xyz[0];
    let target_dz = waypoint[2] - owner_position_xyz[2];
    let target_yaw_radians = target_dx.atan2(target_dz);
    Some(RobotsFleeNavMeshAction::SteerToWaypoint {
        target_yaw_radians,
        target_locomotion_scalar: ROBOTS_FLEE_NAVMESH_TARGET_LOCOMOTION_SCALAR,
        turn_rate_radians_per_second: ROBOTS_FLEE_NAVMESH_TURN_RATE_RADIANS_PER_SECOND,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eb07_priority_uses_native_full_xyz_12_16_hysteresis() {
        let config = RobotsFleeNavMeshConfig::eb07_minebot();
        assert_eq!(
            flee_navmesh_priority(config, false, true, [0.0; 3], Some([12.0, 0.0, 0.0])),
            ROBOTS_FLEE_NAVMESH_PRIORITY
        );
        assert_eq!(
            flee_navmesh_priority(config, false, true, [0.0; 3], Some([12.01, 0.0, 0.0])),
            1
        );
        assert_eq!(
            flee_navmesh_priority(config, true, true, [0.0; 3], Some([15.99, 0.0, 0.0])),
            ROBOTS_FLEE_NAVMESH_PRIORITY
        );
        assert_eq!(
            flee_navmesh_priority(config, true, true, [0.0; 3], Some([16.0, 0.0, 0.0])),
            1
        );
        assert_eq!(
            flee_navmesh_priority(config, false, false, [0.0; 3], Some([1.0, 0.0, 0.0])),
            1
        );
    }
}
