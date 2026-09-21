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
    map::{
        EXGeoLight, EXGeoMap, EXGeoMapZone, EXGeoPath, EXGeoPlacement, EXGeoSound,
        EXGeoTriggerEngineOptions,
    },
    robots_provenance::{
        decode_script_create_flags, RobotsScriptCreateFlags, RobotsScriptCreatorProvenance,
        RobotsScriptSpawnFunctionProvenance, ROBOTS_PC_EXE_SHA256,
        ROBOTS_SCRIPT_CREATOR_PROVENANCE, ROBOTS_SCRIPT_SPAWN_CHAIN,
    },
    script::EXGeoAnimScriptControllerHeader,
    versions::Platform,
    Hashcode, HashcodeUtils,
};

use eurochef_shared::{
    maps::{DefinitionDataType, TriggerInformation, UXGeoTrigger},
    script::{UXGeoScript, UXGeoScriptCommand, UXGeoScriptCommandData},
};
use glam::{Quat, Vec3};
use serde::Serialize;

use crate::PlatformArg;

use super::resource_file_stem;

pub fn execute_command(
    filename: String,
    platform_arg: Option<PlatformArg>,
    output_folder: Option<String>,
    trigger_defs_file: Option<String>,
    script_manifest: Option<String>,
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

    let script_catalog = build_cross_edb_script_catalog(
        header.hashcode,
        &scripts,
        script_manifest.as_deref().map(Path::new),
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
            zones: map.zones.clone(),
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
                    resolved_parameters: resolved_trigger_parameters(
                        trigger_typemap.as_ref(),
                        ttype,
                        tsubtype,
                        &trig.data,
                    ),
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

        let gui_scene = build_gui_scene_export(&export, header.hashcode, &script_catalog);
        let runtime_scene = build_robots_runtime_scene_manifest(
            &export,
            &gui_scene,
            output_folder,
            header.hashcode,
            m.hashcode,
        );
        std::fs::write(
            output_folder.join(format!("{:x}.gui_scene.json", m.hashcode)),
            serde_json::to_string_pretty(&gui_scene)?,
        )?;
        std::fs::write(
            output_folder.join(format!("{:x}.robots_scene.json", m.hashcode)),
            serde_json::to_string_pretty(&runtime_scene)?,
        )?;
    }

    info!("Successfully extracted maps!");

    Ok(())
}

#[derive(Serialize)]
pub struct GuiSceneExport {
    pub source_file: u32,
    pub items: Vec<GuiSceneItem>,
    pub missing: Vec<GuiSceneMissing>,
}

#[derive(Serialize)]
pub struct GuiSceneItem {
    pub kind: &'static str,
    pub source: String,
    pub file_hash: u32,
    pub object_hash: u32,
    pub position: [f32; 3],
    pub rotation_xyzw: [f32; 4],
    pub scale: [f32; 3],
}

#[derive(Serialize)]
pub struct GuiSceneMissing {
    pub source: String,
    pub file_hash: u32,
    pub object_hash: u32,
    pub reason: &'static str,
}

#[derive(Serialize)]
pub struct RobotsRuntimeCoordinateContract {
    pub source_units: &'static str,
    pub native_transform_storage: &'static str,
    pub canonical_fbx_target_axis: &'static str,
    pub canonical_fbx_transform: &'static str,
}

#[derive(Serialize)]
pub struct RobotsRuntimeAssetRef {
    pub resource_kind: &'static str,
    pub owner_file: u32,
    pub uid: u32,
    pub canonical_stem: String,
    pub canonical_import_file: Option<String>,
    pub sources: Vec<String>,
}

#[derive(Serialize)]
pub struct RobotsRuntimeZoneCollisionRef {
    pub zone_index: usize,
    pub entity_refptr: u32,
    pub collision_file: Option<String>,
}

#[derive(Serialize)]
pub struct RobotsRuntimeScriptLibraryContract {
    pub schema: &'static str,
    pub identity: &'static str,
    pub owner_index_pattern: &'static str,
}

