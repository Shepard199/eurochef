use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufReader, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use eurochef_edb::{
    edb::EdbFile,
    particle::{EXGeoParticle, EXGeoParticleCurveRecord},
    versions::Platform,
};
use serde::Serialize;

#[derive(Debug, Clone)]
struct ManifestEntry {
    declared_uid: Option<u32>,
    source_path: PathBuf,
}

#[derive(Debug, Default, Serialize)]
struct ParticleSummary {
    manifest_entries: usize,
    files_scanned: usize,
    files_failed: usize,
    particles: usize,
    particles_with_tail_array_a: usize,
    particles_with_render_entity: usize,
    particles_with_entity_references: usize,
    particles_with_curves: usize,
    entity_references: usize,
    curve_records: usize,
    render_selectors: usize,
    render_selectors_out_of_texture_range: usize,
    selector_counts: BTreeMap<String, usize>,
    curve_channel_counts: BTreeMap<String, usize>,
}

#[derive(Debug, Serialize)]
struct ParticleFileError {
    declared_uid: Option<u32>,
    source_path: String,
    error: String,
}

#[derive(Debug, Serialize)]
struct ParticleRow {
    edb_uid: u32,
    declared_edb_uid: Option<u32>,
    edb_path: String,
    particle_uid: u32,
    particle_index: usize,
    address: u32,
    common: u32,
    selector: u32,
    selector_status: String,
    render_entity: Option<u32>,
    fixed_step: f32,
    emission_rate: f32,
    pool_limit: usize,
    lifetime_center: f32,
    lifetime_extent: f32,
    tail_array_a: Vec<u32>,
    entity_references: Vec<u32>,
    curves: Vec<EXGeoParticleCurveRecord>,
    raw_words: Vec<u32>,
}

#[derive(Debug, Serialize)]
struct ParticleCorpusReport {
    manifest_path: String,
    summary: ParticleSummary,
    file_errors: Vec<ParticleFileError>,
    rows: Vec<ParticleRow>,
}

pub fn execute_command(manifest_path: String, output_folder: Option<String>) -> Result<()> {
    let manifest_path = PathBuf::from(manifest_path);
    let output_folder = output_folder
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./particle_corpus_report"));
    std::fs::create_dir_all(&output_folder)?;

    let entries = read_manifest(&manifest_path)?;
    let mut summary = ParticleSummary {
        manifest_entries: entries.len(),
        ..Default::default()
    };
    let mut rows = Vec::new();
    let mut file_errors = Vec::new();

    for entry in entries {
        match scan_file(&entry) {
            Ok((file_uid, texture_count, particles)) => {
                summary.files_scanned += 1;
                for particle in particles {
                    summary.particles += 1;
                    if !particle.tail_array_a.is_empty() {
                        summary.particles_with_tail_array_a += 1;
                    }
                    if particle.render_entity.is_some() {
                        summary.particles_with_render_entity += 1;
                    }
                    if !particle.entity_references.is_empty() {
                        summary.particles_with_entity_references += 1;
                    }
                    if !particle.curves.is_empty() {
                        summary.particles_with_curves += 1;
                    }
                    summary.entity_references += particle.entity_references.len();
                    summary.curve_records += particle.curves.len();
                    summary.render_selectors += particle.render_resource_selectors.len();
                    summary.render_selectors_out_of_texture_range += particle
                        .render_resource_selectors
                        .iter()
                        .filter(|selector| **selector as usize >= texture_count)
                        .count();
                    for curve in &particle.curves {
                        *summary
                            .curve_channel_counts
                            .entry(curve.channel.to_string())
                            .or_default() += 1;
                    }
                    let selector_key = match particle.particle_type_selector {
                        u32::MAX => "0xFFFFFFFF:default".to_string(),
                        0x1700_0001 => "0x17000001:HT_ParticleType_Inhibitted".to_string(),
                        value => format!("0x{value:08X}:unclassified"),
                    };
                    *summary.selector_counts.entry(selector_key).or_default() += 1;

                    rows.push(ParticleRow {
                        edb_uid: file_uid,
                        declared_edb_uid: entry.declared_uid,
                        edb_path: entry.source_path.to_string_lossy().into_owned(),
                        particle_uid: particle.hashcode,
                        particle_index: particle.index,
                        address: particle.address,
                        common: particle.common,
                        selector: particle.particle_type_selector,
                        selector_status: match particle.particle_type_selector {
                            u32::MAX => "default_runtime_class".to_string(),
                            0x1700_0001 => "inhibitted_runtime_class".to_string(),
                            _ => "unclassified_runtime_class".to_string(),
                        },
                        render_entity: particle.render_entity,
                        fixed_step: particle.fixed_step(),
                        emission_rate: particle.emission_rate(),
                        pool_limit: particle.pool_limit(),
                        lifetime_center: particle.lifetime_center(),
                        lifetime_extent: particle.lifetime_extent(),
                        tail_array_a: particle.tail_array_a,
                        entity_references: particle.entity_references,
                        curves: particle.curves,
                        raw_words: particle.raw_words,
                    });
                }
            }
            Err(error) => {
                summary.files_failed += 1;
                file_errors.push(ParticleFileError {
                    declared_uid: entry.declared_uid,
                    source_path: entry.source_path.to_string_lossy().into_owned(),
                    error: format!("{error:#}"),
                });
            }
        }
    }

    let report = ParticleCorpusReport {
        manifest_path: manifest_path.to_string_lossy().into_owned(),
        summary,
        file_errors,
        rows,
    };
    std::fs::write(
        output_folder.join("particle_corpus_report.json"),
        serde_json::to_string_pretty(&report)?,
    )?;
    write_rows_tsv(
        &output_folder.join("particle_corpus_rows.tsv"),
        &report.rows,
    )?;
    write_summary_tsv(
        &output_folder.join("particle_corpus_summary.tsv"),
        &report.summary,
    )?;

    tracing::info!(
        "Wrote {} EXGeoParticle rows from {} manifest entries to {}",
        report.rows.len(),
        report.summary.manifest_entries,
        output_folder.display()
    );
    Ok(())
}

