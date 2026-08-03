use std::{io::Cursor, sync::Arc};

use anyhow::Context;
use egui::{
    mutex::{Mutex, RwLock},
    Pos2, Rect, Vec2,
};
use eurochef_edb::{Hashcode, HashcodeUtils};
use eurochef_shared::{
    maps::{DefinitionDataType, TriggerInformation},
    script::UXGeoScript,
};
use fxhash::FxHashMap;
use glam::{Quat, Vec3};
use glow::HasContext;
use nohash_hasher::IntMap;

use crate::map_runtime::{
    apply_vehicle_steering_wheel_angle, apply_vehicle_wheel_roll, apply_vehicle_wheel_roll_angle,
    map_trigger_by_link, map_trigger_path_matches, map_trigger_runtime_path,
    robots_vehicle_steering_wheel_angle, robots_vehicle_wheel_roll_angle,
    runtime_path_node_dispatches_between, runtime_path_preview_position_with_event,
    runtime_path_segments, runtime_platform_contact_linear_velocity,
    runtime_trigger_preview_rotation_with_event, RuntimeEventPreviewSnapshot,
    RuntimeEventPreviewState, RuntimePathNodeEvent, ROBOTS_EVENT_ACTIVATE, ROBOTS_EVENT_DEACTIVATE,
};
use crate::{
    maps::{
        robots_camera_flags, robots_camera_marker_scaled_data0, robots_camera_mode,
        robots_camera_scaled_data4, robots_camera_scaled_data5, robots_dev_map_info,
        robots_monster_data15_value, robots_monster_data4_value, robots_monster_flags,
        robots_monster_is_family, robots_monster_proximity_radius, robots_monster_runtime_selector,
        robots_monster_test_runtime_value, robots_monster_transporter_secondary_path_hash,
        robots_native_light_colour, robots_native_light_type_description,
        robots_npc_alternate_cutscenes, robots_npc_cutscene_is_null, robots_npc_flags,
        robots_npc_runtime_selector, robots_npc_runtime_uid, robots_npc_text_group,
        robots_pickup_visual, robots_trigger_path_data_slot, robots_trigger_path_hash,
        robots_trigger_path_is_proven, robots_trigger_platform_angular_velocity,
        robots_trigger_runtime_path_acceleration, robots_trigger_runtime_path_speed,
        robots_watchbot_enter_distance, robots_watchbot_flags, robots_watchbot_leave_distance,
        robots_watchbot_mode, ProcessedMap, ProcessedTrigger,
    },
    render::{
        billboard::BillboardRenderer,
        blend::{set_blending_mode, BlendMode},
        entity::EntityRenderer,
        gl_helper,
        particle::{ParticlePreviewSettings, ParticleRenderer},
        pickbuffer::{decode_pick_value, PickBuffer, PickBufferType},
        robots_global_lighting,
        script::{collect_script_particles, render_script, render_static_script},
        trigger::{CollisionDatumRenderer, LinkLineRenderer, SelectCubeRenderer},
        tweeny::{self, Tweeny3D},
        viewer::{BaseViewer, RenderContext},
        NativeLight, NativeLightZone, RenderStore,
    },
    scripts::fan::{advance_native_fan_angle, apply_native_fan_rotation},
    sound_preview::{SharedSoundPreview, SoundVoiceGroup},
};

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    struct RenderFilter: u32 {
        const MapZone = (1 << 0);
        const Placements = (1 << 1);
        const Triggers = (1 << 2);
        const Opaque = (1 << 16);
        const Transparent = (1 << 17);
    }
}

pub struct MapFrame {
    file: Hashcode,
    gl: Arc<glow::Context>,
    pub ref_renderers: Vec<(u32, Arc<Mutex<EntityRenderer>>)>,
    render_store: Arc<RwLock<RenderStore>>,

    billboard_renderer: Arc<BillboardRenderer>,
    particle_renderer: Arc<ParticleRenderer>,
    particle_settings: ParticlePreviewSettings,
    collision_renderer: Arc<CollisionDatumRenderer>,
    default_trigger_icon: glow::Texture,
    link_renderer: Arc<LinkLineRenderer>,
    selected_trigger: Option<usize>,
    selected_sound: Option<usize>,
    sound_preview: SharedSoundPreview,
    selected_link: Option<i32>,
    select_renderer: Arc<SelectCubeRenderer>,

    pub viewer: Arc<Mutex<BaseViewer>>,
    sky_ent: String,

