use serde::Serialize;

use super::attachment_rotation::postmultiply_local_y_rotation;

pub const ROBOTS_EQ03_SPIDER_ATTACHMENT_SPIN_RADIANS_PER_TICK: f32 = std::f32::consts::PI / 60.0;
pub const ROBOTS_EQ03_SPIDER_ATTACHMENT_FILE_UID: u32 = 0x0100_004B;
pub const ROBOTS_EQ03_SPIDER_ATTACHMENT_ENTITY_UID: u32 = 0x0200_0010;
pub const ROBOTS_EQ03_SPIDER_ATTACHMENT_ROOT_BONE_UID: u32 = 0x0E00_0070;
pub const ROBOTS_EQ03_SPIDER_ATTACHMENT_PRIMARY_DATUM_UID: u32 = 0x1000_0019;
pub const ROBOTS_EQ03_SPIDER_ATTACHMENT_SECONDARY_DATUM_UID: u32 = 0x1000_001A;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsEq03SpiderAttachmentConfig {
    pub file_uid: u32,
    pub entity_uid: u32,
    pub root_bone_uid: u32,
    pub primary_datum_uid: u32,
    pub secondary_datum_uid: u32,
}

impl RobotsEq03SpiderAttachmentConfig {
    /// EQ03 ctor `0x00464F20`:
    /// `0x004543A0(0x0100004B,0x02000010,0x0E000070,0x10000019,0x1000001A)`.
    pub const fn native() -> Self {
        Self {
            file_uid: ROBOTS_EQ03_SPIDER_ATTACHMENT_FILE_UID,
            entity_uid: ROBOTS_EQ03_SPIDER_ATTACHMENT_ENTITY_UID,
            root_bone_uid: ROBOTS_EQ03_SPIDER_ATTACHMENT_ROOT_BONE_UID,
            primary_datum_uid: ROBOTS_EQ03_SPIDER_ATTACHMENT_PRIMARY_DATUM_UID,
            secondary_datum_uid: ROBOTS_EQ03_SPIDER_ATTACHMENT_SECONDARY_DATUM_UID,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsEq03SpiderAttachmentRuntimeState {
    pub local_spin_xyzw: [f32; 4],
}

impl Default for RobotsEq03SpiderAttachmentRuntimeState {
    fn default() -> Self {
        Self {
            local_spin_xyzw: [0.0, 0.0, 0.0, 1.0],
        }
    }
}

/// EQ03 Spider vslot +0x34 `0x00465220`: after common Monster update the
/// handler calls `0x00454CA0(Handler+0x640, 0, angle, 0)`. The setup at
/// `0x00464F20` stores pi in Handler+0x644, so the fixed-step angle is pi/60.
pub fn step_eq03_spider_attachment_rotation(current_quaternion_xyzw: [f32; 4]) -> [f32; 4] {
    postmultiply_local_y_rotation(
        current_quaternion_xyzw,
        ROBOTS_EQ03_SPIDER_ATTACHMENT_SPIN_RADIANS_PER_TICK,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctor_attachment_config_matches_native_arguments() {
        assert_eq!(
            RobotsEq03SpiderAttachmentConfig::native(),
            RobotsEq03SpiderAttachmentConfig {
                file_uid: 0x0100_004B,
                entity_uid: 0x0200_0010,
                root_bone_uid: 0x0E00_0070,
                primary_datum_uid: 0x1000_0019,
                secondary_datum_uid: 0x1000_001A,
            }
        );
    }

    #[test]
    fn attachment_spin_matches_native_pi_over_sixty_fixed_step() {
        let q = step_eq03_spider_attachment_rotation([0.0, 0.0, 0.0, 1.0]);
        let half = ROBOTS_EQ03_SPIDER_ATTACHMENT_SPIN_RADIANS_PER_TICK * 0.5;
        assert!(q[0].abs() < 1.0e-6);
        assert!((q[1] - half.sin()).abs() < 1.0e-6);
        assert!(q[2].abs() < 1.0e-6);
        assert!((q[3] - half.cos()).abs() < 1.0e-6);
    }
}