#[derive(Serialize)]
pub struct RobotsRuntimeSceneManifest<'a> {
    pub schema: &'static str,
    pub source_file: u32,
    pub map_hash: u32,
    pub coordinate_contract: RobotsRuntimeCoordinateContract,
    pub script_library: RobotsRuntimeScriptLibraryContract,
    pub assets: Vec<RobotsRuntimeAssetRef>,
    pub zone_collisions: Vec<RobotsRuntimeZoneCollisionRef>,
    pub scene: &'a GuiSceneExport,
    pub paths: &'a [EXGeoPath],
    pub placements: &'a [EXGeoPlacement],
    pub lights: &'a [EXGeoLight],
    pub sounds: &'a [EXGeoSound],
    pub skies: &'a [u32],
    pub zones: &'a [EXGeoMapZone],
    pub mapzone_entities: &'a [EXGeoMapZoneEntity],
    pub triggers: &'a [UXGeoTrigger],
    pub trigger_forensics: &'a [EurochefTriggerForensicExport],
    pub trigger_scripts: &'a [EurochefTriggerScriptExport],
    pub decoded_scripts_file: String,
}

fn build_robots_runtime_asset_refs(scene: &GuiSceneExport) -> Vec<RobotsRuntimeAssetRef> {
    let mut assets =
        std::collections::BTreeMap::<(&'static str, u32, u32), RobotsRuntimeAssetRef>::new();

    for item in &scene.items {
        let (resource_kind, canonical_stem, canonical_import_file) = match item.kind {
            "placement" | "sky" | "trigger_visual" | "script_entity" => {
                let stem = resource_file_stem("Entity", item.object_hash);
                ("Entity", stem.clone(), Some(format!("{stem}.gltf")))
            }
            "script_animation_target_resource" => {
                let base = resource_file_stem("AnimSkin", item.object_hash);
                let stem = format!("{base}_SK");
                ("AnimSkin", stem.clone(), Some(format!("{stem}.fbx")))
            }
            "script_sound" => {
                let stem = resource_file_stem("Sound", item.object_hash);
                ("Sound", stem, None)
            }
            _ => continue,
        };
        let key = (resource_kind, item.file_hash, item.object_hash);
        let asset = assets.entry(key).or_insert_with(|| RobotsRuntimeAssetRef {
            resource_kind,
            owner_file: item.file_hash,
            uid: item.object_hash,
            canonical_stem,
            canonical_import_file,
            sources: Vec::new(),
        });
        if !asset.sources.contains(&item.source) {
            asset.sources.push(item.source.clone());
        }
    }

    assets.into_values().collect()
}

fn build_robots_runtime_scene_manifest<'a>(
    export: &'a EurochefMapExport,
    scene: &'a GuiSceneExport,
    output_folder: &Path,
    source_file: Hashcode,
    map_hash: Hashcode,
) -> RobotsRuntimeSceneManifest<'a> {
    let zone_collisions = export
        .mapzone_entities
        .iter()
        .enumerate()
        .map(|(zone_index, zone)| {
            let filename = format!("ref_{}.robots_collision.json", zone.entity_refptr);
            RobotsRuntimeZoneCollisionRef {
                zone_index,
                entity_refptr: zone.entity_refptr,
                collision_file: output_folder.join(&filename).is_file().then_some(filename),
            }
        })
        .collect();

    RobotsRuntimeSceneManifest {
        schema: "robots-runtime-scene-v1",
        source_file,
        map_hash,
        coordinate_contract: RobotsRuntimeCoordinateContract {
            source_units: "EuroChef world units (meters in existing glTF path)",
            native_transform_storage: "manifest transforms remain in native EuroChef space",
            canonical_fbx_target_axis: "MayaZUp (+Z up, -Y front, right-handed)",
            canonical_fbx_transform: "(-x, -z, y) * 100",
        },
        script_library: RobotsRuntimeScriptLibraryContract {
            schema: "robots-script-library-v1",
            identity: "(owner_file, script_uid)",
            owner_index_pattern: "{owner_file:08X}/SCRIPTS_INDEX.json",
        },
        assets: build_robots_runtime_asset_refs(scene),
        zone_collisions,
        scene,
        paths: &export.paths,
        placements: &export.placements,
        lights: &export.lights,
        sounds: &export.sounds,
        skies: &export.skies,
        zones: &export.zones,
        mapzone_entities: &export.mapzone_entities,
        triggers: &export.triggers,
        trigger_forensics: &export.trigger_forensics,
        trigger_scripts: &export.trigger_scripts,
        decoded_scripts_file: format!("{source_file:08X}.scripts.json"),
    }
}

type CrossEdbScriptCatalog = std::collections::BTreeMap<(Hashcode, Hashcode), UXGeoScript>;

