use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::File,
    io::{BufReader, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use eurochef_edb::{edb::EdbFile, versions::Platform, HashcodeUtils};
use eurochef_shared::script::{
    robots_script_command_role, robots_script_payload_diagnostic, RobotsScriptPayloadDiagnostic,
    ScriptCommandTypeCounts, UXGeoScript, UXGeoScriptCommand, UXGeoScriptCommandData,
};
use serde::Serialize;

#[derive(Debug, Clone)]
struct ManifestEntry {
    declared_uid: Option<u32>,
    source_path: PathBuf,
}

#[derive(Debug, Clone, Default)]
struct FileCatalogEntry {
    entities: Vec<u32>,
    scripts: Vec<u32>,
    animations: Vec<u32>,
    skins: Vec<u32>,
    particles: Vec<u32>,
}

#[derive(Debug, Clone, Serialize)]
struct ResourceResolution {
    kind: String,
    serialized_file: u32,
    serialized_hash: u32,
    source_file: u32,
    available_count: Option<usize>,
    resolved_hash: Option<u32>,
    status: String,
}

#[derive(Debug, Clone, Serialize)]
struct ScriptCommandHealth {
    command_index: usize,
    opcode: u8,
    start: i16,
    length: u16,
    controller_header_index: u16,
    controller_index: u8,
    parent_controller_index: u8,
    command_kind: String,
    native_role: String,
    native_family: String,
    runtime_subtype: Option<u8>,
    resolutions: Vec<ResourceResolution>,
    event_type: Option<u32>,
    payload_size: usize,
    native_payload: Option<RobotsScriptPayloadDiagnostic>,
}

#[derive(Debug, Clone, Serialize)]
struct ScriptHealthRow {
    edb_path: String,
    edb_uid: u32,
    declared_edb_uid: Option<u32>,
    script_uid: u32,
    framerate: f32,
    length: u32,
    num_threads: u32,
    command_count: usize,
    entity_commands: usize,
    animation_commands: usize,
    subscript_commands: usize,
    particle_commands: usize,
    sound_commands: usize,
    event_commands: usize,
    unknown_commands: usize,
    logical_controller_slots: usize,
    parsed_controller_slots: usize,
    nonempty_controller_slots: usize,
    missing_controller_slots: usize,
    unresolved_entities: usize,
    unresolved_subscripts: usize,
    unresolved_animations: usize,
    animations_without_skin: usize,
    unresolved_particles: usize,
    unsupported_unknown_commands: usize,
    structural_commands: usize,
    renderable_commands: usize,
    resolved_renderable_commands: usize,
    cycle_detected: bool,
    family: String,
    status: String,
    primary_failure_reason: String,
    commands: Vec<ScriptCommandHealth>,
}

#[derive(Debug, Default, Serialize)]
struct ScriptHealthSummary {
    manifest_entries: usize,
    files_indexed: usize,
    files_failed: usize,
    scripts_parsed: usize,
    scripts_parse_failed: usize,
    geometry_scripts: usize,
    effect_control_scripts: usize,
    scripts_with_cycles: usize,
    scripts_with_missing_controller_slots: usize,
    unresolved_entities: usize,
    unresolved_subscripts: usize,
    unresolved_animations: usize,
    animations_without_skin: usize,
    unresolved_particles: usize,
    unsupported_unknown_commands: usize,
    structural_commands: usize,
    command_totals: ScriptCommandTypeCounts,
    status_counts: BTreeMap<String, usize>,
    family_counts: BTreeMap<String, usize>,
    unknown_opcode_counts: BTreeMap<String, usize>,
    native_role_counts: BTreeMap<String, usize>,
    event_type_counts: BTreeMap<String, usize>,
}

#[derive(Debug, Serialize)]
struct ScriptParseError {
    edb_uid: Option<u32>,
    script_uid: Option<u32>,
    source_path: String,
    error: String,
}

#[derive(Debug, Serialize)]
struct ScriptHealthReport {
    manifest_path: String,
    summary: ScriptHealthSummary,
    parse_errors: Vec<ScriptParseError>,
    rows: Vec<ScriptHealthRow>,
}

pub fn execute_command(manifest_path: String, output_folder: Option<String>) -> Result<()> {
    let manifest_path = PathBuf::from(manifest_path);
    let output_folder = output_folder
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./script_health_report"));
    std::fs::create_dir_all(&output_folder)?;

    let manifest_entries = read_manifest(&manifest_path)?;
    let (catalog, mut parse_errors) = build_file_catalog(&manifest_entries);
    let mut summary = ScriptHealthSummary {
        manifest_entries: manifest_entries.len(),
        files_indexed: catalog.len(),
        files_failed: parse_errors.len(),
        ..Default::default()
    };
    let mut rows = Vec::new();

    for entry in &manifest_entries {
        let Some(platform) = Platform::from_path(&entry.source_path) else {
            parse_errors.push(ScriptParseError {
                edb_uid: entry.declared_uid,
                script_uid: None,
                source_path: entry.source_path.to_string_lossy().into_owned(),
                error: "platform detection failed".to_string(),
            });
            summary.files_failed += 1;
            continue;
        };

        let file = match File::open(&entry.source_path) {
            Ok(file) => file,
            Err(error) => {
                parse_errors.push(ScriptParseError {
                    edb_uid: entry.declared_uid,
                    script_uid: None,
                    source_path: entry.source_path.to_string_lossy().into_owned(),
                    error: error.to_string(),
                });
                summary.files_failed += 1;
                continue;
            }
        };
        let reader = BufReader::new(file);
        let mut edb = match EdbFile::new(Box::new(reader), platform) {
            Ok(edb) => edb,
            Err(error) => {
                parse_errors.push(ScriptParseError {
                    edb_uid: entry.declared_uid,
                    script_uid: None,
                    source_path: entry.source_path.to_string_lossy().into_owned(),
                    error: error.to_string(),
                });
                summary.files_failed += 1;
                continue;
            }
        };
        let header = edb.header.clone();

        for script_header in &header.animscript_list {
            match UXGeoScript::read(script_header, &mut edb) {
                Ok(script) => rows.push(analyze_script(
                    entry,
                    header.hashcode,
                    &script,
                    &catalog,
                    &mut summary,
                )),
                Err(error) => {
                    summary.scripts_parse_failed += 1;
                    parse_errors.push(ScriptParseError {
                        edb_uid: Some(header.hashcode),
                        script_uid: Some(script_header.hashcode),
                        source_path: entry.source_path.to_string_lossy().into_owned(),
                        error: error.to_string(),
                    });
                }
            }
        }
    }

    mark_subscript_cycles(&mut rows);
    finalize_summary(&mut summary, &rows);

    let report = ScriptHealthReport {
        manifest_path: manifest_path.to_string_lossy().into_owned(),
        summary,
        parse_errors,
        rows,
    };

    std::fs::write(
        output_folder.join("script_health_report.json"),
        serde_json::to_string_pretty(&report)?,
    )?;
    write_rows_tsv(
        &output_folder.join("script_health_report.tsv"),
        &report.rows,
        false,
    )?;
    write_rows_tsv(
        &output_folder.join("effect_control_scripts.tsv"),
        &report.rows,
        true,
    )?;
    write_commands_tsv(
        &output_folder.join("script_health_commands.tsv"),
        &report.rows,
    )?;
    write_summary_tsv(
        &output_folder.join("script_health_summary.tsv"),
        &report.summary,
    )?;

    info!(
        "Wrote health report for {} scripts from {} EDB files to {}",
        report.rows.len(),
        report.summary.files_indexed,
        output_folder.display()
    );
    Ok(())
}

fn analyze_script(
    entry: &ManifestEntry,
    edb_uid: u32,
    script: &UXGeoScript,
    catalog: &HashMap<u32, FileCatalogEntry>,
    summary: &mut ScriptHealthSummary,
) -> ScriptHealthRow {
    let counts = script.command_type_counts();
    add_command_counts(&mut summary.command_totals, counts);
    let renderable_commands = counts.entities + counts.animations + counts.subscripts;

    let mut commands = Vec::with_capacity(script.commands.len());
    let mut missing_controller_slots = 0;
    let mut unresolved_entities = 0;
    let mut unresolved_subscripts = 0;
    let mut unresolved_animations = 0;
    let mut animations_without_skin = 0;
    let mut unresolved_particles = 0;
    let mut unsupported_unknown_commands = 0;
    let mut structural_commands = 0;
    let mut resolved_renderable_commands = 0;

    for (command_index, command) in script.commands.iter().enumerate() {
        if command.uses_controller_header()
            && command.controller_header_index != u16::MAX
            && command.controller_header_index as usize >= script.controllers.len()
        {
            missing_controller_slots += 1;
        }

        let detail = analyze_command(edb_uid, command_index, command, catalog);
        let resolved = |resolution: &ResourceResolution| resolution.status.starts_with("resolved_");
        match &command.data {
            UXGeoScriptCommandData::Entity { .. } => {
                if detail.resolutions.iter().all(resolved) {
                    resolved_renderable_commands += 1;
                } else {
                    unresolved_entities += 1;
                }
            }
            UXGeoScriptCommandData::SubScript { .. } => {
                if detail.resolutions.iter().all(resolved) {
                    resolved_renderable_commands += 1;
                } else {
                    unresolved_subscripts += 1;
                }
            }
            UXGeoScriptCommandData::Animation { .. } => {
                let skin_absent = detail.resolutions.iter().any(|resolution| {
                    resolution.kind == "skin" && resolution.status == "absent_optional"
                });
                let true_missing = detail.resolutions.iter().any(|resolution| {
                    !resolved(resolution) && resolution.status != "absent_optional"
                });
                if true_missing {
                    unresolved_animations += 1;
                } else if skin_absent {
                    animations_without_skin += 1;
                } else {
                    resolved_renderable_commands += 1;
                }
            }
            UXGeoScriptCommandData::Particle { .. } => {
                if !detail.resolutions.iter().all(resolved) {
                    unresolved_particles += 1;
                }
            }
            UXGeoScriptCommandData::Unknown { cmd, .. } => {
                increment(&mut summary.unknown_opcode_counts, cmd.to_string());
                let role = robots_script_command_role(*cmd, detail.payload_size);
                increment(&mut summary.native_role_counts, role.name.to_string());
                match role.family {
                    "terminator" | "control" | "effect" | "geometry" => structural_commands += 1,
                    _ => unsupported_unknown_commands += 1,
                }
            }
            UXGeoScriptCommandData::Event { event_type, .. } => {
                increment(
                    &mut summary.event_type_counts,
                    format!("0x{event_type:08X}"),
                );
            }
            UXGeoScriptCommandData::Sound { .. } => {}
        }
        commands.push(detail);
    }

    let family = classify_family(counts, unsupported_unknown_commands);
    let (status, primary_failure_reason) = classify_status(
        renderable_commands,
        missing_controller_slots,
        unresolved_entities,
        unresolved_subscripts,
        unresolved_animations,
        animations_without_skin,
        unresolved_particles,
        unsupported_unknown_commands,
    );

    ScriptHealthRow {
        edb_path: entry.source_path.to_string_lossy().into_owned(),
        edb_uid,
        declared_edb_uid: entry.declared_uid,
        script_uid: script.hashcode,
        framerate: script.framerate,
        length: script.length,
        num_threads: script.num_threads,
        command_count: script.commands.len(),
        entity_commands: counts.entities,
        animation_commands: counts.animations,
        subscript_commands: counts.subscripts,
        particle_commands: counts.particles,
        sound_commands: counts.sounds,
        event_commands: counts.events,
        unknown_commands: counts.unknown,
        logical_controller_slots: logical_controller_slots(script),
        parsed_controller_slots: script.controllers.len(),
        nonempty_controller_slots: script
            .controllers
            .iter()
            .filter(|controller| {
                controller.controller_count != 0
                    || controller.channel_count != 0
                    || controller.ctrl_mask != 0
                    || controller.ctrl_channel_mask != 0
            })
            .count(),
        missing_controller_slots,
        unresolved_entities,
        unresolved_subscripts,
        unresolved_animations,
        animations_without_skin,
        unresolved_particles,
        unsupported_unknown_commands,
        structural_commands,
        renderable_commands,
        resolved_renderable_commands,
        cycle_detected: false,
        family,
        status,
        primary_failure_reason,
        commands,
    }
}

fn analyze_command(
    current_file: u32,
    command_index: usize,
    command: &UXGeoScriptCommand,
    catalog: &HashMap<u32, FileCatalogEntry>,
) -> ScriptCommandHealth {
    let mut resolutions = Vec::new();
    let serialized_payload_size = match &command.data {
        UXGeoScriptCommandData::Unknown { data, .. }
        | UXGeoScriptCommandData::Event { data, .. } => data.len(),
        UXGeoScriptCommandData::Sound { .. } => 24,
        UXGeoScriptCommandData::Animation { .. } => 24,
        UXGeoScriptCommandData::Entity { .. }
        | UXGeoScriptCommandData::SubScript { .. }
        | UXGeoScriptCommandData::Particle { .. } => 12,
    };
    let role = robots_script_command_role(command.opcode, serialized_payload_size);
    let native_payload = match &command.data {
        UXGeoScriptCommandData::Unknown { cmd, data } => {
            robots_script_payload_diagnostic(*cmd, data)
        }
        _ => None,
    };
    let (command_kind, event_type, payload_size) = match &command.data {
        UXGeoScriptCommandData::Entity { hashcode, file } => {
            resolutions.push(resolve_resource(
                current_file,
                *file,
                *hashcode,
                "entity",
                catalog,
            ));
            ("entity".to_string(), None, 0)
        }
        UXGeoScriptCommandData::SubScript { hashcode, file } => {
            resolutions.push(resolve_resource(
                current_file,
                *file,
                *hashcode,
                "script",
                catalog,
            ));
            ("subscript".to_string(), None, 0)
        }
        UXGeoScriptCommandData::Particle { hashcode, file } => {
            resolutions.push(resolve_resource(
                current_file,
                *file,
                *hashcode,
                "particle",
                catalog,
            ));
            ("particle".to_string(), None, 0)
        }
        UXGeoScriptCommandData::Animation {
            skin_file,
            skin_hashcode,
            anim_file,
            anim_hashcode,
        } => {
            resolutions.push(resolve_resource(
                current_file,
                *skin_file,
                *skin_hashcode,
                "skin",
                catalog,
            ));
            resolutions.push(resolve_resource(
                current_file,
                *anim_file,
                *anim_hashcode,
                "animation",
                catalog,
            ));
            ("animation".to_string(), None, 0)
        }
        UXGeoScriptCommandData::Sound { hashcode } => {
            resolutions.push(ResourceResolution {
                kind: "sound".to_string(),
                serialized_file: u32::MAX,
                serialized_hash: *hashcode,
                source_file: current_file,
                available_count: None,
                resolved_hash: None,
                status: "external_runtime_unchecked".to_string(),
            });
            ("sound".to_string(), None, 0)
        }
        UXGeoScriptCommandData::Event { event_type, data } => {
            ("event".to_string(), Some(*event_type), data.len())
        }
        UXGeoScriptCommandData::Unknown { cmd, data } => (
            if *cmd == 18 { "terminator" } else { "unknown" }.to_string(),
            None,
            data.len(),
        ),
    };

    ScriptCommandHealth {
        command_index,
        opcode: command.opcode,
        start: command.start,
        length: command.length,
        controller_header_index: command.controller_header_index,
        controller_index: command.controller_index,
        parent_controller_index: command.parent_controller_index,
        command_kind,
        native_role: role.name.to_string(),
        native_family: role.family.to_string(),
        runtime_subtype: role.runtime_subtype,
        resolutions,
        event_type,
        payload_size,
        native_payload,
    }
}

fn resolve_resource(
    current_file: u32,
    serialized_file: u32,
    serialized_hash: u32,
    kind: &str,
    catalog: &HashMap<u32, FileCatalogEntry>,
) -> ResourceResolution {
    if kind == "skin" && matches!(serialized_hash, 0 | u32::MAX) {
        return ResourceResolution {
            kind: kind.to_string(),
            serialized_file,
            serialized_hash,
            source_file: current_file,
            available_count: None,
            resolved_hash: None,
            status: "absent_optional".to_string(),
        };
    }

    let source_file = if serialized_file == u32::MAX || serialized_hash.is_local() {
        current_file
    } else {
        serialized_file
    };
    let Some(source) = catalog.get(&source_file) else {
        return ResourceResolution {
            kind: kind.to_string(),
            serialized_file,
            serialized_hash,
            source_file,
            available_count: None,
            resolved_hash: None,
            status: "missing_source_file".to_string(),
        };
    };
    let list = resource_list(source, kind);

    if serialized_hash.is_local() {
        let index = serialized_hash.index() as usize;
        if let Some(resolved_hash) = list.get(index) {
            ResourceResolution {
                kind: kind.to_string(),
                serialized_file,
                serialized_hash,
                source_file,
                available_count: Some(list.len()),
                resolved_hash: Some(*resolved_hash),
                status: "resolved_local".to_string(),
            }
        } else {
            ResourceResolution {
                kind: kind.to_string(),
                serialized_file,
                serialized_hash,
                source_file,
                available_count: Some(list.len()),
                resolved_hash: None,
                status: "local_index_out_of_range".to_string(),
            }
        }
    } else if list.contains(&serialized_hash) {
        ResourceResolution {
            kind: kind.to_string(),
            serialized_file,
            serialized_hash,
            source_file,
            available_count: Some(list.len()),
            resolved_hash: Some(serialized_hash),
            status: "resolved_global".to_string(),
        }
    } else {
        ResourceResolution {
            kind: kind.to_string(),
            serialized_file,
            serialized_hash,
            source_file,
            available_count: Some(list.len()),
            resolved_hash: None,
            status: "resource_missing".to_string(),
        }
    }
}

fn resource_list<'a>(source: &'a FileCatalogEntry, kind: &str) -> &'a [u32] {
    match kind {
        "entity" => &source.entities,
        "script" => &source.scripts,
        "animation" => &source.animations,
        "skin" => &source.skins,
        "particle" => &source.particles,
        _ => &[],
    }
}

