use serde::Serialize;

pub const ROBOTS_HAZARD_WAIT_FOR_HIT_REGISTRATION_BIT: u32 = 0x0000_0800;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsHazardRegistrationUpdate {
    pub previous_mask: u32,
    pub requested_mask: u32,
    pub reconcile_required: bool,
}

/// XItemHandler_Hazard::HandleScriptCmdEvent 0x004113F4..0x0041140C.
/// Before delegating WaitForHit to ScriptLifecycle, Hazard ensures owner
/// registration bit 0x800. When the bit is absent, native calls 0x00443DF0 to
/// reconcile the 17 registration categories and stores the new mask at +0x268.
pub const fn hazard_wait_for_hit_registration_update(
    current_mask: u32,
) -> RobotsHazardRegistrationUpdate {
    let requested_mask = current_mask | ROBOTS_HAZARD_WAIT_FOR_HIT_REGISTRATION_BIT;
    RobotsHazardRegistrationUpdate {
        previous_mask: current_mask,
        requested_mask,
        reconcile_required: requested_mask != current_mask,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wait_for_hit_sets_only_registration_bit_0x800_when_missing() {
        let update = hazard_wait_for_hit_registration_update(0x0000_0125);
        assert_eq!(update.previous_mask, 0x0000_0125);
        assert_eq!(update.requested_mask, 0x0000_0925);
        assert!(update.reconcile_required);
    }

    #[test]
    fn wait_for_hit_registration_is_idempotent_once_bit_is_present() {
        let update = hazard_wait_for_hit_registration_update(0x1234_8ABC);
        assert_eq!(update.requested_mask, 0x1234_8ABC);
        assert!(!update.reconcile_required);
    }
}