fn build_cross_edb_script_catalog(
    current_file: Hashcode,
    current_scripts: &[UXGeoScript],
    manifest_path: Option<&Path>,
) -> anyhow::Result<CrossEdbScriptCatalog> {
    let mut scripts = current_scripts
        .iter()
        .cloned()
        .map(|script| ((current_file, script.hashcode), script))
        .collect::<CrossEdbScriptCatalog>();
    let Some(manifest_path) = manifest_path else {
        return Ok(scripts);
    };

    let paths = super::read_corpus_manifest_paths(manifest_path)?;
    let mut files_scanned = 0usize;
    for source_path in paths {
        let platform = Platform::from_path(&source_path)
            .with_context(|| format!("failed to detect platform for {}", source_path.display()))?;
        if platform != Platform::Pc {
            continue;
        }
        let file = File::open(&source_path)
            .with_context(|| format!("failed to open corpus EDB {}", source_path.display()))?;
        let mut edb = EdbFile::new(Box::new(BufReader::new(file)), platform)
            .with_context(|| format!("failed to parse corpus EDB {}", source_path.display()))?;
        let owner_file = edb.header.hashcode;
        files_scanned += 1;
        for script in UXGeoScript::read_all(&mut edb)
            .with_context(|| format!("failed to read Scripts from {}", source_path.display()))?
        {
            scripts
                .entry((owner_file, script.hashcode))
                .or_insert(script);
        }
    }
    info!(
        manifest = %manifest_path.display(),
        files = files_scanned,
        scripts = scripts.len(),
        "built owner-scoped cross-EDB Script catalog for map scene export"
    );
    Ok(scripts)
}

fn build_gui_scene_export(
    export: &EurochefMapExport,
    current_file: Hashcode,
    scripts: &CrossEdbScriptCatalog,
) -> GuiSceneExport {
    let mut items = Vec::new();
    let mut missing = Vec::new();

    for (index, placement) in export.placements.iter().enumerate() {
        let position = Vec3::from(placement.position);
        let rotation = Quat::from_euler(
            glam::EulerRot::ZXY,
            placement.rotation[2],
            placement.rotation[0],
            placement.rotation[1],
        );
        let scale = Vec3::from(placement.scale);
        queue_object(
            &mut items,
            &mut missing,
            &scripts,
            current_file,
            placement.object_ref,
            position,
            rotation,
            scale,
            format!("placement[{index}]"),
            "placement",
            true,
        );
    }

    for (index, sky) in export.skies.iter().copied().enumerate() {
        queue_object(
            &mut items,
            &mut missing,
            &scripts,
            current_file,
            sky,
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::ONE,
            format!("sky[{index}]"),
            "sky",
            true,
        );
    }

    for forensic in &export.trigger_forensics {
        let Some(visual_object) = forensic.engine_options.visual_object else {
            continue;
        };
        let Some(trigger) = export.triggers.get(forensic.index) else {
            continue;
        };
        let position = Vec3::from(trigger.position);
        let rotation = Quat::from_euler(
            glam::EulerRot::ZXY,
            trigger.rotation[2],
            trigger.rotation[0],
            trigger.rotation[1],
        );
        let scale = Vec3::from(trigger.scale);
        let visual_file = if visual_object.is_local() {
            current_file
        } else {
            forensic
                .engine_options
                .visual_object_file
                .unwrap_or(current_file)
        };
        queue_object(
            &mut items,
            &mut missing,
            &scripts,
            visual_file,
            visual_object,
            position,
            rotation,
            scale,
            format!("trigger[{}]", forensic.index),
            "trigger_visual",
            true,
        );
    }

    GuiSceneExport {
        source_file: current_file,
        items,
        missing,
    }
}

#[allow(clippy::too_many_arguments)]
fn queue_object(
    items: &mut Vec<GuiSceneItem>,
    missing: &mut Vec<GuiSceneMissing>,
    scripts: &CrossEdbScriptCatalog,
    file_hash: Hashcode,
    object_hash: Hashcode,
    position: Vec3,
    rotation: Quat,
    scale: Vec3,
    source: String,
    kind: &'static str,
    static_scene: bool,
) {
    match object_hash.base() {
        0x0200_0000 | 0x8200_0000 => push_item(
            items,
            kind,
            source,
            file_hash,
            object_hash,
            position,
            rotation,
            scale,
        ),
        0x0400_0000 | 0x8400_0000 => {
            if scripts.contains_key(&(file_hash, object_hash)) {
                render_script_scene(
                    items,
                    missing,
                    scripts,
                    file_hash,
                    object_hash,
                    0.0,
                    position,
                    rotation,
                    scale,
                    source,
                    static_scene,
                    Vec::new(),
                );
            } else {
                missing.push(GuiSceneMissing {
                    source,
                    file_hash,
                    object_hash,
                    reason: "script_not_exported",
                });
            }
        }
        _ => missing.push(GuiSceneMissing {
            source,
            file_hash,
            object_hash,
            reason: "unsupported_object_base",
        }),
    }
}

