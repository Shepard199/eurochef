use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::BufReader,
    path::PathBuf,
};

use anyhow::{bail, Context, Result};
use eurochef_edb::{edb::EdbFile, versions::Platform, Hashcode, HashcodeUtils};
use eurochef_shared::textures::UXGeoTexture;

use super::{resource_atlas::discover_edb_paths_near_manifest, resource_label};

const ALIAS_REPORT_FILENAME: &str = "ROBOTS_TEXTURE_ALIAS_CANDIDATES.tsv";
const OWNER_CATALOG_FILENAME: &str = "ROBOTS_TEXTURE_OWNER_CATALOG.tsv";
const EQUIVALENCE_FILENAME: &str = "ROBOTS_TEXTURE_EQUIVALENCE.tsv";

#[derive(Debug, Clone)]
struct TextureRecord {
    edb_uid: Hashcode,
    edb_path: PathBuf,
    index: usize,
    uid: Hashcode,
    fingerprint: u64,
}

#[derive(Debug)]
struct ExactGroup {
    id: String,
    representative: usize,
    members: Vec<usize>,
    global_uids: BTreeSet<Hashcode>,
}

pub fn execute_command(manifest_path: String, output_folder: Option<String>) -> Result<()> {
    let manifest_path = PathBuf::from(manifest_path);
    let output_folder = output_folder
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./texture_alias_report"));
    std::fs::create_dir_all(&output_folder)
        .with_context(|| format!("create output folder {}", output_folder.display()))?;

    let edb_paths = discover_edb_paths_near_manifest(&manifest_path)?;
    let (mut records, decoded, parse_errors) = scan_texture_records(&edb_paths)?;
    records.sort_by(|a, b| {
        (a.edb_uid, &a.edb_path, a.index, a.uid).cmp(&(b.edb_uid, &b.edb_path, b.index, b.uid))
    });

    let exact_representatives = resolve_exact_representatives(&edb_paths, &records)?;
    let groups = build_exact_groups(&records, &exact_representatives);
    let group_by_member = build_member_group_lookup(&groups);
    let (registered_aliases, unregistered_exact_aliases) =
        validate_registered_aliases(&records, &groups, &group_by_member)?;

    let owner_catalog = build_owner_catalog(&records, &groups, &group_by_member);
    let equivalence = build_equivalence_report(&records, &groups);
    let alias_report = build_alias_report(&records, &groups);

    let owner_catalog_path = output_folder.join(OWNER_CATALOG_FILENAME);
    std::fs::write(&owner_catalog_path, owner_catalog)
        .with_context(|| format!("write {}", owner_catalog_path.display()))?;
    let equivalence_path = output_folder.join(EQUIVALENCE_FILENAME);
    std::fs::write(&equivalence_path, equivalence)
        .with_context(|| format!("write {}", equivalence_path.display()))?;
    let alias_report_path = output_folder.join(ALIAS_REPORT_FILENAME);
    std::fs::write(&alias_report_path, alias_report)
        .with_context(|| format!("write {}", alias_report_path.display()))?;

    let local_textures = records
        .iter()
        .filter(|record| record.uid.is_local())
        .count();
    let exact_duplicate_groups = groups
        .iter()
        .filter(|group| group.members.len() > 1)
        .count();
    let local_exact_duplicates = records
        .iter()
        .enumerate()
        .filter(|(index, record)| {
            record.uid.is_local() && groups[group_by_member[*index]].members.len() > 1
        })
        .count();
    let local_exact_global_aliases = records
        .iter()
        .enumerate()
        .filter(|(index, record)| {
            record.uid.is_local()
                && exact_global_alias(&groups[group_by_member[*index]].global_uids).is_some()
        })
        .count();
    let ambiguous_local_global_matches = records
        .iter()
        .enumerate()
        .filter(|(index, record)| {
            record.uid.is_local() && groups[group_by_member[*index]].global_uids.len() > 1
        })
        .count();

    info!(
        files = edb_paths.len(),
        decoded_textures = decoded,
        parse_errors,
        local_textures,
        exact_groups = groups.len(),
        exact_duplicate_groups,
        local_exact_duplicates,
        local_exact_global_aliases,
        ambiguous_local_global_matches,
        registered_aliases,
        unregistered_exact_aliases,
        owner_catalog = %owner_catalog_path.display(),
        equivalence = %equivalence_path.display(),
        aliases = %alias_report_path.display(),
        "built owner-scoped Robots Texture identity catalog"
    );
    Ok(())
}

