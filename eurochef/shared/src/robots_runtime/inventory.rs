use std::io::{Seek, SeekFrom};

use anyhow::Context;
use eurochef_edb::{binrw::BinReaderExt, edb::EdbFile};
use serde::Serialize;

use crate::spreadsheets::UXGeoSpreadsheet;

pub const ROBOTS_INVENTORY_FILE_UID: u32 = 0x0100_0003;
pub const ROBOTS_INVENTORY_SPREADSHEET_UID: u32 = 0x1400_0002;
pub const ROBOTS_INVENTORY_UID_BASE: u32 = 0x4700_0000;
pub const ROBOTS_SCRAP_UID: u32 = 0x4700_0001;
pub const ROBOTS_ENERGY_CELL_UID: u32 = 0x4700_0003;
pub const ROBOTS_HEALTH_REPLENISH_UID: u32 = 0x4700_0023;
pub const ROBOTS_SCRAP_CAP_500_UPGRADE_UID: u32 = 0x4700_0018;
pub const ROBOTS_SCRAP_CAP_1000_UPGRADE_UID: u32 = 0x4700_0019;
pub const ROBOTS_SCRAP_DOUBLE_VALUE_UPGRADE_UID: u32 = 0x4700_001c;

pub fn is_robots_inventory_uid(uid: u32) -> bool {
    uid != u32::MAX && uid & 0x7f00_0000 == ROBOTS_INVENTORY_UID_BASE
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsInventoryValue {
    pub uid: u32,
    pub current: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsInventoryMutationOutcome {
    pub native_result_code: u32,
    pub previous: Option<i32>,
    pub current: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct RobotsInventoryState {
    pub values: Vec<RobotsInventoryValue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsInventoryDefinition {
    pub uid: u32,
    /// Native words at record offsets +0x04..+0x14. Their individual semantics
    /// are intentionally left unnamed until proven by consumers.
    pub words_04_to_14: [u32; 5],
    pub target_word_18: u32,
}

impl RobotsInventoryDefinition {
    /// Native `0x004D4A80` copies `HT_SpreadSheet_Inventory` as fixed 0x1C-byte
    /// rows and removes rows whose first dword is `0xFFFFFFFF`.
    pub fn from_native_words(words: [u32; 7]) -> Option<Self> {
        (words[0] != u32::MAX).then_some(Self {
            uid: words[0],
            words_04_to_14: [words[1], words[2], words[3], words[4], words[5]],
            target_word_18: words[6],
        })
    }

    pub fn is_bitset(self) -> bool {
        self.words_04_to_14[4] & 1 != 0
    }

    pub fn uses_external_current(self) -> bool {
        matches!(
            self.uid,
            ROBOTS_ENERGY_CELL_UID | ROBOTS_HEALTH_REPLENISH_UID
        )
    }

    pub fn has_native_cap(self) -> bool {
        decode_inventory_target_word(self.target_word_18) > 0 && self.words_04_to_14[4] & 2 == 0
    }

    /// Native vslot +0x18 helper `0x004DA0B0` normally returns the signed
    /// low 16 bits of record +0x18. Scrap is the only recovered override:
    /// upgrades 0x47000019 and 0x47000018 raise its cap to 1000 or 500.
    pub fn target(self, cap_1000_upgrade_count: i32, cap_500_upgrade_count: i32) -> i32 {
        if self.uid == ROBOTS_SCRAP_UID {
            if cap_1000_upgrade_count > 0 {
                return 1000;
            }
            if cap_500_upgrade_count > 0 {
                return 500;
            }
        }
        decode_inventory_target_word(self.target_word_18)
    }
}

pub fn read_robots_inventory_definitions(
    edb: &mut EdbFile,
) -> anyhow::Result<Vec<RobotsInventoryDefinition>> {
    anyhow::ensure!(
        edb.header.hashcode == ROBOTS_INVENTORY_FILE_UID,
        "expected Robots inventory EDB 0x{ROBOTS_INVENTORY_FILE_UID:08X}, got 0x{:08X}",
        edb.header.hashcode
    );
    let spreadsheets = UXGeoSpreadsheet::read_all(edb)?;
    let sheet = spreadsheets
        .iter()
        .find_map(|(uid, spreadsheet)| {
            (*uid == ROBOTS_INVENTORY_SPREADSHEET_UID)
                .then_some(spreadsheet)
                .and_then(|spreadsheet| match spreadsheet {
                    UXGeoSpreadsheet::Data(sheets) => sheets.first(),
                    UXGeoSpreadsheet::Text(_) => None,
                })
        })
        .context("missing HT_SpreadSheet_Inventory sheet #0")?;
    edb.seek(SeekFrom::Start(sheet.address as u64))?;
    let mut definitions = Vec::with_capacity(sheet.row_count as usize);
    for _ in 0..sheet.row_count {
        let words = [
            edb.read_type(edb.endian)?,
            edb.read_type(edb.endian)?,
            edb.read_type(edb.endian)?,
            edb.read_type(edb.endian)?,
            edb.read_type(edb.endian)?,
            edb.read_type(edb.endian)?,
            edb.read_type(edb.endian)?,
        ];
        if let Some(definition) = RobotsInventoryDefinition::from_native_words(words) {
            definitions.push(definition);
        }
    }
    Ok(definitions)
}

impl RobotsInventoryState {
    pub fn clear(&mut self) {
        self.values.clear();
    }

    fn value_index(&self, uid: u32) -> Option<usize> {
        self.values.iter().position(|value| value.uid == uid)
    }

    /// For ordinary family-0x47 values, native `0x0040A8A0` synthesizes
    /// `{uid,0}` when no runtime pair exists, so the observable current is 0.
    pub fn stored_current(&self, uid: u32) -> i32 {
        self.value_index(uid)
            .map(|index| self.values[index].current)
            .unwrap_or(0)
    }

    pub fn effective_target(&self, definition: RobotsInventoryDefinition) -> i32 {
        definition.target(
            self.stored_current(ROBOTS_SCRAP_CAP_1000_UPGRADE_UID),
            self.stored_current(ROBOTS_SCRAP_CAP_500_UPGRADE_UID),
        )
    }

    /// Non-mutating shop/UI preflight. Reuses the exact mutation reducer on a
    /// tiny cloned state so preview result codes cannot drift from commit rules.
    pub fn preview_add(
        &self,
        definition: Option<RobotsInventoryDefinition>,
        argument: i32,
    ) -> RobotsInventoryMutationOutcome {
        let mut preview = self.clone();
        preview.add(definition, argument)
    }

    pub fn add(
        &mut self,
        definition: Option<RobotsInventoryDefinition>,
        argument: i32,
    ) -> RobotsInventoryMutationOutcome {
        let Some(definition) = definition else {
            return outcome(2, None, None);
        };
        if definition.is_bitset() {
            return self.add_bit(definition, argument);
        }

        let mut amount = argument.max(1);
        if definition.uid == ROBOTS_SCRAP_UID
            && self.stored_current(ROBOTS_SCRAP_DOUBLE_VALUE_UPGRADE_UID) > 0
        {
            amount = amount.wrapping_mul(2);
        }

        let target = self.effective_target(definition);
        let capped = target > 0 && definition.words_04_to_14[4] & 2 == 0;
        if let Some(index) = self.value_index(definition.uid) {
            let previous = self.values[index].current;
            if capped && previous >= target {
                return outcome(3, Some(previous), Some(previous));
            }
            let mut current = previous.wrapping_add(amount);
            if capped && current > target {
                current = target;
                self.values[index].current = current;
                return outcome(4, Some(previous), Some(current));
            }
            self.values[index].current = current;
            return outcome(1, Some(previous), Some(current));
        }

        let previous = None;
        let mut current = amount;
        if capped && current > target {
            current = target;
        }
        self.values.push(RobotsInventoryValue {
            uid: definition.uid,
            current,
        });
        let result = if capped && current == target { 4 } else { 1 };
        outcome(result, previous, Some(current))
    }

    fn add_bit(
        &mut self,
        definition: RobotsInventoryDefinition,
        bit_index: i32,
    ) -> RobotsInventoryMutationOutcome {
        let target = self.effective_target(definition);
        if !(0..32).contains(&bit_index) || bit_index >= target {
            return outcome(0, None, None);
        }
        let mask = (1u32 << bit_index as u32) as i32;
        if let Some(index) = self.value_index(definition.uid) {
            let previous = self.values[index].current;
            let current = previous | mask;
            self.values[index].current = current;
            return outcome(1, Some(previous), Some(current));
        }
        self.values.push(RobotsInventoryValue {
            uid: definition.uid,
            current: mask,
        });
        outcome(1, None, Some(mask))
    }

    /// Native generic InventoryRemove with an explicit positive quantity uses
    /// vslot +0x40. `scrap_removal_locked` models the sole Player +0x60 veto;
    /// the veto reports success while deliberately leaving Scrap unchanged.
    pub fn remove_quantity(
        &mut self,
        definition: Option<RobotsInventoryDefinition>,
        quantity_or_bit: i32,
        scrap_removal_locked: bool,
    ) -> RobotsInventoryMutationOutcome {
        let Some(definition) = definition else {
            return outcome(2, None, None);
        };
        if definition.uid == ROBOTS_SCRAP_UID && scrap_removal_locked {
            let current = self
                .value_index(definition.uid)
                .map(|i| self.values[i].current);
            return outcome(1, current, current);
        }
        if definition.is_bitset() {
            return self.remove_bit(definition, quantity_or_bit);
        }

        let Some(index) = self.value_index(definition.uid) else {
            return outcome(5, None, None);
        };
        let previous = self.values[index].current;
        let quantity = if quantity_or_bit < 0 {
            previous
        } else {
            quantity_or_bit
        };
        if quantity < previous {
            let current = previous.wrapping_sub(quantity);
            self.values[index].current = current;
            outcome(1, Some(previous), Some(current))
        } else {
            self.values.remove(index);
            outcome(6, Some(previous), None)
        }
    }

    fn remove_bit(
        &mut self,
        definition: RobotsInventoryDefinition,
        bit_index: i32,
    ) -> RobotsInventoryMutationOutcome {
        let Some(index) = self.value_index(definition.uid) else {
            return outcome(5, None, None);
        };
        if !(0..32).contains(&bit_index) {
            let previous = self.values[index].current;
            return outcome(0, Some(previous), Some(previous));
        }
        let previous = self.values[index].current;
        let mask = !((1u32 << bit_index as u32) as i32);
        let current = previous & mask;
        self.values[index].current = current;
        outcome(1, Some(previous), Some(current))
    }

    /// Native generic InventoryRemove with quantity < 1 routes to vslot +0x48
    /// and physically removes the whole 8-byte `{uid,current}` runtime pair.
    pub fn remove_all(&mut self, uid: u32) -> RobotsInventoryMutationOutcome {
        let Some(index) = self.value_index(uid) else {
            return outcome(0, None, None);
        };
        let previous = self.values[index].current;
        self.values.remove(index);
        outcome(1, Some(previous), None)
    }
}

fn outcome(
    native_result_code: u32,
    previous: Option<i32>,
    current: Option<i32>,
) -> RobotsInventoryMutationOutcome {
    RobotsInventoryMutationOutcome {
        native_result_code,
        previous,
        current,
    }
}

pub fn decode_inventory_target_word(target_word_18: u32) -> i32 {
    (target_word_18 as u16 as i16) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_row_matches_native_1c_byte_contract() {
        let row =
            RobotsInventoryDefinition::from_native_words([0x4700_0012, 1, 2, 3, 4, 5, 0xabcd_0007])
                .unwrap();
        assert_eq!(row.uid, 0x4700_0012);
        assert_eq!(row.words_04_to_14, [1, 2, 3, 4, 5]);
        assert_eq!(row.target(0, 0), 7);
        assert_eq!(
            RobotsInventoryDefinition::from_native_words([u32::MAX, 1, 2, 3, 4, 5, 6]),
            None
        );
    }

    #[test]
    fn target_preserves_signed_short_and_scrap_upgrade_override() {
        assert_eq!(decode_inventory_target_word(0x1234_ffff), -1);
        assert_eq!(decode_inventory_target_word(0xabcd_8000), -32768);

        let scrap = RobotsInventoryDefinition::from_native_words([
            ROBOTS_SCRAP_UID,
            0,
            0,
            0,
            0,
            0,
            0x0000_0064,
        ])
        .unwrap();
        assert_eq!(scrap.target(0, 0), 100);
        assert_eq!(scrap.target(0, 1), 500);
        assert_eq!(scrap.target(1, 1), 1000);
    }

    #[test]
    fn preview_add_reuses_native_result_without_mutating_inventory() {
        let definition = RobotsInventoryDefinition::from_native_words([
            0x4700_0025,
            u32::MAX,
            0,
            0,
            u32::MAX,
            0,
            5,
        ])
        .unwrap();
        let state = RobotsInventoryState::default();
        let preview = state.preview_add(Some(definition), 5);
        assert_eq!(preview.native_result_code, 4);
        assert_eq!(preview.current, Some(5));
        assert_eq!(state.stored_current(definition.uid), 0);
    }

    #[test]
    fn counter_add_and_remove_match_native_codes_and_pair_lifetime() {
        let definition = RobotsInventoryDefinition::from_native_words([
            0x4700_0025,
            u32::MAX,
            0,
            0,
            u32::MAX,
            0,
            5,
        ])
        .unwrap();
        let mut state = RobotsInventoryState::default();

        let first = state.add(Some(definition), 0);
        assert_eq!(first.native_result_code, 1);
        assert_eq!(first.current, Some(1));
        assert_eq!(state.stored_current(definition.uid), 1);

        let exact_cap = state.add(Some(definition), 4);
        assert_eq!(exact_cap.native_result_code, 1);
        assert_eq!(exact_cap.current, Some(5));

        let already_capped = state.add(Some(definition), 1);
        assert_eq!(already_capped.native_result_code, 3);
        assert_eq!(already_capped.current, Some(5));

        let reduced = state.remove_quantity(Some(definition), 2, false);
        assert_eq!(reduced.native_result_code, 1);
        assert_eq!(reduced.current, Some(3));

        let removed = state.remove_quantity(Some(definition), 3, false);
        assert_eq!(removed.native_result_code, 6);
        assert_eq!(removed.current, None);
        assert_eq!(state.stored_current(definition.uid), 0);
    }

    #[test]
    fn new_counter_pair_reaching_cap_uses_native_code_four() {
        let definition = RobotsInventoryDefinition::from_native_words([
            0x4700_0027,
            u32::MAX,
            0,
            0,
            u32::MAX,
            0,
            5,
        ])
        .unwrap();
        let mut state = RobotsInventoryState::default();
        let outcome = state.add(Some(definition), 5);
        assert_eq!(outcome.native_result_code, 4);
        assert_eq!(outcome.current, Some(5));
    }

    #[test]
    fn bitset_add_and_remove_preserve_native_mask_semantics() {
        let definition = RobotsInventoryDefinition::from_native_words([
            0x4700_0012,
            u32::MAX,
            0,
            0,
            u32::MAX,
            1,
            16,
        ])
        .unwrap();
        assert!(definition.is_bitset());
        let mut state = RobotsInventoryState::default();

        let bit_zero = state.add(Some(definition), 0);
        assert_eq!(bit_zero.native_result_code, 1);
        assert_eq!(bit_zero.current, Some(0x01));

        let added = state.add(Some(definition), 4);
        assert_eq!(added.native_result_code, 1);
        assert_eq!(added.current, Some(0x11));

        let invalid = state.add(Some(definition), 16);
        assert_eq!(invalid.native_result_code, 0);
        assert_eq!(state.stored_current(definition.uid), 0x11);

        let removed = state.remove_quantity(Some(definition), 4, false);
        assert_eq!(removed.native_result_code, 1);
        assert_eq!(removed.current, Some(0x01));
    }

    #[test]
    fn scrap_hooks_double_add_and_can_veto_quantity_remove() {
        let scrap = RobotsInventoryDefinition::from_native_words([
            ROBOTS_SCRAP_UID,
            u32::MAX,
            0,
            0,
            u32::MAX,
            4,
            200,
        ])
        .unwrap();
        let mut state = RobotsInventoryState {
            values: vec![RobotsInventoryValue {
                uid: ROBOTS_SCRAP_DOUBLE_VALUE_UPGRADE_UID,
                current: 1,
            }],
        };

        let added = state.add(Some(scrap), 10);
        assert_eq!(added.current, Some(20));

        let vetoed = state.remove_quantity(Some(scrap), 5, true);
        assert_eq!(vetoed.native_result_code, 1);
        assert_eq!(vetoed.previous, Some(20));
        assert_eq!(vetoed.current, Some(20));

        let removed = state.remove_quantity(Some(scrap), 5, false);
        assert_eq!(removed.current, Some(15));
    }
}
