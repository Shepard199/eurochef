use std::sync::Arc;

use egui::{
    mutex::{Mutex, RwLock},
    RichText,
};
use eurochef_edb::{Hashcode, HashcodeUtils};
use eurochef_shared::{
    maps::format_hashcode,
    script::{
        robots_script_command_role, robots_script_payload_diagnostic, UXGeoScript,
        UXGeoScriptCommandData,
    },
};
use glam::{Quat, Vec3};
use glow::HasContext;
use instant::Instant;
use nohash_hasher::IntMap;
use std::fmt::Write;

use crate::{
    animations::AnimationRuntimeStatus,
    map_frame::QueuedEntityRender,
    render::{
        particle::{ParticlePreviewSettings, ParticleRenderer},
        script::{
            collect_script_animations, collect_script_particles,
            render_script_without_static_animations, render_static_script_without_animations,
        },
        viewer::BaseViewer,
        RenderStore,
    },
    sound_preview::SharedSoundPreview,
};

pub(crate) mod fan;
pub(crate) mod sound;
use fan::{
    advance_native_fan_angle, apply_native_fan_rotation, script_contains_native_fan_entity,
};

// ROBOTS_PATCH_0020_SCRIPT_LOCAL_REFERENCE_LABELS
// Local script object hashes encode an index in the current EDB. They are not
// failed global HashDB lookups, so do not display them as HT_Local_Invalid_*.
fn format_script_object_reference(
    hashcodes: &IntMap<Hashcode, String>,
    hashcode: Hashcode,
    resolved_hashcode: Option<Hashcode>,
) -> String {
    if hashcode.is_local() {
        if let Some(resolved) = resolved_hashcode {
            format!(
                "{} [local#{} 0x{hashcode:08x} -> 0x{resolved:08x}]",
                format_hashcode(hashcodes, resolved),
                hashcode.index(),
            )
        } else {
            format!("local#{} [0x{hashcode:08x}, unresolved]", hashcode.index())
        }
    } else {
        format_hashcode(hashcodes, hashcode)
    }
}

fn script_object_file_for_display(hashcode: Hashcode, declared_file: Hashcode) -> Hashcode {
    if hashcode.is_local() {
        u32::MAX
    } else {
        declared_file
    }
}

fn semantic_object_reference(
    hashcodes: &IntMap<Hashcode, String>,
    kind: &str,
    hashcode: Hashcode,
) -> String {
    if hashcode == u32::MAX {
        return format!("implicit {kind} binding");
    }
    if hashcode.is_local() {
        format!("{kind} #{}", hashcode.index())
    } else {
        format_hashcode(hashcodes, hashcode)
    }
}

fn semantic_script_label(
    hashcodes: &IntMap<Hashcode, String>,
    index: usize,
    script: &UXGeoScript,
) -> String {
    if !script.hashcode.is_local() {
        let name = format_hashcode(hashcodes, script.hashcode);
        if !name.contains("_Unknown_") && !name.contains("HT_Invalid") {
            return name;
        }
    }

    let prefix = format!("Script #{index}");
    if let [command] = script.commands.as_slice() {
        let role = match &command.data {
            UXGeoScriptCommandData::Animation {
                anim_hashcode,
                skin_hashcode,
                ..
            } => format!(
                "single {} with {}",
                semantic_object_reference(hashcodes, "Animation", *anim_hashcode),
                semantic_object_reference(hashcodes, "AnimSkin", *skin_hashcode)
            ),
            UXGeoScriptCommandData::Entity { hashcode, .. } => format!(
                "single {}",
                semantic_object_reference(hashcodes, "Entity", *hashcode)
            ),
            UXGeoScriptCommandData::SubScript { hashcode, .. } => format!(
                "single {}",
                semantic_object_reference(hashcodes, "SubScript", *hashcode)
            ),
            UXGeoScriptCommandData::Particle { hashcode, .. } => format!(
                "single {}",
                semantic_object_reference(hashcodes, "Particle", *hashcode)
            ),
            UXGeoScriptCommandData::Sound { hashcode } => format!(
                "single {}",
                semantic_object_reference(hashcodes, "Sound", *hashcode)
            ),
            UXGeoScriptCommandData::Event { event_type, .. } => format!(
                "single {}",
                semantic_object_reference(hashcodes, "Event", *event_type)
            ),
            UXGeoScriptCommandData::Unknown { cmd, .. } => {
                format!("single native opcode 0x{cmd:02X}")
            }
        };
        return format!("{prefix} · {role} · {} frames", command.length);
    }

    let counts = script.command_type_counts();
    format!(
        "{prefix} · Entity {} · Animation {} · SubScript {} · Particle {} · Sound {} · Event {} · Unknown {}",
        counts.entities,
        counts.animations,
        counts.subscripts,
        counts.particles,
        counts.sounds,
        counts.events,
        counts.unknown
    )
}

