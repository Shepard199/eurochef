use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{BufReader, Seek},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use eurochef_edb::{
    anim::EXGeoBaseAnimSkin, binrw::BinReaderExt, edb::EdbFile, entity::EXGeoEntity, robots_hashdb,
    versions::Platform, HashcodeUtils,
};
use serde::Serialize;

#[derive(Debug, Clone)]
struct ManifestEntry {
    declared_uid: Option<u32>,
    source_path: PathBuf,
}

#[derive(Debug, Default, Serialize)]
struct Summary {
    manifest_entries: usize,
    files_scanned: usize,
    files_failed: usize,
    animations: usize,
    animskins: usize,
    animskin_parse_failures: usize,
    animation_skin_bindings: usize,
    animation_no_skin_sentinel: usize,
    animation_skin_unresolved: usize,
    component_entities: usize,
    component_entity_out_of_range: usize,
    weight_payloads: usize,
    weight_stream_bytes: usize,
    weight_stream_empty: usize,
    unique_weight_stream_checksums: usize,
    weight_count_histogram: BTreeMap<String, usize>,
    weight_length_histogram: BTreeMap<String, usize>,
    unique_animation_hashes: usize,
    unique_animskin_hashes: usize,
    unique_component_entity_hashes: usize,
    object_type_counts: BTreeMap<String, usize>,
}

#[derive(Debug, Serialize)]
struct FileError {
    declared_uid: Option<u32>,
    source_path: String,
    error: String,
}

#[derive(Debug, Serialize)]
struct AnimationRow {
    edb_uid: u32,
    edb_path: String,
    animation_index: usize,
    animation_hashcode: u32,
    animation_name: String,
    file_offset: u32,
    data_size: u32,
    motiondata_info_addr: u32,
    skin_num: u32,
    skin_binding_status: String,
    animskin_hashcode: Option<u32>,
    animskin_name: String,
}

#[derive(Debug, Serialize)]
struct AnimSkinRow {
    edb_uid: u32,
    edb_path: String,
    animskin_index: usize,
    animskin_hashcode: u32,
    animskin_name: String,
    file_offset: u32,
    base_skin_num: u32,
    mip_ref: u32,
    object_type: Option<u32>,
    bone_count: Option<u32>,
    primary_entities: Option<usize>,
    secondary_entities: Option<usize>,
    parse_status: String,
}

#[derive(Debug, Serialize)]
struct ComponentRow {
    edb_uid: u32,
    edb_path: String,
    animskin_index: usize,
    animskin_hashcode: u32,
    component_group: String,
    component_index: usize,
    raw_entity_index: u32,
    entity_index: u32,
    entity_binding_status: String,
    entity_hashcode: Option<u32>,
    entity_name: String,
    section_index: u32,
    parts_count: u32,
    morph_index: i32,
}

#[derive(Debug, Serialize)]
struct WeightPayloadRow {
    edb_uid: u32,
    edb_path: String,
    animskin_index: usize,
    animskin_hashcode: u32,
    component_group: String,
    component_index: usize,
    part_index: usize,
    record_offset: u64,
    weight_count: u32,
    stream_offset: u64,
    stream_relative_offset: i32,
    stream_length: usize,
    stream_checksum: u64,
    stream_prefix_hex: String,
    auxiliary_offset: u64,
    auxiliary_relative_offset: i32,
    mesh_vertex_count: Option<u32>,
    auxiliary_bytes_at_stride_16: Option<usize>,
}

#[derive(Debug, Serialize)]
struct Report {
    manifest_path: String,
    summary: Summary,
    file_errors: Vec<FileError>,
    animations: Vec<AnimationRow>,
    animskins: Vec<AnimSkinRow>,
    components: Vec<ComponentRow>,
    weight_payloads: Vec<WeightPayloadRow>,
}

