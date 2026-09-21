use serde::Serialize;

pub const ROBOTS_FOLLOW_NETWORK_PATH_FALLBACK_ARRIVAL_RADIUS: f32 = 1.0;
pub const ROBOTS_FOLLOW_NETWORK_PATH_ANIM_MODE_BASE: u32 = 0x0900_0000;
pub const ROBOTS_FOLLOW_NETWORK_PATH_TICKS_PER_SECOND: u32 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsFollowNetworkPathConfig {
    pub priority: u8,
    pub target_locomotion_scalar: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsFollowNetworkPathNode {
    pub position_xyz: [f32; 3],
    /// Serialized EXGeoPathNode::size.x. Native `0x00420990` uses half of this
    /// value as the strict arrival radius when it is positive; otherwise 1.0.
    pub size_x: f32,
    /// Serialized EXGeoPathNode::value[0..4]. `0x00420890 -> 0x0046BFA0`
    /// consumes lanes 0/1/2 as command type / AnimMode low16 / duration seconds.
    pub value: [u16; 4],
}

impl RobotsFollowNetworkPathNode {
    pub fn arrival_radius(self) -> f32 {
        if self.size_x > 0.0 {
            self.size_x * 0.5
        } else {
            ROBOTS_FOLLOW_NETWORK_PATH_FALLBACK_ARRIVAL_RADIUS
        }
    }

    pub fn command(self) -> Option<RobotsFollowNetworkPathCommand> {
        let anim_mode = ROBOTS_FOLLOW_NETWORK_PATH_ANIM_MODE_BASE + u32::from(self.value[1]);
        match self.value[0] {
            1 => Some(RobotsFollowNetworkPathCommand::UntilSetupIdle { anim_mode }),
            2 => Some(RobotsFollowNetworkPathCommand::Timed {
                anim_mode,
                ticks: u32::from(self.value[2])
                    .wrapping_mul(ROBOTS_FOLLOW_NETWORK_PATH_TICKS_PER_SECOND),
            }),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RobotsFollowNetworkPathCommand {
    UntilSetupIdle { anim_mode: u32 },
    Timed { anim_mode: u32, ticks: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
enum RobotsFollowNetworkPathActiveCommand {
    UntilSetupIdle {
        anim_mode: u32,
    },
    Timed {
        anim_mode: u32,
        remaining_ticks: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsFollowNetworkPathRuntimeState {
    /// XPathController +0x4E. Fresh controller construction initializes this to 0.
    pub current_node_index: usize,
    /// XPathController +0x4C, written to the node just left by `0x00423CF0`.
    pub previous_node_index: Option<usize>,
    /// XPathController +0x42. Path type 0 toggles this at the two endpoints.
    pub reverse: bool,
    pub entered: bool,
    active_command: Option<RobotsFollowNetworkPathActiveCommand>,
}

impl Default for RobotsFollowNetworkPathRuntimeState {
    fn default() -> Self {
        Self {
            current_node_index: 0,
            previous_node_index: None,
            reverse: false,
            entered: false,
            active_command: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsFollowNetworkPathStep {
    pub target_position_xyz: [f32; 3],
    pub target_yaw_radians: f32,
    pub target_locomotion_scalar: f32,
    pub requested_anim_mode: Option<u32>,
    pub movement_enabled: bool,
    pub arrived_node_index: Option<usize>,
}

pub const fn follow_network_path_priority(config: RobotsFollowNetworkPathConfig) -> u8 {
    config.priority
}

impl RobotsFollowNetworkPathRuntimeState {
    /// Native enter `0x0046BF80` does not rewind XPathController. It clears the
    /// path-command state and re-resolves the current node position.
    pub fn enter(&mut self, node_count: usize) -> bool {
        if node_count == 0 {
            self.entered = false;
            return false;
        }
        if self.current_node_index >= node_count {
            self.current_node_index = node_count - 1;
        }
        self.entered = true;
        self.active_command = None;
        true
    }

    pub fn leave(&mut self) {
        self.entered = false;
    }

    /// Event `HT_ScriptEvents_SetupIdle` only clears native command state 1.
    /// Timed command state 2 ignores SetupIdle and owns its countdown.
    pub fn setup_idle(&mut self) {
        if matches!(
            self.active_command,
            Some(RobotsFollowNetworkPathActiveCommand::UntilSetupIdle { .. })
        ) {
            self.active_command = None;
        }
    }

    fn advance_index(&mut self, path_type: u16, node_count: usize) {
        if node_count == 0 {
            return;
        }
        let last = node_count - 1;
        let old = self.current_node_index.min(last);
        self.previous_node_index = Some(old);

        if !self.reverse {
            if old < last {
                self.current_node_index = old + 1;
                return;
            }
            match path_type {
                // XPath type 0: ping-pong. At the forward end native selects the
                // node before the endpoint and flips the direction flag.
                0 => {
                    self.current_node_index = last.saturating_sub(1);
                    self.reverse = true;
                }
                // XPath type 1: loop back to node zero.
                1 => {
                    self.current_node_index = 0;
                }
                // Other native path types retain the final valid node.
                _ => {
                    self.current_node_index = last;
                }
            }
            return;
        }

        if old > 0 {
            self.current_node_index = old - 1;
            return;
        }
        if path_type == 0 {
            self.current_node_index = usize::from(last > 0);
            self.reverse = false;
        } else {
            self.current_node_index = 0;
        }
    }

    fn movement_step(
        &self,
        config: RobotsFollowNetworkPathConfig,
        node: RobotsFollowNetworkPathNode,
        owner_position_xyz: [f32; 3],
        arrived_node_index: Option<usize>,
    ) -> RobotsFollowNetworkPathStep {
        let dx = node.position_xyz[0] - owner_position_xyz[0];
        let dz = node.position_xyz[2] - owner_position_xyz[2];
        RobotsFollowNetworkPathStep {
            target_position_xyz: node.position_xyz,
            target_yaw_radians: dx.atan2(dz),
            target_locomotion_scalar: config.target_locomotion_scalar,
            requested_anim_mode: None,
            movement_enabled: true,
            arrived_node_index,
        }
    }

    pub fn step(
        &mut self,
        config: RobotsFollowNetworkPathConfig,
        path_type: u16,
        nodes: &[RobotsFollowNetworkPathNode],
        owner_position_xyz: [f32; 3],
    ) -> Option<RobotsFollowNetworkPathStep> {
        if !self.entered || nodes.is_empty() {
            return None;
        }
        if self.current_node_index >= nodes.len() {
            self.current_node_index = nodes.len() - 1;
        }

        if let Some(command) = self.active_command {
            match command {
                RobotsFollowNetworkPathActiveCommand::UntilSetupIdle { anim_mode } => {
                    let node = nodes[self.current_node_index];
                    return Some(RobotsFollowNetworkPathStep {
                        target_position_xyz: node.position_xyz,
                        target_yaw_radians: 0.0,
                        target_locomotion_scalar: config.target_locomotion_scalar,
                        requested_anim_mode: Some(anim_mode),
                        movement_enabled: false,
                        arrived_node_index: None,
                    });
                }
                RobotsFollowNetworkPathActiveCommand::Timed {
                    anim_mode,
                    remaining_ticks,
                } => {
                    let remaining_ticks = remaining_ticks.wrapping_sub(1);
                    if remaining_ticks != 0 {
                        self.active_command = Some(RobotsFollowNetworkPathActiveCommand::Timed {
                            anim_mode,
                            remaining_ticks,
                        });
                        let node = nodes[self.current_node_index];
                        return Some(RobotsFollowNetworkPathStep {
                            target_position_xyz: node.position_xyz,
                            target_yaw_radians: 0.0,
                            target_locomotion_scalar: config.target_locomotion_scalar,
                            requested_anim_mode: Some(anim_mode),
                            movement_enabled: false,
                            arrived_node_index: None,
                        });
                    }
                    self.active_command = None;
                }
            }
        }

        let current_index = self.current_node_index;
        let current = nodes[current_index];
        let dx = current.position_xyz[0] - owner_position_xyz[0];
        let dy = current.position_xyz[1] - owner_position_xyz[1];
        let dz = current.position_xyz[2] - owner_position_xyz[2];
        let radius = current.arrival_radius();
        let arrived = dx * dx + dy * dy + dz * dz < radius * radius;

        if !arrived {
            return Some(self.movement_step(config, current, owner_position_xyz, None));
        }

        self.advance_index(path_type, nodes.len());
        let arrived_node_index = self.previous_node_index;
        if let Some(command) = current.command() {
            let (active, anim_mode) = match command {
                RobotsFollowNetworkPathCommand::UntilSetupIdle { anim_mode } => (
                    RobotsFollowNetworkPathActiveCommand::UntilSetupIdle { anim_mode },
                    anim_mode,
                ),
                RobotsFollowNetworkPathCommand::Timed { anim_mode, ticks } => (
                    RobotsFollowNetworkPathActiveCommand::Timed {
                        anim_mode,
                        remaining_ticks: ticks,
                    },
                    anim_mode,
                ),
            };
            self.active_command = Some(active);
            let target = nodes[self.current_node_index];
            return Some(RobotsFollowNetworkPathStep {
                target_position_xyz: target.position_xyz,
                target_yaw_radians: 0.0,
                target_locomotion_scalar: config.target_locomotion_scalar,
                requested_anim_mode: Some(anim_mode),
                movement_enabled: false,
                arrived_node_index,
            });
        }

        let target = nodes[self.current_node_index];
        Some(self.movement_step(config, target, owner_position_xyz, arrived_node_index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: RobotsFollowNetworkPathConfig = RobotsFollowNetworkPathConfig {
        priority: 0x10,
        target_locomotion_scalar: 0.4,
    };

    fn node(x: f32, size_x: f32, value: [u16; 4]) -> RobotsFollowNetworkPathNode {
        RobotsFollowNetworkPathNode {
            position_xyz: [x, 0.0, 0.0],
            size_x,
            value,
        }
    }

    #[test]
    fn arrival_radius_matches_00420990_positive_and_fallback_cases() {
        assert_eq!(node(0.0, 4.0, [0; 4]).arrival_radius(), 2.0);
        assert_eq!(node(0.0, 0.0, [0; 4]).arrival_radius(), 1.0);
        assert_eq!(node(0.0, -3.0, [0; 4]).arrival_radius(), 1.0);
    }

    #[test]
    fn type0_ping_pongs_and_type1_loops_from_node_zero() {
        let nodes = [
            node(0.0, 2.0, [0; 4]),
            node(10.0, 2.0, [0; 4]),
            node(20.0, 2.0, [0; 4]),
        ];
        let mut ping_pong = RobotsFollowNetworkPathRuntimeState::default();
        assert!(ping_pong.enter(nodes.len()));
        assert_eq!(
            ping_pong
                .step(CONFIG, 0, &nodes, [0.0, 0.0, 0.0])
                .unwrap()
                .arrived_node_index,
            Some(0)
        );
        assert_eq!(ping_pong.current_node_index, 1);
        let _ = ping_pong.step(CONFIG, 0, &nodes, [10.0, 0.0, 0.0]);
        assert_eq!(ping_pong.current_node_index, 2);
        let _ = ping_pong.step(CONFIG, 0, &nodes, [20.0, 0.0, 0.0]);
        assert_eq!(ping_pong.current_node_index, 1);
        assert!(ping_pong.reverse);

        let mut looping = RobotsFollowNetworkPathRuntimeState::default();
        assert!(looping.enter(nodes.len()));
        let _ = looping.step(CONFIG, 1, &nodes, [0.0, 0.0, 0.0]);
        let _ = looping.step(CONFIG, 1, &nodes, [10.0, 0.0, 0.0]);
        let _ = looping.step(CONFIG, 1, &nodes, [20.0, 0.0, 0.0]);
        assert_eq!(looping.current_node_index, 0);
        assert!(!looping.reverse);
    }

    #[test]
    fn node_commands_run_after_arrival_and_setup_idle_only_clears_state1() {
        let until_idle = [node(0.0, 2.0, [1, 0x25, 0, 0]), node(5.0, 2.0, [0; 4])];
        let mut state = RobotsFollowNetworkPathRuntimeState::default();
        assert!(state.enter(until_idle.len()));
        let first = state.step(CONFIG, 1, &until_idle, [0.0, 0.0, 0.0]).unwrap();
        assert_eq!(first.requested_anim_mode, Some(0x0900_0025));
        assert!(!first.movement_enabled);
        let held = state.step(CONFIG, 1, &until_idle, [0.0, 0.0, 0.0]).unwrap();
        assert_eq!(held.requested_anim_mode, Some(0x0900_0025));
        state.setup_idle();
        let moving = state.step(CONFIG, 1, &until_idle, [0.0, 0.0, 0.0]).unwrap();
        assert!(moving.movement_enabled);
        assert_eq!(moving.target_position_xyz, [5.0, 0.0, 0.0]);

        let timed = [node(0.0, 2.0, [2, 0x27, 1, 0]), node(5.0, 2.0, [0; 4])];
        let mut state = RobotsFollowNetworkPathRuntimeState::default();
        assert!(state.enter(timed.len()));
        let first = state.step(CONFIG, 1, &timed, [0.0, 0.0, 0.0]).unwrap();
        assert_eq!(first.requested_anim_mode, Some(0x0900_0027));
        state.setup_idle();
        for _ in 0..59 {
            let step = state.step(CONFIG, 1, &timed, [0.0, 0.0, 0.0]).unwrap();
            assert_eq!(step.requested_anim_mode, Some(0x0900_0027));
            assert!(!step.movement_enabled);
        }
        let step = state.step(CONFIG, 1, &timed, [0.0, 0.0, 0.0]).unwrap();
        assert!(step.movement_enabled);
    }
}
