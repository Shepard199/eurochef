use std::{
    fs::File,
    io::{BufReader, Seek, Write},
    path::Path,
};

use anyhow::Context;
use eurochef_edb::{
    binrw::BinReaderExt,
    edb::EdbFile,
    entity::{EXGeoEntity, EXGeoMapZoneEntity},
    map::{EXGeoLight, EXGeoMap, EXGeoPath, EXGeoPlacement, EXGeoSound, EXGeoTriggerEngineOptions},
    robots_provenance::{
        decode_script_create_flags, RobotsScriptCreateFlags, RobotsScriptCreatorProvenance,
        RobotsScriptSpawnFunctionProvenance, ROBOTS_PC_EXE_SHA256,
        ROBOTS_SCRIPT_CREATOR_PROVENANCE, ROBOTS_SCRIPT_SPAWN_CHAIN,
    },
    versions::Platform,
};

use eurochef_shared::{
    maps::{TriggerInformation, UXGeoTrigger},
    script::UXGeoScript,
};
use serde::Serialize;

use crate::PlatformArg;

pub fn execute_command(
    filename: String,
    platform_arg: Option<PlatformArg>,
    output_folder: Option<String>,
    trigger_defs_file: Option<String>,
) -> anyhow::Result<()> {
    let output_folder = output_folder.unwrap_or(format!(
        "./maps/{}/",
        Path::new(&filename).file_name().unwrap().to_string_lossy()
    ));

    let trigger_typemap = if let Some(path) = trigger_defs_file {
        Some(load_trigger_types(path)?)
    } else {
        None
    };

    let platform = platform_arg
        .clone()
        .map(|p| p.into())
        .or(Platform::from_path(&filename))
        .expect("Failed to detect platform");

    let file = File::open(&filename)?;
    let reader = BufReader::new(file);
    let mut edb = EdbFile::new(Box::new(reader), platform)?;
    let header = edb.header.clone();

    let output_folder = Path::new(&output_folder);
    std::fs::create_dir_all(output_folder)?;

    let scripts = UXGeoScript::read_all(&mut edb)?;
    let scripts_path = output_folder.join(format!("{:08X}.scripts.json", header.hashcode));
    std::fs::write(&scripts_path, serde_json::to_string_pretty(&scripts)?)?;
    info!(
        "Wrote {} decoded scripts to {}",
        scripts.len(),
        scripts_path.display()
    );

    let creators: Vec<&'static RobotsScriptCreatorProvenance> = ROBOTS_SCRIPT_CREATOR_PROVENANCE
        .iter()
        .filter(|entry| {
            scripts
                .iter()
                .any(|script| script.hashcode == entry.script_hashcode.0)
        })
        .collect();
    if !creators.is_empty() {
        let provenance_path =
            output_folder.join(format!("{:08X}.script-provenance.json", header.hashcode));
        let provenance = RobotsScriptProvenanceExport {
            target_executable_sha256: ROBOTS_PC_EXE_SHA256,
            generic_spawn_chain: ROBOTS_SCRIPT_SPAWN_CHAIN,
            creators,
        };
        std::fs::write(&provenance_path, serde_json::to_string_pretty(&provenance)?)?;
        info!(
            "Wrote known Robots script creator provenance to {}",
            provenance_path.display()
        );
    }

    if header.map_list.len() == 0 {
        warn!("File does not contain any maps; decoded script export was still written.");
        return Ok(());
    }

    // * Almost as hacky as calling eurochef through a subprocess
    crate::edb::entities::execute_command(
        filename.clone(),
        platform_arg.clone(),
        Some(output_folder.to_string_lossy().to_string()),
        false,
        false,
    )?;

    for m in &header.map_list {
        edb.seek(std::io::SeekFrom::Start(m.address as u64))?;

        let map = edb
            .read_type_args::<EXGeoMap>(edb.endian, (header.version,))
            .context("Failed to read map")?;

        let mut export = EurochefMapExport {
            paths: map.paths.data().clone(),
            placements: map.placements.data().clone(),
            lights: map.lights.data().clone(),
            sounds: map.sounds.data().clone(),
            skies: map.skies.iter().map(|sky| sky.hashcode).collect(),
            mapzone_entities: vec![],
            triggers: vec![],
            scripts: scripts.clone(),
            trigger_forensics: vec![],
            trigger_scripts: map
                .trigger_header
                .trigger_scripts
                .iter()
                .enumerate()
                .map(|(index, (script, aux))| EurochefTriggerScriptExport {
                    index,
                    script_file_offset: script.offset_absolute(),
                    aux: *aux,
                })
                .collect(),
        };

        for z in &map.zones {
            let entity_offset = header.refpointer_list[z.entity_refptr as usize].address;
            edb.seek(std::io::SeekFrom::Start(entity_offset as u64))
                .context("Mapzone refptr pointer to a non-entity object!")?;

            let ent = edb.read_type_args::<EXGeoEntity>(edb.endian, (header.version, platform))?;

            if let EXGeoEntity::MapZone(mapzone) = ent {
                export.mapzone_entities.push(mapzone);
            } else {
                anyhow::bail!("Refptr entity does not have a mapzone entity!");
            }
        }

        for (index, t) in map.trigger_header.triggers.iter().enumerate() {
            let trig = &t.trigger;
            let (ttype, tsubtype) = {
                let t = &map.trigger_header.trigger_types[trig.type_index as usize];

                (t.trig_type, t.trig_subtype)
            };

            let mut trigger = UXGeoTrigger {
                link_ref: t.link_ref,
                ttype: format!("Trig_{ttype}"),
                tsubtype: if tsubtype != 0 && tsubtype != 0x42000001 {
                    Some(format!("TrigSub_{tsubtype}"))
                } else {
                    None
                },
                debug: trig.debug,
                game_flags: trig.game_flags,
                trig_flags: trig.trig_flags,
                position: trig.position,
                rotation: trig.rotation,
                scale: trig.scale,
                // TODO(cohae): Fix engine options for export
                extra_data: vec![],
                data: trig.data.to_vec(),
                links: trig.links.to_vec(),
            };

            if let Some(ref typemap) = trigger_typemap {
                match typemap.triggers.get(&ttype) {
                    Some(t) => trigger.ttype = t.name.clone(),
                    None => warn!("Couldn't find trigger type {ttype}"),
                }

                if trigger.tsubtype.is_some() {
                    match typemap.triggers.get(&tsubtype) {
                        Some(t) => trigger.tsubtype = Some(t.name.clone()),
                        None => warn!("Couldn't find trigger subtype {tsubtype}"),
                    }
                }
            }

            export.triggers.push(trigger);
            export
                .trigger_forensics
                .push(EurochefTriggerForensicExport {
                    index,
                    trigger_file_offset: t.trigger.offset_absolute(),
                    link_ref: t.link_ref,
                    type_index: trig.type_index,
                    trig_type: ttype,
                    trig_subtype: tsubtype,
                    engine_options: trig.engine_options.clone(),
                    script_create_flags: if ttype == 4 {
                        trig.data[0].map(decode_script_create_flags)
                    } else {
                        None
                    },
                    incoming_links: map
                        .trigger_header
                        .triggers
                        .iter()
                        .enumerate()
                        .filter_map(|(source_index, source)| {
                            source
                                .trigger
                                .links
                                .iter()
                                .any(|target| *target == index as i32)
                                .then_some(source_index)
                        })
                        .collect(),
                });
        }

        let mut outfile = File::create(output_folder.join(format!("{:x}.ecm", m.hashcode)))?;

        let json_string =
            gltf::json::serialize::to_string(&export).context("ECM serialization error")?;

        outfile.write_all(json_string.as_bytes())?;
    }

    info!("Successfully extracted maps!");

    Ok(())
}

