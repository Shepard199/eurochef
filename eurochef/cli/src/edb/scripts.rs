use std::{fs::File, io::BufReader, path::Path};

use anyhow::Context;
use eurochef_edb::{edb::EdbFile, versions::Platform};
use eurochef_shared::script::UXGeoScript;
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
    std::fs::create_dir_all(output_folder)?;

    let platform = platform
        .map(|value| value.into())
        .or(Platform::from_path(&filename))
        .expect("Failed to detect platform");

    let file = File::open(&filename).with_context(|| format!("failed to open {filename}"))?;
    let reader = BufReader::new(file);
    let mut edb = EdbFile::new(Box::new(reader), platform)?;
    let header = edb.header.clone();
    let scripts = UXGeoScript::read_all(&mut edb)?;

    let mut index_entries = Vec::with_capacity(scripts.len());
    for (index, script) in scripts.iter().enumerate() {
        let stem = resource_file_stem("Script", script.hashcode);
        let output_name = format!("{stem}.script.json");
        let output_path = output_folder.join(&output_name);
        std::fs::write(&output_path, serde_json::to_vec_pretty(script)?)
            .with_context(|| format!("failed to write {}", output_path.display()))?;
        index_entries.push(ScriptExportIndexEntry {
            index,
            uid: format!("0x{:08X}", script.hashcode),
            label: resource_label("Script", script.hashcode),
            file: output_name,
        });
    }

    let index = ScriptExportIndex {
        source_file: filename,
        source_edb_uid: format!("0x{:08X}", header.hashcode),
        scripts: index_entries,
    };
    let index_path = output_folder.join("SCRIPTS_INDEX.json");
    std::fs::write(&index_path, serde_json::to_vec_pretty(&index)?)
        .with_context(|| format!("failed to write {}", index_path.display()))?;

    info!(
        scripts = scripts.len(),
        source_edb = format_args!("0x{:08X}", header.hashcode),
        output = %output_folder.display(),
        "exported Scripts with canonical resource names"
    );
    Ok(())
}
