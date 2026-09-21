use std::f32::consts::FRAC_PI_2;

use serde::Serialize;

use super::{
    character_physics::RobotsCharacterPhysicsRuntimeState,
    follow_network_path::RobotsFollowNetworkPathConfig,
    monster_navigation::RobotsMonsterNavMeshView,
};

pub const ROBOTS_NPC_RANDOM_IDLE_FALLBACK_PRIORITY: u8 = 1;
pub const ROBOTS_NPC_FLAG2_BEHAVIOR_PRIORITY: u8 = 0x0B;
pub const ROBOTS_NPC_PROXIMITY_PRIORITY: u8 = 0x15;
pub const ROBOTS_NPC_RANDOM_IDLE_PRIORITY: u8 = 0x16;
/// `XItemHandler_Npc::0x0046B4A0 -> AI_FollowNetworkPath::0x0046BF00`.
/// The node ctor seeds priority 0x0F and setup passes locomotion scalar 0.1.
pub const ROBOTS_NPC_FOLLOW_NETWORK_PATH_PRIORITY: u8 = 0x0F;
pub const ROBOTS_NPC_FOLLOW_NETWORK_PATH_LOCOMOTION_SCALAR: f32 = 0.1;
/// Native class name is `AI_PatrolNavMesh2`; NPC `flags & 2` is only one owner.
pub const ROBOTS_PATROL_NAVMESH2_PRIORITY: u8 = ROBOTS_NPC_FLAG2_BEHAVIOR_PRIORITY;
/// Native class name is `AI_PeriodicIdle`; the same node is reused by monsters.
pub const ROBOTS_PERIODIC_IDLE_PRIORITY: u8 = ROBOTS_NPC_RANDOM_IDLE_PRIORITY;
pub const ROBOTS_NPC_RANDOM_IDLE_BASE_DELAY_TICKS: i32 = 300;
pub const ROBOTS_NPC_FLAG2_ROUTE_SAMPLE_COUNT: usize = 5;
pub const ROBOTS_NPC_FLAG2_ROUTE_PARAMETER: f32 = 20.0;
pub const ROBOTS_NPC_FLAG2_WAYPOINT_DISTANCE_SQUARED: f32 = 2.25;
pub const ROBOTS_NPC_FLAG2_REBUILD_COOLDOWN_SECONDS: f32 = 0.25;
pub const ROBOTS_NPC_FLAG2_REBUILD_COOLDOWN_EPSILON: f32 = f32::from_bits(0x3a83_126f);
pub const ROBOTS_NPC_FLAG2_STUCK_ERROR_THRESHOLD: f32 = 0.9;
pub const ROBOTS_NPC_FLAG2_TARGET_LOCOMOTION_SCALAR: f32 = 0.0;
pub const ROBOTS_NPC_FLAG2_TURN_RATE_RADIANS_PER_SECOND: f32 = std::f32::consts::PI;

pub const ROBOTS_NPC_PROXIMITY_ENTER_DISTANCE: f32 = 3.5;
pub const ROBOTS_NPC_PROXIMITY_EXIT_DISTANCE: f32 = 5.0;
pub const ROBOTS_NPC_FACE_TURN_ENTER_RADIANS: f32 = 0.698_131_7;
pub const ROBOTS_NPC_FACE_TURN_EXIT_RADIANS: f32 = 0.174_532_92;
pub const ROBOTS_NPC_FACE_TURN_RATE_RADIANS_PER_SECOND: f32 = FRAC_PI_2;

pub const ROBOTS_ANIM_MODE_IDLE_ATTACK: u32 = 0x0900_0004;
pub const ROBOTS_ANIM_MODE_EXTENDED_IDLE: u32 = 0x0900_0006;
pub const ROBOTS_ANIM_MODE_EXTENDED_IDLE2: u32 = 0x0900_0007;
pub const ROBOTS_ANIM_MODE_EXTENDED_IDLE3: u32 = 0x0900_0008;
pub const ROBOTS_ANIM_MODE_EXTENDED_IDLE4: u32 = 0x0900_0009;
pub const ROBOTS_ANIM_MODE_EXTENDED_IDLE5: u32 = 0x0900_000A;
pub const ROBOTS_ANIM_MODE_TURN_ON_SPOT: u32 = 0x0900_0028;

