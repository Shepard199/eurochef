use serde::Serialize;

pub const ROBOTS_AI_PURSUE_PRIORITY: u8 = 0x1E;

/// Engine-neutral configuration for native `AI_Pursue` (`0x0046D840`).
/// This is the direct, non-NavMesh pursuit node used by EQ04 Mine and RollerBot.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsAiPursueConfig {
    pub radius: f32,
    pub locomotion_scalar: f32,
    pub priority: u8,
}

impl RobotsAiPursueConfig {
    pub const fn new(radius: f32, locomotion_scalar: f32) -> Self {
        Self {
            radius,
            locomotion_scalar,
            priority: ROBOTS_AI_PURSUE_PRIORITY,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsAiPursueStep {
    pub target_yaw_radians: f32,
    pub locomotion_scalar: f32,
}

/// Native gate `0x0046D950`: Player must exist and full XYZ squared distance is
/// accepted inclusively at the configured radius.
pub fn ai_pursue_priority(
    config: RobotsAiPursueConfig,
    owner_position_xyz: [f32; 3],
    target_position_xyz: Option<[f32; 3]>,
) -> u8 {
    let Some(target) = target_position_xyz else {
        return 1;
    };
    let dx = owner_position_xyz[0] - target[0];
    let dy = owner_position_xyz[1] - target[1];
    let dz = owner_position_xyz[2] - target[2];
    let distance_squared = dx * dx + dy * dy + dz * dz;
    if distance_squared <= config.radius * config.radius {
        config.priority
    } else {
        1
    }
}

/// Native execute `0x0046D900`: face the Player in XZ then call owner vslot
/// `+0x10C(config.scalar, -1.0)`. The host owns that common locomotion seam.
pub fn step_ai_pursue(
    config: RobotsAiPursueConfig,
    owner_position_xyz: [f32; 3],
    target_position_xyz: [f32; 3],
) -> RobotsAiPursueStep {
    RobotsAiPursueStep {
        target_yaw_radians: (target_position_xyz[0] - owner_position_xyz[0])
            .atan2(target_position_xyz[2] - owner_position_xyz[2]),
        locomotion_scalar: config.locomotion_scalar,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_pursue_uses_full_xyz_inclusive_radius_and_priority30() {
        let config = RobotsAiPursueConfig::new(15.0, 1.0);
        assert_eq!(config.priority, 0x1E);
        assert_eq!(
            ai_pursue_priority(config, [0.0, 0.0, 0.0], Some([9.0, 12.0, 0.0])),
            0x1E
        );
        assert_eq!(
            ai_pursue_priority(config, [0.0, 0.0, 0.0], Some([15.0, 0.0, 0.0])),
            0x1E
        );
        assert_eq!(
            ai_pursue_priority(config, [0.0, 0.0, 0.0], Some([15.001, 0.0, 0.0])),
            1
        );
        assert_eq!(ai_pursue_priority(config, [0.0, 0.0, 0.0], None), 1);
    }

    #[test]
    fn direct_pursue_faces_target_and_preserves_configured_scalar() {
        let config = RobotsAiPursueConfig::new(15.0, 1.0);
        let step = step_ai_pursue(config, [0.0, 2.0, 0.0], [1.0, 99.0, 0.0]);
        assert!((step.target_yaw_radians - std::f32::consts::FRAC_PI_2).abs() < 1.0e-6);
        assert_eq!(step.locomotion_scalar.to_bits(), 1.0f32.to_bits());
    }
}
