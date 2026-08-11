use super::*;

impl MapFrame {
    pub(super) fn show_canvas(
        &mut self,
        ui: &mut egui::Ui,
        context: &egui::Context,
        map: &ProcessedMap,
    ) {
        let (rect, response) = ui.allocate_exact_size(
            ui.available_size() - egui::vec2(0., 16.),
            egui::Sense::click_and_drag(),
        );
        let time: f64 = ui.input(|input| input.time);
        let runtime_start = *self.runtime_motion_start_time.get_or_insert(time);
        let runtime_time = (time - runtime_start).max(0.0) as f32;
        let runtime_event_snapshots = self.runtime_event_snapshots(map, time);
        let script_start = *self.script_animation_start_time.get_or_insert(time);
        let script_global_time = (time - script_start).max(0.0) as f32;

        if response.clicked() && (self.show_triggers || self.show_sounds) {
            if let Some(pointer_pos) = response.interact_pointer_pos() {
                self.render_pickbuffer(rect.size(), map, runtime_time, &runtime_event_snapshots);
                let mut pixel = [0u8; 4];
                if let Some((x, y)) = pickbuffer_pixel_position(response.rect, pointer_pos) {
                    unsafe {
                        self.gl
                            .bind_framebuffer(glow::FRAMEBUFFER, self.pickbuffer.framebuffer);
                        self.gl.read_buffer(glow::COLOR_ATTACHMENT0);
                        self.gl.read_pixels(
                            x,
                            y,
                            1,
                            1,
                            glow::RGBA,
                            glow::UNSIGNED_BYTE,
                            glow::PixelPackData::Slice(Some(&mut pixel)),
                        );
                        self.gl.bind_framebuffer(glow::FRAMEBUFFER, None);
                    }
                }

                let picked = decode_pick_value(pixel);
                self.selected_trigger = picked
                    .filter(|(ty, _)| *ty == PickBufferType::Trigger as u32)
                    .map(|(_, id)| id as usize)
                    .filter(|id| *id < map.triggers.len());
                self.selected_sound = picked
                    .filter(|(ty, _)| *ty == PickBufferType::Sound as u32)
                    .map(|(_, id)| id as usize)
                    .filter(|id| *id < map.sounds.len());
            }
        }

        self.draw_trigger_inspector(context, map);
        self.draw_sound_inspector(context, map);

        if self.animate_scripts || self.animate_runtime_paths {
            context.request_repaint();
        }

        self.update_native_camera_player_preview(ui, &response, map, time);
        self.sync_native_camera_viewport(map, time);
        if self
            .native_camera_runtime
            .as_ref()
            .is_some_and(|runtime| runtime.is_transitioning())
            || (self.native_camera_live_player_preview
                && self.apply_native_camera_viewport
                && self.active_camera_trigger.is_some())
        {
            context.request_repaint();
        }

        let viewer = self.viewer.clone();
        let (camera_pos, camera_rotation, camera_view_projection, camera_orthographic) = {
            let mut v = viewer.lock();
            if !self.textfield_focused {
                v.update(ui, &response);
            }

            let orbit_orthographic = v.selected_camera == CameraType::Orbit && v.orthographic;
            let camera = v.camera_mut();
            let camera_pos = camera.position();
            let camera_rotation = camera.rotation();
            let vertical_fov = camera.vertical_fov_radians();
            let camera_zoom = camera.zoom();
            let camera_view = camera.calculate_matrix();
            let aspect_ratio = (rect.width() / rect.height().max(1.0)).max(1.0e-6);
            let aspect_ratio_vert = (1.0 / aspect_ratio).max(1.0);
            let orthographic = orbit_orthographic && vertical_fov.is_none();
            let mut projection = if orthographic {
                glam::camera::rh::proj::opengl::orthographic(
                    (-(aspect_ratio * aspect_ratio_vert) * -camera_zoom) * 2.0,
                    ((aspect_ratio * aspect_ratio_vert) * -camera_zoom) * 2.0,
                    (aspect_ratio_vert * -camera_zoom) * 2.0,
                    (-aspect_ratio_vert * -camera_zoom) * 2.0,
                    -2500.0,
                    2500.0,
                )
            } else {
                glam::camera::rh::proj::directx::perspective(
                    vertical_fov.unwrap_or_else(|| 2.0 * aspect_ratio_vert.atan()),
                    aspect_ratio,
                    0.02,
                    2000.0,
                )
            };
            if !orthographic {
                projection.x_axis = -projection.x_axis;
            }

            if let Some(tween) = &mut self.trigger_focus_tween {
                if tween.is_finished() {
                    self.trigger_focus_tween = None;
                } else {
                    let p = tween.update();
                    v.focus_on_point(p, self.trigger_scale);
                }
            }

            (
                camera_pos,
                camera_rotation,
                projection * camera_view,
                orthographic,
            )
        };

        let active_zone_index = map.native_zone_index(camera_pos);
        // Robots uses the perspective portal path recovered from 0x004ED38B.
        // EuroChef's editor-only orthographic camera has no native equivalent,
        // so keep its visual frame pinned to the BSP root instead of feeding an
        // incompatible OpenGL-depth projection through the native portal test.
        let visual_frame = if camera_orthographic {
            let mut frame_depths = vec![0; map.zones.len()];
            let ordered = active_zone_index.into_iter().collect::<Vec<_>>();
            if let Some(root_zone) = ordered.first().copied() {
                frame_depths[root_zone] = 1;
            }
            NativeVisualZoneFrame {
                ordered,
                frame_depths,
                blocked_depth_writes: vec![None; map.zones.len()],
            }
        } else {
            map.native_visual_zone_frame(camera_pos, camera_view_projection)
        };
        let streaming_requested = map.native_streaming_request_zone_indices(camera_pos);
        let ready_resource_mask =
            self.sync_native_zone_runtime(map, &visual_frame, &streaming_requested);
        let active_visual_zones = visual_frame
            .ordered
            .iter()
            .copied()
            .filter(|zone_index| {
                self.native_zone_runtime
                    .get(*zone_index)
                    .is_some_and(|state| state.activated)
            })
            .collect::<Vec<_>>();
        let zone_background_color =
            active_zone_index.map(|index| map.zones[index].identifier.rgba_back_ground);

        self.sync_map_ambient_audio(
            map,
            self.file,
            script_global_time,
            runtime_time,
            camera_pos,
            camera_rotation,
            context,
        );

        // TODO(cohae): How do we get out of this situation
        let map = map.clone(); // FIXME(cohae): ugh.
        let sky_zones = map
            .zones
            .iter()
            .map(|zone| super::MapSkyZoneState {
                bounds_min: Vec3::from(zone.bounds_box[0]),
                bounds_max: Vec3::from(zone.bounds_box[1]),
                sky_index: zone.identifier.sky_index,
                identifier_flags: zone.identifier.flags,
                sky_anchor_y: zone.identifier.sky_anchor_y,
            })
            .collect::<Vec<_>>();
        // Robots.exe 0x004ED203/0x004ED920 builds visual portal record 0 and
        // 0x004EDDDB flattens it into the +0x28/+0x2C zone-pointer array read by
        // 0x004EC2AA. This is deliberately distinct from the +0x6E streaming
        // request traversal built later in the frame.
        let sky_selection = map_sky_selection(
            &self.sky_ent,
            &map.skies,
            &sky_zones,
            &active_visual_zones,
            camera_pos,
        );
        if let Some(selection) = sky_selection {
            robots_sky_cache_activate(&mut self.native_sky_cache, selection);
        }
        let sky_cache_diagnostic = self
            .native_sky_cache
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                entry.map(|entry| (index, entry.object, entry.pending_removal))
            })
            .collect::<Vec<_>>();
        let sky_diagnostic = match sky_selection {
            Some(selection) => {
                let runtime_state = selection
                    .zone_index
                    .and_then(|zone_index| self.native_zone_runtime.get(zone_index))
                    .copied()
                    .unwrap_or_default();
                let frame_depth = selection
                    .zone_index
                    .and_then(|zone_index| visual_frame.frame_depths.get(zone_index))
                    .copied()
                    .unwrap_or(0);
                let stream_now = selection
                    .zone_index
                    .is_some_and(|zone_index| streaming_requested.contains(&zone_index));
                let resource_ref = selection
                    .zone_index
                    .and_then(|zone_index| map.zones.get(zone_index))
                    .map(|zone| zone.zone_resource_ref)
                    .unwrap_or(0);
                format!(
                    "Camera [{:.3}, {:.3}, {:.3}]  bsp_zone={} visual={:?} active={:?} stream={:?} ready_res={:08X?} sky_cache={:?} sky_zone={} resource=0x{:08X} state[6A={} 6B={} 6C={} 6D={} 6E={}] sky_index={} object=0x{:08X} root=[{:.3}, {:.3}, {:.3}]  {}",
                    camera_pos.x,
                    camera_pos.y,
                    camera_pos.z,
                    active_zone_index
                        .map(|index| index.to_string())
                        .unwrap_or_else(|| "none".to_string()),
                    visual_frame.ordered,
                    active_visual_zones,
                    streaming_requested,
                    ready_resource_mask,
                    sky_cache_diagnostic,
                    selection
                        .zone_index
                        .map(|index| index.to_string())
                        .unwrap_or_else(|| "override".to_string()),
                    resource_ref,
                    u8::from(runtime_state.activated),
                    runtime_state.visual_depth,
                    u8::from(runtime_state.stream_latched),
                    frame_depth,
                    u8::from(stream_now),
                    selection
                        .sky_index
                        .map(|index| index.to_string())
                        .unwrap_or_else(|| "override".to_string()),
                    selection.object,
                    selection.root_translation.x,
                    selection.root_translation.y,
                    selection.root_translation.z,
                    match (selection.zone_index, selection.contains_camera) {
                        (None, _) => "override",
                        (Some(zone), true) if Some(zone) == active_zone_index => "bsp root owns sky",
                        (Some(_), true) => "visible sky zone also contains camera",
                        (Some(_), false) => "visible portal sky zone",
                    }
                )
            }
            None => format!(
                "Camera [{:.3}, {:.3}, {:.3}]  bsp_zone={} visual={:?} active={:?} stream={:?} ready_res={:08X?} sky_cache={:?}  no activated visual zone with sky_index >= 0",
                camera_pos.x,
                camera_pos.y,
                camera_pos.z,
                active_zone_index
                    .map(|index| index.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                visual_frame.ordered,
                active_visual_zones,
                streaming_requested,
                ready_resource_mask,
                sky_cache_diagnostic,
            ),
        };
        if self.sky_diagnostic != sky_diagnostic {
            eprintln!("[Robots] map sky selection: {sky_diagnostic}");
            self.sky_diagnostic = sky_diagnostic;
        }
        // 0x004ECB24 only advances cached animator state through
        // 0x004ECB98 -> 0x004E9123. The transform/render submit is the distinct
        // 0x004ECA75 -> 0x004E9188 path invoked by 0x004EC921 for the sky chosen
        // this frame, so a cached-but-unselected sky must not be drawn.
        let sky_sources = sky_selection
            .map(|selection| {
                (
                    selection.object,
                    selection.root_translation,
                    selection.zone_index,
                )
            })
            .into_iter()
            .collect::<Vec<_>>();
        let default_trigger_icon = self.default_trigger_icon;
        let billboard_renderer = self.billboard_renderer.clone();
        let particle_renderer = self.particle_renderer.clone();
        let particle_settings = self.particle_settings;
        let link_renderer = self.link_renderer.clone();
        let selected_trigger = self.selected_trigger;
        let selected_sound = self.selected_sound;
        let select_renderer = self.select_renderer.clone();
        let show_triggers = self.show_triggers;
        let show_sounds = self.show_sounds;
        let show_runtime_path = self.show_runtime_path;
        let animate_runtime_paths = self.animate_runtime_paths;
        let runtime_path_playback_speed = self.runtime_path_playback_speed;
        let platform_rotation_speed_scale = self.platform_rotation_speed_scale;
        let animate_scripts = self.animate_scripts;
        let script_playback_speed = self.script_playback_speed;
        let fan_runtime_value = self.fan_runtime_value;
        let trigger_scale = self.trigger_scale;
        let sound_scale = self.sound_scale;
        let hovered_link = self.selected_link;
        let trigger_info = self.trigger_info.clone();
        let trigger_icons = self.trigger_icons.clone();
        let render_filter = self.render_filter;
        let preview_zone_background = self.preview_zone_background;
        let show_portals = self.show_portals;
        let global_lighting_enabled = self.global_lighting;
        let native_lights_enabled = self.native_lights;
        let native_light_strength = self.native_light_strength;
        let global_lightmap = self.global_lightmap.clone();

        let render_store = self.render_store.clone();
        let current_file = self.file;

        let collision_renderer = self.collision_renderer.clone();
        let renderers = self.ref_renderers.clone();
        let cb = egui_glow::CallbackFn::new(move |info, painter| unsafe {
            let mut v = viewer.lock();
            v.uniforms.global_lighting_enabled = global_lighting_enabled;
            v.uniforms.global_lighting = robots_global_lighting(current_file);
            v.uniforms.native_lights_enabled = native_lights_enabled;
            v.uniforms.native_light_strength = native_light_strength.max(0.0);
            v.uniforms.native_lights = map
                .lights
                .iter()
                .map(|light| NativeLight {
                    position: light.position,
                    direction: light.beam,
                    colour: robots_native_light_colour(light.colour),
                    flags: light.flags,
                    radius: light.radius.max(0.0),
                    effect_fraction: light.max_effect_fraction,
                    light_type: light.light_type,
                    beam_angle_degrees: light.beam_angle as f32,
                })
                .collect();
            v.uniforms.native_light_zones = map
                .zones
                .iter()
                .map(|zone| {
                    let a = Vec3::from(zone.bounds_box[0]);
                    let b = Vec3::from(zone.bounds_box[1]);
                    NativeLightZone {
                        bounds_min: a.min(b),
                        bounds_max: a.max(b),
                        light_indices: zone
                            .light_array
                            .iter()
                            .map(|index| *index as usize)
                            .collect(),
                    }
                })
                .collect();
            v.uniforms.native_fog_zones = map
                .zones
                .iter()
                .map(|zone| RobotsFog {
                    enabled: zone.identifier.fog_method != 0,
                    near: zone.identifier.fog_near,
                    far: zone.identifier.fog_far,
                    min: zone.identifier.fog_min,
                    max: zone.identifier.fog_max,
                    colour: Vec3::new(
                        zone.identifier.rgba_fog[0] as f32,
                        zone.identifier.rgba_fog[1] as f32,
                        zone.identifier.rgba_fog[2] as f32,
                    ) / 255.0,
                })
                .collect();
            v.uniforms.native_fog_zone_override = None;
            let lightmap = {
                let mut cached = global_lightmap.lock();
                if cached
                    .as_ref()
                    .is_none_or(|(hash, _)| *hash != map.hashcode)
                {
                    let baked = crate::render::global_lightmap::bake(
                        painter.gl(),
                        v.shaders.global_lightmap,
                        &map.lighting_triangles,
                    )
                    .map(Arc::new);
                    *cached = baked.map(|lightmap| (map.hashcode, lightmap));
                    painter.gl().viewport(
                        0,
                        0,
                        info.viewport.width() as i32,
                        info.viewport.height() as i32,
                    );
                }
                cached.as_ref().map(|(_, lightmap)| lightmap.clone())
            };
            v.uniforms.global_lightmap = lightmap;
            v.uniforms.native_lighting_triangles.clear();
            if preview_zone_background {
                if let Some(rgba) = zone_background_color {
                    painter.gl().clear_color(
                        rgba[0] as f32 / 255.0,
                        rgba[1] as f32 / 255.0,
                        rgba[2] as f32 / 255.0,
                        rgba[3] as f32 / 255.0,
                    );
                    painter.gl().clear(glow::COLOR_BUFFER_BIT);
                }
            }
            v.start_render(painter.gl(), info.viewport.aspect_ratio(), time as f32);
            let mut sky_uniforms = v.uniforms.clone();
            // Only camera-relative members stay in the unlit sky pass. Map-space
            // facade/decor members are transferred into the ordinary scene queue.
            sky_uniforms.global_lighting_enabled = false;
            sky_uniforms.global_lighting = None;
            sky_uniforms.global_lightmap = None;
            sky_uniforms.native_lights_enabled = false;
            sky_uniforms.native_fog_zone_override = sky_sources
                .first()
                .and_then(|(_, _, zone_index)| *zone_index);
            let mut sky_queue = Vec::<QueuedEntityRender>::new();
            for (sky, root_translation, _) in sky_sources.iter().copied() {
                match sky.base() {
                    0x02000000 => sky_queue.push(QueuedEntityRender {
                        entity: (current_file, sky),
                        entity_alt: None,
                        position: root_translation,
                        rotation: Quat::IDENTITY,
                        scale: Vec3::ONE,
                    }),
                    0x04000000 => {
                        let sky_time = render_store
                            .read()
                            .get_script(current_file, sky)
                            .map(|script| script.time_at_frame(1.0))
                            .unwrap_or_default();
                        render_static_script(
                            root_translation,
                            Quat::IDENTITY,
                            Vec3::ONE,
                            current_file,
                            sky,
                            sky_time,
                            &render_store.read(),
                            &mut |queued| sky_queue.push(queued),
                            vec![],
                        );
                    }
                    _ => {}
                }
            }

            let sky_world_queue = sky_queue;

            let mut render_queue = Vec::<QueuedEntityRender>::new();
            let mut particle_queue = vec![];
            let mut dynamic_light_queue = vec![];

            // Render base (ref) entities
            if render_filter.contains(RenderFilter::MapZone) {
                for (_, r) in renderers.iter().filter(|(i, _)| *i == map.hashcode) {
                    render_queue.push(QueuedEntityRender {
                        entity: (current_file, 0),
                        entity_alt: Some(r.clone()), // TODO(cohae): Find an alternative for rendering ref-entities with the new system
                        position: Vec3::ZERO,
                        rotation: Quat::IDENTITY,
                        scale: Vec3::ONE,
                    })
                }
            }

            if render_filter.contains(RenderFilter::Placements) {
                for p in &map.placements {
                    let position: Vec3 = p.position.into();
                    let rotation: Quat = Quat::from_euler(
                        glam::EulerRot::ZXY,
                        p.rotation[2],
                        p.rotation[0],
                        p.rotation[1],
                    );
                    let scale: Vec3 = p.scale.into();

                    match p.object_ref.base() {
                        0x0200_0000 => render_queue.push(QueuedEntityRender {
                            entity: (current_file, p.object_ref),
                            entity_alt: None,
                            position,
                            rotation,
                            scale,
                        }),
                        0x0400_0000 => {
                            let script_time = resolved_map_script_time(
                                &render_store.read(),
                                current_file,
                                p.object_ref,
                                script_global_time,
                                animate_scripts,
                                script_playback_speed,
                            );
                            render_script(
                                position,
                                rotation,
                                scale,
                                current_file,
                                p.object_ref,
                                script_time,
                                &render_store.read(),
                                &mut |queued| render_queue.push(queued),
                                vec![],
                            );
                            collect_script_particles(
                                position,
                                rotation,
                                scale,
                                current_file,
                                p.object_ref,
                                script_time,
                                &render_store.read(),
                                &mut particle_queue,
                                vec![],
                            );
                            collect_script_dynamic_lights(
                                position,
                                rotation,
                                scale,
                                current_file,
                                p.object_ref,
                                script_time,
                                &render_store.read(),
                                &mut dynamic_light_queue,
                                vec![],
                            );
                        }
                        _ => {}
                    }
                }
            }

            // ROBOTS_PATCH_0024_ACTUAL_VISUAL_GEOMETRY
            let mut resolved_trigger_visuals = vec![false; map.triggers.len()];

            if render_filter.contains(RenderFilter::Triggers) {
                for (i, t) in map.triggers.iter().enumerate() {
                    let trigger_queue_start = render_queue.len();
                    let event_snapshot = runtime_event_snapshots.get(i).copied().flatten();
                    let trigger_position = runtime_path_preview_position_with_event(
                        &map,
                        t,
                        runtime_time,
                        animate_runtime_paths,
                        runtime_path_playback_speed,
                        event_snapshot,
                    );
                    if let Some(v) = t.engine_options.visual_object {
                        let rotation = runtime_trigger_preview_rotation_with_event(
                            &map,
                            t,
                            runtime_time,
                            animate_runtime_paths,
                            runtime_path_playback_speed,
                            platform_rotation_speed_scale,
                            event_snapshot,
                        );

                        match v.base() {
                            0x02000000 => {
                                let visual_file = trigger_visual_file(
                                    current_file,
                                    v,
                                    t.engine_options.visual_object_file,
                                );

                                // Some triggers serialize only the body Entity while an
                                // AnimScript in the same EDB owns the complete assembly.
                                // Use the assembly whenever the requested body participates
                                // in a multi-Entity script; fall back to the body otherwise.
                                let assembly =
                                    render_store.read().find_assembly_script(visual_file, v);
                                if let Some(script_hashcode) = assembly {
                                    let script_time = resolved_map_script_time(
                                        &render_store.read(),
                                        visual_file,
                                        script_hashcode,
                                        script_global_time,
                                        animate_scripts,
                                        script_playback_speed,
                                    );
                                    let queue_start = render_queue.len();
                                    render_script(
                                        trigger_position,
                                        rotation,
                                        t.scale,
                                        visual_file,
                                        script_hashcode,
                                        script_time,
                                        &render_store.read(),
                                        &mut |q| render_queue.push(q),
                                        vec![],
                                    );
                                    if t.ttype == 80 && animate_runtime_paths {
                                        let maximum_speed =
                                            robots_trigger_runtime_path_speed(t.ttype, &t.data)
                                                .unwrap_or_default()
                                                * runtime_path_playback_speed.max(0.0);
                                        let acceleration =
                                            robots_trigger_runtime_path_acceleration(
                                                t.ttype, &t.data,
                                            )
                                            .unwrap_or_default();
                                        if let Some(snapshot) = event_snapshot {
                                            let angle = robots_vehicle_wheel_roll_angle(
                                                snapshot.elapsed_seconds,
                                                maximum_speed,
                                                acceleration,
                                                runtime_path_playback_speed.max(0.0),
                                            );
                                            apply_vehicle_wheel_roll_angle(
                                                &mut render_queue[queue_start..],
                                                &render_store.read(),
                                                angle,
                                            );
                                            if let Some(steering_angle) =
                                                snapshot.vehicle_steering_angle
                                            {
                                                apply_vehicle_steering_wheel_angle(
                                                    &mut render_queue[queue_start..],
                                                    &render_store.read(),
                                                    steering_angle,
                                                );
                                            }
                                        } else {
                                            apply_vehicle_wheel_roll(
                                                &mut render_queue[queue_start..],
                                                &render_store.read(),
                                                runtime_time,
                                                maximum_speed,
                                                acceleration,
                                                runtime_path_playback_speed.max(0.0),
                                            );
                                            if let Some(steering_angle) =
                                                robots_vehicle_steering_wheel_angle(
                                                    &map,
                                                    t,
                                                    runtime_time,
                                                    runtime_path_playback_speed,
                                                )
                                            {
                                                apply_vehicle_steering_wheel_angle(
                                                    &mut render_queue[queue_start..],
                                                    &render_store.read(),
                                                    steering_angle,
                                                );
                                            }
                                        }
                                    }
                                    collect_script_particles(
                                        trigger_position,
                                        rotation,
                                        t.scale,
                                        visual_file,
                                        script_hashcode,
                                        script_time,
                                        &render_store.read(),
                                        &mut particle_queue,
                                        vec![],
                                    );
                                    collect_script_dynamic_lights(
                                        trigger_position,
                                        rotation,
                                        t.scale,
                                        visual_file,
                                        script_hashcode,
                                        script_time,
                                        &render_store.read(),
                                        &mut dynamic_light_queue,
                                        vec![],
                                    );
                                    resolved_trigger_visuals[i] = render_queue.len() > queue_start;
                                    if resolved_trigger_visuals[i] {
                                        continue;
                                    }
                                }

                                if render_store.read().get_entity(visual_file, v).is_some() {
                                    render_queue.push(QueuedEntityRender {
                                        entity: (visual_file, v),
                                        entity_alt: None,
                                        position: trigger_position,
                                        rotation,
                                        scale: t.scale,
                                    });
                                    resolved_trigger_visuals[i] = true;
                                }
                            }
                            0x04000000 => {
                                let visual_file = trigger_visual_file(
                                    current_file,
                                    v,
                                    t.engine_options.visual_object_file,
                                );
                                let script_time = resolved_map_script_time(
                                    &render_store.read(),
                                    visual_file,
                                    v,
                                    script_global_time,
                                    animate_scripts,
                                    script_playback_speed,
                                );

                                let queue_start = render_queue.len();

                                render_script(
                                    trigger_position,
                                    rotation,
                                    t.scale,
                                    visual_file,
                                    v,
                                    script_time,
                                    &render_store.read(),
                                    &mut |q| render_queue.push(q),
                                    vec![],
                                );
                                collect_script_particles(
                                    trigger_position,
                                    rotation,
                                    t.scale,
                                    visual_file,
                                    v,
                                    script_time,
                                    &render_store.read(),
                                    &mut particle_queue,
                                    vec![],
                                );
                                collect_script_dynamic_lights(
                                    trigger_position,
                                    rotation,
                                    t.scale,
                                    visual_file,
                                    v,
                                    script_time,
                                    &render_store.read(),
                                    &mut dynamic_light_queue,
                                    vec![],
                                );

                                resolved_trigger_visuals[i] =
                                    render_queue[queue_start..].iter().any(|queued| {
                                        queued.entity_alt.is_some()
                                            || render_store
                                                .read()
                                                .get_entity(queued.entity.0, queued.entity.1)
                                                .is_some()
                                    });
                            }
                            _ => {}
                        }
                    }

                    // Render the native uncollected Pickup state. Complete pickup
                    // Scripts own multi-Entity assemblies and idle particles; direct
                    // entities remain for the few PC assets without a world Script.
                    if !resolved_trigger_visuals[i] {
                        if let Some(pickup) = robots_pickup_visual(t.ttype, &t.data) {
                            let rotation = Quat::from_euler(
                                glam::EulerRot::ZXY,
                                t.rotation[2],
                                t.rotation[0],
                                t.rotation[1],
                            );
                            match pickup.object.base() {
                                0x04000000 => {
                                    // Frame 1 is the uncollected idle state. Advancing
                                    // beyond opcode 0x10 would incorrectly play the
                                    // collection branch without a gameplay event.
                                    let script_time = render_store
                                        .read()
                                        .get_script(pickup.file, pickup.object)
                                        .map(|script| script.time_at_frame(1.0))
                                        .unwrap_or(0.0);
                                    let queue_start = render_queue.len();
                                    render_script(
                                        trigger_position,
                                        rotation,
                                        t.scale * pickup.scale,
                                        pickup.file,
                                        pickup.object,
                                        script_time,
                                        &render_store.read(),
                                        &mut |q| render_queue.push(q),
                                        vec![],
                                    );
                                    collect_script_particles(
                                        trigger_position,
                                        rotation,
                                        t.scale * pickup.scale,
                                        pickup.file,
                                        pickup.object,
                                        script_time,
                                        &render_store.read(),
                                        &mut particle_queue,
                                        vec![],
                                    );
                                    collect_script_dynamic_lights(
                                        trigger_position,
                                        rotation,
                                        t.scale * pickup.scale,
                                        pickup.file,
                                        pickup.object,
                                        script_time,
                                        &render_store.read(),
                                        &mut dynamic_light_queue,
                                        vec![],
                                    );
                                    // Do not hide the trigger placeholder for a broken
                                    // Script closure that produced particles but no model.
                                    resolved_trigger_visuals[i] = render_queue.len() > queue_start;
                                }
                                0x02000000 => {
                                    if render_store
                                        .read()
                                        .get_entity(pickup.file, pickup.object)
                                        .is_some()
                                    {
                                        render_queue.push(QueuedEntityRender {
                                            entity: (pickup.file, pickup.object),
                                            entity_alt: None,
                                            position: trigger_position,
                                            rotation,
                                            scale: t.scale * pickup.scale,
                                        });
                                        resolved_trigger_visuals[i] = true;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }

                    // Monster/NPC/Fish triggers do not serialize visual_object.
                    // Robots.exe resolves data[0] through d00_mons.edb and creates
                    // an XItem from the selected external character EDB. Queue the
                    // first shipped local Animation Script as a static complete
                    // model preview; gameplay AI and animation-state selection stay
                    // outside this geometry reconstruction.
                    if !resolved_trigger_visuals[i] {
                        if let Some(character) = &t.character_visual {
                            let rotation = Quat::from_euler(
                                glam::EulerRot::ZXY,
                                t.rotation[2],
                                t.rotation[0],
                                t.rotation[1],
                            );
                            let queue_start = render_queue.len();
                            render_static_script(
                                trigger_position,
                                rotation,
                                t.scale,
                                character.file,
                                character.script,
                                0.0,
                                &render_store.read(),
                                &mut |queued| render_queue.push(queued),
                                vec![],
                            );
                            resolved_trigger_visuals[i] = render_queue.len() > queue_start;
                        }
                    }

                    if t.ttype == 34 {
                        let angle = if animate_scripts {
                            advance_native_fan_angle(
                                0.0,
                                script_global_time,
                                fan_runtime_value,
                                script_playback_speed,
                            )
                        } else {
                            0.0
                        };
                        apply_native_fan_rotation(
                            &mut render_queue[trigger_queue_start..],
                            &render_store.read(),
                            angle,
                        );
                    }
                }
            }

            v.uniforms.native_dynamic_lights = dynamic_light_queue
                .into_iter()
                .map(|light| NativeDynamicLight {
                    position: light.position,
                    direction: light.direction,
                    colour: light.colour,
                    radius: light.radius,
                    zone_indices: map.native_dynamic_light_zone_indices(
                        light.position,
                        light.radius,
                        light.containing_zone_only,
                    ),
                })
                .collect();
            let sky_render_context = RenderContext {
                shaders: &v.shaders,
                uniforms: &sky_uniforms,
                lighting_key: 0,
            };
            let render_context = RenderContext {
                shaders: &v.shaders,
                uniforms: &v.uniforms,
                lighting_key: 0,
            };

            if render_filter.contains(RenderFilter::Opaque) {
                for r in &sky_world_queue {
                    if let Some(e) = render_store.read().get_entity(r.entity.0, r.entity.1) {
                        e.draw_opaque(
                            painter.gl(),
                            &sky_render_context,
                            r.position,
                            r.rotation,
                            r.scale,
                            time,
                            &render_store.read(),
                        );
                    }
                }
                for (render_index, r) in render_queue.iter().enumerate() {
                    let keyed_context = RenderContext {
                        shaders: render_context.shaders,
                        uniforms: render_context.uniforms,
                        lighting_key: ((map.hashcode as u64) << 32) | (render_index as u64 + 1),
                    };
                    if let Some(e) = r.entity_alt.as_ref().map(|v| v.lock()) {
                        e.draw_opaque(
                            painter.gl(),
                            &keyed_context,
                            r.position,
                            r.rotation,
                            r.scale,
                            time,
                            &render_store.read(),
                        );
                        continue;
                    }
                    if let Some(e) = render_store.read().get_entity(r.entity.0, r.entity.1) {
                        e.draw_opaque(
                            painter.gl(),
                            &keyed_context,
                            r.position,
                            r.rotation,
                            r.scale,
                            time,
                            &render_store.read(),
                        )
                    }
                }
            }

            painter.gl().depth_mask(false);

            if render_filter.contains(RenderFilter::Transparent) {
                for r in &sky_world_queue {
                    if let Some(e) = render_store.read().get_entity(r.entity.0, r.entity.1) {
                        e.draw_transparent(
                            painter.gl(),
                            &sky_render_context,
                            r.position,
                            r.rotation,
                            r.scale,
                            time,
                            &render_store.read(),
                        );
                    }
                }
                for (render_index, r) in render_queue.iter().enumerate() {
                    let keyed_context = RenderContext {
                        shaders: render_context.shaders,
                        uniforms: render_context.uniforms,
                        lighting_key: ((map.hashcode as u64) << 32) | (render_index as u64 + 1),
                    };
                    if let Some(e) = r.entity_alt.as_ref().map(|v| v.lock()) {
                        e.draw_transparent(
                            painter.gl(),
                            &keyed_context,
                            r.position,
                            r.rotation,
                            r.scale,
                            time,
                            &render_store.read(),
                        );
                        continue;
                    }

                    if let Some(e) = render_store.read().get_entity(r.entity.0, r.entity.1) {
                        e.draw_transparent(
                            painter.gl(),
                            &keyed_context,
                            r.position,
                            r.rotation,
                            r.scale,
                            time,
                            &render_store.read(),
                        )
                    }
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

            if show_portals {
                painter.gl().depth_mask(true);
                for portal in &map.portals {
                    let colour = if active_zone_index.is_some_and(|zone_index| {
                        robots_portal_neighbor_zone(portal, zone_index, map.zones.len()).is_some()
                    }) {
                        Vec3::new(1.0, 0.55, 0.15)
                    } else {
                        Vec3::new(0.2, 0.85, 1.0)
                    };
                    for edge in 0..4usize {
                        link_renderer.render(
                            painter.gl(),
                            &render_context,
                            portal.vertices[edge],
                            portal.vertices[(edge + 1) % 4],
                            colour,
                            trigger_scale * 0.6,
                        );
                    }
                }
            }

            if show_triggers {
                painter.gl().depth_mask(true);
                if let Some(Some(trig)) = selected_trigger.map(|v| map.triggers.get(v)) {
                    let event_snapshot = selected_trigger
                        .and_then(|index| runtime_event_snapshots.get(index))
                        .copied()
                        .flatten();
                    let trig_position = runtime_path_preview_position_with_event(
                        &map,
                        trig,
                        runtime_time,
                        animate_runtime_paths,
                        runtime_path_playback_speed,
                        event_snapshot,
                    );
                    if show_runtime_path {
                        for (_, _, path) in map_trigger_path_matches(&map, trig) {
                            let proven_path = robots_trigger_path_is_proven(
                                trig.ttype,
                                &trig.data,
                                path.hashcode,
                            );
                            let path_colour = if !proven_path {
                                Vec3::new(0.75, 0.45, 0.95)
                            } else if robots_trigger_runtime_path_speed(trig.ttype, &trig.data)
                                .is_some()
                            {
                                Vec3::new(0.25, 0.95, 0.55)
                            } else {
                                Vec3::new(0.25, 0.75, 1.0)
                            };
                            for (start, end) in runtime_path_segments(path) {
                                link_renderer.render(
                                    painter.gl(),
                                    &render_context,
                                    start,
                                    end,
                                    path_colour,
                                    trigger_scale * 0.75,
                                );
                            }
                        }
                    }

                    for l in &trig.links {
                        if *l == -1 {
                            continue;
                        }
                        let Some((_, target)) = map_trigger_by_link(&map, *l) else {
                            warn!("Trigger link target does not exist ({l})");
                            continue;
                        };

                        link_renderer.render(
                            painter.gl(),
                            &render_context,
                            trig_position,
                            target.position,
                            if hovered_link.map(|v| v == *l).unwrap_or_default() {
                                Vec3::ONE
                            } else {
                                Vec3::new(0.913, 0.547, 0.125)
                            },
                            trigger_scale,
                        );
                    }

                    for l in &trig.incoming_links {
                        let Some((_, source)) = map_trigger_by_link(&map, *l) else {
                            warn!("Incoming trigger link source does not exist ({l})");
                            continue;
                        };

                        link_renderer.render(
                            painter.gl(),
                            &render_context,
                            source.position,
                            trig_position,
                            if hovered_link.map(|v| v == *l).unwrap_or_default() {
                                Vec3::ONE
                            } else {
                                Vec3::new(0.169, 0.554, 0.953)
                            },
                            trigger_scale,
                        );
                    }

                    select_renderer.render(
                        painter.gl(),
                        &render_context,
                        trig_position,
                        runtime_trigger_preview_rotation_with_event(
                            &map,
                            trig,
                            runtime_time,
                            animate_runtime_paths,
                            runtime_path_playback_speed,
                            platform_rotation_speed_scale,
                            event_snapshot,
                        ),
                        trigger_scale,
                    );
                }

                for (i, t) in map.triggers.iter().enumerate() {
                    // ROBOTS_PATCH_0022_HIDE_RESOLVED_VISUAL_PLACEHOLDER
                    // PATCH_0024: suppress only when this trigger actually queued renderable
                    // Entity/AnimSkin geometry. Merely resolving a Script object is not enough.
                    if resolved_trigger_visuals.get(i).copied().unwrap_or(false) {
                        continue;
                    }

                    let trigger_texture_path = trigger_info
                        .triggers
                        .get(&t.ttype)
                        .and_then(|m| m.icon.as_ref().map(|v| v.to_lowercase()));

                    let trigger_texture = *trigger_texture_path
                        .and_then(|p| trigger_icons.get(&p))
                        .unwrap_or(&default_trigger_icon);

                    billboard_renderer.render(
                        painter.gl(),
                        &render_context,
                        trigger_texture,
                        runtime_path_preview_position_with_event(
                            &map,
                            t,
                            runtime_time,
                            animate_runtime_paths,
                            runtime_path_playback_speed,
                            runtime_event_snapshots.get(i).copied().flatten(),
                        ),
                        trigger_scale,
                    );
                }

                // Trigger collisions
                set_blending_mode(painter.gl(), BlendMode::Blend);
                // for t in map.triggers.iter() {
                if let Some(t) = selected_trigger.and_then(|t| map.triggers.get(t)) {
                    if let Some(coll) = t
                        .engine_options
                        .collision_index
                        .and_then(|c| map.trigger_collisions.get(c as usize))
                    {
                        if coll.dtype == 0 || coll.dtype == 3 {
                            collision_renderer.render(
                                painter.gl(),
                                &render_context,
                                runtime_path_preview_position_with_event(
                                    &map,
                                    t,
                                    runtime_time,
                                    animate_runtime_paths,
                                    runtime_path_playback_speed,
                                    selected_trigger
                                        .and_then(|index| runtime_event_snapshots.get(index))
                                        .copied()
                                        .flatten(),
                                ) + Vec3::from(coll.position),
                                runtime_trigger_preview_rotation_with_event(
                                    &map,
                                    t,
                                    runtime_time,
                                    animate_runtime_paths,
                                    runtime_path_playback_speed,
                                    platform_rotation_speed_scale,
                                    selected_trigger
                                        .and_then(|index| runtime_event_snapshots.get(index))
                                        .copied()
                                        .flatten(),
                                ),
                                coll,
                            );
                        }
                    }
                }
            }

            if show_sounds {
                let sound_icon = trigger_icons
                    .get("sound")
                    .copied()
                    .unwrap_or(default_trigger_icon);
                for sound in &map.sounds {
                    billboard_renderer.render(
                        painter.gl(),
                        &render_context,
                        sound_icon,
                        sound.position,
                        sound_scale,
                    );
                }
                if let Some(sound) = selected_sound.and_then(|index| map.sounds.get(index)) {
                    select_renderer.render(
                        painter.gl(),
                        &render_context,
                        sound.position,
                        Quat::IDENTITY,
                        sound.outer_radius.max(sound_scale).clamp(sound_scale, 25.0),
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

    fn render_pickbuffer(
        &mut self,
        res: Vec2,
        map: &ProcessedMap,
        time: f32,
        runtime_event_snapshots: &[Option<RuntimeEventPreviewSnapshot>],
    ) {
        self.pickbuffer
            .init_draw(&self.gl, glam::ivec2(res.x as i32, res.y as i32));
        let mut viewer = self.viewer.lock();
        viewer.start_render(&self.gl, res.x / res.y.max(1.0), time);
        let render_context = viewer.render_context();
        if self.show_triggers {
            for (i, t) in map.triggers.iter().enumerate() {
                self.billboard_renderer.render_pickbuffer(
                    &self.gl,
                    &render_context,
                    runtime_path_preview_position_with_event(
                        map,
                        t,
                        time,
                        self.animate_runtime_paths,
                        self.runtime_path_playback_speed,
                        runtime_event_snapshots.get(i).copied().flatten(),
                    ),
                    self.trigger_scale,
                    (PickBufferType::Trigger, i as u32),
                    &self.pickbuffer,
                );
            }
        }
        if self.show_sounds {
            for (i, sound) in map.sounds.iter().enumerate() {
                self.billboard_renderer.render_pickbuffer(
                    &self.gl,
                    &render_context,
                    sound.position,
                    self.sound_scale,
                    (PickBufferType::Sound, i as u32),
                    &self.pickbuffer,
                );
            }
        }
    }
}