pub const ROBOTS_NPC_RANDOM_IDLE_ANIM_MODES: [u32; 5] = [
    ROBOTS_ANIM_MODE_EXTENDED_IDLE,
    ROBOTS_ANIM_MODE_EXTENDED_IDLE2,
    ROBOTS_ANIM_MODE_EXTENDED_IDLE3,
    ROBOTS_ANIM_MODE_EXTENDED_IDLE4,
    ROBOTS_ANIM_MODE_EXTENDED_IDLE5,
];

/// `XItemHandler_Npc::0x0046B4A0` special `flags & 0x20` diner profile.
/// AI_Idle is configured with priority 2 / IdleDiner1, while the existing
/// AI_PeriodicIdle node uses the two optional follow-up diner modes.
pub const ROBOTS_NPC_DINER_IDLE_PRIORITY: u8 = 2;
pub const ROBOTS_NPC_DINER_IDLE_ANIM_MODE: u32 = 0x0900_00EB;
pub const ROBOTS_NPC_DINER_PERIODIC_ANIM_MODES: [u32; 2] = [0x0900_00EC, 0x0900_00ED];

pub const fn npc_follow_network_path_config() -> RobotsFollowNetworkPathConfig {
    RobotsFollowNetworkPathConfig {
        priority: ROBOTS_NPC_FOLLOW_NETWORK_PATH_PRIORITY,
        target_locomotion_scalar: ROBOTS_NPC_FOLLOW_NETWORK_PATH_LOCOMOTION_SCALAR,
    }
}

