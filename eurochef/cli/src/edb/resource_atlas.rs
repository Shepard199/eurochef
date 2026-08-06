use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use eurochef_edb::{edb::EdbFile, robots_hashdb, versions::Platform, Hashcode, HashcodeUtils};

const ATLAS_FILENAME: &str = "ROBOTS_RESOURCE_ATLAS.md";

#[derive(Debug, Clone)]
struct ManifestEntry {
    declared_uid: Option<Hashcode>,
    source_path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ResourceKind {
    Texture,
    Animation,
    AnimationSkin,
    Script,
    Entity,
}

impl ResourceKind {
    const ALL: [Self; 5] = [
        Self::Texture,
        Self::Animation,
        Self::AnimationSkin,
        Self::Script,
        Self::Entity,
    ];

    fn singular(self) -> &'static str {
        match self {
            Self::Texture => "Texture",
            Self::Animation => "Animation",
            Self::AnimationSkin => "AnimSkin",
            Self::Script => "Script",
            Self::Entity => "Entity",
        }
    }

    fn section(self) -> &'static str {
        match self {
            Self::Texture => "Textures",
            Self::Animation => "Animations",
            Self::AnimationSkin => "Animation Skins",
            Self::Script => "Scripts",
            Self::Entity => "Entities",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct AtlasKey {
    kind: ResourceKind,
    uid: Hashcode,
    // Local UIDs are meaningful only inside their owning EDB. Global UIDs use None
    // and are intentionally merged so repeated resources become searchable.
    local_owner_edb: Option<Hashcode>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ResourceOccurrence {
    edb_uid: Hashcode,
    edb_path: String,
    resource_index: usize,
}

#[derive(Debug, Clone)]
struct AtlasEntry {
    canonical_name: String,
    occurrences: Vec<ResourceOccurrence>,
}

#[derive(Debug, Clone)]
struct ScanError {
    declared_uid: Option<Hashcode>,
    source_path: String,
    error: String,
}

#[derive(Debug, Default)]
struct AtlasStats {
    manifest_entries: usize,
    fallback_root: Option<PathBuf>,
    files_scanned: usize,
    occurrences: usize,
    errors: Vec<ScanError>,
}

pub fn execute_command(manifest_path: String, output_folder: Option<String>) -> Result<()> {
    let manifest_path = PathBuf::from(manifest_path);
    let output_folder = output_folder
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./resource_atlas"));
    std::fs::create_dir_all(&output_folder)
        .with_context(|| format!("create output folder {}", output_folder.display()))?;

    let mut manifest_entries = read_manifest(&manifest_path)?;
    let declared_manifest_entries = manifest_entries.len();
    let fallback_root = if manifest_entries.is_empty() {
        let root = discover_game_root(&manifest_path).with_context(|| {
            format!(
                "manifest {} contains no EDB rows and no game root could be inferred",
                manifest_path.display()
            )
        })?;
        manifest_entries = discover_edb_files(&root)?;
        Some(root)
    } else {
        None
    };

    let mut atlas = BTreeMap::new();
    let mut stats = AtlasStats {
        manifest_entries: declared_manifest_entries,
        fallback_root,
        ..Default::default()
    };

    for manifest_entry in &manifest_entries {
        match scan_edb(manifest_entry, &mut atlas) {
            Ok(occurrences) => {
                stats.files_scanned += 1;
                stats.occurrences += occurrences;
            }
            Err(error) => stats.errors.push(ScanError {
                declared_uid: manifest_entry.declared_uid,
                source_path: manifest_entry.source_path.to_string_lossy().into_owned(),
                error: format!("{error:#}"),
            }),
        }
    }

    for entry in atlas.values_mut() {
        entry.occurrences.sort();
        entry.occurrences.dedup();
    }

    let markdown = render_markdown(&manifest_path, &atlas, &stats);
    let atlas_path = output_folder.join(ATLAS_FILENAME);
    std::fs::write(&atlas_path, markdown)
        .with_context(|| format!("write atlas {}", atlas_path.display()))?;

    info!(
        "Wrote Robots resource atlas: files={} resources={} occurrences={} errors={} path={}",
        stats.files_scanned,
        atlas.len(),
        stats.occurrences,
        stats.errors.len(),
        atlas_path.display()
    );
    Ok(())
}

fn scan_edb(
    manifest_entry: &ManifestEntry,
    atlas: &mut BTreeMap<AtlasKey, AtlasEntry>,
) -> Result<usize> {
    let platform = Platform::from_path(&manifest_entry.source_path).with_context(|| {
        format!(
            "detect platform for {}",
            manifest_entry.source_path.display()
        )
    })?;
    let file = File::open(&manifest_entry.source_path)
        .with_context(|| format!("open {}", manifest_entry.source_path.display()))?;
    let edb = EdbFile::new(Box::new(BufReader::new(file)), platform)
        .with_context(|| format!("parse header {}", manifest_entry.source_path.display()))?;
    let edb_uid = edb.header.hashcode;
    let edb_path = manifest_entry.source_path.to_string_lossy().into_owned();
    let mut count = 0usize;

    for (resource_index, header) in edb.header.texture_list.iter().enumerate() {
        add_resource(
            atlas,
            ResourceKind::Texture,
            header.common.hashcode,
            edb_uid,
            &edb_path,
            resource_index,
        );
        count += 1;
    }
    for (resource_index, header) in edb.header.anim_list.iter().enumerate() {
        add_resource(
            atlas,
            ResourceKind::Animation,
            header.common.hashcode,
            edb_uid,
            &edb_path,
            resource_index,
        );
        count += 1;
    }
    for (resource_index, header) in edb.header.animskin_list.iter().enumerate() {
        add_resource(
            atlas,
            ResourceKind::AnimationSkin,
            header.common.hashcode,
            edb_uid,
            &edb_path,
            resource_index,
        );
        count += 1;
    }
    for (resource_index, header) in edb.header.animscript_list.iter().enumerate() {
        add_resource(
            atlas,
            ResourceKind::Script,
            header.hashcode,
            edb_uid,
            &edb_path,
            resource_index,
        );
        count += 1;
    }
    for (resource_index, header) in edb.header.entity_list.iter().enumerate() {
        add_resource(
            atlas,
            ResourceKind::Entity,
            header.common.hashcode,
            edb_uid,
            &edb_path,
            resource_index,
        );
        count += 1;
    }

    Ok(count)
}

fn add_resource(
    atlas: &mut BTreeMap<AtlasKey, AtlasEntry>,
    kind: ResourceKind,
    uid: Hashcode,
    edb_uid: Hashcode,
    edb_path: &str,
    resource_index: usize,
) {
    let key = AtlasKey {
        kind,
        uid,
        local_owner_edb: uid.is_local().then_some(edb_uid),
    };
    let canonical_name = canonical_resource_name(kind, uid);
    atlas
        .entry(key)
        .or_insert_with(|| AtlasEntry {
            canonical_name,
            occurrences: Vec::new(),
        })
        .occurrences
        .push(ResourceOccurrence {
            edb_uid,
            edb_path: edb_path.to_string(),
            resource_index,
        });
}

fn canonical_resource_name(kind: ResourceKind, uid: Hashcode) -> String {
    if uid == Hashcode::MAX {
        return "HT_None".to_string();
    }
    if uid == 0 {
        return "HT_Zero".to_string();
    }
    if uid.is_local() {
        return format!("HT_Local_{}_{uid:08X}", kind.singular());
    }
    robots_hashdb::resolve(uid)
        .map(str::to_owned)
        .unwrap_or_else(|| format!("HT_Invalid_{uid:08X}"))
}

fn canonical_resource_label(kind: ResourceKind, uid: Hashcode) -> String {
    format!("{} [0x{uid:08X}]", canonical_resource_name(kind, uid))
}

fn canonical_edb_label(edb_uid: Hashcode) -> String {
    let name = robots_hashdb::resolve(edb_uid)
        .map(str::to_owned)
        .unwrap_or_else(|| format!("HT_Invalid_{edb_uid:08X}"));
    format!("{name} [0x{edb_uid:08X}]")
}

fn render_markdown(
    manifest_path: &Path,
    atlas: &BTreeMap<AtlasKey, AtlasEntry>,
    stats: &AtlasStats,
) -> String {
    let mut output = String::new();
    output.push_str("# Robots Resource Atlas\n\n");
    output.push_str(
        "Canonical format: `decoded name [0xUID]`. Global UIDs are merged across EDB files. Local UIDs are deliberately scoped by their owning EDB and are never merged across files merely because their numeric value matches.\n\n",
    );
    output.push_str(&format!(
        "Source manifest: `{}`  \nManifest entries: {}  \nFiles scanned: {}  \nUnique scoped resources: {}  \nResource occurrences: {}  \nFile errors: {}\n",
        escape_markdown(&manifest_path.to_string_lossy()),
        stats.manifest_entries,
        stats.files_scanned,
        atlas.len(),
        stats.occurrences,
        stats.errors.len()
    ));
    if let Some(root) = &stats.fallback_root {
        output.push_str(&format!(
            "Manifest fallback: recursively discovered EDB files under `{}` because the manifest contained no rows.\n",
            escape_markdown(&root.to_string_lossy())
        ));
    }
    output.push('\n');

    output.push_str("## Cross-EDB Global Resources\n\n");
    output.push_str(
        "These are exact global resource UIDs found in more than one EDB. This is the primary duplicate-resource lookup table.\n\n",
    );
    let cross_edb: Vec<_> = atlas
        .iter()
        .filter(|(key, entry)| is_cross_edb_lookup_candidate(key, entry))
        .collect();
    if cross_edb.is_empty() {
        output.push_str("No repeated global resource UIDs were found.\n\n");
    } else {
        write_table(&mut output, cross_edb.into_iter());
    }

    for kind in ResourceKind::ALL {
        output.push_str(&format!("## {}\n\n", kind.section()));
        let entries: Vec<_> = atlas.iter().filter(|(key, _)| key.kind == kind).collect();
        if entries.is_empty() {
            output.push_str("No resources found.\n\n");
        } else {
            write_table(&mut output, entries.into_iter());
        }
    }

    append_gltf_sha_dedup(manifest_path, &mut output);

    if !stats.errors.is_empty() {
        output.push_str("## Scan Errors\n\n");
        output.push_str("| Declared EDB UID | Source | Error |\n");
        output.push_str("|---|---|---|\n");
        for error in &stats.errors {
            let uid = error
                .declared_uid
                .map(|uid| format!("0x{uid:08X}"))
                .unwrap_or_else(|| "unknown".to_string());
            output.push_str(&format!(
                "| `{uid}` | `{}` | {} |\n",
                escape_markdown(&error.source_path),
                escape_markdown(&error.error)
            ));
        }
        output.push('\n');
    }

    output
}

#[derive(Debug, Clone)]
struct GltfDedupRow {
    kind: String,
    sha256: String,
    byte_len: u64,
    canonical_path: String,
    occurrences: usize,
    aliases: String,
}

fn append_gltf_sha_dedup(manifest_path: &Path, output: &mut String) {
    let dedup_manifest = manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("_shared/gltf_library/manifest.tsv");
    let Ok(text) = std::fs::read_to_string(&dedup_manifest) else {
        return;
    };
    let mut rows = text
        .lines()
        .skip(1)
        .filter_map(parse_gltf_dedup_row)
        .filter(|row| row.occurrences > 1)
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return;
    }
    rows.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .then_with(|| left.sha256.cmp(&right.sha256))
    });
    let referenced_bytes = rows
        .iter()
        .map(|row| row.byte_len * row.occurrences as u64)
        .sum::<u64>();
    let canonical_bytes = rows.iter().map(|row| row.byte_len).sum::<u64>();

