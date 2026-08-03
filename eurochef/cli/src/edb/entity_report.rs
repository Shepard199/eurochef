use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufReader, Seek},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use eurochef_edb::{
    binrw::BinReaderExt, edb::EdbFile, entity::EXGeoEntity, robots_hashdb, versions::Platform,
    HashcodeUtils,
};
use serde::Serialize;

#[derive(Debug, Clone)]
struct ManifestEntry {
    declared_uid: Option<u32>,
    source_path: PathBuf,
}

#[derive(Debug, Default, Serialize)]
struct EntityCorpusSummary {
    manifest_entries: usize,
    files_scanned: usize,
    files_failed: usize,
    entities: usize,
    parse_failures: usize,
    local_entities: usize,
    global_entities: usize,
    unresolved_global_entities: usize,
    local_index_mismatches: usize,
    type_counts: BTreeMap<String, usize>,
    name_counts: BTreeMap<String, usize>,
    runtime_coverage_counts: BTreeMap<String, usize>,
}

#[derive(Debug, Serialize)]
struct EntityFileError {
    declared_uid: Option<u32>,
    source_path: String,
    error: String,
}

#[derive(Debug, Serialize)]
struct EntityReportRow {
    edb_uid: u32,
    edb_path: String,
    entity_index: usize,
    entity_hashcode: u32,
    entity_name: String,
    hash_scope: String,
    local_index: Option<u32>,
    local_index_matches: Option<bool>,
    runtime_coverage: String,
    file_offset: u32,
    object_type: Option<u32>,
    object_kind: String,
    parse_status: String,
    flags: Option<u32>,
    sort_value: Option<u16>,
    render_order: Option<u8>,
    bounds_min: Option<[f32; 3]>,
    bounds_max: Option<[f32; 3]>,
    vertices: Option<usize>,
    indices: Option<usize>,
    strips: Option<usize>,
    triangles: Option<usize>,
    child_entities: Option<usize>,
    face_collision_offset: Option<u64>,
    face_info_offset: Option<u64>,
    index_data_offset: Option<u64>,
    face_collision_next_known_offset: Option<u64>,
    face_info_next_known_offset: Option<u64>,
    face_collision_to_next_known_span: Option<u64>,
    face_info_to_next_known_span: Option<u64>,
}

#[derive(Debug, Serialize)]
struct EntityCorpusReport {
    manifest_path: String,
    summary: EntityCorpusSummary,
    file_errors: Vec<EntityFileError>,
    rows: Vec<EntityReportRow>,
}

pub fn execute_command(manifest_path: String, output_folder: Option<String>) -> Result<()> {
    let manifest_path = PathBuf::from(manifest_path);
    let output_folder = output_folder
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./ht_entity_corpus_report"));
    std::fs::create_dir_all(&output_folder)?;

    let entries = read_manifest(&manifest_path)?;
    let mut report = EntityCorpusReport {
        manifest_path: manifest_path.to_string_lossy().into_owned(),
        summary: EntityCorpusSummary {
            manifest_entries: entries.len(),
            ..Default::default()
        },
        file_errors: Vec::new(),
        rows: Vec::new(),
    };

    for entry in &entries {
        let Some(platform) = Platform::from_path(&entry.source_path) else {
            report.file_errors.push(EntityFileError {
                declared_uid: entry.declared_uid,
                source_path: entry.source_path.to_string_lossy().into_owned(),
                error: "platform detection failed".to_string(),
            });
            report.summary.files_failed += 1;
            continue;
        };
        match scan_file(entry, platform, &mut report.summary, &mut report.rows) {
            Ok(()) => report.summary.files_scanned += 1,
            Err(error) => {
                report.file_errors.push(EntityFileError {
                    declared_uid: entry.declared_uid,
                    source_path: entry.source_path.to_string_lossy().into_owned(),
                    error: format!("{error:#}"),
                });
                report.summary.files_failed += 1;
            }
        }
    }

    std::fs::write(
        output_folder.join("ht_entity_corpus_report.json"),
        serde_json::to_string_pretty(&report)?,
    )?;
    write_rows_tsv(&output_folder.join("ht_entity_rows.tsv"), &report.rows)?;
    write_summary_tsv(
        &output_folder.join("ht_entity_type_summary.tsv"),
        "object_kind",
        &report.summary.type_counts,
    )?;
    write_summary_tsv(
        &output_folder.join("ht_entity_name_summary.tsv"),
        "entity_name",
        &report.summary.name_counts,
    )?;

    info!(
        "Wrote {} HT_Entity rows from {} manifest entries to {}",
        report.rows.len(),
        report.summary.manifest_entries,
        output_folder.display()
    );
    Ok(())
}

