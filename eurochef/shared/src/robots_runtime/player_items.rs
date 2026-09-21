use serde::Serialize;

pub const ROBOTS_PLAYER_ITEM_NAMESPACE: u32 = 0x4800_0000;
pub const ROBOTS_PLAYER_ITEM_PERSISTENT_GROUP_ID: u32 = 1;
pub const ROBOTS_PLAYER_ITEM_PERSISTENT_RECORD_SIZE: u32 = 8;
pub const ROBOTS_PLAYER_ITEM_WATCHBOT_UPGRADE: u32 = 0x4800_0001;
pub const ROBOTS_PLAYER_ITEM_WATCHBOT_CONTROLLER: u32 = 0x4800_0008;
pub const ROBOTS_PLAYER_ITEM_GATE_AGGREGATE: u32 = 0x4800_0009;
pub const ROBOTS_PLAYER_ITEM_GATE_FIRST: u32 = 0x4800_000e;
pub const ROBOTS_PLAYER_ITEM_GATE_LAST: u32 = 0x4800_001d;
pub const ROBOTS_PLAYER_ITEM_SCRAP_GUN_EXTRA_DAMAGE: u32 = 0x4800_0021;
pub const ROBOTS_PLAYER_ITEM_SCRAP_GUN_SPRAY: u32 = 0x4800_0022;
pub const ROBOTS_PLAYER_ITEM_SCRAP_GUN_HOMING: u32 = 0x4800_0023;
pub const ROBOTS_PLAYER_ITEM_SCRAP_GUN_RICOCHET: u32 = 0x4800_0024;
pub const ROBOTS_PLAYER_ITEM_SCRAP_GUN_DEFAULT: u32 = 0x4800_0025;
pub const ROBOTS_PLAYER_ITEM_SCRAP_GUN_FIRST: u32 = ROBOTS_PLAYER_ITEM_SCRAP_GUN_EXTRA_DAMAGE;
pub const ROBOTS_PLAYER_ITEM_SCRAP_GUN_LAST: u32 = ROBOTS_PLAYER_ITEM_SCRAP_GUN_RICOCHET;
pub const ROBOTS_PLAYER_ITEM_CUTSCENE_PROPERTY: u32 = 0x4800_0026;