fn scan_file(entry: &ManifestEntry) -> Result<(u32, usize, Vec<EXGeoParticle>)> {
    let platform = Platform::from_path(&entry.source_path)
        .with_context(|| format!("detect platform for {}", entry.source_path.display()))?;
    let file = File::open(&entry.source_path)
        .with_context(|| format!("open {}", entry.source_path.display()))?;
    let mut edb = EdbFile::new(Box::new(BufReader::new(file)), platform)
        .with_context(|| format!("parse {}", entry.source_path.display()))?;
    let uid = edb.header.hashcode;
    let texture_count = edb.header.texture_list.len();
    let particles = EXGeoParticle::read_all(&mut edb)
        .with_context(|| format!("read particles from {}", entry.source_path.display()))?;
    Ok((uid, texture_count, particles))
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
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u32::from_str_radix(hex, 16).ok()
    } else if value.len() == 8 && value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        u32::from_str_radix(value, 16).ok()
    } else {
        value.parse().ok()
    }
}

fn write_rows_tsv(path: &Path, rows: &[ParticleRow]) -> Result<()> {
    let mut file = File::create(path)?;
    writeln!(
        file,
        "edb_uid\tedb_path\tparticle_uid\tparticle_index\taddress\tcommon\tselector\tselector_status\ttail_array_a\tentity_references\traw_words"
    )?;
    for row in rows {
        writeln!(
            file,
            "0x{:08X}\t{}\t0x{:08X}\t{}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t{}\t{}\t{}\t{}",
            row.edb_uid,
            escape_tsv(&row.edb_path),
            row.particle_uid,
            row.particle_index,
            row.address,
            row.common,
            row.selector,
            row.selector_status,
            escape_tsv(&serde_json::to_string(&row.tail_array_a)?),
            escape_tsv(&serde_json::to_string(&row.entity_references)?),
            escape_tsv(&serde_json::to_string(&row.raw_words)?),
        )?;
    }
    Ok(())
}

fn write_summary_tsv(path: &Path, summary: &ParticleSummary) -> Result<()> {
    let mut file = File::create(path)?;
    writeln!(file, "category\tkey\tcount")?;
    for (key, value) in [
        ("manifest_entries", summary.manifest_entries),
        ("files_scanned", summary.files_scanned),
        ("files_failed", summary.files_failed),
        ("particles", summary.particles),
        (
            "particles_with_tail_array_a",
            summary.particles_with_tail_array_a,
        ),
        (
            "particles_with_render_entity",
            summary.particles_with_render_entity,
        ),
        (
            "particles_with_entity_references",
            summary.particles_with_entity_references,
        ),
        ("particles_with_curves", summary.particles_with_curves),
        ("entity_references", summary.entity_references),
        ("curve_records", summary.curve_records),
        ("render_selectors", summary.render_selectors),
        (
            "render_selectors_out_of_texture_range",
            summary.render_selectors_out_of_texture_range,
        ),
    ] {
        writeln!(file, "summary\t{key}\t{value}")?;
    }
    for (selector, count) in &summary.selector_counts {
        writeln!(file, "selector\t{}\t{}", escape_tsv(selector), count)?;
    }
    for (channel, count) in &summary.curve_channel_counts {
        writeln!(file, "curve_channel\t{}\t{}", channel, count)?;
    }
    Ok(())
}

fn escape_tsv(value: &str) -> String {
    value.replace('\t', " ").replace(['\r', '\n'], " ")
}

#[cfg(test)]
mod tests {
    use super::parse_u32;

    #[test]
    fn parses_manifest_uids() {
        assert_eq!(parse_u32("01000071"), Some(0x0100_0071));
        assert_eq!(parse_u32("0x01000071"), Some(0x0100_0071));
        assert_eq!(parse_u32("113"), Some(113));
    }
}