fn logical_controller_slots(script: &UXGeoScript) -> usize {
    script
        .commands
        .iter()
        .filter(|command| {
            command.uses_controller_header() && command.controller_header_index != u16::MAX
        })
        .map(|command| command.controller_header_index as usize + 1)
        .max()
        .unwrap_or(0)
}

fn classify_family(counts: ScriptCommandTypeCounts, unsupported_unknown_commands: usize) -> String {
    if counts.entities + counts.animations + counts.subscripts > 0 {
        return "geometry".to_string();
    }
    let mut parts = Vec::new();
    if counts.particles > 0 {
        parts.push("particle");
    }
    if counts.sounds > 0 {
        parts.push("sound");
    }
    if counts.events > 0 {
        parts.push("event");
    }
    if unsupported_unknown_commands > 0 {
        parts.push("unknown");
    }
    if parts.is_empty() {
        "empty_control".to_string()
    } else {
        parts.join("+")
    }
}

#[allow(clippy::too_many_arguments)]
fn classify_status(
    renderable_commands: usize,
    missing_controller_slots: usize,
    unresolved_entities: usize,
    unresolved_subscripts: usize,
    unresolved_animations: usize,
    animations_without_skin: usize,
    unresolved_particles: usize,
    unsupported_unknown_commands: usize,
) -> (String, String) {
    if missing_controller_slots > 0 {
        return (
            "controller_mismatch".to_string(),
            format!("{missing_controller_slots} command controller references exceed parsed slots"),
        );
    }
    let unresolved_geometry = unresolved_entities + unresolved_subscripts + unresolved_animations;
    if unresolved_geometry > 0 {
        return (
            "dependency_failed".to_string(),
            format!("{unresolved_geometry} geometry command dependencies unresolved"),
        );
    }
    if renderable_commands > 0 {
        if animations_without_skin > 0 {
            return (
                "partial_animation_binding".to_string(),
                format!("{animations_without_skin} animation commands have no serialized AnimSkin"),
            );
        }
        return ("complete".to_string(), String::new());
    }
    if unresolved_particles > 0 {
        return (
            "dependency_failed".to_string(),
            format!("{unresolved_particles} particle resources unresolved"),
        );
    }
    if unsupported_unknown_commands > 0 {
        return (
            "unsupported_command".to_string(),
            format!("{unsupported_unknown_commands} non-terminator unknown commands require classification"),
        );
    }
    (
        "effect_control".to_string(),
        "no Entity/Animation/SubScript geometry; particle/event/sound preview required".to_string(),
    )
}

