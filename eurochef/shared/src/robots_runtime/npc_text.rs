use std::{
    collections::BTreeMap,
    io::{Seek, SeekFrom},
};

use anyhow::Context;
use eurochef_edb::{binrw::BinReaderExt, edb::EdbFile};
use serde::Serialize;

use crate::spreadsheets::UXGeoSpreadsheet;

use super::process_rng::robots_process_lcg_step;
pub use super::process_rng::ROBOTS_PROCESS_LCG_STARTUP_SEED;

pub const ROBOTS_TEXT_FILE_UID: u32 = 0x0100_0025;
pub const ROBOTS_TEXT_GROUPS_SPREADSHEET_UID: u32 = 0x1400_000C;
pub const ROBOTS_TEXT_GROUP_UID_BASE: u32 = 0x4508_0000;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RobotsTextGroupCatalog {
    pub groups: BTreeMap<u32, Vec<u32>>,
    pub messages_by_uid: BTreeMap<u32, RobotsTextMessageDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RobotsTextMessageDefinition {
    pub message_uid: u32,
    pub text: String,
    /// Native first userdata dword copied by `0x0049BEA0` into message record +0x18.
    pub ui_flags: u32,
    /// Native second userdata dword copied into message record +0x1C and consumed
    /// by the message box as an optional `0x1A......` SoundDetails UID.
    pub sound_uid: u32,
    /// Separate serialized EXGeoTextItem field retained until its relationship to
    /// the userdata sound UID is proven across the shipped D01_Text corpus.
    pub spreadsheet_sound_uid: u32,
}

impl RobotsTextGroupCatalog {
    pub fn messages(&self, group_uid: u32) -> Option<&[u32]> {
        self.groups.get(&group_uid).map(Vec::as_slice)
    }

    pub fn message_definition(&self, message_uid: u32) -> Option<&RobotsTextMessageDefinition> {
        self.messages_by_uid.get(&message_uid)
    }

    fn push_native_row(&mut self, message_uid: u32, group_uid: u32) {
        // XTextManager::CreateTextGroups 0x0044AEC0 ignores deleted rows and
        // logs/ignores invalid group sentinels instead of constructing entries.
        if message_uid == u32::MAX || matches!(group_uid, u32::MAX | ROBOTS_TEXT_GROUP_UID_BASE) {
            return;
        }
        self.groups.entry(group_uid).or_default().push(message_uid);
    }
}

/// Read the exact `D01_Text / HT_SpreadSheet_TextGroups` layout consumed by
/// `XTextManager::CreateTextGroups` (0x0044AE40 -> 0x0044AEC0). Native uses sheet
/// index 0 and fixed 8-byte rows `[message_uid, group_uid]` in serialized order.
pub fn read_robots_text_groups(edb: &mut EdbFile) -> anyhow::Result<RobotsTextGroupCatalog> {
    anyhow::ensure!(
        edb.header.hashcode == ROBOTS_TEXT_FILE_UID,
        "expected Robots text EDB 0x{ROBOTS_TEXT_FILE_UID:08X}, got 0x{:08X}",
        edb.header.hashcode
    );
    let spreadsheets = UXGeoSpreadsheet::read_all(edb)?;
    let mut catalog = RobotsTextGroupCatalog::default();
    for (_, spreadsheet) in &spreadsheets {
        let UXGeoSpreadsheet::Text(sections) = spreadsheet else {
            continue;
        };
        for section in sections {
            for entry in &section.entries {
                catalog
                    .messages_by_uid
                    .entry(entry.hashcode)
                    .or_insert_with(|| {
                        let [ui_flags, sound_uid] = entry.userdata_words.unwrap_or([0, u32::MAX]);
                        RobotsTextMessageDefinition {
                            message_uid: entry.hashcode,
                            text: entry.text.clone(),
                            ui_flags,
                            sound_uid,
                            spreadsheet_sound_uid: entry.sound_hashcode,
                        }
                    });
            }
        }
    }
    let sheet = spreadsheets
        .iter()
        .find_map(|(uid, spreadsheet)| {
            (*uid == ROBOTS_TEXT_GROUPS_SPREADSHEET_UID)
                .then_some(spreadsheet)
                .and_then(|spreadsheet| match spreadsheet {
                    UXGeoSpreadsheet::Data(sheets) => sheets.first(),
                    UXGeoSpreadsheet::Text(_) => None,
                })
        })
        .context("missing HT_SpreadSheet_TextGroups sheet #0")?;

    edb.seek(SeekFrom::Start(sheet.address as u64))?;
    for _ in 0..sheet.row_count {
        let message_uid = edb.read_type::<u32>(edb.endian)?;
        let group_uid = edb.read_type::<u32>(edb.endian)?;
        catalog.push_native_row(message_uid, group_uid);
    }
    Ok(catalog)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RobotsTextGroupSelectionState {
    /// Native XTextManager `+0x14`. `-1` is represented by `None`; the latch is
    /// global to the manager, not per TextGroup.
    pub previous_message_uid: Option<u32>,
}

/// Exact `XTextManager::GetTextFromGroupRandom` selection contract at
/// 0x0044B0B0. Groups with one entry are deterministic and consume no RNG.
/// Multi-entry groups use process-global `DAT_007BE1E8` and reroll while the
/// chosen message equals the manager-wide previous message.
///
/// `process_lcg_seed == None` means the host does not know the current global
/// LCG boundary; in that case a random group fails closed without mutating the
/// previous-message latch.
pub fn select_text_group_message(
    messages: Option<&[u32]>,
    state: &mut RobotsTextGroupSelectionState,
    process_lcg_seed: &mut Option<u32>,
) -> Option<u32> {
    let Some(messages) = messages else {
        state.previous_message_uid = None;
        return None;
    };
    match messages.len() {
        0 => {
            state.previous_message_uid = None;
            None
        }
        1 => {
            let message = messages[0];
            state.previous_message_uid = Some(message);
            Some(message)
        }
        count => {
            // Shipped data is expected to have at least one distinct alternative.
            // Native would reroll forever on a degenerate all-equal group, so the
            // host fails closed instead of hanging the editor/UE runtime.
            if state
                .previous_message_uid
                .is_some_and(|previous| messages.iter().all(|message| *message == previous))
            {
                return None;
            }
            let mut seed = (*process_lcg_seed)?;
            loop {
                seed = robots_process_lcg_step(seed);
                let message = messages[(seed % count as u32) as usize];
                if Some(message) != state.previous_message_uid {
                    *process_lcg_seed = Some(seed);
                    state.previous_message_uid = Some(message);
                    return Some(message);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_rows_preserve_group_order_and_ignore_sentinels() {
        let mut catalog = RobotsTextGroupCatalog::default();
        catalog.push_native_row(0x4400_0001, 0x4508_0002);
        catalog.push_native_row(u32::MAX, 0x4508_0002);
        catalog.push_native_row(0x4400_0002, ROBOTS_TEXT_GROUP_UID_BASE);
        catalog.push_native_row(0x4400_0003, 0x4508_0002);
        assert_eq!(
            catalog.messages(0x4508_0002),
            Some(&[0x4400_0001, 0x4400_0003][..])
        );
    }

    #[test]
    fn single_message_group_is_deterministic_without_rng_anchor() {
        let mut state = RobotsTextGroupSelectionState::default();
        let mut seed = None;
        assert_eq!(
            select_text_group_message(Some(&[0x4400_0001]), &mut state, &mut seed),
            Some(0x4400_0001)
        );
        assert_eq!(seed, None);
        assert_eq!(state.previous_message_uid, Some(0x4400_0001));
    }

    #[test]
    fn random_group_uses_lcg_and_rerolls_previous_message_globally() {
        let messages = [0x4400_0001, 0x4400_0002, 0x4400_0003];
        let mut state = RobotsTextGroupSelectionState {
            previous_message_uid: Some(messages[0]),
        };
        let mut seed = Some(ROBOTS_PROCESS_LCG_STARTUP_SEED);
        // Seed1 draw1 %3=0 and draw2 %3=0 both repeat previous; draw3 %3=2.
        assert_eq!(
            select_text_group_message(Some(&messages), &mut state, &mut seed),
            Some(messages[2])
        );
        assert_eq!(seed, Some(0x8116_017E));
        assert_eq!(state.previous_message_uid, Some(messages[2]));
    }

    #[test]
    fn random_group_fails_closed_without_process_lcg_anchor() {
        let mut state = RobotsTextGroupSelectionState::default();
        let mut seed = None;
        assert_eq!(
            select_text_group_message(Some(&[1, 2]), &mut state, &mut seed),
            None
        );
        assert_eq!(state.previous_message_uid, None);
    }

    #[test]
    fn real_d01_text_group_sound_contract_matches_shipped_corpus() {
        use eurochef_edb::{edb::EdbFile, versions::Platform};
        use std::{fs::File, io::BufReader, path::Path};

        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/d01_text.edb");
        let file = File::open(&path).expect("open real d01_text.edb");
        let mut edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
            .expect("parse real d01_text.edb");
        let catalog = read_robots_text_groups(&mut edb).expect("read real TextGroups");
        assert_eq!(catalog.messages_by_uid.len(), 837);

        let referenced = catalog
            .groups
            .values()
            .flatten()
            .filter_map(|uid| catalog.message_definition(*uid))
            .collect::<Vec<_>>();
        assert_eq!(referenced.len(), 239);
        assert_eq!(
            referenced
                .iter()
                .filter(|message| message.sound_uid == u32::MAX)
                .count(),
            10
        );
        assert_eq!(
            referenced
                .iter()
                .filter(|message| message.sound_uid & 0x7F00_0000 == 0x1A00_0000)
                .count(),
            229
        );
        assert!(referenced.iter().all(|message| {
            message.sound_uid == u32::MAX || message.sound_uid & 0x7F00_0000 == 0x1A00_0000
        }));
    }
}
