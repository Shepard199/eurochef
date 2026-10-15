//! Native XTrigger_ChangeLevel event 0x100 request data.
//! A request is not a completed level load. The host must resolve the active
//! level and finish the load/reset transaction before Player::Initialise.

use super::{player_items::RobotsPlayerItemState, script_host::RobotsScriptGameplayState};
use serde::{Deserialize, Serialize};

/// Original Robots.exe 0x004DDEF0 fixed Level-record selector: 15 records
/// at 0x005F1B80 with 0xFC stride. The first dword is the shipped EDB file
/// UID, also used as native DAT_0061E840 Level UID for these entries.
/// Record +0x04 is a separate resource ID (0x450002D8..E3 / absent).
/// This is NOT a rule for arbitrary custom or development EDBs.
pub const ROBOTS_SHIPPED_LEVEL_EDB_UIDS: [u32; 15] = [
    0x0100_0012, 0x0100_001d, 0x0100_0071, 0x0100_0015,
    0x0100_0050, 0x0100_0072, 0x0100_0053, 0x0100_0073,
    0x0100_006e, 0x0100_006f, 0x0100_001a, 0x0100_00bb,
    0x0100_0001, 0x0100_000f, 0x0100_0074,
];

pub fn robots_shipped_level_edb_uid(level_uid: u32) -> Option<u32> {
    ROBOTS_SHIPPED_LEVEL_EDB_UIDS
        .iter()
        .copied()
        .find(|&edb_uid| edb_uid == level_uid)
}


/// Confirmed persistent groups. The Player 0x48 container has native group-1
/// save/restore vslots 0x0040B600 / 0x0040B630; inventory and mission groups
/// already have engine-neutral persistent state. Never serialize world XItems,
/// AI animations, map-local focus keys or Player spawn pose into this carrier:
/// the destination Player creator/TriggerManager owns them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RobotsLevelTransitionProgress {
    pub gameplay: RobotsScriptGameplayState,
    pub player_items: RobotsPlayerItemState,
    /// Native SaveRestore 0x58000027 stores Player +0x384/+0x388 as f32.
    /// Preserve the exact scalar bits, including zero, without forcing a
    /// specific maximum-health value on the destination Player.
    pub health_fraction_bits_58000027: Option<u32>,
    /// Native 0x00426FA0 writes pending +0x5E8, falling back to committed
    /// +0x5CC, as raw action UID bits under 0x58000029. No XItem pointers.
    pub selected_action_uid_58000029: Option<u32>,
    /// Independent native Player +0x590 meter stored as f32 under 0x58000028.
    pub player_scalar_bits_58000028: Option<u32>,
}

impl RobotsLevelTransitionProgress {
    /// Native 0x00426FA0 snapshots health as a fraction of max; the
    /// 0x0048BDE1..0x0048BE39 destination Player restores fraction * new max.
    /// Reject corrupt non-finite or zero-denominator host states.
    pub fn capture_health_fraction(&mut self, current: f32, max: f32) {
        self.health_fraction_bits_58000027 = if current.is_finite() && max.is_finite() && max > 0.0 {
            let fraction = current / max;
            fraction.is_finite().then_some(fraction.to_bits())
        } else {
            None
        };
    }

    /// Native Player entry 0x0048BE24 clamps the persisted +0x590 meter
    /// to [0, 100] or [0, 200] when Inventory 0x4700001E is owned.
    /// Unavailable/non-finite values retain the constructor's 100 default.
    pub fn restored_player_scalar_590(&self) -> Option<f32> {
        let value = f32::from_bits(self.player_scalar_bits_58000028?);
        if !value.is_finite() {
            return None;
        }
        let cap = if self.gameplay.inventory.stored_current(0x4700_001e) > 0 {
            200.0
        } else {
            100.0
        };
        Some(value.clamp(0.0, cap))
    }

