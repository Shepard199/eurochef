use super::{runtime_script_xitem::NativeScriptXItemSpawnRequest, MapFrame};

pub(super) const ROBOTS_PLAYER_FOOTDUST_FILE_UID: u32 = 0x0100_005D;
pub(super) const ROBOTS_PLAYER_FOOTDUST_SCRIPT_UID: u32 = 0x0400_0246;
pub(super) const ROBOTS_PLAYER_LEFT_FOOT_BONE_UID: u32 = 0x0E00_0018;
pub(super) const ROBOTS_PLAYER_RIGHT_FOOT_BONE_UID: u32 = 0x0E00_001B;
pub(super) const ROBOTS_PLAYER_FOOTDUST_SUPPRESSED_STATE: u8 = 0x0C;

/// Player +0xF4 override `0x004B0690` remaps FOOTSTEP argument 1/5 to
/// the right-foot bone and all other observed values to the left-foot bone.
/// The ordinary AI-character +0xF4 remains the base implementation; this
/// mapping must therefore stay Player-owned.
pub(super) fn native_player_footstep_bone_uid(arg0: u32) -> u32 {
    if matches!(arg0, 1 | 5) {
        ROBOTS_PLAYER_RIGHT_FOOT_BONE_UID
    } else {
        ROBOTS_PLAYER_LEFT_FOOT_BONE_UID
    }
}

/// Build the generic Script-XItem request after the Player animation host has
/// resolved the selected foot bone into the explicit world-position lane.
///
/// Native `0x004B0827` suppresses only Player state 0x0C. The explicit
/// `0x00404360` call then uses file 0x0100005D / script 0x04000246 and a
/// zero rotation vector; keep rotation identity instead of inheriting Player yaw.
pub(super) fn native_player_footdust_script_request(
    owner_key: u64,
    player_state: u8,
    foot_position_xyzw: [f32; 4],
) -> Option<NativeScriptXItemSpawnRequest> {
    (player_state != ROBOTS_PLAYER_FOOTDUST_SUPPRESSED_STATE).then_some(
        NativeScriptXItemSpawnRequest {
            owner_key,
            file_uid: ROBOTS_PLAYER_FOOTDUST_FILE_UID,
            script_uid: ROBOTS_PLAYER_FOOTDUST_SCRIPT_UID,
            position_xyzw: foot_position_xyzw,
            rotation: glam::Quat::IDENTITY,
        },
    )
}

impl MapFrame {
    /// UE/GUI host boundary for the recovered Player FOOTSTEP event.
    ///
    /// The caller is deliberately responsible for resolving
    /// `native_player_footstep_bone_uid(arg0)` against the live Player pose.
    /// Maps does not yet own a production Player AnimScript scheduler, so no
    /// camera-motion heuristic is allowed to manufacture footsteps.
    #[allow(dead_code)]
    pub(super) fn queue_native_player_footstep_script_xitem(
        &mut self,
        owner_key: u64,
        player_state: u8,
        foot_position_xyzw: [f32; 4],
    ) -> bool {
        let Some(request) =
            native_player_footdust_script_request(owner_key, player_state, foot_position_xyzw)
        else {
            return false;
        };
        self.native_script_xitem_spawns.push(request);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_footstep_argument_selects_exact_native_foot_bone() {
        assert_eq!(
            native_player_footstep_bone_uid(0),
            ROBOTS_PLAYER_LEFT_FOOT_BONE_UID
        );
        assert_eq!(
            native_player_footstep_bone_uid(1),
            ROBOTS_PLAYER_RIGHT_FOOT_BONE_UID
        );
        assert_eq!(
            native_player_footstep_bone_uid(5),
            ROBOTS_PLAYER_RIGHT_FOOT_BONE_UID
        );
        assert_eq!(
            native_player_footstep_bone_uid(4),
            ROBOTS_PLAYER_LEFT_FOOT_BONE_UID
        );
    }

    #[test]
    fn player_footdust_uses_generic_script_xitem_and_exact_state_gate() {
        let position = [1.0, 2.0, 3.0, 0.0];
        let request = native_player_footdust_script_request(0x1234, 0x00, position)
            .expect("ordinary Player state emits FootDust");
        assert_eq!(request.owner_key, 0x1234);
        assert_eq!(request.file_uid, ROBOTS_PLAYER_FOOTDUST_FILE_UID);
        assert_eq!(request.script_uid, ROBOTS_PLAYER_FOOTDUST_SCRIPT_UID);
        assert_eq!(request.position_xyzw, position);
        assert_eq!(request.rotation, glam::Quat::IDENTITY);

        assert!(native_player_footdust_script_request(
            0x1234,
            ROBOTS_PLAYER_FOOTDUST_SUPPRESSED_STATE,
            position
        )
        .is_none());
    }
}