    /// Used to prevent keybinds being triggered while a textfield is focused
    textfield_focused: bool,

    vertex_lighting: bool,
    global_lighting: bool,
    native_lights: bool,
    native_light_strength: f32,
    show_navmesh: bool,
    navmesh_texture_scale: f32,
    show_triggers: bool,
    show_sounds: bool,
    show_runtime_path: bool,
    animate_runtime_paths: bool,
    native_runtime_event_gate: bool,
    runtime_event_states: FxHashMap<u64, RuntimeEventPreviewState>,
    runtime_path_playback_speed: f32,
    platform_rotation_speed_scale: f32,
    runtime_motion_start_time: Option<f64>,
    script_animation_start_time: Option<f64>,
    animate_scripts: bool,
    script_playback_speed: f32,
    pickbuffer: PickBuffer,

    selected_map: usize,
    trigger_scale: f32,
    sound_scale: f32,
    trigger_focus_tween: Option<Tweeny3D>,

    trigger_info: Arc<TriggerInformation>,
    selected_triginfo_path: String,
    available_triginfo_paths: Vec<String>,

    hashcodes: Arc<IntMap<u32, String>>,
    trigger_icons: Arc<FxHashMap<String, glow::Texture>>,
    render_filter: RenderFilter,
    global_lightmap:
        Arc<Mutex<Option<(u32, Arc<crate::render::global_lightmap::GpuGlobalLightmap>)>>>,
}

const TRIGGER_ICON_DATA: &[(&str, &[u8])] = &[
    (
        "default",
        include_bytes!("../../../assets/icons/triggers/default.png"),
    ),
    (
        "tr_timer",
        include_bytes!("../../../assets/icons/triggers/TR_timer.png"),
    ),
    (
        "tr_link",
        include_bytes!("../../../assets/icons/triggers/TR_Link.png"),
    ),
    (
        "tr_killzone",
        include_bytes!("../../../assets/icons/triggers/TR_KillZone.png"),
    ),
    (
        "tr_counter",
        include_bytes!("../../../assets/icons/triggers/TR_Counter.png"),
    ),
    (
        "pl_startpoint",
        include_bytes!("../../../assets/icons/triggers/PL_StartPoint.png"),
    ),
    (
        "pl_checkpoint",
        include_bytes!("../../../assets/icons/triggers/PL_CheckPoint.png"),
    ),
    (
        "ob_static",
        include_bytes!("../../../assets/icons/triggers/OB_Static.png"),
    ),
    (
        "ob_container",
        include_bytes!("../../../assets/icons/triggers/OB_Container.png"),
    ),
    (
        "navigation",
        include_bytes!("../../../assets/icons/triggers/navigation.png"),
    ),
    (
        "fx_lensflare",
        include_bytes!("../../../assets/icons/triggers/FX_LensFlare.png"),
    ),
    (
        "sound",
        include_bytes!("../../../assets/icons/triggers/Sound.png"),
    ),
];
const ROBOTS_TRIGGER_INFO: &str = include_str!("../../../assets/triggers_robots.yml");

fn map_sky_objects(sky_override: &str, skies: &[Hashcode]) -> Vec<Hashcode> {
    u32::from_str_radix(sky_override.trim(), 16)
        .map(|sky| vec![sky])
        .unwrap_or_else(|_| skies.to_vec())
}

fn map_script_time(
    script: &UXGeoScript,
    global_time: f32,
    animate: bool,
    playback_speed: f32,
) -> f32 {
    if animate {
        let duration = script.duration_seconds().max(1.0 / 60.0);
        (global_time * playback_speed.max(0.0)).rem_euclid(duration)
    } else {
        script.time_at_frame(script.first_geometry_frame().unwrap_or(0).max(0) as f32)
    }
}

fn pickbuffer_pixel_position(rect: Rect, pointer: Pos2) -> Option<(i32, i32)> {
    let width = rect.width().floor() as i32;
    let height = rect.height().floor() as i32;
    let x = (pointer.x - rect.min.x).floor() as i32;
    let y = height - 1 - (pointer.y - rect.min.y).floor() as i32;
    (x >= 0 && x < width && y >= 0 && y < height).then_some((x, y))
}

fn load_png_frame(data: &[u8]) -> (Vec<u8>, png::OutputInfo) {
    let mut cursor = Cursor::new(data);
    let mut decoder = png::Decoder::new(std::io::BufReader::new(&mut cursor));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().unwrap();
    let mut img_data = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut img_data).unwrap();
    (img_data[..info.buffer_size()].to_vec(), info)
}

