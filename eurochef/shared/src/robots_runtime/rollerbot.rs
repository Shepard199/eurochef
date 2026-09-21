use serde::Serialize;

use super::{
    ai_character::RobotsAiPatrolConfig,
    ai_pursue::{ai_pursue_priority, step_ai_pursue, RobotsAiPursueConfig},
    generic_attack::RobotsGenericAttackConfig,
    locomotion::{
        step_accel_damped_locomotion, RobotsAiTurnRateInput, RobotsLocomotionState,
        ROBOTS_FIXED_STEP_SECONDS, ROBOTS_ROLLERBOT_FORWARD_ACCELERATION,
        ROBOTS_ROLLERBOT_VELOCITY_DAMPING,
    },
};

pub const ROBOTS_ROLLERBOT_PATH_UID: u32 = 0x0B00_0049;
pub const ROBOTS_ROLLERBOT_PATROL_PRIORITY: u8 = 10;
pub const ROBOTS_ROLLERBOT_PURSUE_PRIORITY: u8 = 0x1E;
pub const ROBOTS_ROLLERBOT_FOLLOW_PATH_PRIORITY: u8 = 0x1F;
pub const ROBOTS_ROLLERBOT_ATTACK_PRIORITY: u8 = 0x32;
pub const ROBOTS_ROLLERBOT_HIT_PRIORITY: u8 = 100;
pub const ROBOTS_ROLLERBOT_FATAL_PRIORITY: u8 = 200;

pub const ROBOTS_ROLLERBOT_PURSUE_RADIUS: f32 = 60.0;
pub const ROBOTS_ROLLERBOT_ATTACK_ANIM_MODE: u32 = 0x0900_0027;
pub const ROBOTS_ROLLERBOT_ATTACK_OUTER_RADIUS: f32 = 3.0;
pub const ROBOTS_ROLLERBOT_ATTACK_YAW_TOLERANCE_RADIANS: f32 = std::f32::consts::PI / 12.0;
pub const ROBOTS_ROLLERBOT_ATTACK_VERTICAL_LIMIT: f32 = 1.0;
pub const ROBOTS_ROLLERBOT_ATTACK_REENTRY_TICKS: u32 = 30;
pub const ROBOTS_ROLLERBOT_TURN_RATE_RADIANS_PER_SECOND: f32 = f32::from_bits(0x4116_CBE4);
pub const ROBOTS_ROLLERBOT_PATH_ARRIVAL_RADIUS: f32 = 1.0;

/// `0x00462C80 -> AI_Patrol::Configure`.
pub const fn rollerbot_patrol_config() -> RobotsAiPatrolConfig {
    RobotsAiPatrolConfig {
        base_yaw_radians: 0.0,
        interval_seconds: 1,
        target_locomotion_scalar: 0.1,
        turn_rate: RobotsAiTurnRateInput::Default,
    }
}

/// `0x00462C80 -> AI_Attack::Configure` plus the explicit base-node +0x18 write.
pub const fn rollerbot_attack_config() -> RobotsGenericAttackConfig {
    RobotsGenericAttackConfig {
        primary_anim_mode: ROBOTS_ROLLERBOT_ATTACK_ANIM_MODE,
        secondary_anim_mode: None,
        inner_radius: 0.0,
        outer_radius: ROBOTS_ROLLERBOT_ATTACK_OUTER_RADIUS,
        yaw_tolerance_radians: ROBOTS_ROLLERBOT_ATTACK_YAW_TOLERANCE_RADIANS,
        vertical_limit: ROBOTS_ROLLERBOT_ATTACK_VERTICAL_LIMIT,
        reentry_delay_ticks: ROBOTS_ROLLERBOT_ATTACK_REENTRY_TICKS,
        secondary_repeat_count: 0,
        sticky_while_active: true,
        priority: ROBOTS_ROLLERBOT_ATTACK_PRIORITY,
    }
}

/// Native `AI_Pursue` gate `0x0046D950`. The executable uses the full XYZ
/// squared distance and accepts the boundary exactly at 60 units.
pub fn rollerbot_pursue_priority(
    owner_position_xyz: [f32; 3],
    target_position_xyz: Option<[f32; 3]>,
) -> u8 {
    ai_pursue_priority(
        RobotsAiPursueConfig {
            radius: ROBOTS_ROLLERBOT_PURSUE_RADIUS,
            locomotion_scalar: 20.0,
            priority: ROBOTS_ROLLERBOT_PURSUE_PRIORITY,
        },
        owner_position_xyz,
        target_position_xyz,
    )
}