#[allow(clippy::too_many_arguments)]
fn render_script_scene(
    items: &mut Vec<GuiSceneItem>,
    missing: &mut Vec<GuiSceneMissing>,
    scripts: &CrossEdbScriptCatalog,
    current_file: Hashcode,
    script_hashcode: Hashcode,
    current_time: f32,
    position: Vec3,
    rotation: Quat,
    scale: Vec3,
    source: String,
    static_scene: bool,
    mut ancestry: Vec<(Hashcode, Hashcode)>,
) {
    let script_key = (current_file, script_hashcode);
    if ancestry.len() >= 64 || ancestry.contains(&script_key) {
        return;
    }
    let Some(script) = scripts.get(&script_key) else {
        missing.push(GuiSceneMissing {
            source,
            file_hash: current_file,
            object_hash: script_hashcode,
            reason: "script_not_exported",
        });
        return;
    };
    ancestry.push(script_key);

    let current_frame = (script.frame_at_time(current_time) + 1.0e-4).floor() as isize;
    for command in script
        .commands
        .iter()
        .filter(|command| static_scene || command.range().contains(&current_frame))
    {
        let controller = command_controller(&script.controllers, command.controller_header_index);
        let (local_position, local_rotation, local_scale) =
            controller_transform(script, command, controller, current_time);
        let child_position = position + rotation.mul_vec3(scale * local_position);
        let child_rotation = rotation * local_rotation;
        let child_scale = scale * local_scale;

        match command.data {
            UXGeoScriptCommandData::Entity { hashcode, file } => {
                let file = if file == u32::MAX || hashcode.is_local() {
                    current_file
                } else {
                    file
                };
                push_item(
                    items,
                    "script_entity",
                    format!("{source}/cmd[{}]", command.opcode),
                    file,
                    hashcode,
                    child_position,
                    child_rotation,
                    child_scale,
                );
            }
            UXGeoScriptCommandData::SubScript { hashcode, file } => {
                let file = if file == u32::MAX || hashcode.is_local() {
                    current_file
                } else {
                    file
                };
                let child_time =
                    (current_time - script.time_at_frame(command.start as f32)).max(0.0);
                render_script_scene(
                    items,
                    missing,
                    scripts,
                    file,
                    hashcode,
                    child_time,
                    child_position,
                    child_rotation,
                    child_scale,
                    format!("{source}/subscript[{hashcode:08X}]"),
                    static_scene,
                    ancestry.clone(),
                );
            }
            UXGeoScriptCommandData::Animation {
                skin_file,
                skin_hashcode,
                ..
            } => {
                if skin_hashcode != u32::MAX {
                    let file = if skin_file == u32::MAX || skin_hashcode.is_local() {
                        current_file
                    } else {
                        skin_file
                    };
                    push_item(
                        items,
                        "script_animation_target_resource",
                        format!("{source}/animation"),
                        file,
                        skin_hashcode,
                        child_position,
                        child_rotation,
                        child_scale,
                    );
                }
            }
            UXGeoScriptCommandData::Sound { hashcode } => push_item(
                items,
                "script_sound",
                format!("{source}/sound"),
                current_file,
                hashcode,
                child_position,
                child_rotation,
                child_scale,
            ),
            _ => {}
        }
    }
}

fn push_item(
    items: &mut Vec<GuiSceneItem>,
    kind: &'static str,
    source: String,
    file_hash: Hashcode,
    object_hash: Hashcode,
    position: Vec3,
    rotation: Quat,
    scale: Vec3,
) {
    items.push(GuiSceneItem {
        kind,
        source,
        file_hash,
        object_hash,
        position: position.to_array(),
        rotation_xyzw: rotation.to_array(),
        scale: scale.to_array(),
    });
}

fn command_controller(
    controllers: &[EXGeoAnimScriptControllerHeader],
    controller_header_index: u16,
) -> Option<&EXGeoAnimScriptControllerHeader> {
    if controller_header_index == u16::MAX {
        return None;
    }

    controllers.get(controller_header_index as usize)
}

