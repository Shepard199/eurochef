use std::{io::Cursor, sync::Arc};

use anyhow::Context;
use egui::{
    mutex::{Mutex, RwLock},
    Pos2, Rect, Vec2,
};
use eurochef_edb::{Hashcode, HashcodeUtils};
use eurochef_shared::{
    maps::{DefinitionDataType, TriggerInformation},
    script::{UXGeoScript, UXGeoScriptCommandData},
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
    map_zone::{robots_map_zone_contains, robots_map_zone_index_by_bounds},
    maps::{
        robots_camera_controller_plan, robots_camera_flags, robots_camera_marker_scaled_data0,
        robots_camera_mode, robots_camera_scaled_data4, robots_camera_scaled_data5,
        robots_dev_map_info, robots_direct_object_audio_profile, robots_monster_data15_value,
        robots_monster_data4_value, robots_monster_flags, robots_monster_is_family,
        robots_monster_proximity_radius, robots_monster_runtime_selector,
        robots_monster_test_runtime_value, robots_monster_transporter_secondary_path_hash,
        robots_native_light_colour, robots_native_light_type_description,
        robots_npc_alternate_cutscenes, robots_npc_cutscene_is_null, robots_npc_flags,
        robots_npc_runtime_selector, robots_npc_runtime_uid, robots_npc_text_group,
        robots_object_audio_is_consumer, robots_object_audio_is_enabled,
        robots_object_audio_profile_for_source, robots_pickup_visual, robots_portal_neighbor_zone,
        robots_trigger_path_data_slot, robots_trigger_path_hash, robots_trigger_path_is_proven,
        robots_trigger_platform_angular_velocity, robots_trigger_runtime_path_acceleration,
        robots_trigger_runtime_path_speed, robots_watchbot_enter_distance, robots_watchbot_flags,
        robots_watchbot_leave_distance, robots_watchbot_mode, ObjectAudioProfile, ProcessedMap,
        ProcessedTrigger,
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
    sky_diagnostic: String,

    /// Used to prevent keybinds being triggered while a textfield is focused
    textfield_focused: bool,

    vertex_lighting: bool,
    global_lighting: bool,
    native_lights: bool,
    native_light_strength: f32,
    show_navmesh: bool,
    show_flag_0x10_geometry: bool,
    navmesh_texture_scale: f32,
    show_triggers: bool,
    show_sounds: bool,
    show_runtime_path: bool,
    animate_runtime_paths: bool,
    native_runtime_event_gate: bool,
    runtime_event_states: FxHashMap<u64, RuntimeEventPreviewState>,
    active_camera_trigger: Option<usize>,
    preview_zone_background: bool,
    show_portals: bool,
    runtime_path_playback_speed: f32,
    platform_rotation_speed_scale: f32,
    runtime_motion_start_time: Option<f64>,
    script_animation_start_time: Option<f64>,
    animate_scripts: bool,
    script_playback_speed: f32,
    fan_runtime_value: i32,
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

#[derive(Clone, Copy, Debug, PartialEq)]
struct MapSkySelection {
    object: Hashcode,
    zone_index: Option<usize>,
    sky_index: Option<usize>,
    contains_camera: bool,
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
    (
        "xtrigger_alerticon",
        include_bytes!("../../../assets/icons/triggers/XTrigger_AlertIcon.png"),
    ),
    (
        "xtrigger_camera",
        include_bytes!("../../../assets/icons/triggers/XTrigger_Camera.png"),
    ),
    (
        "xtrigger_camera_values",
        include_bytes!("../../../assets/icons/triggers/XTrigger_Camera_Values.png"),
    ),
    (
        "xtrigger_changelevel",
        include_bytes!("../../../assets/icons/triggers/XTrigger_ChangeLevel.png"),
    ),
    (
        "xtrigger_cutscene",
        include_bytes!("../../../assets/icons/triggers/XTrigger_Cutscene.png"),
    ),
    (
        "xtrigger_displaymessage",
        include_bytes!("../../../assets/icons/triggers/XTrigger_DisplayMessage.png"),
    ),
    (
        "xtrigger_distance",
        include_bytes!("../../../assets/icons/triggers/XTrigger_Distance.png"),
    ),
    (
        "xtrigger_load",
        include_bytes!("../../../assets/icons/triggers/XTrigger_Load.png"),
    ),
    (
        "xtrigger_mission",
        include_bytes!("../../../assets/icons/triggers/XTrigger_Mission.png"),
    ),
    (
        "xtrigger_monster",
        include_bytes!("../../../assets/icons/triggers/XTrigger_Monster.png"),
    ),
    (
        "xtrigger_npc",
        include_bytes!("../../../assets/icons/triggers/XTrigger_NPC.png"),
    ),
    (
        "xtrigger_objectaudio",
        include_bytes!("../../../assets/icons/triggers/XTrigger_ObjectAudio.png"),
    ),
    (
        "xtrigger_player",
        include_bytes!("../../../assets/icons/triggers/XTrigger_Player.png"),
    ),
    (
        "xtrigger_script",
        include_bytes!("../../../assets/icons/triggers/XTrigger_Script.png"),
    ),
    (
        "xtrigger_tutorial",
        include_bytes!("../../../assets/icons/triggers/XTrigger_Tutorial.png"),
    ),
    (
        "xtrigger_camera_marker",
        include_bytes!("../../../assets/icons/triggers/XTrigger_Camera_Marker.png"),
    ),
    (
        "xtrigger_door",
        include_bytes!("../../../assets/icons/triggers/XTrigger_Door.png"),
    ),
    (
        "xtrigger_interact",
        include_bytes!("../../../assets/icons/triggers/XTrigger_Interact.png"),
    ),
    (
        "xtrigger_slideunder",
        include_bytes!("../../../assets/icons/triggers/XTrigger_SlideUnder.png"),
    ),
];
const ROBOTS_TRIGGER_INFO: &str = include_str!("../../../assets/triggers_robots.yml");

fn map_sky_selection(
    sky_override: &str,
    skies: &[Hashcode],
    zone_skies: &[(Vec3, Vec3, i32)],
    camera_position: Vec3,
) -> Option<MapSkySelection> {
    if let Ok(sky) = u32::from_str_radix(sky_override.trim(), 16) {
        return Some(MapSkySelection {
            object: sky,
            zone_index: None,
            sky_index: None,
            contains_camera: false,
        });
    }

    let zone_index = robots_map_zone_index_by_bounds(zone_skies.len(), camera_position, |index| {
        (zone_skies[index].0, zone_skies[index].1)
    })?;
    let (bounds_min, bounds_max, serialized_sky_index) = zone_skies[zone_index];
    let sky_index = usize::try_from(serialized_sky_index).ok()?;
    let object = *skies.get(sky_index)?;

    Some(MapSkySelection {
        object,
        zone_index: Some(zone_index),
        sky_index: Some(sky_index),
        // Zone zero is the unconditional fallback in 0x004E9E71 and is never
        // tested against the point before later zones.
        contains_camera: zone_index != 0
            && robots_map_zone_contains(bounds_min, bounds_max, camera_position),
    })
}

fn map_sky_background_fallback(
    skies: &[Hashcode],
    selection: Option<MapSkySelection>,
) -> Option<Hashcode> {
    selection
        .is_none()
        .then(|| skies.first().copied())
        .flatten()
}

const ROBOTS_NATIVE_SCALED_SKY_FACTOR: f32 = f32::from_bits(0x3FD8_51E6);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MapSkyEntityClass {
    CameraRelative,
    NativeScaledCameraRelative,
    WorldSpace,
}

fn map_sky_entity_class(entity_flags: u32, base_sky_root: bool) -> MapSkyEntityClass {
    if entity_flags & 0x10 != 0 {
        MapSkyEntityClass::CameraRelative
    } else if base_sky_root && entity_flags & 0x100 != 0 {
        MapSkyEntityClass::NativeScaledCameraRelative
    } else {
        MapSkyEntityClass::WorldSpace
    }
}

/// `EXGeoMap.skies` mixes camera-relative background members with map-space
/// facade/decor geometry. Object-level flag 0x10 keeps the inherited camera
/// translation. The root member of the first/base sky source may additionally
/// use flag 0x100 for the runtime-proven scaled camera-relative matrix (City
/// 0x84000019 -> 0x82000030). Roots of later zone Scripts and later members of
/// the base Script remain map-space assemblies (City 0x8200002E/2F/98).
fn map_sky_entity_transform(
    camera_position: Vec3,
    scripted_position: Vec3,
    scripted_scale: Vec3,
    entity_flags: u32,
    base_sky_root: bool,
) -> (Vec3, Vec3, MapSkyEntityClass) {
    let class = map_sky_entity_class(entity_flags, base_sky_root);
    match class {
        MapSkyEntityClass::CameraRelative => (scripted_position, scripted_scale, class),
        MapSkyEntityClass::NativeScaledCameraRelative => (
            scripted_position,
            scripted_scale * ROBOTS_NATIVE_SCALED_SKY_FACTOR,
            class,
        ),
        MapSkyEntityClass::WorldSpace => {
            (scripted_position - camera_position, scripted_scale, class)
        }
    }
}

#[cfg(test)]
fn map_sky_objects(
    sky_override: &str,
    skies: &[Hashcode],
    zone_skies: &[(Vec3, Vec3, i32)],
    camera_position: Vec3,
) -> Vec<Hashcode> {
    map_sky_selection(sky_override, skies, zone_skies, camera_position)
        .map(|selection| selection.object)
        .into_iter()
        .collect()
}

fn robots_infinite_script_loop(script: &UXGeoScript) -> Option<(f32, f32)> {
    script.commands.iter().find_map(|command| {
        let UXGeoScriptCommandData::Unknown { cmd: 16, data } = &command.data else {
            return None;
        };
        if data.len() < 12 {
            return None;
        }

        let mode = u32::from_le_bytes(data[0..4].try_into().ok()?);
        let repeat_count = i32::from_le_bytes(data[4..8].try_into().ok()?);
        let target_frame = u32::from_le_bytes(data[8..12].try_into().ok()?) as f32;
        let loop_frame = command.start.max(0) as f32;
        (mode == 1
            && repeat_count == -1
            && target_frame < loop_frame
            && loop_frame <= script.length as f32)
            .then_some((target_frame, loop_frame))
    })
}

fn map_script_time(
    script: &UXGeoScript,
    global_time: f32,
    animate: bool,
    playback_speed: f32,
    paused_time: Option<f32>,
) -> f32 {
    if animate {
        let elapsed_time = global_time.max(0.0) * playback_speed.max(0.0);
        if let Some((target_frame, loop_frame)) = robots_infinite_script_loop(script) {
            let elapsed_frame = script.frame_at_time(elapsed_time);
            const LOOP_FRAME_EPSILON: f32 = 1.0e-4;
            let frame = if elapsed_frame + LOOP_FRAME_EPSILON < loop_frame {
                elapsed_frame
            } else {
                let cycle_frames = loop_frame - target_frame;
                let elapsed_in_cycle = (elapsed_frame - loop_frame).max(0.0);
                target_frame + elapsed_in_cycle.rem_euclid(cycle_frames)
            };
            script.time_at_frame(frame)
        } else {
            let duration = script.duration_seconds().max(1.0 / 60.0);
            elapsed_time.rem_euclid(duration)
        }
    } else {
        paused_time.unwrap_or_else(|| {
            script.time_at_frame(script.first_geometry_frame().unwrap_or(0).max(0) as f32)
        })
    }
}

fn resolved_map_script_time(
    render_store: &RenderStore,
    file: Hashcode,
    script_hashcode: Hashcode,
    global_time: f32,
    animate: bool,
    playback_speed: f32,
) -> f32 {
    render_store
        .get_script(file, script_hashcode)
        .map(|script| {
            map_script_time(
                script,
                global_time,
                animate,
                playback_speed,
                crate::render::script::first_resolved_visual_time(
                    file,
                    script_hashcode,
                    render_store,
                ),
            )
        })
        .unwrap_or_default()
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
mod script_sound;
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
            sky_diagnostic: String::new(),
            textfield_focused: false,
            vertex_lighting: true,
            global_lighting: true,
            native_lights: true,
            native_light_strength: 1.0,
            show_navmesh: true,
            show_flag_0x10_geometry: true,
            navmesh_texture_scale: 1.0 / 16.0,
            show_triggers: true,
            show_sounds: true,
            show_runtime_path: true,
            animate_runtime_paths: true,
            native_runtime_event_gate: true,
            runtime_event_states: FxHashMap::default(),
            active_camera_trigger: None,
            preview_zone_background: false,
            show_portals: false,
            runtime_path_playback_speed: 1.0,
            platform_rotation_speed_scale: 1.0,
            runtime_motion_start_time: None,
            script_animation_start_time: None,
            animate_scripts: true,
            script_playback_speed: 1.0,
            fan_runtime_value: 0,
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
            renderer.show_hidden_geometry = self.show_flag_0x10_geometry;
            renderer.navmesh_visible = self.show_navmesh;
            renderer.navmesh_texture_scale = self.navmesh_texture_scale;
        }

        let mut render_store = self.render_store.write();
        render_store.set_vertex_lighting(self.vertex_lighting);
        render_store.set_flag_0x10_geometry_visible(self.show_flag_0x10_geometry);
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
            let mut preview = self.sound_preview.lock();
            preview.reset_group(SoundVoiceGroup::MapAmbient);
            preview.reset_group(SoundVoiceGroup::ObjectAudio);
            preview.reset_group(SoundVoiceGroup::MapScript);
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
                    ui.label(format!("#{trig_id} · {type_name} · type {}", trigger.ttype));
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
        map_script_time, map_sky_background_fallback, map_sky_entity_transform, map_sky_objects,
        pickbuffer_pixel_position, MapSkyEntityClass, QueuedEntityRender, ROBOTS_TRIGGER_INFO,
        TRIGGER_ICON_DATA,
    };
    use crate::map_runtime::{
        apply_vehicle_steering_wheel_angle, closest_route_phase, map_trigger_link_index,
        robots_vehicle_wheel_roll_angle, robots_vehicle_yaw_from_tangent,
        runtime_path_node_dispatches_between, runtime_path_route, runtime_path_segments,
        runtime_path_segments_for_motion, runtime_path_travel_distance, sample_route,
        RuntimeEventPreviewState, RuntimePathNodeEvent, ROBOTS_EVENT_ACTIVATE,
        ROBOTS_EVENT_DEACTIVATE,
    };
    use crate::maps::{ProcessedMap, ProcessedPath, ProcessedPathNode, ProcessedTrigger};
    use crate::render::{entity::EntityRenderer, RenderStore};
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
            character_visual: None,
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
    fn map_sky_separates_scaled_root_members_from_later_world_assemblies() {
        let camera = Vec3::new(100.0, 20.0, -40.0);
        let command_translation = Vec3::new(-53.0, -5.0, 18.0);
        let scripted = camera + command_translation;
        let scripted_scale = Vec3::new(2.0, 3.0, 4.0);

        let (position, scale, class) =
            map_sky_entity_transform(camera, scripted, scripted_scale, 0x10, false);
        assert_eq!(class, MapSkyEntityClass::CameraRelative);
        assert_eq!(position, scripted);
        assert_eq!(scale, scripted_scale);

        let (position, scale, class) =
            map_sky_entity_transform(camera, scripted, scripted_scale, 0x300, true);
        assert_eq!(class, MapSkyEntityClass::NativeScaledCameraRelative);
        assert_eq!(position, scripted);
        assert_eq!(
            scale,
            scripted_scale * super::ROBOTS_NATIVE_SCALED_SKY_FACTOR
        );

        for flags in [0, 0x100, 0x300, 0x4000_0000] {
            let (position, scale, class) =
                map_sky_entity_transform(camera, scripted, scripted_scale, flags, false);
            assert_eq!(class, MapSkyEntityClass::WorldSpace, "flags=0x{flags:08X}");
            assert_eq!(position, command_translation);
            assert_eq!(scale, scripted_scale);
        }
    }

    #[test]
    fn map_sky_missing_zone_assembly_keeps_first_background_source() {
        let skies = [0x8400_000D, 0x8400_000C];
        assert_eq!(map_sky_background_fallback(&skies, None), Some(0x8400_000D));
        assert_eq!(
            map_sky_background_fallback(
                &skies,
                Some(super::MapSkySelection {
                    object: 0x8400_000C,
                    zone_index: Some(29),
                    sky_index: Some(1),
                    contains_camera: true,
                }),
            ),
            None
        );
        assert_eq!(map_sky_background_fallback(&[], None), None);
    }

    #[test]
    fn map_sky_uses_the_first_serialized_matching_zone_and_preserves_override() {
        let skies = [0x8400_0019, 0x8400_0017, 0x8400_0035, 0x8400_0018];
        let zones = [
            (Vec3::splat(-100.0), Vec3::splat(100.0), 0),
            (Vec3::splat(-10.0), Vec3::splat(10.0), 1),
            (Vec3::splat(-1.0), Vec3::splat(1.0), 3),
            (Vec3::splat(20.0), Vec3::splat(22.0), 2),
        ];

        assert_eq!(
            map_sky_objects("", &skies, &zones, Vec3::ZERO),
            [0x8400_0017]
        );
        assert_eq!(
            map_sky_objects("not-hex", &skies, &zones, Vec3::splat(21.0)),
            [0x8400_0035]
        );
        assert_eq!(
            map_sky_objects("0200017e", &skies, &zones, Vec3::ZERO),
            [0x0200_017E]
        );
    }

    #[test]
    fn map_sky_uses_zone_zero_outside_all_non_default_bounds() {
        let skies = [0x8400_0019, 0x8400_0017];
        let zones = [
            (Vec3::splat(100.0), Vec3::splat(102.0), 1),
            (Vec3::splat(10.0), Vec3::splat(12.0), 0),
        ];

        assert_eq!(
            map_sky_objects("", &skies, &zones, Vec3::ZERO),
            [0x8400_0017]
        );
    }

    #[test]
    fn map_sky_no_sky_zone_suppresses_parent_facade() {
        let skies = [0x8400_0019];
        let zones = [
            (Vec3::splat(-10.0), Vec3::splat(10.0), 0),
            (Vec3::splat(-1.0), Vec3::splat(1.0), -1),
        ];

        assert!(map_sky_objects("", &skies, &zones, Vec3::ZERO).is_empty());
    }

    #[test]
    fn map_sky_invalid_selected_index_does_not_fall_back() {
        let skies = [0x8400_0019];
        let zones = [
            (Vec3::splat(-1.0), Vec3::splat(1.0), 7),
            (Vec3::splat(20.0), Vec3::splat(22.0), 0),
        ];

        assert!(map_sky_objects("", &skies, &zones, Vec3::ZERO).is_empty());
        assert!(map_sky_objects("", &skies, &[], Vec3::ZERO).is_empty());
    }

    #[test]
    fn map_sky_outside_bounds_does_not_use_nearest_valid_sky_zone() {
        let skies = [0x8400_0019];
        let zones = [
            (Vec3::splat(100.0), Vec3::splat(102.0), -1),
            (Vec3::splat(20.0), Vec3::splat(22.0), 0),
        ];

        assert!(map_sky_objects("", &skies, &zones, Vec3::ZERO).is_empty());
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

        let expected = [
            (0, "xtrigger_player"),
            (1, "xtrigger_camera"),
            (2, "xtrigger_distance"),
            (3, "xtrigger_monster"),
            (4, "xtrigger_script"),
            (10, "xtrigger_monster"),
            (11, "xtrigger_monster"),
            (15, "xtrigger_changelevel"),
            (16, "xtrigger_load"),
            (18, "xtrigger_monster"),
            (19, "xtrigger_cutscene"),
            (20, "xtrigger_camera_marker"),
            (21, "xtrigger_door"),
            (22, "xtrigger_interact"),
            (23, "xtrigger_interact"),
            (24, "xtrigger_interact"),
            (33, "xtrigger_monster"),
            (35, "xtrigger_camera_values"),
            (39, "xtrigger_displaymessage"),
            (48, "xtrigger_npc"),
            (49, "xtrigger_interact"),
            (53, "xtrigger_mission"),
            (59, "xtrigger_slideunder"),
            (61, "xtrigger_alerticon"),
            (70, "xtrigger_monster"),
            (73, "xtrigger_monster"),
            (74, "xtrigger_monster"),
            (78, "xtrigger_tutorial"),
            (79, "xtrigger_objectaudio"),
            (90, "xtrigger_load"),
        ];

        for (trigger_type, icon) in expected {
            assert_eq!(
                info.triggers
                    .get(&trigger_type)
                    .and_then(|definition| definition.icon.as_deref()),
                Some(icon),
                "wrong icon for serialized trigger type {trigger_type}"
            );
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

        let paused = map_script_time(&script, 100.0, false, 1.0, None);
        assert!((paused - 2.0 / 30.0).abs() < f32::EPSILON);

        let duration = 7.0 / 30.0;
        let looped = map_script_time(&script, duration, true, 1.0, None);
        assert!(looped.abs() < f32::EPSILON);

        let half_speed = map_script_time(&script, 0.2, true, 0.5, None);
        assert!((half_speed - 0.1).abs() < f32::EPSILON);

        let mut sixty_fps = script.clone();
        sixty_fps.framerate = 60.0;
        sixty_fps.length = 120;
        assert!((map_script_time(&sixty_fps, 1.0, true, 1.0, None) - 1.0).abs() < f32::EPSILON);
        assert_eq!(sixty_fps.frame_at_time(1.0), 60.0);
        assert_eq!(sixty_fps.duration_seconds(), 2.0);
    }

    #[test]
    fn map_script_uses_the_native_infinite_loop_target_and_boundary() {
        let mut script = UXGeoScript {
            hashcode: 0x8400_0037,
            framerate: 30.0,
            length: 601,
            num_threads: 1,
            commands: vec![
                UXGeoScriptCommand {
                    opcode: 3,
                    start: 0,
                    length: 601,
                    controller_header_index: 0,
                    controller_index: 0,
                    parent_controller_index: u8::MAX,
                    data: UXGeoScriptCommandData::Entity {
                        hashcode: 0x8200_009E,
                        file: u32::MAX,
                    },
                },
                UXGeoScriptCommand {
                    opcode: 16,
                    start: 598,
                    length: 601,
                    controller_header_index: u16::MAX,
                    controller_index: u8::MAX,
                    parent_controller_index: u8::MAX,
                    data: UXGeoScriptCommandData::Unknown {
                        cmd: 16,
                        data: [
                            1_u32.to_le_bytes(),
                            u32::MAX.to_le_bytes(),
                            1_u32.to_le_bytes(),
                        ]
                        .concat(),
                    },
                },
            ],
            serialized_controller_count: 1,
            controller_record_metadata: vec![[0, 0]],
            controllers: vec![],
            controller_group_indices: vec![],
            controller_groups: vec![],
        };

        let at_597 = map_script_time(&script, 597.0 / 30.0, true, 1.0, None);
        let at_jump = map_script_time(&script, 598.0 / 30.0, true, 1.0, None);
        let after_jump = map_script_time(&script, 600.0 / 30.0, true, 1.0, None);
        assert!((script.frame_at_time(at_597) - 597.0).abs() < 1.0e-4);
        assert!((script.frame_at_time(at_jump) - 1.0).abs() < 1.0e-4);
        assert!((script.frame_at_time(after_jump) - 3.0).abs() < 1.0e-4);

        script.commands[1].data = UXGeoScriptCommandData::Unknown {
            cmd: 16,
            data: [
                1_u32.to_le_bytes(),
                u32::MAX.to_le_bytes(),
                0_u32.to_le_bytes(),
            ]
            .concat(),
        };
        script.commands[1].start = 1000;
        script.length = 1001;
        let at_second_loop_jump = map_script_time(&script, 1000.0 / 30.0, true, 1.0, None);
        assert!(script.frame_at_time(at_second_loop_jump).abs() < 1.0e-4);
    }
}
