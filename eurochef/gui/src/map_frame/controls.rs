use super::*;

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
        let active_zone_index =
            robots_map_zone_index_by_bounds(current_map.zones.len(), camera_position, |index| {
                let zone = &current_map.zones[index];
                (
                    Vec3::from(zone.bounds_box[0]),
                    Vec3::from(zone.bounds_box[1]),
                )
            });

        self.textfield_focused = false;
        let mut render_options_changed = false;
        let response = egui::Window::new("Map Controls")
            .default_pos(egui::pos2(12.0, 72.0))
            .default_width(340.0)
            .min_width(290.0)
            .scroll([false, true])
            .show(ctx, |ui| -> anyhow::Result<()> {
                if let Some(dev_map) = robots_dev_map_info(self.file) {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.strong(format!("DEV MAP: {}", dev_map.label));
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

                egui::CollapsingHeader::new("Scene & Lighting")
                    .default_open(true)
                    .show(ui, |ui| {
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
                                    ui.label("Serialized EXGeoIdentifier.ambience values are retained per exact zone but are not applied until an arithmetic consumer is proven.");
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

                egui::CollapsingHeader::new("Map runtime")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.checkbox(
                            &mut self.preview_zone_background,
                            "Serialized zone background preview",
                        )
                        .on_hover_text(
                            "Clears the map viewport with EXGeoIdentifier.rgba_back_ground from the exact active MapZone. This is a direct serialized preview; native fog composition remains separate.",
                        );
                        ui.checkbox(&mut self.show_portals, "Portals")
                            .on_hover_text("Draws the four serialized EXGeoPortal boundary edges. Portal traversal and level-state transfer are not fabricated.");
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
                                        "mode3 player substitutions: Y={} XZ={}",
                                        plan.mode3_override_player_y,
                                        plan.mode3_override_player_xz,
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
                                ui.small(
                                    "This mirrors the native controller command plan. It does not pretend that the editor owns the game's player/camera-controller state.",
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
                                "camera distance/elevation={:.3}/{:.3} ambience={:.6}",
                                identifier.camera_distance,
                                identifier.camera_elevation,
                                identifier.ambience,
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

                egui::CollapsingHeader::new("Geometry")
                    .default_open(true)
                    .show(ui, |ui| {
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

                egui::CollapsingHeader::new("Audio")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.checkbox(&mut self.show_sounds, "Show Sounds")
                            .on_hover_text("Shows serialized EXGeoSound emitters. Click a marker to inspect the sound reference, volume, fades, tracking type and radii.");
                        egui::CollapsingHeader::new("EuroSound settings")
                            .default_open(false)
                            .show(ui, |ui| self.sound_preview.lock().draw_settings(ui));
                    });

                egui::CollapsingHeader::new("Triggers & Runtime")
                    .default_open(true)
                    .show(ui, |ui| -> anyhow::Result<()> {
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
                        .on_hover_text("Uses per-trigger Platform/Lift/Vehicle path speed from the serialized map, rotates vehicles along the path tangent, and previews Platform angular velocity.");
                        if ui
                            .add_enabled(
                                self.show_triggers && self.animate_runtime_paths,
                                egui::Checkbox::new(
                                    &mut self.native_runtime_event_gate,
                                    "Native Event Gate",
                                ),
                            )
                            .on_hover_text("Starts from the constructor-proven inactive state. Platform/Lift/Vehicle movement reacts to manual 0x100/0x200 events and instruction-proven path-node opcodes 4/8. Opcode 9, class-specific node values and physical contacts remain diagnostic.")
                            .changed()
                        {
                            self.reset_all_runtime_events();
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
                    .body_returned
                    .transpose()?;

                egui::CollapsingHeader::new("Scripts & Particles")
                    .default_open(true)
                    .show(ui, |ui| {
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

                Ok(())
            });

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