fn controller_transform(
    script: &UXGeoScript,
    command: &UXGeoScriptCommand,
    controller: Option<&EXGeoAnimScriptControllerHeader>,
    current_time: f32,
) -> (Vec3, Quat, Vec3) {
    let Some(controller) = controller else {
        return (Vec3::ZERO, Quat::IDENTITY, Vec3::ONE);
    };

    let current_frame = script.frame_at_time(current_time);
    let mut position = Vec3::ZERO;
    let mut rotation = Quat::IDENTITY;
    let mut scale = Vec3::ONE;

    if let Some((_, value)) = controller.channels.quat_0.first() {
        let quat = Quat::from_array(*value);
        if quat.length_squared().is_finite() && quat.length_squared() > f32::EPSILON {
            rotation = quat.normalize();
        }
    }

    if !controller.channels.vector_0.is_empty() {
        position = interpolate_vec3(
            &controller.channels.vector_0,
            current_frame,
            command.start as f32,
            Vec3::ZERO,
        );
    }

    if !controller.channels.vector_1.is_empty() {
        scale = interpolate_vec3(
            &controller.channels.vector_1,
            current_frame,
            command.start as f32,
            Vec3::ONE,
        );
    }

    (position, rotation, scale)
}

fn interpolate_vec3(
    values: &[(f32, [f32; 3])],
    frame: f32,
    command_start: f32,
    default: Vec3,
) -> Vec3 {
    let previous_index = values.iter().rposition(|(key, _)| *key <= frame);
    let (start, start_value, end, end_value) = if let Some(previous_index) = previous_index {
        let (start, start_value) = values[previous_index];
        let (end, end_value) = values
            .get(previous_index + 1)
            .copied()
            .unwrap_or((start, start_value));
        (start, start_value, end, end_value)
    } else if let Some((end, end_value)) = values.first().copied() {
        (command_start, default.to_array(), end, end_value)
    } else {
        (
            command_start,
            default.to_array(),
            command_start,
            default.to_array(),
        )
    };
    let start_value = Vec3::from(start_value);
    let end_value = Vec3::from(end_value);
    if start == end {
        start_value
    } else {
        let offset = ((frame - start) / (end - start)).clamp(0.0, 1.0);
        start_value.lerp(end_value, offset)
    }
}

#[derive(Serialize)]
pub struct EurochefMapExport {
    pub paths: Vec<EXGeoPath>,
    pub placements: Vec<EXGeoPlacement>,
    pub lights: Vec<EXGeoLight>,
    pub sounds: Vec<EXGeoSound>,
    pub skies: Vec<u32>,
    pub zones: Vec<EXGeoMapZone>,
    pub mapzone_entities: Vec<EXGeoMapZoneEntity>,
    pub triggers: Vec<UXGeoTrigger>,
    pub scripts: Vec<UXGeoScript>,
    pub trigger_forensics: Vec<EurochefTriggerForensicExport>,
    pub trigger_scripts: Vec<EurochefTriggerScriptExport>,
}

#[derive(Serialize)]
pub struct EurochefTriggerParameterExport {
    pub index: u32,
    pub name: Option<String>,
    pub dtype: &'static str,
    pub raw: Option<u32>,
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
    pub resolved_parameters: Vec<EurochefTriggerParameterExport>,
    pub script_create_flags: Option<RobotsScriptCreateFlags>,
    pub incoming_links: Vec<usize>,
}

fn trigger_dtype_name(dtype: DefinitionDataType) -> &'static str {
    match dtype {
        DefinitionDataType::Unknown32 => "unknown32",
        DefinitionDataType::U32 => "u32",
        DefinitionDataType::Float => "float",
        DefinitionDataType::Hashcode => "hashcode",
        DefinitionDataType::Pickup => "pickup",
        DefinitionDataType::ScriptCreateFlags => "scriptflags",
    }
}

