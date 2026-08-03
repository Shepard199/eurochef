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
pub use entities::robots_pickup_visual;
pub use triggers::{
    robots_camera_flags, robots_camera_marker_scaled_data0, robots_camera_mode,
    robots_camera_scaled_data4, robots_camera_scaled_data5, robots_monster_data15_value,
    robots_monster_data4_value, robots_monster_flags, robots_monster_is_family,
    robots_monster_proximity_radius, robots_monster_runtime_selector,
    robots_monster_test_runtime_value, robots_monster_transporter_secondary_path_hash,
    robots_npc_alternate_cutscenes, robots_npc_cutscene_is_null, robots_npc_flags,
    robots_npc_runtime_selector, robots_npc_runtime_uid, robots_npc_text_group,
    robots_trigger_path_data_slot, robots_trigger_path_hash, robots_trigger_path_is_proven,
    robots_trigger_platform_angular_velocity, robots_trigger_runtime_path_acceleration,
    robots_trigger_runtime_path_speed, robots_watchbot_enter_distance, robots_watchbot_flags,
    robots_watchbot_leave_distance, robots_watchbot_mode,
};

pub struct MapViewerPanel {
    maps: Vec<ProcessedMap>,

    // TODO(cohae): Replace so we can do funky stuff
    frame: MapFrame,
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct ProcessedMap {
    pub hashcode: u32,
    pub mapzone_entities: Vec<EXGeoMapZoneEntity>,
    pub zones: Vec<EXGeoMapZone>,
    pub skies: Vec<Hashcode>,
    pub placements: Vec<EXGeoPlacement>,
    pub lights: Vec<ProcessedLight>,
    pub sounds: Vec<ProcessedSound>,
    pub lighting_triangles: Vec<NativeLightingTriangle>,
    pub paths: Vec<ProcessedPath>,
    pub triggers: Vec<ProcessedTrigger>,
    pub trigger_collisions: Vec<EXGeoBaseDatum>,
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

            // PATCH_0027: trigger-only pickups do not serialize visual_object/file.
            // Add the proven O01 pickup entity as a synthetic external reference so
            // the existing reference loader fetches real geometry before rendering.
            if let Some(pickup) = robots_pickup_visual(ttype, &trig.data) {
                edb.add_reference(pickup.file, pickup.entity);
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
        read_from_file, robots_camera_flags, robots_camera_marker_scaled_data0, robots_camera_mode,
        robots_camera_scaled_data4, robots_camera_scaled_data5, robots_monster_data15_value,
        robots_monster_data4_value, robots_monster_flags, robots_monster_is_family,
        robots_monster_proximity_radius, robots_monster_runtime_selector,
        robots_monster_test_runtime_value, robots_monster_transporter_secondary_path_hash,
        robots_native_light_colour, robots_native_light_type_description,
        robots_npc_alternate_cutscenes, robots_npc_cutscene_is_null, robots_npc_flags,
        robots_npc_runtime_selector, robots_npc_runtime_uid, robots_npc_text_group,
        robots_trigger_path_data_slot, robots_trigger_path_hash, robots_trigger_path_is_proven,
        robots_trigger_platform_angular_velocity, robots_trigger_runtime_path_acceleration,
        robots_trigger_runtime_path_speed, robots_watchbot_enter_distance, robots_watchbot_flags,
        robots_watchbot_leave_distance, robots_watchbot_mode,
    };
    use eurochef_edb::{edb::EdbFile, versions::Platform};
    use eurochef_shared::script::{UXGeoScript, UXGeoScriptCommandData};
    use glam::Vec3;
    use std::{
        fs::File,
        io::BufReader,
        path::{Path, PathBuf},
    };

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
}