fn scan_texture_records(edb_paths: &[PathBuf]) -> Result<(Vec<TextureRecord>, usize, usize)> {
    let mut records = Vec::new();
    let mut decoded = 0usize;
    let mut parse_errors = 0usize;

    for path in edb_paths {
        let platform = Platform::from_path(path)
            .with_context(|| format!("detect platform for {}", path.display()))?;
        let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
        let mut edb = EdbFile::new(Box::new(BufReader::new(file)), platform)
            .with_context(|| format!("parse header {}", path.display()))?;
        let edb_uid = edb.header.hashcode;

        for (index, texture) in UXGeoTexture::read_all(&mut edb) {
            let Ok(texture_data) = texture.data else {
                parse_errors += 1;
                continue;
            };
            decoded += 1;
            records.push(TextureRecord {
                edb_uid,
                edb_path: path.clone(),
                index,
                uid: texture.hashcode,
                fingerprint: texture_fingerprint(&texture_data),
            });
        }
    }

    Ok((records, decoded, parse_errors))
}

/// FNV-64 is only a fast bucket key. Every bucket containing more than one
/// Texture is reparsed and split by exact decoded properties + RGBA frames
/// before it is allowed to become an equivalence group or a global alias.
fn resolve_exact_representatives(
    edb_paths: &[PathBuf],
    records: &[TextureRecord],
) -> Result<Vec<usize>> {
    let mut fingerprint_members: BTreeMap<u64, Vec<usize>> = BTreeMap::new();
    for (index, record) in records.iter().enumerate() {
        fingerprint_members
            .entry(record.fingerprint)
            .or_default()
            .push(index);
    }

    let duplicate_fingerprints = fingerprint_members
        .iter()
        .filter_map(|(fingerprint, members)| (members.len() > 1).then_some(*fingerprint))
        .collect::<BTreeSet<_>>();

    let mut representatives = (0..records.len()).collect::<Vec<_>>();
    if duplicate_fingerprints.is_empty() {
        return Ok(representatives);
    }

    let record_lookup = records
        .iter()
        .enumerate()
        .filter(|(_, record)| duplicate_fingerprints.contains(&record.fingerprint))
        .map(|(index, record)| ((record.edb_path.clone(), record.index), index))
        .collect::<BTreeMap<_, _>>();
    let mut exact_representatives: BTreeMap<u64, Vec<(usize, UXGeoTexture)>> = BTreeMap::new();
    let mut seen_duplicate_records = BTreeSet::new();

    for path in edb_paths {
        let platform = Platform::from_path(path)
            .with_context(|| format!("detect platform for {}", path.display()))?;
        let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
        let mut edb = EdbFile::new(Box::new(BufReader::new(file)), platform)
            .with_context(|| format!("parse header {}", path.display()))?;

        for (texture_index, texture) in UXGeoTexture::read_all(&mut edb) {
            let Ok(texture_data) = texture.data else {
                continue;
            };
            let fingerprint = texture_fingerprint(&texture_data);
            if !duplicate_fingerprints.contains(&fingerprint) {
                continue;
            }
            let Some(&record_index) = record_lookup.get(&(path.clone(), texture_index)) else {
                bail!(
                    "second-pass Texture identity missing first-pass record: {} index {}",
                    path.display(),
                    texture_index
                );
            };
            seen_duplicate_records.insert(record_index);

            let candidates = exact_representatives.entry(fingerprint).or_default();
            if let Some((representative, _)) = candidates
                .iter()
                .find(|(_, candidate)| same_texture_identity(candidate, &texture_data))
            {
                representatives[record_index] = *representative;
            } else {
                representatives[record_index] = record_index;
                candidates.push((record_index, texture_data));
            }
        }
    }

    let expected_duplicate_records = fingerprint_members
        .values()
        .filter(|members| members.len() > 1)
        .map(Vec::len)
        .sum::<usize>();
    if seen_duplicate_records.len() != expected_duplicate_records {
        bail!(
            "second-pass Texture identity covered {} of {} duplicate-fingerprint records",
            seen_duplicate_records.len(),
            expected_duplicate_records
        );
    }

    Ok(representatives)
}