fn mark_subscript_cycles(rows: &mut [ScriptHealthRow]) {
    let index_by_key: HashMap<(u32, u32), usize> = rows
        .iter()
        .enumerate()
        .map(|(index, row)| ((row.edb_uid, row.script_uid), index))
        .collect();
    let adjacency: Vec<Vec<usize>> = rows
        .iter()
        .map(|row| {
            row.commands
                .iter()
                .filter(|command| command.command_kind == "subscript")
                .flat_map(|command| &command.resolutions)
                .filter_map(|resolution| {
                    let hash = resolution.resolved_hash?;
                    index_by_key.get(&(resolution.source_file, hash)).copied()
                })
                .collect()
        })
        .collect();

    let mut cycle_nodes = HashSet::new();
    let mut state = vec![0u8; rows.len()];
    let mut stack = Vec::new();
    for node in 0..rows.len() {
        find_cycles(node, &adjacency, &mut state, &mut stack, &mut cycle_nodes);
    }
    for node in cycle_nodes {
        rows[node].cycle_detected = true;
    }
}

fn find_cycles(
    node: usize,
    adjacency: &[Vec<usize>],
    state: &mut [u8],
    stack: &mut Vec<usize>,
    cycle_nodes: &mut HashSet<usize>,
) {
    if state[node] == 2 {
        return;
    }
    if state[node] == 1 {
        if let Some(position) = stack.iter().position(|value| *value == node) {
            cycle_nodes.extend(stack[position..].iter().copied());
        }
        return;
    }
    state[node] = 1;
    stack.push(node);
    for next in &adjacency[node] {
        find_cycles(*next, adjacency, state, stack, cycle_nodes);
    }
    stack.pop();
    state[node] = 2;
}

