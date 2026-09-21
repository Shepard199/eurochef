use std::io::{Seek, SeekFrom};

use anyhow::Context;
use eurochef_edb::{binrw::BinReaderExt, edb::EdbFile};
use serde::Serialize;

use crate::spreadsheets::UXGeoSpreadsheet;

use super::inventory::{RobotsInventoryDefinition, RobotsInventoryState};

pub const ROBOTS_MISSION_UID_BASE: u32 = 0x5400_0000;
pub const ROBOTS_MISSION_LINK_GATE_ITEM_FIRST: u32 = 0x4800_000e;
pub const ROBOTS_MISSION_LINK_GATE_ITEM_LAST: u32 = 0x4800_001d;
pub const ROBOTS_MISSIONS_FILE_UID: u32 = 0x0100_007d;
pub const ROBOTS_MISSIONS_SPREADSHEET_UID: u32 = 0x1400_000d;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsMissionDefinition {
    pub mission_uid: u32,
    pub objective_uid: u32,
    pub hud_item_uid: u32,
    pub mission_text_uid: u32,
}

impl RobotsMissionDefinition {
    /// Native `0x004D4ED0` copies `HT_SpreadSheet_Missions` as fixed 0x10-byte
    /// rows and removes rows whose first dword is `0xFFFFFFFF`.
    pub fn from_native_words(words: [u32; 4]) -> Option<Self> {
        (words[0] != u32::MAX).then_some(Self {
            mission_uid: words[0],
            objective_uid: words[1],
            hud_item_uid: words[2],
            mission_text_uid: words[3],
        })
    }
}

