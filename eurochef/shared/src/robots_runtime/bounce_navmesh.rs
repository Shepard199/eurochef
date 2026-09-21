use serde::Serialize;

use super::locomotion::{shortest_yaw_delta, RobotsAiTurnRateInput};

pub const ROBOTS_BOUNCE_NAVMESH_PRIORITY: u8 = 0x0b;
pub const ROBOTS_BOUNCE_NAVMESH_COOLDOWN_TICKS: u32 = 15;
pub const ROBOTS_BOUNCE_NAVMESH_INITIAL_YAW_MODULUS: u32 = 0x167;
pub const ROBOTS_BOUNCE_NAVMESH_ONE_DEGREE_RADIANS: f32 = f32::from_bits(0x3c8e_fa35);
pub const ROBOTS_BOUNCE_NAVMESH_TURN_RATE_RADIANS_PER_SECOND: f32 = std::f32::consts::PI * 4.0;
pub const ROBOTS_BOUNCE_NAVMESH_IMPACT_ANIM_MODE: u32 = 0x0900_0029;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RobotsBounceNavMeshPhase {
    Moving,
    ImpactAnimation,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsBounceNavMeshRuntimeState {
    pub active: bool,
    pub phase: RobotsBounceNavMeshPhase,
    pub steering_target_yaw_radians: f32,
    pub bounce_cooldown_ticks: u32,
    /// Native node +0x34 caches owner Handler+0x37C and starts at -1.
    pub cached_query_serial_snapshot: u16,
}

impl RobotsBounceNavMeshRuntimeState {
    /// Shared AI_BounceNavMesh setup `0x0046D060`. Setup consumes one gameplay
    /// RNG draw and stores draw%359 degrees into both node +0x24 and Handler+0x5D8.
    pub fn from_setup_draw(draw: u32) -> Self {
        Self {
            active: false,
            phase: RobotsBounceNavMeshPhase::Moving,
            steering_target_yaw_radians: (draw % ROBOTS_BOUNCE_NAVMESH_INITIAL_YAW_MODULUS) as f32
                * ROBOTS_BOUNCE_NAVMESH_ONE_DEGREE_RADIANS,
            bounce_cooldown_ticks: 0,
            cached_query_serial_snapshot: u16::MAX,
        }
    }

    pub const fn priority(navigation_ready: bool) -> u8 {
        if navigation_ready {
            ROBOTS_BOUNCE_NAVMESH_PRIORITY
        } else {
            1
        }
    }

    pub fn enter(&mut self) {
        self.active = true;
        self.phase = RobotsBounceNavMeshPhase::Moving;
    }

    pub fn leave(&mut self) {
        self.active = false;
    }

    pub fn tick(&mut self) {
        self.bounce_cooldown_ticks = self.bounce_cooldown_ticks.saturating_sub(1);
    }

    /// Native `0x0046D260`: Handler+0x37C is the raw query serial snapshot, not
    /// the last accepted serial at +0x62C. A new non-sentinel marker requests
    /// the setup-provided impact animation while the owner still has health.
    pub fn observe_query_serial(
        &mut self,
        query_serial_snapshot: u16,
        health: u8,
        source_yaw_radians: Option<f32>,
    ) -> bool {
        if query_serial_snapshot == u16::MAX
            || query_serial_snapshot == self.cached_query_serial_snapshot
            || health == 0
        {
            return false;
        }
        self.cached_query_serial_snapshot = query_serial_snapshot;
        if let Some(source_yaw_radians) = source_yaw_radians {
            self.steering_target_yaw_radians = source_yaw_radians;
        }
        true
    }

    /// Native `0x0046D2C0`: reflect current owner heading around the correction
    /// heading and suppress another boundary bounce for fifteen fixed updates.
    pub fn apply_boundary_bounce(
        &mut self,
        owner_yaw_radians: f32,
        correction_yaw_radians: f32,
    ) -> bool {
        if self.bounce_cooldown_ticks != 0 {
            return false;
        }
        self.steering_target_yaw_radians = correction_yaw_radians
            - shortest_yaw_delta(correction_yaw_radians, owner_yaw_radians)
            + std::f32::consts::PI;
        self.bounce_cooldown_ticks = ROBOTS_BOUNCE_NAVMESH_COOLDOWN_TICKS;
        true
    }

    pub fn request_impact_animation(&mut self) {
        self.phase = RobotsBounceNavMeshPhase::ImpactAnimation;
    }

    pub fn setup_idle(&mut self, got_hit_latch: &mut bool) {
        if self.phase == RobotsBounceNavMeshPhase::ImpactAnimation {
            self.phase = RobotsBounceNavMeshPhase::Moving;
        }
        *got_hit_latch = false;
    }

    /// Shared `0x0046D1C0` health lane used by both EW07 Dodgem and SpinTop.
    pub const fn target_locomotion_scalar(health: u8) -> f32 {
        if health >= 3 {
            0.0
        } else if health == 2 {
            0.25
        } else {
            0.5
        }
    }

    pub const fn turn_rate() -> RobotsAiTurnRateInput {
        RobotsAiTurnRateInput::Explicit(ROBOTS_BOUNCE_NAVMESH_TURN_RATE_RADIANS_PER_SECOND)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_setup_health_and_bounce_constants_match_native_node() {
        let mut state = RobotsBounceNavMeshRuntimeState::from_setup_draw(358);
        assert_eq!(
            state.steering_target_yaw_radians.to_bits(),
            (358.0f32 * ROBOTS_BOUNCE_NAVMESH_ONE_DEGREE_RADIANS).to_bits()
        );
        assert_eq!(
            RobotsBounceNavMeshRuntimeState::target_locomotion_scalar(3),
            0.0
        );
        assert_eq!(
            RobotsBounceNavMeshRuntimeState::target_locomotion_scalar(2),
            0.25
        );
        assert_eq!(
            RobotsBounceNavMeshRuntimeState::target_locomotion_scalar(1),
            0.5
        );
        assert!(state.apply_boundary_bounce(0.25, 1.0));
        assert_eq!(
            state.bounce_cooldown_ticks,
            ROBOTS_BOUNCE_NAVMESH_COOLDOWN_TICKS
        );
    }
}