fn finalize_summary(summary: &mut ScriptHealthSummary, rows: &[ScriptHealthRow]) {
    summary.scripts_parsed = rows.len();
    for row in rows {
        if row.renderable_commands > 0 {
            summary.geometry_scripts += 1;
        } else {
            summary.effect_control_scripts += 1;
        }
        if row.cycle_detected {
            summary.scripts_with_cycles += 1;
        }
        if row.missing_controller_slots > 0 {
            summary.scripts_with_missing_controller_slots += 1;
        }
        summary.unresolved_entities += row.unresolved_entities;
        summary.unresolved_subscripts += row.unresolved_subscripts;
        summary.unresolved_animations += row.unresolved_animations;
        summary.animations_without_skin += row.animations_without_skin;
        summary.unresolved_particles += row.unresolved_particles;
        summary.unsupported_unknown_commands += row.unsupported_unknown_commands;
        summary.structural_commands += row.structural_commands;
        increment(&mut summary.status_counts, row.status.clone());
        increment(&mut summary.family_counts, row.family.clone());
    }
}

fn build_file_catalog(
    manifest_entries: &[ManifestEntry],
) -> (HashMap<u32, FileCatalogEntry>, Vec<ScriptParseError>) {
    let mut catalog = HashMap::new();
    let mut errors = Vec::new();
    for entry in manifest_entries {
        let result = (|| -> Result<(u32, FileCatalogEntry)> {
            let platform = Platform::from_path(&entry.source_path)
                .with_context(|| format!("detect platform for {}", entry.source_path.display()))?;
            let file = File::open(&entry.source_path)?;
            let reader = BufReader::new(file);
            let edb = EdbFile::new(Box::new(reader), platform)?;
            let header = &edb.header;
            Ok((
                header.hashcode,
                FileCatalogEntry {
                    entities: header
                        .entity_list
                        .iter()
                        .map(|value| value.common.hashcode)
                        .collect(),
                    scripts: header
                        .animscript_list
                        .iter()
                        .map(|value| value.hashcode)
                        .collect(),
                    animations: header
                        .anim_list
                        .iter()
                        .map(|value| value.common.hashcode)
                        .collect(),
                    skins: header
                        .animskin_list
                        .iter()
                        .map(|value| value.common.hashcode)
                        .collect(),
                    particles: header
                        .particle_list
                        .iter()
                        .map(|value| value.hashcode)
                        .collect(),
                },
            ))
        })();
        match result {
            Ok((uid, file)) => {
                catalog.insert(uid, file);
            }
            Err(error) => errors.push(ScriptParseError {
                edb_uid: entry.declared_uid,
                script_uid: None,
                source_path: entry.source_path.to_string_lossy().into_owned(),
                error: format!("{error:#}"),
            }),
        }
    }
    (catalog, errors)
}