fn build_exact_groups(records: &[TextureRecord], representatives: &[usize]) -> Vec<ExactGroup> {
    let mut members_by_representative: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (member, representative) in representatives.iter().copied().enumerate() {
        members_by_representative
            .entry(representative)
            .or_default()
            .push(member);
    }

    members_by_representative
        .into_iter()
        .enumerate()
        .map(|(ordinal, (representative, members))| {
            let global_uids = members
                .iter()
                .filter_map(|member| {
                    let uid = records[*member].uid;
                    (!uid.is_local()).then_some(uid)
                })
                .collect::<BTreeSet<_>>();
            ExactGroup {
                id: format!("TXG{:05}", ordinal + 1),
                representative,
                members,
                global_uids,
            }
        })
        .collect()
}

fn build_member_group_lookup(groups: &[ExactGroup]) -> Vec<usize> {
    let member_count = groups
        .iter()
        .map(|group| group.members.len())
        .sum::<usize>();
    let mut result = vec![usize::MAX; member_count];
    for (group_index, group) in groups.iter().enumerate() {
        for member in &group.members {
            result[*member] = group_index;
        }
    }
    assert!(result.iter().all(|group| *group != usize::MAX));
    result
}

fn validate_registered_aliases(
    records: &[TextureRecord],
    groups: &[ExactGroup],
    group_by_member: &[usize],
) -> Result<(usize, usize)> {
    let recovered = records
        .iter()
        .enumerate()
        .filter_map(|(index, record)| {
            if !record.uid.is_local() {
                return None;
            }
            exact_global_alias(&groups[group_by_member[index]].global_uids)
                .map(|global_uid| (record.edb_uid, record.uid, global_uid))
        })
        .collect::<BTreeSet<_>>();
    let registered = eurochef_edb::robots_texture_aliases::entries()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();

    let stale = registered
        .difference(&recovered)
        .copied()
        .collect::<Vec<_>>();
    if !stale.is_empty() {
        bail!(
            "registered Robots Texture aliases are not exact corpus matches: {:?}",
            stale
        );
    }

    let unregistered = recovered.difference(&registered).count();
    Ok((registered.len(), unregistered))
}

fn build_owner_catalog(
    records: &[TextureRecord],
    groups: &[ExactGroup],
    group_by_member: &[usize],
) -> String {
    let mut rows = String::from(
        "owner_edb_uid\towner_edb_label\towner_edb_path\ttexture_index\ttexture_uid\tserialized_label\tscope\tfingerprint_fnv64\texact_group\texact_group_size\texact_local_count\texact_global_count\talias_status\trecovered_global_uid\trecovered_global_label\n",
    );

    for (record_index, record) in records.iter().enumerate() {
        let group = &groups[group_by_member[record_index]];
        let local_count = group
            .members
            .iter()
            .filter(|member| records[**member].uid.is_local())
            .count();
        let global_count = group.members.len() - local_count;
        let recovered = record
            .uid
            .is_local()
            .then(|| exact_global_alias(&group.global_uids))
            .flatten();
        let status = alias_status(record.uid, group, recovered);
        let recovered_uid = recovered
            .map(|uid| format!("0x{uid:08X}"))
            .unwrap_or_default();
        let recovered_label = recovered
            .map(|uid| resource_label("Texture", uid))
            .unwrap_or_default();

        rows.push_str(&format!(
            "0x{owner_uid:08X}\t{}\t{}\t{}\t0x{:08X}\t{}\t{}\t0x{:016X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            sanitize_tsv(&resource_label("File", record.edb_uid)),
            sanitize_tsv(&record.edb_path.to_string_lossy()),
            record.index,
            record.uid,
            sanitize_tsv(&resource_label("Texture", record.uid)),
            if record.uid.is_local() { "local" } else { "global" },
            record.fingerprint,
            group.id,
            group.members.len(),
            local_count,
            global_count,
            status,
            recovered_uid,
            sanitize_tsv(&recovered_label),
            owner_uid = record.edb_uid,
        ));
    }
    rows
}