pub fn rollerbot_target_yaw(owner_position_xyz: [f32; 3], target_position_xyz: [f32; 3]) -> f32 {
    step_ai_pursue(
        RobotsAiPursueConfig {
            radius: ROBOTS_ROLLERBOT_PURSUE_RADIUS,
            locomotion_scalar: 20.0,
            priority: ROBOTS_ROLLERBOT_PURSUE_PRIORITY,
        },
        owner_position_xyz,
        target_position_xyz,
    )
    .target_yaw_radians
}

/// Exact Roller `+0x10C/+0x168` movement policy. The scalar passed by Patrol,
/// Pursue or FollowNetworkPath is not a world speed for this class: Roller uses
/// Handler+0x664 = 10 as an acceleration-like lane, then damps retained motion.
pub fn step_rollerbot_locomotion(state: &mut RobotsLocomotionState, target_yaw_radians: f32) {
    step_accel_damped_locomotion(
        state,
        target_yaw_radians,
        ROBOTS_ROLLERBOT_TURN_RATE_RADIANS_PER_SECOND,
        ROBOTS_ROLLERBOT_FORWARD_ACCELERATION,
        ROBOTS_ROLLERBOT_VELOCITY_DAMPING,
        ROBOTS_FIXED_STEP_SECONDS,
    );
}