// ROBOTS_PATCH_0022_TRIGGER_VISUAL_OBJECT_RESOLUTION
// Local visual-object hashes belong to the current EDB namespace.
// The serialized visual_object_file field is not authoritative for local hashes.
fn trigger_visual_file(
    current_file: Hashcode,
    visual_object: Hashcode,
    visual_object_file: Option<Hashcode>,
) -> Hashcode {
    if visual_object.is_local() {
        current_file
    } else {
        visual_object_file.unwrap_or(current_file)
    }
}
pub struct QueuedEntityRender {
    pub entity: (Hashcode, Hashcode),
    pub entity_alt: Option<Arc<Mutex<EntityRenderer>>>,
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

mod canvas;
mod controls;
mod inspector;
mod runtime_events;
mod sound;

impl MapFrame {
    pub fn new(
        file: Hashcode,
        ref_renderers: Vec<(u32, Arc<Mutex<EntityRenderer>>)>,
        gl: Arc<glow::Context>,
        render_store: Arc<RwLock<RenderStore>>,
        hashcodes: Arc<IntMap<u32, String>>,
        game: &str,
        sound_preview: SharedSoundPreview,
    ) -> Self {
        let mut available_triginfo_paths = vec![];
        let exe_path = std::env::current_exe().unwrap();
        let exe_dir = exe_path.parent().unwrap();
        if let Ok(d) = exe_dir.join("assets").read_dir() {
            available_triginfo_paths = d
                .filter(|d| {
                    d.as_ref().unwrap().file_type().unwrap().is_file()
                        && d.as_ref()
                            .unwrap()
                            .file_name()
                            .to_os_string()
                            .to_string_lossy()
                            .to_lowercase()
                            .ends_with(".yml")
                })
                .map(|d| {
                    d.as_ref()
                        .unwrap()
                        .file_name()
                        .as_os_str()
                        .to_string_lossy()
                        .to_string()
                })
                .collect();
        }

        let mut trigger_icons = FxHashMap::default();
        for (name, data) in TRIGGER_ICON_DATA {
            let (img_data, info) = load_png_frame(data);
            trigger_icons.insert((*name).to_string(), unsafe {
                gl_helper::load_texture(
                    &gl,
                    info.width as i32,
                    info.height as i32,
                    &img_data,
                    glow::RGBA,
                    0,
                )
            });
        }

        let mut s = Self {
            file,
            ref_renderers,
            render_store,
            viewer: Arc::new(Mutex::new(BaseViewer::new(&gl))),
            sky_ent: String::new(),
            textfield_focused: false,
            vertex_lighting: true,
            global_lighting: true,
            native_lights: true,
            native_light_strength: 1.0,
            show_navmesh: true,
            navmesh_texture_scale: 1.0 / 16.0,
            show_triggers: true,
            show_sounds: true,
            show_runtime_path: true,
            animate_runtime_paths: true,
            native_runtime_event_gate: false,
            runtime_event_states: FxHashMap::default(),
            runtime_path_playback_speed: 1.0,
            platform_rotation_speed_scale: 1.0,
            runtime_motion_start_time: None,
            script_animation_start_time: None,
            animate_scripts: true,
            script_playback_speed: 1.0,
            billboard_renderer: Arc::new(BillboardRenderer::new(&gl).unwrap()),
            particle_renderer: Arc::new(ParticleRenderer::new(&gl).unwrap()),
            particle_settings: ParticlePreviewSettings::default(),
            link_renderer: Arc::new(LinkLineRenderer::new(&gl).unwrap()),
            select_renderer: Arc::new(SelectCubeRenderer::new(&gl).unwrap()),
            default_trigger_icon: *trigger_icons.get("default").unwrap(),
            selected_trigger: None,
            selected_sound: None,
            sound_preview,
            pickbuffer: PickBuffer::new(&gl),
            collision_renderer: Arc::new(CollisionDatumRenderer::new(&gl).unwrap()),
            gl: gl.clone(),
            selected_map: 0,
            trigger_scale: 0.5,
            sound_scale: 0.4,
            trigger_focus_tween: None,
            selected_link: None,
            trigger_info: Default::default(),
            selected_triginfo_path: format!("triggers_{game}.yml"),
            available_triginfo_paths,
            hashcodes,
            trigger_icons: Arc::new(trigger_icons),
            render_filter: RenderFilter::all(),
            global_lightmap: Arc::new(Mutex::new(None)),
        };

        if s.reload_trigger_defs().is_err() {
            s.selected_triginfo_path = "None".to_string();
        }

        s
    }