    output.push_str("## Human-readable glTF Resource Library\n\n");
    output.push_str(
        "Byte-identical resources are merged by SHA-256, while canonical files use decoded names plus exact UIDs and are grouped under their owning EDB. Different EDB-local names remain aliases in per-EDB indexes; no hardlinks or SHA-only filenames are used.\n\n",
    );
    output.push_str(&format!(
        "Duplicate SHA groups: {}  \nReferenced bytes before dedup: {}  \nCanonical bytes after dedup: {}  \nDuplicate bytes removed: {}  \nDedup manifest: `{}`\n\n",
        rows.len(),
        referenced_bytes,
        canonical_bytes,
        referenced_bytes.saturating_sub(canonical_bytes),
        escape_markdown(&dedup_manifest.to_string_lossy())
    ));
    output.push_str("| Kind | SHA-256 | Bytes | Occurrences | Canonical path | Aliases |\n");
    output.push_str("|---|---|---:|---:|---|---|\n");
    for row in rows {
        output.push_str(&format!(
            "| {} | `{}` | {} | {} | `{}` | {} |\n",
            escape_markdown(&row.kind),
            row.sha256,
            row.byte_len,
            row.occurrences,
            escape_markdown(&row.canonical_path),
            row.aliases
                .split("; ")
                .map(|alias| format!("`{}`", escape_markdown(alias)))
                .collect::<Vec<_>>()
                .join("<br>")
        ));
    }
    output.push('\n');
}

