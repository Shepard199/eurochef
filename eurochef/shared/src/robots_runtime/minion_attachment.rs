use serde::Serialize;

use super::{
    ai_character::RobotsAiHandlerClass,
    attachment_rotation::postmultiply_local_z_rotation,
};

pub const ROBOTS_MINION_ATTACHMENT_ENTITY_UID: u32 = 0x0200_0039;
pub const ROBOTS_MINION_ATTACHMENT_ROOT_DATUM: u32 = 0x0E00_0070;
pub const ROBOTS_MINION_ATTACHMENT_PRIMARY_DATUM: u32 = 0x1000_0019;
pub const ROBOTS_MINION_ATTACHMENT_SECONDARY_DATUM: u32 = 0x1000_001A;

pub const ROBOTS_EB14_ATTACHMENT_SPIN_RADIANS_PER_TICK: f32 =
    -std::f32::consts::PI / 60.0;
pub const ROBOTS_EW10_ATTACHMENT_SPIN_RADIANS_PER_TICK: f32 =
    -std::f32::consts::TAU / 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsMinionAttachmentConfig {
    pub owner_file_uid: u32,
    pub entity_uid: u32,
    pub root_datum: u32,
    pub primary_datum: u32,
    pub secondary_datum: Option<u32>,
    pub spin_radians_per_tick: f32,
}

impl RobotsMinionAttachmentConfig {
    /// EB14 ctor 0x004641D0:
    /// 0x004543A0(0x01000056, 0x02000039, 0x0E000070, 0x10000019, 0).
    pub const fn eb14() -> Self {
        Self {
            owner_file_uid: 0x0100_0056,
            entity_uid: ROBOTS_MINION_ATTACHMENT_ENTITY_UID,
            root_datum: ROBOTS_MINION_ATTACHMENT_ROOT_DATUM,
            primary_datum: ROBOTS_MINION_ATTACHMENT_PRIMARY_DATUM,
            secondary_datum: None,
            spin_radians_per_tick: ROBOTS_EB14_ATTACHMENT_SPIN_RADIANS_PER_TICK,
        }
    }

    /// EW10 ctor 0x00463BD0:
    /// 0x004543A0(0x0100004F, 0x02000039, 0x0E000070, 0x10000019, 0x1000001A).
    pub const fn ew10() -> Self {
        Self {
            owner_file_uid: 0x0100_004F,
            entity_uid: ROBOTS_MINION_ATTACHMENT_ENTITY_UID,
            root_datum: ROBOTS_MINION_ATTACHMENT_ROOT_DATUM,
            primary_datum: ROBOTS_MINION_ATTACHMENT_PRIMARY_DATUM,
            secondary_datum: Some(ROBOTS_MINION_ATTACHMENT_SECONDARY_DATUM),
            spin_radians_per_tick: ROBOTS_EW10_ATTACHMENT_SPIN_RADIANS_PER_TICK,
        }
    }

    pub const fn for_handler_class(handler_class: RobotsAiHandlerClass) -> Option<Self> {
        match handler_class {
            RobotsAiHandlerClass::Eb14Minion => Some(Self::eb14()),
            RobotsAiHandlerClass::Ew10Minion => Some(Self::ew10()),
            _ => None,
        }
    }
}

/// EB14 0x004647B0 and EW10 0x00460340 both call the common Monster update
/// first, then 0x00454CA0(Handler+0x640, 0, 0, angle). The only gameplay
/// difference is the native per-tick local-Z angle.
pub fn step_minion_attachment_rotation(
    current_quaternion_xyzw: [f32; 4],
    config: RobotsMinionAttachmentConfig,
) -> [f32; 4] {
    postmultiply_local_z_rotation(current_quaternion_xyzw, config.spin_radians_per_tick)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eb14_ctor_and_post_update_match_native_constants() {
        let config = RobotsMinionAttachmentConfig::eb14();
        assert_eq!(config.owner_file_uid, 0x0100_0056);
        assert_eq!(config.entity_uid, 0x0200_0039);
        assert_eq!(config.root_datum, 0x0E00_0070);
        assert_eq!(config.primary_datum, 0x1000_0019);
        assert_eq!(config.secondary_datum, None);
        assert_eq!(
            config.spin_radians_per_tick.to_bits(),
            (-std::f32::consts::PI / 60.0).to_bits()
        );
        let q = step_minion_attachment_rotation([0.0, 0.0, 0.0, 1.0], config);
        let half = config.spin_radians_per_tick * 0.5;
        assert!(q[0].abs() < 1.0e-6);
        assert!(q[1].abs() < 1.0e-6);
        assert!((q[2] - half.sin()).abs() < 1.0e-6);
        assert!((q[3] - half.cos()).abs() < 1.0e-6);
    }

    #[test]
    fn ew10_ctor_and_post_update_match_native_constants() {
        let config = RobotsMinionAttachmentConfig::ew10();
        assert_eq!(config.owner_file_uid, 0x0100_004F);
        assert_eq!(config.entity_uid, 0x0200_0039);
        assert_eq!(config.root_datum, 0x0E00_0070);
        assert_eq!(config.primary_datum, 0x1000_0019);
        assert_eq!(config.secondary_datum, Some(0x1000_001A));
        assert_eq!(
            config.spin_radians_per_tick.to_bits(),
            (-std::f32::consts::TAU / 60.0).to_bits()
        );
    }

    #[test]
    fn only_the_two_native_minion_handlers_expose_this_attachment_contract() {
        assert_eq!(
            RobotsMinionAttachmentConfig::for_handler_class(RobotsAiHandlerClass::Eb14Minion),
            Some(RobotsMinionAttachmentConfig::eb14())
        );
        assert_eq!(
            RobotsMinionAttachmentConfig::for_handler_class(RobotsAiHandlerClass::Ew10Minion),
            Some(RobotsMinionAttachmentConfig::ew10())
        );
        assert_eq!(
            RobotsMinionAttachmentConfig::for_handler_class(RobotsAiHandlerClass::Eb13KnightBot),
            None
        );
    }
}