fn build_equivalence_report(records: &[TextureRecord], groups: &[ExactGroup]) -> String {
    let mut rows = String::from(
        "exact_group\tfingerprint_fnv64\tmembers\tlocal_members\tglobal_members\trepresentative_owner_edb_uid\trepresentative_owner_edb_label\trepresentative_index\trepresentative_uid\tglobal_uids\tglobal_labels\tmember_keys\n",
    );

    for group in groups {
        let representative = &records[group.representative];
        let local_count = group
            .members
            .iter()
            .filter(|member| records[**member].uid.is_local())
            .count();
        let global_count = group.members.len() - local_count;
        let global_uids = group
            .global_uids
            .iter()
            .map(|uid| format!("0x{uid:08X}"))
            .collect::<Vec<_>>()
            .join(";");
        let global_labels = group
            .global_uids
            .iter()
            .map(|uid| sanitize_field(&resource_label("Texture", *uid)))
            .collect::<Vec<_>>()
            .join(";");
        let member_keys = group
            .members
            .iter()
            .map(|member| {
                let record = &records[*member];
                format!(
                    "0x{:08X}:{}:0x{:08X}",
                    record.edb_uid, record.index, record.uid
                )
            })
            .collect::<Vec<_>>()
            .join(";");

        rows.push_str(&format!(
            "{}\t0x{:016X}\t{}\t{}\t{}\t0x{:08X}\t{}\t{}\t0x{:08X}\t{}\t{}\t{}\n",
            group.id,
            representative.fingerprint,
            group.members.len(),
            local_count,
            global_count,
            representative.edb_uid,
            sanitize_tsv(&resource_label("File", representative.edb_uid)),
            representative.index,
            representative.uid,
            global_uids,
            global_labels,
            member_keys,
        ));
    }
    rows
}

fn build_alias_report(records: &[TextureRecord], groups: &[ExactGroup]) -> String {
    let mut rows = String::from(
        "local_edb_uid\tlocal_edb\tlocal_index\tlocal_uid\tlocal_label\tfingerprint_fnv64\texact_group\tglobal_edb_uid\tglobal_edb\tglobal_index\tglobal_uid\tglobal_label\n",
    );

    for group in groups {
        if group.global_uids.is_empty() {
            continue;
        }
        let globals = group
            .members
            .iter()
            .filter(|member| !records[**member].uid.is_local())
            .copied()
            .collect::<Vec<_>>();
        for local_index in group
            .members
            .iter()
            .filter(|member| records[**member].uid.is_local())
        {
            let local = &records[*local_index];
            for global_index in &globals {
                let global = &records[*global_index];
                rows.push_str(&format!(
                    "0x{local_edb:08X}\t{}\t{}\t0x{:08X}\t{}\t0x{:016X}\t{}\t0x{global_edb:08X}\t{}\t{}\t0x{:08X}\t{}\n",
                    sanitize_tsv(&local.edb_path.to_string_lossy()),
                    local.index,
                    local.uid,
                    sanitize_tsv(&resource_label("Texture", local.uid)),
                    local.fingerprint,
                    group.id,
                    sanitize_tsv(&global.edb_path.to_string_lossy()),
                    global.index,
                    global.uid,
                    sanitize_tsv(&resource_label("Texture", global.uid)),
                    local_edb = local.edb_uid,
                    global_edb = global.edb_uid,
                ));
            }
        }
    }
    rows
}

fn exact_global_alias(global_uids: &BTreeSet<Hashcode>) -> Option<Hashcode> {
    if global_uids.len() != 1 {
        return None;
    }
    let uid = *global_uids.first()?;
    eurochef_edb::robots_hashdb::resolve(uid).map(|_| uid)
}

fn alias_status(uid: Hashcode, group: &ExactGroup, recovered: Option<Hashcode>) -> &'static str {
    if !uid.is_local() {
        if eurochef_edb::robots_hashdb::resolve(uid).is_some() {
            return "global_named";
        }
        return "global_unknown";
    }
    if recovered.is_some() {
        return "exact_global_alias";
    }
    if group.global_uids.len() > 1 {
        return "ambiguous_global_content_match";
    }
    if !group.global_uids.is_empty() {
        return "global_content_match_unknown_name";
    }
    if group.members.len() > 1 {
        return "local_exact_duplicate";
    }
    "local_unique"
}

fn same_texture_identity(left: &UXGeoTexture, right: &UXGeoTexture) -> bool {
    left.width == right.width
        && left.height == right.height
        && left.depth == right.depth
        && left.format_internal == right.format_internal
        && left.flags == right.flags
        && left.game_flags == right.game_flags
        && left.scroll == right.scroll
        && left.framerate == right.framerate
        && left.frame_count == right.frame_count
        && left.color == right.color
        && left.external_texture == right.external_texture
        && left.frames == right.frames
}