pub fn execute_command(manifest_path: String, output_folder: Option<String>) -> Result<()> {
    let manifest_path = PathBuf::from(manifest_path);
    let output_folder = output_folder
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./anim_binding_corpus_report"));
    std::fs::create_dir_all(&output_folder)?;

    let entries = read_manifest(&manifest_path)?;
    let mut report = Report {
        manifest_path: manifest_path.to_string_lossy().into_owned(),
        summary: Summary {
            manifest_entries: entries.len(),
            ..Default::default()
        },
        file_errors: Vec::new(),
        animations: Vec::new(),
        animskins: Vec::new(),
        components: Vec::new(),
        weight_payloads: Vec::new(),
    };

    for entry in &entries {
        let Some(platform) = Platform::from_path(&entry.source_path) else {
            report.file_errors.push(FileError {
                declared_uid: entry.declared_uid,
                source_path: entry.source_path.to_string_lossy().into_owned(),
                error: "platform detection failed".to_string(),
            });
            report.summary.files_failed += 1;
            continue;
        };
        match scan_file(entry, platform, &mut report) {
            Ok(()) => report.summary.files_scanned += 1,
            Err(error) => {
                report.file_errors.push(FileError {
                    declared_uid: entry.declared_uid,
                    source_path: entry.source_path.to_string_lossy().into_owned(),
                    error: format!("{error:#}"),
                });
                report.summary.files_failed += 1;
            }
        }
    }

    report.summary.unique_animation_hashes = report
        .animations
        .iter()
        .map(|row| row.animation_hashcode)
        .collect::<BTreeSet<_>>()
        .len();
    report.summary.unique_animskin_hashes = report
        .animskins
        .iter()
        .map(|row| row.animskin_hashcode)
        .collect::<BTreeSet<_>>()
        .len();
    report.summary.unique_component_entity_hashes = report
        .components
        .iter()
        .filter_map(|row| row.entity_hashcode)
        .collect::<BTreeSet<_>>()
        .len();
    report.summary.unique_weight_stream_checksums = report
        .weight_payloads
        .iter()
        .map(|row| row.stream_checksum)
        .collect::<BTreeSet<_>>()
        .len();

    std::fs::write(
        output_folder.join("anim_binding_corpus_report.json"),
        serde_json::to_string_pretty(&report)?,
    )?;
    write_animation_tsv(
        &output_folder.join("animation_skin_bindings.tsv"),
        &report.animations,
    )?;
    write_animskin_tsv(&output_folder.join("animskin_rows.tsv"), &report.animskins)?;
    write_component_tsv(
        &output_folder.join("animskin_entity_bindings.tsv"),
        &report.components,
    )?;
    write_weight_payload_tsv(
        &output_folder.join("animskin_weight_payloads.tsv"),
        &report.weight_payloads,
    )?;

    info!(
        "Wrote {} animations, {} AnimSkins and {} component bindings from {} manifest entries to {}",
        report.animations.len(),
        report.animskins.len(),
        report.components.len(),
        report.summary.manifest_entries,
        output_folder.display()
    );
    Ok(())
}

