use super::*;

impl MapFrame {
    pub(super) fn draw_trigger_inspector(&mut self, ctx: &egui::Context, map: &ProcessedMap) {
        let screen_space = ctx.content_rect();
        egui::Window::new("Inspector")
            .scroll([false, true])
            .show(ctx, |ui| {
                if self.selected_trigger.is_none() || !self.show_triggers {
                    ui.heading("No object selected");
                    return;
                }

                macro_rules! readonly_input {
                    ($ui:expr, $string:expr) => {
                        let mut tmp = $string;
                        $ui.add_enabled(false, egui::TextEdit::singleline(&mut tmp));
                    };
                    ($ui:expr, $label:expr, $string:expr) => {
                        // $ui.horizontal(|ui| {
                        $ui.label($label);
                        let mut tmp = $string;
                        $ui.add_enabled(
                            false,
                            egui::TextEdit::singleline(&mut tmp), // .desired_width(f32::INFINITY),
                        );
                        // })
                    };
                }

                macro_rules! ttype_or_hex {
                    ($v:expr) => {
                        if let Some(ti) = self.trigger_info.triggers.get(&$v) {
                            format!("{} (0x{:x})", ti.name, $v)
                        } else {
                            format!("0x{:x}", $v)
                        }
                    };
                }

                macro_rules! quick_grid {
                    ($ui:expr, $label:expr, $contents:expr) => {
                        egui::Grid::new($label)
                            .num_columns(2)
                            .spacing([40.0, 4.0])
                            .striped(true)
                            .show($ui, $contents);
                    };
                }

                egui::ScrollArea::vertical()
                    .max_height(screen_space.height() - 100.0)
                    .show(ui, |ui| {
                        if let Some(Some(trig)) = self.selected_trigger.map(|v| map.triggers.get(v))
                        {
                            quick_grid!(ui, "t_info", |ui| {
                                readonly_input!(ui, "Type ", ttype_or_hex!(trig.ttype));
                                ui.end_row();
                                readonly_input!(
                                    ui,
                                    "Subtype ",
                                    if let Some(subtype) = trig.tsubtype {
                                        ttype_or_hex!(subtype)
                                    } else {
                                        "None".to_string()
                                    }
                                );
                                ui.end_row();
                                readonly_input!(ui, "Type Index ", format!("{} / 0x{:x}", trig.type_index, trig.type_index));
                                ui.end_row();
                                readonly_input!(ui, "File Offset ", format!("0x{:08x}", trig.file_offset));
                                ui.end_row();
                                readonly_input!(ui, "Link Ref ", trig.link_ref.to_string());
                                ui.end_row();
                                readonly_input!(ui, "Debug ", format!("{} / 0x{:x}", trig.debug, trig.debug));
                                ui.end_row();
                                readonly_input!(ui, "Game Flags ", format!("0x{:08x}", trig.game_flags));
                                ui.end_row();
                                readonly_input!(ui, "Trigger Flags ", format!("0x{:08x}", trig.trig_flags));
                                ui.end_row();

                                ui.label("Position");
                                ui.horizontal(|ui| {
                                    ui.label(format!(
                                        "{:.3}, {:.3},  {:.3}",
                                        trig.position.x, trig.position.y, trig.position.z
                                    ));
                                });
                                ui.end_row();

                                ui.label("Rotation");
                                ui.horizontal(|ui| {
                                    ui.label(format!(
                                        "{:.3}, {:.3},  {:.3}",
                                        trig.rotation.x.to_degrees(),
                                        trig.rotation.y.to_degrees(),
                                        trig.rotation.z.to_degrees()
                                    ));
                                });
                                ui.end_row();

                                ui.label("Scale");
                                ui.horizontal(|ui| {
                                    ui.label(format!(
                                        "{:.2}, {:.2},  {:.2}",
                                        trig.scale.x, trig.scale.y, trig.scale.z
                                    ));
                                });
                                ui.end_row();

                                if let Some(coll) = trig
                                    .engine_options
                                    .collision_index
                                    .and_then(|c| map.trigger_collisions.get(c as usize))
                                {
                                    ui.label("Collision");
                                    match coll.dtype {
                                        0 => ui.label("Box"),
                                        3 => ui.label("Cylinder"),
                                        u => ui.label(format!(
                                            "{} Unknown collision type {}",
                                            font_awesome::EXCLAMATION_TRIANGLE,
                                            u
                                        )),
                                    };
                                    ui.end_row();
                                }
                            });

                            if matches!(trig.ttype, 1 | 20) {
                                ui.separator();
                                ui.strong("Native Camera Diagnostics");
                                quick_grid!(ui, "t_native_camera", |ui| {
                                    readonly_input!(
                                        ui,
                                        "Runtime Class",
                                        if trig.ttype == 1 {
                                            "XTrigger_Camera"
                                        } else {
                                            "XTrigger_Camera_Marker"
                                        }
                                        .to_string()
                                    );
                                    ui.end_row();
                                    if let Some(mode) = robots_camera_mode(trig.ttype, &trig.data) {
                                        readonly_input!(
                                            ui,
                                            "data[0] mode selector",
                                            format!("{} / 0x{:08x}", mode, mode)
                                        );
                                        ui.end_row();
                                    }
                                    if let Some(value) =
                                        robots_camera_marker_scaled_data0(trig.ttype, &trig.data)
                                    {
                                        readonly_input!(
                                            ui,
                                            "data[0] runtime scale",
                                            format!("{value:.4} (signed value × 0.1)")
                                        );
                                        ui.end_row();
                                    }
                                    if let Some(value) =
                                        robots_camera_scaled_data4(trig.ttype, &trig.data)
                                    {
                                        readonly_input!(
                                            ui,
                                            "data[4] runtime scale",
                                            format!("{value:.4} (signed value × 0.1)")
                                        );
                                        ui.end_row();
                                    }
                                    if let Some(value) =
                                        robots_camera_scaled_data5(trig.ttype, &trig.data)
                                    {
                                        readonly_input!(
                                            ui,
                                            "data[5] runtime scale",
                                            format!("{value:.4} (signed value × 0.1)")
                                        );
                                        ui.end_row();
                                    }
                                    if let Some(flags) = robots_camera_flags(trig.ttype, &trig.data) {
                                        readonly_input!(
                                            ui,
                                            "data[2] flags",
                                            format!("0x{flags:08x}")
                                        );
                                        ui.end_row();
                                        readonly_input!(
                                            ui,
                                            "Proven flag tests",
                                            if trig.ttype == 1 {
                                                "0x0008 and 0x8000"
                                            } else {
                                                "0x0002 and 0x8000"
                                            }
                                            .to_string()
                                        );
                                        ui.end_row();
                                    }
                                    readonly_input!(
                                        ui,
                                        "Runtime Boundary",
                                        if trig.ttype == 1 {
                                            "Mode 4 compares data[1] with the active runtime path UID; this is not proof that the camera travels along the path"
                                        } else {
                                            "data[4] reaches native camera setup; exact camera interpolation along that EXGeoPath is not proven"
                                        }
                                        .to_string()
                                    );
                                    ui.end_row();
                                });
                            }

                            if trig.ttype == 48 {
                                ui.separator();
                                ui.strong("Native NPC Mission/Cutscene Diagnostics");
                                quick_grid!(ui, "t_native_npc", |ui| {
                                    readonly_input!(ui, "Trigger Class", "XTrigger_NPC".to_string());
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Handler Classes",
                                        "XItemHandler_Npc / XItemHandler_Npc_Fender".to_string()
                                    );
                                    ui.end_row();
                                    if let Some(selector) =
                                        robots_npc_runtime_selector(trig.ttype, &trig.data)
                                    {
                                        readonly_input!(
                                            ui,
                                            "data[0] native value",
                                            format!("{} / 0x{selector:08x}", selector)
                                        );
                                        ui.end_row();
                                    }
                                    if let Some(runtime_uid) =
                                        robots_npc_runtime_uid(trig.ttype, &trig.data)
                                    {
                                        readonly_input!(
                                            ui,
                                            "data[1] native UID getter",
                                            if runtime_uid == 0x0B00_0000 {
                                                "0x0b000000 sentinel; no path promotion".to_string()
                                            } else {
                                                DefinitionDataType::Hashcode
                                                    .to_string(&self.hashcodes, runtime_uid)
                                            }
                                        );
                                        ui.end_row();
                                    }
                                    if let Some(flags) = robots_npc_flags(trig.ttype, &trig.data) {
                                        readonly_input!(
                                            ui,
                                            "data[2] NPC flags",
                                            format!(
                                                "0x{flags:08x}; native 0x8000 test={}",
                                                flags & 0x8000 != 0
                                            )
                                        );
                                        ui.end_row();
                                    }
                                    if let Some(text_group) =
                                        robots_npc_text_group(trig.ttype, &trig.data)
                                    {
                                        readonly_input!(
                                            ui,
                                            "data[3] text group",
                                            DefinitionDataType::Hashcode
                                                .to_string(&self.hashcodes, text_group)
                                        );
                                        ui.end_row();
                                    }
                                    if let Some(cutscenes) =
                                        robots_npc_alternate_cutscenes(trig.ttype, &trig.data)
                                    {
                                        for (index, cutscene) in cutscenes.into_iter().enumerate() {
                                            readonly_input!(
                                                ui,
                                                format!("data[{}] alternate cutscene", index + 4),
                                                cutscene
                                                    .map(|hash| {
                                                        if robots_npc_cutscene_is_null(hash) {
                                                            format!("0x{hash:08x} null/sentinel")
                                                        } else {
                                                            DefinitionDataType::Hashcode
                                                                .to_string(&self.hashcodes, hash)
                                                        }
                                                    })
                                                    .unwrap_or_else(|| "missing".to_string())
                                            );
                                            ui.end_row();
                                        }
                                    }
                                    readonly_input!(
                                        ui,
                                        "Native Proof",
                                        "vtable getters: data[0] +0xF4, data[1] +0xFC, data[2] +0x10C, data[3] +0x110; XTrigger_NPC::ActivateCutscene selects data[4..7]"
                                            .to_string()
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Mission/Tutorial State",
                                        "native setup resolves XTrigger_Mission, falls back to XTrigger_Tutorial, and persists a 0x40-byte NPC state block"
                                            .to_string()
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Runtime Boundary",
                                        "alternate-cutscene and mission context are diagnostic only; dialogue selection, AI movement, player focus and cutscene execution are not simulated"
                                            .to_string()
                                    );
                                    ui.end_row();
                                });
                            }

                            if trig.ttype == 60 {
                                ui.separator();
                                ui.strong("Native Watchbot Diagnostics");
                                quick_grid!(ui, "t_native_watchbot", |ui| {
                                    readonly_input!(
                                        ui,
                                        "Runtime Class",
                                        "XTrigger_Watchbot".to_string()
                                    );
                                    ui.end_row();
                                    if let Some(mode) = robots_watchbot_mode(trig.ttype, &trig.data) {
                                        readonly_input!(
                                            ui,
                                            "data[0] mode selector",
                                            format!("{} / 0x{:08x}", mode, mode)
                                        );
                                        ui.end_row();
                                    }
                                    if let Some(flags) = robots_watchbot_flags(trig.ttype, &trig.data) {
                                        readonly_input!(
                                            ui,
                                            "data[2] flags",
                                            format!("0x{flags:08x}")
                                        );
                                        ui.end_row();
                                        readonly_input!(
                                            ui,
                                            "Proven flag tests",
                                            "0x0001 and 0x0002".to_string()
                                        );
                                        ui.end_row();
                                    }
                                    if let Some(distance) =
                                        robots_watchbot_enter_distance(trig.ttype, &trig.data)
                                    {
                                        readonly_input!(
                                            ui,
                                            "Enter Distance",
                                            format!("{distance:.4} (signed data[3] × 0.1)")
                                        );
                                        ui.end_row();
                                    }
                                    if let Some(distance) =
                                        robots_watchbot_leave_distance(trig.ttype, &trig.data)
                                    {
                                        readonly_input!(
                                            ui,
                                            "Leave Distance",
                                            format!("{distance:.4} (signed data[4] × 0.1)")
                                        );
                                        ui.end_row();
                                    }
                                    readonly_input!(
                                        ui,
                                        "Runtime Boundary",
                                        "Mode 3 compares/assigns data[1] as the active Watchbot path UID; player state, controller traversal and path timing are not simulated"
                                            .to_string()
                                    );
                                    ui.end_row();
                                });
                            }

                            if trig.ttype == 72 {
                                ui.separator();
                                ui.strong("Native BossRatchet Diagnostics");
                                quick_grid!(ui, "t_native_boss_ratchet", |ui| {
                                    readonly_input!(ui, "Runtime Class", "XTrigger_BossRatchet".to_string());
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Primary Path",
                                        trig.data
                                            .first()
                                            .copied()
                                            .flatten()
                                            .map(|hash| format!("0x{hash:08x} from data[0]"))
                                            .unwrap_or_else(|| "missing".to_string())
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Runtime Boundary",
                                        "data[0] is passed into the created Ratchet boss runtime; boss AI traversal and timing are not simulated".to_string()
                                    );
                                    ui.end_row();
                                });
                            }

                            if trig.ttype == 73 {
                                ui.separator();
                                ui.strong("Native Monster Transporter Diagnostics");
                                quick_grid!(ui, "t_native_monster_transporter", |ui| {
                                    readonly_input!(ui, "Runtime Class", "XTrigger_Monster_Transporter".to_string());
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Primary Path",
                                        trig.data
                                            .get(1)
                                            .copied()
                                            .flatten()
                                            .map(|hash| format!("0x{hash:08x} from data[1]"))
                                            .unwrap_or_else(|| "missing".to_string())
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Secondary Path",
                                        robots_monster_transporter_secondary_path_hash(trig.ttype, &trig.data)
                                            .map(|hash| format!("0x{hash:08x} from data[4]"))
                                            .unwrap_or_else(|| {
                                                trig.data
                                                    .get(4)
                                                    .copied()
                                                    .flatten()
                                                    .map(|hash| format!("0x{hash:08x} sentinel/non-path value"))
                                                    .unwrap_or_else(|| "missing".to_string())
                                            })
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Runtime Boundary",
                                        "data[1] is parsed into the Transporter route and data[4] reaches monster-controller setup; actor traversal and spawn timing are not simulated".to_string()
                                    );
                                    ui.end_row();
                                });
                            }

                            if robots_monster_is_family(trig.ttype) {
                                ui.separator();
                                ui.strong("Native Monster Family Diagnostics");
                                quick_grid!(ui, "t_native_monster", |ui| {
                                    readonly_input!(
                                        ui,
                                        "Runtime Class",
                                        match trig.ttype {
                                            3 => "XTrigger_Monster_Test",
                                            70 => "XTrigger_Monster_Fish",
                                            _ => "XTrigger_Monster",
                                        }
                                        .to_string()
                                    );
                                    ui.end_row();
                                    if let Some(selector) =
                                        robots_monster_runtime_selector(trig.ttype, &trig.data)
                                    {
                                        readonly_input!(
                                            ui,
                                            "data[0] config record index",
                                            format!("{} / 0x{selector:08x}", selector)
                                        );
                                        ui.end_row();
                                    }
                                    if let Some(radius) =
                                        robots_monster_proximity_radius(trig.ttype, &trig.data)
                                    {
                                        readonly_input!(
                                            ui,
                                            "data[1] proximity radius",
                                            format!("{radius:.4} (signed dword × 0.1)")
                                        );
                                        ui.end_row();
                                    } else if let Some(value) =
                                        robots_monster_test_runtime_value(trig.ttype, &trig.data)
                                    {
                                        readonly_input!(
                                            ui,
                                            "data[1] test runtime value",
                                            format!("{} / 0x{value:08x}", value)
                                        );
                                        ui.end_row();
                                    }
                                    if let Some(path_hash) =
                                        robots_trigger_path_hash(trig.ttype, &trig.data)
                                    {
                                        readonly_input!(
                                            ui,
                                            "data[2] path getter",
                                            if path_hash == 0x0B00_0000 {
                                                "0x0b000000 sentinel; no runtime path".to_string()
                                            } else {
                                                DefinitionDataType::Hashcode
                                                    .to_string(&self.hashcodes, path_hash)
                                            }
                                        );
                                        ui.end_row();
                                    }
                                    if let Some(value) =
                                        robots_monster_data4_value(trig.ttype, &trig.data)
                                    {
                                        readonly_input!(
                                            ui,
                                            "data[4] native value",
                                            format!("{} / 0x{value:08x}", value)
                                        );
                                        ui.end_row();
                                    }
                                    if let Some(flags) = robots_monster_flags(trig.ttype, &trig.data) {
                                        readonly_input!(
                                            ui,
                                            "data[7] flags",
                                            format!(
                                                "0x{flags:08x}; 0x8000={} 0x4000={}",
                                                flags & 0x8000 != 0,
                                                flags & 0x4000 != 0
                                            )
                                        );
                                        ui.end_row();
                                    }
                                    if let Some(value) =
                                        robots_monster_data15_value(trig.ttype, &trig.data)
                                    {
                                        readonly_input!(
                                            ui,
                                            "data[15] native value",
                                            format!("{} / 0x{value:08x}", value)
                                        );
                                        ui.end_row();
                                    }
                                    readonly_input!(
                                        ui,
                                        "Native Proof",
                                        match trig.ttype {
                                            3 => "Monster_Test vtable exposes data[0], raw data[1], data[4], data[7] and data[15]",
                                            70 => "Monster_Fish vtable exposes data[0] and signed data[1] × 0.1; its path getter returns the 0x0b000000 sentinel",
                                            _ => "Base Monster vtable exposes data[0], signed data[1] × 0.1, data[2], data[4], data[7] and data[15]; setup uses data[0] to index a 24-byte Monster configuration record and uses the radius in a distance test",
                                        }
                                        .to_string()
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Runtime Boundary",
                                        "native trigger getters and proximity/path context are diagnostic; navigation, target selection, combat, damage and AI timing are not simulated"
                                            .to_string()
                                    );
                                    ui.end_row();
                                });
                            }

                            if trig.ttype == 75 {
                                ui.separator();
                                ui.strong("BossSewer Path-like Value Rejected");
                                quick_grid!(ui, "t_boss_sewer_selector", |ui| {
                                    readonly_input!(
                                        ui,
                                        "Runtime Class",
                                        "XTrigger_BossSewer".to_string()
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "data[0] selector",
                                        trig.data
                                            .first()
                                            .copied()
                                            .flatten()
                                            .map(|value| format!("{} / 0x{value:08x}", value))
                                            .unwrap_or_else(|| "missing".to_string())
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Static Rejection",
                                        "0x00484C30 and 0x00484CF0 compare data[0] with integer 1; no path lookup or dereference occurs"
                                            .to_string()
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Status",
                                        "A matching EXGeoPath hash is coincidental and remains purple diagnostic data"
                                            .to_string()
                                    );
                                    ui.end_row();
                                });
                            }

                            if matches!(trig.ttype, 8 | 37 | 80)
                                && robots_trigger_runtime_path_speed(trig.ttype, &trig.data).is_some()
                            {
                                let trigger_index = self.selected_trigger.unwrap_or_default();
                                let wall_time = ctx.input(|input| input.time);
                                let snapshot = self.runtime_event_snapshot(
                                    map,
                                    trigger_index,
                                    wall_time,
                                );
                                let runtime_time = self
                                    .runtime_motion_start_time
                                    .map(|start| (wall_time - start).max(0.0) as f32)
                                    .unwrap_or_default();
                                let contact_linear_velocity = snapshot
                                    .and_then(|state| state.platform_contact_linear_velocity)
                                    .or_else(|| {
                                        runtime_platform_contact_linear_velocity(
                                            map,
                                            trig,
                                            runtime_time,
                                            self.animate_runtime_paths,
                                            self.runtime_path_playback_speed,
                                        )
                                    });
                                ui.separator();
                                ui.strong("Native Runtime Event Gate");
                                quick_grid!(ui, "t_native_runtime_event", |ui| {
                                    readonly_input!(
                                        ui,
                                        "Constructor State",
                                        "inactive (trigger+0xE4 = 0)".to_string()
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Preview Mode",
                                        if self.native_runtime_event_gate {
                                            "native event-gated"
                                        } else {
                                            "continuous diagnostic"
                                        }
                                        .to_string()
                                    );
                                    ui.end_row();
                                    if let Some(snapshot) = snapshot {
                                        readonly_input!(
                                            ui,
                                            "State",
                                            if snapshot.active { "active" } else { "inactive" }
                                                .to_string()
                                        );
                                        ui.end_row();
                                        readonly_input!(
                                            ui,
                                            "Active Time",
                                            format!("{:.3} s", snapshot.elapsed_seconds)
                                        );
                                        ui.end_row();
                                        readonly_input!(
                                            ui,
                                            "Direction",
                                            if snapshot.direction_reversed {
                                                "reversed"
                                            } else {
                                                "forward"
                                            }
                                            .to_string()
                                        );
                                        ui.end_row();
                                        readonly_input!(
                                            ui,
                                            "Path Distance",
                                            format!("{:.4}", snapshot.path_distance)
                                        );
                                        ui.end_row();
                                        if let Some(steering_angle) =
                                            snapshot.vehicle_steering_angle
                                        {
                                            readonly_input!(
                                                ui,
                                                "Wheel Steering",
                                                format!(
                                                    "{steering_angle:.5} rad / {:.2} deg (drive + passive local Y)",
                                                    steering_angle.to_degrees()
                                                )
                                            );
                                            ui.end_row();
                                        }
                                        readonly_input!(
                                            ui,
                                            "Last Event",
                                            snapshot
                                                .last_event
                                                .map(|event| format!("0x{event:08X}"))
                                                .unwrap_or_else(|| "none".to_string())
                                        );
                                        ui.end_row();
                                    }
                                    if let Some(velocity) = contact_linear_velocity {
                                        readonly_input!(
                                            ui,
                                            "Contact Carry",
                                            format!(
                                                "({:.4}, {:.4}, {:.4}) units/s",
                                                velocity.x, velocity.y, velocity.z
                                            )
                                        );
                                        ui.end_row();
                                        readonly_input!(
                                            ui,
                                            "Physics Role",
                                            "moving-platform linear carry; angular term requires a real contact point"
                                                .to_string()
                                        );
                                        ui.end_row();
                                    }
                                    if let Some(snapshot) = snapshot {
                                        readonly_input!(
                                            ui,
                                            "Last Path Node",
                                            snapshot
                                                .last_node_index
                                                .map(|index| index.to_string())
                                                .unwrap_or_else(|| "none".to_string())
                                        );
                                        ui.end_row();
                                        readonly_input!(
                                            ui,
                                            "Last Node Opcode",
                                            snapshot
                                                .last_node_opcode
                                                .map(|opcode| opcode.to_string())
                                                .unwrap_or_else(|| "none".to_string())
                                        );
                                        ui.end_row();
                                    }
                                    ui.label("Manual Events");
                                    ui.horizontal(|ui| {
                                        if ui.button("0x100 Start").clicked() {
                                            self.dispatch_runtime_event(
                                                map,
                                                trigger_index,
                                                ROBOTS_EVENT_ACTIVATE,
                                                wall_time,
                                            );
                                        }
                                        if ui.button("0x200 Stop").clicked() {
                                            self.dispatch_runtime_event(
                                                map,
                                                trigger_index,
                                                ROBOTS_EVENT_DEACTIVATE,
                                                wall_time,
                                            );
                                        }
                                        if ui.button("Reset ctor").clicked() {
                                            self.reset_runtime_event(map, trigger_index, wall_time);
                                        }
                                    });
                                    ui.end_row();
                                    if trig.ttype == 8 {
                                        let reverse = trig
                                            .data
                                            .get(7)
                                            .copied()
                                            .flatten()
                                            .is_some_and(|flags| flags & 0x200 != 0);
                                        readonly_input!(
                                            ui,
                                            "Active Retrigger",
                                            if reverse {
                                                "0x100 reverses controller direction (data[7] & 0x200)"
                                            } else {
                                                "0x100 keeps current direction"
                                            }
                                            .to_string()
                                        );
                                        ui.end_row();
                                    }
                                    readonly_input!(
                                        ui,
                                        "Boundary",
                                        "Manual events and instruction-proven node opcodes 4/8 are executed on path arrival; opcode 9, class-specific node values, waits and physics response remain diagnostic"
                                            .to_string()
                                    );
                                    ui.end_row();
                                });
                            }

                            if let Some(path_hash) = robots_trigger_path_hash(trig.ttype, &trig.data) {
                                ui.separator();
                                ui.strong(match trig.ttype {
                                    1 | 20 => "Native Camera Path Context",
                                    60 => "Native Watchbot Path Context",
                                    72 => "Native BossRatchet Path Context",
                                    73 => "Native Transporter Path Context",
                                    74 => "Native Monster Path Context",
                                    _ => "Runtime Path",
                                });
                                quick_grid!(ui, "t_runtime_path", |ui| {
                                    readonly_input!(ui, "Path Hash", format!("0x{path_hash:08x}"));
                                    ui.end_row();
                                    if let Some(data_slot) = robots_trigger_path_data_slot(trig.ttype) {
                                        readonly_input!(
                                            ui,
                                            "Serialized Source",
                                            format!("data[{data_slot}]")
                                        );
                                        ui.end_row();
                                    }
                                    if let Some((_, path_index, path)) =
                                        map_trigger_runtime_path(map, trig)
                                    {
                                        readonly_input!(ui, "Path Index", path_index.to_string());
                                        ui.end_row();
                                        readonly_input!(
                                            ui,
                                            "Path Flags",
                                            format!("0x{:08x}", path.flags)
                                        );
                                        ui.end_row();
                                        readonly_input!(ui, "Path Type", path.path_type.to_string());
                                        ui.end_row();
                                        readonly_input!(ui, "Nodes", path.nodes.len().to_string());
                                        ui.end_row();
                                        readonly_input!(
                                            ui,
                                            "Segments",
                                            runtime_path_segments(path).len().to_string()
                                        );
                                        ui.end_row();
                                        let annotated_nodes = path
                                            .nodes
                                            .iter()
                                            .enumerate()
                                            .filter(|(_, node)| {
                                                node.value != [0; 4]
                                                    || node.flags != 0
                                                    || node.distance.abs() > f32::EPSILON
                                            })
                                            .collect::<Vec<_>>();
                                        ui.label("Native Node Metadata");
                                        if annotated_nodes.is_empty() {
                                            ui.label("none");
                                        } else {
                                            ui.vertical(|ui| {
                                                for (index, node) in annotated_nodes.iter().take(24) {
                                                    let opcode = match node.value[0] {
                                                        4 => "event 0x200 dispatch",
                                                        8 => "linked-trigger mask dispatch",
                                                        9 => "alternate path UID 0x0B000000 + value[1]",
                                                        0 => "no value opcode",
                                                        _ => "not handled by common path-event dispatcher; class-specific meaning unresolved",
                                                    };
                                                    let mut text = format!(
                                                        "#{index}: value={:?} ({opcode}) flags=0x{:08X}",
                                                        node.value, node.flags
                                                    );
                                                    if node.flags & 0x8 != 0 {
                                                        text.push_str(" [switch-path]");
                                                    }
                                                    if node.distance.abs() > f32::EPSILON {
                                                        text.push_str(&format!(
                                                            " distance={:.4} [semantic unresolved]",
                                                            node.distance
                                                        ));
                                                    }
                                                    ui.monospace(text).on_hover_text(format!(
                                                        "size=({:.4}, {:.4}), serialized links={}",
                                                        node.size.x, node.size.y, node.num_links
                                                    ));
                                                }
                                                if annotated_nodes.len() > 24 {
                                                    ui.label(format!(
                                                        "… and {} more annotated nodes",
                                                        annotated_nodes.len() - 24
                                                    ));
                                                }
                                            });
                                        }
                                        ui.end_row();
                                        if let Some(speed) = robots_trigger_runtime_path_speed(
                                            trig.ttype,
                                            &trig.data,
                                        ) {
                                            readonly_input!(
                                                ui,
                                                "Map Speed",
                                                format!("{speed:.4} world units/s")
                                            );
                                            ui.end_row();
                                        }
                                        if let Some(acceleration) =
                                            robots_trigger_runtime_path_acceleration(
                                                trig.ttype,
                                                &trig.data,
                                            )
                                        {
                                            readonly_input!(
                                                ui,
                                                "Map Acceleration",
                                                format!("{acceleration:.4}")
                                            );
                                            ui.end_row();
                                        }
                                        readonly_input!(
                                            ui,
                                            "Motion Mode",
                                            match trig.ttype {
                                                80 => "Vehicle loop + tangent yaw",
                                                37 => "Lift path + endpoint reversal",
                                                8 => "Platform path + endpoint reversal",
                                                1 => "Camera mode/context reference; no fabricated path motion",
                                                20 => "Camera Marker setup context; no fabricated path motion",
                                                60 => "Watchbot controller path context; no fabricated path motion",
                                                72 => "BossRatchet runtime path context; no fabricated path motion",
                                                73 => "Monster Transporter route context; no fabricated path motion",
                                                74 => "Monster actor path context; no fabricated path motion",
                                                _ => "Unsupported path consumer",
                                            }
                                            .to_string()
                                        );
                                        ui.end_row();
                                        readonly_input!(
                                            ui,
                                            "Remaining Limit",
                                            match trig.ttype {
                                                1 | 20 => "Native camera activation, player state and interpolation are not simulated",
                                                60 => "Watchbot player state, controller traversal and path timing are not simulated",
                                                72 => "Ratchet boss AI traversal and timing are not simulated",
                                                73 => "Transporter actor traversal and spawn timing are not simulated",
                                                74 => "Monster actor traversal, path state and timing are not simulated",
                                                _ => "Activation/events and exact node-state waits are not simulated",
                                            }
                                            .to_string()
                                        );
                                        ui.end_row();
                                    } else {
                                        readonly_input!(
                                            ui,
                                            "Status",
                                            "Referenced EXGeoPath is missing from this map".to_string()
                                        );
                                        ui.end_row();
                                    }
                                });
                            }

                            let unsupported_path_matches = map_trigger_path_matches(map, trig)
                                .into_iter()
                                .filter(|(_, _, path)| {
                                    !robots_trigger_path_is_proven(
                                        trig.ttype,
                                        &trig.data,
                                        path.hashcode,
                                    )
                                })
                                .collect::<Vec<_>>();
                            if !unsupported_path_matches.is_empty() {
                                ui.separator();
                                ui.strong("Referenced Paths (native handler unsupported)");
                                quick_grid!(ui, "t_unsupported_paths", |ui| {
                                    for (data_slot, path_index, path) in &unsupported_path_matches {
                                        readonly_input!(
                                            ui,
                                            format!("data[{data_slot}]"),
                                            format!(
                                                "0x{:08x}, path #{}, {} nodes",
                                                path.hashcode,
                                                path_index,
                                                path.nodes.len()
                                            )
                                        );
                                        ui.end_row();
                                    }
                                    readonly_input!(
                                        ui,
                                        "Status",
                                        if trig.ttype == 75 {
                                            "Path-like value is preserved and drawn in purple, but static code proves data[0] is an integer selector rather than a path UID"
                                        } else {
                                            "Reference is preserved and drawn in purple; class-specific runtime behavior is not simulated"
                                        }
                                        .to_string()
                                    );
                                    ui.end_row();
                                });
                            }

                            if let Some(angular_velocity) =
                                robots_trigger_platform_angular_velocity(trig.ttype, &trig.data)
                            {
                                ui.separator();
                                ui.strong("Platform Rotation");
                                quick_grid!(ui, "t_platform_rotation", |ui| {
                                    readonly_input!(
                                        ui,
                                        "Angular Velocity",
                                        format!(
                                            "{:.4}, {:.4}, {:.4} deg/s",
                                            angular_velocity.x,
                                            angular_velocity.y,
                                            angular_velocity.z
                                        )
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Serialized Slots",
                                        "X=data[3], Y=data[4], Z=data[1]".to_string()
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Preview Scale",
                                        format!("{:.2}x", self.platform_rotation_speed_scale)
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Runtime Proof",
                                        "XPathController_Platform converts deg/s with pi/180 and 1/60"
                                            .to_string()
                                    );
                                    ui.end_row();
                                });
                            }

                            if !trig.data.is_empty() {
                                ui.separator();
                                ui.strong("Values");
                                quick_grid!(ui, "t_values", |ui| {
                                    for (i, v) in trig.data.iter().enumerate() {
                                        if let Some(v) = v {
                                            let (name, dtype) = if let Some(Some(ti)) = self
                                                .trigger_info
                                                .triggers
                                                .get(&trig.ttype)
                                                .map(|v| v.values.get(&(i as u32)))
                                            {
                                                (ti.name.clone(), ti.dtype)
                                            } else {
                                                (None, DefinitionDataType::default())
                                            };

                                            readonly_input!(
                                                ui,
                                                name.unwrap_or(format!("#{i} ")),
                                                dtype.to_string(&self.hashcodes, *v)
                                            );
                                            ui.end_row();
                                        }
                                    }
                                });
                            }

                            let any_engine_options = {
                                let e = &trig.engine_options;
                                e.visual_object.is_some()
                                    || e.visual_object_file.is_some()
                                    || e.gamescript_index.is_some()
                                    || e.collision_index.is_some()
                                    || e.trigger_color.is_some()
                                    || e._unk5.is_some()
                                    || e._unk6.is_some()
                                    || e._unk7.is_some()
                            };

                            if any_engine_options {
                                ui.separator();
                                ui.strong("Engine values");
                                quick_grid!(ui, "t_extravalues", |ui| {
                                    if let Some(v) = trig.engine_options.visual_object {
                                        readonly_input!(
                                            ui,
                                            "Visual Object",
                                            DefinitionDataType::Hashcode.to_string(&self.hashcodes, v)
                                        );
                                        ui.end_row();
                                    }
                                    if let Some(v) = trig.engine_options.visual_object_file {
                                        readonly_input!(
                                            ui,
                                            "Visual Object File",
                                            DefinitionDataType::Hashcode.to_string(&self.hashcodes, v)
                                        );
                                        ui.end_row();
                                    }
                                    if let Some(v) = trig.engine_options.gamescript_index {
                                        readonly_input!(
                                            ui,
                                            "GameScript Index",
                                            DefinitionDataType::U32.to_string(&self.hashcodes, v)
                                        );
                                        ui.end_row();
                                        if let Some(script) = &trig.trigger_script {
                                            readonly_input!(ui, "GameScript Offset", format!("0x{:08x}", script.file_offset));
                                            ui.end_row();
                                            readonly_input!(ui, "GameScript Aux", format!("{} / 0x{:08x}", script.aux, script.aux));
                                            ui.end_row();
                                        } else {
                                            readonly_input!(ui, "GameScript Binding", "Invalid or missing serialized entry".to_string());
                                            ui.end_row();
                                        }
                                    }
                                    if let Some(v) = trig.engine_options.collision_index {
                                        readonly_input!(
                                            ui,
                                            "Collision Index",
                                            DefinitionDataType::U32.to_string(&self.hashcodes, v)
                                        );
                                        ui.end_row();
                                    }
                                    if let Some(v) = trig.engine_options.trigger_color {
                                        ui.label("Trigger Color");
                                        ui.horizontal(|ui| {
                                            let (_, color_rect) = ui.allocate_painter(egui::vec2(16.0, 16.0), egui::Sense::hover());
                                            color_rect.rect_filled(color_rect.clip_rect(), 2.0, egui::Color32::from_rgba_premultiplied(v[0], v[1], v[2], v[3]));

                                            ui.label(format!("rgba({0}, {1}, {2}, {3}) / #{0:02x}{1:02x}{2:02x}{3:02x}", v[0], v[1], v[2], v[3]));
                                        });
                                        ui.end_row();
                                    }
                                    if let Some(v) = trig.engine_options._unk5 {
                                        readonly_input!(
                                            ui,
                                            "Unk5",
                                            DefinitionDataType::Unknown32.to_string(&self.hashcodes, v)
                                        );
                                        ui.end_row();
                                    }
                                    if let Some(v) = trig.engine_options._unk6 {
                                        readonly_input!(
                                            ui,
                                            "Unk6",
                                            DefinitionDataType::Unknown32.to_string(&self.hashcodes, v)
                                        );
                                        ui.end_row();
                                    }
                                    if let Some(v) = trig.engine_options._unk7 {
                                        readonly_input!(
                                            ui,
                                            "Unk7",
                                            DefinitionDataType::Unknown32.to_string(&self.hashcodes, v)
                                        );
                                        ui.end_row();
                                    }
                                });
                            }

                            if let Some(coll) = trig
                                .engine_options
                                .collision_index
                                .and_then(|index| map.trigger_collisions.get(index as usize))
                            {
                                ui.separator();
                                ui.strong("Collision datum");
                                quick_grid!(ui, "t_collision", |ui| {
                                    readonly_input!(ui, "Hash Ref", format!("0x{:08x}", coll.hashref));
                                    ui.end_row();
                                    readonly_input!(ui, "Flags", format!("0x{:04x}", coll.flags));
                                    ui.end_row();
                                    readonly_input!(ui, "Type", format!("{} / 0x{:02x}", coll.dtype, coll.dtype));
                                    ui.end_row();
                                    readonly_input!(ui, "Hash Index", format!("{} / 0x{:02x}", coll.hash_index, coll.hash_index));
                                    ui.end_row();
                                    readonly_input!(ui, "Extents", format!("{:.4}, {:.4}, {:.4}", coll.extents[0], coll.extents[1], coll.extents[2]));
                                    ui.end_row();
                                    readonly_input!(ui, "Position", format!("{:.4}, {:.4}, {:.4}", coll.position[0], coll.position[1], coll.position[2]));
                                    ui.end_row();
                                    readonly_input!(ui, "Quaternion", format!("{:.5}, {:.5}, {:.5}, {:.5}", coll.q[0], coll.q[1], coll.q[2], coll.q[3]));
                                    ui.end_row();
                                });
                            }

                            if trig.links.iter().any(|v| *v != -1) {
                                ui.separator();
                                ui.strong("Outgoing Links");

                                quick_grid!(ui, "t_outlinks", |ui| {
                                    for (i, l) in
                                        trig.links.iter().enumerate().filter(|(_, v)| **v != -1)
                                    {
                                        let target = map_trigger_by_link(map, *l);
                                        let resp = ui.horizontal(|ui| {
                                            if let Some((target_index, ltrig)) = target {
                                                readonly_input!(
                                                    ui,
                                                    format!("#{i} "),
                                                    format!(
                                                        "{} (type {})",
                                                        l,
                                                        ttype_or_hex!(ltrig.ttype)
                                                    )
                                                );

                                                if ui
                                                    .button(font_awesome::BULLSEYE.to_string())
                                                    .clicked()
                                                {
                                                    self.go_to_trigger(target_index, ltrig)
                                                }
                                            } else {
                                                readonly_input!(
                                                    ui,
                                                    format!("#{i} "),
                                                    format!("{} (invalid target)", l)
                                                );
                                            }
                                        });

                                        if target.is_some() && resp.response.hovered() {
                                            self.selected_link = Some(*l);
                                        }

                                        ui.end_row();
                                    }
                                });
                            }

                            if !trig.incoming_links.is_empty() {
                                ui.separator();
                                ui.strong(format!(
                                    "Incoming Links ({} links)",
                                    trig.incoming_links.len()
                                ));

                                for l in trig.incoming_links.iter() {
                                    let source = map_trigger_by_link(map, *l);
                                    let resp = ui.horizontal(|ui| {
                                        if let Some((source_index, ltrig)) = source {
                                            readonly_input!(
                                                ui,
                                                format!("{} (type {})", l, ttype_or_hex!(ltrig.ttype))
                                            );

                                            if ui.button(font_awesome::BULLSEYE.to_string()).clicked() {
                                                self.go_to_trigger(source_index, ltrig)
                                            }
                                        } else {
                                            readonly_input!(ui, format!("{} (invalid source)", l));
                                        }
                                    });

                                    if source.is_some() && resp.response.hovered() {
                                        self.selected_link = Some(*l);
                                    }
                                }
                            }
                        }
                    });
            });
    }

    pub(super) fn draw_sound_inspector(&mut self, ctx: &egui::Context, map: &ProcessedMap) {
        let Some(index) = self.selected_sound.filter(|_| self.show_sounds) else {
            return;
        };
        let Some(sound) = map.sounds.get(index) else {
            self.selected_sound = None;
            return;
        };

        let object_name = self
            .hashcodes
            .get(&sound.hashcode)
            .cloned()
            .unwrap_or_else(|| format!("0x{:08x}", sound.hashcode));
        let sound_name = self
            .hashcodes
            .get(&sound.sound_ref)
            .cloned()
            .unwrap_or_else(|| format!("0x{:08x}", sound.sound_ref));
        let zones = map
            .zones
            .iter()
            .enumerate()
            .filter_map(|(zone_index, zone)| {
                zone.sound_array
                    .iter()
                    .any(|sound_index| *sound_index as usize == index)
                    .then_some(zone_index.to_string())
            })
            .collect::<Vec<_>>()
            .join(", ");

        egui::Window::new("Sound Inspector")
            .default_width(430.0)
            .scroll([false, true])
            .show(ctx, |ui| {
                ui.heading(object_name);
                egui::Grid::new("sound_info")
                    .num_columns(2)
                    .striped(true)
                    .spacing([24.0, 4.0])
                    .show(ui, |ui| {
                        let mut row = |label: &str, value: String| {
                            ui.label(label);
                            ui.monospace(value);
                            ui.end_row();
                        };
                        row("Map sound index", index.to_string());
                        row("Object hash", format!("0x{:08x}", sound.hashcode));
                        row(
                            "Sound reference",
                            format!("{} [0x{:08x}]", sound_name, sound.sound_ref),
                        );
                        row(
                            "Position",
                            format!(
                                "{:.3}, {:.3}, {:.3}",
                                sound.position.x, sound.position.y, sound.position.z
                            ),
                        );
                        row("Flags", format!("0x{:08x}", sound.flags));
                        row("Volume", sound.volume.to_string());
                        row("Fade in", sound.fade_in.to_string());
                        row("Fade out", sound.fade_out.to_string());
                        row(
                            "Tracking type",
                            format!(
                                "{} [0x{:02x}, map-emitter semantics unresolved]",
                                sound.tracking_type, sound.tracking_type
                            ),
                        );
                        row("Inner radius", format!("{:.3}", sound.inner_radius));
                        row("Outer radius", format!("{:.3}", sound.outer_radius));
                        row("Base map on", format!("0x{:08x}", sound.base_map_on));
                        row(
                            "Colour",
                            format!(
                                "#{:02X}{:02X}{:02X}{:02X}",
                                sound.color[0], sound.color[1], sound.color[2], sound.color[3]
                            ),
                        );
                        row(
                            "MapZone indices",
                            if zones.is_empty() {
                                "none".to_string()
                            } else {
                                zones.clone()
                            },
                        );
                    });
                ui.separator();
                ui.strong("EuroSound bank preview");
                let mut preview = self.sound_preview.lock();
                preview.draw_settings(ui);
                preview.draw_actions(ui, sound.sound_ref);
            });
    }

    fn go_to_trigger(&mut self, index: usize, trig: &ProcessedTrigger) {
        self.selected_trigger = Some(index);

        let mut v = self.viewer.lock();
        let camera = v.camera_mut();

        self.trigger_focus_tween = Some(Tweeny3D::new(
            tweeny::ease_out_exponential,
            camera.position() + camera.focus_offset(self.trigger_scale),
            trig.position,
            0.5,
        ))
    }
}
