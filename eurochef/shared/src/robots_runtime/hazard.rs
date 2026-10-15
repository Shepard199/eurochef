use serde::Serialize;

pub const ROBOTS_HAZARD_WAIT_FOR_HIT_REGISTRATION_BIT: u32 = 0x0000_0800;
pub const ROBOTS_SCRIPT_WAIT_FOR_HIT_FILTER_MASK: u32 = 0x0000_015A;
pub const ROBOTS_SCRIPT_WAIT_FOR_HIT_MISSILES_FLAG: u32 = 0x0010_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsScriptWaitForHitResult {
    pub hold_timeline: bool,
    pub notify_context: bool,
    pub next_lifecycle_flags_3bc: u8,
}

/// ScriptLifecycle WaitForHit 0x004172C0.
pub const fn script_wait_for_hit_result(
    requested_mask: u32,
    hit_flags_378: u32,
    lifecycle_flags_3bc: u8,
) -> RobotsScriptWaitForHitResult {
    if lifecycle_flags_3bc & 1 == 0 {
        return RobotsScriptWaitForHitResult {
            hold_timeline: true,
            notify_context: false,
            next_lifecycle_flags_3bc: lifecycle_flags_3bc,
        };
    }
    let requested = requested_mask & ROBOTS_SCRIPT_WAIT_FOR_HIT_FILTER_MASK;
    let observed = hit_flags_378 & ROBOTS_SCRIPT_WAIT_FOR_HIT_FILTER_MASK;
    if requested != 0 && (observed == 0 || observed & requested != requested) {
        return RobotsScriptWaitForHitResult {
            hold_timeline: true,
            notify_context: false,
            next_lifecycle_flags_3bc: lifecycle_flags_3bc,
        };
    }
    let signed_byte1 = ((hit_flags_378 >> 8) & 0xFF) as u8;
    if signed_byte1 & 0x80 != 0 && requested_mask & 1 == 0 && hit_flags_378 & 0x0001_0000 == 0 {
        return RobotsScriptWaitForHitResult {
            hold_timeline: true,
            notify_context: false,
            next_lifecycle_flags_3bc: lifecycle_flags_3bc,
        };
    }
    RobotsScriptWaitForHitResult {
        hold_timeline: false,
        notify_context: true,
        next_lifecycle_flags_3bc: lifecycle_flags_3bc | 2,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsScriptWaitForHitMissilesResult {
    pub hold_timeline: bool,
    pub notify_context: bool,
    pub next_lifecycle_flags_3bc: u8,
}

/// ScriptLifecycle WaitForHitMissiles branch in 0x00416D10.
/// Handler +0x3BC bit0 arms the wait. Command arg0 bit0 additionally requires
/// Handler +0x378 bit0x100000. Success notifies the context and sets +0x3BC bit1.
pub const fn script_wait_for_hit_missiles_result(
    requested_mask: u32,
    hit_flags_378: u32,
    lifecycle_flags_3bc: u8,
) -> RobotsScriptWaitForHitMissilesResult {
    if lifecycle_flags_3bc & 1 == 0
        || (requested_mask & 1 != 0
            && hit_flags_378 & ROBOTS_SCRIPT_WAIT_FOR_HIT_MISSILES_FLAG == 0)
    {
        return RobotsScriptWaitForHitMissilesResult {
            hold_timeline: true,
            notify_context: false,
            next_lifecycle_flags_3bc: lifecycle_flags_3bc,
        };
    }
    RobotsScriptWaitForHitMissilesResult {
        hold_timeline: false,
        notify_context: true,
        next_lifecycle_flags_3bc: lifecycle_flags_3bc | 2,
    }
}

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
    fn script_wait_for_hit_matches_native_004172c0_predicate() {
        let unarmed = script_wait_for_hit_result(0x15A, 0x15A, 0);
        assert!(unarmed.hold_timeline);
        assert!(!unarmed.notify_context);

        let partial = script_wait_for_hit_result(0x15A, 0x10A, 1);
        assert!(partial.hold_timeline);

        let matched = script_wait_for_hit_result(0x15A, 0x15A, 1);
        assert!(!matched.hold_timeline);
        assert!(matched.notify_context);
        assert_eq!(matched.next_lifecycle_flags_3bc, 3);

        let signed_rejected = script_wait_for_hit_result(0x0002, 0x0000_8002, 1);
        assert!(signed_rejected.hold_timeline);
        let signed_override = script_wait_for_hit_result(0x0003, 0x0000_8002, 1);
        assert!(!signed_override.hold_timeline);
    }

    #[test]
    fn script_wait_for_hit_missiles_matches_native_predicate() {
        let unarmed = script_wait_for_hit_missiles_result(0, 0, 0);
        assert!(unarmed.hold_timeline);
        assert!(!unarmed.notify_context);

        let armed_without_mask = script_wait_for_hit_missiles_result(0, 0, 1);
        assert!(!armed_without_mask.hold_timeline);
        assert!(armed_without_mask.notify_context);
        assert_eq!(armed_without_mask.next_lifecycle_flags_3bc, 3);

        let missing_missile_hit = script_wait_for_hit_missiles_result(1, 0, 1);
        assert!(missing_missile_hit.hold_timeline);

        let missile_hit =
            script_wait_for_hit_missiles_result(1, ROBOTS_SCRIPT_WAIT_FOR_HIT_MISSILES_FLAG, 1);
        assert!(!missile_hit.hold_timeline);
        assert!(missile_hit.notify_context);
        assert_eq!(missile_hit.next_lifecycle_flags_3bc, 3);

        let unrelated_mask = script_wait_for_hit_missiles_result(2, 0, 1);
        assert!(!unrelated_mask.hold_timeline);
    }

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
