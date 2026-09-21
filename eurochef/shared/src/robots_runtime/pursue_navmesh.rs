use serde::Serialize;

use super::{
    locomotion::{
        shortest_yaw_delta, step_ai_direct_turn_request, step_ai_locomotion_after_steering_prepass,
        RobotsAiDirectTurnStep, RobotsAiLocomotionInput, RobotsAiLocomotionRuntimeState,
        RobotsAiLocomotionStep, RobotsAiTurnRateInput, ROBOTS_ANIM_MODE_TURN_ON_SPOT,
    },
    monster_navigation::RobotsMonsterNavMeshView,
};

pub const ROBOTS_PURSUE_NAV_PRIORITY: u8 = 0x1f;
pub const ROBOTS_PURSUE_NAV_ROUTE_ARRIVAL_DISTANCE_SQUARED: f32 = 1.0;
pub const ROBOTS_PURSUE_NAV_DISTANCE_RAMP: f32 = 1.0;
pub const ROBOTS_PURSUE_NAV_ROUTED_SCALAR_MULTIPLIER: f32 = 0.75;
pub const ROBOTS_PURSUE_NAV_NO_STOP_DISTANCE: f32 = -100.0;
pub const ROBOTS_DOGBOT_PURSUE_NAV_STOP_DISTANCE: f32 = 5.0;
/// Native direct Monster factory `0x0047E8B0 -> 0x00443D40` allocates the XItem
/// through `0x004445A0`, which initializes XItem+0x154 creator to null. Therefore
/// `0x00451780` leaves Handler+0x605 at its ctor-zero value for these dynamic AI.
pub const ROBOTS_DIRECT_MONSTER_FACTORY_ALLOW_CROSS_GROUP: bool = false;
pub const ROBOTS_PURSUE_NAV_PRELUDE_YAW_EPSILON_RADIANS: f32 = f32::from_bits(0x3c8e_fa35);
pub const ROBOTS_PURSUE_NAV_PRELUDE_TURN_RATE_RADIANS_PER_SECOND: f32 = std::f32::consts::PI;
pub const ROBOTS_MALFBOT_PURSUE_NAV_PRELUDE_ANIM_MODE: u32 = 0x0900_0027;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub enum RobotsPursueNavMeshMode {
    #[default]
    Unavailable,
    Direct,
    Routed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub enum RobotsPursueNavMeshExecutePhase {
    PreludeTurn,
    PreludeAnimation,
    #[default]
    Moving,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RobotsPursueNavMeshRuntimeState {
    /// Inherited BehaviorNode+0x09. Native `0x00456F00` sets this on Enter
    /// and `0x00456F10` clears it on Leave. `0x0046DFB0` uses it to keep
    /// an unfinished cached route while this node remains the selector winner.
    pub active: bool,
    pub mode: RobotsPursueNavMeshMode,
    pub route_faces: Vec<u32>,
    pub route_index: usize,
    pub waypoint_xyz: Option<[f32; 3]>,
    pub target_face: Option<u32>,
    /// Native node+0x26. `0x0046DB60` sets it when the cached route reaches
    /// its final waypoint; the following refresh may then rebuild the route.
    pub route_completed: bool,
    pub execute_phase: RobotsPursueNavMeshExecutePhase,
    pub prelude_anim_mode: Option<u32>,
    pub locomotion: RobotsAiLocomotionRuntimeState,
}

impl Default for RobotsPursueNavMeshRuntimeState {
    fn default() -> Self {
        Self {
            active: false,
            mode: RobotsPursueNavMeshMode::Unavailable,
            route_faces: Vec::new(),
            route_index: 0,
            waypoint_xyz: None,
            target_face: None,
            route_completed: false,
            execute_phase: RobotsPursueNavMeshExecutePhase::Moving,
            prelude_anim_mode: None,
            locomotion: RobotsAiLocomotionRuntimeState::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsPursueNavMeshInput {
    pub owner_position_xyz: [f32; 3],
    pub owner_yaw_radians: f32,
    pub current_face: u32,
    pub target_position_xyz: [f32; 3],
    pub handler_flags_628: u32,
    pub move_mode_active_on_entry: bool,
    pub runtime_rate_scale: f32,
    /// Handler+0x605, populated from the creator vslot +0x104 by `0x00451780`.
    /// Zero keeps native route/direct tests inside the current NavMesh group.
    pub allow_cross_group: bool,
    /// AI_PursueNavMesh node+0x5C. DogBot builder stores exactly 5.0.
    pub stop_distance: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsPursueNavMeshStep {
    pub mode: RobotsPursueNavMeshMode,
    pub target_face: Option<u32>,
    pub waypoint_xyz: Option<[f32; 3]>,
    pub target_yaw_radians: Option<f32>,
    pub target_locomotion_scalar: f32,
    pub direct_turn: Option<RobotsAiDirectTurnStep>,
    pub requested_anim_mode: Option<u32>,
    pub locomotion: Option<RobotsAiLocomotionStep>,
}

fn distance_squared(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let dz = b[2] - a[2];
    dx * dx + dy * dy + dz * dz
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    distance_squared(a, b).max(0.0).sqrt()
}

fn target_scalar(owner: [f32; 3], target: [f32; 3], stop_distance: f32) -> f32 {
    if stop_distance <= ROBOTS_PURSUE_NAV_NO_STOP_DISTANCE {
        return 1.0;
    }
    ((distance(owner, target) - stop_distance) / ROBOTS_PURSUE_NAV_DISTANCE_RAMP).clamp(0.0, 1.0)
}

impl RobotsPursueNavMeshRuntimeState {
    /// Native enter `0x0046E0E0`. A nonzero node+0x54 inserts the exact
    /// TurnToTarget -> requested AnimMode -> SetupIdle prelude before movement.
    pub fn enter(&mut self, prelude_anim_mode: Option<u32>) {
        self.active = true;
        self.route_completed = false;
        self.prelude_anim_mode = prelude_anim_mode;
        self.execute_phase = if prelude_anim_mode.is_some() {
            RobotsPursueNavMeshExecutePhase::PreludeTurn
        } else {
            RobotsPursueNavMeshExecutePhase::Moving
        };
    }

    /// Native event `0x0046E120`: SetupIdle advances state2 -> state3.
    pub fn setup_idle(&mut self) {
        if self.execute_phase == RobotsPursueNavMeshExecutePhase::PreludeAnimation {
            self.execute_phase = RobotsPursueNavMeshExecutePhase::Moving;
        }
    }

    pub fn leave(&mut self) {
        self.active = false;
        self.prelude_anim_mode = None;
    }

    /// Host seam for native node vslot +0x2C (`0x0046DFB0`). This updates the
    /// direct-path bit every time it is serviced, but preserves an unchanged live
    /// route instead of manufacturing per-frame ownership inside the execute path.
    pub fn refresh_navigation(
        &mut self,
        nav: RobotsMonsterNavMeshView<'_>,
        input: RobotsPursueNavMeshInput,
    ) -> bool {
        if !input
            .owner_position_xyz
            .iter()
            .all(|value| value.is_finite())
            || !input
                .target_position_xyz
                .iter()
                .all(|value| value.is_finite())
            || input.current_face as usize >= nav.faces.len()
        {
            self.mode = RobotsPursueNavMeshMode::Unavailable;
            self.route_faces.clear();
            self.waypoint_xyz = None;
            self.target_face = None;
            return false;
        }

        if nav.direct_segment_reaches_target(
            input.owner_position_xyz,
            input.target_position_xyz,
            input.current_face,
            input.allow_cross_group,
        ) {
            self.mode = RobotsPursueNavMeshMode::Direct;
            // Native 0x0046DFB0 sets +0x24 and returns immediately. Cached
            // +0x25/+0x28..+0x50 route ownership deliberately survives.
            return true;
        }

        // BehaviorNode+0x09 is the inherited active flag. While an active
        // route has not reached +0x26 completion, native does not rebuild it.
        if self.active && !self.route_completed {
            let route_ready = !self.route_faces.is_empty()
                && self.route_index < self.route_faces.len()
                && self.waypoint_xyz.is_some();
            self.mode = if route_ready {
                RobotsPursueNavMeshMode::Routed
            } else {
                RobotsPursueNavMeshMode::Unavailable
            };
            return route_ready;
        }

        // Native clears +0x25 and resets route index before attempting the
        // next route build.
        self.route_faces.clear();
        self.route_index = 0;
        self.waypoint_xyz = None;

        let Some(target_face) = nav.find_face(input.target_position_xyz) else {
            self.mode = RobotsPursueNavMeshMode::Unavailable;
            self.route_faces.clear();
            self.route_index = 0;
            self.waypoint_xyz = None;
            self.target_face = None;
            return false;
        };

        let route_still_usable = self.mode == RobotsPursueNavMeshMode::Routed
            && self.target_face == Some(target_face)
            && self.route_index < self.route_faces.len();
        if route_still_usable {
            return true;
        }

        let Some(route_faces) = nav.find_face_route(input.current_face, target_face) else {
            self.mode = RobotsPursueNavMeshMode::Unavailable;
            self.route_faces.clear();
            self.route_index = 0;
            self.waypoint_xyz = None;
            self.target_face = Some(target_face);
            return false;
        };
        let waypoint_xyz = route_faces.first().and_then(|face| nav.face_center(*face));
        if waypoint_xyz.is_none() {
            self.mode = RobotsPursueNavMeshMode::Unavailable;
            self.route_faces.clear();
            self.route_index = 0;
            self.waypoint_xyz = None;
            self.target_face = Some(target_face);
            return false;
        }

        self.mode = RobotsPursueNavMeshMode::Routed;
        self.route_faces = route_faces;
        self.route_index = 0;
        self.waypoint_xyz = waypoint_xyz;
        self.target_face = Some(target_face);
        self.route_completed = false;
        true
    }

    /// Native route/steering half of `0x00457400 -> 0x0046DB60 -> 0x0046DCA0`.
    /// This leaves Handler locomotion ownership to the caller, which matters for
    /// handlers such as MalfBot where several behavior nodes share +0x5D0/+0x5F9.
    pub fn step_steering(
        &mut self,
        nav: RobotsMonsterNavMeshView<'_>,
        input: RobotsPursueNavMeshInput,
    ) -> RobotsPursueNavMeshStep {
        let target_dx = input.target_position_xyz[0] - input.owner_position_xyz[0];
        let target_dz = input.target_position_xyz[2] - input.owner_position_xyz[2];
        let target_yaw = target_dx.atan2(target_dz);
        if self.execute_phase == RobotsPursueNavMeshExecutePhase::PreludeTurn {
            let yaw_error = shortest_yaw_delta(input.owner_yaw_radians, target_yaw);
            if yaw_error.abs() < ROBOTS_PURSUE_NAV_PRELUDE_YAW_EPSILON_RADIANS {
                self.execute_phase = RobotsPursueNavMeshExecutePhase::PreludeAnimation;
            } else {
                return RobotsPursueNavMeshStep {
                    mode: self.mode,
                    target_face: self.target_face,
                    waypoint_xyz: self.waypoint_xyz,
                    target_yaw_radians: Some(target_yaw),
                    target_locomotion_scalar: 0.0,
                    direct_turn: Some(step_ai_direct_turn_request(
                        input.owner_yaw_radians,
                        yaw_error,
                        ROBOTS_PURSUE_NAV_PRELUDE_TURN_RATE_RADIANS_PER_SECOND,
                        input.handler_flags_628,
                        ROBOTS_ANIM_MODE_TURN_ON_SPOT,
                        input.runtime_rate_scale,
                    )),
                    requested_anim_mode: None,
                    locomotion: None,
                };
            }
        }
        if self.execute_phase == RobotsPursueNavMeshExecutePhase::PreludeAnimation {
            return RobotsPursueNavMeshStep {
                mode: self.mode,
                target_face: self.target_face,
                waypoint_xyz: self.waypoint_xyz,
                target_yaw_radians: Some(target_yaw),
                target_locomotion_scalar: 0.0,
                direct_turn: None,
                requested_anim_mode: self.prelude_anim_mode,
                locomotion: None,
            };
        }

        if self.mode == RobotsPursueNavMeshMode::Routed {
            if let Some(waypoint) = self.waypoint_xyz {
                if distance_squared(input.owner_position_xyz, waypoint)
                    < ROBOTS_PURSUE_NAV_ROUTE_ARRIVAL_DISTANCE_SQUARED
                {
                    self.route_index = self.route_index.saturating_add(1);
                    if self.route_index >= self.route_faces.len() {
                        // Native +0x26 is a completion latch. Route storage and
                        // +0x25 remain live until the next 0x0046DFB0 rebuild.
                        self.route_completed = true;
                        self.mode = RobotsPursueNavMeshMode::Direct;
                    } else {
                        self.waypoint_xyz = nav.route_waypoint(&self.route_faces, self.route_index);
                    }
                }
            }
        }

        let steering_target = match self.mode {
            RobotsPursueNavMeshMode::Direct => Some(input.target_position_xyz),
            RobotsPursueNavMeshMode::Routed => self.waypoint_xyz,
            RobotsPursueNavMeshMode::Unavailable => None,
        };
        let Some(steering_target) = steering_target else {
            return RobotsPursueNavMeshStep {
                mode: self.mode,
                target_face: self.target_face,
                waypoint_xyz: self.waypoint_xyz,
                target_yaw_radians: None,
                target_locomotion_scalar: 0.0,
                direct_turn: None,
                requested_anim_mode: None,
                locomotion: None,
            };
        };

        let dx = steering_target[0] - input.owner_position_xyz[0];
        let dz = steering_target[2] - input.owner_position_xyz[2];
        let target_yaw_radians = dx.atan2(dz);
        let mut target_locomotion_scalar = target_scalar(
            input.owner_position_xyz,
            input.target_position_xyz,
            input.stop_distance,
        );
        if self.mode == RobotsPursueNavMeshMode::Routed {
            target_locomotion_scalar *= ROBOTS_PURSUE_NAV_ROUTED_SCALAR_MULTIPLIER;
        }

        RobotsPursueNavMeshStep {
            mode: self.mode,
            target_face: self.target_face,
            waypoint_xyz: self.waypoint_xyz,
            target_yaw_radians: Some(target_yaw_radians),
            target_locomotion_scalar,
            direct_turn: None,
            requested_anim_mode: None,
            locomotion: None,
        }
    }

    /// Compatibility wrapper used by handlers whose only locomotion-owning node
    /// is PursueNavMesh (currently DogBot). Multi-movement-node handlers call
    /// `step_steering` and feed the intent through their one Handler-owned state.
    pub fn step_execute(
        &mut self,
        nav: RobotsMonsterNavMeshView<'_>,
        input: RobotsPursueNavMeshInput,
    ) -> RobotsPursueNavMeshStep {
        let mut step = self.step_steering(nav, input);
        if step.direct_turn.is_some() || step.requested_anim_mode.is_some() {
            return step;
        }
        let Some(target_yaw_radians) = step.target_yaw_radians else {
            return step;
        };
        step.locomotion = Some(step_ai_locomotion_after_steering_prepass(
            &mut self.locomotion,
            RobotsAiLocomotionInput {
                steering_target_yaw_radians: target_yaw_radians,
                target_locomotion_scalar: step.target_locomotion_scalar,
                turn_rate: RobotsAiTurnRateInput::Default,
                handler_flags_628: input.handler_flags_628,
                move_mode_active_on_entry: input.move_mode_active_on_entry,
                current_owner_yaw_radians: input.owner_yaw_radians,
                runtime_rate_scale: input.runtime_rate_scale,
            },
        ));
        step
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::robots_runtime::monster_navigation::{
        RobotsMonsterNavGroup, RobotsMonsterNavMeshView,
    };

    fn nav<'a>(
        vertices: &'a [[f32; 3]],
        faces: &'a [[u32; 3]],
        adjacency: &'a [[Option<u32>; 3]],
    ) -> RobotsMonsterNavMeshView<'a> {
        RobotsMonsterNavMeshView {
            vertices,
            faces,
            adjacency,
            groups: &[] as &[RobotsMonsterNavGroup],
        }
    }

    fn input(owner: [f32; 3], target: [f32; 3], current_face: u32) -> RobotsPursueNavMeshInput {
        RobotsPursueNavMeshInput {
            owner_position_xyz: owner,
            owner_yaw_radians: 0.0,
            current_face,
            target_position_xyz: target,
            handler_flags_628: 0,
            move_mode_active_on_entry: true,
            runtime_rate_scale: 1.0,
            allow_cross_group: true,
            stop_distance: ROBOTS_DOGBOT_PURSUE_NAV_STOP_DISTANCE,
        }
    }

    #[test]
    fn dogbot_scalar_matches_native_five_unit_stop_and_routed_three_quarter_cap() {
        assert_eq!(target_scalar([0.0, 0.0, 0.0], [0.0, 0.0, 4.0], 5.0), 0.0);
        assert!((target_scalar([0.0, 0.0, 0.0], [0.0, 0.0, 5.5], 5.0) - 0.5).abs() < 1.0e-6);
        assert_eq!(target_scalar([0.0, 0.0, 0.0], [0.0, 0.0, 7.0], 5.0), 1.0);
    }

    #[test]
    fn direct_pursuit_targets_player_and_common_move_locomotion() {
        let vertices = [[0.0, 0.0, 0.0], [8.0, 0.0, 0.0], [0.0, 0.0, 8.0]];
        let faces = [[0, 1, 2]];
        let adjacency = [[None, None, None]];
        let nav = nav(&vertices, &faces, &adjacency);
        let mut state = RobotsPursueNavMeshRuntimeState::default();
        let input = input([0.2, 0.0, 0.2], [0.2, 0.0, 6.2], 0);
        assert!(state.refresh_navigation(nav, input));
        assert_eq!(state.mode, RobotsPursueNavMeshMode::Direct);
        let step = state.step_execute(nav, input);
        assert_eq!(step.mode, RobotsPursueNavMeshMode::Direct);
        assert_eq!(step.target_yaw_radians, Some(0.0));
        assert_eq!(step.target_locomotion_scalar, 1.0);
        assert!(step.locomotion.is_some());
    }

    #[test]
    fn active_pursuit_preserves_native_cached_route_until_leave_or_completion() {
        let vertices = [[0.0, 0.0, 0.0], [8.0, 0.0, 0.0], [0.0, 0.0, 8.0]];
        let faces = [[0, 1, 2]];
        let adjacency = [[None, None, None]];
        let nav = nav(&vertices, &faces, &adjacency);
        let mut state = RobotsPursueNavMeshRuntimeState::default();
        state.route_faces = vec![0];
        state.route_index = 0;
        state.waypoint_xyz = Some([1.0, 0.0, 1.0]);
        state.target_face = Some(0);
        state.enter(None);

        // 0x0046DFB0: direct success sets +0x24 and returns without touching
        // the cached +0x25/+0x28..+0x50 route.
        let direct = input([0.2, 0.0, 0.2], [0.2, 0.0, 6.2], 0);
        assert!(state.refresh_navigation(nav, direct));
        assert_eq!(state.mode, RobotsPursueNavMeshMode::Direct);
        assert_eq!(state.route_faces, vec![0]);
        assert_eq!(state.waypoint_xyz, Some([1.0, 0.0, 1.0]));

        // Once direct reachability disappears, active +0x09 with clear +0x26
        // reuses the unfinished cached route instead of rebuilding it.
        let blocked = input([0.2, 0.0, 0.2], [20.0, 0.0, 20.0], 0);
        assert!(state.refresh_navigation(nav, blocked));
        assert_eq!(state.mode, RobotsPursueNavMeshMode::Routed);
        assert_eq!(state.route_faces, vec![0]);

        // Native Leave clears +0x09. The same refresh is now free to discard
        // the stale cache and fails because the target is outside this mesh.
        state.leave();
        assert!(!state.refresh_navigation(nav, blocked));
        assert_eq!(state.mode, RobotsPursueNavMeshMode::Unavailable);
        assert!(state.route_faces.is_empty());
    }

    #[test]
    fn routed_pursuit_advances_from_start_face_center_to_shared_edge() {
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
        let nav = nav(&vertices, &faces, &adjacency);
        let mut state = RobotsPursueNavMeshRuntimeState::default();
        state.mode = RobotsPursueNavMeshMode::Routed;
        state.route_faces = vec![0, 1, 2, 3];
        state.route_index = 0;
        state.target_face = Some(3);
        state.waypoint_xyz = nav.face_center(0);
        let owner = state.waypoint_xyz.expect("start center");
        let step = state.step_execute(nav, input(owner, [1.8, 0.0, 0.8], 0));
        assert_eq!(state.route_index, 1);
        assert_eq!(step.mode, RobotsPursueNavMeshMode::Routed);
        assert_eq!(step.waypoint_xyz, nav.shared_edge_midpoint(1, 2));
        assert!(step.target_locomotion_scalar <= ROBOTS_PURSUE_NAV_ROUTED_SCALAR_MULTIPLIER);
    }

    #[test]
    fn malfbot_pursuit_turns_with_pi_rate_then_plays_prelude_before_move() {
        let vertices = [[0.0, 0.0, 0.0], [8.0, 0.0, 0.0], [0.0, 0.0, 8.0]];
        let faces = [[0, 1, 2]];
        let adjacency = [[None, None, None]];
        let nav = nav(&vertices, &faces, &adjacency);
        let mut state = RobotsPursueNavMeshRuntimeState::default();
        let mut input = input([0.2, 0.0, 0.2], [6.2, 0.0, 0.2], 0);
        input.stop_distance = ROBOTS_PURSUE_NAV_NO_STOP_DISTANCE;
        assert!(state.refresh_navigation(nav, input));
        state.enter(Some(ROBOTS_MALFBOT_PURSUE_NAV_PRELUDE_ANIM_MODE));

        let turn = state.step_execute(nav, input);
        let direct = turn.direct_turn.expect("native prelude direct turn");
        assert_eq!(direct.requested_anim_mode, ROBOTS_ANIM_MODE_TURN_ON_SPOT);
        assert!((direct.owner_yaw_radians - std::f32::consts::PI / 60.0).abs() < 1.0e-6);

        input.owner_yaw_radians = std::f32::consts::FRAC_PI_2;
        let prelude = state.step_execute(nav, input);
        assert_eq!(prelude.direct_turn, None);
        assert_eq!(
            prelude.requested_anim_mode,
            Some(ROBOTS_MALFBOT_PURSUE_NAV_PRELUDE_ANIM_MODE)
        );
        assert_eq!(
            state.execute_phase,
            RobotsPursueNavMeshExecutePhase::PreludeAnimation
        );

        state.setup_idle();
        let moving = state.step_execute(nav, input);
        assert!(moving.locomotion.is_some());
        assert_eq!(state.execute_phase, RobotsPursueNavMeshExecutePhase::Moving);
    }
}