fn read_manifest(path: &Path) -> Result<Vec<ManifestEntry>> {
    let manifest = std::fs::read_to_string(path)
        .with_context(|| format!("read manifest {}", path.display()))?;
    let base = path.parent().unwrap_or_else(|| Path::new("."));
    let mut entries = Vec::new();
    for (line_index, line) in manifest.lines().enumerate() {
        if line_index == 0 {
            continue;
        }
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((uid_text, source_text)) = line.split_once('\t') else {
            continue;
        };
        let source_path = PathBuf::from(source_text.trim());
        entries.push(ManifestEntry {
            declared_uid: parse_u32(uid_text.trim()),
            source_path: if source_path.is_absolute() {
                source_path
            } else {
                base.join(source_path)
            },
        });
    }
    Ok(entries)
}

fn parse_u32(value: &str) -> Option<u32> {
    let value = value.trim();
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u32::from_str_radix(hex, 16).ok()
    } else if value.len() == 8 && value.chars().all(|char| char.is_ascii_hexdigit()) {
        u32::from_str_radix(value, 16).ok()
    } else {
        value.parse().ok()
    }
}

fn add_command_counts(total: &mut ScriptCommandTypeCounts, counts: ScriptCommandTypeCounts) {
    total.entities += counts.entities;
    total.animations += counts.animations;
    total.subscripts += counts.subscripts;
    total.particles += counts.particles;
    total.sounds += counts.sounds;
    total.events += counts.events;
    total.unknown += counts.unknown;
}

