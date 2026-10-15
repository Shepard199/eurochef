use super::*;

fn control_card<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    egui::Frame::new()
        .fill(egui::Color32::from_rgb(20, 23, 40))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(67, 71, 104)))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(8))
        .show(ui, add_contents)
}

fn control_tab(ui: &mut egui::Ui, active_tab: &mut usize, tab: usize, icon: char, label: &str) {
    let selected = *active_tab == tab;
    if ui
        .selectable_label(selected, format!("{icon} {label}"))
        .on_hover_text(label)
        .clicked()
    {
        *active_tab = tab;
    }
}

fn control_badge(ui: &mut egui::Ui, label: impl std::fmt::Display, colour: egui::Color32) {
    egui::Frame::new()
        .fill(colour.gamma_multiply(0.18))
        .corner_radius(egui::CornerRadius::same(5))
        .inner_margin(egui::Margin::symmetric(6, 2))
        .show(ui, |ui| ui.colored_label(colour, label.to_string()));
}

impl MapFrame {
    pub(super) fn draw_map_controls(
        &mut self,
        ctx: &egui::Context,
        maps: &[ProcessedMap],
    ) -> anyhow::Result<()> {
        let Some(current_map) = maps.get(self.selected_map) else {
            return Ok(());
        };
        let camera_position = {
            let mut viewer = self.viewer.lock();
            viewer.camera_mut().position()
        };
        let active_zone_index = current_map.native_zone_index(camera_position);

        self.textfield_focused = false;
        let mut render_options_changed = false;
        // Persist the compact segmented navigation in egui memory rather than
        // adding UI-only state to MapFrame or changing map/runtime semantics.
        let tab_id = egui::Id::new("map_controls_active_tab");
        let mut active_tab = ctx.data(|data| data.get_temp::<usize>(tab_id).unwrap_or(0));
        let response = egui::Window::new("Map Controls")
            .default_pos(egui::pos2(12.0, 72.0))
            .default_width(340.0)
            .min_width(290.0)
            .scroll([false, true])
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(13, 16, 29))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(92, 82, 142)))
                    .corner_radius(egui::CornerRadius::same(10)),
            )
            .show(ctx, |ui| -> anyhow::Result<()> {
                if let Some(dev_map) = robots_dev_map_info(self.file) {
                    control_card(ui, |ui| {
                        ui.strong(format!("{} DEV MAP: {}", font_awesome::FLASK, dev_map.label));
                        ui.monospace(format!(
                            "Level {}  {}  EDB 0x{:08X}",
                            dev_map.level_id, dev_map.source_edb, dev_map.file
                        ));
                        ui.label(dev_map.evidence_role);
                        ui.small(
                            "High-signal serialized regression source. Native semantics still require an instruction-proven consumer.",
                        );
                    });
                    ui.add_space(4.0);
                }

                // Four tabs replace the previous always-open diagnostic wall.
                // The active tab is persistent for this egui session and does
                // not change map loading, playback, or render state.
                control_card(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        control_tab(ui, &mut active_tab, 0, font_awesome::LIGHTBULB, "Lighting & Scene");
                        control_tab(ui, &mut active_tab, 1, font_awesome::CAMERA, "Camera & Runtime");
                        control_tab(ui, &mut active_tab, 2, font_awesome::CUBE, "Geometry & Triggers");
                        control_tab(ui, &mut active_tab, 3, font_awesome::VOLUME_UP, "Audio & Particles");
                    });
                });
                ui.add_space(6.0);

                if active_tab == 0 {
                    control_card(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Sky override");
                            let response = egui::TextEdit::singleline(&mut self.sky_ent)
                                .desired_width(92.0)
                                .hint_text("hashcode")
                                .show(ui)
                                .response;
                            self.textfield_focused = response.has_focus();

                            if !self.sky_ent.trim().is_empty() {
                                let parsed_sky = u32::from_str_radix(self.sky_ent.trim(), 16);
                                if let Ok(hashcode) = parsed_sky {
                                    if self
                                        .render_store
                                        .read()
                                        .get_entity(self.file, hashcode)
                                        .is_none()
                                        && self
                                            .render_store
                                            .read()
                                            .get_script(self.file, hashcode)
                                            .is_none()
                                    {
                                        ui.strong(font_awesome::EXCLAMATION_TRIANGLE.to_string())
                                            .on_hover_text("Entity or script was not found");
                                    }
                                } else {
                                    ui.strong(font_awesome::EXCLAMATION_TRIANGLE.to_string())
                                        .on_hover_text(
                                            "String is not formatted as a valid hashcode",
                                        );
                                }
                            }
                        });
                        ui.monospace(&self.sky_diagnostic);

                        render_options_changed |= ui
                            .checkbox(&mut self.vertex_lighting, "Vertex Lighting")
                            .changed();
                        ui.checkbox(&mut self.global_lighting, "Global Lighting")
                            .on_hover_text("Applies the recovered spatial vertex-colour world-light sample, three live smoothed directional slots, level coefficients and fallback sun. Unknown EDB UIDs remain unlit rather than using guessed defaults.");
                        ui.checkbox(&mut self.native_lights, "Native Lights")
                            .on_hover_text("Applies the serialized MapZone light set and the exact EXGeoLight type-bit influence functions recovered from Robots.exe.");
                        ui.add_enabled(
                            self.native_lights,
                            egui::DragValue::new(&mut self.native_light_strength)
                                .range(0.0..=4.0)
                                .max_decimals(2)
                                .speed(0.02)
                                .prefix("Light strength "),
                        );

                        egui::CollapsingHeader::new("Lighting diagnostics")
                            .default_open(false)
                            .show(ui, |ui| {
                                if let Some(global) = robots_global_lighting(self.file) {
                                    ui.label(format!(
                                        "Sun ({:.3}, {:.3}, {:.3}) RGB {:.2} Ambient {:.2}",
                                        global.direction.x,
                                        global.direction.y,
                                        global.direction.z,
                                        global.colour.x,
                                        global.ambient.x,
                                    ))
                                    .on_hover_text(format!(
                                        "Robots.exe level coefficients: {:?}",
                                        global.level_coefficients
                                    ));
                                } else {
                                    ui.label("Sun unavailable").on_hover_text(
                                        "This EDB UID has no proven record in the 15-entry Robots.exe level-lighting table.",
                                    );
                                }

                                ui.label(format!(
                                    "World samples {}  Lights {}  Sounds {}",
                                    current_map.lighting_triangles.len(),
                                    current_map.lights.len(),
                                    current_map.sounds.len(),
                                ))
                                .on_hover_ui(|ui| {
                                    ui.label("Spatial world-light triangles are reconstructed from MapZone geometry and vertex colours.");
                                    ui.label("EXGeoIdentifier.ambience is retained as serialized provenance. All 351 shipped Robots MapZones store the exact identity value 1.0; no native rendering consumer of identifier +0x14 was recovered, so it is intentionally not part of lighting uniforms.");
                                    for (index, zone) in current_map.zones.iter().enumerate().take(16) {
                                        ui.monospace(format!(
                                            "zone {} ambience={:.6} lights={} sounds={}",
                                            index,
                                            zone.identifier.ambience,
                                            zone.light_array.len(),
                                            zone.sound_array.len(),
                                        ));
                                    }
                                });

                                ui.label(format!("Serialized lights {}", current_map.lights.len()))
                                    .on_hover_ui(|ui| {
                                        for light in current_map.lights.iter().take(16) {
                                            ui.monospace(format!(
                                                "0x{:08x} type={} [{}] flags=0x{:08x} rgba={:02X}{:02X}{:02X}{:02X} radius={:.3}",
                                                light.hashcode,
                                                light.light_type,
                                                robots_native_light_type_description(light.light_type),
                                                light.flags,
                                                light.colour[0],
                                                light.colour[1],
                                                light.colour[2],
                                                light.colour[3],
                                                light.radius
                                            ));
                                        }
                                        if current_map.lights.len() > 16 {
                                            ui.label(format!(
                                                "… and {} more",
                                                current_map.lights.len() - 16
                                            ));
                                        }
                                    });
                            });
                    });
                }

                if active_tab == 1 {
                    control_card(ui, |ui| {
                        ui.horizontal(|ui| {
                            match active_zone_index {
                                Some(zone) => control_badge(
                                    ui,
                                    format!("{} Zone #{zone}", font_awesome::CUBE),
                                    egui::Color32::from_rgb(77, 196, 133),
                                ),
                                None => control_badge(
                                    ui,
                                    format!("{} Outside MapZone", font_awesome::EXCLAMATION_TRIANGLE),
                                    egui::Color32::from_rgb(232, 166, 63),
                                ),
                            }
                            let camera_status = self
                                .active_camera_trigger
                                .map(|index| format!("{} Camera #{index}", font_awesome::CAMERA))
                                .unwrap_or_else(|| format!("{} Free camera", font_awesome::EYE));
                            control_badge(ui, camera_status, egui::Color32::from_rgb(112, 158, 255));
                        });
                        ui.add_space(6.0);
                        egui::CollapsingHeader::new("Native surface diagnostics")
                            .default_open(false)
                            .show(ui, |ui| {
                                if let Some(zone_index) = active_zone_index {
                                    ui.monospace(format!("Active MapZone {zone_index}"));
                                    if let Some(counts) =
                                        current_map.zone_surface_mask_counts.get(zone_index)
                                    {
                                        if counts.is_empty() {
                                            ui.label("No nonzero Robots face-info surface metadata in this zone.");
                                        } else {
                                            for (mask, count) in counts {
                                                ui.label(format!(
                                                    "{}: {} face(s)",
                                                    crate::entities::robots_surface_mask_label(*mask),
                                                    count
                                                ));
                                            }
                                        }
                                    }
                                } else {
                                    ui.label("Camera is outside a resolved MapZone.");
                                }
                                ui.small(
                                    "Diagnostic only. Native 0x0041C2C0 ORs all active contact-face metadata before choosing the runtime surface category; isolated face masks are not a fabricated editor collision result.",
                                );
                            });
                        ui.checkbox(&mut self.show_native_surfaces, "Native Surface Faces")
                            .on_hover_text(
                                "Draws diagnostic outlines for nonzero Robots v248 face_info surface metadata. These are exact source collision/material faces; colours are editor-only diagnostics, not native material colours.",
                            );

                        ui.checkbox(
                            &mut self.preview_zone_background,
                            "Serialized zone background preview",
                        )
                        .on_hover_text(
                            "Clears the map viewport with EXGeoIdentifier.rgba_back_ground from the exact active MapZone. Native fog is composed independently from that zone's fog_near/far/min/max/method/rgba_fog fields and per-Entity NoFog flag.",
                        );
                        ui.checkbox(&mut self.show_portals, "Portals")
                            .on_hover_text("Draws the four serialized EXGeoPortal boundary edges. Portal traversal and level-state transfer are not fabricated.");
                        ui.checkbox(
                            &mut self.apply_native_camera_viewport,
                            "Apply native Camera to viewport",
                        )
                        .on_hover_text(
                            "Applies the instruction-proven ordinary Player mode 1 plus trigger-owned Camera modes 0/3/4, native SetVFOV and fixed-60-Hz interpolation. Default mode 1 is accepted for gameplay MapZone state only while the static target-to-eye path is unobstructed; native Camera contact correction near walls remains a Physics boundary.",
                        );
                        let live_player_changed = ui
                            .checkbox(
                                &mut self.native_camera_live_player_preview,
                                "Live player pose for Camera",
                            )
                            .on_hover_text(
                                "Editor-only Camera proxy before Player creation. Once Simulate AI has a real Player XItem, Camera tracks that owner instead. This preview never drives TriggerManager/NPC targeting and does not replace Player Physics.",
                            )
                            .changed();
                        if live_player_changed && self.native_camera_live_player_preview {
                            self.reset_native_camera_player_preview(current_map);
                        }
                        ui.horizontal(|ui| {
                            if ui.button("Reset player to XTrigger_Player").clicked() {
                                self.reset_native_camera_player_preview(current_map);
                            }
                            ui.add_enabled(
                                self.native_camera_live_player_preview,
                                egui::DragValue::new(&mut self.native_camera_player_preview.speed_mul)
                                    .range(0.0..=5.0)
                                    .speed(0.05)
                                    .prefix("move speed "),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.label("Player XYZ");
                            ui.add_enabled(
                                self.native_camera_live_player_preview,
                                egui::DragValue::new(&mut self.native_camera_player_preview.position.x)
                                    .speed(0.05),
                            );
                            ui.add_enabled(
                                self.native_camera_live_player_preview,
                                egui::DragValue::new(&mut self.native_camera_player_preview.position.y)
                                    .speed(0.05),
                            );
                            ui.add_enabled(
                                self.native_camera_live_player_preview,
                                egui::DragValue::new(&mut self.native_camera_player_preview.position.z)
                                    .speed(0.05),
                            );
                        });
                        if self.simulate_ai {
                            if let Some(player) = self.native_runtime_player_state {
                                ui.monospace(format!(
                                    "Gameplay Player XItem: trigger #{}, XYZ [{:.2}, {:.2}, {:.2}]",
                                    player.trigger_index,
                                    player.position.x,
                                    player.position.y,
                                    player.position.z,
                                ));
                            } else {
                                ui.small(
                                    "Gameplay Player XItem not created. Camera proxy does not trigger AI proximity, attacks or pickups. Native Player creation/input/physics is still incomplete.",
                                );
                            }
                        }
                        ui.separator();
                        ui.strong("Sweeper synchronized inputs");
                        ui.horizontal(|ui| {
                            ui.checkbox(
                                &mut self.native_sweeper_player_health_known,
                                "Known Player health",
                            );
                            ui.add_enabled(
                                self.native_sweeper_player_health_known,
                                egui::DragValue::new(
                                    &mut self.native_sweeper_player_current_health,
                                )
                                .speed(0.25)
                                .prefix("current "),
                            );
                            ui.add_enabled(
                                self.native_sweeper_player_health_known,
                                egui::DragValue::new(&mut self.native_sweeper_player_max_health)
                                    .speed(0.25)
                                    .prefix("max "),
                            );
                        });
                        let health_input_valid = self.native_sweeper_player_current_health.is_finite()
                            && self.native_sweeper_player_max_health.is_finite()
                            && self.native_sweeper_player_current_health >= 0.0
                            && self.native_sweeper_player_max_health > 0.0
                            && self.native_sweeper_player_current_health
                                <= self.native_sweeper_player_max_health;
                        if self.native_sweeper_player_health_known && !health_input_valid {
                            ui.colored_label(
                                egui::Color32::from_rgb(232, 166, 63),
                                "Health input is invalid; Sweeper keeps it unknown.",
                            );
                        }

                        let seed_text = self.native_global_gameplay_rng_seed_input.trim();
                        let seed_hex = seed_text
                            .strip_prefix("0x")
                            .or_else(|| seed_text.strip_prefix("0X"))
                            .unwrap_or(seed_text);
                        let parsed_seed = (!seed_hex.is_empty())
                            .then(|| u32::from_str_radix(seed_hex, 16).ok())
                            .flatten();
                        ui.horizontal(|ui| {
                            ui.label("Observed global RNG");
                            let response = egui::TextEdit::singleline(
                                &mut self.native_global_gameplay_rng_seed_input,
                            )
                            .desired_width(92.0)
                            .hint_text("hex seed")
                            .show(ui)
                            .response;
                            self.textfield_focused |= response.has_focus();
                            if ui
                                .add_enabled(parsed_seed.is_some(), egui::Button::new("Anchor"))
                                .clicked()
                            {
                                self.native_global_gameplay_rng =
                                    RuntimeRobotsGlobalRngState::from_observed_seed(
                                        parsed_seed.expect("enabled only for a valid seed"),
                                    );
                            }
                            if ui.button("Clear").clicked() {
                                self.native_global_gameplay_rng.invalidate();
                            }
                        });
                        ui.monospace(match self.native_global_gameplay_rng.seed() {
                            Some(seed) => format!(
                                "RNG {:?}: 0x{seed:08X}, draws {}",
                                self.native_global_gameplay_rng.provenance,
                                self.native_global_gameplay_rng.draws_from_anchor,
                            ),
                            None => "RNG UnknownSession".to_owned(),
                        });
                        ui.small(
                            "Anchor only with a seed observed at the exact currently represented native runtime boundary. A sampled seed from an unrelated live moment is not synchronization. Player health is likewise used only when explicitly marked known.",
                        );
                        ui.small(
                            "While native Camera viewport is enabled: WASD/A-D move the live Player pose, Q/E move vertically, drag changes Player heading. Ordinary mode 1 and recovered trigger Camera modes consume this pose each frame; Player physics and mode-1 Camera contact correction remain separate runtime boundaries.",
                        );
                        ui.monospace(format!(
                            "Camera [{:.3}, {:.3}, {:.3}]  active zone {}",
                            camera_position.x,
                            camera_position.y,
                            camera_position.z,
                            active_zone_index
                                .map(|index| index.to_string())
                                .unwrap_or_else(|| "none".to_string())
                        ));

                        if let Some(trigger_index) = self.active_camera_trigger {
                            if let Some(plan) =
                                robots_camera_controller_plan(current_map, trigger_index)
                            {
                                ui.monospace(format!(
                                    "Active XTrigger_Camera #{}  {}",
                                    trigger_index,
                                    plan.setup_kind.description(),
                                ));
                                ui.monospace(format!(
                                    "flags=0x{:08X} tested=0x{:03X} data3(raw)=0x{:08X} data4/5(scaled)={:?}/{:?}",
                                    plan.flags,
                                    plan.native_tested_flags,
                                    plan.controller_data3_raw,
                                    plan.scaled_data4,
                                    plan.scaled_data5,
                                ));
                                if let Some(marker_index) = plan.linked_marker_index {
                                    ui.monospace(format!(
                                        "first linked Camera Marker #{} position={:?}",
                                        marker_index, plan.linked_marker_position,
                                    ));
                                }
                                if let Some(yaw) = plan.mode1_yaw_radians {
                                    ui.monospace(format!(
                                        "mode1 native yaw atan2+2pi={:.6} rad",
                                        yaw,
                                    ));
                                }
                                if plan.mode == 3 {
                                    ui.monospace(format!(
                                        "mode3 preserve current Camera axes: Y={} XZ={}",
                                        plan.mode3_preserve_current_camera_y,
                                        plan.mode3_preserve_current_camera_xz,
                                    ));
                                }
                                if plan.mode == 4 {
                                    ui.monospace(format!(
                                        "mode4 path={:?} data6/7(raw float)={:?}/{:?} option flags=0x{:02X}",
                                        plan.path_hashcode,
                                        plan.mode4_data6,
                                        plan.mode4_data7,
                                        plan.mode4_option_flags,
                                    ));
                                }
                                if let Some(runtime) = self.native_camera_runtime.as_ref() {
                                    ui.monospace(format!(
                                        "viewport pose: pos={:?} target={:?} VFOV={:.3} roll={:.3} rate={:.3}",
                                        runtime.current.position,
                                        runtime.current.target,
                                        runtime.current.vertical_fov_degrees,
                                        runtime.current.roll_degrees,
                                        runtime.interpolation_rate,
                                    ));
                                    ui.monospace(format!(
                                        "player anchor={:?} interpolation={} mode4 parameter={:?} F0={:?}",
                                        runtime.player_anchor,
                                        runtime.interpolating,
                                        runtime.mode4_parameter(),
                                        runtime.mode4_direction_scalar(),
                                    ));
                                    if let Some(boundary) = runtime.boundary {
                                        ui.colored_label(
                                            egui::Color32::YELLOW,
                                            format!("Viewport boundary: {}", boundary.description()),
                                        );
                                    }
                                }
                                ui.small(
                                    "Mode 0/3 and shipped mode-4 XPath_Spline viewport pose, SetVFOV, dynamic path follow and interpolation are native. Mode 1/2 gameplay controller state remains unresolved. Mode 3/4 now consume the live player-pose preview above; only full Player gameplay physics/collision remains outside the Camera runtime.",
                                );
                            } else {
                                ui.monospace("Active XTrigger_Camera: stale or invalid controller plan");
                            }
                        } else {
                            ui.monospace("Active XTrigger_Camera: none");
                        }

                        if let Some(zone_index) = active_zone_index {
                            let identifier = &current_map.zones[zone_index].identifier;
                            ui.separator();
                            ui.monospace(format!(
                                "zone {} sky={} fog_method={} fog near/far={:.3}/{:.3} min/max={:.3}/{:.3}",
                                zone_index,
                                identifier.sky_index,
                                identifier.fog_method,
                                identifier.fog_near,
                                identifier.fog_far,
                                identifier.fog_min,
                                identifier.fog_max,
                            ));
                            ui.monospace(format!(
                                "camera distance/elevation={:.3}/{:.3} ambience={:.6} sky_anchor_y={:.3}",
                                identifier.camera_distance,
                                identifier.camera_elevation,
                                identifier.ambience,
                                identifier.sky_anchor_y,
                            ));
                            ui.monospace(format!(
                                "flags=0x{:04X} effects=0x{:04X} background={:02X}{:02X}{:02X}{:02X} fog={:02X}{:02X}{:02X}{:02X}",
                                identifier.flags,
                                identifier.effect_flags,
                                identifier.rgba_back_ground[0],
                                identifier.rgba_back_ground[1],
                                identifier.rgba_back_ground[2],
                                identifier.rgba_back_ground[3],
                                identifier.rgba_fog[0],
                                identifier.rgba_fog[1],
                                identifier.rgba_fog[2],
                                identifier.rgba_fog[3],
                            ));
                        }

                        if let Some(zone_index) = active_zone_index {
                            let connections = current_map
                                .portals
                                .iter()
                                .enumerate()
                                .filter_map(|(portal_index, portal)| {
                                    robots_portal_neighbor_zone(
                                        portal,
                                        zone_index,
                                        current_map.zones.len(),
                                    )
                                    .map(|neighbor| (portal_index, neighbor, portal))
                                })
                                .collect::<Vec<_>>();
                            ui.monospace(format!(
                                "active zone portal connections: {}",
                                connections.len()
                            ));
                            for (portal_index, neighbor, portal) in connections.iter().take(16) {
                                ui.monospace(format!(
                                    "portal #{} -> zone {} flags=0x{:04X} distance={:.3}",
                                    portal_index, neighbor, portal.flags, portal.distance,
                                ));
                            }
                            if connections.len() > 16 {
                                ui.label(format!("… and {} more", connections.len() - 16));
                            }
                        }

                        ui.separator();
                        ui.label(format!(
                            "Serialized cameras {}  portals {}  placement groups {}  isounds {}",
                            current_map.cameras.len(),
                            current_map.portals.len(),
                            current_map.placement_group_count,
                            current_map.isounds.len(),
                        ));
                        egui::CollapsingHeader::new("Serialized cameras")
                            .default_open(false)
                            .show(ui, |ui| {
                                for (index, camera) in current_map.cameras.iter().enumerate() {
                                    ui.horizontal(|ui| {
                                        if ui.small_button("Jump").on_hover_text(
                                            "Editor-only viewport jump to the exact serialized position/look vector. This is not XTrigger_Camera gameplay interpolation.",
                                        ).clicked() {
                                            self.viewer
                                                .lock()
                                                .set_fly_camera_pose(camera.position, camera.look);
                                        }
                                        ui.monospace(format!(
                                            "#{} 0x{:08X} pos=[{:.2},{:.2},{:.2}] look=[{:.3},{:.3},{:.3}] flags=0x{:08X} focal={:.3} aperture={:.3}x{:.3}",
                                            index,
                                            camera.hashcode,
                                            camera.position.x,
                                            camera.position.y,
                                            camera.position.z,
                                            camera.look.x,
                                            camera.look.y,
                                            camera.look.z,
                                            camera.flags,
                                            camera.focal_length,
                                            camera.aperture_width,
                                            camera.aperture_height,
                                        ));
                                    });
                                }
                            });
                        egui::CollapsingHeader::new("Serialized portals")
                            .default_open(false)
                            .show(ui, |ui| {
                                for (index, portal) in current_map.portals.iter().enumerate() {
                                    ui.monospace(format!(
                                        "#{} endpoints={}/{} flags=0x{:04X} distance={:.3} face common=0x{:08X} tex=0x{:08X} flags=0x{:08X} vertices={}",
                                        index,
                                        portal.map_a,
                                        portal.map_b,
                                        portal.flags,
                                        portal.distance,
                                        portal.face_common,
                                        portal.face_texture_ref,
                                        portal.face_flags,
                                        portal.face_vertices.len(),
                                    ));
                                }
                            });
                        if !current_map.isounds.is_empty() {
                            ui.monospace(format!("Raw isounds: {:?}", current_map.isounds));
                        }
                    });
                }

                if active_tab == 2 {
                    control_card(ui, |ui| {
                        render_options_changed |= ui
                            .checkbox(
                                &mut self.show_flag_0x10_geometry,
                                "Geometry with strip flag 0x10",
                            )
                            .on_hover_text("Shows or hides geometry whose serialized strip flags contain bit 0x10. It is loaded and visible by default; the bit is not treated as permission to discard it.")
                            .changed();
                        render_options_changed |= ui
                            .checkbox(&mut self.show_navmesh, "NavMesh")
                            .on_hover_text("Shows or hides the already loaded 0x607 NavMesh without reloading the map.")
                            .changed();
                        render_options_changed |= ui
                            .add_enabled(
                                self.show_navmesh,
                                egui::DragValue::new(&mut self.navmesh_texture_scale)
                                    .range((1.0 / 1024.0)..=4.0)
                                    .max_decimals(4)
                                    .speed(0.005)
                                    .prefix("Nav tex scale "),
                            )
                            .on_hover_text("World-space UV multiplier. Default 0.0625 repeats the texture once per 16 world units.")
                            .changed();
                    });

                    control_card(ui, |ui| {
                        ui.checkbox(&mut self.show_sounds, "Show Sounds")
                            .on_hover_text("Shows serialized EXGeoSound emitters. Click a marker to inspect the sound reference, volume, fades, tracking type and radii.");
                        egui::CollapsingHeader::new("EuroSound settings")
                            .default_open(false)
                            .show(ui, |ui| self.sound_preview.lock().draw_settings(ui));
                    });

                    control_card(ui, |ui| -> anyhow::Result<()> {
                        ui.checkbox(&mut self.show_triggers, "Show Triggers");
                        ui.add_enabled(
                            self.show_triggers,
                            egui::Checkbox::new(&mut self.show_runtime_path, "Runtime Path"),
                        )
                        .on_hover_text("For the selected path-driven Robots Platform/Lift/Vehicle, draws the serialized EXGeoPath nodes and links.");
                        ui.add_enabled(
                            self.show_triggers,
                            egui::Checkbox::new(
                                &mut self.animate_runtime_paths,
                                "Animate Runtime Motion",
                            ),
                        )
                        .on_hover_text("Editor preview of Platform/Lift/Vehicle movement. During Simulate AI, native TriggerManager and event-driven paths run independently of this checkbox, using serialized speeds at 1.0.");
                        if ui
                            .add_enabled(
                                self.show_triggers && self.animate_runtime_paths && !self.simulate_ai,
                                egui::Checkbox::new(
                                    &mut self.native_runtime_event_gate,
                                    "Native Event Gate",
                                ),
                            )
                            .on_hover_text("Editor-only path-event preview gate. During Simulate AI, native TriggerManager is always serviced and paths remain inactive until their real events; this checkbox cannot pause the gameplay clock.")
                            .changed()
                        {
                            self.reset_all_runtime_events();
                            self.native_submap_states.clear();
                        }
                        if ui
                            .checkbox(&mut self.simulate_ai, "Simulate AI")
                            .on_hover_text("Runs fixed-step AI. Gameplay simulation uses actual TriggerManager XItem creation/destruction, zone activation and player proximity; no NPCs are bulk-spawned in Fly/Orbit. Use the separate editor-only checkbox below to preview all NPCs.")
                            .changed()
                        {
                            // Crossing from editor path preview to gameplay
                            // changes who owns the global TriggerManager clock.
                            // Reset the *entire* level timeline, not only AI/RNG:
                            // old XItems, missions or camera sequences must not
                            // leak into a freshly started simulation.
                            self.reset_all_runtime_events();
                            self.native_submap_states.clear();
                        }
                        if self.simulate_ai {
                            if let Some((map_uid, source_index, request)) =
                                self.native_change_level_request
                            {
                                if map_uid == current_map.hashcode {
                                    ui.small(format!(
                                        "ChangeLevel #{source_index}: {:?}, entry context {}. Pending gameplay transition: active Mission owner blocks ChangeLevel; the host loads only verified target EDBs.",
                                        request.target, request.player_entry_context,
                                    ));
                                }
                            }
                            // A map can contain many Player destinations:
                            // m03_hub1 ships 16, routed from Cutscene/Relay.
                            // Picking the first serialized one silently starts
                            // the wrong scene. Reuse the selected trigger as
                            // an explicit diagnostic spawn destination only.
                            let player_selection_before = self.selected_trigger;
                            egui::ComboBox::from_id_salt(("player-creator", current_map.hashcode))
                                .selected_text(
                                    self.selected_trigger
                                        .and_then(|index| current_map.triggers.get(index)
                                            .filter(|t| t.ttype == ROBOTS_TRIGGER_PLAYER)
                                            .map(|_| format!("Player trigger #{index}")))
                                        .unwrap_or_else(|| "Select Player trigger (diagnostic)".to_string()),
                                )
                                .show_ui(ui, |ui| {
                                    for (index, trigger) in current_map.triggers.iter().enumerate() {
                                        if trigger.ttype == ROBOTS_TRIGGER_PLAYER {
                                            ui.selectable_value(
                                                &mut self.selected_trigger,
                                                Some(index),
                                                format!("#{index}  [{:.1}, {:.1}, {:.1}]",
                                                    trigger.position.x,
                                                    trigger.position.y,
                                                    trigger.position.z),
                                            );
                                        }
                                    }
                                });
                            if player_selection_before != self.selected_trigger
                                && self.native_runtime_player_state.is_none()
                            {
                                // Re-anchor Camera preview only for an explicit
                                // diagnostic selection, never mutate gameplay.
                                self.reset_native_camera_player_preview(current_map);
                            }
                            let player_creator = self.selected_trigger.filter(|&index| {
                                current_map.triggers.get(index)
                                    .is_some_and(|trigger| trigger.ttype == ROBOTS_TRIGGER_PLAYER)
                            });
                            if let Some(player_index) = player_creator {
                                ui.collapsing("Player inbound routes (EDB)", |ui| {
                                    let mut count = 0usize;
                                    for (source_index, source) in current_map.triggers.iter().enumerate() {
                                        for (slot, link) in source.links.iter().take(8).enumerate() {
                                            if map_trigger_by_link(current_map, *link)
                                                .is_some_and(|(target, _)| target == player_index)
                                            {
                                                ui.monospace(format!(
                                                    "source #{source_index} type={} link[{slot}]",
                                                    source.ttype,
                                                ));
                                                count += 1;
                                            }
                                        }
                                    }
                                    if count == 0 {
                                        ui.small("No direct incoming event links; context-based Initialise is a separate creation path.");
                                    } else {
                                        ui.small(format!(
                                            "{count} direct link(s). These are possible event routes, not proof of the initial level-entry source."
                                        ));
                                    }
                                });
                            }
                            let can_activate_player =
                                self.native_script_trigger_lifecycle_valid
                                    && self.native_zone_runtime_map == Some(current_map.hashcode)
                                    && self.native_runtime_player_state.is_none();
                            // Native GameWnd +0x308 (DAT_007B3000) is independent
                            // of editor Player selection. Keep diagnostic context
                            // unset until explicitly chosen; no guessed level start.
                            egui::ComboBox::from_id_salt(("player-context", current_map.hashcode))
                                .selected_text(
                                    self.native_player_context_diagnostic
                                        .map(|context| format!("Game context {context} [diagnostic]"))
                                        .unwrap_or_else(|| "Game context unknown".to_owned())
                                )
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut self.native_player_context_diagnostic,
                                        None,
                                        "Unknown (original source not reconstructed)",
                                    );
                                    let mut contexts = std::collections::BTreeSet::new();
                                    for trigger in current_map.triggers.iter()
                                        .filter(|trigger| trigger.ttype == ROBOTS_TRIGGER_PLAYER)
                                    {
                                        let context = trigger.data.first().copied()
                                            .flatten().unwrap_or_default();
                                        if contexts.insert(context) {
                                            ui.selectable_value(
                                                &mut self.native_player_context_diagnostic,
                                                Some(context),
                                                format!("Context {context} (0x{context:08X})"),
                                            );
                                        }
                                    }
                                });
                            if let Some(context) = self.native_player_context_diagnostic {
                                let candidate =
                                    Self::native_player_initialise_candidate(current_map, context);
                                ui.monospace(format!(
                                    "Native Initialise context {context} -> Player {}",
                                    candidate.map(|i| format!("#{i}")).unwrap_or_else(|| "none".to_string())
                                ));
                                if ui
                                    .add_enabled(
                                        can_activate_player && candidate.is_some(),
                                        egui::Button::new("Initialise Player by context [diagnostic]"),
                                    )
                                    .on_hover_text(
                                        "Emulates the proved GameWnd +0x308 context equality and singleton owner gate in serialized InitialiseTriggers order. Context was selected manually and is NOT the recovered game's level-start context. Other triggers are not auto-activated.",
                                    )
                                    .clicked()
                                {
                                    let wall_time = ui.input(|input| input.time);
                                    self.initialise_native_player_for_context(
                                        current_map, context, wall_time,
                                    );
                                }
                            }
                            if ui
                                .add_enabled(
                                    can_activate_player && player_creator.is_some(),
                                    egui::Button::new("Activate Player XItem (0x100) [diagnostic]"),
                                )
                                .on_hover_text("Manually delivers the shipped Player activation event through the existing class dispatcher. Does not start other NPCs/triggers and does not claim original level-start initialization or Player Physics are implemented.")
                                .clicked()
                            {
                                if let Some(trigger_index) = player_creator {
                                    let wall_time = ui.input(|input| input.time);
                                    self.dispatch_runtime_event(
                                        current_map,
                                        trigger_index,
                                        ROBOTS_EVENT_ACTIVATE,
                                        wall_time,
                                    );
                                }
                            }
                            if ui
                                .checkbox(&mut self.preview_all_ai, "Preview ALL NPCs (editor-only)")
                                .on_hover_text("Diagnostic only: ignores native zone/distance/spawn constraints in Fly/Orbit and starts all decoded NPC/Monster owners. Disable for real level progression. Native Camera mode always uses actual TriggerManager ownership.")
                                .changed()
                            {
                                self.reset_native_ai_runtime_state();
                            }
                            if self.native_script_trigger_lifecycle_valid {
                                let created = self
                                    .native_ai_trigger_lifecycle
                                    .values()
                                    .filter(|state| state.xitem_exists)
                                    .count();
                                ui.monospace(format!(
                                    "Level lifecycle: TriggerManager ({created} AI XItems live)"
                                ));
                            } else if self.preview_all_ai {
                                ui.monospace(format!(
                                    "Editor preview: {} AI owners (not gameplay)",
                                    self.native_ai_preview_live.len()
                                ));
                            } else {
                                ui.label(
                                    "Level lifecycle unavailable: select a valid native Camera view. No NPCs auto-spawn in Fly/Orbit.",
                                );
                            }
                        }
                        if self.simulate_ai
                            || !self.native_transient_script_xitems.is_empty()
                            || !self.native_script_xitem_spawns.is_empty()
                            || !self.native_dynamic_pickups.is_empty()
                            || !self.native_dynamic_pickup_spawns.is_empty()
                        {
                            ui.label(format!(
                                "Runtime Script XItems: {} active, {} queued",
                                self.native_transient_script_xitems.len(),
                                self.native_script_xitem_spawns.len()
                            ))
                            .on_hover_text(
                                "Gameplay-created XItemHandler_Script instances emitted by recovered AI/projectile/explosion gameplay. They use the same Script/Particle/Light adapters intended for the UE runtime.",
                            );
                            ui.label(format!(
                                "Dynamic Pickups: {} live, {} queued",
                                self.native_dynamic_pickups.len(),
                                self.native_dynamic_pickup_spawns.len()
                            ))
                            .on_hover_text(
                                "Gameplay-created XItemHandler_Pickup owners from GeneratePickupFromHash. Serialized map Pickup triggers remain a separate ownership path.",
                            );
                        }
                        ui.add_enabled(
                            self.show_triggers && self.animate_runtime_paths,
                            egui::DragValue::new(&mut self.runtime_path_playback_speed)
                                .range(0.0..=10.0)
                                .max_decimals(2)
                                .speed(0.05)
                                .prefix("Motion speed scale "),
                        )
                        .on_hover_text("Multiplier for the recovered per-trigger speed. Default 1.0 matches the map values.");
                        ui.add_enabled(
                            self.show_triggers && self.animate_runtime_paths,
                            egui::DragValue::new(&mut self.platform_rotation_speed_scale)
                                .range(0.0..=4.0)
                                .max_decimals(2)
                                .speed(0.05)
                                .prefix("Rotation scale "),
                        )
                        .on_hover_text("Multiplier for the serialized Platform angular velocity in degrees per second.");
                        ui.add(
                            egui::DragValue::new(&mut self.trigger_scale)
                                .range(0.1..=2.0)
                                .max_decimals(2)
                                .speed(0.05)
                                .prefix("Trigger scale "),
                        );

                        ui.separator();
                        let mut reload_requested = false;
                        let mut definitions_changed = false;
                        ui.horizontal(|ui| {
                            let definitions = egui::ComboBox::from_label("Definitions")
                                .selected_text(&self.selected_triginfo_path)
                                .width(164.0)
                                .show_ui(ui, |ui| {
                                    let mut response = ui.selectable_value(
                                        &mut self.selected_triginfo_path,
                                        "None".to_string(),
                                        "None",
                                    );
                                    for path in &self.available_triginfo_paths {
                                        response = response.union(ui.selectable_value(
                                            &mut self.selected_triginfo_path,
                                            path.to_string(),
                                            path,
                                        ));
                                    }
                                    response
                                });
                            definitions_changed = definitions
                                .inner
                                .map(|response| response.changed())
                                .unwrap_or_default();
                            if ui
                                .button("\u{f2f1}")
                                .on_hover_text("Reload definitions")
                                .clicked()
                            {
                                reload_requested = true;
                            }
                        });
                        if definitions_changed || reload_requested {
                            if self.selected_triginfo_path.is_empty()
                                || self.selected_triginfo_path == "None"
                            {
                                self.trigger_info = Default::default();
                            } else {
                                self.reload_trigger_defs()?;
                            }
                        }
                        Ok(())
                    })
                    .inner?;
                }

                if active_tab == 3 {
                    control_card(ui, |ui| {
                        if ui
                            .checkbox(&mut self.animate_scripts, "Animate Scripts")
                            .changed()
                        {
                            self.script_animation_start_time = None;
                        }
                        ui.add_enabled(
                            self.animate_scripts,
                            egui::DragValue::new(&mut self.script_playback_speed)
                                .range(0.0..=4.0)
                                .max_decimals(2)
                                .speed(0.05)
                                .prefix("Script speed "),
                        );
                        ui.add_enabled(
                            self.animate_scripts,
                            egui::DragValue::new(&mut self.fan_runtime_value)
                                .range(-10_000..=10_000)
                                .speed(1)
                                .prefix("Fan XItem +0x6C "),
                        )
                        .on_hover_text("Explicit gameplay-runtime input for XItemHandler_FanHorizontal. The value is not serialized by the standalone AnimScript or proven to come from a FanHorizontal trigger data slot.");
                        ui.checkbox(&mut self.particle_settings.enabled, "Native Particles")
                            .on_hover_text("Uses native EXParticleSys simulation: serialized rate and pool, lifetime variance, emitter box, angular/speed distribution, acceleration, damping, render selectors and appended RGBA/scale/rotation curves.");
                    });
                }

                Ok(())
            });

        ctx.data_mut(|data| data.insert_temp(tab_id, active_tab));

        if let Some(response) = response {
            if let Some(inner) = response.inner {
                inner?;
            }
        }

        if render_options_changed {
            self.navmesh_texture_scale = self.navmesh_texture_scale.clamp(1.0 / 1024.0, 4.0);
            self.apply_entity_render_options();
        }

        Ok(())
    }
}