fn resolved_trigger_parameters(
    typemap: Option<&TriggerInformation>,
    trig_type: u32,
    trig_subtype: u32,
    data: &[Option<u32>],
) -> Vec<EurochefTriggerParameterExport> {
    let Some(typemap) = typemap else {
        return Vec::new();
    };
    let mut definitions = std::collections::BTreeMap::new();
    if let Some(primary) = typemap.triggers.get(&trig_type) {
        definitions.extend(primary.values.iter().map(|(index, value)| (*index, value)));
    }
    if trig_subtype != 0 && trig_subtype != 0x4200_0001 {
        if let Some(subtype) = typemap.triggers.get(&trig_subtype) {
            definitions.extend(subtype.values.iter().map(|(index, value)| (*index, value)));
        }
    }
    definitions
        .into_iter()
        .map(|(index, value)| EurochefTriggerParameterExport {
            index,
            name: value.name.clone(),
            dtype: trigger_dtype_name(value.dtype),
            raw: data.get(index as usize).copied().flatten(),
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn robots_runtime_scene_manifest_keeps_native_space_and_versioned_schema() {
        let export = EurochefMapExport {
            paths: vec![],
            placements: vec![],
            lights: vec![],
            sounds: vec![],
            skies: vec![],
            zones: vec![],
            mapzone_entities: vec![],
            triggers: vec![],
            scripts: vec![],
            trigger_forensics: vec![],
            trigger_scripts: vec![],
        };
        let scene = GuiSceneExport {
            source_file: 0x1234_5678,
            items: vec![
                GuiSceneItem {
                    kind: "placement",
                    source: "placement[0]".to_string(),
                    file_hash: 0x1234_5678,
                    object_hash: 0x8200_0001,
                    position: [0.0; 3],
                    rotation_xyzw: [0.0, 0.0, 0.0, 1.0],
                    scale: [1.0; 3],
                },
                GuiSceneItem {
                    kind: "script_entity",
                    source: "placement[1]/cmd[3]".to_string(),
                    file_hash: 0x1234_5678,
                    object_hash: 0x8200_0001,
                    position: [0.0; 3],
                    rotation_xyzw: [0.0, 0.0, 0.0, 1.0],
                    scale: [1.0; 3],
                },
                GuiSceneItem {
                    kind: "script_animation_target_resource",
                    source: "placement[2]/animation".to_string(),
                    file_hash: 0x8765_4321,
                    object_hash: 0x8300_0007,
                    position: [0.0; 3],
                    rotation_xyzw: [0.0, 0.0, 0.0, 1.0],
                    scale: [1.0; 3],
                },
                GuiSceneItem {
                    kind: "script_sound",
                    source: "placement[2]/sound".to_string(),
                    file_hash: 0x8765_4321,
                    object_hash: 0x0A00_0010,
                    position: [0.0; 3],
                    rotation_xyzw: [0.0, 0.0, 0.0, 1.0],
                    scale: [1.0; 3],
                },
            ],
            missing: vec![],
        };

        let manifest = build_robots_runtime_scene_manifest(
            &export,
            &scene,
            Path::new("."),
            0x1234_5678,
            0x9ABC_DEF0,
        );
        let value = serde_json::to_value(manifest).expect("runtime scene manifest JSON");

        assert_eq!(value["schema"], "robots-runtime-scene-v1");
        assert_eq!(value["source_file"], 0x1234_5678u32);
        assert_eq!(value["map_hash"], 0x9ABC_DEF0u32);
        assert_eq!(
            value["coordinate_contract"]["native_transform_storage"],
            "manifest transforms remain in native EuroChef space"
        );
        assert_eq!(
            value["coordinate_contract"]["canonical_fbx_transform"],
            "(-x, -z, y) * 100"
        );
        assert_eq!(
            value["script_library"]["schema"],
            "robots-script-library-v1"
        );
        assert_eq!(
            value["script_library"]["identity"],
            "(owner_file, script_uid)"
        );
        assert_eq!(value["decoded_scripts_file"], "12345678.scripts.json");
        assert_eq!(value["zone_collisions"].as_array().map(Vec::len), Some(0));
        assert_eq!(value["assets"].as_array().map(Vec::len), Some(3));
        assert_eq!(value["assets"][0]["resource_kind"], "AnimSkin");
        assert_eq!(value["assets"][0]["owner_file"], 0x8765_4321u32);
        assert!(value["assets"][0]["canonical_import_file"]
            .as_str()
            .is_some_and(|path| path.ends_with("_SK.fbx")));
        assert_eq!(value["assets"][1]["resource_kind"], "Entity");
        assert_eq!(value["assets"][1]["owner_file"], 0x1234_5678u32);
        assert!(value["assets"][1]["canonical_import_file"]
            .as_str()
            .is_some_and(|path| path.ends_with(".gltf")));
        assert_eq!(
            value["assets"][1]["sources"].as_array().map(Vec::len),
            Some(2)
        );
        assert_eq!(value["assets"][2]["resource_kind"], "Sound");
        assert_eq!(value["assets"][2]["owner_file"], 0x8765_4321u32);
        assert!(value["assets"][2]["canonical_import_file"].is_null());
    }
}