fn parse_gltf_dedup_row(line: &str) -> Option<GltfDedupRow> {
    let mut columns = line.splitn(6, '\t');
    Some(GltfDedupRow {
        kind: columns.next()?.to_string(),
        sha256: columns.next()?.to_string(),
        byte_len: columns.next()?.parse().ok()?,
        canonical_path: columns.next()?.to_string(),
        occurrences: columns.next()?.parse().ok()?,
        aliases: columns.next().unwrap_or_default().to_string(),
    })
}

fn write_table<'a>(
    output: &mut String,
    entries: impl Iterator<Item = (&'a AtlasKey, &'a AtlasEntry)>,
) {
    output.push_str("| Canonical resource | Scope | Occurrences | EDB locations |\n");
    output.push_str("|---|---:|---:|---|\n");
    for (key, entry) in entries {
        let label = canonical_resource_label(key.kind, key.uid);
        debug_assert_eq!(
            label,
            format!("{} [0x{:08X}]", entry.canonical_name, key.uid)
        );
        let scope = key
            .local_owner_edb
            .map(|owner| format!("local to {}", canonical_edb_label(owner)))
            .unwrap_or_else(|| "global".to_string());
        let locations = entry
            .occurrences
            .iter()
            .map(|occurrence| {
                format!(
                    "{} · index {} · `{}`",
                    canonical_edb_label(occurrence.edb_uid),
                    occurrence.resource_index,
                    escape_markdown(&occurrence.edb_path)
                )
            })
            .collect::<Vec<_>>()
            .join("<br>");
        output.push_str(&format!(
            "| `{}` | {} | {} | {} |\n",
            escape_markdown(&label),
            escape_markdown(&scope),
            entry.occurrences.len(),
            locations
        ));
    }
    output.push('\n');
}

