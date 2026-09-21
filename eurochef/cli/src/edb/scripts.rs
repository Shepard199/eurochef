use std::{collections::BTreeMap, fs::File, io::BufReader, path::Path};

use anyhow::Context;
use eurochef_edb::{edb::EdbFile, versions::Platform};
use eurochef_shared::{
    robots_runtime::events::{
        classify_generic_handler_script_command, RobotsHandlerScriptCommandFamily,
        RobotsScriptEventView,
    },
    script::{
        robots_script_command_role, robots_script_payload_diagnostic, UXGeoScript,
        UXGeoScriptCommandData,
    },
};
use serde::Serialize;

use crate::{
    edb::{resource_file_stem, resource_label},
    PlatformArg,
};

#[derive(Debug, Serialize)]
struct ScriptExportIndexEntry {
    index: usize,
    uid: String,
    label: String,
    file: String,
}

#[derive(Debug, Serialize)]
struct ScriptExportIndex {
    source_file: String,
    source_edb_uid: String,
    scripts: Vec<ScriptExportIndexEntry>,
}

#[derive(Debug, Serialize)]
struct ScriptLibraryFileEntry {
    source_edb_uid: String,
    source_file: String,
    index_file: String,
    script_count: usize,
}

#[derive(Debug, Serialize)]
struct ScriptLibraryIndex {
    schema: &'static str,
    command_semantics_schema: &'static str,
    source_manifest: String,
    file_count: usize,
    script_count: usize,
    files: Vec<ScriptLibraryFileEntry>,
}

fn read_scripts(filename: &Path, platform: Platform) -> anyhow::Result<(u32, Vec<UXGeoScript>)> {
    let file =
        File::open(filename).with_context(|| format!("failed to open {}", filename.display()))?;
    let reader = BufReader::new(file);
    let mut edb = EdbFile::new(Box::new(reader), platform)?;
    let source_edb_uid = edb.header.hashcode;
    let scripts = UXGeoScript::read_all(&mut edb)?;
    Ok((source_edb_uid, scripts))
}

fn serialized_payload_size(data: &UXGeoScriptCommandData) -> usize {
    match data {
        UXGeoScriptCommandData::Animation { .. } => 24,
        UXGeoScriptCommandData::Entity { .. }
        | UXGeoScriptCommandData::Particle { .. }
        | UXGeoScriptCommandData::SubScript { .. } => 12,
        UXGeoScriptCommandData::Sound { .. } => 24,
        UXGeoScriptCommandData::Event { data, .. } => 4 + data.len(),
        UXGeoScriptCommandData::Unknown { data, .. } => data.len(),
    }
}

fn owner_handler_event_payload(event_type: u32, data: &[u8]) -> serde_json::Value {
    let generic_family = RobotsHandlerScriptCommandFamily::Generic;
    let generic_plan = classify_generic_handler_script_command(RobotsScriptEventView {
        event_type,
        data,
        start: None,
        length: None,
    });
    serde_json::json!({
        "kind": "owner_handler_event",
        "event_type": event_type,
        "event_name": eurochef_edb::robots_hashdb::resolve(event_type),
        "generic_command_reference": {
            "family": generic_family,
            "native_target": format!("0x{:08X}", generic_family.native_target()),
            "plan": generic_plan,
        },
    })
}

fn runtime_script_json(script: &UXGeoScript) -> anyhow::Result<Vec<u8>> {
    let mut value = serde_json::to_value(script)?;
    let root = value
        .as_object_mut()
        .context("serialized Script root is not a JSON object")?;
    root.insert(
        "runtime_semantics_schema".to_string(),
        serde_json::Value::String("robots-script-command-semantics-v2".to_string()),
    );
    let commands = root
        .get_mut("commands")
        .and_then(serde_json::Value::as_array_mut)
        .context("serialized Script commands are not a JSON array")?;
    anyhow::ensure!(
        commands.len() == script.commands.len(),
        "serialized Script command count changed during runtime enrichment"
    );

    for (json_command, command) in commands.iter_mut().zip(&script.commands) {
        let object = json_command
            .as_object_mut()
            .context("serialized Script command is not a JSON object")?;
        let role =
            robots_script_command_role(command.opcode, serialized_payload_size(&command.data));
        object.insert("native_role".to_string(), serde_json::to_value(role)?);
        let native_payload = match &command.data {
            UXGeoScriptCommandData::Event { event_type, data } => {
                owner_handler_event_payload(*event_type, data)
            }
            UXGeoScriptCommandData::Unknown { data, .. } if command.opcode == 17 => {
                let count = (command.controller_header_index & 0x00FF) as usize;
                let mut indices = vec![
                    (command.controller_header_index >> 8) as u8,
                    command.controller_index,
                    command.parent_controller_index,
                ];
                indices.extend_from_slice(data);
                anyhow::ensure!(
                    count <= indices.len(),
                    "opcode17 fan-out count exceeds serialized inline controller list"
                );
                serde_json::json!({
                    "kind": "controller_fan_out",
                    "controller_indices": &indices[..count],
                })
            }
            UXGeoScriptCommandData::Unknown { data, .. } => {
                serde_json::to_value(robots_script_payload_diagnostic(command.opcode, data))?
            }
            _ => serde_json::Value::Null,
        };
        object.insert("native_payload".to_string(), native_payload);
    }

    Ok(serde_json::to_vec_pretty(&value)?)
}