/// Exact `XItemHandler_Npc::0x0046B5BB..0x0046B5E3` diner-profile
/// CharacterPhysics bootstrap. Handler+0x604 is cleared by the native caller too,
/// but that byte has no recovered NPC consumer yet and therefore remains host-side.
pub fn bootstrap_npc_diner_physics(state: &mut RobotsCharacterPhysicsRuntimeState) {
    state.apply_handler_606_mode(false);
    state.set_object_flag_bit2(true);
    state.service_attack_bit4(true);
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RobotsNpcFlag2RuntimeState {
    /// Native node vector `+0x28..+0x34`, populated by `0x0046EDF0`.
    pub route: Vec<u32>,
    /// Native node `+0x38` route cursor.
    pub route_index: usize,
    /// Native node `+0x4C`: false walks forward, true walks the same route backward.
    pub reverse: bool,
    /// Native node `+0x3C..+0x48`; W is not consumed by steering, so the
    /// engine-neutral seam keeps XYZ only.
    pub waypoint_xyz: Option<[f32; 3]>,
    /// Native node `+0x50`. `0x0046CF90` subtracts one scaled fixed step.
    pub rebuild_cooldown_seconds: f32,
}

impl Default for RobotsNpcFlag2RuntimeState {
    fn default() -> Self {
        Self {
            route: Vec::new(),
            route_index: 0,
            reverse: false,
            waypoint_xyz: None,
            rebuild_cooldown_seconds: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsNpcFlag2Action {
    RequestIdleAttack,
    SteerToWaypoint {
        target_yaw_radians: f32,
        target_locomotion_scalar: f32,
        turn_rate_radians_per_second: f32,
    },
}

pub type RobotsPatrolNavMesh2RuntimeState = RobotsNpcFlag2RuntimeState;
pub type RobotsPatrolNavMesh2Action = RobotsNpcFlag2Action;

/// Priority callback `0x0046CE40`. The caller resolves the native global state
/// stack gate (`1/2/3/0x0F`) into `global_state_blocked` before entering shared code.
pub fn npc_flag2_priority(nav_region_available: bool, global_state_blocked: bool) -> u8 {
    if nav_region_available && !global_state_blocked {
        ROBOTS_NPC_FLAG2_BEHAVIOR_PRIORITY
    } else {
        1
    }
}

pub fn npc_flag2_enter_needs_rebuild(
    state: &RobotsNpcFlag2RuntimeState,
    current_face: u32,
) -> bool {
    if state.route.is_empty() {
        return true;
    }
    let Some(found_index) = state.route.iter().position(|face| *face == current_face) else {
        return true;
    };
    found_index.abs_diff(state.route_index) > 2
}

pub fn npc_flag2_step_needs_rebuild(
    state: &RobotsNpcFlag2RuntimeState,
    movement_error_ratio: f32,
) -> bool {
    state.route.len() < 2 || movement_error_ratio > ROBOTS_NPC_FLAG2_STUCK_ERROR_THRESHOLD
}

pub fn npc_flag2_rebuild_will_consume_rng(state: &RobotsNpcFlag2RuntimeState) -> bool {
    state.rebuild_cooldown_seconds <= ROBOTS_NPC_FLAG2_REBUILD_COOLDOWN_EPSILON
}

fn rebuild_npc_flag2_route(
    state: &mut RobotsNpcFlag2RuntimeState,
    nav: RobotsMonsterNavMeshView<'_>,
    current_face: u32,
    group_flags0: u8,
    draws: &mut Option<[u32; ROBOTS_NPC_FLAG2_ROUTE_SAMPLE_COUNT]>,
) {
    // `0x0046CEF0` always resets traversal direction/cursor, even when the
    // cooldown suppresses an actual A* rebuild.
    state.route_index = 0;
    state.reverse = false;
    if !npc_flag2_rebuild_will_consume_rng(state) {
        return;
    }
    let Some(draws) = draws.take() else {
        // Native always has the global RNG. An editor host without a proven
        // stream fails closed instead of manufacturing five draws.
        return;
    };
    state.rebuild_cooldown_seconds = ROBOTS_NPC_FLAG2_REBUILD_COOLDOWN_SECONDS;
    if let Some(route) = nav.sampled_farthest_face_route(current_face, group_flags0, draws) {
        state.route = route;
        if state.route.len() > 1 {
            state.waypoint_xyz = nav.shared_edge_midpoint(state.route[0], state.route[1]);
        }
    }
}

/// Enter callback `0x0046CE90`: retain an existing route only when the current
/// Handler+0x614 face is present within two route slots of the retained cursor.
pub fn enter_npc_flag2(
    state: &mut RobotsNpcFlag2RuntimeState,
    nav: RobotsMonsterNavMeshView<'_>,
    current_face: u32,
    group_flags0: u8,
    draws: Option<[u32; ROBOTS_NPC_FLAG2_ROUTE_SAMPLE_COUNT]>,
) {
    if !npc_flag2_enter_needs_rebuild(state, current_face) {
        return;
    }
    let mut draws = draws;
    rebuild_npc_flag2_route(state, nav, current_face, group_flags0, &mut draws);
}

fn advance_npc_flag2_waypoint(
    state: &mut RobotsNpcFlag2RuntimeState,
    nav: RobotsMonsterNavMeshView<'_>,
) {
    if !state.reverse {
        state.route_index = state.route_index.saturating_add(1);
        if state.route_index < state.route.len().saturating_sub(1) {
            state.waypoint_xyz = nav.shared_edge_midpoint(
                state.route[state.route_index],
                state.route[state.route_index + 1],
            );
        } else if let Some(face) = state.route.get(state.route_index).copied() {
            state.reverse = true;
            state.waypoint_xyz = nav.face_center(face);
        }
    } else {
        state.route_index = state.route_index.saturating_sub(1);
        if state.route_index < 1 {
            state.reverse = false;
            state.waypoint_xyz = state
                .route
                .get(state.route_index)
                .copied()
                .and_then(|face| nav.face_center(face));
        } else {
            state.waypoint_xyz = nav.shared_edge_midpoint(
                state.route[state.route_index],
                state.route[state.route_index - 1],
            );
        }
    }
}

/// Execute callback `0x0046CC70`. Route generation and traversal stay here;
/// actual locomotion remains in the shared AI locomotion reducer `0x00452800`.
pub fn step_npc_flag2(
    state: &mut RobotsNpcFlag2RuntimeState,
    nav: RobotsMonsterNavMeshView<'_>,
    owner_xyz: [f32; 3],
    current_face: u32,
    group_flags0: u8,
    movement_error_ratio: f32,
    draws: Option<[u32; ROBOTS_NPC_FLAG2_ROUTE_SAMPLE_COUNT]>,
) -> RobotsNpcFlag2Action {
    let mut draws = draws;
    if state.route.len() < 2 {
        rebuild_npc_flag2_route(state, nav, current_face, group_flags0, &mut draws);
    }
    if movement_error_ratio > ROBOTS_NPC_FLAG2_STUCK_ERROR_THRESHOLD {
        rebuild_npc_flag2_route(state, nav, current_face, group_flags0, &mut draws);
    }
    if state.route.len() < 2 {
        return RobotsNpcFlag2Action::RequestIdleAttack;
    }
    let Some(mut waypoint) = state.waypoint_xyz else {
        return RobotsNpcFlag2Action::RequestIdleAttack;
    };
    let dx = owner_xyz[0] - waypoint[0];
    let dy = owner_xyz[1] - waypoint[1];
    let dz = owner_xyz[2] - waypoint[2];
    if dx * dx + dy * dy + dz * dz < ROBOTS_NPC_FLAG2_WAYPOINT_DISTANCE_SQUARED {
        advance_npc_flag2_waypoint(state, nav);
        let Some(next_waypoint) = state.waypoint_xyz else {
            return RobotsNpcFlag2Action::RequestIdleAttack;
        };
        waypoint = next_waypoint;
    }
    RobotsNpcFlag2Action::SteerToWaypoint {
        target_yaw_radians: (waypoint[0] - owner_xyz[0]).atan2(waypoint[2] - owner_xyz[2]),
        target_locomotion_scalar: ROBOTS_NPC_FLAG2_TARGET_LOCOMOTION_SCALAR,
        turn_rate_radians_per_second: ROBOTS_NPC_FLAG2_TURN_RATE_RADIANS_PER_SECOND,
    }
}

pub fn tick_npc_flag2(state: &mut RobotsNpcFlag2RuntimeState, runtime_rate_scale: f32) {
    state.rebuild_cooldown_seconds -=
        runtime_rate_scale.max(0.0) * super::locomotion::ROBOTS_FIXED_STEP_SECONDS;
}

// Native names for the same reusable node. Keep the established NPC API as
// compatibility wrappers while monster brains consume the class-accurate names.
pub use enter_npc_flag2 as enter_patrol_navmesh2;
pub use npc_flag2_enter_needs_rebuild as patrol_navmesh2_enter_needs_rebuild;
pub use npc_flag2_priority as patrol_navmesh2_priority;
pub use npc_flag2_rebuild_will_consume_rng as patrol_navmesh2_rebuild_will_consume_rng;
pub use npc_flag2_step_needs_rebuild as patrol_navmesh2_step_needs_rebuild;
pub use step_npc_flag2 as step_patrol_navmesh2;
pub use tick_npc_flag2 as tick_patrol_navmesh2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsNpcRandomIdleRuntimeState {
    /// Base behavior-node active latch `+0x09`.
    pub node_active: bool,
    /// Base behavior completion latch `+0x21`. `0x00459360` sets it on SetupIdle.
    pub completion_latch: bool,
    /// Random node `+0x28`; decremented only while the node is inactive.
    pub delay_ticks: i32,
    /// Random node `+0x3C`; selected from the shipped ExtendedIdle prefix on enter.
    pub selected_anim_mode: Option<u32>,
    /// Native config `0x004591C0` consumes one gameplay-global RNG draw immediately.
    pub initialized: bool,
}

impl Default for RobotsNpcRandomIdleRuntimeState {
    fn default() -> Self {
        Self {
            node_active: false,
            completion_latch: false,
            delay_ticks: 0,
            selected_anim_mode: None,
            initialized: false,
        }
    }
}

pub type RobotsPeriodicIdleRuntimeState = RobotsNpcRandomIdleRuntimeState;

/// Exact positive-delay jitter used by `0x004591C0` and `0x004592D0`.
/// With the shipped base delay 300 this yields the inclusive range 225..=374.
pub fn npc_random_idle_delay_from_draw(base_delay_ticks: i32, draw: u32) -> i32 {
    let quarter = base_delay_ticks / 4;
    let width = quarter.saturating_mul(2);
    if width <= 0 {
        base_delay_ticks.saturating_sub(quarter)
    } else {
        base_delay_ticks + (draw % width as u32) as i32 - quarter
    }
}

pub fn initialize_periodic_idle(
    state: &mut RobotsPeriodicIdleRuntimeState,
    base_delay_ticks: i32,
    draw: u32,
) {
    state.delay_ticks = npc_random_idle_delay_from_draw(base_delay_ticks, draw);
    state.initialized = true;
}

pub fn initialize_npc_random_idle(state: &mut RobotsNpcRandomIdleRuntimeState, draw: u32) {
    initialize_periodic_idle(state, ROBOTS_NPC_RANDOM_IDLE_BASE_DELAY_TICKS, draw);
}

/// Exact `AI_PeriodicIdle` priority callback `0x004592A0` after the base gate.
pub fn periodic_idle_priority(state: &RobotsPeriodicIdleRuntimeState) -> u8 {
    if !state.initialized {
        return ROBOTS_NPC_RANDOM_IDLE_FALLBACK_PRIORITY;
    }
    if (!state.node_active || state.completion_latch) && state.delay_ticks > 0 {
        ROBOTS_NPC_RANDOM_IDLE_FALLBACK_PRIORITY
    } else {
        ROBOTS_PERIODIC_IDLE_PRIORITY
    }
}

pub fn npc_random_idle_priority(state: &RobotsNpcRandomIdleRuntimeState) -> u8 {
    periodic_idle_priority(state)
}

/// Native enter `0x004592D0`: activate, choose one available idle animation,
/// then resample the delay for the next activation. The caller supplies the two
/// consecutive `0x00509C48` draws so the shared layer does not own global RNG.
pub fn enter_periodic_idle(
    state: &mut RobotsPeriodicIdleRuntimeState,
    base_delay_ticks: i32,
    available_anim_modes: &[u32],
    select_draw: u32,
    delay_draw: u32,
) -> Option<u32> {
    if available_anim_modes.is_empty() {
        return None;
    }
    state.node_active = true;
    state.completion_latch = false;
    let selected = available_anim_modes[select_draw as usize % available_anim_modes.len()];
    state.selected_anim_mode = Some(selected);
    state.delay_ticks = npc_random_idle_delay_from_draw(base_delay_ticks, delay_draw);
    Some(selected)
}

pub fn enter_npc_random_idle(
    state: &mut RobotsNpcRandomIdleRuntimeState,
    available_anim_modes: &[u32],
    select_draw: u32,
    delay_draw: u32,
) -> Option<u32> {
    enter_periodic_idle(
        state,
        ROBOTS_NPC_RANDOM_IDLE_BASE_DELAY_TICKS,
        available_anim_modes,
        select_draw,
        delay_draw,
    )
}

pub fn leave_periodic_idle(state: &mut RobotsPeriodicIdleRuntimeState) {
    state.node_active = false;
}

pub fn leave_npc_random_idle(state: &mut RobotsNpcRandomIdleRuntimeState) {
    leave_periodic_idle(state);
}

pub fn tick_periodic_idle(state: &mut RobotsPeriodicIdleRuntimeState) {
    if !state.node_active && state.delay_ticks > 0 {
        state.delay_ticks -= 1;
    }
}

pub fn tick_npc_random_idle(state: &mut RobotsNpcRandomIdleRuntimeState) {
    tick_periodic_idle(state);
}

pub fn complete_periodic_idle_setup_idle(state: &mut RobotsPeriodicIdleRuntimeState) {
    state.completion_latch = true;
}

pub fn complete_npc_random_idle_setup_idle(state: &mut RobotsNpcRandomIdleRuntimeState) {
    complete_periodic_idle_setup_idle(state);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsNpcFacePlayerState {
    Aligned,
    Turning,
}

impl Default for RobotsNpcFacePlayerState {
    fn default() -> Self {
        Self::Aligned
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsNpcProximityBehaviorState {
    /// Base behavior-node active latch `+0x09`, consumed by `0x00458D00` to
    /// choose the 3.5/5.0 distance hysteresis threshold.
    pub node_active: bool,
    /// Native proximity-node `+0x2C`: 0 = aligned/IdleAttack, 1 = turn-to-player.
    pub face_state: RobotsNpcFacePlayerState,
}

impl Default for RobotsNpcProximityBehaviorState {
    fn default() -> Self {
        Self {
            node_active: false,
            face_state: RobotsNpcFacePlayerState::Aligned,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsNpcProximityAction {
    RequestAnimMode {
        anim_mode: u32,
    },
    TurnTowardPlayer {
        yaw_error_radians: f32,
        max_turn_radians_per_second: f32,
        anim_mode: u32,
    },
}

/// Exact eligibility/priority gate from proximity behavior node `0x00458D00`.
/// Native uses squared 3D XItem distance and strict `<` comparisons.
pub fn npc_proximity_priority(
    node_active: bool,
    player_target_exists: bool,
    distance_squared: f32,
) -> u8 {
    if !player_target_exists || !distance_squared.is_finite() {
        return 1;
    }
    let distance = if node_active {
        ROBOTS_NPC_PROXIMITY_EXIT_DISTANCE
    } else {
        ROBOTS_NPC_PROXIMITY_ENTER_DISTANCE
    };
    if distance_squared < distance * distance {
        ROBOTS_NPC_PROXIMITY_PRIORITY
    } else {
        1
    }
}

/// Exact `+0x3C -> 0x00458DE0`, then `+0x40 -> 0x00458E50` state/action order.
///
/// State 0 starts a turn only when owner vslot `+0x154` allows it and absolute
/// Player yaw error is strictly greater than 40 degrees. State 1 exits only when
/// the error is strictly below 10 degrees. The action is selected after that
/// state update in the same behavior execute call.
pub fn step_npc_proximity_face_player(
    state: &mut RobotsNpcProximityBehaviorState,
    owner_face_player_allowed: bool,
    yaw_error_radians: f32,
) -> RobotsNpcProximityAction {
    let abs_yaw = yaw_error_radians.abs();
    match state.face_state {
        RobotsNpcFacePlayerState::Turning => {
            if abs_yaw < ROBOTS_NPC_FACE_TURN_EXIT_RADIANS {
                state.face_state = RobotsNpcFacePlayerState::Aligned;
            }
        }
        RobotsNpcFacePlayerState::Aligned => {
            if owner_face_player_allowed && abs_yaw > ROBOTS_NPC_FACE_TURN_ENTER_RADIANS {
                state.face_state = RobotsNpcFacePlayerState::Turning;
            }
        }
    }

    match state.face_state {
        RobotsNpcFacePlayerState::Aligned => RobotsNpcProximityAction::RequestAnimMode {
            anim_mode: ROBOTS_ANIM_MODE_IDLE_ATTACK,
        },
        RobotsNpcFacePlayerState::Turning => RobotsNpcProximityAction::TurnTowardPlayer {
            yaw_error_radians,
            max_turn_radians_per_second: ROBOTS_NPC_FACE_TURN_RATE_RADIANS_PER_SECOND,
            anim_mode: ROBOTS_ANIM_MODE_TURN_ON_SPOT,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flag2_test_nav<'a>(
        vertices: &'a [[f32; 3]],
        faces: &'a [[u32; 3]],
        adjacency: &'a [[Option<u32>; 3]],
        groups: &'a [super::super::monster_navigation::RobotsMonsterNavGroup],
    ) -> RobotsMonsterNavMeshView<'a> {
        RobotsMonsterNavMeshView {
            vertices,
            faces,
            adjacency,
            groups,
        }
    }

    #[test]
    fn npc_follow_network_path_config_matches_native_setup() {
        let config = npc_follow_network_path_config();
        assert_eq!(config.priority, ROBOTS_NPC_FOLLOW_NETWORK_PATH_PRIORITY);
        assert_eq!(config.priority, 0x0f);
        assert_eq!(config.target_locomotion_scalar.to_bits(), 0.1f32.to_bits());
    }

    #[test]
    fn npc_diner_profile_matches_native_modes_and_physics_bootstrap() {
        assert_eq!(ROBOTS_NPC_DINER_IDLE_PRIORITY, 2);
        assert_eq!(ROBOTS_NPC_DINER_IDLE_ANIM_MODE, 0x0900_00eb);
        assert_eq!(
            ROBOTS_NPC_DINER_PERIODIC_ANIM_MODES,
            [0x0900_00ec, 0x0900_00ed]
        );

        let mut physics = RobotsCharacterPhysicsRuntimeState::default();
        bootstrap_npc_diner_physics(&mut physics);
        assert!(physics.object_flag_bit0);
        assert!(physics.object_flag_bit2);
        assert!(physics.object_flag_bit4);
        assert_eq!(physics.object_flags_14c & 0x4, 0x4);
        assert!(!physics.handler_606);
    }

    #[test]
    fn flag2_route_rebuild_and_waypoint_traversal_match_native_contract() {
        use super::super::monster_navigation::RobotsMonsterNavGroup;

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
        let nav = flag2_test_nav(&vertices, &faces, &adjacency, &groups);
        let mut state = RobotsNpcFlag2RuntimeState::default();

        assert_eq!(npc_flag2_priority(true, false), 0x0B);
        assert_eq!(npc_flag2_priority(false, false), 1);
        assert!(npc_flag2_enter_needs_rebuild(&state, 0));
        assert!(npc_flag2_rebuild_will_consume_rng(&state));
        enter_npc_flag2(&mut state, nav, 0, 7, Some([1, 2, 3, 3, 1]));
        assert_eq!(state.route, vec![0, 1, 2, 3]);
        assert_eq!(state.route_index, 0);
        assert!(!state.reverse);
        assert_eq!(state.rebuild_cooldown_seconds, 0.25);
        assert_eq!(state.waypoint_xyz, Some([0.5, 0.0, 0.5]));

        let action = step_npc_flag2(&mut state, nav, [0.5, 0.0, 0.5], 0, 7, 0.0, None);
        assert_eq!(state.route_index, 1);
        assert!(matches!(
            action,
            RobotsNpcFlag2Action::SteerToWaypoint { .. }
        ));

        state.route_index = 2;
        state.reverse = false;
        state.waypoint_xyz = nav.shared_edge_midpoint(2, 3);
        let waypoint = state.waypoint_xyz.unwrap();
        let _ = step_npc_flag2(&mut state, nav, waypoint, 2, 7, 0.0, None);
        assert_eq!(state.route_index, 3);
        assert!(state.reverse);
        assert_eq!(state.waypoint_xyz, nav.face_center(3));
    }

    #[test]
    fn flag2_cooldown_suppresses_rng_rebuild_but_still_resets_route_cursor() {
        let mut state = RobotsNpcFlag2RuntimeState {
            route: vec![2, 3],
            route_index: 1,
            reverse: true,
            waypoint_xyz: Some([2.0, 0.0, 0.5]),
            rebuild_cooldown_seconds: 0.25,
        };
        let nav = flag2_test_nav(&[], &[], &[], &[]);
        let _ = step_npc_flag2(&mut state, nav, [0.0; 3], 2, 7, 1.0, None);
        assert_eq!(state.route_index, 0);
        assert!(!state.reverse);
        assert_eq!(state.rebuild_cooldown_seconds, 0.25);
        tick_npc_flag2(&mut state, 1.0);
        assert!(state.rebuild_cooldown_seconds < 0.25);
    }

    #[test]
    fn random_idle_delay_matches_native_300_tick_jitter_window() {
        assert_eq!(npc_random_idle_delay_from_draw(300, 0), 225);
        assert_eq!(npc_random_idle_delay_from_draw(300, 149), 374);
        assert_eq!(npc_random_idle_delay_from_draw(300, 150), 225);
    }

    #[test]
    fn random_idle_priority_arms_at_zero_then_waits_for_setup_idle_to_release() {
        let mut state = RobotsNpcRandomIdleRuntimeState::default();
        initialize_npc_random_idle(&mut state, 0);
        assert_eq!(state.delay_ticks, 225);
        assert_eq!(npc_random_idle_priority(&state), 1);
        state.delay_ticks = 0;
        assert_eq!(npc_random_idle_priority(&state), 0x16);

        assert_eq!(
            enter_npc_random_idle(&mut state, &ROBOTS_NPC_RANDOM_IDLE_ANIM_MODES[..2], 3, 149),
            Some(ROBOTS_ANIM_MODE_EXTENDED_IDLE2)
        );
        assert_eq!(state.delay_ticks, 374);
        assert_eq!(npc_random_idle_priority(&state), 0x16);
        complete_npc_random_idle_setup_idle(&mut state);
        assert_eq!(npc_random_idle_priority(&state), 1);
        leave_npc_random_idle(&mut state);
        tick_npc_random_idle(&mut state);
        assert_eq!(state.delay_ticks, 373);
    }

    #[test]
    fn random_idle_tick_does_not_consume_delay_while_active() {
        let mut state = RobotsNpcRandomIdleRuntimeState {
            node_active: true,
            completion_latch: false,
            delay_ticks: 300,
            selected_anim_mode: Some(ROBOTS_ANIM_MODE_EXTENDED_IDLE),
            initialized: true,
        };
        tick_npc_random_idle(&mut state);
        assert_eq!(state.delay_ticks, 300);
    }

    #[test]
    fn proximity_priority_uses_exact_3_5_and_5_0_hysteresis() {
        assert_eq!(
            npc_proximity_priority(false, true, 3.4999_f32.powi(2)),
            0x15
        );
        assert_eq!(npc_proximity_priority(false, true, 3.5_f32.powi(2)), 1);
        assert_eq!(npc_proximity_priority(true, true, 4.9999_f32.powi(2)), 0x15);
        assert_eq!(npc_proximity_priority(true, true, 5.0_f32.powi(2)), 1);
        assert_eq!(npc_proximity_priority(false, false, 0.0), 1);
    }

    #[test]
    fn aligned_state_enters_turning_only_above_40_degrees_when_allowed() {
        let mut state = RobotsNpcProximityBehaviorState::default();
        let action =
            step_npc_proximity_face_player(&mut state, true, ROBOTS_NPC_FACE_TURN_ENTER_RADIANS);
        assert_eq!(state.face_state, RobotsNpcFacePlayerState::Aligned);
        assert_eq!(
            action,
            RobotsNpcProximityAction::RequestAnimMode {
                anim_mode: ROBOTS_ANIM_MODE_IDLE_ATTACK
            }
        );

        let action = step_npc_proximity_face_player(
            &mut state,
            true,
            ROBOTS_NPC_FACE_TURN_ENTER_RADIANS + 0.0001,
        );
        assert_eq!(state.face_state, RobotsNpcFacePlayerState::Turning);
        assert!(matches!(
            action,
            RobotsNpcProximityAction::TurnTowardPlayer { .. }
        ));
    }

    #[test]
    fn owner_gate_blocks_aligned_to_turning_transition() {
        let mut state = RobotsNpcProximityBehaviorState::default();
        let action = step_npc_proximity_face_player(&mut state, false, 1.0);
        assert_eq!(state.face_state, RobotsNpcFacePlayerState::Aligned);
        assert!(matches!(
            action,
            RobotsNpcProximityAction::RequestAnimMode { .. }
        ));
    }

    #[test]
    fn turning_state_exits_only_strictly_below_10_degrees() {
        let mut state = RobotsNpcProximityBehaviorState {
            node_active: true,
            face_state: RobotsNpcFacePlayerState::Turning,
        };
        let action =
            step_npc_proximity_face_player(&mut state, true, ROBOTS_NPC_FACE_TURN_EXIT_RADIANS);
        assert_eq!(state.face_state, RobotsNpcFacePlayerState::Turning);
        assert!(matches!(
            action,
            RobotsNpcProximityAction::TurnTowardPlayer { .. }
        ));

        let action = step_npc_proximity_face_player(
            &mut state,
            true,
            ROBOTS_NPC_FACE_TURN_EXIT_RADIANS - 0.0001,
        );
        assert_eq!(state.face_state, RobotsNpcFacePlayerState::Aligned);
        assert_eq!(
            action,
            RobotsNpcProximityAction::RequestAnimMode {
                anim_mode: ROBOTS_ANIM_MODE_IDLE_ATTACK
            }
        );
    }
}
