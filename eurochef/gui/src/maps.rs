use std::{io::Seek, sync::Arc};

use anyhow::Context;

use egui::mutex::{Mutex, RwLock};
use eurochef_edb::{
    binrw::BinReaderExt,
    edb::EdbFile,
    entity::{EXGeoEntity, EXGeoMapZoneEntity},
    map::{EXGeoBaseDatum, EXGeoMap, EXGeoMapZone, EXGeoPlacement, EXGeoTriggerEngineOptions},
    versions::Platform,
    Hashcode,
};
use eurochef_shared::IdentifiableResult;
use glam::{Vec2, Vec3, Vec4};
use nohash_hasher::IntMap;

use crate::{
    entities::ProcessedEntityMesh,
    map_frame::MapFrame,
    render::{entity::EntityRenderer, viewer::CameraType, NativeLightingTriangle, RenderStore},
    sound_preview::SharedSoundPreview,
};

mod dev_map;
mod entities;
mod triggers;

pub(crate) use dev_map::robots_dev_map_info;
pub use entities::{resolve_robots_character_visuals, robots_pickup_visual};
pub use triggers::{
    robots_camera_controller_plan, robots_camera_flags, robots_camera_marker_scaled_data0,
    robots_camera_mode, robots_camera_scaled_data4, robots_camera_scaled_data5,
    robots_direct_object_audio_profile, robots_monster_data15_value, robots_monster_data4_value,
    robots_monster_flags, robots_monster_is_family, robots_monster_proximity_radius,
    robots_monster_runtime_selector, robots_monster_test_runtime_value,
    robots_monster_transporter_secondary_path_hash, robots_npc_alternate_cutscenes,
    robots_npc_cutscene_is_null, robots_npc_flags, robots_npc_runtime_selector,
    robots_npc_runtime_uid, robots_npc_text_group, robots_object_audio_is_consumer,
    robots_object_audio_is_enabled, robots_object_audio_profile_for_source,
    robots_trigger_path_data_slot, robots_trigger_path_hash, robots_trigger_path_is_proven,
    robots_trigger_platform_angular_velocity, robots_trigger_runtime_path_acceleration,
    robots_trigger_runtime_path_speed, robots_watchbot_enter_distance, robots_watchbot_flags,
    robots_watchbot_leave_distance, robots_watchbot_mode, ObjectAudioProfile,
};

pub struct MapViewerPanel {
    maps: Vec<ProcessedMap>,

    // TODO(cohae): Replace so we can do funky stuff
    frame: MapFrame,
}

#[derive(Clone, Default)]
#[allow(dead_code)]
pub struct ProcessedMap {
    pub hashcode: u32,
    pub mapzone_entities: Vec<EXGeoMapZoneEntity>,
    pub zones: Vec<EXGeoMapZone>,
    pub skies: Vec<Hashcode>,
    pub placements: Vec<EXGeoPlacement>,
    pub placement_group_count: usize,
    pub cameras: Vec<ProcessedCamera>,
    pub portals: Vec<ProcessedPortal>,
    pub isounds: Vec<u16>,
    pub lights: Vec<ProcessedLight>,
    pub sounds: Vec<ProcessedSound>,
    pub lighting_triangles: Vec<NativeLightingTriangle>,
    pub paths: Vec<ProcessedPath>,
    pub triggers: Vec<ProcessedTrigger>,
    pub trigger_collisions: Vec<EXGeoBaseDatum>,
}

fn map_editor_start_position(map: &ProcessedMap) -> Option<Vec3> {
    let mut bounds_min = Vec3::splat(f32::INFINITY);
    let mut bounds_max = Vec3::splat(f32::NEG_INFINITY);

    for zone in &map.zones {
        let a = Vec3::from(zone.bounds_box[0]);
        let b = Vec3::from(zone.bounds_box[1]);
        bounds_min = bounds_min.min(a.min(b));
        bounds_max = bounds_max.max(a.max(b));
    }

    if !bounds_min.is_finite() || !bounds_max.is_finite() {
        return None;
    }

    let center = (bounds_min + bounds_max) * 0.5;
    let y = if bounds_min.y <= 0.0 && bounds_max.y >= 0.0 {
        0.0
    } else {
        center.y
    };
    Some(Vec3::new(center.x, y, center.z))
}

#[derive(Debug, Clone)]
pub struct ProcessedCamera {
    pub hashcode: u32,
    pub position: Vec3,
    pub flags: u32,
    pub look: Vec3,
    pub focal_length: f32,
    pub aperture_width: f32,
    pub aperture_height: f32,
}

#[derive(Debug, Clone)]
pub struct ProcessedPortal {
    pub map_a: u16,
    pub map_b: u16,
    pub flags: u16,
    pub distance: f32,
    pub vertices: [Vec3; 4],
    pub face_common: u32,
    pub face_texture_ref: u32,
    pub face_flags: u32,
    pub face_vertices: Vec<Vec3>,
}

pub fn robots_portal_neighbor_zone(
    portal: &ProcessedPortal,
    zone_index: usize,
    zone_count: usize,
) -> Option<usize> {
    let zone_a = portal.map_a as usize;
    let zone_b = portal.map_b as usize;
    if zone_a >= zone_count || zone_b >= zone_count || zone_a == zone_b {
        return None;
    }
    if zone_index == zone_a {
        Some(zone_b)
    } else if zone_index == zone_b {
        Some(zone_a)
    } else {
        None
    }
}

#[derive(Debug, Clone)]
pub struct ProcessedSound {
    pub hashcode: u32,
    pub position: Vec3,
    pub flags: u32,
    pub sound_ref: u32,
    pub color: [u8; 4],
    pub volume: u8,
    pub fade_in: u8,
    pub fade_out: u8,
    pub tracking_type: u8,
    pub inner_radius: f32,
    pub outer_radius: f32,
    pub base_map_on: u32,
}

#[derive(Debug, Clone)]
pub struct ProcessedLight {
    pub hashcode: u32,
    pub position: Vec3,
    pub flags: u32,
    pub beam: Vec3,
    pub light_type: u16,
    pub beam_angle: u16,
    pub colour: [u8; 4],
    pub radius: f32,
    pub max_effect_fraction: f32,
}

/// Robots.exe 0x00554C20..0x00554D8A converts the serialized RGB bytes to
/// floating-point light colour with a 1/128 scale, not the usual 1/255 scale.
/// Values above 0x80 are therefore intentionally brighter than 1.0 and are
/// allowed to saturate later in the original D3D lighting pipeline.
pub fn robots_native_light_colour(colour: [u8; 4]) -> Vec3 {
    const ROBOTS_LIGHT_COLOUR_SCALE: f32 = 1.0 / 128.0;
    Vec3::new(
        colour[0] as f32 * ROBOTS_LIGHT_COLOUR_SCALE,
        colour[1] as f32 * ROBOTS_LIGHT_COLOUR_SCALE,
        colour[2] as f32 * ROBOTS_LIGHT_COLOUR_SCALE,
    )
}

pub fn robots_native_light_type_description(light_type: u16) -> String {
    let mut features = Vec::new();
    if light_type & 0x1 != 0 {
        features.push("range");
    }
    if light_type & 0x2 != 0 {
        features.push("position-normal");
    }
    if light_type & 0x4 != 0 {
        features.push("beam-cone");
    }
    if light_type & 0x8 != 0 {
        features.push("beam-normal");
    }

    let unknown_bits = light_type & !0x000f;
    if unknown_bits != 0 {
        features.push("unknown-bits");
    }
    if features.is_empty() {
        features.push("constant");
    }

    if unknown_bits != 0 {
        format!("{} + 0x{unknown_bits:04x}", features.join(" + "))
    } else {
        features.join(" + ")
    }
}

#[derive(Clone, Debug)]
pub struct ProcessedPathNode {
    pub position: Vec3,
    pub size: Vec2,
    pub value: [u16; 4],
    pub flags: u32,
    pub distance: f32,
    pub num_links: u16,
}

#[derive(Clone)]
pub struct ProcessedPath {
    pub hashcode: u32,
    pub position: Vec3,
    pub flags: u32,
    pub path_type: u16,
    pub nodes: Vec<ProcessedPathNode>,
    pub links: Vec<(usize, usize)>,
}