pub fn read_robots_mission_definitions(
    edb: &mut EdbFile,
) -> anyhow::Result<Vec<RobotsMissionDefinition>> {
    anyhow::ensure!(
        edb.header.hashcode == ROBOTS_MISSIONS_FILE_UID,
        "expected Robots missions EDB 0x{ROBOTS_MISSIONS_FILE_UID:08X}, got 0x{:08X}",
        edb.header.hashcode
    );
    let spreadsheets = UXGeoSpreadsheet::read_all(edb)?;
    let sheet = spreadsheets
        .iter()
        .find_map(|(uid, spreadsheet)| {
            (*uid == ROBOTS_MISSIONS_SPREADSHEET_UID)
                .then_some(spreadsheet)
                .and_then(|spreadsheet| match spreadsheet {
                    UXGeoSpreadsheet::Data(sheets) => sheets.first(),
                    UXGeoSpreadsheet::Text(_) => None,
                })
        })
        .context("missing HT_SpreadSheet_Missions sheet #0")?;
    edb.seek(SeekFrom::Start(sheet.address as u64))?;
    let mut definitions = Vec::with_capacity(sheet.row_count as usize);
    for _ in 0..sheet.row_count {
        let words = [
            edb.read_type(edb.endian)?,
            edb.read_type(edb.endian)?,
            edb.read_type(edb.endian)?,
            edb.read_type(edb.endian)?,
        ];
        if let Some(definition) = RobotsMissionDefinition::from_native_words(words) {
            definitions.push(definition);
        }
    }
    Ok(definitions)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[repr(u8)]
pub enum RobotsMissionStatus {
    #[default]
    Inactive = 0,
    Active = 1,
    Failed = 2,
    Completed = 3,
}

impl RobotsMissionStatus {
    /// Generic HT_ScriptEvents_MissionUpdate at 0x00402EF0 maps payload mode
    /// 1 -> activate, 2 -> complete, 3 -> fail. Every other mode leaves status
    /// unchanged.
    pub const fn from_update_mode(mode: u32) -> Option<Self> {
        match mode {
            1 => Some(Self::Active),
            2 => Some(Self::Completed),
            3 => Some(Self::Failed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsMissionStatusValue {
    pub mission_uid: u32,
    pub status: RobotsMissionStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct RobotsMissionState {
    pub values: Vec<RobotsMissionStatusValue>,
}

impl RobotsMissionState {
    pub fn clear(&mut self) {
        self.values.clear();
    }

    pub fn status(&self, mission_uid: u32) -> RobotsMissionStatus {
        self.values
            .iter()
            .find(|value| value.mission_uid == mission_uid)
            .map(|value| value.status)
            .unwrap_or_default()
    }

    pub fn get(&self, mission_uid: &u32) -> Option<&RobotsMissionStatus> {
        self.values
            .iter()
            .find(|value| value.mission_uid == *mission_uid)
            .map(|value| &value.status)
    }

    pub fn set_status(
        &mut self,
        mission_uid: u32,
        status: RobotsMissionStatus,
    ) -> RobotsMissionStatus {
        if let Some(value) = self
            .values
            .iter_mut()
            .find(|value| value.mission_uid == mission_uid)
        {
            let previous = value.status;
            value.status = status;
            return previous;
        }
        self.values.push(RobotsMissionStatusValue {
            mission_uid,
            status,
        });
        RobotsMissionStatus::Inactive
    }

    pub fn remove(&mut self, mission_uid: u32) -> Option<RobotsMissionStatus> {
        let index = self
            .values
            .iter()
            .position(|value| value.mission_uid == mission_uid)?;
        Some(self.values.remove(index).status)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsObjectiveProgress {
    pub current: i32,
    pub target: i32,
}

impl RobotsObjectiveProgress {
    pub fn condition_met(self) -> bool {
        self.current == self.target
    }
}

pub fn is_robots_mission_uid(uid: u32) -> bool {
    uid != ROBOTS_MISSION_UID_BASE && (uid & 0xffff_0000) == ROBOTS_MISSION_UID_BASE
}

pub fn objective_condition_met(progress: Option<RobotsObjectiveProgress>) -> bool {
    progress.is_some_and(RobotsObjectiveProgress::condition_met)
}

/// Join the two native definition databases used by `0x0048F4F0`: the
/// Mission spreadsheet maps Mission UID -> family-0x47 objective UID, then the
/// Inventory spreadsheet supplies its target while the runtime pair supplies
/// current. EnergyCell/HealthReplenish use Player-health/energy state instead
/// of the stored pair, so this helper deliberately fails closed for them.
pub fn resolve_mission_objective_progress(
    mission: RobotsMissionDefinition,
    inventory_definitions: &[RobotsInventoryDefinition],
    inventory_state: &RobotsInventoryState,
) -> Option<RobotsObjectiveProgress> {
    if mission.objective_uid == u32::MAX {
        return None;
    }
    let definition = inventory_definitions
        .iter()
        .copied()
        .find(|definition| definition.uid == mission.objective_uid)?;
    if definition.uses_external_current() {
        return None;
    }
    Some(RobotsObjectiveProgress {
        current: inventory_state.stored_current(definition.uid),
        target: inventory_state.effective_target(definition),
    })
}

/// `XTrigger_NPC` objective refresh (0x0047F070) maps Mission link count 1..16
/// to Player family-0x48 gate items 0x4800000E..0x4800001D. The gate is queried
/// through embedded-interface preflight vslot +0x2C with quantity 1.
pub fn npc_linked_mission_gate_item(link_count: usize) -> Option<u32> {
    let index = link_count.checked_sub(1)?;
    let uid = ROBOTS_MISSION_LINK_GATE_ITEM_FIRST.checked_add(u32::try_from(index).ok()?)?;
    (uid <= ROBOTS_MISSION_LINK_GATE_ITEM_LAST).then_some(uid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn objective_condition_is_exact_current_target_equality() {
        assert!(RobotsObjectiveProgress {
            current: 7,
            target: 7,
        }
        .condition_met());
        assert!(!RobotsObjectiveProgress {
            current: 6,
            target: 7,
        }
        .condition_met());
        assert!(!objective_condition_met(None));
    }

    #[test]
    fn linked_mission_gate_table_matches_native_range() {
        assert_eq!(npc_linked_mission_gate_item(0), None);
        assert_eq!(npc_linked_mission_gate_item(1), Some(0x4800_000e));
        assert_eq!(npc_linked_mission_gate_item(16), Some(0x4800_001d));
        assert_eq!(npc_linked_mission_gate_item(17), None);
    }

    #[test]
    fn mission_spreadsheet_row_matches_native_16_byte_contract() {
        assert_eq!(
            RobotsMissionDefinition::from_native_words([
                0x5400_0001,
                0x4700_0010,
                0x0200_0072,
                0x1234_5678,
            ]),
            Some(RobotsMissionDefinition {
                mission_uid: 0x5400_0001,
                objective_uid: 0x4700_0010,
                hud_item_uid: 0x0200_0072,
                mission_text_uid: 0x1234_5678,
            })
        );
        assert_eq!(
            RobotsMissionDefinition::from_native_words([u32::MAX, 1, 2, 3]),
            None
        );
    }

    #[test]
    fn mission_progress_joins_native_mission_and_inventory_definitions() {
        let mission = RobotsMissionDefinition {
            mission_uid: 0x5400_0002,
            objective_uid: 0x4700_0012,
            hud_item_uid: 0x0200_006b,
            mission_text_uid: 0x4500_022d,
        };
        let objective = RobotsInventoryDefinition::from_native_words([
            0x4700_0012,
            u32::MAX,
            0x0400_002b,
            0x0100_0003,
            u32::MAX,
            1,
            16,
        ])
        .unwrap();
        let mut state = RobotsInventoryState::default();

        assert_eq!(
            resolve_mission_objective_progress(mission, &[objective], &state),
            Some(RobotsObjectiveProgress {
                current: 0,
                target: 16,
            })
        );
        state.add(Some(objective), 4);
        let progress = resolve_mission_objective_progress(mission, &[objective], &state).unwrap();
        assert_eq!(progress.current, 16);
        assert_eq!(progress.target, 16);
        assert!(progress.condition_met());
    }

    #[test]
    fn mission_progress_fails_closed_for_external_current_inventory_types() {
        let mission = RobotsMissionDefinition {
            mission_uid: 0x5400_0001,
            objective_uid: 0x4700_0003,
            hud_item_uid: u32::MAX,
            mission_text_uid: u32::MAX,
        };
        let energy = RobotsInventoryDefinition::from_native_words([
            0x4700_0003,
            u32::MAX,
            0,
            0,
            u32::MAX,
            0,
            4,
        ])
        .unwrap();
        assert_eq!(
            resolve_mission_objective_progress(
                mission,
                &[energy],
                &RobotsInventoryState::default()
            ),
            None
        );
    }
}