fn scan_file(
    entry: &ManifestEntry,
    platform: Platform,
    summary: &mut EntityCorpusSummary,
    rows: &mut Vec<EntityReportRow>,
) -> Result<()> {
    let file = File::open(&entry.source_path)
        .with_context(|| format!("open {}", entry.source_path.display()))?;
    let reader = BufReader::new(file);
    let mut edb = EdbFile::new(Box::new(reader), platform)
        .with_context(|| format!("parse header {}", entry.source_path.display()))?;
    let header = edb.header.clone();

    for (entity_index, entity_header) in header.entity_list.iter().enumerate() {
        let hashcode = entity_header.common.hashcode;
        let (entity_name, hash_scope, local_index, local_index_matches) =
            classify_entity_hash(hashcode, entity_index);
        let runtime_coverage = entity_runtime_coverage(hashcode, &hash_scope);
        let file_offset = entity_header.common.address;
        edb.seek(std::io::SeekFrom::Start(file_offset as u64))?;

        let row = match edb.read_type_args::<EXGeoEntity>(edb.endian, (header.version, platform)) {
            Ok(entity) => row_from_entity(
                header.hashcode,
                &entry.source_path,
                entity_index,
                hashcode,
                entity_name.clone(),
                hash_scope.clone(),
                local_index,
                local_index_matches,
                runtime_coverage.clone(),
                file_offset,
                entity,
            ),
            Err(error) => {
                summary.parse_failures += 1;
                EntityReportRow {
                    edb_uid: header.hashcode,
                    edb_path: entry.source_path.to_string_lossy().into_owned(),
                    entity_index,
                    entity_hashcode: hashcode,
                    entity_name: entity_name.clone(),
                    hash_scope: hash_scope.clone(),
                    local_index,
                    local_index_matches,
                    runtime_coverage: runtime_coverage.clone(),
                    file_offset,
                    object_type: None,
                    object_kind: "parse_failure".to_string(),
                    parse_status: format!("error: {error}"),
                    flags: None,
                    sort_value: None,
                    render_order: None,
                    bounds_min: None,
                    bounds_max: None,
                    vertices: None,
                    indices: None,
                    strips: None,
                    triangles: None,
                    child_entities: None,
                    face_collision_offset: None,
                    face_info_offset: None,
                    index_data_offset: None,
                    face_collision_next_known_offset: None,
                    face_info_next_known_offset: None,
                    face_collision_to_next_known_span: None,
                    face_info_to_next_known_span: None,
                }
            }
        };

        increment(&mut summary.type_counts, row.object_kind.clone());
        increment(&mut summary.name_counts, entity_name);
        increment(
            &mut summary.runtime_coverage_counts,
            row.runtime_coverage.clone(),
        );
        match row.hash_scope.as_str() {
            "local" => summary.local_entities += 1,
            "global" => summary.global_entities += 1,
            "unresolved_global" => summary.unresolved_global_entities += 1,
            _ => {}
        }
        if row.local_index_matches == Some(false) {
            summary.local_index_mismatches += 1;
        }
        summary.entities += 1;
        rows.push(row);
    }
    Ok(())
}