    /// Native 0x0048BE8A..0x0048BECF checks the destination Player-item
    /// definition and positive count before requesting action vslot +0xB8.
    /// The engine-neutral host can only attach supported production classes.
    pub fn restorable_action_uid(&self) -> Option<u32> {
        use super::player_action::{robots_player_action_profile, ROBOTS_PLAYER_ACTION_48000000};
        let uid = self.selected_action_uid_58000029?;
        if uid == ROBOTS_PLAYER_ACTION_48000000
            || self.player_items.stored_current(uid) == 0
            || !robots_player_action_profile(uid)?.production_attach_proven
        {
            return None;
        }
        Some(uid)
    }

    pub fn restored_health(&self, destination_max_health: f32) -> Option<f32> {
        (destination_max_health.is_finite() && destination_max_health > 0.0)
            .then_some(())?;
        let fraction = f32::from_bits(self.health_fraction_bits_58000027?);
        if !fraction.is_finite() {
            return None;
        }
        let health = fraction * destination_max_health;
        health.is_finite().then_some(health)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RobotsChangeLevelTarget {
    Level(u32),
    CurrentLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RobotsChangeLevelRequest {
    pub target: RobotsChangeLevelTarget,
    pub player_entry_context: u32,
}

impl RobotsChangeLevelRequest {
    /// 0x0048C6A0: +0x6C is data[0] (level), +0x70 is data[1]
    /// (Player context), +0x74 bit0 requests the current level with context 0.
    /// The external DAT_007B31CC transition gate is NOT evaluated here.
    pub fn from_trigger_event(event_mask: u32, data: &[Option<u32>]) -> Option<Self> {
        if event_mask & 0x100 == 0 {
            return None;
        }
        if data.get(2).copied().flatten().unwrap_or_default() & 1 != 0 {
            return Some(Self {
                target: RobotsChangeLevelTarget::CurrentLevel,
                player_entry_context: 0,
            });
        }
        let level = data.first().copied().flatten().unwrap_or_default();
        if matches!(level, 0x0100_0000 | u32::MAX) {
            return None;
        }
        Some(Self {
            target: RobotsChangeLevelTarget::Level(level),
            player_entry_context: data.get(1).copied().flatten().unwrap_or_default(),
        })
    }

    /// Native XTrigger_ChangeLevel 0x0048C6A0 accepts event 0x100 only when
    /// DAT_007B31CC is null. That global is the ACTIVE MISSION owner pointer,
    /// proven by Mission 0x0048F340 assigning it and 0x0048F2C0 clearing it.
    pub fn from_trigger_event_when_mission_idle(
        event_mask: u32,
        data: &[Option<u32>],
        active_mission_owner: bool,
    ) -> Option<Self> {
        (!active_mission_owner)
            .then(|| Self::from_trigger_event(event_mask, data))
            .flatten()
    }

    /// Native same-level non-forced path calls the GameWnd context setter
    /// directly. Forced-current requests do a full level reset/reload.
    pub fn needs_full_level_reload(self, active_level_uid: Option<u32>) -> Option<bool> {
        let active = active_level_uid?;
        match self.target {
            RobotsChangeLevelTarget::CurrentLevel => Some(true),
            RobotsChangeLevelTarget::Level(uid) => Some(uid != active),
        }
    }

    /// No same-level comparison is possible while the live level UID is unknown.
    pub fn target_level_uid(self, active_level_uid: Option<u32>) -> Option<u32> {
        match self.target {
            RobotsChangeLevelTarget::Level(uid) => Some(uid),
            RobotsChangeLevelTarget::CurrentLevel => active_level_uid,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn change_level_decodes_exact_serialized_slots_and_event_gate() {
        let data = [Some(0x0100_0012), Some(9), Some(0)];
        assert_eq!(RobotsChangeLevelRequest::from_trigger_event(1, &data), None);
        assert_eq!(RobotsChangeLevelRequest::from_trigger_event(0x200, &data), None);
        let request = RobotsChangeLevelRequest::from_trigger_event(0x101, &data).unwrap();
        assert_eq!(request.target, RobotsChangeLevelTarget::Level(0x0100_0012));
        assert_eq!(request.player_entry_context, 9);
        assert_eq!(request.target_level_uid(None), Some(0x0100_0012));
    }

    #[test]
    fn forced_current_level_discards_serialized_context_but_does_not_infer_level() {
        let request = RobotsChangeLevelRequest::from_trigger_event(
            0x100,
            &[Some(u32::MAX), Some(99), Some(1)],
        ).unwrap();
        assert_eq!(request.target, RobotsChangeLevelTarget::CurrentLevel);
        assert_eq!(request.player_entry_context, 0);
        assert_eq!(request.target_level_uid(None), None);
        assert_eq!(request.target_level_uid(Some(0x0100_0014)), Some(0x0100_0014));
    }

    #[test]
    fn active_mission_owner_blocks_change_level_event_as_in_native() {
        let data = [Some(0x0100_0012), Some(5), Some(0)];
        assert_eq!(
            RobotsChangeLevelRequest::from_trigger_event_when_mission_idle(
                0x100, &data, true,
            ),
            None,
        );
        assert_eq!(
            RobotsChangeLevelRequest::from_trigger_event_when_mission_idle(
                0x100, &data, false,
            )
            .unwrap()
            .player_entry_context,
            5,
        );
        assert_eq!(
            RobotsChangeLevelRequest::from_trigger_event_when_mission_idle(
                0x200, &data, false,
            ),
            None,
        );
    }

    #[test]
    fn native_same_level_direct_setter_is_not_a_full_edb_reload() {
        let level = 0x0100_0012;
        let direct = RobotsChangeLevelRequest::from_trigger_event(
            0x100, &[Some(level), Some(9), Some(0)],
        )
        .unwrap();
        assert_eq!(direct.needs_full_level_reload(Some(level)), Some(false));
        assert_eq!(
            direct.needs_full_level_reload(Some(0x0100_0013)),
            Some(true),
        );
        assert_eq!(direct.needs_full_level_reload(None), None);
        let forced = RobotsChangeLevelRequest::from_trigger_event(
            0x100, &[Some(level), Some(9), Some(1)],
        )
        .unwrap();
        assert_eq!(forced.needs_full_level_reload(Some(level)), Some(true));
    }

    #[test]
    fn shipped_level_catalog_resolves_exact_native_edb_identity() {
        use std::collections::HashSet;
        assert_eq!(ROBOTS_SHIPPED_LEVEL_EDB_UIDS.len(), 15);
        assert_eq!(ROBOTS_SHIPPED_LEVEL_EDB_UIDS.iter().collect::<HashSet<_>>().len(), 15);
        for &uid in &ROBOTS_SHIPPED_LEVEL_EDB_UIDS {
            assert_eq!(robots_shipped_level_edb_uid(uid), Some(uid));
        }
        assert_eq!(robots_shipped_level_edb_uid(0x0100_0012), Some(0x0100_0012));
        assert_eq!(robots_shipped_level_edb_uid(0x0100_001d), Some(0x0100_001d));
        assert_eq!(robots_shipped_level_edb_uid(0x0100_0074), Some(0x0100_0074));
        assert_eq!(robots_shipped_level_edb_uid(0x4500_02d8), None);
        assert_eq!(robots_shipped_level_edb_uid(0x0100_00d9), None);
        assert_eq!(robots_shipped_level_edb_uid(0), None);
    }

    #[test]
    fn native_player_health_fraction_survives_level_entry_with_new_max_health() {
        let mut progress = RobotsLevelTransitionProgress::default();
        progress.capture_health_fraction(35.0, 100.0);
        assert_eq!(progress.health_fraction_bits_58000027, Some(0.35f32.to_bits()));
        assert!((progress.restored_health(100.0).unwrap() - 35.0).abs() < 0.0001);
        assert!((progress.restored_health(200.0).unwrap() - 70.0).abs() < 0.0001);

        progress.capture_health_fraction(0.0, 100.0);
        assert_eq!(progress.restored_health(175.0), Some(0.0));
        progress.capture_health_fraction(35.0, 0.0);
        assert_eq!(progress.health_fraction_bits_58000027, None);
        assert_eq!(progress.restored_health(200.0), None);
        progress.capture_health_fraction(f32::NAN, 100.0);
        assert_eq!(progress.restored_health(100.0), None);
        progress.capture_health_fraction(50.0, 100.0);
        assert_eq!(progress.restored_health(f32::INFINITY), None);
    }

    #[test]
    fn native_player_scalar_28_restores_with_inventory_dependent_bounds() {
        use super::super::inventory::RobotsInventoryValue;
        use super::super::player_action::RobotsPlayerActionRuntime;
        assert_eq!(
            f32::from_bits(RobotsPlayerActionRuntime::default().player_scalar_590_bits),
            100.0,
        );
        let mut progress = RobotsLevelTransitionProgress::default();
        progress.player_scalar_bits_58000028 = Some(180.0f32.to_bits());
        assert_eq!(progress.restored_player_scalar_590(), Some(100.0));
        progress.gameplay.inventory.values.push(RobotsInventoryValue {
            uid: 0x4700_001e,
            current: 1,
        });
        assert_eq!(progress.restored_player_scalar_590(), Some(180.0));
        progress.player_scalar_bits_58000028 = Some(300.0f32.to_bits());
        assert_eq!(progress.restored_player_scalar_590(), Some(200.0));
        progress.player_scalar_bits_58000028 = Some((-10.0f32).to_bits());
        assert_eq!(progress.restored_player_scalar_590(), Some(0.0));
        progress.player_scalar_bits_58000028 = Some(f32::NAN.to_bits());
        assert_eq!(progress.restored_player_scalar_590(), None);
        progress.player_scalar_bits_58000028 = None;
        assert_eq!(progress.restored_player_scalar_590(), None);
    }

    #[test]
    fn saved_player_action_29_restores_only_owned_implemented_weapon() {
        use super::super::player_action::{
            RobotsPlayerActionRequestStatus, RobotsPlayerActionRuntime,
            ROBOTS_PLAYER_ACTION_SCRAP_GUN, ROBOTS_PLAYER_ACTION_48000003,
        };
        let mut progress = RobotsLevelTransitionProgress::default();
        progress.selected_action_uid_58000029 = Some(ROBOTS_PLAYER_ACTION_SCRAP_GUN);
        assert_eq!(progress.restorable_action_uid(), None, "unowned gun must not spawn");
        assert!(progress.player_items.seed_profile_value(ROBOTS_PLAYER_ACTION_SCRAP_GUN, 1));
        assert_eq!(progress.restorable_action_uid(), Some(ROBOTS_PLAYER_ACTION_SCRAP_GUN));
        let mut destination = RobotsPlayerActionRuntime::default();
        let outcome = destination.request_cutscene_action(
            progress.restorable_action_uid().unwrap(), 2, |uid| progress.player_items.stored_current(uid) as i32,
        );
        assert_eq!(outcome.status, RobotsPlayerActionRequestStatus::Attached);
        assert_eq!(destination.committed_action_uid_5cc, ROBOTS_PLAYER_ACTION_SCRAP_GUN);
        progress.selected_action_uid_58000029 = Some(ROBOTS_PLAYER_ACTION_48000003);
        assert!(progress.player_items.seed_profile_value(ROBOTS_PLAYER_ACTION_48000003, 1));
        assert_eq!(progress.restorable_action_uid(), None, "unimplemented native action must fail closed");
        progress.selected_action_uid_58000029 = Some(u32::MAX);
        assert_eq!(progress.restorable_action_uid(), None);
    }

    #[test]
    fn invalid_level_sentinels_are_rejected_without_force_current() {
        for uid in [0x0100_0000, u32::MAX] {
            assert_eq!(
                RobotsChangeLevelRequest::from_trigger_event(0x100, &[Some(uid), Some(1), Some(0)]),
                None
            );
        }
        // The base loader zero-fills an absent slot; zero isn't a native sentinel.
        assert_eq!(
            RobotsChangeLevelRequest::from_trigger_event(0x100, &[]).unwrap().target,
            RobotsChangeLevelTarget::Level(0)
        );
    }
}