    fn reload_trigger_defs(&mut self) -> anyhow::Result<()> {
        let exe_path = std::env::current_exe().unwrap();
        let exe_dir = exe_path.parent().unwrap();
        let v = std::fs::read_to_string(
            exe_dir.join(format!("./assets/{}", self.selected_triginfo_path)),
        )
        .unwrap_or_else(|_| ROBOTS_TRIGGER_INFO.to_string());
        self.trigger_info =
            serde_yaml::from_str(&v).context("Failed to load trigger definition file")?;
        self.trigger_scale = self.trigger_info.icon_scale;

        info!(
            "Loaded {} trigger definitions from trigger file '{}'",
            self.trigger_info.triggers.len(),
            self.selected_triginfo_path
        );

        Ok(())
    }

    fn apply_entity_render_options(&self) {
        for (_, renderer) in &self.ref_renderers {
            let mut renderer = renderer.lock();
            renderer.vertex_lighting = self.vertex_lighting;
            renderer.navmesh_visible = self.show_navmesh;
            renderer.navmesh_texture_scale = self.navmesh_texture_scale;
        }

        let mut render_store = self.render_store.write();
        render_store.set_vertex_lighting(self.vertex_lighting);
        render_store.set_navmesh_options(self.show_navmesh, self.navmesh_texture_scale);
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        context: &egui::Context,
        maps: &[ProcessedMap],
    ) -> anyhow::Result<()> {
        self.selected_link = None;
        let previous_map = self.selected_map;
        ui.horizontal(|ui| -> anyhow::Result<()> {
            egui::ComboBox::from_label("Map")
                .selected_text({
                    let map = &maps[self.selected_map];
                    format!("{:x} ({} zones)", map.hashcode, map.mapzone_entities.len())
                })
                .show_ui(ui, |ui| {
                    for (i, m) in maps.iter().enumerate() {
                        ui.selectable_value(
                            &mut self.selected_map,
                            i,
                            format!("{:x} ({} zones)", m.hashcode, m.mapzone_entities.len()),
                        );
                    }
                });

            self.viewer.lock().show_toolbar(ui);
            Ok(())
        })
        .inner?;

        self.draw_map_controls(context, maps)?;

        if self.selected_map != previous_map {
            self.runtime_motion_start_time = None;
            self.runtime_event_states.clear();
            self.script_animation_start_time = None;
            self.selected_trigger = None;
            self.selected_sound = None;
            self.sound_preview
                .lock()
                .reset_group(SoundVoiceGroup::MapAmbient);
        }
        let map = &maps[self.selected_map];

        egui::Frame::canvas(ui.style()).show(ui, |ui| self.show_canvas(ui, context, map));

        ui.horizontal(|ui| {
            self.viewer.lock().show_statusbar(ui);
            if let Some(trig_id) = self.selected_trigger {
                ui.strong("Selected trigger:");
                if let Some(trigger) = map.triggers.get(trig_id) {
                    let type_name = self
                        .trigger_info
                        .triggers
                        .get(&trigger.ttype)
                        .map(|definition| definition.name.as_str())
                        .unwrap_or("Unknown trigger type");
                    ui.label(format!(
                        "#{trig_id} · {type_name} · type {}",
                        trigger.ttype
                    ));
                } else {
                    ui.label(format!("#{trig_id} · invalid index"));
                }
            }
            if let Some(sound_id) = self.selected_sound {
                ui.strong("Selected sound:");
                ui.label(format!("{}", sound_id));
            }
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        map_script_time, map_sky_objects, pickbuffer_pixel_position, QueuedEntityRender,
        ROBOTS_TRIGGER_INFO, TRIGGER_ICON_DATA,
    };
    use crate::map_runtime::{
        apply_vehicle_steering_wheel_angle, closest_route_phase, map_trigger_link_index,
        robots_vehicle_wheel_roll_angle, robots_vehicle_yaw_from_tangent,
        runtime_path_node_dispatches_between, runtime_path_route, runtime_path_segments,
        runtime_path_segments_for_motion, runtime_path_travel_distance, sample_route,
        RuntimeEventPreviewState, RuntimePathNodeEvent, ROBOTS_EVENT_ACTIVATE,
        ROBOTS_EVENT_DEACTIVATE,
    };
    use crate::render::{entity::EntityRenderer, RenderStore};
    use crate::maps::{ProcessedMap, ProcessedPath, ProcessedPathNode, ProcessedTrigger};
    use egui::{Pos2, Rect};
    use eurochef_edb::{map::EXGeoTriggerEngineOptions, versions::Platform};
    use eurochef_shared::{
        maps::TriggerInformation,
        script::{UXGeoScript, UXGeoScriptCommand, UXGeoScriptCommandData},
    };
    use glam::{Quat, Vec2, Vec3};

    fn path_node(position: Vec3) -> ProcessedPathNode {
        ProcessedPathNode {
            position,
            size: Vec2::ZERO,
            value: [0; 4],
            flags: 0,
            distance: 0.0,
            num_links: 0,
        }
    }

    fn runtime_trigger(trigger_type: u32, data: Vec<Option<u32>>) -> ProcessedTrigger {
        ProcessedTrigger {
            file_offset: 0,
            link_ref: -1,
            type_index: 0,
            ttype: trigger_type,
            tsubtype: None,
            debug: 0,
            game_flags: 0,
            trig_flags: 0,
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            scale: Vec3::ONE,
            data,
            links: vec![-1; 8],
            engine_options: EXGeoTriggerEngineOptions::default(),
            trigger_script: None,
            incoming_links: vec![],
        }
    }

    #[test]
    fn pickbuffer_coordinates_are_in_bounds_and_flip_y_without_an_off_by_one() {
        let rect = Rect::from_min_size(Pos2::new(10.0, 20.0), egui::vec2(100.0, 50.0));
        assert_eq!(
            pickbuffer_pixel_position(rect, Pos2::new(10.0, 20.0)),
            Some((0, 49))
        );
        assert_eq!(
            pickbuffer_pixel_position(rect, Pos2::new(109.9, 69.9)),
            Some((99, 0))
        );
        assert_eq!(
            pickbuffer_pixel_position(rect, Pos2::new(110.0, 30.0)),
            None
        );
    }

    #[test]
    fn runtime_path_segments_use_links_or_serialized_node_order() {
        let linked = ProcessedPath {
            hashcode: 0x0B00_0001,
            position: Vec3::new(10.0, 0.0, 0.0),
            flags: 0,
            path_type: 0,
            nodes: vec![
                path_node(Vec3::ZERO),
                path_node(Vec3::X),
                path_node(Vec3::Y),
            ],
            links: vec![(2, 0)],
        };
        assert_eq!(
            runtime_path_segments(&linked),
            vec![(Vec3::new(10.0, 1.0, 0.0), Vec3::new(10.0, 0.0, 0.0))]
        );
        assert_eq!(
            runtime_path_route(&linked),
            vec![Vec3::new(10.0, 1.0, 0.0), Vec3::new(10.0, 0.0, 0.0)]
        );

        let ordered = ProcessedPath {
            links: vec![],
            ..linked
        };
        assert_eq!(runtime_path_segments(&ordered).len(), 2);
    }

    #[test]
    fn runtime_motion_starts_at_nearest_route_phase_and_vehicle_yaw_follows_tangent() {
        let route = vec![
            Vec3::ZERO,
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 10.0),
        ];
        let segments = runtime_path_segments_for_motion(&route, false);
        let initial = Vec3::new(5.0, 2.0, 1.0);
        let (phase, root_offset) = closest_route_phase(&segments, initial);
        assert!((phase - 5.0).abs() < f32::EPSILON);

        let sample = sample_route(&segments, phase, false, root_offset).unwrap();
        assert!(sample.position.distance(initial) < 0.0001);
        assert_eq!(sample.tangent, Vec3::X);

        let yaw = robots_vehicle_yaw_from_tangent(Vec3::Z).unwrap();
        assert!((yaw.abs() - std::f32::consts::PI).abs() < 0.0001);
        assert_eq!(robots_vehicle_yaw_from_tangent(Vec3::Y), None);
    }

    #[test]
    fn runtime_motion_distance_matches_controller_update() {
        let accelerated = runtime_path_travel_distance(2.0 / 60.0, 4.0, 0.5, 1.0);
        assert!((accelerated - (2.0 / 60.0)).abs() < 0.0001);

        let default_start = runtime_path_travel_distance(2.0 / 60.0, 7.0, 0.0, 1.0);
        assert!((default_start - (8.0 / 60.0)).abs() < 0.0001);
        assert_eq!(runtime_path_travel_distance(0.0, 4.0, 0.5, 1.0), 0.0);
    }

    #[test]
    fn native_path_node_events_fire_only_when_the_node_is_crossed() {
        let path_hash = 0x0B00_0042;
        let mut stop_node = path_node(Vec3::new(5.0, 0.0, 0.0));
        stop_node.value = [4, 0, 0, 0];
        let mut linked_node = path_node(Vec3::new(10.0, 0.0, 0.0));
        linked_node.value = [8, 0x0100, 0b0000_0101, 0];
        let path = ProcessedPath {
            hashcode: path_hash,
            position: Vec3::ZERO,
            flags: 0,
            path_type: 0,
            nodes: vec![path_node(Vec3::ZERO), stop_node, linked_node],
            links: vec![],
        };
        let mut data = vec![None; 16];
        data[2] = Some(path_hash);
        data[5] = Some(10.0f32.to_bits());
        let platform = runtime_trigger(8, data);
        let map = ProcessedMap {
            hashcode: 1,
            mapzone_entities: vec![],
            zones: vec![],
            skies: vec![],
            placements: vec![],
            lights: vec![],
            sounds: vec![],
            lighting_triangles: vec![],
            paths: vec![path],
            triggers: vec![platform.clone()],
            trigger_collisions: vec![],
        };

        assert!(runtime_path_node_dispatches_between(&map, &platform, 0.0, 4.9).is_empty());
        assert_eq!(
            runtime_path_node_dispatches_between(&map, &platform, 4.9, 5.1)[0].event,
            RuntimePathNodeEvent::DeactivateSelf
        );
        assert_eq!(
            runtime_path_node_dispatches_between(&map, &platform, 9.9, 10.1)[0].event,
            RuntimePathNodeEvent::DispatchLinked {
                event_mask: 0x100,
                link_mask: 0b0000_0101,
            }
        );
        assert!(runtime_path_node_dispatches_between(&map, &platform, 5.1, 9.9).is_empty());
    }

    #[test]
    fn native_path_node_events_handle_ping_pong_reverse_arrival_once() {
        let path_hash = 0x0B00_0043;
        let mut event_node = path_node(Vec3::new(5.0, 0.0, 0.0));
        event_node.value = [4, 0, 0, 0];
        let path = ProcessedPath {
            hashcode: path_hash,
            position: Vec3::ZERO,
            flags: 0,
            path_type: 0,
            nodes: vec![
                path_node(Vec3::ZERO),
                event_node,
                path_node(Vec3::new(10.0, 0.0, 0.0)),
            ],
            links: vec![],
        };
        let mut data = vec![None; 16];
        data[2] = Some(path_hash);
        data[5] = Some(10.0f32.to_bits());
        let platform = runtime_trigger(8, data);
        let map = ProcessedMap {
            hashcode: 2,
            mapzone_entities: vec![],
            zones: vec![],
            skies: vec![],
            placements: vec![],
            lights: vec![],
            sounds: vec![],
            lighting_triangles: vec![],
            paths: vec![path],
            triggers: vec![platform.clone()],
            trigger_collisions: vec![],
        };

        assert_eq!(
            runtime_path_node_dispatches_between(&map, &platform, 4.9, 5.1).len(),
            1
        );
        assert_eq!(
            runtime_path_node_dispatches_between(&map, &platform, 14.9, 15.1).len(),
            1
        );
        assert!(runtime_path_node_dispatches_between(&map, &platform, 15.1, 15.2).is_empty());
    }

    #[test]
    fn native_runtime_event_gate_preserves_pause_and_platform_retrigger_continuity() {
        let mut data = vec![None; 16];
        data[5] = Some(10.0f32.to_bits());
        data[6] = Some(0.0f32.to_bits());
        data[7] = Some(0x200);
        let platform = runtime_trigger(8, data);
        let mut state = RuntimeEventPreviewState::default();

        assert!(!state.snapshot(&platform, 1.0).active);
        state.dispatch(&platform, ROBOTS_EVENT_ACTIVATE, 0.0, 1.0);
        state.advance(1.0);
        let running = state.snapshot(&platform, 1.0);
        assert!(running.active);
        assert!((running.elapsed_seconds - 1.0).abs() < 0.0001);
        assert!((running.path_distance - 1.0).abs() < 0.0001);

        state.dispatch(&platform, ROBOTS_EVENT_DEACTIVATE, 1.0, 1.0);
        state.advance(2.0);
        let paused = state.snapshot(&platform, 1.0);
        assert!(!paused.active);
        assert!((paused.elapsed_seconds - 1.0).abs() < 0.0001);
        assert!((paused.path_distance - running.path_distance).abs() < 0.0001);

        state.dispatch(&platform, ROBOTS_EVENT_ACTIVATE, 2.0, 1.0);
        state.advance(3.0);
        let before_reverse = state.snapshot(&platform, 1.0);
        state.dispatch(&platform, ROBOTS_EVENT_ACTIVATE, 3.0, 1.0);
        let after_reverse = state.snapshot(&platform, 1.0);
        assert!(after_reverse.direction_reversed);
        assert!((after_reverse.path_distance - before_reverse.path_distance).abs() < 0.0001);

        state.advance(4.0);
        let reversed_motion = state.snapshot(&platform, 1.0);
        assert!(reversed_motion.path_distance < after_reverse.path_distance);
    }

    #[test]
    fn native_runtime_event_activate_branch_wins_for_combined_mask() {
        let mut data = vec![None; 16];
        data[4] = Some(10);
        let lift = runtime_trigger(37, data);
        let mut state = RuntimeEventPreviewState::default();
        state.dispatch(
            &lift,
            ROBOTS_EVENT_ACTIVATE | ROBOTS_EVENT_DEACTIVATE,
            10.0,
            1.0,
        );
        assert!(state.snapshot(&lift, 1.0).active);
    }

    #[test]
    fn vehicle_wheel_roll_matches_runtime_sixty_hz_update() {
        let first_frame = robots_vehicle_wheel_roll_angle(1.0 / 60.0, 7.0, 0.0, 1.0);
        let expected_first = (-2.0 * (0.02f32).asin()).rem_euclid(std::f32::consts::TAU);
        assert!((first_frame - expected_first).abs() < 0.0001);

        let second_frame = robots_vehicle_wheel_roll_angle(2.0 / 60.0, 7.0, 0.0, 1.0);
        let expected_second =
            (-2.0 * (0.02f32).asin() - 2.0 * (0.14f32).asin()).rem_euclid(std::f32::consts::TAU);
        assert!((second_frame - expected_second).abs() < 0.0001);
        assert_eq!(robots_vehicle_wheel_roll_angle(0.0, 7.0, 0.0, 1.0), 0.0);
    }

    #[test]
    fn vehicle_steering_applies_to_drive_and_passive_wheel_records() {
        let file = 0x0100_00C1;
        let mut store = RenderStore::new();
        for (hashcode, index) in [
            (0x0200_017A, 0usize),
            (0x0200_017B, 1usize),
            (0x0200_01AE, 2usize),
        ] {
            store.insert_entity(
                file,
                hashcode,
                index,
                EntityRenderer::new(file, Platform::Pc),
            );
        }
        let mut queue = [
            QueuedEntityRender {
                entity: (file, 0x8200_0000),
                entity_alt: None,
                position: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            },
            QueuedEntityRender {
                entity: (file, 0x8200_0001),
                entity_alt: None,
                position: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            },
            QueuedEntityRender {
                entity: (file, 0x8200_0002),
                entity_alt: None,
                position: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            },
        ];
        let angle = 0.35;
        apply_vehicle_steering_wheel_angle(&mut queue, &store, angle);
        let expected = Quat::from_rotation_y(angle);
        assert!(queue[0].rotation.dot(expected).abs() > 0.99999);
        assert!(queue[1].rotation.dot(expected).abs() > 0.99999);
        assert!(queue[2].rotation.dot(Quat::IDENTITY).abs() > 0.99999);
    }

    #[test]
    fn vehicle_steering_wheel_uses_fixed_step_heading_delta_and_recenters_when_stopped() {
        let path_hash = 0x0B00_0080;
        let path = ProcessedPath {
            hashcode: path_hash,
            position: Vec3::ZERO,
            flags: 0,
            path_type: 0,
            nodes: vec![
                path_node(Vec3::ZERO),
                path_node(Vec3::new(0.0, 0.0, -0.75)),
                path_node(Vec3::new(-2.0, 0.0, -0.75)),
            ],
            links: vec![],
        };
        let mut data = vec![None; 16];
        data[1] = Some(path_hash);
        data[2] = Some(300.0f32.to_bits());
        data[3] = Some(10.0f32.to_bits());
        let vehicle = runtime_trigger(80, data);
        let map = ProcessedMap {
            hashcode: 80,
            mapzone_entities: vec![],
            zones: vec![],
            skies: vec![],
            placements: vec![],
            lights: vec![],
            sounds: vec![],
            lighting_triangles: vec![],
            paths: vec![path],
            triggers: vec![vehicle.clone()],
            trigger_collisions: vec![],
        };
        let mut state = RuntimeEventPreviewState::default();

        state.advance_runtime(&map, &vehicle, 0.0, 1.0);
        state.advance_runtime(&map, &vehicle, 2.0, 1.0);
        state.dispatch(&vehicle, ROBOTS_EVENT_ACTIVATE, 2.0, 1.0);
        state.advance_runtime(&map, &vehicle, 2.0 + 3.0 / 60.0, 1.0);

        let turning_snapshot = state.snapshot(&vehicle, 1.0);
        let turning = turning_snapshot.vehicle_steering_angle.unwrap();
        assert!(
            (turning - std::f32::consts::FRAC_PI_2).abs() < 0.001,
            "turning={turning}"
        );
        let carry = turning_snapshot.platform_contact_linear_velocity.unwrap();
        assert!(
            (carry - Vec3::new(-15.0, 0.0, -15.0)).length() < 0.001,
            "carry={carry:?}"
        );

        state.dispatch(&vehicle, ROBOTS_EVENT_DEACTIVATE, 2.0 + 3.0 / 60.0, 1.0);
        state.advance_runtime(&map, &vehicle, 3.0 + 3.0 / 60.0, 1.0);
        let stopped_snapshot = state.snapshot(&vehicle, 1.0);
        let stopped = stopped_snapshot.vehicle_steering_angle.unwrap();
        assert!(stopped.abs() < 0.01);
        assert_eq!(
            stopped_snapshot.platform_contact_linear_velocity,
            Some(Vec3::ZERO)
        );
    }

    #[test]
    fn trigger_link_indices_reject_negative_and_out_of_range_values() {
        assert_eq!(map_trigger_link_index(-1, 10), None);
        assert_eq!(map_trigger_link_index(-2, 10), None);
        assert_eq!(map_trigger_link_index(9, 10), Some(9));
        assert_eq!(map_trigger_link_index(10, 10), None);
    }

    #[test]
    fn map_sky_list_is_used_until_an_override_is_supplied() {
        let skies = [0x0200_017D, 0x0400_0123];

        assert_eq!(map_sky_objects("", &skies), skies);
        assert_eq!(map_sky_objects("not-hex", &skies), skies);
        assert_eq!(map_sky_objects("0200017e", &skies), [0x0200_017E]);
    }

    #[test]
    fn robots_trigger_icons_are_embedded_and_referenced_by_the_type_map() {
        let info: TriggerInformation = serde_yaml::from_str(ROBOTS_TRIGGER_INFO).unwrap();

        for definition in info.triggers.values() {
            if let Some(icon) = &definition.icon {
                assert!(
                    TRIGGER_ICON_DATA.iter().any(|(name, _)| name == icon),
                    "missing embedded icon {icon}"
                );
            }
        }
    }

    #[test]
    fn map_script_8400000a_loops_and_pauses_on_its_first_geometry_frame() {
        let script = UXGeoScript {
            hashcode: 0x8400_000A,
            framerate: 30.0,
            length: 7,
            num_threads: 1,
            commands: vec![UXGeoScriptCommand {
                opcode: 3,
                start: 2,
                length: 5,
                controller_header_index: 0,
                controller_index: 2,
                parent_controller_index: u8::MAX,
                data: UXGeoScriptCommandData::Entity {
                    hashcode: 0x8200_0001,
                    file: u32::MAX,
                },
            }],
            serialized_controller_count: 1,
            controller_record_metadata: vec![[0, 0]],
            controllers: vec![],
            controller_group_indices: vec![],
            controller_groups: vec![],
        };

        let paused = map_script_time(&script, 100.0, false, 1.0);
        assert!((paused - 2.0 / 30.0).abs() < f32::EPSILON);

        let duration = 7.0 / 30.0;
        let looped = map_script_time(&script, duration, true, 1.0);
        assert!(looped.abs() < f32::EPSILON);

        let half_speed = map_script_time(&script, 0.2, true, 0.5);
        assert!((half_speed - 0.1).abs() < f32::EPSILON);

        let mut sixty_fps = script.clone();
        sixty_fps.framerate = 60.0;
        sixty_fps.length = 120;
        assert!((map_script_time(&sixty_fps, 1.0, true, 1.0) - 1.0).abs() < f32::EPSILON);
        assert_eq!(sixty_fps.frame_at_time(1.0), 60.0);
        assert_eq!(sixty_fps.duration_seconds(), 2.0);
    }
}