#[derive(Serialize)]
pub struct EurochefMapExport {
    pub paths: Vec<EXGeoPath>,
    pub placements: Vec<EXGeoPlacement>,
    pub lights: Vec<EXGeoLight>,
    pub sounds: Vec<EXGeoSound>,
    pub skies: Vec<u32>,
    pub mapzone_entities: Vec<EXGeoMapZoneEntity>,
    pub triggers: Vec<UXGeoTrigger>,
    pub scripts: Vec<UXGeoScript>,
    pub trigger_forensics: Vec<EurochefTriggerForensicExport>,
    pub trigger_scripts: Vec<EurochefTriggerScriptExport>,
}

#[derive(Serialize)]
pub struct EurochefTriggerForensicExport {
    pub index: usize,
    pub trigger_file_offset: u64,
    pub link_ref: i32,
    pub type_index: u16,
    pub trig_type: u32,
    pub trig_subtype: u32,
    pub engine_options: EXGeoTriggerEngineOptions,
    pub script_create_flags: Option<RobotsScriptCreateFlags>,
    pub incoming_links: Vec<usize>,
}

#[derive(Serialize)]
pub struct EurochefTriggerScriptExport {
    pub index: usize,
    pub script_file_offset: u64,
    pub aux: u32,
}

#[derive(Serialize)]
pub struct RobotsScriptProvenanceExport {
    pub target_executable_sha256: &'static str,
    pub generic_spawn_chain: &'static [RobotsScriptSpawnFunctionProvenance],
    pub creators: Vec<&'static RobotsScriptCreatorProvenance>,
}

fn load_trigger_types<P: AsRef<Path>>(path: P) -> anyhow::Result<TriggerInformation> {
    let file = File::open(path).unwrap();
    let mut reader = BufReader::new(file);
    Ok(serde_yaml::from_reader(&mut reader)?)
}
