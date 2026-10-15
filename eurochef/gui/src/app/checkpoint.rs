//! EuroChef checkpoint format v1. This stores *only* reverse-proven
//! persistent Player/mission/inventory groups and a verified level/Player
//! entry context. It is deliberately NOT a full native SaveGame or a snapshot
//! of NPC/XItem/world state. Use versioning so future full saves can coexist.
use std::{fs, path::PathBuf};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use eurochef_shared::robots_runtime::{
    change_level::{robots_shipped_level_edb_uid, RobotsLevelTransitionProgress},
    inventory::is_robots_inventory_uid,
    mission::is_robots_mission_uid,
    player_items::is_robots_player_item_uid,
};

pub(super) const FORMAT_VERSION: u32 = 1;
const MAX_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct RobotsCheckpoint {
    pub format_version: u32,
    pub level_edb_uid: u32,
    pub player_entry_context: u32,
    pub progress: RobotsLevelTransitionProgress,
}

impl RobotsCheckpoint {
    fn validate(&self) -> Result<()> {
        anyhow::ensure!(self.format_version == FORMAT_VERSION, "Unsupported checkpoint version {}", self.format_version);
        anyhow::ensure!(robots_shipped_level_edb_uid(self.level_edb_uid) == Some(self.level_edb_uid),
            "Checkpoint level UID is not in the shipped Robots catalog");
        anyhow::ensure!(self.progress.gameplay.inventory.values.len() <= 4096
            && self.progress.gameplay.missions.values.len() <= 4096
            && self.progress.player_items.values.len() <= 4096,
            "Checkpoint contains too many persistent records");
        anyhow::ensure!(self.progress.gameplay.inventory.values.iter().all(|v| is_robots_inventory_uid(v.uid)),
            "Invalid inventory UID");
        anyhow::ensure!(self.progress.gameplay.missions.values.iter().all(|v| is_robots_mission_uid(v.mission_uid)),
            "Invalid mission UID");
        anyhow::ensure!(self.progress.player_items.values.iter().all(|v| is_robots_player_item_uid(v.uid)),
            "Invalid Player item UID");
        Ok(())
    }
}

fn slot_path(slot: u8) -> Result<PathBuf> {
    anyhow::ensure!((1..=3).contains(&slot), "Invalid slot number");
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .context("No user data directory available")?;
    Ok(base.join("Robots_Revers").join("checkpoints").join(format!("slot_{slot:02}.json")))
}

pub(super) fn slot_exists(slot: u8) -> bool {
    slot_path(slot).is_ok_and(|path| path.is_file())
}

pub(super) fn save_slot(slot: u8, checkpoint: &RobotsCheckpoint) -> Result<()> {
    checkpoint.validate()?;
    let path = slot_path(slot)?;
    let bytes = serde_json::to_vec_pretty(checkpoint)?;
    anyhow::ensure!(bytes.len() <= MAX_BYTES, "Checkpoint too large");
    fs::create_dir_all(path.parent().context("Invalid save path")?)?;
    // Temp file prevents partial JSON from being mistaken for a valid slot.
    let staged = path.with_extension("json.tmp");
    fs::write(&staged, bytes).context("Writing checkpoint staging file")?;
    if path.exists() {
        // Windows rename does not overwrite existing files. This format is
        // explicitly an experimental checkpoint, not an atomic native save.
        fs::remove_file(&path).context("Replacing old checkpoint")?;
    }
    fs::rename(&staged, &path).context("Committing checkpoint")?;
    Ok(())
}

pub(super) fn load_slot(slot: u8) -> Result<RobotsCheckpoint> {
    let path = slot_path(slot)?;
    let bytes = fs::read(&path).with_context(|| format!("Reading checkpoint {}", path.display()))?;
    anyhow::ensure!(bytes.len() <= MAX_BYTES, "Checkpoint is too large");
    let checkpoint: RobotsCheckpoint = serde_json::from_slice(&bytes).context("Parsing checkpoint")?;
    checkpoint.validate()?;
    Ok(checkpoint)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eurochef_shared::robots_runtime::{
        inventory::RobotsInventoryValue,
        mission::RobotsMissionStatus,
    };

    #[test]
    fn checkpoint_json_roundtrip_preserves_known_native_groups() {
        let mut checkpoint = RobotsCheckpoint {
            format_version: FORMAT_VERSION,
            level_edb_uid: 0x0100_0012,
            player_entry_context: 7,
            progress: RobotsLevelTransitionProgress::default(),
        };
        checkpoint.progress.gameplay.inventory.values.push(RobotsInventoryValue { uid: 0x4700_0001, current: 23 });
        checkpoint.progress.gameplay.missions.set_status(0x5400_0001, RobotsMissionStatus::Completed);
        checkpoint.progress.player_items.seed_profile_value(0x4800_0008, 1);
        checkpoint.progress.health_fraction_bits_58000027 = Some(0.65f32.to_bits());
        checkpoint.progress.player_scalar_bits_58000028 = Some(60.0f32.to_bits());
        checkpoint.progress.selected_action_uid_58000029 = Some(0x4800_0005);
        let encoded = serde_json::to_vec(&checkpoint).unwrap();
        let decoded: RobotsCheckpoint = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(checkpoint, decoded);
        decoded.validate().unwrap();
    }
    #[test]
    fn corrupt_or_unknown_checkpoint_is_rejected_before_level_load() {
        let mut checkpoint = RobotsCheckpoint {
            format_version: FORMAT_VERSION,
            level_edb_uid: 0x0100_0012,
            player_entry_context: 0,
            progress: RobotsLevelTransitionProgress::default(),
        };
        checkpoint.format_version += 1;
        assert!(checkpoint.validate().is_err());
        checkpoint.format_version = FORMAT_VERSION;
        checkpoint.level_edb_uid = 0x4500_02d8;
        assert!(checkpoint.validate().is_err());
        checkpoint.level_edb_uid = 0x0100_0012;
        checkpoint.progress.gameplay.inventory.values.push(RobotsInventoryValue { uid: 0x4800_0001, current: 1 });
        assert!(checkpoint.validate().is_err());
        assert!(slot_path(0).is_err());
        assert!(slot_path(4).is_err());
    }
}
