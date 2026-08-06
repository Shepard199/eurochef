use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufReader, Seek, SeekFrom},
};

use anyhow::{bail, Context};
use eurochef_edb::{
    binrw::BinReaderExt, edb::EdbFile, versions::Platform, Hashcode, HashcodeUtils,
};
use eurochef_shared::{
    script::{UXGeoScript, UXGeoScriptCommandData},
    spreadsheets::UXGeoSpreadsheet,
};
use nohash_hasher::IntMap;
use tracing::warn;

use super::super::{ProcessedCharacterVisual, ProcessedMap};

pub const ROBOTS_MONSTER_DATABASE_FILE: Hashcode = 0x0100_0023;
const ROBOTS_MONSTER_DATABASE_SHEET: Hashcode = 0x1400_0005;

/// The exact eight-sheet layout consumed by XTrigger_AI_Character's native
/// resource selector at 0x0047E9E0. Monster rows are 24 bytes and NPC rows are
/// 8 bytes; the first dword of every row is the external character EDB UID.
#[derive(Clone, Debug)]
pub struct RobotsCharacterDatabase {
    files_by_runtime_type: BTreeMap<u32, Vec<Hashcode>>,
}

impl RobotsCharacterDatabase {
    pub fn read(edb: &mut EdbFile) -> anyhow::Result<Self> {
        let spreadsheets = UXGeoSpreadsheet::read_all(edb)?;
        let (_, UXGeoSpreadsheet::Data(sheets)) = spreadsheets
            .into_iter()
            .find(|(hashcode, _)| *hashcode == ROBOTS_MONSTER_DATABASE_SHEET)
            .context("HT_SpreadSheet_MonsterDatabase is missing")?
        else {
            bail!("HT_SpreadSheet_MonsterDatabase is not a data spreadsheet");
        };

        if sheets.len() != 8 {
            bail!(
                "unexpected MonsterDatabase sheet count: expected 8, got {}",
                sheets.len()
            );
        }

        let mut files_by_runtime_type = BTreeMap::new();
        for (sheet_index, sheet) in sheets.into_iter().enumerate() {
            let runtime_type = sheet_index as u32 + 5;
            let row_size = if runtime_type == 11 { 8u64 } else { 24u64 };
            let mut files = Vec::with_capacity(sheet.row_count as usize);
            for row_index in 0..sheet.row_count {
                edb.seek(SeekFrom::Start(
                    sheet.address as u64 + row_index as u64 * row_size,
                ))?;
                files.push(edb.read_type::<u32>(edb.endian)?);
            }
            files_by_runtime_type.insert(runtime_type, files);
        }

        Ok(Self {
            files_by_runtime_type,
        })
    }

    pub fn file_for_trigger(&self, serialized_type: u32, data: &[Option<u32>]) -> Option<Hashcode> {
        let runtime_type = robots_character_runtime_type(serialized_type)?;
        let config_index = data.first().copied().flatten()? as usize;
        self.files_by_runtime_type
            .get(&runtime_type)?
            .get(config_index)
            .copied()
            .filter(|file| file.base() == 0x0100_0000 && *file != 0x0100_0000)
    }
}

/// Exact serialized EXGeoTriggerType -> runtime AI-character family bridge.
/// Runtime types 5..12 index the eight MonsterDatabase sheets.
pub fn robots_character_runtime_type(serialized_type: u32) -> Option<u32> {
    match serialized_type {
        10 => Some(5),
        11 => Some(6),
        18 => Some(7),
        33 => Some(8),
        74 => Some(9),
        3 => Some(10),
        48 => Some(11),
        70 => Some(12),
        _ => None,
    }
}

fn preview_script(edb: &mut EdbFile) -> anyhow::Result<Option<Hashcode>> {
    let saved_internal_references = edb.internal_references.clone();
    let saved_external_references = edb.external_references.clone();
    let scripts = UXGeoScript::read_all(edb)?;
    edb.internal_references = saved_internal_references;
    edb.external_references = saved_external_references;

    Ok(scripts
        .iter()
        .filter(|script| script.hashcode.is_local())
        .find(|script| {
            script
                .commands
                .iter()
                .any(|command| matches!(command.data, UXGeoScriptCommandData::Animation { .. }))
        })
        .or_else(|| {
            scripts.iter().find(|script| {
                script
                    .commands
                    .iter()
                    .any(|command| matches!(command.data, UXGeoScriptCommandData::Animation { .. }))
            })
        })
        .map(|script| script.hashcode))
}