fn row_from_entity(
    edb_uid: u32,
    path: &Path,
    entity_index: usize,
    hashcode: u32,
    entity_name: String,
    hash_scope: String,
    local_index: Option<u32>,
    local_index_matches: Option<bool>,
    runtime_coverage: String,
    file_offset: u32,
    entity: EXGeoEntity,
) -> EntityReportRow {
    let object_type = entity.type_code();
    let base = entity.base().cloned();
    let (object_kind, vertices, indices, strips, triangles, child_entities) = match &entity {
        EXGeoEntity::Mesh(mesh) => (
            "mesh",
            Some(mesh.vertices.len()),
            Some(mesh.indices.len()),
            Some(mesh.tristrips.len() + mesh.tristrips_gx.len() + mesh.tristrips_ps2.len()),
            Some(
                mesh.tristrips
                    .iter()
                    .map(|strip| strip.tricount as usize)
                    .sum(),
            ),
            None,
        ),
        EXGeoEntity::Split(split) => ("split", None, None, None, None, Some(split.entities.len())),
        EXGeoEntity::Instance(instance) => (
            "instance",
            Some(instance.robots_v248_vertices.len()),
            None,
            Some(instance.robots_v248_primitive_count as usize),
            Some(instance.robots_v248_primitive_count as usize),
            None,
        ),
        EXGeoEntity::NavMesh(navmesh) => (
            "navmesh",
            Some(navmesh.vertex_count as usize),
            Some(navmesh.face_count as usize * 3),
            Some(navmesh.face_count as usize),
            Some(navmesh.face_count as usize),
            None,
        ),
        EXGeoEntity::MapZone(_) => ("mapzone", None, None, None, None, None),
        EXGeoEntity::UnknownType(_) => ("unknown", None, None, None, None, None),
    };

    let (
        face_collision_offset,
        face_info_offset,
        index_data_offset,
        face_collision_next_known_offset,
        face_info_next_known_offset,
        face_collision_to_next_known_span,
        face_info_to_next_known_span,
    ) = match &entity {
        EXGeoEntity::Mesh(mesh) => {
            let face_collision_offset = mesh
                .data
                .face_collision
                .as_ref()
                .filter(|ptr| ptr.offset_relative() != 0)
                .map(|ptr| ptr.offset_absolute());
            let face_info_offset = mesh
                .data
                .face_info
                .as_ref()
                .filter(|ptr| ptr.offset_relative() != 0)
                .map(|ptr| ptr.offset_absolute());
            let index_data_offset = (mesh.data.index_data.offset_relative() != 0)
                .then(|| mesh.data.index_data.offset_absolute());
            let tristrip_data_offset = (mesh.data.tristrip_data_offset.offset_relative() != 0)
                .then(|| mesh.data.tristrip_data_offset.offset_absolute());
            let vertex_data_offset = (mesh.data.vertex_data_offset.offset_relative() != 0)
                .then(|| mesh.data.vertex_data_offset.offset_absolute());
            let vertex_color_offset = mesh
                .data
                .vertex_color_offset
                .as_ref()
                .filter(|ptr| ptr.offset_relative() != 0)
                .map(|ptr| ptr.offset_absolute());
            let known_offsets = [
                face_collision_offset,
                face_info_offset,
                index_data_offset,
                tristrip_data_offset,
                vertex_data_offset,
                vertex_color_offset,
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
            let next_known = |start: Option<u64>| {
                start.and_then(|start| {
                    known_offsets
                        .iter()
                        .copied()
                        .filter(|candidate| *candidate > start)
                        .min()
                })
            };
            let face_collision_next_known_offset = next_known(face_collision_offset);
            let face_info_next_known_offset = next_known(face_info_offset);
            let face_collision_to_next_known_span = face_collision_offset
                .zip(face_collision_next_known_offset)
                .and_then(|(start, end)| end.checked_sub(start));
            let face_info_to_next_known_span = face_info_offset
                .zip(face_info_next_known_offset)
                .and_then(|(start, end)| end.checked_sub(start));
            (
                face_collision_offset,
                face_info_offset,
                index_data_offset,
                face_collision_next_known_offset,
                face_info_next_known_offset,
                face_collision_to_next_known_span,
                face_info_to_next_known_span,
            )
        }
        _ => (None, None, None, None, None, None, None),
    };

    let (flags, sort_value, render_order, bounds_min, bounds_max) = base
        .map(|base| {
            (
                Some(base.flags),
                Some(base.sort_value),
                Some(base.render_order),
                Some([
                    base.bounds_box[0][0].min(base.bounds_box[1][0]),
                    base.bounds_box[0][1].min(base.bounds_box[1][1]),
                    base.bounds_box[0][2].min(base.bounds_box[1][2]),
                ]),
                Some([
                    base.bounds_box[0][0].max(base.bounds_box[1][0]),
                    base.bounds_box[0][1].max(base.bounds_box[1][1]),
                    base.bounds_box[0][2].max(base.bounds_box[1][2]),
                ]),
            )
        })
        .unwrap_or((None, None, None, None, None));

    EntityReportRow {
        edb_uid,
        edb_path: path.to_string_lossy().into_owned(),
        entity_index,
        entity_hashcode: hashcode,
        entity_name,
        hash_scope,
        local_index,
        local_index_matches,
        runtime_coverage,
        file_offset,
        object_type: Some(object_type),
        object_kind: object_kind.to_string(),
        parse_status: "parsed".to_string(),
        flags,
        sort_value,
        render_order,
        bounds_min,
        bounds_max,
        vertices,
        indices,
        strips,
        triangles,
        child_entities,
        face_collision_offset,
        face_info_offset,
        index_data_offset,
        face_collision_next_known_offset,
        face_info_next_known_offset,
        face_collision_to_next_known_span,
        face_info_to_next_known_span,
    }
}

fn entity_runtime_coverage(hashcode: u32, hash_scope: &str) -> String {
    match hashcode {
        0x0200_0012 => "native_fan_rotation_diagnostic",
        0x0200_017A => "native_vehicle_steering",
        0x0200_017B => "native_vehicle_passive_roll",
        0x0200_01AE => "native_vehicle_trigger_motion",
        _ if hash_scope == "local" => "structural_render_and_local_script_resolution",
        _ => "structural_render",
    }
    .to_string()
}

fn classify_entity_hash(
    hashcode: u32,
    entity_index: usize,
) -> (String, String, Option<u32>, Option<bool>) {
    if hashcode.is_local() {
        let local_index = hashcode.index();
        return (
            format!("LocalEntity[{local_index}]"),
            "local".to_string(),
            Some(local_index),
            Some(local_index as usize == entity_index),
        );
    }
    let resolved = robots_hashdb::resolve(hashcode);
    (
        resolved
            .map(str::to_owned)
            .unwrap_or_else(|| format!("HT_Invalid_{hashcode:08x}")),
        if resolved.is_some() {
            "global"
        } else {
            "unresolved_global"
        }
        .to_string(),
        None,
        None,
    )
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

fn increment(counts: &mut BTreeMap<String, usize>, key: String) {
    *counts.entry(key).or_default() += 1;
}

fn format_vec3(value: Option<[f32; 3]>) -> String {
    value
        .map(|value| format!("{:.6},{:.6},{:.6}", value[0], value[1], value[2]))
        .unwrap_or_default()
}

fn format_offset(value: Option<u64>) -> String {
    value
        .map(|value| format!("0x{value:08X}"))
        .unwrap_or_default()
}

fn write_rows_tsv(path: &Path, rows: &[EntityReportRow]) -> Result<()> {
    let mut output = String::from(
        "edb_uid	edb_path	entity_index	entity_hashcode	entity_name	hash_scope	local_index	local_index_matches	runtime_coverage	file_offset	object_type	object_kind	parse_status	flags	sort_value	render_order	bounds_min	bounds_max	vertices	indices	strips	triangles	child_entities	face_collision_offset	face_info_offset	index_data_offset	face_collision_next_known_offset	face_info_next_known_offset	face_collision_to_next_known_span	face_info_to_next_known_span
",
    );
    for row in rows {
        output.push_str(&format!(
            "0x{:08X}	{}	{}	0x{:08X}	{}	{}	{}	{}	{}	0x{:08X}	{}	{}	{}	{}	{}	{}	{}	{}	{}	{}	{}	{}	{}	{}	{}	{}	{}	{}	{}	{}
",
            row.edb_uid,
            row.edb_path,
            row.entity_index,
            row.entity_hashcode,
            row.entity_name,
            row.hash_scope,
            row.local_index
                .map(|value| value.to_string())
                .unwrap_or_default(),
            row.local_index_matches
                .map(|value| value.to_string())
                .unwrap_or_default(),
            row.runtime_coverage,
            row.file_offset,
            row.object_type
                .map(|value| format!("0x{value:03X}"))
                .unwrap_or_default(),
            row.object_kind,
            row.parse_status
                .replace([char::from(9), char::from(10)], " "),
            row.flags
                .map(|value| format!("0x{value:08X}"))
                .unwrap_or_default(),
            row.sort_value
                .map(|value| value.to_string())
                .unwrap_or_default(),
            row.render_order
                .map(|value| value.to_string())
                .unwrap_or_default(),
            format_vec3(row.bounds_min),
            format_vec3(row.bounds_max),
            row.vertices
                .map(|value| value.to_string())
                .unwrap_or_default(),
            row.indices
                .map(|value| value.to_string())
                .unwrap_or_default(),
            row.strips
                .map(|value| value.to_string())
                .unwrap_or_default(),
            row.triangles
                .map(|value| value.to_string())
                .unwrap_or_default(),
            row.child_entities
                .map(|value| value.to_string())
                .unwrap_or_default(),
            format_offset(row.face_collision_offset),
            format_offset(row.face_info_offset),
            format_offset(row.index_data_offset),
            format_offset(row.face_collision_next_known_offset),
            format_offset(row.face_info_next_known_offset),
            row.face_collision_to_next_known_span
                .map(|value| value.to_string())
                .unwrap_or_default(),
            row.face_info_to_next_known_span
                .map(|value| value.to_string())
                .unwrap_or_default(),
        ));
    }
    std::fs::write(path, output)?;
    Ok(())
}

fn write_summary_tsv(
    path: &Path,
    key_header: &str,
    counts: &BTreeMap<String, usize>,
) -> Result<()> {
    let mut rows = counts.iter().collect::<Vec<_>>();
    rows.sort_by(|(left_name, left_count), (right_name, right_count)| {
        right_count
            .cmp(left_count)
            .then_with(|| left_name.cmp(right_name))
    });
    let mut output = format!("{key_header}\tcount\n");
    for (name, count) in rows {
        output.push_str(&format!("{name}\t{count}\n"));
    }
    std::fs::write(path, output)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_manifest_uids_without_guessing_short_decimal_as_hex() {
        assert_eq!(parse_u32("00072"), Some(72));
        assert_eq!(parse_u32("01000037"), Some(0x0100_0037));
        assert_eq!(parse_u32("0x0200017A"), Some(0x0200_017A));
    }

    #[test]
    fn entity_names_use_the_embedded_robots_hash_database() {
        assert_eq!(
            robots_hashdb::resolve(0x0200_017A),
            Some("HT_Entity_Wheel_Drive")
        );
        assert_eq!(
            robots_hashdb::resolve(0x0200_0012),
            Some("HT_Entity_FanRotatingEntity")
        );
    }
}