fn write_scripts(
    source_file: &Path,
    source_edb_uid: u32,
    scripts: &[UXGeoScript],
    output_folder: &Path,
    enrich_runtime_semantics: bool,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(output_folder)?;
    let mut index_entries = Vec::with_capacity(scripts.len());
    for (index, script) in scripts.iter().enumerate() {
        let stem = resource_file_stem("Script", script.hashcode);
        let output_name = format!("{stem}.script.json");
        let output_path = output_folder.join(&output_name);
        let payload = if enrich_runtime_semantics {
            runtime_script_json(script)?
        } else {
            serde_json::to_vec_pretty(script)?
        };
        std::fs::write(&output_path, payload)
            .with_context(|| format!("failed to write {}", output_path.display()))?;
        index_entries.push(ScriptExportIndexEntry {
            index,
            uid: format!("0x{:08X}", script.hashcode),
            label: resource_label("Script", script.hashcode),
            file: output_name,
        });
    }

    let index = ScriptExportIndex {
        source_file: source_file.display().to_string(),
        source_edb_uid: format!("0x{source_edb_uid:08X}"),
        scripts: index_entries,
    };
    let index_path = output_folder.join("SCRIPTS_INDEX.json");
    std::fs::write(&index_path, serde_json::to_vec_pretty(&index)?)
        .with_context(|| format!("failed to write {}", index_path.display()))?;
    Ok(())
}

pub fn execute_command(
    filename: String,
    platform: Option<PlatformArg>,
    output_folder: Option<String>,
) -> anyhow::Result<()> {
    let output_folder = output_folder.unwrap_or(format!(
        "./scripts/{}/",
        Path::new(&filename).file_name().unwrap().to_string_lossy(),
    ));
    let output_folder = Path::new(&output_folder);
    let platform = platform
        .map(|value| value.into())
        .or(Platform::from_path(&filename))
        .expect("Failed to detect platform");
    let source_file = Path::new(&filename);
    let (source_edb_uid, scripts) = read_scripts(source_file, platform)?;
    write_scripts(
        source_file,
        source_edb_uid,
        &scripts,
        output_folder,
        platform == Platform::Pc,
    )?;

    info!(
        scripts = scripts.len(),
        source_edb = format_args!("0x{source_edb_uid:08X}"),
        output = %output_folder.display(),
        "exported Scripts with canonical resource names"
    );
    Ok(())
}

pub fn execute_library_command(
    manifest: String,
    output_folder: Option<String>,
) -> anyhow::Result<()> {
    let output_folder = output_folder.unwrap_or_else(|| "./scripts/library/".to_string());
    let output_folder = Path::new(&output_folder);
    std::fs::create_dir_all(output_folder)?;

    let paths = super::read_corpus_manifest_paths(Path::new(&manifest))?;
    let mut owners = BTreeMap::<u32, String>::new();
    let mut files = Vec::new();
    let mut total_scripts = 0usize;

    for source_file in paths {
        let platform = Platform::from_path(&source_file)
            .with_context(|| format!("failed to detect platform for {}", source_file.display()))?;
        if platform != Platform::Pc {
            continue;
        }
        let (source_edb_uid, scripts) = read_scripts(&source_file, platform)?;
        let source_text = source_file.display().to_string();
        if let Some(existing) = owners.get(&source_edb_uid) {
            anyhow::ensure!(
                existing == &source_text,
                "duplicate EDB UID 0x{source_edb_uid:08X} in {existing} and {source_text}"
            );
            continue;
        }
        owners.insert(source_edb_uid, source_text.clone());

        let owner_folder_name = format!("{source_edb_uid:08X}");
        let owner_folder = output_folder.join(&owner_folder_name);
        write_scripts(&source_file, source_edb_uid, &scripts, &owner_folder, true)?;
        total_scripts += scripts.len();
        files.push(ScriptLibraryFileEntry {
            source_edb_uid: format!("0x{source_edb_uid:08X}"),
            source_file: source_text,
            index_file: format!("{owner_folder_name}/SCRIPTS_INDEX.json"),
            script_count: scripts.len(),
        });
    }

    let index = ScriptLibraryIndex {
        schema: "robots-script-library-v1",
        command_semantics_schema: "robots-script-command-semantics-v2",
        source_manifest: manifest,
        file_count: files.len(),
        script_count: total_scripts,
        files,
    };
    let index_path = output_folder.join("ROBOTS_SCRIPT_LIBRARY.json");
    std::fs::write(&index_path, serde_json::to_vec_pretty(&index)?)
        .with_context(|| format!("failed to write {}", index_path.display()))?;

    info!(
        files = index.file_count,
        scripts = index.script_count,
        output = %output_folder.display(),
        "exported owner-scoped Robots Script library"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use eurochef_shared::robots_runtime::events::event_type;

    fn payload_words(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|word| word.to_le_bytes()).collect()
    }

    #[test]
    fn owner_event_export_keeps_generic_command_reference_separate() {
        let value = owner_handler_event_payload(event_type::EXIT_SCRIPT, &[]);
        assert_eq!(value["kind"], "owner_handler_event");
        assert_eq!(value["generic_command_reference"]["family"], "generic");
        assert_eq!(
            value["generic_command_reference"]["native_target"],
            "0x00402EF0"
        );
        assert_eq!(
            value["generic_command_reference"]["plan"]["semantic"]["kind"],
            "exit_script"
        );
        assert_eq!(
            value["generic_command_reference"]["plan"]["return_policy"],
            "one"
        );
    }

    #[test]
    fn owner_event_export_uses_shared_generic_payload_decoder() {
        let data = payload_words(&[0xDEAD_BEEF, 0, 0x4700_0023, 1.0f32.to_bits()]);
        let value = owner_handler_event_payload(event_type::INVENTORY_ADD, &data);
        let semantic = &value["generic_command_reference"]["plan"]["semantic"];
        assert_eq!(semantic["kind"], "inventory_add");
        assert_eq!(semantic["owner_selector"], 0);
        assert_eq!(semantic["item"], 0x4700_0023u32);
        assert_eq!(semantic["quantity"], 1.0);
    }
}