#[derive(Clone)]
pub struct ProcessedTriggerScript {
    pub file_offset: u64,
    pub aux: u32,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct ProcessedCharacterVisual {
    pub file: Hashcode,
    pub script: Hashcode,
    pub runtime_type: u32,
    pub config_index: u32,
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct ProcessedTrigger {
    pub file_offset: u64,
    pub link_ref: i32,
    pub type_index: u16,

    pub ttype: u32,
    pub tsubtype: Option<u32>,

    pub debug: u16,
    pub game_flags: u32,
    pub trig_flags: u32,
    pub position: Vec3,
    pub rotation: Vec3,
    pub scale: Vec3,

    pub data: Vec<Option<u32>>,
    pub links: Vec<i32>,
    pub engine_options: EXGeoTriggerEngineOptions,
    pub trigger_script: Option<ProcessedTriggerScript>,
    pub character_visual: Option<ProcessedCharacterVisual>,

    /// Every trigger that links to this one
    pub incoming_links: Vec<i32>,
}

impl MapViewerPanel {
    pub fn new(
        file: Hashcode,
        gl: Arc<glow::Context>,
        maps: Vec<ProcessedMap>,
        ref_entities: Vec<IdentifiableResult<(EXGeoEntity, ProcessedEntityMesh)>>,
        render_store: Arc<RwLock<RenderStore>>,
        platform: Platform,
        hashcodes: Arc<IntMap<u32, String>>,
        game: &str,
        sound_preview: SharedSoundPreview,
    ) -> Self {
        let mut maps = maps;
        Self::populate_lighting_triangles(&mut maps, &ref_entities);
        let initial_camera_position = maps.first().and_then(map_editor_start_position);
        MapViewerPanel {
            frame: {
                let ef = MapFrame::new(
                    file,
                    Self::load_map_meshes(file, &gl, &maps, &ref_entities, platform),
                    gl,
                    render_store,
                    hashcodes,
                    game,
                    sound_preview,
                );

                {
                    let mut e = ef.viewer.lock();
                    e.selected_camera = CameraType::Fly;
                    e.show_grid = false;
                    if let Some(position) = initial_camera_position {
                        e.camera_fly.position = position;
                        e.camera_fly.front = Vec3::Z;
                        e.camera_fly.right = Vec3::X;
                    }
                }

                ef
            },
            maps,
        }
    }

    fn populate_lighting_triangles(
        maps: &mut [ProcessedMap],
        ref_entities: &[IdentifiableResult<(EXGeoEntity, ProcessedEntityMesh)>],
    ) {
        for map in maps {
            map.lighting_triangles.clear();
            for (zone_index, zone_entity) in map.mapzone_entities.iter().enumerate() {
                let Some(Ok((_, mesh))) = ref_entities
                    .iter()
                    .find(|entry| entry.hashcode == zone_entity.entity_refptr)
                    .map(|entry| entry.data.as_ref())
                else {
                    continue;
                };

                for strip in mesh.strips.iter().filter(|strip| !strip.is_navmesh) {
                    let start = strip.start_index as usize;
                    let count = strip.index_count as usize;
                    let Some(indices) = mesh.indices.get(start..start.saturating_add(count)) else {
                        continue;
                    };
                    for triangle_index in 0..indices.len().saturating_sub(2) {
                        let mut tri_indices = [
                            indices[triangle_index] as usize,
                            indices[triangle_index + 1] as usize,
                            indices[triangle_index + 2] as usize,
                        ];
                        if triangle_index & 1 != 0 {
                            tri_indices.swap(0, 1);
                        }
                        if tri_indices[0] == tri_indices[1]
                            || tri_indices[1] == tri_indices[2]
                            || tri_indices[0] == tri_indices[2]
                        {
                            continue;
                        }
                        let Some(a) = mesh.vertex_data.get(tri_indices[0]) else {
                            continue;
                        };
                        let Some(b) = mesh.vertex_data.get(tri_indices[1]) else {
                            continue;
                        };
                        let Some(c) = mesh.vertex_data.get(tri_indices[2]) else {
                            continue;
                        };
                        map.lighting_triangles.push(NativeLightingTriangle {
                            positions: [Vec3::from(a.pos), Vec3::from(b.pos), Vec3::from(c.pos)],
                            colours: [
                                Vec4::from(a.color),
                                Vec4::from(b.color),
                                Vec4::from(c.color),
                            ],
                            zone_index,
                        });
                    }
                }
            }
        }
    }

    fn load_map_meshes(
        file: Hashcode,
        gl: &glow::Context,
        maps: &[ProcessedMap],
        ref_entities: &[IdentifiableResult<(EXGeoEntity, ProcessedEntityMesh)>],
        platform: Platform,
    ) -> Vec<(u32, Arc<Mutex<EntityRenderer>>)> {
        let mut ref_renderers = vec![];

        // FIXME(cohae): Map picking is a bit dirty at the moment
        for map in maps.iter() {
            for (zone_index, v) in map.mapzone_entities.iter().enumerate() {
                if let Some(Ok((_, e))) = &ref_entities
                    .iter()
                    .find(|ir| ir.hashcode == v.entity_refptr)
                    .map(|v| v.data.as_ref())
                {
                    let r = Arc::new(Mutex::new(EntityRenderer::new(file, platform)));
                    {
                        let mut renderer = r.lock();
                        renderer.native_light_zone = Some(zone_index);
                        renderer.native_light_sample_position =
                            map.zones.get(zone_index).map(|zone| {
                                let a = Vec3::from(zone.bounds_box[0]);
                                let b = Vec3::from(zone.bounds_box[1]);
                                (a + b) * 0.5
                            });
                        unsafe {
                            renderer.load_mesh(gl, e);
                        }
                    }
                    ref_renderers.push((map.hashcode, r));
                } else {
                    error!(
                        "Couldn't find ref entity #{} for mapzone entity!",
                        v.entity_refptr
                    );
                }
            }
        }

        ref_renderers
    }

    pub fn show(&mut self, context: &egui::Context, ui: &mut egui::Ui) -> anyhow::Result<()> {
        self.frame.show(ui, context, &self.maps)
    }
}

pub fn read_from_file(edb: &mut EdbFile) -> Vec<ProcessedMap> {
    let header = edb.header.clone();

    let mut maps = vec![];
    for m in header.map_list.iter() {
        edb.seek(std::io::SeekFrom::Start(m.address as u64))
            .unwrap();

        let xmap = edb
            .read_type_args::<EXGeoMap>(edb.endian, (header.version,))
            .context("Failed to read map")
            .unwrap();

        let mut map = ProcessedMap {
            hashcode: m.hashcode,
            mapzone_entities: vec![],
            placements: xmap.placements.data().clone(),
            placement_group_count: xmap.placement_groups.serialized_len(),
            cameras: xmap
                .cameras
                .iter()
                .map(|camera| ProcessedCamera {
                    hashcode: camera.hashcode,
                    position: camera.position.into(),
                    flags: camera.flags,
                    look: camera.look.into(),
                    focal_length: camera.focal_length,
                    aperture_width: camera.aperture_width,
                    aperture_height: camera.aperture_height,
                })
                .collect(),
            portals: xmap
                .portals
                .iter()
                .map(|portal| ProcessedPortal {
                    map_a: portal.map_a,
                    map_b: portal.map_b,
                    flags: portal.flags,
                    distance: portal.distance,
                    vertices: portal.vertices.map(Vec3::from),
                    face_common: portal.portal_face.common,
                    face_texture_ref: portal.portal_face.texture_ref,
                    face_flags: portal.portal_face.flags,
                    face_vertices: portal
                        .portal_face
                        .vertices
                        .iter()
                        .map(|vertex| Vec3::new(vertex.v[0], vertex.v[1], vertex.v[2]))
                        .collect(),
                })
                .collect(),
            isounds: xmap.isounds.data().clone(),
            lights: xmap
                .lights
                .iter()
                .map(|light| ProcessedLight {
                    hashcode: light.hashcode,
                    position: light.position.into(),
                    flags: light.flags,
                    beam: light.beam.into(),
                    light_type: light.ltype,
                    beam_angle: light.beam_angle,
                    colour: light.colour,
                    radius: light.radius,
                    max_effect_fraction: light.max_effect_fraction,
                })
                .collect(),
            sounds: xmap
                .sounds
                .iter()
                .map(|sound| ProcessedSound {
                    hashcode: sound.hashcode,
                    position: sound.position.into(),
                    flags: sound.flags,
                    sound_ref: sound.sound_ref,
                    color: sound.color,
                    volume: sound.volume,
                    fade_in: sound.fade_in,
                    fade_out: sound.fade_out,
                    tracking_type: sound.tracking_type,
                    inner_radius: sound.inner_radius,
                    outer_radius: sound.outer_radius,
                    base_map_on: sound.base_map_on,
                })
                .collect(),
            lighting_triangles: Vec::new(),
            paths: xmap
                .paths
                .iter()
                .map(|path| ProcessedPath {
                    hashcode: path.hashcode,
                    position: path.position.into(),
                    flags: path.flags,
                    path_type: path.ptype,
                    nodes: path
                        .nodes
                        .iter()
                        .map(|node| ProcessedPathNode {
                            position: node.position.into(),
                            size: node.size.into(),
                            value: node.value,
                            flags: node.flags,
                            distance: node.distance,
                            num_links: node.num_links,
                        })
                        .collect(),
                    links: path
                        .links
                        .iter()
                        .map(|link| (link.node_a as usize, link.node_b as usize))
                        .collect(),
                })
                .collect(),
            triggers: vec![],
            trigger_collisions: xmap.trigger_header.trigger_collisions.0.clone(),
            skies: xmap.skies.iter().map(|s| s.hashcode).collect(),
            zones: vec![],
        };

        for z in &xmap.zones {
            let entity_offset = header.refpointer_list[z.entity_refptr as usize].address;
            edb.seek(std::io::SeekFrom::Start(entity_offset as u64))
                .context("Mapzone refptr pointer to a non-entity object!")
                .unwrap();

            let ent = edb
                .read_type_args::<EXGeoEntity>(edb.endian, (header.version, edb.platform))
                .unwrap();

            if let EXGeoEntity::MapZone(mapzone) = ent {
                map.mapzone_entities.push(mapzone);
            } else {
                error!("Refptr entity does not have a mapzone entity!");
                // Result::<()>::Err(anyhow::anyhow!(
                //     "Refptr entity does not have a mapzone entity!"
                // ))
                // .unwrap();
            }
        }

        map.zones = xmap.zones;

        for t in xmap.trigger_header.triggers.iter() {
            let trig = &t.trigger;
            let (ttype, tsubtype) = {
                let t = &xmap.trigger_header.trigger_types[trig.type_index as usize];

                (t.trig_type, t.trig_subtype)
            };

            // Trigger-only pickups do not serialize visual_object/file. Resolve the
            // native pickup Script/entity and preserve local 0x82 objects in the
            // namespace of their explicitly named external EDB.
            if let Some(pickup) = robots_pickup_visual(ttype, &trig.data) {
                edb.add_external_reference(pickup.file, pickup.object);
            }

            let trigger_script = trig.engine_options.gamescript_index.and_then(|index| {
                xmap.trigger_header
                    .trigger_scripts
                    .get(index as usize)
                    .map(|(script, aux)| ProcessedTriggerScript {
                        file_offset: script.offset_absolute(),
                        aux: *aux,
                    })
            });

            let trigger = ProcessedTrigger {
                file_offset: t.trigger.offset_absolute(),
                link_ref: t.link_ref,
                type_index: trig.type_index,
                ttype,
                tsubtype: if tsubtype != 0 && tsubtype != 0x42000001 {
                    Some(tsubtype)
                } else {
                    None
                },
                debug: trig.debug,
                game_flags: trig.game_flags,
                trig_flags: trig.trig_flags,
                position: trig.position.into(),
                rotation: trig.rotation.into(),
                scale: trig.scale.into(),
                engine_options: trig.engine_options.clone(),
                trigger_script,
                character_visual: None,
                data: trig.data.to_vec(),
                links: trig.links.to_vec(),
                incoming_links: vec![],
            };

            map.triggers.push(trigger);
        }

        for i in 0..map.triggers.len() {
            for ei in 0..map.triggers.len() {
                if i == ei {
                    continue;
                }

                if map.triggers[ei].links.iter().any(|v| *v == i as i32) {
                    map.triggers[i].incoming_links.push(ei as i32);
                }
            }
        }

        maps.push(map);
    }

    maps
}

#[cfg(test)]
mod tests {
    use super::{
        map_editor_start_position, read_from_file, robots_camera_controller_plan,
        robots_camera_flags, robots_camera_marker_scaled_data0, robots_camera_mode,
        robots_camera_scaled_data4, robots_camera_scaled_data5, robots_monster_data15_value,
        robots_monster_data4_value, robots_monster_flags, robots_monster_is_family,
        robots_monster_proximity_radius, robots_monster_runtime_selector,
        robots_monster_test_runtime_value, robots_monster_transporter_secondary_path_hash,
        robots_native_light_colour, robots_native_light_type_description,
        robots_npc_alternate_cutscenes, robots_npc_cutscene_is_null, robots_npc_flags,
        robots_npc_runtime_selector, robots_npc_runtime_uid, robots_npc_text_group,
        robots_portal_neighbor_zone, robots_trigger_path_data_slot, robots_trigger_path_hash,
        robots_trigger_path_is_proven, robots_trigger_platform_angular_velocity,
        robots_trigger_runtime_path_acceleration, robots_trigger_runtime_path_speed,
        robots_watchbot_enter_distance, robots_watchbot_flags, robots_watchbot_leave_distance,
        robots_watchbot_mode, ProcessedPortal,
    };
    use eurochef_edb::{
        binrw::BinReaderExt, edb::EdbFile, entity::EXGeoEntity, script::EXGeoAnimScript,
        versions::Platform, HashcodeUtils,
    };
    use eurochef_shared::script::{UXGeoScript, UXGeoScriptCommandData};
    use glam::Vec3;
    use std::{
        fs::File,
        io::{BufReader, Seek},
        path::{Path, PathBuf},
    };

    fn format_mesh_diagnostics(mesh: &crate::entities::ProcessedEntityMesh) -> String {
        let strips = mesh
            .strips
            .iter()
            .enumerate()
            .map(|(index, strip)| {
                format!(
                    "#{index}:indices={} triangles={} texture={} transparency=0x{:04X} flags=0x{:04X} navmesh={}",
                    strip.index_count,
                    strip.tri_count,
                    strip.texture_index,
                    strip.transparency,
                    strip.flags,
                    strip.is_navmesh,
                )
            })
            .collect::<Vec<_>>()
            .join(";");
        format!(
            "vertices={} indices={} strips={} entity_flags=0x{:08X} strip_data=[{}]",
            mesh.vertex_data.len(),
            mesh.indices.len(),
            mesh.strips.len(),
            mesh.flags,
            strips,
        )
    }

    fn audio_manifest_edb_paths(manifest_path: &str, manifest: &str) -> Vec<PathBuf> {
        let mut lines = manifest.lines();
        let header = lines
            .next()
            .expect("real audio manifest does not contain a header")
            .split('\t')
            .collect::<Vec<_>>();
        let path_column = ["source_path", "physical_path", "path", "file_name"]
            .into_iter()
            .find_map(|name| header.iter().position(|column| *column == name))
            .expect("real audio manifest has no source_path/path/file_name column");
        let relative_root = std::env::var_os("EUROCHEF_REAL_AUDIO_EDB_ROOT")
            .map(PathBuf::from)
            .or_else(|| {
                Path::new(manifest_path)
                    .ancestors()
                    .find(|ancestor| {
                        ancestor
                            .file_name()
                            .and_then(|name| name.to_str())
                            .is_some_and(|name| name.eq_ignore_ascii_case("_eurotools_out"))
                    })
                    .map(|eurotools_out| eurotools_out.join("extracted_main/robots/binary/_bin_pc"))
            });

        lines
            .filter_map(|line| {
                let value = line.split('\t').nth(path_column)?.trim();
                if value.is_empty() {
                    return None;
                }
                let path = PathBuf::from(value);
                Some(if path.is_absolute() {
                    path
                } else {
                    relative_root
                        .as_ref()
                        .expect("relative manifest paths require EUROCHEF_REAL_AUDIO_EDB_ROOT")
                        .join(path)
                })
            })
            .collect()
    }

    #[test]
    fn robots_platform_and_lift_use_the_runtime_proven_path_slots() {
        let mut data = vec![None; 16];
        data[2] = Some(0x0B00_0014);
        assert_eq!(robots_trigger_path_hash(8, &data), Some(0x0B00_0014));

        data[2] = None;
        data[1] = Some(0x0B00_0037);
        assert_eq!(robots_trigger_path_hash(37, &data), Some(0x0B00_0037));
        assert_eq!(robots_trigger_path_hash(80, &data), Some(0x0B00_0037));
    }

    #[test]
    fn robots_camera_and_marker_use_runtime_proven_path_slots() {
        let mut camera = vec![None; 16];
        camera[0] = Some(4);
        camera[1] = Some(0x0B00_002E);
        camera[2] = Some(0x0000_8008);
        camera[4] = Some(40);
        camera[5] = Some(45);
        assert_eq!(robots_trigger_path_hash(1, &camera), Some(0x0B00_002E));
        assert_eq!(robots_trigger_path_data_slot(1), Some(1));
        assert_eq!(robots_camera_mode(1, &camera), Some(4));
        assert_eq!(robots_camera_scaled_data4(1, &camera), Some(4.0));
        assert_eq!(robots_camera_scaled_data5(1, &camera), Some(4.5));
        assert_eq!(robots_camera_flags(1, &camera), Some(0x0000_8008));
        assert_eq!(robots_trigger_runtime_path_speed(1, &camera), None);

        let mut marker = vec![None; 16];
        marker[0] = Some((-30i32) as u32);
        marker[2] = Some(0x0000_8002);
        marker[4] = Some(0x0B00_0053);
        assert_eq!(robots_trigger_path_hash(20, &marker), Some(0x0B00_0053));
        assert_eq!(robots_trigger_path_data_slot(20), Some(4));
        assert_eq!(robots_camera_marker_scaled_data0(20, &marker), Some(-3.0));
        assert_eq!(robots_camera_flags(20, &marker), Some(0x0000_8002));
        assert_eq!(robots_trigger_runtime_path_speed(20, &marker), None);
    }

    #[test]
    fn portal_endpoints_form_bidirectional_zone_adjacency() {
        let portal = ProcessedPortal {
            map_a: 2,
            map_b: 7,
            flags: 0,
            distance: 1.0,
            vertices: [Vec3::ZERO; 4],
            face_common: 0,
            face_texture_ref: 0,
            face_flags: 0,
            face_vertices: vec![],
        };
        assert_eq!(robots_portal_neighbor_zone(&portal, 2, 8), Some(7));
        assert_eq!(robots_portal_neighbor_zone(&portal, 7, 8), Some(2));
        assert_eq!(robots_portal_neighbor_zone(&portal, 3, 8), None);
        assert_eq!(robots_portal_neighbor_zone(&portal, 2, 7), None);
    }

    #[test]
    fn robots_npc_uses_runtime_proven_getters_and_alternate_cutscene_slots() {
        let data = [
            Some(7),
            Some(0x0B00_0000),
            Some(0x0000_0340),
            Some(0x4508_0005),
            Some(0x0400_00E5),
            Some(0x0400_00E6),
            Some(0x0400_0000),
            Some(0x0400_00E8),
        ];
        assert_eq!(robots_npc_runtime_selector(48, &data), Some(7));
        assert_eq!(robots_npc_runtime_uid(48, &data), Some(0x0B00_0000));
        assert_eq!(robots_npc_flags(48, &data), Some(0x340));
        assert_eq!(robots_npc_text_group(48, &data), Some(0x4508_0005));
        assert_eq!(
            robots_npc_alternate_cutscenes(48, &data),
            Some([
                Some(0x0400_00E5),
                Some(0x0400_00E6),
                Some(0x0400_0000),
                Some(0x0400_00E8),
            ])
        );
        assert!(robots_npc_cutscene_is_null(0x0400_0000));
        assert!(!robots_npc_cutscene_is_null(0x0400_00E5));
        assert_eq!(robots_npc_runtime_selector(47, &data), None);
    }

    #[test]
    fn robots_watchbot_uses_runtime_proven_path_and_hysteresis_slots() {
        let mut watchbot = vec![None; 16];
        watchbot[0] = Some(3);
        watchbot[1] = Some(0x0B00_0035);
        watchbot[2] = Some(3);
        watchbot[3] = Some(20);
        watchbot[4] = Some(30);

        assert_eq!(robots_trigger_path_hash(60, &watchbot), Some(0x0B00_0035));
        assert_eq!(robots_trigger_path_data_slot(60), Some(1));
        assert_eq!(robots_watchbot_mode(60, &watchbot), Some(3));
        assert_eq!(robots_watchbot_flags(60, &watchbot), Some(3));
        assert_eq!(robots_watchbot_enter_distance(60, &watchbot), Some(2.0));
        assert_eq!(robots_watchbot_leave_distance(60, &watchbot), Some(3.0));
        assert_eq!(robots_trigger_runtime_path_speed(60, &watchbot), None);
    }

    #[test]
    fn robots_ratchet_and_transporter_use_runtime_proven_path_context_slots() {
        let mut ratchet = vec![None; 16];
        ratchet[0] = Some(0x0B00_001F);
        assert_eq!(robots_trigger_path_hash(72, &ratchet), Some(0x0B00_001F));
        assert_eq!(robots_trigger_path_data_slot(72), Some(0));
        assert!(robots_trigger_path_is_proven(72, &ratchet, 0x0B00_001F));
        assert_eq!(robots_trigger_runtime_path_speed(72, &ratchet), None);

        let mut transporter = vec![None; 16];
        transporter[1] = Some(0x0B00_0094);
        transporter[4] = Some(0x0B00_0093);
        assert_eq!(
            robots_trigger_path_hash(73, &transporter),
            Some(0x0B00_0094)
        );
        assert_eq!(robots_trigger_path_data_slot(73), Some(1));
        assert_eq!(
            robots_monster_transporter_secondary_path_hash(73, &transporter),
            Some(0x0B00_0093)
        );
        assert!(robots_trigger_path_is_proven(73, &transporter, 0x0B00_0094));
        assert!(robots_trigger_path_is_proven(73, &transporter, 0x0B00_0093));
        assert_eq!(robots_trigger_runtime_path_speed(73, &transporter), None);

        transporter[4] = Some(0x0B00_0000);
        assert_eq!(
            robots_monster_transporter_secondary_path_hash(73, &transporter),
            None
        );
    }

    #[test]
    fn robots_monster_family_uses_runtime_proven_getters() {
        let mut monster = vec![None; 16];
        monster[0] = Some(7);
        monster[1] = Some(25);
        monster[2] = Some(0x0B00_003C);
        monster[4] = Some(4);
        monster[7] = Some(0xC000);
        monster[15] = Some(9);

        assert!(robots_monster_is_family(10));
        assert!(robots_monster_is_family(3));
        assert!(robots_monster_is_family(70));
        assert!(!robots_monster_is_family(48));
        assert_eq!(robots_monster_runtime_selector(10, &monster), Some(7));
        assert_eq!(robots_monster_proximity_radius(10, &monster), Some(2.5));
        assert_eq!(robots_trigger_path_hash(10, &monster), Some(0x0B00_003C));
        assert_eq!(robots_trigger_path_data_slot(10), Some(2));
        assert_eq!(robots_monster_data4_value(10, &monster), Some(4));
        assert_eq!(robots_monster_flags(10, &monster), Some(0xC000));
        assert_eq!(robots_monster_data15_value(10, &monster), Some(9));

        assert_eq!(robots_monster_test_runtime_value(3, &monster), Some(25));
        assert_eq!(robots_monster_proximity_radius(3, &monster), None);
        assert_eq!(robots_trigger_path_hash(3, &monster), None);
        assert_eq!(robots_trigger_path_data_slot(3), None);
    }

    #[test]
    fn robots_monster_path_is_proven_but_boss_sewer_path_like_value_is_not() {
        let mut monster = vec![None; 16];
        monster[2] = Some(0x0B00_003C);
        assert_eq!(robots_trigger_path_hash(74, &monster), Some(0x0B00_003C));
        assert_eq!(robots_trigger_path_data_slot(74), Some(2));
        assert!(robots_trigger_path_is_proven(74, &monster, 0x0B00_003C));
        assert_eq!(robots_trigger_runtime_path_speed(74, &monster), None);

        let mut boss_sewer = vec![None; 16];
        boss_sewer[0] = Some(0x0B00_0059);
        assert_eq!(robots_trigger_path_hash(75, &boss_sewer), None);
        assert_eq!(robots_trigger_path_data_slot(75), None);
        assert!(!robots_trigger_path_is_proven(75, &boss_sewer, 0x0B00_0059));
    }

    #[test]
    fn robots_runtime_path_sentinels_are_not_treated_as_paths() {
        for sentinel in [0, u32::MAX, 0x0B00_0000] {
            let mut data = vec![None; 16];
            data[2] = Some(sentinel);
            assert_eq!(robots_trigger_path_hash(8, &data), None);
        }
    }

    #[test]
    fn robots_runtime_path_speed_uses_each_controller_serialized_field() {
        let mut lift = vec![None; 16];
        lift[3] = Some(4.0f32.to_bits());
        lift[4] = Some(30);
        assert_eq!(robots_trigger_runtime_path_speed(37, &lift), Some(3.0));
        assert_eq!(
            robots_trigger_runtime_path_acceleration(37, &lift),
            Some(0.4)
        );

        let mut vehicle = vec![None; 16];
        vehicle[2] = Some(70.0f32.to_bits());
        assert_eq!(robots_trigger_runtime_path_speed(80, &vehicle), Some(7.0));

        let mut platform = vec![None; 16];
        platform[5] = Some(25.0f32.to_bits());
        platform[6] = Some(2.0f32.to_bits());
        assert_eq!(robots_trigger_runtime_path_speed(8, &platform), Some(2.5));
        assert_eq!(
            robots_trigger_runtime_path_acceleration(8, &platform),
            Some(0.2)
        );

        assert_eq!(
            robots_trigger_runtime_path_speed(37, &[None; 16]),
            Some(1.0)
        );
    }

    #[test]
    fn robots_platform_rotation_uses_runtime_proven_serialized_slots() {
        let mut data = vec![None; 16];
        data[1] = Some((-5.0f32).to_bits());
        data[3] = Some(10.0f32.to_bits());
        data[4] = Some(2.5f32.to_bits());

        assert_eq!(
            robots_trigger_platform_angular_velocity(8, &data),
            Some(Vec3::new(10.0, 2.5, -5.0))
        );
        assert_eq!(robots_trigger_platform_angular_velocity(37, &data), None);
    }

    #[test]
    fn robots_native_light_types_are_decoded_as_feature_masks() {
        assert_eq!(robots_native_light_type_description(1), "range");
        assert_eq!(
            robots_native_light_type_description(3),
            "range + position-normal"
        );
        assert_eq!(robots_native_light_type_description(5), "range + beam-cone");
        assert_eq!(
            robots_native_light_type_description(7),
            "range + position-normal + beam-cone"
        );
        assert_eq!(
            robots_native_light_type_description(11),
            "range + position-normal + beam-normal"
        );
        assert_eq!(
            robots_native_light_type_description(0x21),
            "range + unknown-bits + 0x0020"
        );
    }

    #[test]
    fn robots_native_light_colour_uses_the_runtime_one_over_128_scale() {
        let colour = robots_native_light_colour([0x80, 0x40, 0x20, 0xff]);
        assert_eq!(colour, Vec3::new(1.0, 0.5, 0.25));

        let full_red = robots_native_light_colour([0xff, 0, 0, 0xff]);
        assert!((full_red.x - 255.0 / 128.0).abs() < f32::EPSILON);
        assert_eq!(full_red.y, 0.0);
        assert_eq!(full_red.z, 0.0);
    }

    #[test]
    fn real_audio_corpus_when_fixture_is_requested() {
        let Ok(path) = std::env::var("EUROCHEF_REAL_AUDIO_EDB") else {
            eprintln!("SKIP real_audio_corpus_when_fixture_is_requested: EUROCHEF_REAL_AUDIO_EDB is not set");
            return;
        };

        let open_edb = || {
            let file = File::open(&path).expect("real audio EDB fixture is missing");
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
                .expect("real audio EDB fixture is not a valid PC EDB")
        };

        let mut map_edb = open_edb();
        let maps = read_from_file(&mut map_edb);
        assert!(!maps.is_empty(), "real audio EDB has no maps");

        let map_sound_count = maps.iter().map(|map| map.sounds.len()).sum::<usize>();
        let spatial_sound_count = maps
            .iter()
            .flat_map(|map| &map.sounds)
            .filter(|sound| sound.outer_radius > 0.0)
            .count();
        let tracking_type_counts = maps.iter().flat_map(|map| &map.sounds).fold(
            std::collections::BTreeMap::<u8, usize>::new(),
            |mut counts, sound| {
                *counts.entry(sound.tracking_type).or_default() += 1;
                counts
            },
        );
        let zone_sound_references = maps
            .iter()
            .flat_map(|map| &map.zones)
            .map(|zone| zone.sound_array.len())
            .sum::<usize>();
        let invalid_zone_sound_indices = maps
            .iter()
            .map(|map| {
                map.zones
                    .iter()
                    .flat_map(|zone| zone.sound_array.iter())
                    .filter(|index| **index as usize >= map.sounds.len())
                    .count()
            })
            .sum::<usize>();

        let mut script_edb = open_edb();
        let scripts = UXGeoScript::read_all(&mut script_edb).expect("could not read real scripts");
        let script_sound_commands = scripts
            .iter()
            .flat_map(|script| &script.commands)
            .filter(|command| matches!(command.data, UXGeoScriptCommandData::Sound { .. }))
            .count();
        let referenced_sound_hashes = maps
            .iter()
            .flat_map(|map| &map.sounds)
            .map(|sound| sound.sound_ref)
            .chain(
                scripts
                    .iter()
                    .flat_map(|script| &script.commands)
                    .filter_map(|command| match &command.data {
                        UXGeoScriptCommandData::Sound { hashcode } => Some(*hashcode),
                        _ => None,
                    }),
            )
            .collect::<std::collections::BTreeSet<_>>();
        let referenced_sound_hash_list = referenced_sound_hashes
            .iter()
            .map(|hashcode| format!("0x{hashcode:08x}"))
            .collect::<Vec<_>>()
            .join(",");

        assert!(map_sound_count > 0, "real map has no EXGeoSound emitters");
        assert!(
            zone_sound_references > 0,
            "real MapZone data has no sound references"
        );
        assert_eq!(
            invalid_zone_sound_indices, 0,
            "MapZone.sound_array contains invalid indices"
        );
        assert!(
            script_sound_commands > 0,
            "real scripts contain no Sound commands"
        );

        let report = format!(
            "key\tvalue\nmaps\t{}\nmap_sounds\t{}\nspatial_sounds_by_radius\t{}\nzone_sound_references\t{}\ntracking_type_counts\t{:?}\ninvalid_zone_sound_indices\t{}\nscripts\t{}\nscript_sound_commands\t{}\nunique_referenced_sound_hashes\t{}\nreferenced_sound_hashes\t{}\n",
            maps.len(),
            map_sound_count,
            spatial_sound_count,
            zone_sound_references,
            tracking_type_counts,
            invalid_zone_sound_indices,
            scripts.len(),
            script_sound_commands,
            referenced_sound_hashes.len(),
            referenced_sound_hash_list,
        );
        eprintln!("{report}");
        if let Ok(output) = std::env::var("EUROCHEF_REAL_AUDIO_REPORT") {
            if let Some(parent) = std::path::Path::new(&output).parent() {
                std::fs::create_dir_all(parent).expect("could not create audio report folder");
            }
            std::fs::write(output, report).expect("could not write real audio report");
        }
    }

    #[test]
    fn real_audio_manifest_corpus_when_requested() {
        let Ok(manifest_path) = std::env::var("EUROCHEF_REAL_AUDIO_MANIFEST") else {
            eprintln!("SKIP real_audio_manifest_corpus_when_requested: EUROCHEF_REAL_AUDIO_MANIFEST is not set");
            return;
        };
        let manifest =
            std::fs::read_to_string(&manifest_path).expect("real audio manifest could not be read");
        let mut references = std::collections::BTreeSet::<u32>::new();
        let mut edb_count = 0usize;
        let mut map_count = 0usize;
        let mut map_sound_count = 0usize;
        let mut zone_sound_reference_count = 0usize;
        let mut script_count = 0usize;
        let mut script_sound_command_count = 0usize;
        let mut failures = Vec::<String>::new();

        for source_path in audio_manifest_edb_paths(&manifest_path, &manifest) {
            edb_count += 1;

            let open_edb = || -> Result<EdbFile, String> {
                let file = File::open(&source_path)
                    .map_err(|error| format!("{}: {error}", source_path.display()))?;
                EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
                    .map_err(|error| format!("{}: {error}", source_path.display()))
            };

            match open_edb() {
                Ok(mut edb) => {
                    let maps = read_from_file(&mut edb);
                    map_count += maps.len();
                    for map in maps {
                        map_sound_count += map.sounds.len();
                        references.extend(map.sounds.iter().map(|sound| sound.sound_ref));
                        zone_sound_reference_count += map
                            .zones
                            .iter()
                            .map(|zone| zone.sound_array.len())
                            .sum::<usize>();
                    }
                }
                Err(error) => failures.push(error),
            }

            match open_edb() {
                Ok(mut edb) => match UXGeoScript::read_all(&mut edb) {
                    Ok(scripts) => {
                        script_count += scripts.len();
                        for command in scripts.iter().flat_map(|script| &script.commands) {
                            if let UXGeoScriptCommandData::Sound { hashcode } = command.data {
                                script_sound_command_count += 1;
                                references.insert(hashcode);
                            }
                        }
                    }
                    Err(error) => {
                        failures.push(format!("{} scripts: {error}", source_path.display()))
                    }
                },
                Err(error) => failures.push(error),
            }
        }

        assert_eq!(edb_count, 179, "unexpected Robots manifest EDB count");
        assert!(map_count > 0, "manifest corpus has no maps");
        assert!(map_sound_count > 0, "manifest corpus has no map sounds");
        assert!(
            script_sound_command_count > 0,
            "manifest corpus has no script sounds"
        );
        assert!(
            failures.is_empty(),
            "manifest audio parse failures: {failures:?}"
        );

        let reference_list = references
            .iter()
            .map(|hashcode| format!("0x{hashcode:08x}"))
            .collect::<Vec<_>>()
            .join(",");
        let report = format!(
            "key\tvalue\nedbs\t{}\nmaps\t{}\nmap_sounds\t{}\nzone_sound_references\t{}\nscripts\t{}\nscript_sound_commands\t{}\nunique_audio_references\t{}\naudio_references\t{}\nparse_failures\t{}\n",
            edb_count,
            map_count,
            map_sound_count,
            zone_sound_reference_count,
            script_count,
            script_sound_command_count,
            references.len(),
            reference_list,
            failures.len(),
        );
        eprintln!("{report}");
        if let Ok(output) = std::env::var("EUROCHEF_REAL_AUDIO_MANIFEST_REPORT") {
            if let Some(parent) = std::path::Path::new(&output).parent() {
                std::fs::create_dir_all(parent)
                    .expect("could not create manifest audio report folder");
            }
            std::fs::write(output, report).expect("could not write manifest audio report");
        }
    }

    #[test]
    fn real_map_runtime_sections_manifest_when_requested() {
        let Ok(source) = std::env::var("EUROCHEF_REAL_MAP_RUNTIME_MANIFEST") else {
            return;
        };
        let source = PathBuf::from(source);
        let source_paths = if source.is_dir() {
            let mut pending = vec![source.clone()];
            let mut paths = std::collections::BTreeSet::new();
            while let Some(directory) = pending.pop() {
                for entry in std::fs::read_dir(&directory)
                    .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
                {
                    let entry = entry.expect("map runtime corpus directory entry is invalid");
                    let path = entry.path();
                    if path.is_dir() {
                        pending.push(path);
                    } else if path
                        .extension()
                        .and_then(|extension| extension.to_str())
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("edb"))
                    {
                        paths.insert(path);
                    }
                }
            }
            paths
        } else {
            let manifest = std::fs::read_to_string(&source)
                .expect("real map runtime manifest could not be read");
            let manifest_base = source.parent().unwrap_or_else(|| Path::new("."));
            let mut manifest_lines = manifest.lines();
            let header = manifest_lines
                .next()
                .expect("real map runtime manifest has no header")
                .split('\t')
                .collect::<Vec<_>>();
            let path_column = header
                .iter()
                .position(|column| {
                    let normalized = column.trim().to_ascii_lowercase().replace(' ', "_");
                    matches!(
                        normalized.as_str(),
                        "source_edb"
                            | "source_path"
                            | "physical_path"
                            | "path"
                            | "file_name"
                            | "edb_path"
                    )
                })
                .expect("real map runtime manifest has no EDB path column");
            manifest_lines
                .filter_map(|line| {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        return None;
                    }
                    let source_text = line.split('\t').nth(path_column)?.trim();
                    if source_text.is_empty() {
                        return None;
                    }
                    let source_path = PathBuf::from(source_text);
                    Some(if source_path.is_absolute() {
                        source_path
                    } else {
                        manifest_base.join(source_path)
                    })
                })
                .collect::<std::collections::BTreeSet<_>>()
        };

        let mut edb_count = 0usize;
        let mut map_count = 0usize;
        let mut camera_count = 0usize;
        let mut portal_count = 0usize;
        let mut placement_group_count = 0usize;
        let mut isound_count = 0usize;
        let mut maps_with_cameras = 0usize;
        let mut maps_with_portals = 0usize;
        let mut trigger_camera_count = 0usize;
        let mut camera_controller_plan_count = 0usize;
        let mut camera_mode_counts = std::collections::BTreeMap::<u32, usize>::new();
        let mut camera_plans_with_marker = 0usize;
        let mut portal_endpoint_in_zone_range = 0usize;
        let mut portal_endpoint_outside_zone_range = 0usize;
        let mut portal_self_pairs = 0usize;
        let mut portal_endpoint_max = 0u16;
        let mut failures = Vec::<String>::new();

        for source_path in source_paths {
            edb_count += 1;
            let file = match File::open(&source_path) {
                Ok(file) => file,
                Err(error) => {
                    failures.push(format!("{}: {error}", source_path.display()));
                    continue;
                }
            };
            let mut edb = match EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc) {
                Ok(edb) => edb,
                Err(error) => {
                    failures.push(format!("{}: {error}", source_path.display()));
                    continue;
                }
            };
            let maps = read_from_file(&mut edb);
            map_count += maps.len();
            for map in maps {
                camera_count += map.cameras.len();
                portal_count += map.portals.len();
                placement_group_count += map.placement_group_count;
                isound_count += map.isounds.len();
                maps_with_cameras += usize::from(!map.cameras.is_empty());
                maps_with_portals += usize::from(!map.portals.is_empty());
                for (trigger_index, trigger) in map.triggers.iter().enumerate() {
                    if trigger.ttype != 1 {
                        continue;
                    }
                    trigger_camera_count += 1;
                    if let Some(plan) = robots_camera_controller_plan(&map, trigger_index) {
                        camera_controller_plan_count += 1;
                        *camera_mode_counts.entry(plan.mode).or_default() += 1;
                        camera_plans_with_marker += usize::from(plan.linked_marker_index.is_some());
                    }
                }
                for portal in &map.portals {
                    portal_endpoint_max = portal_endpoint_max.max(portal.map_a).max(portal.map_b);
                    portal_self_pairs += usize::from(portal.map_a == portal.map_b);
                    for endpoint in [portal.map_a, portal.map_b] {
                        if (endpoint as usize) < map.zones.len() {
                            portal_endpoint_in_zone_range += 1;
                        } else {
                            portal_endpoint_outside_zone_range += 1;
                        }
                    }
                }
                assert!(map.cameras.iter().all(|camera| {
                    camera.position.is_finite()
                        && camera.look.is_finite()
                        && camera.focal_length.is_finite()
                        && camera.aperture_width.is_finite()
                        && camera.aperture_height.is_finite()
                }));
                assert!(map.portals.iter().all(|portal| {
                    portal.distance.is_finite()
                        && portal.vertices.iter().all(|vertex| vertex.is_finite())
                        && portal.face_vertices.iter().all(|vertex| vertex.is_finite())
                }));
            }
        }

