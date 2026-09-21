use std::path::{Path, PathBuf};

use anyhow::Context;

const TICK_STRINGS: &str = "⠁⠂⠄⡀⢀⠠⠐⠈";

pub(crate) fn resource_name(kind: &str, uid: u32) -> String {
    if uid == u32::MAX {
        return "HT_None".to_string();
    }
    if uid == 0 {
        return "HT_Zero".to_string();
    }
    if let Some(name) = eurochef_edb::robots_hashdb::resolve(uid) {
        return name.to_string();
    }
    if (uid & 0x8000_0000) != 0 {
        return format!("HT_Local_{kind}_{uid:08X}");
    }
    format!("HT_{kind}_Unknown_{uid:08X}")
}

pub(crate) fn resource_label(kind: &str, uid: u32) -> String {
    format!("{} [0x{uid:08X}]", resource_name(kind, uid))
}

pub(crate) fn resource_file_stem(kind: &str, uid: u32) -> String {
    format!("{}_[0x{uid:08X}]", resource_name(kind, uid))
}

pub(crate) fn resource_name_in_edb(kind: &str, owner_edb_uid: u32, uid: u32) -> String {
    if kind == "Texture" {
        if let Some(global_uid) = eurochef_edb::robots_texture_aliases::resolve(owner_edb_uid, uid)
        {
            return resource_name(kind, global_uid);
        }
    }
    resource_name(kind, uid)
}

pub(crate) fn resource_file_stem_in_edb(kind: &str, owner_edb_uid: u32, uid: u32) -> String {
    format!(
        "{}_[0x{uid:08X}]",
        resource_name_in_edb(kind, owner_edb_uid, uid)
    )
}

pub(crate) fn read_corpus_manifest_paths(path: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read corpus manifest {}", path.display()))?;
    let base = path.parent().unwrap_or_else(|| Path::new("."));
    let mut entries = Vec::new();
    for (line_index, line) in content.lines().enumerate() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let Some((_, source_text)) = line.split_once('\t') else {
            if line_index == 0 {
                continue;
            }
            continue;
        };
        let source_text = source_text.trim();
        if source_text.is_empty()
            || source_text.to_ascii_lowercase().contains("source edb")
            || source_text.eq_ignore_ascii_case("path")
        {
            continue;
        }
        let source_path = PathBuf::from(source_text);
        entries.push(if source_path.is_absolute() {
            source_path
        } else {
            base.join(source_path)
        });
    }
    if entries.is_empty() {
        entries = resource_atlas::discover_edb_paths_near_manifest(path)?;
    }
    anyhow::ensure!(
        !entries.is_empty(),
        "corpus manifest {} contains no EDB paths",
        path.display()
    );
    Ok(entries)
}

pub mod anim_binding_report;
pub mod animations;
pub mod entities;
pub mod entity_report;
pub mod fbx_characters;
mod gltf_export;
pub mod maps;
pub mod particle_report;
pub mod resource_atlas;
pub mod script_health;
pub mod scripts;
pub mod spreadsheets;
pub mod texture_alias_report;
pub mod textures;
pub mod trigger_report;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_names_keep_decoded_symbol_and_exact_uid() {
        assert_eq!(
            resource_file_stem("Entity", 0x0200_01B4),
            "HT_Entity_Vehicle_Taxi_Collision_[0x020001B4]"
        );
        assert_eq!(
            resource_label("Script", 0x8400_0009),
            "HT_Local_Script_84000009 [0x84000009]"
        );
    }

    #[test]
    fn texture_alias_names_are_owner_scoped() {
        assert_eq!(
            resource_file_stem_in_edb("Texture", 0x0100_0071, 0x8600_00D7),
            "HT_Texture_BlankWhite_[0x860000D7]"
        );
        assert_eq!(
            resource_file_stem_in_edb("Texture", 0x0100_0012, 0x8600_00D7),
            "HT_Local_Texture_860000D7_[0x860000D7]"
        );
    }
}
