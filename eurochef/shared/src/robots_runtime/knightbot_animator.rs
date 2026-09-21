use serde::Serialize;

pub const ROBOTS_KNIGHTBOT_ANIMATOR_FILE_UID: u32 = 0x0100_004E;
pub const ROBOTS_KNIGHTBOT_ANIMATOR_ANIMATION_UID: u32 = 0x0300_0012;
pub const ROBOTS_KNIGHTBOT_ANIMATOR_ROOT_BONE_UID: u32 = 0x0E00_0070;
pub const ROBOTS_KNIGHTBOT_ANIMATOR_POSITION_DATUM_UID: u32 = 0x1000_0019;
pub const ROBOTS_KNIGHTBOT_ANIMATOR_ROTATION_DATUM_UID: u32 = 0x1000_001A;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsKnightBotAnimatorConfig {
    pub file_uid: u32,
    pub animation_uid: u32,
    pub root_bone_uid: u32,
    pub position_datum_uid: u32,
    pub rotation_datum_uid: u32,
}

impl RobotsKnightBotAnimatorConfig {
    /// EB13 builder `0x00465770` calls AttachAnimator `0x004547E0` before any
    /// behavior node is allocated:
    /// `(HT_File_EB13_KnightBot, HT_Animation_Idle_Animator, Object01,
    ///   AnimatorPosition, AnimatorRotation, 0)`.
    pub const fn eb13() -> Self {
        Self {
            file_uid: ROBOTS_KNIGHTBOT_ANIMATOR_FILE_UID,
            animation_uid: ROBOTS_KNIGHTBOT_ANIMATOR_ANIMATION_UID,
            root_bone_uid: ROBOTS_KNIGHTBOT_ANIMATOR_ROOT_BONE_UID,
            position_datum_uid: ROBOTS_KNIGHTBOT_ANIMATOR_POSITION_DATUM_UID,
            rotation_datum_uid: ROBOTS_KNIGHTBOT_ANIMATOR_ROTATION_DATUM_UID,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsKnightBotAnimatorRuntimeState {
    pub attached: bool,
    pub position_xyz: [f32; 3],
    pub rotation_xyzw: [f32; 4],
}

impl Default for RobotsKnightBotAnimatorRuntimeState {
    fn default() -> Self {
        Self {
            attached: true,
            position_xyz: [0.0; 3],
            rotation_xyzw: [0.0, 0.0, 0.0, 1.0],
        }
    }
}

/// Exact mode-4 branch of the native Euler-to-quaternion helper `0x005094A6`.
/// Angles are radians. Keeping the formula explicit avoids importing an engine
/// Euler-order convention into the shared UE5.8 gameplay contract.
pub fn native_mode4_euler_quaternion(angles_xyz: [f32; 3]) -> [f32; 4] {
    let [x, y, z] = angles_xyz;
    let (sx, cx) = (0.5 * x).sin_cos();
    let (sy, cy) = (0.5 * y).sin_cos();
    let (sz, cz) = (0.5 * z).sin_cos();
    [
        cy * sx * cz + sz * sy * cx,
        sy * cx * cz - cy * sx * sz,
        cy * cx * sz - sy * sx * cz,
        cy * cx * cz + sy * sx * sz,
    ]
}

/// EB13 `0x00465E30` derives the attached animator orientation from the line
/// AnimatorPosition -> AnimatorRotation. Native computes:
/// pitch = atan2(sqrt(dx^2 + dz^2), dy) - PI/2
/// yaw   = atan2(dx, dz)
/// roll  = 0
/// and feeds those three angles to `0x005094A6` mode 4.
pub fn knightbot_animator_rotation_from_points(
    animator_position_xyz: [f32; 3],
    animator_rotation_point_xyz: [f32; 3],
) -> [f32; 4] {
    let dx = animator_rotation_point_xyz[0] - animator_position_xyz[0];
    let dy = animator_rotation_point_xyz[1] - animator_position_xyz[1];
    let dz = animator_rotation_point_xyz[2] - animator_position_xyz[2];
    let horizontal = (dx * dx + dz * dz).sqrt();
    let pitch = horizontal.atan2(dy) - std::f32::consts::FRAC_PI_2;
    let yaw = dx.atan2(dz);
    native_mode4_euler_quaternion([pitch, yaw, 0.0])
}

pub fn step_knightbot_animator(
    state: &mut RobotsKnightBotAnimatorRuntimeState,
    animator_position_xyz: [f32; 3],
    animator_rotation_point_xyz: [f32; 3],
) {
    if !state.attached {
        return;
    }
    state.position_xyz = animator_position_xyz;
    state.rotation_xyzw = knightbot_animator_rotation_from_points(
        animator_position_xyz,
        animator_rotation_point_xyz,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1.0e-6
    }

    #[test]
    fn builder_attach_animator_identity_matches_native_arguments() {
        assert_eq!(
            RobotsKnightBotAnimatorConfig::eb13(),
            RobotsKnightBotAnimatorConfig {
                file_uid: 0x0100_004E,
                animation_uid: 0x0300_0012,
                root_bone_uid: 0x0E00_0070,
                position_datum_uid: 0x1000_0019,
                rotation_datum_uid: 0x1000_001A,
            }
        );
    }

    #[test]
    fn forward_direction_produces_identity_like_native_mode4() {
        let q = knightbot_animator_rotation_from_points([1.0, 2.0, 3.0], [1.0, 2.0, 4.0]);
        assert!(near(q[0], 0.0));
        assert!(near(q[1], 0.0));
        assert!(near(q[2], 0.0));
        assert!(near(q[3], 1.0));
    }

    #[test]
    fn right_and_up_directions_preserve_native_pitch_yaw_convention() {
        let right = knightbot_animator_rotation_from_points([0.0; 3], [1.0, 0.0, 0.0]);
        let h = std::f32::consts::FRAC_PI_4;
        assert!(near(right[0], 0.0));
        assert!(near(right[1], h.sin()));
        assert!(near(right[2], 0.0));
        assert!(near(right[3], h.cos()));

        let up = knightbot_animator_rotation_from_points([0.0; 3], [0.0, 1.0, 0.0]);
        assert!(near(up[0], -h.sin()));
        assert!(near(up[1], 0.0));
        assert!(near(up[2], 0.0));
        assert!(near(up[3], h.cos()));
    }

    #[test]
    fn step_copies_animator_position_and_updates_rotation() {
        let mut state = RobotsKnightBotAnimatorRuntimeState::default();
        step_knightbot_animator(&mut state, [2.0, 3.0, 4.0], [2.0, 3.0, 5.0]);
        assert_eq!(state.position_xyz, [2.0, 3.0, 4.0]);
        assert_eq!(state.rotation_xyzw, [0.0, 0.0, 0.0, 1.0]);
    }
}