fn texture_fingerprint(texture: &UXGeoTexture) -> u64 {
    let mut hash = FNV1A64_OFFSET;
    hash_bytes(&mut hash, &texture.width.to_le_bytes());
    hash_bytes(&mut hash, &texture.height.to_le_bytes());
    hash_bytes(&mut hash, &texture.depth.to_le_bytes());
    hash_bytes(&mut hash, &[texture.format_internal]);
    hash_bytes(&mut hash, &texture.flags.to_le_bytes());
    hash_bytes(&mut hash, &texture.game_flags.to_le_bytes());
    hash_bytes(&mut hash, &texture.scroll[0].to_le_bytes());
    hash_bytes(&mut hash, &texture.scroll[1].to_le_bytes());
    hash_bytes(&mut hash, &[texture.framerate, texture.frame_count]);
    hash_bytes(&mut hash, &texture.color);
    match texture.external_texture {
        Some((file, texture_uid)) => {
            hash_bytes(&mut hash, &[1]);
            hash_bytes(&mut hash, &file.to_le_bytes());
            hash_bytes(&mut hash, &texture_uid.to_le_bytes());
        }
        None => hash_bytes(&mut hash, &[0]),
    }
    hash_bytes(&mut hash, &(texture.frames.len() as u64).to_le_bytes());
    for frame in &texture.frames {
        hash_bytes(&mut hash, &(frame.len() as u64).to_le_bytes());
        hash_bytes(&mut hash, frame);
    }
    hash
}

const FNV1A64_OFFSET: u64 = 0xcbf29ce484222325;
const FNV1A64_PRIME: u64 = 0x100000001b3;

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(FNV1A64_PRIME);
    }
}

fn sanitize_tsv(path: &str) -> String {
    path.replace(['\t', '\r', '\n'], " ")
}

fn sanitize_field(value: &str) -> String {
    value.replace(['\t', '\r', '\n', ';'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use eurochef_shared::textures::UXTextureDiagnostics;

    fn texture(frame: &[u8]) -> UXGeoTexture {
        UXGeoTexture {
            width: 1,
            height: 1,
            depth: 1,
            format_internal: 0,
            flags: 0,
            game_flags: 0,
            scroll: [0, 0],
            framerate: 0,
            frame_count: 1,
            frames: vec![frame.to_vec()],
            color: [0, 0, 0, 0],
            external_texture: None,
            diagnostics: UXTextureDiagnostics::default(),
        }
    }

    fn group(global_uids: &[Hashcode], members: usize) -> ExactGroup {
        ExactGroup {
            id: "TXG00001".into(),
            representative: 0,
            members: (0..members).collect(),
            global_uids: global_uids.iter().copied().collect(),
        }
    }

    #[test]
    fn texture_fingerprint_changes_with_decoded_payload() {
        assert_ne!(
            texture_fingerprint(&texture(&[1, 2, 3, 4])),
            texture_fingerprint(&texture(&[1, 2, 3, 5]))
        );
    }

    #[test]
    fn texture_fingerprint_is_stable_for_identical_payloads() {
        assert_eq!(
            texture_fingerprint(&texture(&[1, 2, 3, 4])),
            texture_fingerprint(&texture(&[1, 2, 3, 4]))
        );
    }

    #[test]
    fn exact_identity_checks_payload_after_fast_fingerprint_bucket() {
        let mut left = texture(&[1, 2, 3, 4]);
        let right = texture(&[1, 2, 3, 4]);
        assert!(same_texture_identity(&left, &right));
        left.game_flags = 1;
        assert!(!same_texture_identity(&left, &right));
    }

    #[test]
    fn local_alias_requires_one_named_global_uid() {
        let blank_white = 0x0600_0010;
        let one = group(&[blank_white], 2);
        assert_eq!(exact_global_alias(&one.global_uids), Some(blank_white));
        assert_eq!(
            alias_status(0x8600_0001, &one, exact_global_alias(&one.global_uids)),
            "exact_global_alias"
        );

        let ambiguous = group(&[blank_white, 0x0600_0001], 3);
        assert_eq!(exact_global_alias(&ambiguous.global_uids), None);
        assert_eq!(
            alias_status(0x8600_0001, &ambiguous, None),
            "ambiguous_global_content_match"
        );
    }
}