/// Resolves runtime-created Monster/NPC/Fish models before the normal external
/// reference closure is loaded. The game creates these XItems from data[0] and
/// d00_mons.edb rather than serializing visual_object on the trigger.
pub fn resolve_robots_character_visuals(
    current_edb: &mut EdbFile,
    maps: &mut [ProcessedMap],
    path_cache: &IntMap<Hashcode, String>,
    platform: Platform,
) -> anyhow::Result<usize> {
    let Some(database_path) = path_cache.get(&ROBOTS_MONSTER_DATABASE_FILE) else {
        warn!(
            "Robots MonsterDatabase EDB 0x{:08X} is absent from path_cache",
            ROBOTS_MONSTER_DATABASE_FILE
        );
        return Ok(0);
    };

    let database_file = File::open(database_path)
        .with_context(|| format!("open MonsterDatabase EDB {database_path}"))?;
    let mut database_edb = EdbFile::new(Box::new(BufReader::new(database_file)), platform)?;
    let database = RobotsCharacterDatabase::read(&mut database_edb)?;

    let mut scripts_by_file = BTreeMap::<Hashcode, Option<Hashcode>>::new();
    let mut resolved = 0usize;
    for map in maps {
        for trigger in &mut map.triggers {
            let Some(file) = database.file_for_trigger(trigger.ttype, &trigger.data) else {
                continue;
            };
            let runtime_type = robots_character_runtime_type(trigger.ttype).unwrap();
            let config_index = trigger.data[0].unwrap();

            let script = if let Some(script) = scripts_by_file.get(&file) {
                *script
            } else {
                let script = match path_cache.get(&file) {
                    Some(path) => {
                        let external_file = File::open(path)
                            .with_context(|| format!("open character EDB {path}"))?;
                        let mut external_edb =
                            EdbFile::new(Box::new(BufReader::new(external_file)), platform)?;
                        preview_script(&mut external_edb)?
                    }
                    None => {
                        warn!(
                            "Character EDB 0x{file:08X} selected by runtime type {runtime_type} config {config_index} is absent from path_cache"
                        );
                        None
                    }
                };
                scripts_by_file.insert(file, script);
                script
            };

            let Some(script) = script else {
                continue;
            };
            current_edb.add_external_reference(file, script);
            trigger.character_visual = Some(ProcessedCharacterVisual {
                file,
                script,
                runtime_type,
                config_index,
            });
            resolved += 1;
        }
    }

    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader, path::Path};

    use eurochef_edb::{edb::EdbFile, versions::Platform};
    use nohash_hasher::IntMap;

    use super::{
        resolve_robots_character_visuals, robots_character_runtime_type, RobotsCharacterDatabase,
        ROBOTS_MONSTER_DATABASE_FILE,
    };

    #[test]
    fn serialized_character_types_use_the_exact_runtime_database_sheets() {
        assert_eq!(robots_character_runtime_type(10), Some(5));
        assert_eq!(robots_character_runtime_type(11), Some(6));
        assert_eq!(robots_character_runtime_type(18), Some(7));
        assert_eq!(robots_character_runtime_type(33), Some(8));
        assert_eq!(robots_character_runtime_type(74), Some(9));
        assert_eq!(robots_character_runtime_type(3), Some(10));
        assert_eq!(robots_character_runtime_type(48), Some(11));
        assert_eq!(robots_character_runtime_type(70), Some(12));
        assert_eq!(robots_character_runtime_type(4), None);
    }

    #[test]
    fn real_m02_city_resolves_every_runtime_character_visual_when_requested() {
        let Ok(city_path) = std::env::var("EUROCHEF_REAL_M02_CITY_EDB") else {
            return;
        };
        let city_path = Path::new(&city_path);
        let root = city_path.parent().expect("m02_city fixture has no parent");

        let mut path_cache = IntMap::default();
        for entry in std::fs::read_dir(root).expect("could not scan Robots EDB folder") {
            let path = entry.expect("bad EDB directory entry").path();
            if path.extension().and_then(|value| value.to_str()) != Some("edb") {
                continue;
            }
            let file = File::open(&path).expect("could not open indexed EDB");
            let edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
                .expect("indexed EDB header did not parse");
            path_cache.insert(edb.header.hashcode, path.to_string_lossy().into_owned());
        }
        assert!(path_cache.contains_key(&ROBOTS_MONSTER_DATABASE_FILE));

        let database_file = File::open(
            path_cache
                .get(&ROBOTS_MONSTER_DATABASE_FILE)
                .expect("d00_mons.edb was not indexed"),
        )
        .expect("could not open d00_mons.edb");
        let mut database_edb = EdbFile::new(Box::new(BufReader::new(database_file)), Platform::Pc)
            .expect("d00_mons.edb did not parse");
        let database =
            RobotsCharacterDatabase::read(&mut database_edb).expect("MonsterDatabase failed");
        assert_eq!(database.file_for_trigger(10, &[Some(0)]), Some(0x0100_0007));
        assert_eq!(database.file_for_trigger(10, &[Some(1)]), Some(0x0100_001F));
        assert_eq!(database.file_for_trigger(10, &[Some(3)]), Some(0x0100_0027));
        assert_eq!(database.file_for_trigger(11, &[Some(1)]), Some(0x0100_0024));
        assert_eq!(database.file_for_trigger(18, &[Some(2)]), Some(0x0100_002E));
        assert_eq!(
            database.file_for_trigger(48, &[Some(10)]),
            Some(0x0100_0098)
        );
        assert_eq!(database.file_for_trigger(70, &[Some(0)]), Some(0x0100_0066));

        let city_file = File::open(city_path).expect("could not open m02_city.edb");
        let mut city_edb = EdbFile::new(Box::new(BufReader::new(city_file)), Platform::Pc)
            .expect("m02_city.edb did not parse");
        let mut maps = crate::maps::read_from_file(&mut city_edb);
        let resolved =
            resolve_robots_character_visuals(&mut city_edb, &mut maps, &path_cache, Platform::Pc)
                .expect("City character visuals did not resolve");
        assert_eq!(resolved, 77);

        let character_triggers = maps
            .iter()
            .flat_map(|map| &map.triggers)
            .filter(|trigger| robots_character_runtime_type(trigger.ttype).is_some())
            .collect::<Vec<_>>();
        assert_eq!(character_triggers.len(), 77);
        assert!(character_triggers
            .iter()
            .all(|trigger| trigger.character_visual.is_some()));
        assert!(character_triggers.iter().all(|trigger| {
            let visual = trigger.character_visual.as_ref().unwrap();
            city_edb
                .external_references
                .iter()
                .any(|(file, object)| *file == visual.file && *object == visual.script)
        }));
    }
}