fn increment(map: &mut BTreeMap<String, usize>, key: String) {
    *map.entry(key).or_default() += 1;
}

fn write_rows_tsv(path: &Path, rows: &[ScriptHealthRow], effect_control_only: bool) -> Result<()> {
    let mut file = File::create(path)?;
    writeln!(
        file,
        "edb_path\tedb_uid\tscript_uid\tcommand_count\tentity_commands\tanimation_commands\tsubscript_commands\tparticle_commands\tsound_commands\tevent_commands\tunknown_commands\tlogical_controller_slots\tparsed_controller_slots\tnonempty_controller_slots\tmissing_controller_slots\tunresolved_entities\tunresolved_subscripts\tunresolved_animations\tanimations_without_skin\tunresolved_particles\tunsupported_unknown_commands\tstructural_commands\trenderable_commands\tresolved_renderable_commands\tcycle_detected\tfamily\tstatus\tprimary_failure_reason"
    )?;
    for row in rows {
        if effect_control_only && row.renderable_commands != 0 {
            continue;
        }
        let fields = [
            escape_tsv(&row.edb_path),
            format!("0x{:08X}", row.edb_uid),
            format!("0x{:08X}", row.script_uid),
            row.command_count.to_string(),
            row.entity_commands.to_string(),
            row.animation_commands.to_string(),
            row.subscript_commands.to_string(),
            row.particle_commands.to_string(),
            row.sound_commands.to_string(),
            row.event_commands.to_string(),
            row.unknown_commands.to_string(),
            row.logical_controller_slots.to_string(),
            row.parsed_controller_slots.to_string(),
            row.nonempty_controller_slots.to_string(),
            row.missing_controller_slots.to_string(),
            row.unresolved_entities.to_string(),
            row.unresolved_subscripts.to_string(),
            row.unresolved_animations.to_string(),
            row.animations_without_skin.to_string(),
            row.unresolved_particles.to_string(),
            row.unsupported_unknown_commands.to_string(),
            row.structural_commands.to_string(),
            row.renderable_commands.to_string(),
            row.resolved_renderable_commands.to_string(),
            row.cycle_detected.to_string(),
            row.family.clone(),
            row.status.clone(),
            escape_tsv(&row.primary_failure_reason),
        ];
        writeln!(file, "{}", fields.join("\t"))?;
    }
    Ok(())
}