fn scan_file(entry: &ManifestEntry, platform: Platform, report: &mut Report) -> Result<()> {
    let file = File::open(&entry.source_path)
        .with_context(|| format!("open {}", entry.source_path.display()))?;
    let reader = BufReader::new(file);
    let mut edb = EdbFile::new(Box::new(reader), platform)
        .with_context(|| format!("parse header {}", entry.source_path.display()))?;
    let header = edb.header.clone();
    let path = entry.source_path.to_string_lossy().into_owned();

    for (animation_index, animation) in header.anim_list.iter().enumerate() {
        let skin = header
            .animskin_list
            .iter()
            .find(|skin| skin.base_skin_num == animation.skin_num);
        let skin_binding_status = if animation.skin_num == u32::MAX {
            report.summary.animation_no_skin_sentinel += 1;
            "no_skin_sentinel"
        } else if skin.is_some() {
            report.summary.animation_skin_bindings += 1;
            "resolved_by_base_skin_num"
        } else {
            report.summary.animation_skin_unresolved += 1;
            "unresolved_base_skin_num"
        };
        report.animations.push(AnimationRow {
            edb_uid: header.hashcode,
            edb_path: path.clone(),
            animation_index,
            animation_hashcode: animation.common.hashcode,
            animation_name: hash_name(animation.common.hashcode),
            file_offset: animation.common.address,
            data_size: animation.datasize,
            motiondata_info_addr: animation.motiondata_info_addr,
            skin_num: animation.skin_num,
            skin_binding_status: skin_binding_status.to_string(),
            animskin_hashcode: skin.map(|skin| skin.common.hashcode),
            animskin_name: skin
                .map(|skin| hash_name(skin.common.hashcode))
                .unwrap_or_default(),
        });
        report.summary.animations += 1;
    }

    for (animskin_index, skin_header) in header.animskin_list.iter().enumerate() {
        edb.seek(std::io::SeekFrom::Start(skin_header.common.address as u64))?;
        match edb.read_type_args::<EXGeoBaseAnimSkin>(edb.endian, (header.version,)) {
            Ok(skin) => {
                *report
                    .summary
                    .object_type_counts
                    .entry(format!("0x{:08X}", skin.object_type))
                    .or_default() += 1;
                report.animskins.push(AnimSkinRow {
                    edb_uid: header.hashcode,
                    edb_path: path.clone(),
                    animskin_index,
                    animskin_hashcode: skin_header.common.hashcode,
                    animskin_name: hash_name(skin_header.common.hashcode),
                    file_offset: skin_header.common.address,
                    base_skin_num: skin_header.base_skin_num,
                    mip_ref: skin_header.mip_ref,
                    object_type: Some(skin.object_type),
                    bone_count: Some(skin.bone_count),
                    primary_entities: Some(skin.entities.len()),
                    secondary_entities: Some(skin.more_entities.len()),
                    parse_status: "ok".to_string(),
                });

                for (group, entries) in [
                    ("primary", skin.entities.data().as_slice()),
                    ("secondary", skin.more_entities.data().as_slice()),
                ] {
                    for (component_index, component) in entries.iter().enumerate() {
                        let entity_index = component.entity_index & 0x00FF_FFFF;
                        let entity = header.entity_list.data().get(entity_index as usize);
                        if entity.is_none() {
                            report.summary.component_entity_out_of_range += 1;
                        }
                        let mesh_vertex_counts = if entity.is_some() {
                            read_entity_mesh_vertex_counts(
                                &mut edb,
                                &header,
                                entity_index as usize,
                            )?
                        } else {
                            Vec::new()
                        };
                        report.components.push(ComponentRow {
                            edb_uid: header.hashcode,
                            edb_path: path.clone(),
                            animskin_index,
                            animskin_hashcode: skin_header.common.hashcode,
                            component_group: group.to_string(),
                            component_index,
                            raw_entity_index: component.entity_index,
                            entity_index,
                            entity_binding_status: if entity.is_some() {
                                "resolved"
                            } else {
                                "out_of_range"
                            }
                            .to_string(),
                            entity_hashcode: entity.map(|entity| entity.common.hashcode),
                            entity_name: entity
                                .map(|entity| hash_name(entity.common.hashcode))
                                .unwrap_or_default(),
                            section_index: component.section_index,
                            parts_count: component.parts_count,
                            morph_index: component.morph_index,
                        });
                        for (part_index, payload) in component.skin_data.iter().enumerate() {
                            let stream = payload.bone_palette.as_slice();
                            let stream_checksum = fnv1a64(stream);
                            let stream_prefix_hex = stream
                                .iter()
                                .take(64)
                                .map(|byte| format!("{byte:02X}"))
                                .collect::<Vec<_>>()
                                .join(" ");
                            report.weight_payloads.push(WeightPayloadRow {
                                edb_uid: header.hashcode,
                                edb_path: path.clone(),
                                animskin_index,
                                animskin_hashcode: skin_header.common.hashcode,
                                component_group: group.to_string(),
                                component_index,
                                part_index,
                                record_offset: payload.offset_absolute(),
                                weight_count: payload.palette_count,
                                stream_offset: payload.bone_palette.offset_absolute(),
                                stream_relative_offset: payload.bone_palette.offset_relative(),
                                stream_length: stream.len(),
                                stream_checksum,
                                stream_prefix_hex,
                                auxiliary_offset: payload.vertex_influence_data.offset_absolute(),
                                auxiliary_relative_offset: payload
                                    .vertex_influence_data
                                    .offset_relative(),
                                mesh_vertex_count: mesh_vertex_counts.get(part_index).copied(),
                                auxiliary_bytes_at_stride_16: mesh_vertex_counts
                                    .get(part_index)
                                    .map(|count| *count as usize * 16),
                            });
                            report.summary.weight_payloads += 1;
                            report.summary.weight_stream_bytes += stream.len();
                            report.summary.weight_stream_empty += usize::from(stream.is_empty());
                            *report
                                .summary
                                .weight_count_histogram
                                .entry(payload.palette_count.to_string())
                                .or_default() += 1;
                            *report
                                .summary
                                .weight_length_histogram
                                .entry(stream.len().to_string())
                                .or_default() += 1;
                        }
                        report.summary.component_entities += 1;
                    }
                }
            }
            Err(error) => {
                report.summary.animskin_parse_failures += 1;
                report.animskins.push(AnimSkinRow {
                    edb_uid: header.hashcode,
                    edb_path: path.clone(),
                    animskin_index,
                    animskin_hashcode: skin_header.common.hashcode,
                    animskin_name: hash_name(skin_header.common.hashcode),
                    file_offset: skin_header.common.address,
                    base_skin_num: skin_header.base_skin_num,
                    mip_ref: skin_header.mip_ref,
                    object_type: None,
                    bone_count: None,
                    primary_entities: None,
                    secondary_entities: None,
                    parse_status: format!("error: {error}"),
                });
            }
        }
        report.summary.animskins += 1;
    }
    Ok(())
}