        assert_eq!(edb_count, 179, "unexpected Robots manifest EDB count");
        assert_eq!(map_count, 18, "unexpected Robots map count");
        assert_eq!(
            camera_count, 0,
            "shipped PC maps unexpectedly use EXGeoCamera"
        );
        assert_eq!(portal_count, 402, "unexpected EXGeoPortal count");
        assert_eq!(maps_with_portals, 12, "unexpected maps-with-portals count");
        assert_eq!(
            placement_group_count, 0,
            "shipped PC maps unexpectedly use placement groups"
        );
        assert_eq!(
            isound_count, 0,
            "shipped PC maps unexpectedly use EXGeoMap.isounds"
        );
        assert_eq!(trigger_camera_count, 45, "unexpected XTrigger_Camera count");
        assert_eq!(
            camera_controller_plan_count, trigger_camera_count,
            "every shipped Camera must produce a native controller plan"
        );
        assert_eq!(
            camera_mode_counts,
            std::collections::BTreeMap::from([(0, 5), (3, 22), (4, 18)]),
            "unexpected shipped Camera mode census"
        );
        assert_eq!(
            camera_plans_with_marker, 27,
            "unexpected Camera/Marker plan count"
        );
        assert_eq!(portal_endpoint_in_zone_range, portal_count * 2);
        assert_eq!(portal_endpoint_outside_zone_range, 0);
        assert_eq!(portal_self_pairs, 0);
        eprintln!(
            "camera plans: triggers={} plans={} modes={:?} with_marker={}; portal endpoints: in_zone_range={} outside_zone_range={} self_pairs={} max={}",
            trigger_camera_count,
            camera_controller_plan_count,
            camera_mode_counts,
            camera_plans_with_marker,
            portal_endpoint_in_zone_range,
            portal_endpoint_outside_zone_range,
            portal_self_pairs,
            portal_endpoint_max,
        );
        assert!(
            failures.is_empty(),
            "map runtime parse failures: {failures:?}"
        );
        eprintln!(
            "map runtime corpus: edbs={} maps={} cameras={} maps_with_cameras={} portals={} maps_with_portals={} placement_groups={} isounds={} failures={}",
            edb_count,
            map_count,
            camera_count,
            maps_with_cameras,
            portal_count,
            maps_with_portals,
            placement_group_count,
            isound_count,
            failures.len(),
        );
    }

    #[test]
    fn real_m02_city_map_runtime_sections_when_requested() {
        let Ok(path) = std::env::var("EUROCHEF_REAL_M02_CITY_EDB") else {
            return;
        };
        let file = File::open(&path).expect("m02_city fixture is missing");
        let mut edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
            .expect("m02_city fixture is not a valid PC EDB");
        let maps = read_from_file(&mut edb);
        let map = maps.first().expect("m02_city map is missing");
        eprintln!(
            "m02_city runtime sections: cameras={} portals={} placement_groups={} isounds={:?}",
            map.cameras.len(),
            map.portals.len(),
            map.placement_group_count,
            map.isounds,
        );
        assert!(map.cameras.iter().all(|camera| {
            camera.position.is_finite()
                && camera.look.is_finite()
                && camera.focal_length.is_finite()
                && camera.aperture_width.is_finite()
                && camera.aperture_height.is_finite()
        }));
        assert!(map.portals.iter().all(|portal| {
            portal.distance.is_finite()
                && portal.vertices.iter().all(|vertex| vertex.is_finite())
                && portal.face_vertices.iter().all(|vertex| vertex.is_finite())
        }));
    }

    #[test]
    fn real_m02_city_structural_audit_when_requested() {
        let Ok(path) = std::env::var("EUROCHEF_REAL_M02_CITY_EDB") else {
            return;
        };
        let open_edb = || {
            let file = File::open(&path).expect("m02_city fixture is missing");
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
                .expect("m02_city fixture is not a valid PC EDB")
        };

        let mut map_edb = open_edb();
        assert_eq!(map_edb.header.hashcode, 0x0100_001D);
        assert_eq!(map_edb.header.entity_list.len(), 171);
        let maps = read_from_file(&mut map_edb);
        assert_eq!(maps.len(), 1);
        let map = &maps[0];
        assert_eq!(map.triggers.len(), 726);

        let invalid_trigger_links = map
            .triggers
            .iter()
            .flat_map(|trigger| &trigger.links)
            .filter(|link| **link != -1 && (**link < 0 || **link as usize >= map.triggers.len()))
            .count();
        let invalid_path_links = map
            .paths
            .iter()
            .map(|path| {
                path.links
                    .iter()
                    .filter(|(from, to)| *from >= path.nodes.len() || *to >= path.nodes.len())
                    .count()
            })
            .sum::<usize>();
        assert_eq!(invalid_trigger_links, 0);
        assert_eq!(invalid_path_links, 0);

        let trigger_info: eurochef_shared::maps::TriggerInformation =
            serde_yaml::from_str(include_str!("../../../assets/triggers_robots.yml"))
                .expect("Robots trigger typemap did not parse");
        let missing_trigger_type_definitions = map
            .triggers
            .iter()
            .filter(|trigger| !trigger_info.triggers.contains_key(&trigger.ttype))
            .count();
        let mut non_null_trigger_values = 0usize;
        let mut named_trigger_values = 0usize;
        for trigger in &map.triggers {
            let definition = trigger_info
                .triggers
                .get(&trigger.ttype)
                .expect("m02_city trigger type is missing from the typemap");
            for (slot, value) in trigger.data.iter().enumerate() {
                if value.is_none() {
                    continue;
                }
                non_null_trigger_values += 1;
                if definition.values.contains_key(&(slot as u32)) {
                    named_trigger_values += 1;
                }
            }
        }
        assert_eq!(missing_trigger_type_definitions, 0);
        assert_eq!(non_null_trigger_values, 1821);
        let raw_only_trigger_values = non_null_trigger_values - named_trigger_values;

        let pickup_types = [
            0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x29, 0x3E, 0x3F, 0x40, 0x41, 0x43, 0x47, 0x52,
            0x53,
        ];
        let pickup_triggers = map
            .triggers
            .iter()
            .filter(|trigger| pickup_types.contains(&trigger.ttype))
            .collect::<Vec<_>>();
        let unresolved_pickups = pickup_triggers
            .iter()
            .filter(|trigger| super::robots_pickup_visual(trigger.ttype, &trigger.data).is_none())
            .count();
        assert_eq!(pickup_triggers.len(), 311);
        assert_eq!(unresolved_pickups, 0);

        let zero_entities = [0x8200_0026, 0x8200_0027, 0x8200_0058];
        let placement_zero_refs = zero_entities.map(|entity| {
            map.placements
                .iter()
                .filter(|placement| placement.object_ref == entity)
                .count()
        });
        assert_eq!(placement_zero_refs, [0, 0, 1]);
        let zero_entity_placements = map
            .placements
            .iter()
            .filter(|placement| zero_entities.contains(&placement.object_ref))
            .map(|placement| {
                format!(
                    "placement=0x{:08X} object=0x{:08X} pos={:?} rot={:?} scale={:?} flags=0x{:08X} engine_flags=0x{:04X} map_on=0x{:04X} light_set={} group={} unk=0x{:08X}",
                    placement.hashcode,
                    placement.object_ref,
                    placement.position,
                    placement.rotation,
                    placement.scale,
                    placement.flags,
                    placement.engine_flags,
                    placement.map_on,
                    placement.light_set,
                    placement.group,
                    placement.unk,
                )
            })
            .collect::<Vec<_>>();

        let mut entity_edb = open_edb();
        let entity_header = entity_edb.header.clone();
        let entity_endian = entity_edb.endian;
        let zero_geometry_counts = zero_entities.map(|entity| {
            let record = entity_header
                .entity_list
                .iter()
                .find(|record| record.common.hashcode == entity)
                .expect("zero-geometry entity header is missing");
            entity_edb
                .seek(std::io::SeekFrom::Start(record.common.address as u64))
                .expect("could not seek to zero-geometry entity");
            let parsed = entity_edb
                .read_type_args::<EXGeoEntity>(entity_endian, (entity_header.version, Platform::Pc))
                .expect("zero-geometry entity did not parse");
            let EXGeoEntity::Mesh(mesh) = parsed else {
                panic!("0x{entity:08X} is not a mesh entity");
            };
            (
                mesh.vertices.len(),
                mesh.indices.len(),
                mesh.tristrips.len(),
            )
        });
        assert_eq!(zero_geometry_counts, [(0, 0, 0); 3]);

        let mut script_edb = open_edb();
        let scripts =
            UXGeoScript::read_all(&mut script_edb).expect("m02_city scripts did not parse");
        assert_eq!(scripts.len(), 60);

        let expected_skies = [
            0x8400_0019,
            0x8400_0017,
            0x8400_0035,
            0x8400_0018,
            0x8400_0033,
            0x8400_0036,
        ];
        assert_eq!(map.skies.as_slice(), expected_skies.as_slice());
        let expected_zone_skies = [
            -1, 0, 0, 0, 1, 1, 1, 1, -1, 2, 1, 1, 3, -1, -1, 3, 4, 1, 1, 1, -1, -1, 5,
        ];
        let zone_skies = map
            .zones
            .iter()
            .map(|zone| zone.identifier.sky_index)
            .collect::<Vec<_>>();
        assert_eq!(zone_skies.as_slice(), expected_zone_skies.as_slice());
        assert!(map.zones.iter().all(|zone| {
            let min = zone.bounds_box[0];
            let max = zone.bounds_box[1];
            min.iter().chain(max.iter()).all(|value| value.is_finite())
                && min[0] <= max[0]
                && min[1] <= max[1]
                && min[2] <= max[2]
        }));
        let expected_zone0_bounds = [
            [82.593_84, -8.488_69, 98.871_7],
            [116.718_2, 6.008_4, 133.052_1],
        ];
        for (actual, expected) in map.zones[0]
            .bounds_box
            .iter()
            .flatten()
            .zip(expected_zone0_bounds.iter().flatten())
        {
            assert!(
                (*actual - *expected).abs() < 0.002,
                "City v248 zone-0 bounds shifted: actual={actual} expected={expected}"
            );
        }

        let editor_start = map_editor_start_position(map).expect("City editor start position");
        assert!(
            editor_start.distance(Vec3::new(200.076_77, 0.0, 88.114_5)) < 0.003,
            "unexpected City editor start position: {editor_start:?}"
        );
        let start_zone = crate::map_zone::robots_map_zone_index_by_bounds(
            map.zones.len(),
            editor_start,
            |index| {
                let a = Vec3::from(map.zones[index].bounds_box[0]);
                let b = Vec3::from(map.zones[index].bounds_box[1]);
                (a.min(b), a.max(b))
            },
        )
        .expect("City start MapZone");
        assert_eq!(start_zone, 6);
        assert_eq!(map.zones[start_zone].identifier.sky_index, 1);
        assert_eq!(map.skies[1], 0x8400_0017);

        let sky_script_contains = |script_hashcode, entity_hashcode| {
            scripts
                .iter()
                .find(|script| script.hashcode == script_hashcode)
                .expect("City sky Script is missing")
                .commands
                .iter()
                .any(|command| {
                    matches!(
                        &command.data,
                        UXGeoScriptCommandData::Entity { hashcode, .. }
                            if *hashcode == entity_hashcode
                    )
                })
        };
        assert!(sky_script_contains(0x8400_0017, 0x8200_002E));
        assert!(sky_script_contains(0x8400_0035, 0x8200_0098));

        let sky_zone_rows = map
            .zones
            .iter()
            .enumerate()
            .filter(|(_, zone)| matches!(zone.identifier.sky_index, 1 | 2))
            .map(|(index, zone)| {
                format!(
                    "zone={index} sky_index={} sky=0x{:08X} bounds={:?}",
                    zone.identifier.sky_index,
                    map.skies[zone.identifier.sky_index as usize],
                    zone.bounds_box,
                )
            })
            .collect::<Vec<_>>();
        assert!(
            sky_zone_rows.iter().any(|row| row.contains("sky_index=1")),
            "m02_city has no zone selecting 0x84000017 / 0x8200002E"
        );
        assert!(
            sky_zone_rows.iter().any(|row| row.contains("sky_index=2")),
            "m02_city has no zone selecting 0x84000035 / 0x82000098"
        );

        let script_zero_refs = zero_entities.map(|entity| {
            scripts
                .iter()
                .flat_map(|script| &script.commands)
                .filter(|command| {
                    matches!(
                        &command.data,
                        UXGeoScriptCommandData::Entity { hashcode, .. } if *hashcode == entity
                    )
                })
                .count()
        });
        assert_eq!(script_zero_refs, [2, 1, 0]);

        let report = format!(
            "uid=0x0100001D maps={} zones={} placements={} lights={} sounds={} paths={} triggers={} entities={} scripts={} pickups={} unresolved_pickups={} invalid_trigger_links={} invalid_path_links={} missing_trigger_type_definitions={} trigger_values_total={} trigger_values_named={} trigger_values_raw_only={} zero_entity_geometry_counts={:?} zero_entity_placement_refs={:?} zero_entity_script_refs={:?} zero_entity_placements={:?} sky_zone_rows={:?}",
            maps.len(),
            map.zones.len(),
            map.placements.len(),
            map.lights.len(),
            map.sounds.len(),
            map.paths.len(),
            map.triggers.len(),
            map_edb.header.entity_list.len(),
            scripts.len(),
            pickup_triggers.len(),
            unresolved_pickups,
            invalid_trigger_links,
            invalid_path_links,
            missing_trigger_type_definitions,
            non_null_trigger_values,
            named_trigger_values,
            raw_only_trigger_values,
            zero_geometry_counts,
            placement_zero_refs,
            script_zero_refs,
            zero_entity_placements,
            sky_zone_rows,
        );
        eprintln!("{report}");
        if let Ok(output) = std::env::var("EUROCHEF_REAL_M02_CITY_REPORT") {
            if let Some(parent) = std::path::Path::new(&output).parent() {
                std::fs::create_dir_all(parent)
                    .expect("could not create m02_city audit report folder");
            }
            std::fs::write(output, report).expect("could not write m02_city audit report");
        }
    }

    #[test]
    fn real_m02_city_geometry_usage_audit_when_requested() {
        let Ok(path) = std::env::var("EUROCHEF_REAL_M02_CITY_EDB") else {
            return;
        };
        let open_edb = || {
            let file = File::open(&path).expect("m02_city fixture is missing");
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
                .expect("m02_city fixture is not a valid PC EDB")
        };

        let mut map_edb = open_edb();
        let maps = read_from_file(&mut map_edb);
        let map = maps.first().expect("m02_city has no map");

        let mut entity_edb = open_edb();
        let (entities, _, ref_entities) = crate::entities::read_from_file(&mut entity_edb, None)
            .expect("m02_city entities did not parse");
        let mut script_edb = open_edb();
        let scripts =
            UXGeoScript::read_all(&mut script_edb).expect("m02_city scripts did not parse");
        let mut render_store = crate::render::RenderStore::new();
        for (entity_index, entry) in &entities {
            if let Ok((_, mesh)) = &entry.data {
                let mut renderer = crate::render::entity::EntityRenderer::new(
                    map_edb.header.hashcode,
                    Platform::Pc,
                );
                renderer.set_serialized_vertex_count_for_test(mesh.vertex_data.len());
                render_store.insert_entity(
                    map_edb.header.hashcode,
                    entry.hashcode,
                    *entity_index,
                    renderer,
                );
            }
        }
        for script in &scripts {
            render_store.insert_script(map_edb.header.hashcode, script.clone());
        }

        let selected = [
            0x0200_01B4,
            // User-reported City gears.
            0x8200_002D,
            0x8200_0046,
            0x8200_009E,
            0x8200_009F,
            0x8200_00A0,
            0x8200_00A1,
            0x8200_00A9,
            // User-reported missing or misplaced City entities.
            0x8200_0006,
            0x8200_0007,
            0x8200_000D,
            0x8200_0026,
            0x8200_0027,
            0x8200_002E,
            0x8200_002F,
            0x8200_0030,
            0x8200_0031,
            0x8200_0032,
            0x8200_0033,
            0x8200_0034,
            0x8200_0035,
            0x8200_0036,
            0x8200_0037,
            0x8200_0038,
            0x8200_0039,
            0x8200_003A,
            0x8200_003B,
            0x8200_003C,
            0x8200_003D,
            0x8200_003E,
            0x8200_0058,
            0x8200_0094,
            0x8200_0098,
            0x8200_0099,
            0x8200_009A,
            0x8200_009D,
            // Existing structural sentinels retained for regression coverage.
            0x8200_0047,
            0x8200_00A8,
            0x8200_00AA,
        ];
        let mut lines = Vec::<String>::new();

        lines.push("-- City static-script header census --".to_string());
        for script_hashcode in [
            0x8400_0017,
            0x8400_0018,
            0x8400_0019,
            0x8400_0033,
            0x8400_0035,
            0x8400_0036,
        ] {
            let header = map_edb
                .header
                .animscript_list
                .iter()
                .find(|header| header.hashcode == script_hashcode)
                .unwrap_or_else(|| panic!("0x{script_hashcode:08X} is missing from City headers"));
            let mut raw_edb = open_edb();
            raw_edb
                .seek(std::io::SeekFrom::Start(header.address as u64))
                .expect("could not seek to City Script header");
            let raw = raw_edb
                .read_type::<EXGeoAnimScript>(raw_edb.endian)
                .expect("could not parse City Script header");
            let processed = scripts
                .iter()
                .find(|script| script.hashcode == script_hashcode)
                .expect("processed City Script is missing");
            lines.push(format!(
                "script=0x{script_hashcode:08X}\taddress=0x{:08X}\tlength={}\tthreads={}\ttimejumps={}\tflags=0x{:04X}\tfps={}\tbounds={:?}\tunk30=0x{:08X}\tserialized_controllers={}\tused_controller_types=0x{:08X}\trecord_metadata={:?}",
                header.address,
                raw.length,
                raw._unk8,
                raw.timejump_count,
                raw.script_flags,
                raw.frame_rate,
                raw.bounds_box,
                raw.unk30,
                raw.thread_controller_count,
                raw.used_controller_types,
                processed.controller_record_metadata,
            ));
        }

        lines.push("-- mapzone render-pass census --".to_string());
        for (zone_index, zone) in map.mapzone_entities.iter().enumerate() {
            let Some(Ok((_, mesh))) = ref_entities
                .iter()
                .find(|entry| entry.hashcode == zone.entity_refptr)
                .map(|entry| entry.data.as_ref())
            else {
                lines.push(format!(
                    "zone#{zone_index}\tref={}\tunresolved",
                    zone.entity_refptr
                ));
                continue;
            };
            let normal_triangles = mesh
                .strips
                .iter()
                .filter(|strip| strip.flags & 0x10 == 0)
                .map(|strip| strip.index_count.saturating_sub(2) as usize)
                .sum::<usize>();
            let excluded_triangles = mesh
                .strips
                .iter()
                .filter(|strip| strip.flags & 0x10 != 0)
                .map(|strip| strip.index_count.saturating_sub(2) as usize)
                .sum::<usize>();
            lines.push(format!(
                "zone#{zone_index}\tref={}\tvertices={}\tstrips={}\tnormal_triangles={normal_triangles}\texcluded_0x10_triangles={excluded_triangles}",
                zone.entity_refptr,
                mesh.vertex_data.len(),
                mesh.strips.len(),
            ));
        }

        let gear_script_hashcode = 0x8400_0015;
        let gear_script = render_store
            .get_script(map_edb.header.hashcode, gear_script_hashcode)
            .expect("City gear Script 0x84000015 is missing");
        lines.push("-- animated entity controller census --".to_string());
        for script in &scripts {
            for (command_index, command) in script.commands.iter().enumerate() {
                let UXGeoScriptCommandData::Entity { hashcode, .. } = command.data else {
                    continue;
                };
                let Some(controller) = script
                    .controllers
                    .get(command.controller_header_index as usize)
                else {
                    continue;
                };
                if controller.channels.vector_0.is_empty()
                    && controller.channels.quat_0.is_empty()
                    && controller.channels.vector_1.is_empty()
                {
                    continue;
                }
                lines.push(format!(
                    "script=0x{:08X}\tcmd={command_index}\tentity=0x{hashcode:08X}\tcontroller={}\tstart={}\tlength={}\tposition_keys={}\trotation_keys={}\tscale_keys={}\tposition_first_last={:?}/{:?}\trotation_first_last={:?}/{:?}",
                    script.hashcode,
                    command.controller_header_index,
                    command.start,
                    command.length,
                    controller.channels.vector_0.len(),
                    controller.channels.quat_0.len(),
                    controller.channels.vector_1.len(),
                    controller.channels.vector_0.first(),
                    controller.channels.vector_0.last(),
                    controller.channels.quat_0.first(),
                    controller.channels.quat_0.last(),
                ));
            }
        }

        lines.push("-- city gear script 0x84000015 controllers --".to_string());
        for (controller_index, controller) in gear_script.controllers.iter().enumerate() {
            lines.push(format!(
                "controller#{controller_index}\tmask=0x{:08X}\tchannel_mask=0x{:08X}\tvector_0={:?}\tquat_0={:?}\tvector_1={:?}\tvector_2={:?}",
                controller.ctrl_mask,
                controller.ctrl_channel_mask,
                controller.channels.vector_0,
                controller.channels.quat_0,
                controller.channels.vector_1,
                controller.channels.vector_2,
            ));
        }
        lines.push("-- city gear script 0x84000015 queued transforms --".to_string());
        for frame in 0..gear_script.length {
            let time = gear_script.time_at_frame(frame as f32);
            let mut queue = Vec::new();
            crate::render::script::render_script(
                Vec3::ZERO,
                glam::Quat::IDENTITY,
                Vec3::ONE,
                map_edb.header.hashcode,
                gear_script_hashcode,
                time,
                &render_store,
                &mut |queued| queue.push(queued),
                vec![],
            );
            let gears = queue
                .iter()
                .filter(|queued| matches!(queued.entity.1, 0x8200_002B | 0x8200_002C))
                .map(|queued| {
                    format!(
                        "entity=0x{:08X},pos={:?},rot={:?},scale={:?}",
                        queued.entity.1, queued.position, queued.rotation, queued.scale
                    )
                })
                .collect::<Vec<_>>();
            lines.push(format!(
                "frame={frame}\ttime={time:.9}\t{}",
                gears.join("\t")
            ));
        }

        for script_hashcode in [0x8400_0028, 0x8400_0029, 0x8400_002A] {
            let script = render_store
                .get_script(map_edb.header.hashcode, script_hashcode)
                .unwrap_or_else(|| panic!("0x{script_hashcode:08X} is missing from City Scripts"));
            let root_visual_time = script
                .first_visual_frame()
                .map(|frame| script.time_at_frame(frame.max(0) as f32));
            let resolved_visual_time = crate::render::script::first_resolved_visual_time(
                map_edb.header.hashcode,
                script_hashcode,
                &render_store,
            )
            .unwrap_or_else(|| {
                panic!("0x{script_hashcode:08X} has no recursively resolved visual time")
            });
            let mut queue = Vec::new();
            crate::render::script::render_script(
                Vec3::ZERO,
                glam::Quat::IDENTITY,
                Vec3::ONE,
                map_edb.header.hashcode,
                script_hashcode,
                resolved_visual_time,
                &render_store,
                &mut |queued| queue.push(queued),
                vec![],
            );
            assert!(
                !queue.is_empty(),
                "0x{script_hashcode:08X} still queues no model at its recursively resolved visual time"
            );
            lines.push(format!(
                "script=0x{script_hashcode:08X}\troot_visual_time={root_visual_time:?}\tresolved_visual_time={resolved_visual_time}\tqueued_entities={}",
                queue.len()
            ));
        }

        let ref_6 = ref_entities
            .iter()
            .find(|entry| entry.hashcode == 6)
            .expect("ref_6 is missing from the decoded refpointer entities");
        let ref_6_mesh = ref_6
            .data
            .as_ref()
            .expect("ref_6 failed to parse")
            .1
            .clone();
        assert!(
            !ref_6_mesh.vertex_data.is_empty(),
            "ref_6 has no decoded geometry"
        );
        let ref_6_zones = map
            .mapzone_entities
            .iter()
            .enumerate()
            .filter_map(|(zone_index, zone)| (zone.entity_refptr == 6).then_some(zone_index))
            .collect::<Vec<_>>();
        lines.push(format!(
            "ref_6\tusage=mapzone_refpointer\tzones={ref_6_zones:?}\t{}",
            format_mesh_diagnostics(&ref_6_mesh)
        ));

        for hashcode in selected {
            let entity = entities
                .iter()
                .find(|(_, entry)| entry.hashcode == hashcode)
                .unwrap_or_else(|| panic!("0x{hashcode:08X} is missing from the entity list"));
            let (raw_entity, mesh) = entity
                .1
                .data
                .as_ref()
                .unwrap_or_else(|error| panic!("0x{hashcode:08X} failed to parse: {error}"));
            let placements = map
                .placements
                .iter()
                .enumerate()
                .filter(|(_, placement)| placement.object_ref == hashcode)
                .map(|(index, placement)| {
                    format!(
                        "#{index}@pos={:?},rot={:?},scale={:?},map_on=0x{:04X},flags=0x{:08X}",
                        placement.position,
                        placement.rotation,
                        placement.scale,
                        placement.map_on,
                        placement.flags,
                    )
                })
                .collect::<Vec<_>>();
            let script_refs = scripts
                .iter()
                .flat_map(|script| {
                    script.commands.iter().enumerate().filter_map(move |(command_index, command)| {
                        matches!(
                            command.data,
                            UXGeoScriptCommandData::Entity { hashcode: object, .. } if object == hashcode
                        )
                        .then_some(format!(
                            "0x{:08X}/cmd{command_index}/start{}/len{}/controller{}",
                            script.hashcode,
                            command.start,
                            command.length,
                            command.controller_header_index,
                        ))
                    })
                })
                .collect::<Vec<_>>();
            let trigger_refs = map
                .triggers
                .iter()
                .enumerate()
                .filter_map(|(index, trigger)| {
                    (trigger.engine_options.visual_object == Some(hashcode)).then_some(format!(
                        "#{index}/type{}@pos={:?},rot={:?},scale={:?}",
                        trigger.ttype, trigger.position, trigger.rotation, trigger.scale
                    ))
                })
                .collect::<Vec<_>>();
            let kind = match raw_entity {
                EXGeoEntity::Mesh(_) => "mesh",
                EXGeoEntity::Split(_) => "split",
                EXGeoEntity::MapZone(_) => "mapzone",
                EXGeoEntity::Instance(_) => "instance",
                EXGeoEntity::NavMesh(_) => "navmesh",
                EXGeoEntity::UnknownType(_) => "unknown",
            };
            lines.push(format!(
                "0x{hashcode:08X}\tkind={kind}\tplacements={placements:?}\tscripts={script_refs:?}\ttriggers={trigger_refs:?}\t{}",
                format_mesh_diagnostics(mesh)
            ));
        }

        lines.push("-- all placements --".to_string());
        for (placement_index, placement) in map.placements.iter().enumerate() {
            let position = Vec3::from(placement.position);
            let rotation = glam::Quat::from_euler(
                glam::EulerRot::ZXY,
                placement.rotation[2],
                placement.rotation[0],
                placement.rotation[1],
            );
            let scale = Vec3::from(placement.scale);
            if placement.object_ref.base() == 0x0200_0000 {
                let Some(entity) = entities
                    .iter()
                    .find(|(_, entry)| entry.hashcode == placement.object_ref)
                else {
                    lines.push(format!(
                        "placement#{placement_index}\tobject=0x{:08X}\tunresolved_entity",
                        placement.object_ref
                    ));
                    continue;
                };
                let mesh = &entity.1.data.as_ref().unwrap().1;
                let (bb_min, bb_max) = mesh.bounding_box();
                let local_center = if mesh.vertex_data.is_empty() {
                    Vec3::ZERO
                } else {
                    (bb_min + bb_max) * 0.5
                };
                let world_center = position + rotation.mul_vec3(scale * local_center);
                let containing_zones = map
                    .zones
                    .iter()
                    .enumerate()
                    .filter_map(|(zone_index, zone)| {
                        let a = Vec3::from(zone.bounds_box[0]);
                        let b = Vec3::from(zone.bounds_box[1]);
                        let min = a.min(b);
                        let max = a.max(b);
                        (world_center.cmpge(min).all() && world_center.cmple(max).all())
                            .then_some(zone_index)
                    })
                    .collect::<Vec<_>>();
                lines.push(format!(
                    "placement#{placement_index}\tobject=0x{:08X}\tlocal_center={local_center:?}\tposition={position:?}\tworld_center={world_center:?}\tzones={containing_zones:?}\tmap_on=0x{:04X}\t{}",
                    placement.object_ref,
                    placement.map_on,
                    format_mesh_diagnostics(mesh),
                ));
            } else {
                lines.push(format!(
                    "placement#{placement_index}\tobject=0x{:08X}\tposition={position:?}\trotation={:?}\tscale={scale:?}\tmap_on=0x{:04X}",
                    placement.object_ref,
                    placement.rotation,
                    placement.map_on,
                ));
            }
        }

        let true_zero_geometry = [0x8200_0026, 0x8200_0027, 0x8200_0058];
        for hashcode in selected {
            let mesh = &entities
                .iter()
                .find(|(_, entry)| entry.hashcode == hashcode)
                .unwrap()
                .1
                .data
                .as_ref()
                .unwrap()
                .1;
            assert_eq!(
                mesh.vertex_data.is_empty(),
                true_zero_geometry.contains(&hashcode),
                "unexpected zero/nonzero geometry classification for 0x{hashcode:08X}"
            );
        }

        let report = lines.join("\n") + "\n";
        eprintln!("{report}");
        if let Ok(output) = std::env::var("EUROCHEF_REAL_M02_GEOMETRY_REPORT") {
            if let Some(parent) = std::path::Path::new(&output).parent() {
                std::fs::create_dir_all(parent)
                    .expect("could not create m02_city geometry report folder");
            }
            std::fs::write(output, report).expect("could not write m02_city geometry report");
        }
    }

    #[test]
    fn real_map_geometry_and_motion_corpus_when_requested() {
        let Ok(root) = std::env::var("EUROCHEF_REAL_MAP_CORPUS_ROOT") else {
            return;
        };

        fn collect_edb_paths(root: &Path, output: &mut Vec<PathBuf>) {
            let Ok(entries) = std::fs::read_dir(root) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    collect_edb_paths(&path, output);
                } else if path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("edb"))
                {
                    output.push(path);
                }
            }
        }

        fn script_has_nonzero_entity(
            current_file: u32,
            script_hashcode: u32,
            render_store: &crate::render::RenderStore,
            nonzero_entities: &std::collections::HashSet<u32>,
            ancestry: &mut Vec<u32>,
        ) -> bool {
            if ancestry.len() >= 64 || ancestry.contains(&script_hashcode) {
                return false;
            }
            let Some(script) = render_store.get_script(current_file, script_hashcode) else {
                return false;
            };
            ancestry.push(script_hashcode);
            let found = script.commands.iter().any(|command| match command.data {
                UXGeoScriptCommandData::Entity { hashcode, file } => {
                    let source_file = if file == u32::MAX || hashcode.is_local() {
                        current_file
                    } else {
                        file
                    };
                    source_file == current_file
                        && render_store
                            .resolve_entity_hashcode(source_file, hashcode)
                            .is_some_and(|resolved| nonzero_entities.contains(&resolved))
                }
                UXGeoScriptCommandData::SubScript { hashcode, file } => {
                    let source_file = if file == u32::MAX || hashcode.is_local() {
                        current_file
                    } else {
                        file
                    };
                    source_file == current_file
                        && script_has_nonzero_entity(
                            source_file,
                            hashcode,
                            render_store,
                            nonzero_entities,
                            ancestry,
                        )
                }
                _ => false,
            });
            ancestry.pop();
            found
        }

        fn finite_vec3(value: [f32; 3]) -> bool {
            value.into_iter().all(f32::is_finite)
        }

        fn finite_vec4(value: [f32; 4]) -> bool {
            value.into_iter().all(f32::is_finite)
        }

        let mut paths = Vec::new();
        collect_edb_paths(Path::new(&root), &mut paths);
        paths.sort();

        let mut edb_files = 0usize;
        let mut map_files = 0usize;
        let mut map_count = 0usize;
        let mut entity_placements = 0usize;
        let mut script_placements = 0usize;
        let mut internal_model_scripts = 0usize;
        let mut moving_triggers = 0usize;
        let mut controller_keyframes = 0usize;
        let mut parse_failures = Vec::<String>::new();
        let mut unresolved_entity_placements = Vec::<String>::new();
        let mut model_scripts_without_queue = Vec::<String>::new();
        let mut nonfinite_rows = Vec::<String>::new();
        let mut missing_motion_paths = Vec::<String>::new();
        let mut suspicious_double_transforms = Vec::<String>::new();
        let mut motion_rows = Vec::<String>::new();
        let mut translation_axis_counts = [0usize; 3];
        let mut rotation_controller_count = 0usize;

        for path in paths {
            edb_files += 1;
            let open_edb = || {
                let file = File::open(&path).ok()?;
                EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).ok()
            };
            let Some(mut map_edb) = open_edb() else {
                parse_failures.push(format!("{}:header", path.display()));
                continue;
            };
            let maps = read_from_file(&mut map_edb);
            if maps.is_empty() {
                continue;
            }
            map_files += 1;
            map_count += maps.len();
            let file_uid = map_edb.header.hashcode;
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("<unknown>");

            let Some(mut entity_edb) = open_edb() else {
                parse_failures.push(format!("{file_name}:entities-open"));
                continue;
            };
            let Ok((entities, _, _)) = crate::entities::read_from_file(&mut entity_edb, None)
            else {
                parse_failures.push(format!("{file_name}:entities-parse"));
                continue;
            };
            let Some(mut script_edb) = open_edb() else {
                parse_failures.push(format!("{file_name}:scripts-open"));
                continue;
            };
            let Ok(scripts) = UXGeoScript::read_all(&mut script_edb) else {
                parse_failures.push(format!("{file_name}:scripts-parse"));
                continue;
            };

            let mut render_store = crate::render::RenderStore::new();
            let mut nonzero_entities = std::collections::HashSet::<u32>::new();
            let mut mesh_by_hash =
                std::collections::HashMap::<u32, &crate::entities::ProcessedEntityMesh>::new();
            for (entity_index, entry) in &entities {
                let Ok((_, mesh)) = &entry.data else {
                    parse_failures.push(format!("{file_name}:entity:0x{:08X}", entry.hashcode));
                    continue;
                };
                let mut renderer =
                    crate::render::entity::EntityRenderer::new(file_uid, Platform::Pc);
                renderer.set_serialized_vertex_count_for_test(mesh.vertex_data.len());
                render_store.insert_entity(file_uid, entry.hashcode, *entity_index, renderer);
                mesh_by_hash.insert(entry.hashcode, mesh);
                if !mesh.vertex_data.is_empty() {
                    nonzero_entities.insert(entry.hashcode);
                }
            }
            for script in &scripts {
                render_store.insert_script(file_uid, script.clone());

                for controller in &script.controllers {
                    for (time, value) in controller
                        .channels
                        .vector_0
                        .iter()
                        .chain(controller.channels.vector_1.iter())
                        .chain(controller.channels.vector_2.iter())
                    {
                        controller_keyframes += 1;
                        if !time.is_finite() || !finite_vec3(*value) {
                            nonfinite_rows.push(format!(
                                "{file_name}:script=0x{:08X}:vector-key",
                                script.hashcode
                            ));
                        }
                    }
                    for (time, value) in &controller.channels.quat_0 {
                        controller_keyframes += 1;
                        rotation_controller_count += 1;
                        if !time.is_finite() || !finite_vec4(*value) {
                            nonfinite_rows.push(format!(
                                "{file_name}:script=0x{:08X}:quat-key",
                                script.hashcode
                            ));
                        }
                    }
                    if let (Some((_, first)), Some((_, last))) = (
                        controller.channels.vector_0.first(),
                        controller.channels.vector_0.last(),
                    ) {
                        for axis in 0..3 {
                            if (last[axis] - first[axis]).abs() > 0.0001 {
                                translation_axis_counts[axis] += 1;
                            }
                        }
                    }
                }
            }

            for script in &scripts {
                if !script_has_nonzero_entity(
                    file_uid,
                    script.hashcode,
                    &render_store,
                    &nonzero_entities,
                    &mut Vec::new(),
                ) {
                    continue;
                }
                internal_model_scripts += 1;
                let Some(time) = crate::render::script::first_resolved_visual_time(
                    file_uid,
                    script.hashcode,
                    &render_store,
                ) else {
                    model_scripts_without_queue.push(format!(
                        "{file_name}:script=0x{:08X}:no-visual-time",
                        script.hashcode
                    ));
                    continue;
                };
                let mut queue = Vec::new();
                crate::render::script::render_script(
                    Vec3::ZERO,
                    glam::Quat::IDENTITY,
                    Vec3::ONE,
                    file_uid,
                    script.hashcode,
                    time,
                    &render_store,
                    &mut |queued| queue.push(queued),
                    vec![],
                );
                let visible_count = queue
                    .iter()
                    .filter(|queued| {
                        render_store
                            .resolve_entity_hashcode(queued.entity.0, queued.entity.1)
                            .is_some_and(|resolved| nonzero_entities.contains(&resolved))
                    })
                    .count();
                if visible_count == 0 {
                    model_scripts_without_queue.push(format!(
                        "{file_name}:script=0x{:08X}:time={time}:queue={}",
                        script.hashcode,
                        queue.len()
                    ));
                }
            }

            for map in &maps {
                for (placement_index, placement) in map.placements.iter().enumerate() {
                    if !placement.position.into_iter().all(f32::is_finite)
                        || !placement.rotation.into_iter().all(f32::is_finite)
                        || !placement.scale.into_iter().all(f32::is_finite)
                    {
                        nonfinite_rows.push(format!(
                            "{file_name}:map=0x{:08X}:placement#{placement_index}",
                            map.hashcode
                        ));
                    }
                    match placement.object_ref.base() {
                        0x0200_0000 => {
                            entity_placements += 1;
                            let Some(resolved) = render_store
                                .resolve_entity_hashcode(file_uid, placement.object_ref)
                            else {
                                unresolved_entity_placements.push(format!(
                                    "{file_name}:map=0x{:08X}:placement#{placement_index}:0x{:08X}",
                                    map.hashcode, placement.object_ref
                                ));
                                continue;
                            };
                            let Some(mesh) = mesh_by_hash.get(&resolved) else {
                                continue;
                            };
                            if !mesh.vertex_data.is_empty() {
                                let (minimum, maximum) = mesh.bounding_box();
                                let center = (minimum + maximum) * 0.5;
                                let position = Vec3::from(placement.position);
                                if position.length() > 10.0 && center.length() > 50.0 {
                                    suspicious_double_transforms.push(format!(
                                        "{file_name}:map=0x{:08X}:placement#{placement_index}:object=0x{resolved:08X}:position={position:?}:mesh_center={center:?}",
                                        map.hashcode
                                    ));
                                }
                            }
                        }
                        0x0400_0000 => {
                            script_placements += 1;
                        }
                        _ => {}
                    }
                }

                for (trigger_index, trigger) in map.triggers.iter().enumerate() {
                    if !matches!(trigger.ttype, 8 | 37 | 80) {
                        continue;
                    }
                    moving_triggers += 1;
                    let speed = robots_trigger_runtime_path_speed(trigger.ttype, &trigger.data)
                        .unwrap_or_default();
                    let acceleration =
                        robots_trigger_runtime_path_acceleration(trigger.ttype, &trigger.data)
                            .unwrap_or_default();
                    let angular =
                        robots_trigger_platform_angular_velocity(trigger.ttype, &trigger.data)
                            .unwrap_or(Vec3::ZERO);
                    let sample = crate::map_runtime::runtime_path_preview_position(
                        map, trigger, 1.0, true, 1.0,
                    );
                    if !speed.is_finite()
                        || !acceleration.is_finite()
                        || !angular.is_finite()
                        || !sample.is_finite()
                    {
                        nonfinite_rows.push(format!(
                            "{file_name}:map=0x{:08X}:trigger#{trigger_index}:type{}",
                            map.hashcode, trigger.ttype
                        ));
                    }
                    let path_hash = robots_trigger_path_hash(trigger.ttype, &trigger.data);
                    if let Some(path_hash) = path_hash {
                        if !map.paths.iter().any(|path| path.hashcode == path_hash) {
                            missing_motion_paths.push(format!(
                                "{file_name}:map=0x{:08X}:trigger#{trigger_index}:type{}:path=0x{path_hash:08X}",
                                map.hashcode, trigger.ttype
                            ));
                        }
                    }
                    motion_rows.push(format!(
                        "{file_name}\tmap=0x{:08X}\ttrigger={trigger_index}\ttype={}\tpath={path_hash:?}\tspeed={speed}\tacceleration={acceleration}\tangular_xyz_deg_s={angular:?}\tposition_t1={sample:?}",
                        map.hashcode, trigger.ttype
                    ));
                }
            }
        }

        let mut report = format!(
            "edb_files={edb_files}\nmap_files={map_files}\nmaps={map_count}\nentity_placements={entity_placements}\nscript_placements={script_placements}\ninternal_model_scripts={internal_model_scripts}\nmoving_triggers={moving_triggers}\ncontroller_keyframes={controller_keyframes}\ntranslation_axis_x={}\ntranslation_axis_y={}\ntranslation_axis_z={}\nrotation_controller_keys={rotation_controller_count}\nparse_failures={}\nunresolved_entity_placements={}\nmodel_scripts_without_queue={}\nnonfinite_rows={}\nmissing_motion_paths={}\nsuspicious_double_transforms={}\n",
            translation_axis_counts[0],
            translation_axis_counts[1],
            translation_axis_counts[2],
            parse_failures.len(),
            unresolved_entity_placements.len(),
            model_scripts_without_queue.len(),
            nonfinite_rows.len(),
            missing_motion_paths.len(),
            suspicious_double_transforms.len(),
        );
        for (heading, rows) in [
            ("parse_failures", &parse_failures),
            (
                "unresolved_entity_placements",
                &unresolved_entity_placements,
            ),
            ("model_scripts_without_queue", &model_scripts_without_queue),
            ("nonfinite_rows", &nonfinite_rows),
            ("missing_motion_paths", &missing_motion_paths),
            (
                "suspicious_double_transforms",
                &suspicious_double_transforms,
            ),
            ("motion", &motion_rows),
        ] {
            report.push_str(&format!("\n[{heading}]\n"));
            for row in rows {
                report.push_str(row);
                report.push('\n');
            }
        }

        assert!(edb_files >= 179, "expected the shipped 179-EDB PC corpus");
        assert!(map_files >= 18, "expected the shipped map EDB corpus");
        assert!(parse_failures.is_empty(), "{parse_failures:#?}");
        assert!(
            unresolved_entity_placements.is_empty(),
            "{unresolved_entity_placements:#?}"
        );
        assert!(
            model_scripts_without_queue.is_empty(),
            "{model_scripts_without_queue:#?}"
        );
        assert!(nonfinite_rows.is_empty(), "{nonfinite_rows:#?}");
        assert!(missing_motion_paths.is_empty(), "{missing_motion_paths:#?}");

        if let Ok(output) = std::env::var("EUROCHEF_REAL_MAP_CORPUS_REPORT") {
            if let Some(parent) = Path::new(&output).parent() {
                std::fs::create_dir_all(parent)
                    .expect("could not create map geometry/motion corpus folder");
            }
            std::fs::write(output, report)
                .expect("could not write map geometry/motion corpus report");
        }
    }

    #[test]
    fn real_m02_city_reported_sky_entities_preserve_native_flag_classes_when_requested() {
        let Ok(path) = std::env::var("EUROCHEF_REAL_M02_CITY_EDB") else {
            return;
        };
        let open_edb = || {
            let file = File::open(&path).expect("m02_city fixture is missing");
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
                .expect("m02_city fixture is not a valid PC EDB")
        };

        let targets = [
            (0x8200_0006, 0x8400_0017, None, 0u32),
            (0x8200_0007, 0x8400_0017, Some(15usize), 0u32),
            (0x8200_002E, 0x8400_0017, Some(70usize), 768u32),
            (0x8200_002F, 0x8400_0018, Some(8usize), 768u32),
            (0x8200_0030, 0x8400_0019, Some(64usize), 768u32),
            (0x8200_0098, 0x8400_0035, Some(27usize), 768u32),
        ];
        let mut map_edb = open_edb();
        let maps = read_from_file(&mut map_edb);
        let map = maps.first().expect("m02_city map is missing");

        let mut script_edb = open_edb();
        let scripts =
            UXGeoScript::read_all(&mut script_edb).expect("m02_city scripts did not parse");

        for (target, expected_script, expected_children, expected_flags) in targets {
            assert!(
                map.placements
                    .iter()
                    .all(|placement| placement.object_ref != target),
                "0x{target:08X} is Script-owned and must not become a direct placement"
            );
            assert!(
                map.triggers
                    .iter()
                    .all(|trigger| trigger.engine_options.visual_object != Some(target)),
                "0x{target:08X} is not a direct trigger visual"
            );

            let mut matching_commands = Vec::new();
            for script in &scripts {
                for (command_index, command) in script.commands.iter().enumerate() {
                    let UXGeoScriptCommandData::Entity { hashcode, file } = &command.data else {
                        continue;
                    };
                    if *hashcode == target {
                        matching_commands.push((script.hashcode, command_index, command, *file));
                    }
                }
            }
            let (_, command_index, command, file) = matching_commands
                .iter()
                .find(|(script, _, _, _)| *script == expected_script)
                .copied()
                .unwrap_or_else(|| {
                    panic!(
                        "0x{target:08X} is missing from expected sky Script 0x{expected_script:08X}"
                    )
                });
            let is_base_sky_root =
                map.skies.first().copied() == Some(expected_script) && command_index == 0;
            assert_eq!(is_base_sky_root, target == 0x8200_0030);
            assert_eq!(file, u32::MAX);
            assert_eq!(command.opcode, 3);
            assert_eq!(command.parent_controller_index, u8::MAX);
            let controller = scripts
                .iter()
                .find(|script| script.hashcode == expected_script)
                .and_then(|script| {
                    script
                        .controllers
                        .get(command.controller_header_index as usize)
                })
                .expect("reported sky Entity controller is missing");
            if target == 0x8200_002E {
                assert_eq!(controller.channels.vector_0.len(), 1);
                let (frame, translation) = controller.channels.vector_0[0];
                assert_eq!(frame, 0.0);
                let expected = [-0.0016253801, -0.0042165825, 0.00225354];
                for (actual, expected) in translation.into_iter().zip(expected) {
                    assert!((actual - expected).abs() < 1.0e-8);
                }
            } else {
                assert_eq!(controller.controller_count, 0);
                assert_eq!(controller.channel_count, 0);
                assert!(controller.channels.vector_0.is_empty());
                assert!(controller.channels.quat_0.is_empty());
            }

            let mut entity_edb = open_edb();
            let header = entity_edb.header.clone();
            let endian = entity_edb.endian;
            let record = header
                .entity_list
                .iter()
                .find(|record| record.common.hashcode == target)
                .expect("reported entity header is missing");
            entity_edb
                .seek(std::io::SeekFrom::Start(record.common.address as u64))
                .expect("could not seek to reported entity");
            let entity = entity_edb
                .read_type_args::<EXGeoEntity>(endian, (header.version, Platform::Pc))
                .expect("reported entity did not parse");
            let base = entity.base().expect("reported entity has no base");
            assert_eq!(
                entity.type_code(),
                if expected_children.is_some() {
                    0x603
                } else {
                    0x601
                }
            );
            assert_eq!(
                base.flags, expected_flags,
                "0x{target:08X} changed its native sky transform flag class"
            );
            assert!(base
                .bounds_box
                .iter()
                .flatten()
                .all(|value| value.is_finite()));
            match (entity, expected_children) {
                (EXGeoEntity::Mesh(_), None) => {}
                (EXGeoEntity::Split(split), Some(expected_children)) => {
                    assert_eq!(split.entities.len(), expected_children);
                }
                _ => panic!("0x{target:08X} changed its expected Mesh/Split shape"),
            }
        }

        for script_hashcode in [0x8400_0019, 0x8400_0035] {
            let script = scripts
                .iter()
                .find(|script| script.hashcode == script_hashcode)
                .unwrap_or_else(|| panic!("City sky Script 0x{script_hashcode:08X} is missing"));
            let command = script
                .commands
                .iter()
                .find(|command| {
                    matches!(
                        command.data,
                        UXGeoScriptCommandData::Entity {
                            hashcode: 0x8200_003A,
                            ..
                        }
                    )
                })
                .unwrap_or_else(|| {
                    panic!("0x8200003A is missing from City sky Script 0x{script_hashcode:08X}")
                });
            let controller = script
                .controllers
                .get(command.controller_header_index as usize)
                .expect("0x8200003A controller is missing");
            assert_eq!(controller.channels.vector_0.len(), 1);
            let (frame, translation) = controller.channels.vector_0[0];
            assert_eq!(frame, 0.0);
            let expected = [26.549105, 0.0111720245, 155.493];
            for (actual, expected) in translation.into_iter().zip(expected) {
                assert!((actual - expected).abs() < 1.0e-5);
            }
        }

        let mut entity_edb = open_edb();
        let header = entity_edb.header.clone();
        let endian = entity_edb.endian;
        let record = header
            .entity_list
            .iter()
            .find(|record| record.common.hashcode == 0x8200_003A)
            .expect("City sky Entity 0x8200003A is missing");
        entity_edb
            .seek(std::io::SeekFrom::Start(record.common.address as u64))
            .expect("could not seek to City sky Entity 0x8200003A");
        let entity = entity_edb
            .read_type_args::<EXGeoEntity>(endian, (header.version, Platform::Pc))
            .expect("City sky Entity 0x8200003A did not parse");
        assert_eq!(
            entity
                .base()
                .expect("City sky Entity 0x8200003A has no base")
                .flags,
            0
        );
        assert!(matches!(entity, EXGeoEntity::Split(_)));

        for background in [0x8200_003C, 0x8200_003D, 0x8200_003E] {
            let mut entity_edb = open_edb();
            let header = entity_edb.header.clone();
            let endian = entity_edb.endian;
            let record = header
                .entity_list
                .iter()
                .find(|record| record.common.hashcode == background)
                .expect("City sky background entity header is missing");
            entity_edb
                .seek(std::io::SeekFrom::Start(record.common.address as u64))
                .expect("could not seek to City sky background entity");
            let entity = entity_edb
                .read_type_args::<EXGeoEntity>(endian, (header.version, Platform::Pc))
                .expect("City sky background entity did not parse");
            let base = entity
                .base()
                .expect("City sky background entity has no base");
            assert_ne!(
                base.flags & 0x10,
                0,
                "0x{background:08X} must retain the camera-relative background flag"
            );
        }
    }

    #[test]
    fn real_main_map_sky_background_flag_corpus_when_requested() {
        let Ok(root) = std::env::var("EUROCHEF_REAL_MAIN_MAP_SKY_ROOT") else {
            return;
        };

        fn collect_sky_entities(
            object: u32,
            scripts: &std::collections::HashMap<u32, &UXGeoScript>,
            entities: &mut std::collections::HashSet<u32>,
            ancestry: &mut Vec<u32>,
        ) {
            match object.base() {
                0x0200_0000 => {
                    entities.insert(object);
                }
                0x0400_0000 => {
                    if ancestry.len() >= 64 || ancestry.contains(&object) {
                        return;
                    }
                    let Some(script) = scripts.get(&object).copied() else {
                        return;
                    };
                    ancestry.push(object);
                    for command in &script.commands {
                        match command.data {
                            UXGeoScriptCommandData::Entity { hashcode, file }
                                if file == u32::MAX || hashcode.is_local() =>
                            {
                                entities.insert(hashcode);
                            }
                            UXGeoScriptCommandData::SubScript { hashcode, file }
                                if file == u32::MAX || hashcode.is_local() =>
                            {
                                collect_sky_entities(hashcode, scripts, entities, ancestry);
                            }
                            _ => {}
                        }
                    }
                    ancestry.pop();
                }
                _ => {}
            }
        }

        let mut paths = std::fs::read_dir(&root)
            .expect("main-map sky root is missing")
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("edb"))
                    && path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.starts_with('m'))
            })
            .collect::<Vec<_>>();
        paths.sort();

        let mut audited_maps = 0usize;
        let mut world_only_maps = Vec::new();
        for path in paths {
            let open_edb = || {
                let file = File::open(&path).expect("main-map fixture disappeared");
                EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
                    .expect("main-map fixture is not a valid PC EDB")
            };

            let mut map_edb = open_edb();
            let maps = read_from_file(&mut map_edb);
            let sky_objects = maps
                .iter()
                .flat_map(|map| map.skies.iter().copied())
                .collect::<std::collections::HashSet<_>>();
            if sky_objects.is_empty() {
                continue;
            }
            let mut sky_object_list = sky_objects.iter().copied().collect::<Vec<_>>();
            sky_object_list.sort_unstable();

            let mut script_edb = open_edb();
            let scripts =
                UXGeoScript::read_all(&mut script_edb).expect("main-map sky Scripts did not parse");
            let scripts = scripts
                .iter()
                .map(|script| (script.hashcode, script))
                .collect::<std::collections::HashMap<_, _>>();
            let mut sky_entities = std::collections::HashSet::new();
            for sky in sky_objects {
                collect_sky_entities(sky, &scripts, &mut sky_entities, &mut Vec::new());
            }

            let mut entity_edb = open_edb();
            let header = entity_edb.header.clone();
            let endian = entity_edb.endian;
            let mut background_count = 0usize;
            let mut world_count = 0usize;
            let mut classified_entities = Vec::new();
            for record in header
                .entity_list
                .iter()
                .filter(|record| sky_entities.contains(&record.common.hashcode))
            {
                entity_edb
                    .seek(std::io::SeekFrom::Start(record.common.address as u64))
                    .expect("could not seek to main-map sky Entity");
                let entity = entity_edb
                    .read_type_args::<EXGeoEntity>(endian, (header.version, Platform::Pc))
                    .expect("main-map sky Entity did not parse");
                let flags = entity.base().map(|base| base.flags).unwrap_or_default();
                classified_entities.push((record.common.hashcode, flags));
                if flags & 0x10 != 0 {
                    background_count += 1;
                } else {
                    world_count += 1;
                }
            }

            let file_name = path.file_name().unwrap().to_string_lossy().into_owned();
            if background_count == 0 {
                world_only_maps.push(file_name.clone());
            }
            eprintln!(
                "{}: skies={:?} camera_relative={} world_space={} entities={:?}",
                file_name, sky_object_list, background_count, world_count, classified_entities
            );
            audited_maps += 1;
        }

        assert!(
            audited_maps >= 5,
            "too few main maps with skies were audited"
        );
        assert_eq!(world_only_maps, ["m08_chas.edb"]);
    }

    #[test]
    fn real_m03_hub1_no_sky_zones_keep_a_camera_relative_background_source_when_requested() {
        let Ok(path) = std::env::var("EUROCHEF_REAL_M03_HUB1_EDB") else {
            return;
        };
        let open_edb = || {
            let file = File::open(&path).expect("m03_hub1 fixture is missing");
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
                .expect("m03_hub1 fixture is not a valid PC EDB")
        };

        let mut map_edb = open_edb();
        let maps = read_from_file(&mut map_edb);
        let map = maps.first().expect("m03_hub1 map is missing");
        assert_eq!(map.skies.first().copied(), Some(0x8400_000D));
        let no_sky_zones = map
            .zones
            .iter()
            .enumerate()
            .filter_map(|(index, zone)| (zone.identifier.sky_index == -1).then_some(index))
            .collect::<Vec<_>>();
        assert_eq!(no_sky_zones, [29, 30]);

        let mut script_edb = open_edb();
        let scripts =
            UXGeoScript::read_all(&mut script_edb).expect("m03_hub1 scripts did not parse");
        let base_sky = scripts
            .iter()
            .find(|script| script.hashcode == 0x8400_000D)
            .expect("m03_hub1 base sky Script 0x8400000D is missing");
        assert!(base_sky.commands.iter().any(|command| {
            matches!(
                command.data,
                UXGeoScriptCommandData::Entity {
                    hashcode: 0x8200_0040,
                    ..
                }
            )
        }));

        let mut entity_edb = open_edb();
        let header = entity_edb.header.clone();
        let endian = entity_edb.endian;
        let record = header
            .entity_list
            .iter()
            .find(|record| record.common.hashcode == 0x8200_0040)
            .expect("m03_hub1 background Entity 0x82000040 is missing");
        entity_edb
            .seek(std::io::SeekFrom::Start(record.common.address as u64))
            .expect("could not seek to m03_hub1 background Entity");
        let entity = entity_edb
            .read_type_args::<EXGeoEntity>(endian, (header.version, Platform::Pc))
            .expect("m03_hub1 background Entity did not parse");
        let flags = entity
            .base()
            .expect("m03_hub1 background Entity has no base")
            .flags;
        assert_ne!(flags & 0x10, 0);
    }

    #[test]
    fn real_m04_cour_platform_paths_start_at_serialized_trigger_positions_when_requested() {
        let Ok(path) = std::env::var("EUROCHEF_REAL_M04_COUR_EDB") else {
            return;
        };
        let file = File::open(&path).expect("m04_cour fixture is missing");
        let mut edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
            .expect("m04_cour fixture is not a valid PC EDB");
        let maps = read_from_file(&mut edb);
        let map = maps.first().expect("m04_cour map is missing");

        let mut path_platform_count = 0usize;
        for (index, trigger) in map
            .triggers
            .iter()
            .enumerate()
            .filter(|(_, trigger)| trigger.ttype == 8)
        {
            let Some(path_hash) = robots_trigger_path_hash(trigger.ttype, &trigger.data) else {
                continue;
            };
            let Some(path) = map.paths.iter().find(|path| path.hashcode == path_hash) else {
                continue;
            };
            if crate::map_runtime::runtime_path_route(path).len() < 2 {
                continue;
            }
            path_platform_count += 1;
            let sample =
                crate::map_runtime::runtime_path_preview_sample_at_distance(map, trigger, 0.0)
                    .unwrap_or_else(|| {
                        panic!("Courtyard Platform #{index} has no initial path sample")
                    });
            assert!(
                sample.position.distance(trigger.position) < 0.001,
                "Courtyard Platform #{index} shifted from {:?} to {:?}",
                trigger.position,
                sample.position,
            );
        }
        assert!(
            path_platform_count >= 7,
            "m04_cour path-driven Platform corpus unexpectedly shrank"
        );
    }
}