fn write_commands_tsv(path: &Path, rows: &[ScriptHealthRow]) -> Result<()> {
    let mut file = File::create(path)?;
    writeln!(
        file,
        "edb_uid\tscript_uid\tcommand_index\topcode\tcommand_kind\tnative_role\tnative_family\truntime_subtype\tstart\tlength\tcontroller_header_index\tcontroller_index\tparent_controller_index\tevent_type\tpayload_size\tnative_payload\tresolutions"
    )?;
    for row in rows {
        for command in &row.commands {
            writeln!(
                file,
                "0x{:08X}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                row.edb_uid,
                row.script_uid,
                command.command_index,
                command.opcode,
                command.command_kind,
                command.native_role,
                command.native_family,
                command
                    .runtime_subtype
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                command.start,
                command.length,
                command.controller_header_index,
                command.controller_index,
                command.parent_controller_index,
                command
                    .event_type
                    .map(|value| format!("0x{value:08X}"))
                    .unwrap_or_default(),
                command.payload_size,
                escape_tsv(&serde_json::to_string(&command.native_payload)?),
                escape_tsv(&serde_json::to_string(&command.resolutions)?),
            )?;
        }
    }
    Ok(())
}

fn write_summary_tsv(path: &Path, summary: &ScriptHealthSummary) -> Result<()> {
    let mut file = File::create(path)?;
    writeln!(file, "category\tkey\tcount")?;
    let metrics = [
        ("manifest_entries", summary.manifest_entries),
        ("files_indexed", summary.files_indexed),
        ("files_failed", summary.files_failed),
        ("scripts_parsed", summary.scripts_parsed),
        ("scripts_parse_failed", summary.scripts_parse_failed),
        ("geometry_scripts", summary.geometry_scripts),
        ("effect_control_scripts", summary.effect_control_scripts),
        ("scripts_with_cycles", summary.scripts_with_cycles),
        (
            "scripts_with_missing_controller_slots",
            summary.scripts_with_missing_controller_slots,
        ),
        ("unresolved_entities", summary.unresolved_entities),
        ("unresolved_subscripts", summary.unresolved_subscripts),
        ("unresolved_animations", summary.unresolved_animations),
        ("animations_without_skin", summary.animations_without_skin),
        ("unresolved_particles", summary.unresolved_particles),
        (
            "unsupported_unknown_commands",
            summary.unsupported_unknown_commands,
        ),
        ("structural_commands", summary.structural_commands),
        ("entity_commands", summary.command_totals.entities),
        ("animation_commands", summary.command_totals.animations),
        ("subscript_commands", summary.command_totals.subscripts),
        ("particle_commands", summary.command_totals.particles),
        ("sound_commands", summary.command_totals.sounds),
        ("event_commands", summary.command_totals.events),
        ("raw_unknown_commands", summary.command_totals.unknown),
    ];
    for (key, count) in metrics {
        writeln!(file, "metric\t{key}\t{count}")?;
    }
    for (key, count) in &summary.status_counts {
        writeln!(file, "status\t{}\t{}", escape_tsv(key), count)?;
    }
    for (key, count) in &summary.family_counts {
        writeln!(file, "family\t{}\t{}", escape_tsv(key), count)?;
    }
    for (key, count) in &summary.unknown_opcode_counts {
        writeln!(file, "unknown_opcode\t{}\t{}", escape_tsv(key), count)?;
    }
    for (key, count) in &summary.native_role_counts {
        writeln!(file, "native_role\t{}\t{}", escape_tsv(key), count)?;
    }
    for (key, count) in &summary.event_type_counts {
        writeln!(file, "event_type\t{}\t{}", escape_tsv(key), count)?;
    }
    Ok(())
}