fn read_entity_mesh_vertex_counts(
    edb: &mut EdbFile,
    header: &eurochef_edb::header::EXGeoHeader,
    entity_index: usize,
) -> Result<Vec<u32>> {
    let entity_header = header
        .entity_list
        .data()
        .get(entity_index)
        .context("AnimSkin entity index outside entity list")?;
    let saved_position = edb.stream_position()?;
    edb.seek(std::io::SeekFrom::Start(
        entity_header.common.address as u64,
    ))?;
    let entity: EXGeoEntity = edb.read_type_args(edb.endian, (header.version, edb.platform))?;
    edb.seek(std::io::SeekFrom::Start(saved_position))?;

    let mut counts = Vec::new();
    collect_mesh_vertex_counts(&entity, &mut counts);
    Ok(counts)
}

fn collect_mesh_vertex_counts(entity: &EXGeoEntity, counts: &mut Vec<u32>) {
    match entity {
        EXGeoEntity::Mesh(mesh) => counts.push(mesh.data.vertex_count),
        EXGeoEntity::Split(split) => {
            for child in &split.entities {
                collect_mesh_vertex_counts(child, counts);
            }
        }
        _ => {}
    }
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn hash_name(hashcode: u32) -> String {
    if hashcode.is_local() {
        return format!("Local[{}]", hashcode.index());
    }
    robots_hashdb::resolve(hashcode)
        .map(str::to_string)
        .unwrap_or_else(|| format!("0x{hashcode:08X}"))
}

fn write_animation_tsv(path: &Path, rows: &[AnimationRow]) -> Result<()> {
    let mut output = String::from("edb_uid\tedb_path\tanimation_index\tanimation_hashcode\tanimation_name\tfile_offset\tdata_size\tmotiondata_info_addr\tskin_num\tskin_binding_status\tanimskin_hashcode\tanimskin_name\n");
    for row in rows {
        output.push_str(&format!(
            "0x{:08X}\t{}\t{}\t0x{:08X}\t{}\t0x{:08X}\t{}\t0x{:08X}\t{}\t{}\t{}\t{}\n",
            row.edb_uid,
            row.edb_path,
            row.animation_index,
            row.animation_hashcode,
            row.animation_name,
            row.file_offset,
            row.data_size,
            row.motiondata_info_addr,
            row.skin_num,
            row.skin_binding_status,
            row.animskin_hashcode
                .map(|value| format!("0x{value:08X}"))
                .unwrap_or_default(),
            row.animskin_name,
        ));
    }
    std::fs::write(path, output)?;
    Ok(())
}

fn write_animskin_tsv(path: &Path, rows: &[AnimSkinRow]) -> Result<()> {
    let mut output = String::from("edb_uid\tedb_path\tanimskin_index\tanimskin_hashcode\tanimskin_name\tfile_offset\tbase_skin_num\tmip_ref\tobject_type\tbone_count\tprimary_entities\tsecondary_entities\tparse_status\n");
    for row in rows {
        output.push_str(&format!(
            "0x{:08X}\t{}\t{}\t0x{:08X}\t{}\t0x{:08X}\t{}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\n",
            row.edb_uid,
            row.edb_path,
            row.animskin_index,
            row.animskin_hashcode,
            row.animskin_name,
            row.file_offset,
            row.base_skin_num,
            row.mip_ref,
            row.object_type
                .map(|value| format!("0x{value:08X}"))
                .unwrap_or_default(),
            row.bone_count
                .map(|value| value.to_string())
                .unwrap_or_default(),
            row.primary_entities
                .map(|value| value.to_string())
                .unwrap_or_default(),
            row.secondary_entities
                .map(|value| value.to_string())
                .unwrap_or_default(),
            row.parse_status.replace(['\t', '\n'], " "),
        ));
    }
    std::fs::write(path, output)?;
    Ok(())
}

fn write_component_tsv(path: &Path, rows: &[ComponentRow]) -> Result<()> {
    let mut output = String::from("edb_uid\tedb_path\tanimskin_index\tanimskin_hashcode\tcomponent_group\tcomponent_index\traw_entity_index\tentity_index\tentity_binding_status\tentity_hashcode\tentity_name\tsection_index\tparts_count\tmorph_index\n");
    for row in rows {
        output.push_str(&format!(
            "0x{:08X}\t{}\t{}\t0x{:08X}\t{}\t{}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            row.edb_uid,
            row.edb_path,
            row.animskin_index,
            row.animskin_hashcode,
            row.component_group,
            row.component_index,
            row.raw_entity_index,
            row.entity_index,
            row.entity_binding_status,
            row.entity_hashcode
                .map(|value| format!("0x{value:08X}"))
                .unwrap_or_default(),
            row.entity_name,
            row.section_index,
            row.parts_count,
            row.morph_index,
        ));
    }
    std::fs::write(path, output)?;
    Ok(())
}

fn write_weight_payload_tsv(path: &Path, rows: &[WeightPayloadRow]) -> Result<()> {
    let mut output = String::from("edb_uid\tedb_path\tanimskin_index\tanimskin_hashcode\tcomponent_group\tcomponent_index\tpart_index\trecord_offset\tweight_count\tstream_offset\tstream_relative_offset\tstream_length\tstream_checksum\tstream_prefix_hex\tauxiliary_offset\tauxiliary_relative_offset\tmesh_vertex_count\tauxiliary_bytes_at_stride_16\n");
    for row in rows {
        output.push_str(&format!(
            "0x{:08X}\t{}\t{}\t0x{:08X}\t{}\t{}\t{}\t0x{:08X}\t{}\t0x{:08X}\t{}\t{}\t0x{:016X}\t{}\t0x{:08X}\t{}\t{}\t{}\n",
            row.edb_uid,
            row.edb_path,
            row.animskin_index,
            row.animskin_hashcode,
            row.component_group,
            row.component_index,
            row.part_index,
            row.record_offset,
            row.weight_count,
            row.stream_offset,
            row.stream_relative_offset,
            row.stream_length,
            row.stream_checksum,
            row.stream_prefix_hex,
            row.auxiliary_offset,
            row.auxiliary_relative_offset,
            row.mesh_vertex_count
                .map(|value| value.to_string())
                .unwrap_or_default(),
            row.auxiliary_bytes_at_stride_16
                .map(|value| value.to_string())
                .unwrap_or_default(),
        ));
    }
    std::fs::write(path, output)?;
    Ok(())
}

fn read_manifest(path: &Path) -> Result<Vec<ManifestEntry>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("read manifest {}", path.display()))?;
    let mut entries = Vec::new();
    for (line_index, line) in content.lines().enumerate() {
        if line.trim().is_empty() || (line_index == 0 && line.to_ascii_lowercase().contains("path"))
        {
            continue;
        }
        let columns: Vec<&str> = line.split('\t').collect();
        if columns.len() < 2 {
            continue;
        }
        let declared_uid = parse_uid(columns[0]);
        if declared_uid.is_none() && columns[1].to_ascii_lowercase().contains("source") {
            continue;
        }
        entries.push(ManifestEntry {
            declared_uid,
            source_path: PathBuf::from(columns[1]),
        });
    }
    Ok(entries)
}

fn parse_uid(value: &str) -> Option<u32> {
    let value = value.trim();
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u32::from_str_radix(hex, 16).ok()
    } else {
        value.parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::parse_uid;

    #[test]
    fn parses_manifest_uids_without_treating_decimal_as_hex() {
        assert_eq!(parse_uid("0x01000037"), Some(0x0100_0037));
        assert_eq!(parse_uid("16777271"), Some(16_777_271));
    }
}