pub fn is_robots_player_item_uid(uid: u32) -> bool {
    uid != u32::MAX && uid & 0x7f00_0000 == ROBOTS_PLAYER_ITEM_NAMESPACE
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsPlayerItemValue {
    pub uid: u32,
    pub raw_value: u32,
}

impl RobotsPlayerItemValue {
    pub fn current(self) -> u8 {
        self.raw_value as u8
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RobotsPlayerItemPersistentGroup {
    pub group_id: u32,
    pub record_size: u32,
    pub records: Vec<RobotsPlayerItemValue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsPlayerItemMutationOutcome {
    pub uid: u32,
    pub previous: u8,
    pub current: u8,
}

impl RobotsPlayerItemMutationOutcome {
    pub fn changed(self) -> bool {
        self.previous != self.current
    }
}

/// Engine-neutral projection of `XItemHandler_Player +0x4A4`.
///
/// Native Player vslot `+0xB8 = 0x004AFD20` routes namespace `0x48` to this
/// embedded container. Its `+0x14 = 0x0040B730` ordinary path searches
/// `{uid, raw_u32}` pairs and returns the low byte when the pair is present,
/// or zero when absent. Persistent vslot `+0x08 = 0x0040A620` reports an
/// eight-byte record size, `+0x0C = 0x0040B600` serializes each pair, and
/// `+0x10 = 0x0040B630` appends restored pairs in stream order. `+0x30 =
/// 0x0040B900` mutates the low byte while `+0x40 = 0x0040BCD0` can remove the
/// same stored pair.
///
/// Definition-specific external hooks remain a host/database concern. This
/// state therefore stores only values that have entered through a proved
/// Player-item producer or the explicit persistent group1 restore boundary.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct RobotsPlayerItemState {
    pub values: Vec<RobotsPlayerItemValue>,
}

impl RobotsPlayerItemState {
    pub fn clear(&mut self) {
        self.values.clear();
    }

    fn value_index(&self, uid: u32) -> Option<usize> {
        self.values.iter().position(|value| value.uid == uid)
    }

    pub fn stored_current(&self, uid: u32) -> u8 {
        self.value_index(uid)
            .map(|index| self.values[index].current())
            .unwrap_or(0)
    }

    pub fn is_enabled(&self, uid: u32) -> bool {
        self.stored_current(uid) != 0
    }

    /// Seed one host-resolved Player item value. This convenience boundary
    /// preserves the native low byte exactly; it does not booleanize it.
    /// Full native restore should use `restore_persistent_records` instead.
    pub fn seed_profile_value(&mut self, uid: u32, current: u8) -> bool {
        if !is_robots_player_item_uid(uid) {
            return false;
        }
        self.set_low_byte(uid, current);
        true
    }

    /// Exact `+0x2C = 0x0040B860` preflight for the Player `0x48` family.
    /// Native returns 2 when the definition is absent, 3 when the stored flag
    /// is already positive, otherwise 4 for a positive requested quantity and
    /// 1 for a non-positive quantity. Shop treats result 1 or 4 as purchasable.
    pub fn preflight_add_result(&self, uid: u32, quantity: i32, definition_exists: bool) -> u32 {
        if !definition_exists || !is_robots_player_item_uid(uid) {
            return 2;
        }
        if self.stored_current(uid) >= 1 {
            return 3;
        }
        if quantity >= 1 {
            4
        } else {
            1
        }
    }

    /// Exact ordinary `+0x30 = 0x0040B900` mutation result for the Player
    /// `0x48` flag container. `+0x5C = 0x0040A650` is an unconditional allow
    /// hook. Native promotes quantity <1 to one and clamps the stored low byte
    /// to one. It returns 2 for a missing definition, 3 for an already-active
    /// pair, 4 when creating a new pair and 1 when re-enabling an existing
    /// zero-valued pair.
    pub fn add_with_native_result(
        &mut self,
        uid: u32,
        _quantity: i32,
        definition_exists: bool,
    ) -> u32 {
        if !definition_exists || !is_robots_player_item_uid(uid) {
            return 2;
        }
        let existing_index = self.value_index(uid);
        if existing_index.is_some_and(|index| self.values[index].current() >= 1) {
            return 3;
        }
        let result = if existing_index.is_some() { 1 } else { 4 };
        self.set_low_byte(uid, 1);
        self.apply_native_definition_callback(uid);
        result
    }

    /// Convenience ingress for already-resolved Player-item producers.
    pub fn add_or_enable(&mut self, uid: u32, current: u8) -> bool {
        if !is_robots_player_item_uid(uid) {
            return false;
        }
        self.set_low_byte(uid, current.max(1));
        self.apply_native_definition_callback(uid);
        true
    }

    /// Exact Player persistent group1 restore boundary used by
    /// `0x004D4770 -> 0x00428130 -> container +0x10`. Native clears the live
    /// container, appends each eight-byte pair in stream order, then invokes
    /// definition callback `+0x50 = 0x004DA5C0` for that restored row. The
    /// ScrapGun callback makes `0x48000021..24` mutually exclusive and lets
    /// `0x48000025` clear the whole selector family.
    pub fn restore_persistent_records(&mut self, records: &[RobotsPlayerItemValue]) {
        self.values.clear();
        for record in records {
            self.values.push(*record);
            self.apply_native_definition_callback(record.uid);
        }
    }

    pub fn persistent_records(&self) -> &[RobotsPlayerItemValue] {
        &self.values
    }

    /// Native `0x004D1180 -> 0x00427FE0` capture of Player group1. The
    /// persistence registry stores the container-reported record size beside
    /// the group payload and checks it again before restore.
    pub fn capture_persistent_group(&self) -> RobotsPlayerItemPersistentGroup {
        RobotsPlayerItemPersistentGroup {
            group_id: ROBOTS_PLAYER_ITEM_PERSISTENT_GROUP_ID,
            record_size: ROBOTS_PLAYER_ITEM_PERSISTENT_RECORD_SIZE,
            records: self.values.clone(),
        }
    }

    /// Native `0x004D4770 -> 0x00428130` restore. Group lookup is by id and
    /// `0x00428130` refuses to call container `+0x10` when its stored record
    /// size differs from the live container `+0x08` result.
    pub fn restore_persistent_group(&mut self, group: &RobotsPlayerItemPersistentGroup) -> bool {
        if group.group_id != ROBOTS_PLAYER_ITEM_PERSISTENT_GROUP_ID
            || group.record_size != ROBOTS_PLAYER_ITEM_PERSISTENT_RECORD_SIZE
        {
            return false;
        }
        self.restore_persistent_records(&group.records);
        true
    }

    /// Exact state projection needed by Cutscene `SetPropertiesPlayer`: native
    /// arg0==1 calls `+0x30(0x48000026,1)`; any other concrete arg0 calls
    /// `+0x40(0x48000026,1)`.
    pub fn set_cutscene_property_enabled(
        &mut self,
        enabled: bool,
    ) -> RobotsPlayerItemMutationOutcome {
        let uid = ROBOTS_PLAYER_ITEM_CUTSCENE_PROPERTY;
        let previous = self.stored_current(uid);
        let current = u8::from(enabled);
        if enabled {
            self.add_or_enable(uid, current);
        } else {
            self.remove_first(uid);
        }
        RobotsPlayerItemMutationOutcome {
            uid,
            previous,
            current,
        }
    }

    fn set_low_byte(&mut self, uid: u32, current: u8) {
        match self.value_index(uid) {
            Some(index) => {
                self.values[index].raw_value =
                    (self.values[index].raw_value & 0xffff_ff00) | u32::from(current);
            }
            None => self.values.push(RobotsPlayerItemValue {
                uid,
                raw_value: u32::from(current),
            }),
        }
    }

    fn remove_first(&mut self, uid: u32) {
        if let Some(index) = self.value_index(uid) {
            self.values.remove(index);
        }
    }

    /// Player-item definition callback `+0x50 = 0x004DA5C0`. Native first maps
    /// the 16 gate items `0x4800000E..0x4800001D` to aggregate `0x48000009`,
    /// then handles the independent ScrapGun selector family. The native gate
    /// branch also calls `0x0049C3C0(6)`; that presentation/refresh side effect
    /// remains a host concern while the gameplay-visible aggregate flag is owned here.
    fn apply_native_definition_callback(&mut self, uid: u32) {
        if (ROBOTS_PLAYER_ITEM_GATE_FIRST..=ROBOTS_PLAYER_ITEM_GATE_LAST).contains(&uid) {
            self.set_low_byte(ROBOTS_PLAYER_ITEM_GATE_AGGREGATE, 1);
            return;
        }
        if uid == ROBOTS_PLAYER_ITEM_SCRAP_GUN_DEFAULT {
            for selector in ROBOTS_PLAYER_ITEM_SCRAP_GUN_FIRST..=ROBOTS_PLAYER_ITEM_SCRAP_GUN_LAST {
                self.remove_first(selector);
            }
            self.remove_first(ROBOTS_PLAYER_ITEM_SCRAP_GUN_DEFAULT);
            return;
        }
        if (ROBOTS_PLAYER_ITEM_SCRAP_GUN_FIRST..=ROBOTS_PLAYER_ITEM_SCRAP_GUN_LAST).contains(&uid) {
            for selector in ROBOTS_PLAYER_ITEM_SCRAP_GUN_FIRST..=ROBOTS_PLAYER_ITEM_SCRAP_GUN_LAST {
                if selector != uid {
                    self.remove_first(selector);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespace_router_matches_native_player_slot_b8_split() {
        assert!(is_robots_player_item_uid(0x4800_0021));
        assert!(is_robots_player_item_uid(0x4800_0026));
        assert!(!is_robots_player_item_uid(0x4700_0021));
        assert!(!is_robots_player_item_uid(u32::MAX));
    }

    #[test]
    fn absent_player_item_reads_zero_like_native_0040b730_pair_lookup() {
        let state = RobotsPlayerItemState::default();
        assert_eq!(state.stored_current(0x4800_0021), 0);
        assert!(!state.is_enabled(0x4800_0021));
    }

    #[test]
    fn profile_seed_preserves_the_native_low_byte_instead_of_booleanizing_it() {
        let mut state = RobotsPlayerItemState::default();
        assert!(state.seed_profile_value(0x4800_0023, 9));
        assert_eq!(state.stored_current(0x4800_0023), 9);
        assert!(!state.seed_profile_value(0x4700_0001, 1));
        assert_eq!(state.stored_current(0x4700_0001), 0);
    }

    #[test]
    fn persistent_group1_restore_preserves_raw_dwords_duplicates_and_order() {
        let records = [
            RobotsPlayerItemValue {
                uid: 0x4800_0022,
                raw_value: 0xaabb_cc09,
            },
            RobotsPlayerItemValue {
                uid: 0x4800_0022,
                raw_value: 0x1122_3301,
            },
        ];
        let mut state = RobotsPlayerItemState::default();
        state.restore_persistent_records(&records);
        assert_eq!(state.persistent_records(), &records);
        assert_eq!(state.stored_current(0x4800_0022), 9);
        assert_eq!(ROBOTS_PLAYER_ITEM_PERSISTENT_GROUP_ID, 1);
        assert_eq!(ROBOTS_PLAYER_ITEM_PERSISTENT_RECORD_SIZE, 8);
    }

    #[test]
    fn persistent_group_capture_and_restore_match_native_group1_size_gate() {
        let mut source = RobotsPlayerItemState::default();
        source.restore_persistent_records(&[
            RobotsPlayerItemValue {
                uid: 0x4800_0021,
                raw_value: 0xdead_be01,
            },
            RobotsPlayerItemValue {
                uid: ROBOTS_PLAYER_ITEM_CUTSCENE_PROPERTY,
                raw_value: 0x1234_5602,
            },
        ]);
        let snapshot = source.capture_persistent_group();
        assert_eq!(snapshot.group_id, 1);
        assert_eq!(snapshot.record_size, 8);

        let mut restored = RobotsPlayerItemState::default();
        assert!(restored.restore_persistent_group(&snapshot));
        assert_eq!(restored.persistent_records(), source.persistent_records());

        let mut wrong_size = snapshot.clone();
        wrong_size.record_size = 4;
        assert!(!restored.restore_persistent_group(&wrong_size));
        assert_eq!(restored.persistent_records(), source.persistent_records());
    }

    #[test]
    fn native_player_item_add_preflight_and_commit_match_shop_success_codes() {
        let mut state = RobotsPlayerItemState::default();
        assert_eq!(state.preflight_add_result(0x4800_0021, 1, false), 2);
        assert_eq!(state.preflight_add_result(0x4700_0021, 1, true), 2);
        assert_eq!(state.preflight_add_result(0x4800_0021, 0, true), 1);
        assert_eq!(state.preflight_add_result(0x4800_0021, 1, true), 4);
        assert_eq!(state.add_with_native_result(0x4800_0021, 1, true), 4);
        assert_eq!(state.preflight_add_result(0x4800_0021, 1, true), 3);
        assert_eq!(state.add_with_native_result(0x4800_0021, 1, true), 3);

        state.seed_profile_value(0x4800_0022, 0);
        assert_eq!(state.preflight_add_result(0x4800_0022, 1, true), 4);
        assert_eq!(state.add_with_native_result(0x4800_0022, 1, true), 1);
        assert_eq!(state.stored_current(0x4800_0022), 1);
    }

    #[test]
    fn gate_item_callback_enables_native_48000009_aggregate() {
        let mut state = RobotsPlayerItemState::default();
        assert_eq!(
            state.add_with_native_result(ROBOTS_PLAYER_ITEM_GATE_FIRST, 1, true),
            4
        );
        assert_eq!(state.stored_current(ROBOTS_PLAYER_ITEM_GATE_FIRST), 1);
        assert_eq!(state.stored_current(ROBOTS_PLAYER_ITEM_GATE_AGGREGATE), 1);

        let mut restored = RobotsPlayerItemState::default();
        restored.restore_persistent_records(&[RobotsPlayerItemValue {
            uid: ROBOTS_PLAYER_ITEM_GATE_LAST,
            raw_value: 1,
        }]);
        assert_eq!(restored.stored_current(ROBOTS_PLAYER_ITEM_GATE_LAST), 1);
        assert_eq!(
            restored.stored_current(ROBOTS_PLAYER_ITEM_GATE_AGGREGATE),
            1
        );
    }

    #[test]
    fn scrapgun_upgrade_add_and_restore_keep_only_the_latest_concrete_selector() {
        let mut added = RobotsPlayerItemState::default();
        assert!(added.add_or_enable(ROBOTS_PLAYER_ITEM_SCRAP_GUN_EXTRA_DAMAGE, 1));
        assert!(added.add_or_enable(ROBOTS_PLAYER_ITEM_SCRAP_GUN_HOMING, 1));
        assert_eq!(
            added.stored_current(ROBOTS_PLAYER_ITEM_SCRAP_GUN_EXTRA_DAMAGE),
            0
        );
        assert_eq!(added.stored_current(ROBOTS_PLAYER_ITEM_SCRAP_GUN_HOMING), 1);
        assert_eq!(added.values.len(), 1);

        let mut restored = RobotsPlayerItemState::default();
        restored.restore_persistent_records(&[
            RobotsPlayerItemValue {
                uid: ROBOTS_PLAYER_ITEM_SCRAP_GUN_SPRAY,
                raw_value: 1,
            },
            RobotsPlayerItemValue {
                uid: ROBOTS_PLAYER_ITEM_SCRAP_GUN_RICOCHET,
                raw_value: 1,
            },
        ]);
        assert_eq!(
            restored.stored_current(ROBOTS_PLAYER_ITEM_SCRAP_GUN_SPRAY),
            0
        );
        assert_eq!(
            restored.stored_current(ROBOTS_PLAYER_ITEM_SCRAP_GUN_RICOCHET),
            1
        );
        assert_eq!(restored.values.len(), 1);
    }

    #[test]
    fn scrapgun_default_callback_clears_all_concrete_selectors_and_itself() {
        let mut state = RobotsPlayerItemState::default();
        assert!(state.add_or_enable(ROBOTS_PLAYER_ITEM_SCRAP_GUN_SPRAY, 1));
        assert!(state.add_or_enable(ROBOTS_PLAYER_ITEM_SCRAP_GUN_DEFAULT, 1));
        for uid in ROBOTS_PLAYER_ITEM_SCRAP_GUN_FIRST..=ROBOTS_PLAYER_ITEM_SCRAP_GUN_LAST {
            assert_eq!(state.stored_current(uid), 0);
        }
        assert_eq!(
            state.stored_current(ROBOTS_PLAYER_ITEM_SCRAP_GUN_DEFAULT),
            0
        );
        assert!(state.values.is_empty());
    }

    #[test]
    fn cutscene_setpropertiesplayer_preserves_upper_restore_bits_then_removes_pair() {
        let mut state = RobotsPlayerItemState::default();
        state.restore_persistent_records(&[RobotsPlayerItemValue {
            uid: ROBOTS_PLAYER_ITEM_CUTSCENE_PROPERTY,
            raw_value: 0xaabb_cc00,
        }]);

        let enabled = state.set_cutscene_property_enabled(true);
        assert!(enabled.changed());
        assert_eq!(enabled.uid, ROBOTS_PLAYER_ITEM_CUTSCENE_PROPERTY);
        assert_eq!(
            state.stored_current(ROBOTS_PLAYER_ITEM_CUTSCENE_PROPERTY),
            1
        );
        assert_eq!(state.values[0].raw_value, 0xaabb_cc01);

        let enabled_again = state.set_cutscene_property_enabled(true);
        assert!(!enabled_again.changed());
        assert_eq!(state.values[0].raw_value, 0xaabb_cc01);

        let disabled = state.set_cutscene_property_enabled(false);
        assert!(disabled.changed());
        assert_eq!(
            state.stored_current(ROBOTS_PLAYER_ITEM_CUTSCENE_PROPERTY),
            0
        );
        assert!(state.values.is_empty());
    }

    #[test]
    fn scrapgun_selector_flags_share_the_same_player_item_namespace() {
        let mut state = RobotsPlayerItemState::default();
        for uid in ROBOTS_PLAYER_ITEM_SCRAP_GUN_FIRST..=ROBOTS_PLAYER_ITEM_SCRAP_GUN_LAST {
            assert!(state.seed_profile_value(uid, 1));
            assert_eq!(state.stored_current(uid), 1);
        }
    }
}