#[derive(Debug, Clone, Copy, Default)]
struct AnimationRuntimeSummary {
    active: usize,
    rendered: usize,
    missing_runtime: usize,
    missing_animation: usize,
    missing_skin: usize,
    skin_mismatch: usize,
    missing_pose_cache: usize,
    invalid_pose: usize,
    missing_geometry: usize,
}

impl AnimationRuntimeSummary {
    fn failure_description(self) -> String {
        let mut reasons = Vec::new();
        for (count, label) in [
            (self.missing_runtime, "runtime not loaded"),
            (self.missing_animation, "animation unresolved"),
            (self.missing_skin, "AnimSkin unresolved"),
            (self.skin_mismatch, "Animation/AnimSkin mismatch"),
            (self.missing_pose_cache, "RAPCV002 cache missing"),
            (self.invalid_pose, "native pose invalid"),
            (self.missing_geometry, "component geometry missing"),
        ] {
            if count > 0 {
                reasons.push(format!("{label}: {count}"));
            }
        }
        reasons.join(", ")
    }
}

pub struct ScriptListPanel {
    file: Hashcode,
    scripts: IntMap<Hashcode, (usize, UXGeoScript)>,
    selected_script: Hashcode,
    viewer: Arc<Mutex<BaseViewer>>,
    hashcodes: Arc<IntMap<Hashcode, String>>,
    render_store: Arc<RwLock<RenderStore>>,
    sound_preview: SharedSoundPreview,

    current_time: f32,
    playback_speed: f32,
    is_playing: bool,
    loop_script: bool,
    show_full_assembly: bool,
    particle_renderer: Arc<ParticleRenderer>,
    particle_settings: ParticlePreviewSettings,
    fan_runtime_value: i32,
    fan_runtime_angle: f32,

    last_frame: Instant,
    last_audio_script: Hashcode,
    last_audio_time: f32,
}

impl ScriptListPanel {
    pub fn new(
        file: Hashcode,
        gl: &glow::Context,
        scripts: Vec<UXGeoScript>,
        render_store: Arc<RwLock<RenderStore>>,
        hashcodes: Arc<IntMap<Hashcode, String>>,
        sound_preview: SharedSoundPreview,
    ) -> Self {
        let selected_script = scripts.first().map(|s| s.hashcode).unwrap_or(u32::MAX);
        let current_time = scripts
            .first()
            .and_then(|script| {
                script
                    .first_visual_frame()
                    .map(|frame| script.time_at_frame(frame.max(0) as f32))
            })
            .unwrap_or(0.0);

        Self {
            file,
            selected_script,
            scripts: scripts
                .into_iter()
                .enumerate()
                .map(|(i, s)| (s.hashcode, (i, s)))
                .collect(),
            viewer: Arc::new(Mutex::new(BaseViewer::new(gl))),
            render_store,
            hashcodes,
            sound_preview,
            current_time,
            playback_speed: 1.0,
            is_playing: false,
            loop_script: false,
            show_full_assembly: false,
            particle_renderer: Arc::new(ParticleRenderer::new(gl).unwrap()),
            particle_settings: ParticlePreviewSettings::default(),
            fan_runtime_value: 50,
            fan_runtime_angle: 0.0,
            last_frame: Instant::now(),
            last_audio_script: selected_script,
            last_audio_time: current_time,
        }
    }

