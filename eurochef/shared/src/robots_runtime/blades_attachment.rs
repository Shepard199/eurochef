use serde::Serialize;

use super::{
    ai_character::RobotsAiHandlerClass,
    attachment_rotation::postmultiply_local_y_rotation,
};

pub const ROBOTS_BLADES_ENTITY_UID: u32 = 0x0200_000F;
pub const ROBOTS_BLADES_BONE_UID: u32 = 0x0E00_0070;
pub const ROBOTS_BLADES_ANIM_DATUM_UID: u32 = 0x1000_0019;
pub const ROBOTS_BLADES_ANGULAR_VELOCITY_RADIANS_PER_SECOND: f32 = std::f32::consts::TAU * 4.0;
pub const ROBOTS_BLADES_FIXED_STEP_SECONDS: f32 = 1.0 / 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsBladesAttachmentSpec {
    pub file_uid: u32,
    pub entity_uid: u32,
    pub bone_uid: u32,
    pub anim_datum_uid: u32,
}

impl RobotsBladesAttachmentSpec {
    /// EF01 ctor `0x00463620`.
    pub const fn ef01() -> Self {
        Self {
            file_uid: 0x0100_0048,
            entity_uid: ROBOTS_BLADES_ENTITY_UID,
            bone_uid: ROBOTS_BLADES_BONE_UID,
            anim_datum_uid: ROBOTS_BLADES_ANIM_DATUM_UID,
        }
    }

    /// EF03 ctor `0x00466F70`.
    pub const fn ef03() -> Self {
        Self {
            file_uid: 0x0100_0057,
            entity_uid: ROBOTS_BLADES_ENTITY_UID,
            bone_uid: ROBOTS_BLADES_BONE_UID,
            anim_datum_uid: ROBOTS_BLADES_ANIM_DATUM_UID,
        }
    }

    pub const fn for_handler_class(handler_class: RobotsAiHandlerClass) -> Option<Self> {
        match handler_class {
            RobotsAiHandlerClass::Ef01Mine => Some(Self::ef01()),
            RobotsAiHandlerClass::Ef03EvilBot => Some(Self::ef03()),
            _ => None,
        }
    }
}

/// Shared vslot +0x34 `0x00467500`: common Monster update first, then
/// `0x00454CA0(Handler+0x640, 0, 8*pi/60, 0)`.
pub fn step_blades_attachment_rotation(
    current_quaternion_xyzw: [f32; 4],
) -> [f32; 4] {
    postmultiply_local_y_rotation(
        current_quaternion_xyzw,
        ROBOTS_BLADES_ANGULAR_VELOCITY_RADIANS_PER_SECOND * ROBOTS_BLADES_FIXED_STEP_SECONDS,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ef01_and_ef03_ctor_specs_share_the_native_blades_shape() {
        let ef01 = RobotsBladesAttachmentSpec::ef01();
        let ef03 = RobotsBladesAttachmentSpec::ef03();
        assert_eq!(ef01.file_uid, 0x0100_0048);
        assert_eq!(ef03.file_uid, 0x0100_0057);
        for spec in [ef01, ef03] {
            assert_eq!(spec.entity_uid, 0x0200_000F);
            assert_eq!(spec.bone_uid, 0x0E00_0070);
            assert_eq!(spec.anim_datum_uid, 0x1000_0019);
        }
    }

    #[test]
    fn shared_blades_spin_exactly_four_turns_per_second_at_sixty_hz() {
        let mut quaternion = [0.0, 0.0, 0.0, 1.0];
        for _ in 0..60 {
            quaternion = step_blades_attachment_rotation(quaternion);
        }
        assert!(quaternion[0].abs() < 2.0e-6);
        assert!(quaternion[1].abs() < 2.0e-5);
        assert!(quaternion[2].abs() < 2.0e-6);
        assert!((quaternion[3] - 1.0).abs() < 2.0e-5);
    }

    #[test]
    fn only_ef01_and_ef03_own_the_shared_00467500_blades_attachment() {
        assert_eq!(
            RobotsBladesAttachmentSpec::for_handler_class(RobotsAiHandlerClass::Ef01Mine),
            Some(RobotsBladesAttachmentSpec::ef01())
        );
        assert_eq!(
            RobotsBladesAttachmentSpec::for_handler_class(RobotsAiHandlerClass::Ef03EvilBot),
            Some(RobotsBladesAttachmentSpec::ef03())
        );
        assert_eq!(
            RobotsBladesAttachmentSpec::for_handler_class(RobotsAiHandlerClass::Eq02MineBot),
            None
        );
    }
}