fn escape_tsv(value: &str) -> String {
    value.replace('\t', " ").replace(['\r', '\n'], " ")
}

#[cfg(test)]
mod tests {
    use super::{classify_family, classify_status, parse_u32, resolve_resource};
    use eurochef_shared::script::ScriptCommandTypeCounts;
    use std::collections::HashMap;

    #[test]
    fn classifies_effect_control_families_without_calling_them_geometry() {
        let counts = ScriptCommandTypeCounts {
            particles: 2,
            events: 1,
            ..Default::default()
        };
        assert_eq!(classify_family(counts, 0), "particle+event");
        assert_eq!(classify_status(0, 0, 0, 0, 0, 0, 0, 0).0, "effect_control");
    }

    #[test]
    fn dependency_and_controller_failures_take_priority() {
        assert_eq!(
            classify_status(1, 1, 0, 0, 0, 0, 0, 0).0,
            "controller_mismatch"
        );
        assert_eq!(
            classify_status(1, 0, 1, 0, 0, 0, 0, 0).0,
            "dependency_failed"
        );
    }

    #[test]
    fn absent_animskin_is_optional_and_geometry_stays_partial() {
        let catalog = HashMap::new();
        let resolution = resolve_resource(0x0100_0020, u32::MAX, u32::MAX, "skin", &catalog);
        assert_eq!(resolution.status, "absent_optional");
        assert_eq!(
            classify_status(1, 0, 0, 0, 0, 1, 0, 0).0,
            "partial_animation_binding"
        );
    }

    #[test]
    fn parses_manifest_hashes() {
        assert_eq!(parse_u32("01000071"), Some(0x0100_0071));
        assert_eq!(parse_u32("113"), Some(113));
    }
}