    fn current_script(&self) -> Option<&UXGeoScript> {
        self.scripts.get(&self.selected_script).map(|(_, v)| v)
    }

    fn thread_count(&self) -> isize {
        self.current_script()
            .map(|v| {
                v.commands
                    .iter()
                    .map(|c| {
                        if let UXGeoScriptCommandData::Unknown { cmd, .. } = c.data {
                            if cmd == 0x10 || cmd == 0x11 || cmd == 0x12 {
                                return 0;
                            }
                        }

                        if c.controller_index == u8::MAX {
                            0
                        } else {
                            c.controller_index as isize + 1
                        }
                    })
                    .max()
                    .unwrap_or_default()
            })
            .unwrap_or(0)
    }

    fn animation_runtime_summary(&self) -> AnimationRuntimeSummary {
        let store = self.render_store.read();
        let mut queue = Vec::new();
        collect_script_animations(
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::ONE,
            self.file,
            self.selected_script,
            self.current_time,
            &store,
            &mut queue,
            vec![],
            self.show_full_assembly,
        );
        let mut summary = AnimationRuntimeSummary {
            active: queue.len(),
            ..Default::default()
        };
        for animation in queue {
            let Some(runtime) = store.get_animation_runtime(animation.animation.0) else {
                summary.missing_runtime += 1;
                continue;
            };
            match runtime.status(animation.animation.1, animation.skin.1) {
                AnimationRuntimeStatus::Rendered => summary.rendered += 1,
                AnimationRuntimeStatus::MissingAnimation => summary.missing_animation += 1,
                AnimationRuntimeStatus::MissingSkin => summary.missing_skin += 1,
                AnimationRuntimeStatus::SkinMismatch => summary.skin_mismatch += 1,
                AnimationRuntimeStatus::MissingPoseCache => summary.missing_pose_cache += 1,
                AnimationRuntimeStatus::InvalidPose => summary.invalid_pose += 1,
                AnimationRuntimeStatus::MissingGeometry => summary.missing_geometry += 1,
            }
        }
        summary
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let delta_time = self.last_frame.elapsed().as_secs_f32();
        self.last_frame = Instant::now();
        let native_fan_script = self
            .current_script()
            .map(|script| {
                script_contains_native_fan_entity(script, self.file, &self.render_store.read())
            })
            .unwrap_or(false);
        if self.is_playing && native_fan_script {
            self.fan_runtime_angle = advance_native_fan_angle(
                self.fan_runtime_angle,
                delta_time,
                self.fan_runtime_value,
                self.playback_speed,
            );
        }

        egui::CollapsingHeader::new("EuroSound preview")
            .default_open(false)
            .show(ui, |ui| self.sound_preview.lock().draw_settings(ui));

        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                egui::ScrollArea::vertical()
                    .id_salt("script_scroll_area")
                    .show(ui, |ui| {
                        for i in 0..self.scripts.len() {
                            if let Some((hc, (_, script))) =
                                self.scripts.iter().find(|(_, (idx, _))| *idx == i)
                            {
                                let label = semantic_script_label(&self.hashcodes, i, script);
                                if ui
                                    .selectable_value(
                                        &mut self.selected_script,
                                        *hc,
                                        format!("{label}  [0x{hc:08X}]"),
                                    )
                                    .clicked()
                                {
                                    self.current_time = script
                                        .first_visual_frame()
                                        .map(|frame| script.time_at_frame(frame.max(0) as f32))
                                        .unwrap_or(0.0);
                                    self.is_playing = false;
                                    self.fan_runtime_angle = 0.0;
                                }
                            }
                        }
                    });
            });

            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    self.viewer.lock().show_toolbar(ui);
                    ui.add(
                        egui::DragValue::new(&mut self.playback_speed)
                            .range(0.05..=3.0)
                            .speed(0.01),
                    );
                    ui.label("Speed");
                    ui.separator();
                    ui.checkbox(&mut self.show_full_assembly, "Full assembly");
                    ui.checkbox(&mut self.particle_settings.enabled, "Native Particles")
                        .on_hover_text("Uses EXParticleSys rate, pool, lifetime, emitter bounds, velocity, acceleration, damping, resource selection and appended colour/scale/rotation curves.");
                });

                egui::Frame::canvas(ui.style()).show(ui, |ui| self.show_canvas(ui));

                let animation_runtime_summary = self.animation_runtime_summary();
                if native_fan_script {
                    ui.horizontal(|ui| {
                        ui.strong("Fan XItem runtime +0x6C:");
                        ui.add(
                            egui::DragValue::new(&mut self.fan_runtime_value)
                                .range(-10_000..=10_000)
                                .speed(1),
                        );
                        if ui.button("Reset angle").clicked() {
                            self.fan_runtime_angle = 0.0;
                        }
                        ui.label(format!(
                            "angle {:.3} rad / {:.1}°",
                            self.fan_runtime_angle,
                            self.fan_runtime_angle.to_degrees()
                        ));
                    });
                }

                ui.horizontal_wrapped(|ui| {
                    if let Some(script) = self.current_script() {
                        ui.strong("Frame:");
                        ui.label(format!("{}", script.frame_at_time(self.current_time) as isize));

                        let counts = script.command_type_counts();
                        ui.separator();
                        ui.label(format!(
                            "Entity {}  Anim {}  SubScript {}  Particle {}  Sound {}  Event {}  Unknown {}",
                            counts.entities,
                            counts.animations,
                            counts.subscripts,
                            counts.particles,
                            counts.sounds,
                            counts.events,
                            counts.unknown
                        ));

                        if counts.animations > 0 {
                            ui.separator();
                            let summary = animation_runtime_summary;
                            if summary.active == 0 {
                                ui.label(
                                    "No Animation command is active at the current Script frame.",
                                );
                            } else if summary.rendered == summary.active {
                                ui.colored_label(
                                    egui::Color32::LIGHT_GREEN,
                                    format!(
                                        "Native skeletal sampling active for {}/{} Animation command(s). Serialized Script FPS defines command duration; normalized command phase samples the complete motion asset.",
                                        summary.rendered, summary.active
                                    ),
                                );
                            } else {
                                ui.colored_label(
                                    egui::Color32::YELLOW,
                                    format!(
                                        "Native skeletal sampling active for {}/{} Animation command(s). {}.",
                                        summary.rendered,
                                        summary.active,
                                        summary.failure_description()
                                    ),
                                );
                            }
                        }

                        if native_fan_script {
                            ui.separator();
                            ui.colored_label(
                                egui::Color32::LIGHT_GREEN,
                                "Native FanHorizontal rotation is active: HT_Entity_FanRotatingEntity receives a local-Z delta at fixed 60 Hz using signed XItem runtime [+0x6C] × 0.002 rad. The live-runtime value above supplies the gameplay state that is not serialized in the AnimScript; opcode 0x10 remains only the frame-1 loop command.",
                            );
                        }

                        if counts.entities + counts.animations + counts.subscripts == 0 {
                            ui.separator();
                            if counts.particles > 0 {
                                ui.colored_label(
                                    egui::Color32::LIGHT_GREEN,
                                    "Native particle playback is active: EXParticleSys timing, pool limits, resource selection, texture/material state, emission, lifetime, velocity, acceleration, damping and colour/scale/rotation curves are evaluated. The preview keeps a deterministic emitter stream so seeking and regression tests remain reproducible; only the game's process-global RNG call order remains outside standalone Script context.",
                                );
                            } else {
                                ui.colored_label(
                                    egui::Color32::GRAY,
                                    "No geometry commands in this script",
                                );
                            }
                        }
                    }
                });

                self.show_controls(ui);
                ui.add_space(4.0);

                if let Some(script) = self.current_script() {
                    egui::ScrollArea::vertical()
                        .id_salt("script_graph_scroll_area")
                        .show(ui, |ui| self.draw_script_graph(script, ui));
                }
            });
        });

        if self.is_playing {
            self.current_time += delta_time * self.playback_speed;
            ui.ctx().request_repaint();
        }
        if let Some(script) = self.current_script() {
            if self.current_time > script.duration_seconds() {
                if self.loop_script {
                    self.current_time = 0.0;
                } else {
                    self.current_time = script.duration_seconds();
                    self.is_playing = false;
                }
            }
        }
        self.sync_script_timeline_audio(ui.ctx());
    }

    fn show_canvas(&mut self, ui: &mut egui::Ui) {
        let (rect, response) = ui.allocate_exact_size(
            (ui.available_size()
                - egui::vec2(0., 96.)
                - egui::vec2(0., self.thread_count() as f32 * 17.0))
            .clamp(
                egui::vec2(f32::MIN, ui.available_height() / 2.0),
                egui::vec2(f32::MAX, f32::MAX),
            ),
            egui::Sense::click_and_drag(),
        );

        let time: f64 = ui.input(|t| t.time);
        let render_store = self.render_store.clone();

        let current_file = self.file;
        let current_script = self.selected_script;
        let current_time = self.current_time;
        let show_full_assembly = self.show_full_assembly;
        let particle_renderer = self.particle_renderer.clone();
        let particle_settings = self.particle_settings;
        let fan_runtime_angle = self.fan_runtime_angle;
        self.viewer.lock().update(ui, &response);
        let viewer = self.viewer.clone();
        let cb = egui_glow::CallbackFn::new(move |info, painter| unsafe {
            let mut v = viewer.lock();
            v.start_render(painter.gl(), info.viewport.aspect_ratio(), time as f32);
            let render_context = v.render_context();

            let mut render_queue: Vec<QueuedEntityRender> = vec![];
            let mut animation_queue = vec![];
            let mut particle_queue = vec![];

            {
                let store = render_store.read();
                if show_full_assembly {
                    render_static_script_without_animations(
                        Vec3::ZERO,
                        Quat::IDENTITY,
                        Vec3::ONE,
                        current_file,
                        current_script,
                        current_time,
                        &store,
                        &mut |q| render_queue.push(q),
                        vec![],
                    );
                } else {
                    render_script_without_static_animations(
                        Vec3::ZERO,
                        Quat::IDENTITY,
                        Vec3::ONE,
                        current_file,
                        current_script,
                        current_time,
                        &store,
                        &mut |q| render_queue.push(q),
                        vec![],
                    );
                }

                collect_script_animations(
                    Vec3::ZERO,
                    Quat::IDENTITY,
                    Vec3::ONE,
                    current_file,
                    current_script,
                    current_time,
                    &store,
                    &mut animation_queue,
                    vec![],
                    show_full_assembly,
                );
                collect_script_particles(
                    Vec3::ZERO,
                    Quat::IDENTITY,
                    Vec3::ONE,
                    current_file,
                    current_script,
                    current_time,
                    &store,
                    &mut particle_queue,
                    vec![],
                );
                apply_native_fan_rotation(&mut render_queue, &store, fan_runtime_angle);
            }

            for r in render_queue.iter() {
                if let Some(e) = render_store.read().get_entity(r.entity.0, r.entity.1) {
                    e.draw_opaque(
                        painter.gl(),
                        &render_context,
                        r.position,
                        r.rotation,
                        r.scale,
                        time,
                        &render_store.read(),
                    )
                }
            }

            {
                let store = render_store.read();
                for animation in &animation_queue {
                    if let Some(runtime) = store.get_animation_runtime(animation.animation.0) {
                        runtime.draw(
                            painter.gl(),
                            &render_context,
                            &store,
                            animation.animation.1,
                            animation.skin.1,
                            animation.phase,
                            animation.position,
                            animation.rotation,
                            animation.scale,
                            time,
                        );
                    }
                }
            }

            painter.gl().depth_mask(false);

            for r in render_queue.iter() {
                if let Some(e) = render_store.read().get_entity(r.entity.0, r.entity.1) {
                    e.draw_transparent(
                        painter.gl(),
                        &render_context,
                        r.position,
                        r.rotation,
                        r.scale,
                        time,
                        &render_store.read(),
                    )
                }
            }

            for emitter in &particle_queue {
                let store = render_store.read();
                if let Some(particle) = store.get_particle(emitter.particle.0, emitter.particle.1) {
                    particle_renderer.render(
                        painter.gl(),
                        &render_context,
                        emitter,
                        particle,
                        particle_settings,
                        &store,
                    );
                }
            }
        });

        let callback = egui::PaintCallback {
            rect,
            callback: Arc::new(cb),
        };
        ui.painter().add(callback);
    }

    fn show_controls(&mut self, ui: &mut egui::Ui) {
        centerer(ui, |ui| {
            ui.style_mut().spacing.button_padding = egui::vec2(6., 4.);

            if ui
                .button(RichText::new(font_awesome::STEP_BACKWARD).size(16.))
                .clicked()
                || ui.input(|i| i.key_pressed(egui::Key::ArrowLeft))
            {
                if let Some(s) = self.current_script() {
                    let current_frame = s.frame_at_time(self.current_time) as i32;
                    self.current_time = s.time_at_frame((current_frame - 1).max(0) as f32);
                }
            }

            if ui
                .button(
                    RichText::new(if self.is_playing {
                        font_awesome::PAUSE
                    } else {
                        font_awesome::PLAY
                    })
                    .size(16.),
                )
                .clicked()
                || ui.input(|i| i.key_pressed(egui::Key::Space))
            {
                self.is_playing = !self.is_playing;

                if let Some(script) = self.current_script() {
                    if self.current_time >= script.duration_seconds() {
                        self.current_time = 0.0;
                    }
                }
            }

            if ui
                .button(RichText::new(font_awesome::STEP_FORWARD).size(16.))
                .clicked()
                || ui.input(|i| i.key_pressed(egui::Key::ArrowRight))
            {
                if let Some(s) = self.current_script() {
                    let current_frame = s.frame_at_time(self.current_time) as i32;
                    self.current_time = s.time_at_frame((current_frame + 1) as f32);
                }
            }

            let loop_button = ui
                .button(
                    RichText::new(if self.loop_script {
                        '\u{f363}'
                    } else {
                        '\u{f178}'
                    })
                    .size(16.),
                )
                .on_hover_text("Loop playback");
            if loop_button.clicked() {
                self.loop_script = !self.loop_script;
            }
        });
    }

    const COMMAND_COLOR_ENTITY: egui::Color32 = egui::Color32::from_rgb(98, 176, 255);
    const COMMAND_COLOR_PARTICLE: egui::Color32 = egui::Color32::from_rgb(168, 235, 247);
    const COMMAND_COLOR_ANIMATION: egui::Color32 = egui::Color32::from_rgb(255, 173, 134);
    const COMMAND_COLOR_SUBSCRIPT: egui::Color32 = egui::Color32::from_rgb(238, 145, 234);
    const COMMAND_COLOR_SOUND: egui::Color32 = egui::Color32::from_rgb(255, 188, 255);
    const COMMAND_COLOR_EVENT: egui::Color32 = egui::Color32::WHITE;
    const COMMAND_COLOR_UNKNOWN: egui::Color32 = egui::Color32::WHITE;

    fn draw_script_graph(&self, script: &UXGeoScript, ui: &mut egui::Ui) {
        let num_threads = script
            .commands
            .iter()
            .filter_map(|command| {
                (command.controller_index != u8::MAX)
                    .then_some(command.controller_index as usize + 1)
            })
            .max()
            .unwrap_or(1);

        let current_frame = script.frame_at_time(self.current_time);
        let width = ui.available_width();
        let single_frame_width = width / script.length as f32;

        let (rect, _response) = ui.allocate_exact_size(
            egui::vec2(width, num_threads as f32 * 17.0),
            egui::Sense::click(),
        );

        let render_store = self.render_store.read();
        for c in &script.commands {
            let sound_hashcode = match &c.data {
                UXGeoScriptCommandData::Sound { hashcode } => Some(*hashcode),
                _ => None,
            };
            let mut extra_info = String::new();
            let (color, label, file_hash) = match &c.data {
                UXGeoScriptCommandData::Entity { hashcode, file } => (
                    Self::COMMAND_COLOR_ENTITY,
                    format!(
                        "Entity {}",
                        format_script_object_reference(
                            &self.hashcodes,
                            *hashcode,
                            render_store.resolve_entity_hashcode(self.file, *hashcode),
                        )
                    ),
                    script_object_file_for_display(*hashcode, *file),
                ),
                UXGeoScriptCommandData::Animation {
                    skin_file,
                    skin_hashcode,
                    anim_file,
                    anim_hashcode,
                } => (
                    Self::COMMAND_COLOR_ANIMATION,
                    format!(
                        "{} (skin {}{})",
                        semantic_object_reference(&self.hashcodes, "Animation", *anim_hashcode),
                        if *skin_hashcode == u32::MAX {
                            "implicit Animation binding".to_string()
                        } else {
                            format_script_object_reference(
                                &self.hashcodes,
                                *skin_hashcode,
                                render_store.resolve_animskin_hashcode(self.file, *skin_hashcode),
                            )
                        },
                        if *skin_hashcode == u32::MAX
                            || skin_hashcode.is_local()
                            || *skin_file == u32::MAX
                        {
                            String::new()
                        } else {
                            format!(" {}", format_hashcode(&self.hashcodes, *skin_file))
                        }
                    ),
                    script_object_file_for_display(*anim_hashcode, *anim_file),
                ),
                UXGeoScriptCommandData::SubScript { hashcode, file } => (
                    Self::COMMAND_COLOR_SUBSCRIPT,
                    format!(
                        "Sub-Script {}",
                        format_script_object_reference(
                            &self.hashcodes,
                            *hashcode,
                            render_store.resolve_script_hashcode(self.file, *hashcode),
                        )
                    ),
                    script_object_file_for_display(*hashcode, *file),
                ),
                UXGeoScriptCommandData::Sound { hashcode } => (
                    Self::COMMAND_COLOR_SOUND,
                    format!(
                        "Sound {}",
                        format_script_object_reference(&self.hashcodes, *hashcode, None)
                    ),
                    u32::MAX,
                ),
                UXGeoScriptCommandData::Particle { hashcode, file } => (
                    Self::COMMAND_COLOR_PARTICLE,
                    format!(
                        "Particle {}",
                        format_script_object_reference(
                            &self.hashcodes,
                            *hashcode,
                            render_store.resolve_particle_hashcode(self.file, *hashcode),
                        )
                    ),
                    script_object_file_for_display(*hashcode, *file),
                ),
                UXGeoScriptCommandData::Event { event_type, data } => {
                    extra_info = hex::encode(data);
                    (
                        Self::COMMAND_COLOR_EVENT,
                        format!("Event {}", format_hashcode(&self.hashcodes, *event_type)),
                        u32::MAX,
                    )
                }
                UXGeoScriptCommandData::Unknown { cmd, data } => {
                    let role = robots_script_command_role(*cmd, data.len());
                    if role.family == "terminator" {
                        continue;
                    }

                    extra_info = if let Some(payload) = robots_script_payload_diagnostic(*cmd, data)
                    {
                        format!(
                            "{}\nraw payload={}",
                            payload.native_summary(),
                            hex::encode(data)
                        )
                    } else {
                        format!(
                            "native family={} runtime_subtype={} payload={}",
                            role.family,
                            role.runtime_subtype
                                .map(|value| value.to_string())
                                .unwrap_or_else(|| "none".to_string()),
                            hex::encode(data)
                        )
                    };
                    (
                        Self::COMMAND_COLOR_UNKNOWN,
                        if role.classified {
                            format!("{} [0x{cmd:x}]", role.name)
                        } else {
                            format!("Unknown 0x{cmd:x}")
                        },
                        u32::MAX,
                    )
                }
            };

            let start = c.start.clamp(0, i16::MAX);
            let record_row = if c.controller_index == u8::MAX {
                0.0
            } else {
                c.controller_index as f32
            };
            let cmd_response = ui.allocate_rect(
                egui::Rect::from_min_size(
                    rect.min + egui::vec2(start as f32 * single_frame_width, record_row * 19.0),
                    egui::vec2(c.length as f32 * single_frame_width, 18.0),
                ),
                if sound_hashcode.is_some() {
                    egui::Sense::click()
                } else {
                    egui::Sense::hover()
                },
            );
            let clicked_sound = sound_hashcode.filter(|_| cmd_response.clicked());

            let mut extra_info_split = String::new();
            writeln!(extra_info_split).ok();
            for (i, v) in extra_info
                .chars()
                .collect::<Vec<char>>()
                .chunks(8)
                .enumerate()
            {
                write!(extra_info_split, "{} ", v.iter().collect::<String>()).ok();

                if (i % 4) == 3 {
                    writeln!(extra_info_split).ok();
                }
            }
            cmd_response.on_hover_ui_at_pointer(|ui| {
                ui.label(format!(
                    "{}{}\nStart: {}\nLength: {}\nRuntime record: {}\nController header: {}\nParent record: {}\n",
                    label,
                    if file_hash != u32::MAX {
                        format!(" ({})", format_hashcode(&self.hashcodes, file_hash))
                    } else {
                        String::new()
                    },
                    c.start,
                    c.length,
                    c.controller_index,
                    c.controller_header_index,
                    c.parent_controller_index,
                ));
                ui.monospace(extra_info_split);
                if sound_hashcode.is_some() {
                    ui.separator();
                    ui.strong("Click to decode and play this EuroSound sample");
                }
            });
            if let Some(hashcode) = clicked_sound {
                self.sound_preview.lock().play_manual(hashcode);
            }

            let cmd_rect = egui::Rect::from_min_size(
                rect.min + egui::vec2(start as f32 * single_frame_width, record_row * 19.0),
                egui::vec2(c.length as f32 * single_frame_width, 18.0),
            );
            let graph_paint_clipped = ui.painter_at(cmd_rect);

            graph_paint_clipped.rect_filled(cmd_rect, egui::CornerRadius::same(4), color);

            if let Some(controller) = script.controllers.get(c.controller_header_index as usize) {
                let mut keyframes: Vec<f32> = controller
                    .channels
                    .vector_0
                    .iter()
                    .map(|(frame, _)| *frame)
                    .chain(controller.channels.quat_0.iter().map(|(frame, _)| *frame))
                    .chain(controller.channels.vector_1.iter().map(|(frame, _)| *frame))
                    .collect();

                keyframes.sort_by(|a, b| a.total_cmp(b));
                keyframes.dedup();

                for keyframe in keyframes {
                    graph_paint_clipped.text(
                        rect.min
                            + egui::vec2(keyframe * single_frame_width, record_row * 19.0 + 18.5),
                        egui::Align2::CENTER_BOTTOM,
                        "🔺",
                        egui::FontId::proportional(6.0),
                        egui::Color32::BLACK,
                    );
                }
            }

            graph_paint_clipped.text(
                rect.min
                    + egui::vec2(
                        4.0 + start as f32 * single_frame_width,
                        record_row * 19.0 + 9.0,
                    ),
                egui::Align2::LEFT_CENTER,
                format!("{} - {}", c.start, label),
                egui::FontId::proportional(12.0),
                egui::Color32::BLACK,
            );
        }

        // Render playhead
        ui.painter_at(rect).vline(
            rect.min.x + current_frame * single_frame_width,
            rect.min.y..=(rect.min.y + num_threads as f32 * 19.0),
            egui::Stroke::new(1.0_f32, egui::Color32::RED),
        );
    }
}

// Helper function to center arbitrary widgets. It works by measuring the width of the widgets after rendering, and
// then using that offset on the next frame.
fn centerer(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        let id = ui.id().with("_centerer");
        let last_width: Option<f32> = ui.memory_mut(|mem| mem.data.get_temp(id));
        if let Some(last_width) = last_width {
            ui.add_space((ui.available_width() - last_width) / 2.0);
        }
        let res = ui
            .scope(|ui| {
                add_contents(ui);
            })
            .response;
        let width = res.rect.width();
        ui.memory_mut(|mem| mem.data.insert_temp(id, width));

        // Repaint if width changed
        match last_width {
            None => ui.ctx().request_repaint(),
            Some(last_width) if last_width != width => ui.ctx().request_repaint(),
            Some(_) => {}
        }
    });
}
