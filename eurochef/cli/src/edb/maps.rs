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
    script::EXGeoAnimScriptControllerHeader,
    versions::Platform,
    Hashcode, HashcodeUtils,
};

use eurochef_shared::{
    maps::{TriggerInformation, UXGeoTrigger},
    script::{UXGeoScript, UXGeoScriptCommand, UXGeoScriptCommandData},
};
use glam::{Quat, Vec3};
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

        let gui_scene = build_gui_scene_export(&export, header.hashcode);
        std::fs::write(
            output_folder.join(format!("{:x}.gui_scene.json", m.hashcode)),
            serde_json::to_string_pretty(&gui_scene)?,
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

fn build_gui_scene_export(export: &EurochefMapExport, current_file: Hashcode) -> GuiSceneExport {
    let mut items = Vec::new();
    let mut missing = Vec::new();
    let scripts = export
        .scripts
        .iter()
        .map(|script| (script.hashcode, script))
        .collect::<std::collections::BTreeMap<_, _>>();

    for (index, zone) in export.mapzone_entities.iter().enumerate() {
        push_item(
            &mut items,
            "mapzone",
            format!("mapzone[{index}]"),
            current_file,
            zone.entity_refptr,
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::ONE,
        );
    }

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
    scripts: &std::collections::BTreeMap<Hashcode, &UXGeoScript>,
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
            if scripts.contains_key(&object_hash) {
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
    scripts: &std::collections::BTreeMap<Hashcode, &UXGeoScript>,
    current_file: Hashcode,
    script_hashcode: Hashcode,
    current_time: f32,
    position: Vec3,
    rotation: Quat,
    scale: Vec3,
    source: String,
    static_scene: bool,
    mut ancestry: Vec<Hashcode>,
) {
    if ancestry.len() >= 64 || ancestry.contains(&script_hashcode) {
        return;
    }
    let Some(script) = scripts.get(&script_hashcode).copied() else {
        missing.push(GuiSceneMissing {
            source,
            file_hash: current_file,
            object_hash: script_hashcode,
            reason: "script_not_exported",
        });
        return;
    };
    ancestry.push(script_hashcode);

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
            UXGeoScriptCommandData::Sound { hashcode } => missing.push(GuiSceneMissing {
                source: format!("{source}/sound"),
                file_hash: current_file,
                object_hash: hashcode,
                reason: "sound_item",
            }),
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
