use serde::Serialize;

use super::attachment_rotation::postmultiply_local_z_rotation;

pub const ROBOTS_THIEFBOT_ATTACHMENT_FILE_UID: u32 = 0x0100_0031; // HT_File_EW05_ThiefBot
pub const ROBOTS_THIEFBOT_ATTACHMENT_ENTITY_UID: u32 = 0x0200_000f; // HT_Entity_Blades
pub const ROBOTS_THIEFBOT_ATTACHMENT_BONE_UID: u32 = 0x0e00_0070; // HT_AnimBone_Object01

/// Static initializers immediately before ThiefBot ctor:
/// 0x0045F1C0 -> 0x40C90FDB and 0x0045F1D0 -> 0x4196CBE4.
pub const ROBOTS_THIEFBOT_SLOW_SPIN_RADIANS_PER_SECOND: f32 = f32::from_bits(0x40c9_0fdb);
pub const ROBOTS_THIEFBOT_FAST_SPIN_RADIANS_PER_SECOND: f32 = f32::from_bits(0x4196_cbe4);
pub const ROBOTS_THIEFBOT_SPIN_ACCEL_RADIANS_PER_SECOND2: f32 =
    ROBOTS_THIEFBOT_FAST_SPIN_RADIANS_PER_SECOND;
pub const ROBOTS_THIEFBOT_FAST_DISTANCE_SQUARED: f32 = 16.0;
pub const ROBOTS_THIEFBOT_FAST_YAW_LIMIT_RADIANS: f32 = f32::from_bits(0x3f06_0a92);
pub const ROBOTS_THIEFBOT_FIXED_STEP_SECONDS: f32 = f32::from_bits(0x3c88_8889);

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsThiefBotAttachmentRuntimeState {
    /// Handler+0x644.
    pub current_spin_radians_per_second: f32,
    /// Handler+0x648.
    pub target_spin_radians_per_second: f32,
    /// Presentation-neutral local rotation accumulated for Handler+0x640.
    pub local_spin_xyzw: [f32; 4],
}

impl Default for RobotsThiefBotAttachmentRuntimeState {
    fn default() -> Self {
        Self {
            current_spin_radians_per_second: ROBOTS_THIEFBOT_SLOW_SPIN_RADIANS_PER_SECOND,
            target_spin_radians_per_second: ROBOTS_THIEFBOT_SLOW_SPIN_RADIANS_PER_SECOND,
            local_spin_xyzw: [0.0, 0.0, 0.0, 1.0],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsThiefBotAttachmentInput {
    pub target_available: bool,
    pub target_distance_squared: f32,
    /// Common Monster vslot +0x13C (0x00456CB0) relative target yaw.
    pub target_yaw_error_radians: f32,
}

pub fn step_thiefbot_attachment(
    state: &mut RobotsThiefBotAttachmentRuntimeState,
    input: RobotsThiefBotAttachmentInput,
) {
    // 0x0045F7C0 selects 6PI only for a live target strictly inside four
    // world units and strictly inside the 30-degree facing cone. Handler+0x64C
    // is the AttackGroup. Its base +0x18 re-entry threshold remains zero after
    // setup, so the native more-than-29-ticks-remaining veto cannot trigger.
    state.target_spin_radians_per_second = if input.target_available
        && input.target_distance_squared < ROBOTS_THIEFBOT_FAST_DISTANCE_SQUARED
        && input.target_yaw_error_radians.abs() < ROBOTS_THIEFBOT_FAST_YAW_LIMIT_RADIANS
    {
        ROBOTS_THIEFBOT_FAST_SPIN_RADIANS_PER_SECOND
    } else {
        ROBOTS_THIEFBOT_SLOW_SPIN_RADIANS_PER_SECOND
    };

    let max_delta =
        ROBOTS_THIEFBOT_SPIN_ACCEL_RADIANS_PER_SECOND2 * ROBOTS_THIEFBOT_FIXED_STEP_SECONDS;
    let delta = (state.target_spin_radians_per_second - state.current_spin_radians_per_second)
        .clamp(-max_delta, max_delta);
    state.current_spin_radians_per_second += delta;

    // Native passes the third Euler lane to 0x00454CA0, so this is local Z,
    // not the local-Y lane used by EQ03/EW09.
    let local_angle = -(state.current_spin_radians_per_second * ROBOTS_THIEFBOT_FIXED_STEP_SECONDS);
    state.local_spin_xyzw = postmultiply_local_z_rotation(state.local_spin_xyzw, local_angle);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctor_constants_and_fast_gate_match_0045f1c0_0045f1d0_and_0045f7c0() {
        assert_eq!(
            ROBOTS_THIEFBOT_SLOW_SPIN_RADIANS_PER_SECOND.to_bits(),
            0x40c9_0fdb
        );
        assert_eq!(
            ROBOTS_THIEFBOT_FAST_SPIN_RADIANS_PER_SECOND.to_bits(),
            0x4196_cbe4
        );

        let mut state = RobotsThiefBotAttachmentRuntimeState::default();
        step_thiefbot_attachment(
            &mut state,
            RobotsThiefBotAttachmentInput {
                target_available: true,
                target_distance_squared: 15.999,
                target_yaw_error_radians: 0.0,
            },
        );
        assert_eq!(
            state.target_spin_radians_per_second,
            ROBOTS_THIEFBOT_FAST_SPIN_RADIANS_PER_SECOND
        );
        let expected = ROBOTS_THIEFBOT_SLOW_SPIN_RADIANS_PER_SECOND
            + ROBOTS_THIEFBOT_FAST_SPIN_RADIANS_PER_SECOND * ROBOTS_THIEFBOT_FIXED_STEP_SECONDS;
        assert!((state.current_spin_radians_per_second - expected).abs() < 1.0e-6);

        let mut distance_boundary = RobotsThiefBotAttachmentRuntimeState::default();
        step_thiefbot_attachment(
            &mut distance_boundary,
            RobotsThiefBotAttachmentInput {
                target_available: true,
                target_distance_squared: 16.0,
                target_yaw_error_radians: 0.0,
            },
        );
        assert_eq!(
            distance_boundary.target_spin_radians_per_second,
            ROBOTS_THIEFBOT_SLOW_SPIN_RADIANS_PER_SECOND
        );

        let mut yaw_boundary = RobotsThiefBotAttachmentRuntimeState::default();
        step_thiefbot_attachment(
            &mut yaw_boundary,
            RobotsThiefBotAttachmentInput {
                target_available: true,
                target_distance_squared: 0.0,
                target_yaw_error_radians: ROBOTS_THIEFBOT_FAST_YAW_LIMIT_RADIANS,
            },
        );
        assert_eq!(
            yaw_boundary.target_spin_radians_per_second,
            ROBOTS_THIEFBOT_SLOW_SPIN_RADIANS_PER_SECOND
        );
    }

    #[test]
    fn fast_spin_postmultiplies_negative_local_z_delta() {
        let mut state = RobotsThiefBotAttachmentRuntimeState::default();
        step_thiefbot_attachment(
            &mut state,
            RobotsThiefBotAttachmentInput {
                target_available: true,
                target_distance_squared: 0.0,
                target_yaw_error_radians: 0.0,
            },
        );
        assert!(state.local_spin_xyzw[2] < 0.0);
        assert!(state.local_spin_xyzw[3] > 0.0);
    }
}