fn is_cross_edb_lookup_candidate(key: &AtlasKey, entry: &AtlasEntry) -> bool {
    key.local_owner_edb.is_none()
        && key.uid != 0
        && key.uid != Hashcode::MAX
        && (key.uid & 0xFFFF) != 0
        && distinct_edb_count(entry) > 1
}

fn distinct_edb_count(entry: &AtlasEntry) -> usize {
    entry
        .occurrences
        .iter()
        .map(|occurrence| occurrence.edb_uid)
        .collect::<BTreeSet<_>>()
        .len()
}

fn discover_game_root(manifest_path: &Path) -> Option<PathBuf> {
    manifest_path
        .ancestors()
        .find(|ancestor| {
            ancestor
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case("_eurotools_out"))
        })
        .and_then(Path::parent)
        .map(Path::to_path_buf)
}

fn discover_edb_files(root: &Path) -> Result<Vec<ManifestEntry>> {
    let mut pending = vec![root.to_path_buf()];
    let mut paths = Vec::new();
    while let Some(directory) = pending.pop() {
        let entries = std::fs::read_dir(&directory)
            .with_context(|| format!("read fallback EDB directory {}", directory.display()))?;
        for entry in entries {
            let entry = entry.with_context(|| {
                format!("read entry in fallback directory {}", directory.display())
            })?;
            let file_type = entry
                .file_type()
                .with_context(|| format!("read file type {}", entry.path().display()))?;
            if file_type.is_dir() {
                pending.push(entry.path());
            } else if file_type.is_file()
                && entry
                    .path()
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("edb"))
            {
                paths.push(entry.path());
            }
        }
    }
    paths.sort();
    paths.dedup();
    Ok(paths
        .into_iter()
        .map(|source_path| ManifestEntry {
            declared_uid: None,
            source_path,
        })
        .collect())
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
        let mut columns = line.split('\t');
        let Some(uid_text) = columns.next() else {
            continue;
        };
        let Some(source_text) = columns.next() else {
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

fn escape_markdown(value: &str) -> String {
    value
        .replace('|', "\\|")
        .replace(['\r', '\n'], " ")
        .replace('`', "'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoded_global_label_keeps_name_and_exact_uid() {
        assert_eq!(
            canonical_resource_label(ResourceKind::Entity, 0x0200_01B4),
            "HT_Entity_Vehicle_Taxi_Collision [0x020001B4]"
        );
    }

    #[test]
    fn equal_local_uids_in_different_edbs_remain_separate() {
        let mut atlas = BTreeMap::new();
        add_resource(
            &mut atlas,
            ResourceKind::Entity,
            0x8200_0007,
            0x0100_0012,
            "m01_vill.edb",
            7,
        );
        add_resource(
            &mut atlas,
            ResourceKind::Entity,
            0x8200_0007,
            0x0100_001D,
            "m02_city.edb",
            7,
        );

        assert_eq!(atlas.len(), 2);
        assert!(atlas.keys().all(|key| key.local_owner_edb.is_some()));
    }

    #[test]
    fn namespace_base_is_not_a_cross_edb_lookup_result() {
        let mut atlas = BTreeMap::new();
        add_resource(
            &mut atlas,
            ResourceKind::Texture,
            0x0600_0000,
            0x0100_0012,
            "m01_vill.edb",
            0,
        );
        add_resource(
            &mut atlas,
            ResourceKind::Texture,
            0x0600_0000,
            0x0100_001D,
            "m02_city.edb",
            0,
        );

        let (key, entry) = atlas.iter().next().unwrap();
        assert!(!is_cross_edb_lookup_candidate(key, entry));
    }

    #[test]
    fn gltf_sha_manifest_row_preserves_aliases_and_occurrence_count() {
        let row = parse_gltf_dedup_row(
            "image\t0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\t4096\t_shared/gltf_content/images/0123.png\t3\t00001_a/entities/a.gltf#images[0]; 00002_b/entities/b.gltf#images[1]",
        )
        .expect("valid glTF dedup manifest row");

        assert_eq!(row.kind, "image");
        assert_eq!(row.byte_len, 4096);
        assert_eq!(row.occurrences, 3);
        assert_eq!(row.canonical_path, "_shared/gltf_content/images/0123.png");
        assert!(row.aliases.contains("00002_b/entities/b.gltf#images[1]"));
    }

    #[test]
    fn equal_global_uids_in_different_edbs_merge_for_duplicate_lookup() {
        let mut atlas = BTreeMap::new();
        add_resource(
            &mut atlas,
            ResourceKind::Entity,
            0x0200_01B4,
            0x0100_0012,
            "m01_vill.edb",
            3,
        );
        add_resource(
            &mut atlas,
            ResourceKind::Entity,
            0x0200_01B4,
            0x0100_001D,
            "m02_city.edb",
            8,
        );

        assert_eq!(atlas.len(), 1);
        let entry = atlas.values().next().unwrap();
        assert_eq!(distinct_edb_count(entry), 2);
    }
}
