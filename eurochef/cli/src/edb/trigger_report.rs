use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    io::{BufReader, Seek, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use eurochef_edb::{
    binrw::BinReaderExt,
    edb::EdbFile,
    map::{EXGeoBaseDatum, EXGeoMap},
    versions::Platform,
    HashcodeUtils,
};
use eurochef_shared::maps::{TriggerDefinition, TriggerInformation};
use serde::Serialize;

#[derive(Debug, Clone)]
struct ManifestEntry {
    declared_uid: Option<u32>,
    source_path: PathBuf,
}

#[derive(Debug, Clone)]
struct FileCatalogEntry {
    entities: Vec<u32>,
}

#[derive(Debug, Serialize)]
pub struct TriggerCorpusReport {
    manifest_path: String,
    summary: TriggerCorpusSummary,
    file_errors: Vec<TriggerFileError>,
    rows: Vec<TriggerReportRow>,
}

#[derive(Debug, Default, Serialize)]
pub struct TriggerCorpusSummary {
    manifest_entries: usize,
    files_indexed: usize,
    files_failed: usize,
    files_without_maps: usize,
    maps: usize,
    triggers: usize,
    invalid_outgoing_links: usize,
    missing_trigger_script_bindings: usize,
    missing_collision_bindings: usize,
    unresolved_visual_objects: usize,
    triggers_with_path_hash_matches: usize,
    trigger_type_counts: BTreeMap<String, usize>,
    trigger_subtype_counts: BTreeMap<String, usize>,
    trigger_script_status_counts: BTreeMap<String, usize>,
    trigger_path_match_counts: BTreeMap<String, usize>,
    runtime_preview_status_counts: BTreeMap<String, usize>,
    runtime_preview_mode_counts: BTreeMap<String, usize>,
    collision_status_counts: BTreeMap<String, usize>,
    collision_type_counts: BTreeMap<String, usize>,
    visual_status_counts: BTreeMap<String, usize>,
}

#[derive(Debug, Serialize)]
pub struct TriggerFileError {
    declared_uid: Option<u32>,
    source_path: String,
    error: String,
}

#[derive(Debug, Serialize)]
pub struct TriggerPathMatch {
    data_slot: usize,
    path_index: usize,
    path_hashcode: u32,
    path_position: [f32; 3],
    path_flags: u32,
    path_type: u16,
    node_positions: Vec<[f32; 3]>,
    node_sizes: Vec<[f32; 2]>,
    node_values: Vec<[u16; 4]>,
    node_flags: Vec<u32>,
    node_distances: Vec<f32>,
    node_link_counts: Vec<u16>,
    links: Vec<[u16; 2]>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TriggerRuntimePreview {
    status: String,
    mode: String,
    limitations: String,
}

#[derive(Debug, Serialize)]
pub struct TriggerScriptBinding {
    index: Option<u32>,
    file_offset: Option<u64>,
    aux: Option<u32>,
    status: String,
}

#[derive(Debug, Serialize)]
pub struct TriggerCollisionBinding {
    index: Option<u32>,
    status: String,
    datum: Option<EXGeoBaseDatum>,
}

#[derive(Debug, Serialize)]
pub struct TriggerVisualBinding {
    object: Option<u32>,
    file: Option<u32>,
    resolved_entity: Option<u32>,
    status: String,
}

#[derive(Debug, Serialize)]
pub struct TriggerReportRow {
    edb_uid: u32,
    declared_edb_uid: Option<u32>,
    edb_path: String,
    map_uid: u32,
    map_index: usize,
    trigger_index: usize,
    trigger_file_offset: u64,
    link_ref: i32,
    type_index: u16,
    trig_type: u32,
    trig_type_name: String,
    trig_subtype: u32,
    trig_subtype_name: Option<String>,
    debug: u16,
    game_flags: u32,
    trig_flags: u32,
    position: [f32; 3],
    rotation: [f32; 3],
    scale: [f32; 3],
    data: [Option<u32>; 16],
    outgoing_links: [i32; 8],
    invalid_outgoing_links: Vec<i32>,
    incoming_links: Vec<usize>,
    path_hash_matches: Vec<TriggerPathMatch>,
    runtime_preview: TriggerRuntimePreview,
    visual: TriggerVisualBinding,
    trigger_script: TriggerScriptBinding,
    collision: TriggerCollisionBinding,
}

pub fn execute_command(
    manifest_path: String,
    output_folder: Option<String>,
    trigger_defs_file: Option<String>,
) -> Result<()> {
    let manifest_path = PathBuf::from(manifest_path);
    let output_folder = output_folder
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./xtrigger_corpus_report"));
    std::fs::create_dir_all(&output_folder)?;

    let trigger_types = trigger_defs_file
        .map(load_trigger_types)
        .transpose()?
        .unwrap_or_default();
    let manifest_entries = read_manifest(&manifest_path)?;
    let (catalog, mut file_errors) = build_file_catalog(&manifest_entries);

    let mut summary = TriggerCorpusSummary {
        manifest_entries: manifest_entries.len(),
        files_indexed: catalog.len(),
        files_failed: file_errors.len(),
        ..Default::default()
    };
    let mut rows = Vec::new();

    for entry in &manifest_entries {
        let Some(platform) = Platform::from_path(&entry.source_path) else {
            if !file_errors
                .iter()
                .any(|error| error.source_path == entry.source_path.to_string_lossy())
            {
                file_errors.push(TriggerFileError {
                    declared_uid: entry.declared_uid,
                    source_path: entry.source_path.to_string_lossy().into_owned(),
                    error: "platform detection failed".to_string(),
                });
                summary.files_failed += 1;
            }
            continue;
        };

        let result = scan_file(
            entry,
            platform,
            &catalog,
            &trigger_types,
            &mut summary,
            &mut rows,
        );
        if let Err(error) = result {
            file_errors.push(TriggerFileError {
                declared_uid: entry.declared_uid,
                source_path: entry.source_path.to_string_lossy().into_owned(),
                error: format!("{error:#}"),
            });
            summary.files_failed += 1;
        }
    }

    let report = TriggerCorpusReport {
        manifest_path: manifest_path.to_string_lossy().into_owned(),
        summary,
        file_errors,
        rows,
    };

    let json_path = output_folder.join("xtrigger_corpus_report.json");
    std::fs::write(&json_path, serde_json::to_string_pretty(&report)?)?;

    let rows_path = output_folder.join("xtrigger_corpus_rows.tsv");
    write_rows_tsv(&rows_path, &report.rows)?;

    let type_summary_path = output_folder.join("xtrigger_type_summary.tsv");
    write_type_summary_tsv(&type_summary_path, &report.summary)?;

    let path_rows_path = output_folder.join("xtrigger_path_rows.tsv");
    write_path_rows_tsv(&path_rows_path, &report.rows)?;

    let path_node_rows_path = output_folder.join("xtrigger_path_node_rows.tsv");
    write_path_node_rows_tsv(&path_node_rows_path, &report.rows)?;

    let runtime_coverage_path = output_folder.join("xtrigger_runtime_coverage.tsv");
    write_runtime_coverage_tsv(&runtime_coverage_path, &report.rows)?;

    info!(
        "Wrote {} XTrigger rows from {} manifest entries to {}",
        report.rows.len(),
        report.summary.manifest_entries,
        output_folder.display()
    );
    Ok(())
}

fn scan_file(
    entry: &ManifestEntry,
    platform: Platform,
    catalog: &HashMap<u32, FileCatalogEntry>,
    trigger_types: &TriggerInformation,
    summary: &mut TriggerCorpusSummary,
    rows: &mut Vec<TriggerReportRow>,
) -> Result<()> {
    let file = File::open(&entry.source_path)
        .with_context(|| format!("open {}", entry.source_path.display()))?;
    let reader = BufReader::new(file);
    let mut edb = EdbFile::new(Box::new(reader), platform)
        .with_context(|| format!("parse header {}", entry.source_path.display()))?;
    let header = edb.header.clone();

    if header.map_list.len() == 0 {
        summary.files_without_maps += 1;
        return Ok(());
    }

    for (map_index, map_header) in header.map_list.iter().enumerate() {
        edb.seek(std::io::SeekFrom::Start(map_header.address as u64))?;
        let map = edb
            .read_type_args::<EXGeoMap>(edb.endian, (header.version,))
            .with_context(|| {
                format!(
                    "parse map 0x{:08X} in {}",
                    map_header.hashcode,
                    entry.source_path.display()
                )
            })?;
        summary.maps += 1;

        let path_indices: HashMap<u32, usize> = map
            .paths
            .iter()
            .enumerate()
            .map(|(index, path)| (path.hashcode, index))
            .collect();
        let incoming_links = build_incoming_links(&map);

        for (trigger_index, trigger_header) in map.trigger_header.triggers.iter().enumerate() {
            let trigger = &trigger_header.trigger;
            let trigger_type = map
                .trigger_header
                .trigger_types
                .get(trigger.type_index as usize)
                .with_context(|| {
                    format!(
                        "trigger {} type index {} is outside type table",
                        trigger_index, trigger.type_index
                    )
                })?;

            let trig_type_name = trigger_name(trigger_types, trigger_type.trig_type);
            let trig_subtype_name = (trigger_type.trig_subtype != 0
                && trigger_type.trig_subtype != 0x4200_0001)
                .then(|| trigger_name(trigger_types, trigger_type.trig_subtype));
            increment(
                &mut summary.trigger_type_counts,
                format!("{}:{}", trigger_type.trig_type, trig_type_name),
            );
            if let Some(name) = &trig_subtype_name {
                increment(
                    &mut summary.trigger_subtype_counts,
                    format!("{}:{}", trigger_type.trig_subtype, name),
                );
            }

            let invalid_outgoing_links =
                invalid_links(&trigger.links, map.trigger_header.triggers.len());
            summary.invalid_outgoing_links += invalid_outgoing_links.len();

            let path_hash_matches = trigger
                .data
                .iter()
                .enumerate()
                .filter_map(|(data_slot, value)| {
                    let value = value.as_ref()?;
                    path_indices.get(value).map(|path_index| {
                        let path = &map.paths[*path_index];
                        TriggerPathMatch {
                            data_slot,
                            path_index: *path_index,
                            path_hashcode: *value,
                            path_position: path.position,
                            path_flags: path.flags,
                            path_type: path.ptype,
                            node_positions: path.nodes.iter().map(|node| node.position).collect(),
                            node_sizes: path.nodes.iter().map(|node| node.size).collect(),
                            node_values: path.nodes.iter().map(|node| node.value).collect(),
                            node_flags: path.nodes.iter().map(|node| node.flags).collect(),
                            node_distances: path.nodes.iter().map(|node| node.distance).collect(),
                            node_link_counts: path
                                .nodes
                                .iter()
                                .map(|node| node.num_links)
                                .collect(),
                            links: path
                                .links
                                .iter()
                                .map(|link| [link.node_a, link.node_b])
                                .collect(),
                        }
                    })
                })
                .collect::<Vec<_>>();
            if !path_hash_matches.is_empty() {
                summary.triggers_with_path_hash_matches += 1;
                for path_match in &path_hash_matches {
                    increment(
                        &mut summary.trigger_path_match_counts,
                        format!(
                            "{}:{}:data{}",
                            trigger_type.trig_type, trig_type_name, path_match.data_slot
                        ),
                    );
                }
            }

            let runtime_preview =
                classify_runtime_preview(trigger_type.trig_type, &trigger.data, &path_hash_matches);
            increment(
                &mut summary.runtime_preview_status_counts,
                runtime_preview.status.clone(),
            );
            increment(
                &mut summary.runtime_preview_mode_counts,
                runtime_preview.mode.clone(),
            );

            let trigger_script =
                resolve_trigger_script(&map, trigger.engine_options.gamescript_index);
            if trigger_script.status == "invalid_index" {
                summary.missing_trigger_script_bindings += 1;
            }
            increment(
                &mut summary.trigger_script_status_counts,
                trigger_script.status.clone(),
            );

            let collision = resolve_collision(&map, trigger.engine_options.collision_index);
            if collision.status == "invalid_index" {
                summary.missing_collision_bindings += 1;
            }
            increment(
                &mut summary.collision_status_counts,
                collision.status.clone(),
            );
            if let Some(datum) = &collision.datum {
                increment(
                    &mut summary.collision_type_counts,
                    format!("{}", datum.dtype),
                );
            }

            let visual = resolve_visual(
                header.hashcode,
                trigger.engine_options.visual_object,
                trigger.engine_options.visual_object_file,
                catalog,
            );
            if !matches!(
                visual.status.as_str(),
                "none" | "resolved_global" | "resolved_local"
            ) {
                summary.unresolved_visual_objects += 1;
            }
            increment(&mut summary.visual_status_counts, visual.status.clone());

            rows.push(TriggerReportRow {
                edb_uid: header.hashcode,
                declared_edb_uid: entry.declared_uid,
                edb_path: entry.source_path.to_string_lossy().into_owned(),
                map_uid: map_header.hashcode,
                map_index,
                trigger_index,
                trigger_file_offset: trigger_header.trigger.offset_absolute(),
                link_ref: trigger_header.link_ref,
                type_index: trigger.type_index,
                trig_type: trigger_type.trig_type,
                trig_type_name,
                trig_subtype: trigger_type.trig_subtype,
                trig_subtype_name,
                debug: trigger.debug,
                game_flags: trigger.game_flags,
                trig_flags: trigger.trig_flags,
                position: trigger.position,
                rotation: trigger.rotation,
                scale: trigger.scale,
                data: trigger.data,
                outgoing_links: trigger.links,
                invalid_outgoing_links,
                incoming_links: incoming_links[trigger_index].clone(),
                path_hash_matches,
                runtime_preview,
                visual,
                trigger_script,
                collision,
            });
            summary.triggers += 1;
        }
    }

    Ok(())
}

fn build_file_catalog(
    manifest_entries: &[ManifestEntry],
) -> (HashMap<u32, FileCatalogEntry>, Vec<TriggerFileError>) {
    let mut catalog = HashMap::new();
    let mut errors = Vec::new();

    for entry in manifest_entries {
        let result = (|| -> Result<(u32, FileCatalogEntry)> {
            let platform = Platform::from_path(&entry.source_path)
                .with_context(|| format!("detect platform for {}", entry.source_path.display()))?;
            let file = File::open(&entry.source_path)?;
            let reader = BufReader::new(file);
            let edb = EdbFile::new(Box::new(reader), platform)?;
            Ok((
                edb.header.hashcode,
                FileCatalogEntry {
                    entities: edb
                        .header
                        .entity_list
                        .iter()
                        .map(|entity| entity.common.hashcode)
                        .collect(),
                },
            ))
        })();

        match result {
            Ok((uid, catalog_entry)) => {
                catalog.insert(uid, catalog_entry);
            }
            Err(error) => errors.push(TriggerFileError {
                declared_uid: entry.declared_uid,
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
        let source_text = source_text.trim();
        if source_text.is_empty() || source_text.eq_ignore_ascii_case("source_path") {
            continue;
        }
        let source_path = PathBuf::from(source_text);
        let source_path = if source_path.is_absolute() {
            source_path
        } else {
            base.join(source_path)
        };
        entries.push(ManifestEntry {
            declared_uid: parse_u32(uid_text.trim()),
            source_path,
        });
    }

    Ok(entries)
}

fn parse_u32(value: &str) -> Option<u32> {
    let value = value.trim();
    let hex = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"));
    if let Some(hex) = hex {
        u32::from_str_radix(hex, 16).ok()
    } else if value.len() == 8 && value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        u32::from_str_radix(value, 16).ok()
    } else {
        value.parse().ok()
    }
}

fn load_trigger_types<P: AsRef<Path>>(path: P) -> Result<TriggerInformation> {
    let file = File::open(path.as_ref())?;
    let reader = BufReader::new(file);
    Ok(serde_yaml::from_reader(reader)?)
}

fn trigger_name(trigger_types: &TriggerInformation, trigger_type: u32) -> String {
    trigger_types
        .triggers
        .get(&trigger_type)
        .map(|entry: &TriggerDefinition| entry.name.clone())
        .unwrap_or_else(|| format!("Trig_{trigger_type}"))
}

fn build_incoming_links(map: &EXGeoMap) -> Vec<Vec<usize>> {
    let mut incoming = vec![Vec::new(); map.trigger_header.triggers.len()];
    for (source_index, source) in map.trigger_header.triggers.iter().enumerate() {
        for target in source
            .trigger
            .links
            .iter()
            .copied()
            .filter(|target| *target != -1)
        {
            if let Some(target_index) = valid_link_index(target, incoming.len()) {
                incoming[target_index].push(source_index);
            }
        }
    }
    incoming
}

fn invalid_links(links: &[i32; 8], trigger_count: usize) -> Vec<i32> {
    links
        .iter()
        .copied()
        .filter(|link| *link != -1 && valid_link_index(*link, trigger_count).is_none())
        .collect()
}

fn valid_link_index(link: i32, trigger_count: usize) -> Option<usize> {
    let index = usize::try_from(link).ok()?;
    (index < trigger_count).then_some(index)
}

fn classify_runtime_preview(
    trigger_type: u32,
    data: &[Option<u32>; 16],
    path_matches: &[TriggerPathMatch],
) -> TriggerRuntimePreview {
    let has_path_slot = |slot: usize| path_matches.iter().any(|path| path.data_slot == slot);
    let has_platform_rotation = [1usize, 3, 4].into_iter().any(|slot| {
        data[slot]
            .map(f32::from_bits)
            .is_some_and(|value| value.is_finite() && value.abs() > f32::EPSILON)
    });

    let (status, mode, limitations) = match trigger_type {
        1 if has_path_slot(1) => (
            "native_context_diagnostic",
            "camera_path_context",
            "data[1] is instruction-proven as a runtime path UID comparison/setup input; native activation, player state and camera interpolation are not simulated",
        ),
        20 if has_path_slot(4) => (
            "native_context_diagnostic",
            "camera_marker_path_context",
            "data[4] is instruction-proven as a native camera setup input; exact camera interpolation along the EXGeoPath is not simulated",
        ),
        48 => (
            "native_context_diagnostic",
            "npc_mission_cutscene_context",
            "data[0] and data[1] have native getters, data[2] is the NPC flag word, data[3] is the HT_TextGroup value, and data[4..7] are alternate cutscene UIDs selected by XTrigger_NPC::ActivateCutscene; Mission/Tutorial lookup and NPC dialogue AI are not simulated",
        ),
        60 if has_path_slot(1) => (
            "native_context_diagnostic",
            "watchbot_path_context",
            "mode 3 assigns data[1] to the runtime Watchbot path controller; player state, controller traversal and path timing are not simulated",
        ),
        72 if has_path_slot(0) => (
            "native_context_diagnostic",
            "boss_ratchet_path_context",
            "data[0] is passed into the created Ratchet boss runtime; boss AI traversal and timing are not simulated",
        ),
        73 if has_path_slot(1) => (
            "native_context_diagnostic",
            "monster_transporter_path_context",
            "data[1] is parsed into the Transporter route and data[4] reaches monster-controller setup; actor traversal and spawn timing are not simulated",
        ),
        10 | 11 | 18 | 33 | 74 if has_path_slot(2) => (
            "native_context_diagnostic",
            "monster_path_context",
            "the base Monster vtable +0xF4 returns data[0], which indexes a 24-byte Monster configuration record copied into handler +0x628..+0x63D; +0xF8 returns signed data[1] × 0.1 for the proximity test, +0xFC returns data[2], and the actor builder validates the 0x0B path family and creates a runtime path component; actor AI traversal and timing are not simulated",
        ),
        10 | 11 | 18 | 33 | 74 => (
            "native_context_diagnostic",
            "monster_native_getter_context",
            "the base Monster vtable exposes data[0] as a 24-byte Monster configuration-record index, signed data[1] × 0.1, data[2], data[4], data[7] flag tests 0x8000/0x4000 and data[15]; no valid path is present and actor AI is not simulated",
        ),
        3 => (
            "native_context_diagnostic",
            "monster_test_native_context",
            "XTrigger_Monster_Test exposes data[0], raw data[1], data[4], data[7] flag tests 0x8000/0x4000 and data[15]; test-monster AI is not simulated",
        ),
        70 => (
            "native_context_diagnostic",
            "monster_fish_native_context",
            "XTrigger_Monster_Fish exposes data[0] and signed data[1] × 0.1 while its path getter returns the 0x0B000000 sentinel; fish AI is not simulated",
        ),
        75 if has_path_slot(0) => (
            "serialized_only",
            "boss_sewer_path_like_value_rejected",
            "data[0] is instruction-proven as an integer selector compared with 1 at 0x00484C30/0x00484CF0; the matching EXGeoPath hash is coincidental and is not a native path reference",
        ),
        8 if has_path_slot(2) && has_platform_rotation => (
            "motion_preview_supported",
            "platform_event_gated_path_and_angular",
            "manual native 0x100/0x200 gating and active retrigger reversal are implemented; trigger-graph dispatch, node events/path switching and physics response are not executed",
        ),
        8 if has_path_slot(2) => (
            "motion_preview_supported",
            "platform_event_gated_path",
            "manual native 0x100/0x200 gating and active retrigger reversal are implemented; trigger-graph dispatch, node events/path switching and physics response are not executed",
        ),
        8 if has_platform_rotation => (
            "motion_preview_supported",
            "platform_event_gated_angular",
            "manual native 0x100/0x200 gating is implemented; trigger-graph dispatch and physics response are not executed",
        ),
        8 => (
            "serialized_only",
            "platform_native_mode_unimplemented",
            "no proven path or angular mode; remaining native Platform state is not simulated",
        ),
        37 if has_path_slot(1) => (
            "motion_preview_supported",
            "lift_event_gated_path",
            "map speed and manual native 0x100/0x200 gating are recovered; trigger-graph dispatch, node events/path switching and collision response are not executed",
        ),
        37 => (
            "missing_expected_reference",
            "lift_path_missing",
            "Lift runtime expects data[1] to resolve an EXGeoPath",
        ),
        80 if has_path_slot(1) => (
            "motion_preview_supported",
            "vehicle_event_gated_path_and_tangent_yaw",
            "map speed, tangent yaw, passive wheel roll and manual native 0x100/0x200 gating are recovered; traffic trigger graph, steering-wheel animation, physics and collision response are not executed",
        ),
        80 => (
            "missing_expected_reference",
            "vehicle_path_missing",
            "Vehicle runtime expects data[1] to resolve an EXGeoPath",
        ),
        _ if !path_matches.is_empty() => (
            "serialized_only",
            "native_path_consumer_unimplemented",
            "path reference is preserved, but this XTrigger class-specific path handler is not simulated",
        ),
        _ => (
            "serialized_only",
            "native_handler_unimplemented",
            "serialized fields, links, visual and collision references are inspectable; native class behavior is not simulated",
        ),
    };

    TriggerRuntimePreview {
        status: status.to_string(),
        mode: mode.to_string(),
        limitations: limitations.to_string(),
    }
}

fn resolve_trigger_script(map: &EXGeoMap, index: Option<u32>) -> TriggerScriptBinding {
    let Some(index) = index else {
        return TriggerScriptBinding {
            index: None,
            file_offset: None,
            aux: None,
            status: "none".to_string(),
        };
    };
    if let Some((script, aux)) = map.trigger_header.trigger_scripts.get(index as usize) {
        TriggerScriptBinding {
            index: Some(index),
            file_offset: Some(script.offset_absolute()),
            aux: Some(*aux),
            status: "resolved".to_string(),
        }
    } else {
        TriggerScriptBinding {
            index: Some(index),
            file_offset: None,
            aux: None,
            status: "invalid_index".to_string(),
        }
    }
}

fn resolve_collision(map: &EXGeoMap, index: Option<u32>) -> TriggerCollisionBinding {
    let Some(index) = index else {
        return TriggerCollisionBinding {
            index: None,
            status: "none".to_string(),
            datum: None,
        };
    };
    if let Some(datum) = map.trigger_header.trigger_collisions.0.get(index as usize) {
        TriggerCollisionBinding {
            index: Some(index),
            status: "resolved".to_string(),
            datum: Some(datum.clone()),
        }
    } else {
        TriggerCollisionBinding {
            index: Some(index),
            status: "invalid_index".to_string(),
            datum: None,
        }
    }
}

fn resolve_visual(
    current_file: u32,
    object: Option<u32>,
    file: Option<u32>,
    catalog: &HashMap<u32, FileCatalogEntry>,
) -> TriggerVisualBinding {
    let Some(object) = object else {
        return TriggerVisualBinding {
            object: None,
            file,
            resolved_entity: None,
            status: "none".to_string(),
        };
    };
    let source_file = file.unwrap_or(current_file);
    let Some(source) = catalog.get(&source_file) else {
        return TriggerVisualBinding {
            object: Some(object),
            file: Some(source_file),
            resolved_entity: None,
            status: "missing_source_file".to_string(),
        };
    };

    if object.is_local() {
        let index = object.index() as usize;
        if let Some(resolved) = source.entities.get(index) {
            TriggerVisualBinding {
                object: Some(object),
                file: Some(source_file),
                resolved_entity: Some(*resolved),
                status: "resolved_local".to_string(),
            }
        } else {
            TriggerVisualBinding {
                object: Some(object),
                file: Some(source_file),
                resolved_entity: None,
                status: "local_index_out_of_range".to_string(),
            }
        }
    } else if source.entities.contains(&object) {
        TriggerVisualBinding {
            object: Some(object),
            file: Some(source_file),
            resolved_entity: Some(object),
            status: "resolved_global".to_string(),
        }
    } else {
        TriggerVisualBinding {
            object: Some(object),
            file: Some(source_file),
            resolved_entity: None,
            status: "entity_missing".to_string(),
        }
    }
}

fn increment(map: &mut BTreeMap<String, usize>, key: String) {
    *map.entry(key).or_default() += 1;
}

fn write_rows_tsv(path: &Path, rows: &[TriggerReportRow]) -> Result<()> {
    let mut file = File::create(path)?;
    writeln!(
        file,
        "edb_uid\tedb_path\tmap_uid\tmap_index\ttrigger_index\ttrigger_file_offset\tlink_ref\ttype_index\ttrig_type\ttrig_type_name\ttrig_subtype\ttrig_subtype_name\tdebug\tgame_flags\ttrig_flags\tposition\trotation\tscale\tdata\toutgoing_links\tinvalid_outgoing_links\tincoming_links\tpath_hash_matches\tvisual_object\tvisual_file\tvisual_resolved_entity\tvisual_status\tgamescript_index\tgamescript_offset\tgamescript_aux\tgamescript_status\tcollision_index\tcollision_status\tcollision_type\tcollision_hashref\tcollision_extents\tcollision_position\tcollision_quaternion"
    )?;
    for row in rows {
        let collision = row.collision.datum.as_ref();
        writeln!(
            file,
            "0x{edb_uid:08X}\t{edb_path}\t0x{map_uid:08X}\t{map_index}\t{trigger_index}\t0x{trigger_file_offset:08X}\t{link_ref}\t{type_index}\t{trig_type}\t{trig_type_name}\t{trig_subtype}\t{trig_subtype_name}\t{debug}\t0x{game_flags:08X}\t0x{trig_flags:08X}\t{position}\t{rotation}\t{scale}\t{data}\t{outgoing}\t{invalid}\t{incoming}\t{path_matches}\t{visual_object}\t{visual_file}\t{visual_resolved}\t{visual_status}\t{script_index}\t{script_offset}\t{script_aux}\t{script_status}\t{collision_index}\t{collision_status}\t{collision_type}\t{collision_hashref}\t{collision_extents}\t{collision_position}\t{collision_quaternion}",
            edb_uid = row.edb_uid,
            edb_path = escape_tsv(&row.edb_path),
            map_uid = row.map_uid,
            map_index = row.map_index,
            trigger_index = row.trigger_index,
            trigger_file_offset = row.trigger_file_offset,
            link_ref = row.link_ref,
            type_index = row.type_index,
            trig_type = row.trig_type,
            trig_type_name = escape_tsv(&row.trig_type_name),
            trig_subtype = row.trig_subtype,
            trig_subtype_name = escape_tsv(row.trig_subtype_name.as_deref().unwrap_or("")),
            debug = row.debug,
            game_flags = row.game_flags,
            trig_flags = row.trig_flags,
            position = json_cell(&row.position)?,
            rotation = json_cell(&row.rotation)?,
            scale = json_cell(&row.scale)?,
            data = json_cell(&row.data)?,
            outgoing = json_cell(&row.outgoing_links)?,
            invalid = json_cell(&row.invalid_outgoing_links)?,
            incoming = json_cell(&row.incoming_links)?,
            path_matches = json_cell(&row.path_hash_matches)?,
            visual_object = optional_hex(row.visual.object),
            visual_file = optional_hex(row.visual.file),
            visual_resolved = optional_hex(row.visual.resolved_entity),
            visual_status = row.visual.status,
            script_index = optional_decimal(row.trigger_script.index),
            script_offset = row
                .trigger_script
                .file_offset
                .map(|value| format!("0x{value:08X}"))
                .unwrap_or_default(),
            script_aux = optional_hex(row.trigger_script.aux),
            script_status = row.trigger_script.status,
            collision_index = optional_decimal(row.collision.index),
            collision_status = row.collision.status,
            collision_type = collision.map(|datum| datum.dtype.to_string()).unwrap_or_default(),
            collision_hashref = collision
                .map(|datum| format!("0x{:08X}", datum.hashref))
                .unwrap_or_default(),
            collision_extents = collision
                .map(|datum| json_cell(&datum.extents))
                .transpose()?
                .unwrap_or_default(),
            collision_position = collision
                .map(|datum| json_cell(&datum.position))
                .transpose()?
                .unwrap_or_default(),
            collision_quaternion = collision
                .map(|datum| json_cell(&datum.q))
                .transpose()?
                .unwrap_or_default(),
        )?;
    }
    Ok(())
}

fn write_path_rows_tsv(path: &Path, rows: &[TriggerReportRow]) -> Result<()> {
    let mut file = File::create(path)?;
    writeln!(
        file,
        "edb_uid	edb_path	map_uid	trigger_index	trig_type	trig_type_name	trigger_position	data_slot	path_index	path_hashcode	path_position	path_flags	path_type	node_positions	node_sizes	node_values	node_flags	node_distances	node_link_counts	links"
    )?;
    for row in rows {
        for path_match in &row.path_hash_matches {
            writeln!(
                file,
                "0x{edb_uid:08X}	{edb_path}	0x{map_uid:08X}	{trigger_index}	{trig_type}	{trig_type_name}	{trigger_position}	{data_slot}	{path_index}	0x{path_hashcode:08X}	{path_position}	0x{path_flags:08X}	{path_type}	{node_positions}	{node_sizes}	{node_values}	{node_flags}	{node_distances}	{node_link_counts}	{links}",
                edb_uid = row.edb_uid,
                edb_path = escape_tsv(&row.edb_path),
                map_uid = row.map_uid,
                trigger_index = row.trigger_index,
                trig_type = row.trig_type,
                trig_type_name = escape_tsv(&row.trig_type_name),
                trigger_position = json_cell(&row.position)?,
                data_slot = path_match.data_slot,
                path_index = path_match.path_index,
                path_hashcode = path_match.path_hashcode,
                path_position = json_cell(&path_match.path_position)?,
                path_flags = path_match.path_flags,
                path_type = path_match.path_type,
                node_positions = json_cell(&path_match.node_positions)?,
                node_sizes = json_cell(&path_match.node_sizes)?,
                node_values = json_cell(&path_match.node_values)?,
                node_flags = json_cell(&path_match.node_flags)?,
                node_distances = json_cell(&path_match.node_distances)?,
                node_link_counts = json_cell(&path_match.node_link_counts)?,
                links = json_cell(&path_match.links)?,
            )?;
        }
    }
    Ok(())
}

fn write_path_node_rows_tsv(path: &Path, rows: &[TriggerReportRow]) -> Result<()> {
    let mut file = File::create(path)?;
    writeln!(
        file,
        "edb_uid	edb_path	map_uid	trigger_index	trig_type	trig_type_name	path_hashcode	node_index	position	size	value	flags	distance	link_count	proven_semantic"
    )?;
    for row in rows {
        for path_match in &row.path_hash_matches {
            for node_index in 0..path_match.node_positions.len() {
                let value = path_match
                    .node_values
                    .get(node_index)
                    .copied()
                    .unwrap_or([0; 4]);
                let flags = path_match
                    .node_flags
                    .get(node_index)
                    .copied()
                    .unwrap_or_default();
                let distance = path_match
                    .node_distances
                    .get(node_index)
                    .copied()
                    .unwrap_or_default();
                if value == [0; 4] && flags == 0 && distance.abs() <= f32::EPSILON {
                    continue;
                }
                let semantic = match value[0] {
                    4 => "event_0x200_dispatch",
                    8 => "linked_trigger_mask_dispatch",
                    9 => "alternate_path_uid_0x0B000000_plus_value1",
                    0 if flags & 0x8 != 0 => "flags_bit_0x8_path_switch",
                    0 => "metadata_without_value_opcode",
                    _ => "class_specific_metadata_not_common_path_dispatch",
                };
                writeln!(
                    file,
                    "0x{edb_uid:08X}	{edb_path}	0x{map_uid:08X}	{trigger_index}	{trig_type}	{trig_type_name}	0x{path_hashcode:08X}	{node_index}	{position}	{size}	{value}	0x{flags:08X}	{distance}	{link_count}	{semantic}",
                    edb_uid = row.edb_uid,
                    edb_path = escape_tsv(&row.edb_path),
                    map_uid = row.map_uid,
                    trigger_index = row.trigger_index,
                    trig_type = row.trig_type,
                    trig_type_name = escape_tsv(&row.trig_type_name),
                    path_hashcode = path_match.path_hashcode,
                    node_index = node_index,
                    position = json_cell(&path_match.node_positions[node_index])?,
                    size = json_cell(path_match.node_sizes.get(node_index).unwrap_or(&[0.0; 2]))?,
                    value = json_cell(&value)?,
                    flags = flags,
                    distance = distance,
                    link_count = path_match
                        .node_link_counts
                        .get(node_index)
                        .copied()
                        .unwrap_or_default(),
                    semantic = semantic,
                )?;
            }
        }
    }
    Ok(())
}

fn write_runtime_coverage_tsv(path: &Path, rows: &[TriggerReportRow]) -> Result<()> {
    let mut grouped = BTreeMap::<(u32, String, String, String, String), usize>::new();
    for row in rows {
        *grouped
            .entry((
                row.trig_type,
                row.trig_type_name.clone(),
                row.runtime_preview.status.clone(),
                row.runtime_preview.mode.clone(),
                row.runtime_preview.limitations.clone(),
            ))
            .or_default() += 1;
    }

    let mut file = File::create(path)?;
    writeln!(
        file,
        "trig_type\ttrig_type_name\tcount\truntime_preview_status\truntime_preview_mode\tlimitations"
    )?;
    for ((trig_type, trig_type_name, status, mode, limitations), count) in grouped {
        writeln!(
            file,
            "{trig_type}\t{}\t{count}\t{}\t{}\t{}",
            escape_tsv(&trig_type_name),
            escape_tsv(&status),
            escape_tsv(&mode),
            escape_tsv(&limitations),
        )?;
    }
    Ok(())
}

fn write_type_summary_tsv(path: &Path, summary: &TriggerCorpusSummary) -> Result<()> {
    let mut file = File::create(path)?;
    writeln!(file, "category\tkey\tcount")?;
    for (key, count) in &summary.trigger_type_counts {
        writeln!(file, "trigger_type\t{}\t{}", escape_tsv(key), count)?;
    }
    for (key, count) in &summary.trigger_subtype_counts {
        writeln!(file, "trigger_subtype\t{}\t{}", escape_tsv(key), count)?;
    }
    for (key, count) in &summary.collision_type_counts {
        writeln!(file, "collision_type\t{}\t{}", escape_tsv(key), count)?;
    }
    for (key, count) in &summary.visual_status_counts {
        writeln!(file, "visual_status\t{}\t{}", escape_tsv(key), count)?;
    }
    for (key, count) in &summary.trigger_script_status_counts {
        writeln!(
            file,
            "trigger_script_status\t{}\t{}",
            escape_tsv(key),
            count
        )?;
    }
    for (key, count) in &summary.trigger_path_match_counts {
        writeln!(file, "trigger_path_match\t{}\t{}", escape_tsv(key), count)?;
    }
    for (key, count) in &summary.runtime_preview_status_counts {
        writeln!(
            file,
            "runtime_preview_status\t{}\t{}",
            escape_tsv(key),
            count
        )?;
    }
    for (key, count) in &summary.runtime_preview_mode_counts {
        writeln!(file, "runtime_preview_mode\t{}\t{}", escape_tsv(key), count)?;
    }
    Ok(())
}

fn json_cell<T: Serialize>(value: &T) -> Result<String> {
    Ok(escape_tsv(&serde_json::to_string(value)?))
}

fn escape_tsv(value: &str) -> String {
    value.replace('\t', " ").replace(['\r', '\n'], " ")
}

fn optional_hex(value: Option<u32>) -> String {
    value
        .map(|value| format!("0x{value:08X}"))
        .unwrap_or_default()
}

fn optional_decimal(value: Option<u32>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{
        classify_runtime_preview, invalid_links, parse_u32, valid_link_index, TriggerPathMatch,
    };

    #[test]
    fn parses_manifest_hashes_without_guessing_decimal_eight_digit_ids() {
        assert_eq!(parse_u32("0x01000071"), Some(0x0100_0071));
        assert_eq!(parse_u32("01000071"), Some(0x0100_0071));
        assert_eq!(parse_u32("113"), Some(113));
    }

    #[test]
    fn camera_path_consumers_are_diagnostic_not_motion_previews() {
        let path_match = |data_slot| TriggerPathMatch {
            data_slot,
            path_index: 0,
            path_hashcode: 0x0B00_002E,
            path_position: [0.0; 3],
            path_flags: 0,
            path_type: 0,
            node_positions: vec![],
            node_sizes: vec![],
            node_values: vec![],
            node_flags: vec![],
            node_distances: vec![],
            node_link_counts: vec![],
            links: vec![],
        };
        let camera = classify_runtime_preview(1, &[None; 16], &[path_match(1)]);
        assert_eq!(camera.status, "native_context_diagnostic");
        assert_eq!(camera.mode, "camera_path_context");

        let marker = classify_runtime_preview(20, &[None; 16], &[path_match(4)]);
        assert_eq!(marker.status, "native_context_diagnostic");
        assert_eq!(marker.mode, "camera_marker_path_context");

        let npc = classify_runtime_preview(48, &[None; 16], &[]);
        assert_eq!(npc.status, "native_context_diagnostic");
        assert_eq!(npc.mode, "npc_mission_cutscene_context");

        let watchbot = classify_runtime_preview(60, &[None; 16], &[path_match(1)]);
        assert_eq!(watchbot.status, "native_context_diagnostic");
        assert_eq!(watchbot.mode, "watchbot_path_context");

        let ratchet = classify_runtime_preview(72, &[None; 16], &[path_match(0)]);
        assert_eq!(ratchet.status, "native_context_diagnostic");
        assert_eq!(ratchet.mode, "boss_ratchet_path_context");

        let transporter = classify_runtime_preview(73, &[None; 16], &[path_match(1)]);
        assert_eq!(transporter.status, "native_context_diagnostic");
        assert_eq!(transporter.mode, "monster_transporter_path_context");

        let monster = classify_runtime_preview(74, &[None; 16], &[path_match(2)]);
        assert_eq!(monster.status, "native_context_diagnostic");
        assert_eq!(monster.mode, "monster_path_context");

        let monster_no_path = classify_runtime_preview(10, &[None; 16], &[]);
        assert_eq!(monster_no_path.status, "native_context_diagnostic");
        assert_eq!(monster_no_path.mode, "monster_native_getter_context");

        let monster_test = classify_runtime_preview(3, &[None; 16], &[]);
        assert_eq!(monster_test.status, "native_context_diagnostic");
        assert_eq!(monster_test.mode, "monster_test_native_context");

        let monster_fish = classify_runtime_preview(70, &[None; 16], &[]);
        assert_eq!(monster_fish.status, "native_context_diagnostic");
        assert_eq!(monster_fish.mode, "monster_fish_native_context");

        let boss_sewer = classify_runtime_preview(75, &[None; 16], &[path_match(0)]);
        assert_eq!(boss_sewer.status, "serialized_only");
        assert_eq!(boss_sewer.mode, "boss_sewer_path_like_value_rejected");

        let platform = classify_runtime_preview(8, &[None; 16], &[path_match(2)]);
        assert_eq!(platform.status, "motion_preview_supported");
        assert_eq!(platform.mode, "platform_event_gated_path");

        let lift = classify_runtime_preview(37, &[None; 16], &[path_match(1)]);
        assert_eq!(lift.status, "motion_preview_supported");
        assert_eq!(lift.mode, "lift_event_gated_path");

        let vehicle = classify_runtime_preview(80, &[None; 16], &[path_match(1)]);
        assert_eq!(vehicle.status, "motion_preview_supported");
        assert_eq!(vehicle.mode, "vehicle_event_gated_path_and_tangent_yaw");
    }

    #[test]
    fn trigger_report_rejects_negative_and_out_of_range_links() {
        assert_eq!(valid_link_index(-2, 4), None);
        assert_eq!(valid_link_index(3, 4), Some(3));
        assert_eq!(valid_link_index(4, 4), None);
        assert_eq!(
            invalid_links(&[-1, -2, 0, 3, 4, -1, -1, -1], 4),
            vec![-2, 4]
        );
    }
}