/// Roller main update `0x00467F80` damps and integrates retained motion even when
/// the active behavior does not issue a +0x10C movement command (Attack/Hit/etc.).
pub fn step_rollerbot_inertia(state: &mut RobotsLocomotionState) {
    step_accel_damped_locomotion(
        state,
        state.yaw_radians,
        0.0,
        0.0,
        ROBOTS_ROLLERBOT_VELOCITY_DAMPING,
        ROBOTS_FIXED_STEP_SECONDS,
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RobotsRollerBotBehaviorWinner {
    Patrol,
    Pursue,
    Attack,
    Hit,
    HitFatal,
    FollowNetworkPath,
}

/// Native builder insertion order from `0x00462C80`, combined with selector
/// `0x00457140`'s strict-greater replacement rule. Keeping the order explicit is
/// important if a later recovered gate produces an equal priority.
pub fn rollerbot_behavior_winner(
    patrol_priority: u8,
    pursue_priority: u8,
    attack_priority: u8,
    hit_priority: u8,
    fatal_priority: u8,
    follow_path_priority: u8,
) -> Option<RobotsRollerBotBehaviorWinner> {
    let mut best_priority = 1u8;
    let mut best = None;
    for (priority, candidate) in [
        (patrol_priority, RobotsRollerBotBehaviorWinner::Patrol),
        (pursue_priority, RobotsRollerBotBehaviorWinner::Pursue),
        (attack_priority, RobotsRollerBotBehaviorWinner::Attack),
        (hit_priority, RobotsRollerBotBehaviorWinner::Hit),
        (fatal_priority, RobotsRollerBotBehaviorWinner::HitFatal),
        (
            follow_path_priority,
            RobotsRollerBotBehaviorWinner::FollowNetworkPath,
        ),
    ] {
        if priority > best_priority {
            best_priority = priority;
            best = Some(candidate);
        }
    }
    best
}

#[derive(Debug, Clone, Copy)]
pub struct RobotsRollerBotPathGraphView<'a> {
    pub node_positions: &'a [[f32; 3]],
    /// Native `0x00420A40` walks serialized links in storage order. Preserve it.
    pub links: &'a [(usize, usize)],
}

impl<'a> RobotsRollerBotPathGraphView<'a> {
    pub fn nearest_node(self, point: [f32; 3]) -> Option<usize> {
        let mut best_index = None;
        let mut best_distance_squared = f32::INFINITY;
        for (index, node) in self.node_positions.iter().enumerate() {
            let dx = point[0] - node[0];
            let dy = point[1] - node[1];
            let dz = point[2] - node[2];
            let distance_squared = dx * dx + dy * dy + dz * dz;
            // Native `0x0041FCE0` uses strict less-than, so equal distances keep
            // the first serialized node.
            if distance_squared < best_distance_squared {
                best_distance_squared = distance_squared;
                best_index = Some(index);
            }
        }
        best_index
    }

    pub fn neighbors(self, node_index: usize) -> Vec<usize> {
        self.links
            .iter()
            .filter_map(|&(a, b)| {
                if a == node_index {
                    Some(b)
                } else if b == node_index {
                    Some(a)
                } else {
                    None
                }
            })
            .filter(|index| *index < self.node_positions.len())
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsRollerBotFollowPathRuntimeState {
    /// Builder `2 + FUN_00509C48()%3`, stored at node+0x98.
    pub completion_arrivals: u32,
    /// Node+0x9C.
    pub arrivals_completed: u32,
    pub current_node: Option<usize>,
    pub previous_node: Option<usize>,
    pub target_position_xyz: [f32; 3],
    pub entered: bool,
    /// `0x0046C430` always consumes this draw before nearest-node replacement.
    pub last_entry_random_bit: Option<u8>,
}

impl RobotsRollerBotFollowPathRuntimeState {
    pub fn from_setup_draw(setup_draw: u32) -> Self {
        Self {
            completion_arrivals: 2 + setup_draw % 3,
            arrivals_completed: 0,
            current_node: None,
            previous_node: None,
            target_position_xyz: [0.0; 3],
            entered: false,
            last_entry_random_bit: None,
        }
    }

    /// `0x0046C4E0` returns the node's configured priority while +0x9C < +0x98.
    pub fn priority(self) -> u8 {
        if self.arrivals_completed < self.completion_arrivals {
            ROBOTS_ROLLERBOT_FOLLOW_PATH_PRIORITY
        } else {
            1
        }
    }

    /// Native enter `0x0046C430`: consume `draw&1`, resolve that temporary node,
    /// then replace it with the nearest node when `0x0041FDC0` succeeds. A valid
    /// non-empty graph therefore always ends on the nearest serialized node.
    pub fn enter(
        &mut self,
        graph: RobotsRollerBotPathGraphView<'_>,
        owner_position_xyz: [f32; 3],
        entry_draw: u32,
    ) -> bool {
        self.last_entry_random_bit = Some((entry_draw & 1) as u8);
        let Some(node_index) = graph.nearest_node(owner_position_xyz) else {
            self.entered = false;
            self.current_node = None;
            return false;
        };
        self.current_node = Some(node_index);
        self.previous_node = None;
        self.target_position_xyz = graph.node_positions[node_index];
        self.entered = true;
        true
    }

    pub fn leave(&mut self) {
        self.entered = false;
    }

    pub fn needs_neighbor_rng(
        self,
        graph: RobotsRollerBotPathGraphView<'_>,
        owner_position_xyz: [f32; 3],
    ) -> bool {
        if !self.entered || !self.at_target(owner_position_xyz) {
            return false;
        }
        self.current_node
            .is_some_and(|node| !graph.neighbors(node).is_empty())
    }

    fn at_target(self, owner_position_xyz: [f32; 3]) -> bool {
        let dx = owner_position_xyz[0] - self.target_position_xyz[0];
        let dy = owner_position_xyz[1] - self.target_position_xyz[1];
        let dz = owner_position_xyz[2] - self.target_position_xyz[2];
        let distance_squared = dx * dx + dy * dy + dz * dz;
        distance_squared
            <= ROBOTS_ROLLERBOT_PATH_ARRIVAL_RADIUS * ROBOTS_ROLLERBOT_PATH_ARRIVAL_RADIUS
    }

    /// `0x0046BFA0 -> 0x0046C490`. If an arrived node has neighbors, native must
    /// consume one gameplay RNG draw before the state mutates. Passing `None` in
    /// that case is a transactional fail-closed preview.
    pub fn step(
        &mut self,
        graph: RobotsRollerBotPathGraphView<'_>,
        owner_position_xyz: [f32; 3],
        neighbor_draw: Option<u32>,
    ) -> Option<RobotsRollerBotFollowPathStep> {
        if !self.entered {
            return None;
        }

        let mut arrived = false;
        if self.at_target(owner_position_xyz) {
            let current_node = self.current_node?;
            let neighbors = graph.neighbors(current_node);
            if !neighbors.is_empty() && neighbor_draw.is_none() {
                return Some(RobotsRollerBotFollowPathStep {
                    target_position_xyz: self.target_position_xyz,
                    target_yaw_radians: rollerbot_target_yaw(
                        owner_position_xyz,
                        self.target_position_xyz,
                    ),
                    arrived: false,
                    needs_neighbor_rng: true,
                    arrivals_completed: self.arrivals_completed,
                    completed: self.arrivals_completed >= self.completion_arrivals,
                });
            }

            if let Some(draw) = neighbor_draw.filter(|_| !neighbors.is_empty()) {
                let next_node = neighbors[(draw as usize) % neighbors.len()];
                self.previous_node = self.current_node;
                self.current_node = Some(next_node);
                self.target_position_xyz = graph.node_positions[next_node];
            }
            self.arrivals_completed = self.arrivals_completed.wrapping_add(1);
            arrived = true;
        }

        Some(RobotsRollerBotFollowPathStep {
            target_position_xyz: self.target_position_xyz,
            target_yaw_radians: rollerbot_target_yaw(owner_position_xyz, self.target_position_xyz),
            arrived,
            needs_neighbor_rng: false,
            arrivals_completed: self.arrivals_completed,
            completed: self.arrivals_completed >= self.completion_arrivals,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsRollerBotFollowPathStep {
    pub target_position_xyz: [f32; 3],
    pub target_yaw_radians: f32,
    pub arrived: bool,
    pub needs_neighbor_rng: bool,
    pub arrivals_completed: u32,
    pub completed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph<'a>(
        nodes: &'a [[f32; 3]],
        links: &'a [(usize, usize)],
    ) -> RobotsRollerBotPathGraphView<'a> {
        RobotsRollerBotPathGraphView {
            node_positions: nodes,
            links,
        }
    }

    #[test]
    fn builder_constants_match_recovered_roller_nodes() {
        let patrol = rollerbot_patrol_config();
        assert_eq!(patrol.interval_seconds, 1);
        assert_eq!(patrol.target_locomotion_scalar, 0.1);

        let attack = rollerbot_attack_config();
        assert_eq!(attack.primary_anim_mode, 0x0900_0027);
        assert_eq!(attack.outer_radius, 3.0);
        assert_eq!(attack.reentry_delay_ticks, 30);
        assert_eq!(attack.priority, 0x32);
        assert!(attack.sticky_while_active);

        assert_eq!(
            RobotsRollerBotFollowPathRuntimeState::from_setup_draw(0).completion_arrivals,
            2
        );
        assert_eq!(
            RobotsRollerBotFollowPathRuntimeState::from_setup_draw(1).completion_arrivals,
            3
        );
        assert_eq!(
            RobotsRollerBotFollowPathRuntimeState::from_setup_draw(2).completion_arrivals,
            4
        );
    }

    #[test]
    fn follow_path_priority_is_final_builder_31_not_ctor_default_15() {
        let state = RobotsRollerBotFollowPathRuntimeState::from_setup_draw(0);
        assert_eq!(state.priority(), 0x1f);
        assert!(state.priority() > ROBOTS_ROLLERBOT_PURSUE_PRIORITY);
    }

    #[test]
    fn nearest_node_keeps_first_serialized_tie_and_enter_consumes_bit_but_replaces_it() {
        let nodes = [[-1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [10.0, 0.0, 0.0]];
        let links = [(0, 2), (1, 2)];
        let graph = graph(&nodes, &links);
        assert_eq!(graph.nearest_node([0.0, 0.0, 0.0]), Some(0));

        let mut state = RobotsRollerBotFollowPathRuntimeState::from_setup_draw(0);
        assert!(state.enter(graph, [0.0, 0.0, 0.0], 7));
        assert_eq!(state.last_entry_random_bit, Some(1));
        assert_eq!(state.current_node, Some(0));
        assert_eq!(state.target_position_xyz, nodes[0]);
    }

    #[test]
    fn neighbor_selection_preserves_serialized_edge_order_and_is_transactional_without_rng() {
        let nodes = [[0.0, 0.0, 0.0], [4.0, 0.0, 0.0], [8.0, 0.0, 0.0]];
        let links = [(0, 2), (0, 1)];
        let graph = graph(&nodes, &links);
        assert_eq!(graph.neighbors(0), vec![2, 1]);

        let mut state = RobotsRollerBotFollowPathRuntimeState::from_setup_draw(0);
        assert!(state.enter(graph, nodes[0], 0));
        let before = state;
        let preview = state.step(graph, nodes[0], None).expect("preview");
        assert!(preview.needs_neighbor_rng);
        assert_eq!(state, before);

        let committed = state.step(graph, nodes[0], Some(1)).expect("commit");
        assert!(committed.arrived);
        assert_eq!(state.current_node, Some(1));
        assert_eq!(state.arrivals_completed, 1);
    }

    #[test]
    fn selector_uses_native_strict_greater_order_and_path_preempts_pursue_until_complete() {
        assert_eq!(
            rollerbot_behavior_winner(10, 30, 1, 1, 1, 31),
            Some(RobotsRollerBotBehaviorWinner::FollowNetworkPath)
        );
        assert_eq!(
            rollerbot_behavior_winner(10, 30, 50, 1, 1, 31),
            Some(RobotsRollerBotBehaviorWinner::Attack)
        );
        assert_eq!(
            rollerbot_behavior_winner(10, 30, 50, 100, 200, 31),
            Some(RobotsRollerBotBehaviorWinner::HitFatal)
        );
    }

    #[test]
    fn pursue_gate_is_full_3d_and_inclusive_at_sixty() {
        assert_eq!(
            rollerbot_pursue_priority([0.0, 0.0, 0.0], Some([0.0, 60.0, 0.0])),
            30
        );
        assert_eq!(
            rollerbot_pursue_priority([0.0, 0.0, 0.0], Some([0.0, 60.001, 0.0])),
            1
        );
        assert_eq!(rollerbot_pursue_priority([0.0; 3], None), 1);
    }
}
