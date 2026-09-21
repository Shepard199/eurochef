use super::super::super::super::*;
use super::super::super::widgets::{format_object_audio_sound, property_grid, property_row};

impl MapFrame {
    pub(crate) fn draw_trigger_inspector(&mut self, ctx: &egui::Context, map: &ProcessedMap) {
        let screen_space = ctx.content_rect();
        egui::Window::new(format!("{} Inspector", font_awesome::SEARCH))
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(15, 18, 32))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(82, 75, 130)))
                    .corner_radius(egui::CornerRadius::same(10)),
            )
            .default_width(420.0)
            .scroll([false, true])
            .show(ctx, |ui| {
                // Compact density keeps the inspector readable on smaller viewports.
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 4.0);
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
                        property_row($ui, $label, $string);
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
                        property_grid($label, $ui, $contents);
                    };
                }

                egui::ScrollArea::vertical()
                    .max_height(screen_space.height() - 100.0)
                    .show(ui, |ui| {
                        if let Some(Some(trig)) = self.selected_trigger.map(|v| map.triggers.get(v))
                        {
                            self.draw_trigger_overview(ui, map, trig);

                            egui::CollapsingHeader::new(format!(
                                "{} Diagnostics, runtime & links",
                                font_awesome::STETHOSCOPE
                            ))
                            .default_open(false)
                            .show(ui, |ui| {
                            self.draw_character_body_diagnostics(ctx, ui, map, trig);

                            self.draw_camera_diagnostics(ui, map, trig);

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

                            if trig.ttype == ROBOTS_SWEEPER_EYE_TYPE {
                                ui.separator();
                                ui.strong("Native Sweeper Boss Eye Diagnostics");
                                quick_grid!(ui, "t_native_sweeper_boss_eye", |ui| {
                                    readonly_input!(
                                        ui,
                                        "Runtime Class",
                                        "XTrigger_Sweeper_Boss".to_string()
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Native Role",
                                        "linked eye/spawn trigger owned by XTrigger_Sweeper_Boss_Controller; controller sends 0x100 and consumes the created eye handler state".to_string()
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Runtime Boundary",
                                        "eye handler combat/destruction state is live gameplay input to the final-boss controller; editor proxies are not substituted".to_string()
                                    );
                                    ui.end_row();
                                });
                            }

                            if trig.ttype == ROBOTS_SWEEPER_CONTROLLER_TYPE {
                                let linked_eye_count = trig
                                    .links
                                    .iter()
                                    .filter_map(|link| usize::try_from(*link).ok())
                                    .filter(|index| {
                                        map.triggers
                                            .get(*index)
                                            .is_some_and(|target| target.ttype == ROBOTS_SWEEPER_EYE_TYPE)
                                    })
                                    .count();
                                let initial = NativeSweeperBossControllerSnapshot::default();
                                let pattern_status = map
                                    .sweeper_boss_patterns
                                    .as_ref()
                                    .map(|patterns| {
                                        format!(
                                            "loaded {} / {} native rows",
                                            patterns.rows.len(), ROBOTS_SWEEPER_PATTERN_ROW_COUNT
                                        )
                                    })
                                    .unwrap_or_else(|| "unavailable".to_string());
                                let pattern_candidates = map
                                    .sweeper_boss_patterns
                                    .as_ref()
                                    .map(|patterns| {
                                        (0u8..5)
                                            .map(|random_mod5| {
                                                let group = RobotsSweeperBossPatterns::group_index(
                                                    initial.difficulty,
                                                    random_mod5,
                                                );
                                                let commands = (0..ROBOTS_SWEEPER_EYE_COUNT)
                                                    .filter_map(|eye| {
                                                        patterns.command_for_eye(
                                                            initial.difficulty,
                                                            random_mod5,
                                                            eye,
                                                        )
                                                    })
                                                    .map(|command| {
                                                        format!(
                                                            "e{}:m{}x{}@{:.3}",
                                                            command.eye_ordinal,
                                                            command.monster_id,
                                                            command.spawn_count,
                                                            command.countdown_seed
                                                        )
                                                    })
                                                    .collect::<Vec<_>>();
                                                format!(
                                                    "rng{}→g{} [{}]",
                                                    random_mod5,
                                                    group,
                                                    commands.join(", ")
                                                )
                                            })
                                            .collect::<Vec<_>>()
                                            .join("; ")
                                    })
                                    .unwrap_or_else(|| "unavailable".to_string());
                                let monster_spawn_map = map
                                    .sweeper_boss_patterns
                                    .as_ref()
                                    .map(|patterns| {
                                        let describe = |monster_id: i8, random_mod100: u8| {
                                            match robots_sweeper_boss_spawn_selection(
                                                monster_id,
                                                random_mod100,
                                            ) {
                                                NativeSweeperBossSpawnSelection::NoSpawn => {
                                                    "none".to_string()
                                                }
                                                NativeSweeperBossSpawnSelection::ActivateMonsterTransporter => {
                                                    "MonsterTransporter".to_string()
                                                }
                                                NativeSweeperBossSpawnSelection::Monster {
                                                    config_index,
                                                    ..
                                                } => {
                                                    let file = patterns
                                                        .monster_file(config_index)
                                                        .map(|file| {
                                                            self.hashcodes
                                                                .get(&file)
                                                                .map(|name| {
                                                                    format!(
                                                                        "{name} 0x{file:08X}"
                                                                    )
                                                                })
                                                                .unwrap_or_else(|| {
                                                                    format!("0x{file:08X}")
                                                                })
                                                        })
                                                        .unwrap_or_else(|| {
                                                            "file unresolved".to_string()
                                                        });
                                                    format!("selector {config_index} → {file}")
                                                }
                                            }
                                        };
                                        format!(
                                            "m1 {}; m2 RNG<50 {}, RNG>=50 {}; m3 {}; m4 {}; m5 {}; m6 {}",
                                            describe(1, 0),
                                            describe(2, 49),
                                            describe(2, 50),
                                            describe(3, 0),
                                            describe(4, 0),
                                            describe(5, 0),
                                            describe(6, 0)
                                        )
                                    })
                                    .unwrap_or_else(|| "unavailable".to_string());
                                let eye_spawn_roots = trig
                                    .links
                                    .iter()
                                    .filter_map(|link| usize::try_from(*link).ok())
                                    .filter_map(|index| map.triggers.get(index))
                                    .filter(|target| target.ttype == ROBOTS_SWEEPER_EYE_TYPE)
                                    .enumerate()
                                    .map(|(eye_ordinal, eye)| {
                                        let transform: NativeSweeperBossSpawnTransform =
                                            robots_sweeper_boss_spawn_transform(
                                                eye.position.x,
                                                1.0,
                                                eye.rotation.y,
                                                0,
                                            );
                                        format!(
                                            "e{eye_ordinal}: rootX={:.3} yaw={:.3} → spawn0=[{:.3}, {:.3}, {:.3}, {:.1}]",
                                            eye.position.x,
                                            transform.yaw,
                                            transform.position[0],
                                            transform.position[1],
                                            transform.position[2],
                                            transform.position[3]
                                        )
                                    })
                                    .collect::<Vec<_>>()
                                    .join("; ");
                                let ratchet_missile_hand_summary = map
                                    .sweeper_ratchet_missile_hand_local
                                    .map(|position| {
                                        format!(
                                            "[{:.9}, {:.9}, {:.9}]",
                                            position[0], position[1], position[2]
                                        )
                                    })
                                    .unwrap_or_else(|| "R_Hand firing pose unresolved".to_string());
                                let ratchet_script_clock_summary = map
                                    .sweeper_ratchet_scripts
                                    .as_ref()
                                    .map(|scripts| scripts.diagnostic_summary())
                                    .unwrap_or_else(||
                                        "nb11_rat Script clock profiles unresolved".to_string()
                                    );
                                let ratchet_anchor_summary = map
                                    .sweeper_ratchet_position_local
                                    .map(|local_center| {
                                        trig.links
                                            .iter()
                                            .take(ROBOTS_SWEEPER_EYE_COUNT)
                                            .enumerate()
                                            .map(|(eye_ordinal, link)| {
                                                usize::try_from(*link)
                                                    .ok()
                                                    .and_then(|index| map.triggers.get(index))
                                                    .filter(|eye| eye.ttype == ROBOTS_SWEEPER_EYE_TYPE)
                                                    .map(|eye| {
                                                        let anchor = robots_sweeper_ratchet_anchor(
                                                            [
                                                                eye.position.x,
                                                                eye.position.y,
                                                                eye.position.z,
                                                            ],
                                                            [
                                                                eye.rotation.x,
                                                                eye.rotation.y,
                                                                eye.rotation.z,
                                                            ],
                                                            local_center,
                                                        );
                                                        format!(
                                                            "e{eye_ordinal}=[{:.3}, {:.3}, {:.3}, {:.1}]",
                                                            anchor[0], anchor[1], anchor[2], anchor[3]
                                                        )
                                                    })
                                                    .unwrap_or_else(|| {
                                                        format!("e{eye_ordinal}=unresolved")
                                                    })
                                            })
                                            .collect::<Vec<_>>()
                                            .join("; ")
                                    })
                                    .unwrap_or_else(|| "RatchetPosition datum unresolved".to_string());
                                let format_cutscene_target = |index: usize, target: &ProcessedTrigger| {
                                    if target.ttype == 19 {
                                        format!(
                                            "#{index} XTrigger_Cutscene (type 19, debug {})",
                                            target.debug
                                        )
                                    } else {
                                        format!(
                                            "#{index} type {} (debug {}) [expected XTrigger_Cutscene]",
                                            target.ttype, target.debug
                                        )
                                    }
                                };
                                let ratchet_phase_link_summary = trig
                                    .links
                                    .get(6)
                                    .copied()
                                    .and_then(|link| usize::try_from(link).ok())
                                    .and_then(|index| {
                                        map.triggers
                                            .get(index)
                                            .map(|target| format_cutscene_target(index, target))
                                    })
                                    .unwrap_or_else(|| "controller link6 unresolved".to_string());
                                let ratchet_death_link_summary = map
                                    .triggers
                                    .iter()
                                    .enumerate()
                                    .find(|(_, trigger)| trigger.ttype == 0)
                                    .and_then(|(_, player)| player.links.first().copied())
                                    .and_then(|link| usize::try_from(link).ok())
                                    .and_then(|index| {
                                        map.triggers
                                            .get(index)
                                            .map(|target| format_cutscene_target(index, target))
                                    })
                                    .unwrap_or_else(|| "Player-owner link0 unresolved".to_string());
                                ui.separator();
                                ui.strong("Native Sweeper Boss Controller Diagnostics");
                                quick_grid!(ui, "t_native_sweeper_boss_controller", |ui| {
                                    readonly_input!(
                                        ui,
                                        "Runtime Class",
                                        format!(
                                            "XTrigger_Sweeper_Boss_Controller / class code {}",
                                            ROBOTS_SWEEPER_CONTROLLER_RUNTIME_CLASS_CODE
                                        )
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Controller Service",
                                        format!(
                                            "base distance event0 near-band radius {:.1}; eligible visual-zone service tail-calls +0x60 = 0x004CDF10",
                                            ROBOTS_SWEEPER_CONTROLLER_SERVICE_RADIUS
                                        )
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Controller Replay Core",
                                        "exact composed order after init/transporter refresh: damage-phase arm/stall/ack -> eye-pressure (>5 spawn requests) -> pattern scheduler -> Player-health scheduler; TriggerManager service precedes eye/Ratchet XItem handlers, so eye spawn-count increments become visible on the next controller service; live AI membership, Transporter activity, Player health/contact and pickup ownership remain explicit native inputs"
                                            .to_string()
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Linked Sweeper Eyes",
                                        format!(
                                            "{linked_eye_count} resolved type-{} targets; native shipped controller expects {}",
                                            ROBOTS_SWEEPER_EYE_TYPE, ROBOTS_SWEEPER_EYE_COUNT
                                        )
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Native Init",
                                        format!(
                                            "difficulty={} (cap {}) / fresh eye HP={} / persisted HP={:?} / health-check timer=5.0s",
                                            ROBOTS_SWEEPER_INITIAL_DIFFICULTY,
                                            ROBOTS_SWEEPER_MAX_DIFFICULTY,
                                            ROBOTS_SWEEPER_EYE_INITIAL_HIT_POINTS,
                                            initial.eye_hit_points
                                        )
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Save Record",
                                        format!(
                                            "0x{:X} bytes = difficulty DWORD + {} eye-hit-point DWORDs",
                                            ROBOTS_SWEEPER_CONTROLLER_SAVE_SIZE, ROBOTS_SWEEPER_EYE_COUNT
                                        )
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Eye Vulnerability Cycle",
                                        format!(
                                            ">{} requested Monster instances -> one non-destroyed Closed eye: Opening -> Open {:.1}s -> Closing -> Closed; damage is accepted only while Open",
                                            ROBOTS_SWEEPER_EYE_PRESSURE_THRESHOLD,
                                            ROBOTS_SWEEPER_EYE_OPEN_SECONDS
                                        )
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Eye Hit Outcome",
                                        "fresh HP=1 therefore the first accepted Open-state hit uses HT_Script_Sweeper_Boss_Eye_Destroyed; restored HP>1 uses Eye_Hit then Closing; every accepted hit increments controller difficulty by 1 up to the native cap".to_string()
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Health Pickup Scheduler",
                                        format!(
                                            "type87 service subtracts 1/60s; expired timer with no active pickup rearms to {:.1}s + RNG×{:.1}s before checking Player currentHealth < maxHealth",
                                            ROBOTS_SWEEPER_HEALTH_PICKUP_TIMER_INITIAL_SECONDS,
                                            ROBOTS_SWEEPER_HEALTH_PICKUP_TIMER_JITTER_SECONDS
                                        )
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Health Pickup XItem",
                                        format!(
                                            "HT_Script_Pickup_Health_EnergyCell 0x{:08X}; mask 0x{:X}; raw spawn=[RNG×{:.1}-{:.1}, 0, {:.1}, 0]; yaw += {:.9} rad/update",
                                            ROBOTS_SWEEPER_HEALTH_PICKUP_SCRIPT,
                                            ROBOTS_SWEEPER_HEALTH_PICKUP_REGISTRATION_MASK,
                                            ROBOTS_SWEEPER_HEALTH_PICKUP_SPAWN_X_SCALE,
                                            ROBOTS_SWEEPER_HEALTH_PICKUP_SPAWN_X_BIAS,
                                            ROBOTS_SWEEPER_HEALTH_PICKUP_SPAWN_Z,
                                            ROBOTS_SWEEPER_HEALTH_PICKUP_ROTATION_PER_UPDATE
                                        )
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Health Pickup Lifetime",
                                        format!(
                                            "{:.1}s gate: Event 0x{:08X} WaitForHit pauses Script until valid contact or expiry; Event 0x{:08X} InventoryAdd always releases controller ownership and, only before expiry, adds 1 × HT_Pickup_HealthReplenish 0x{:08X}; successful native inventory status 1/4 immediately restores Player currentHealth to maxHealth",
                                            ROBOTS_SWEEPER_HEALTH_PICKUP_LIFETIME_SECONDS,
                                            ROBOTS_SWEEPER_HEALTH_PICKUP_WAIT_FOR_HIT_EVENT,
                                            ROBOTS_SWEEPER_HEALTH_PICKUP_INVENTORY_ADD_EVENT,
                                            ROBOTS_SWEEPER_HEALTH_PICKUP_ITEM
                                        )
                                    );
                                    ui.end_row();
                                    readonly_input!(ui, "Spawn Pattern Spreadsheet", pattern_status.clone());
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Difficulty-5 RNG Candidates",
                                        pattern_candidates.clone()
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Pattern Cell Role",
                                        "each of 70 rows is 5 × (monster_id:i8, spawn_count:i8); group=min(difficulty*5 + rng%5,69), eye ordinal comes from native controller link order".to_string()
                                    );
                                    ui.end_row();
                                    readonly_input!(ui, "Monster ID → Native Spawn", monster_spawn_map.clone());
                                    ui.end_row();
                                    readonly_input!(ui, "Eye Root → Spawn0", eye_spawn_roots.clone());
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Monster Spawn Pose",
                                        "eye owner stays at the generic CreateItem bootstrap: X=serialized trigger position.x, W=1, yaw=serialized rotation.y; spawn vec4=[eyeX, 2.4, 35.0 + 3.0*spawnOrdinal, 1] and helper ignores eye Y/Z".to_string()
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Monster XItem Factory",
                                        format!(
                                            "create resource 0x{:08X}; XItemPhysics_Character descriptor 0x{:08X}; update-manager mask 0x{:02X} / priority 0x{:02X}; XItem registration flags 0x{:04X}",
                                            ROBOTS_SWEEPER_MONSTER_CREATE_RESOURCE,
                                            ROBOTS_SWEEPER_MONSTER_PHYSICS_DESCRIPTOR,
                                            ROBOTS_SWEEPER_MONSTER_UPDATE_REGISTRATION_MASK,
                                            ROBOTS_SWEEPER_MONSTER_UPDATE_REGISTRATION_PRIORITY,
                                            ROBOTS_SWEEPER_MONSTER_XITEM_REGISTRATION_FLAGS
                                        )
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Ratchet Resource",
                                        format!(
                                            "nb11_rat.edb 0x{:08X}; initial AnimMode 0x{:08X}",
                                            ROBOTS_SWEEPER_RAT_RESOURCE_FILE, ROBOTS_SWEEPER_BOSS_ANIM_MODE
                                        )
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Ratchet Position Datum",
                                        format!(
                                            "HT_Script_Sweeper_Boss 0x{:08X} local Entity HT_Entity_Sweeper contains static HT_AnimDatum_RatchetPosition 0x{:08X}; Script controller is identity",
                                            ROBOTS_SWEEPER_RAT_BASE_SCRIPT,
                                            ROBOTS_SWEEPER_RAT_POSITION_DATUM
                                        )
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Ratchet Eye Anchors",
                                        ratchet_anchor_summary.clone()
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Ratchet Relocation",
                                        format!(
                                            "current<target: one eye step + HT_AnimMode_JumpRight 0x{:08X}, yaw={:.6}; current>target: JumpLeft 0x{:08X}, yaw={:.6}; current==target consumes RNG%5 until a distinct target, then arrival uses Appear 0x{:08X}",
                                            ROBOTS_SWEEPER_JUMP_RIGHT_ANIM_MODE,
                                            ROBOTS_SWEEPER_RAT_JUMP_RIGHT_YAW,
                                            ROBOTS_SWEEPER_JUMP_LEFT_ANIM_MODE,
                                            ROBOTS_SWEEPER_RAT_JUMP_LEFT_YAW,
                                            ROBOTS_SWEEPER_APPEAR_ANIM_MODE
                                        )
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Ratchet Arrival / Facing",
                                        format!(
                                            "SetScriptValue signal {} advances another one-eye relocation step or, at target, requests Appear and forces yaw={:.6}; normal post-switch facing targets Player with native shortest-angle interpolation alpha={:.3} per handler update",
                                            ROBOTS_SWEEPER_RAT_SCRIPT_VALUE_SIGNAL_1,
                                            ROBOTS_SWEEPER_RAT_ARRIVAL_YAW,
                                            ROBOTS_SWEEPER_RAT_FACE_PLAYER_ALPHA
                                        )
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Ratchet Script Handshake",
                                        format!(
                                            "HT_ScriptEvents_SetScriptValue 0x{:08X} writes handler+0x45C via ftol(payload): JumpLeft/Right frame25 -> {}; Appear frame3 -> {}, frame18 -> {}; DisAppear frame18 -> {}; Attack frame35 -> {}; Attack2 frame10 -> {}, frame145 -> {}; IdleCombat1/2/3 frames 85/75/90 -> {}",
                                            ROBOTS_SWEEPER_RAT_SCRIPT_VALUE_EVENT,
                                            ROBOTS_SWEEPER_RAT_SCRIPT_VALUE_SIGNAL_1,
                                            ROBOTS_SWEEPER_RAT_SCRIPT_VALUE_SIGNAL_1,
                                            ROBOTS_SWEEPER_RAT_SCRIPT_VALUE_SIGNAL_2,
                                            ROBOTS_SWEEPER_RAT_SCRIPT_VALUE_SIGNAL_1,
                                            ROBOTS_SWEEPER_RAT_SCRIPT_VALUE_SIGNAL_1,
                                            ROBOTS_SWEEPER_RAT_SCRIPT_VALUE_SIGNAL_1,
                                            ROBOTS_SWEEPER_RAT_SCRIPT_VALUE_SIGNAL_2,
                                            ROBOTS_SWEEPER_RAT_SCRIPT_VALUE_SIGNAL_1
                                        )
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Ratchet Family4 Clock",
                                        ratchet_script_clock_summary.clone()
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Serialized AI Ownership",
                                        "type10 near-band service 0x0044D110 invokes +0x24=0x0047E4F0 only while trigger+0x68 has no live XItem; create stores bidirectional ownership trigger+0x68 <-> XItem+0x154. Base XItem destructor 0x00443CC0 verifies creator +0x80 returns the same XItem, then clears both pointers, allowing later proximity recreate with a new live identity. Boss live-AI registry tracks SerializedTrigger(index) separately from EyeSpawn identities"
                                            .to_string()
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Boss Replay Order",
                                        "native 60Hz slice: TriggerManager/controller first; five Eye XItems use update priority 0x14, Ratchet uses 0x32. Controller pattern commands are owned by Eye command_state/countdown; ready Eyes execute 0x004CF1D0 before Ratchet. Fresh monster XItems register immediately at priority 0x32 after the already-registered Ratchet: Ratchet sees them in the live-AI blocker scan this tick although their own Handler updates later. monster_id6 activates controller link5 and writes +0x136 immediately, so transporter blocking is also same-tick. Eye spawn requests increment controller+0x110 after controller service and are consumed next service. Controller cleanup marks old AI destroy-bit 0x10; 0x00444E80 flushes them only after all XItem updates"
                                            .to_string()
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Ratchet Top-State Core",
                                        "exact 0..10 reducer: 0 bootstrap -> IdleAttack/state4; state1 relocation/Jump/Appear returns before common pose tail; state2 DisAppear completion -> travel; state3 Appear signal2 -> IdleAttack; state4 runs the 51-update combat selector; states5/6/7 signal1 -> IdleAttack; state8 signal1 persists controller +0x495 ack and signal2 -> IdleAttack; state9 signal1 persists ack, requests DisAppear and resets combat phase; state10 is death. Native Ratchet-local writers produce states 0/1/2/3/4/5/6/8/9/10; state7 has proven consumer semantics but no Ratchet-local producer in the executable write census. Damage +0x48E is derived from entry-state, so a state4 tick that selects Attack remains invulnerable until the next Ratchet update"
                                            .to_string()
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Ratchet Damage Phase",
                                        format!(
                                            "HP={}; difficulty >= {} arms handler+0x494 once, clears live AI and stalls controller until +0x495 acknowledgement; +0x48E is rebuilt every Ratchet update and accepts damage only in Appear after signal or states 5..8",
                                            ROBOTS_SWEEPER_RAT_INITIAL_HIT_POINTS,
                                            ROBOTS_SWEEPER_RAT_DAMAGE_PHASE_DIFFICULTY
                                        )
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Phase / Death Links",
                                        format!(
                                            "difficulty-arm event mask1: controller link6 -> {}; HP<=0 event mask1: current Player XItem owner XTrigger_Player link0 -> {}; both use common trigger lifecycle, not a fabricated direct cutscene call",
                                            ratchet_phase_link_summary,
                                            ratchet_death_link_summary
                                        )
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Ratchet Hit / Death",
                                        format!(
                                            "accepted nonlethal hit: RNG%100 < {} -> Hit_F 0x{:08X} / AnimSet 0x{:08X} / Script 0x{:08X} signal1@frame{}; otherwise Hit_B 0x{:08X} / 0x{:08X} / 0x{:08X} signal1@frame{}; HP<=0 -> Death1 0x{:08X} / 0x{:08X} / 0x{:08X} signal1@frame{} + live-AI cleanup",
                                            ROBOTS_SWEEPER_RAT_HIT_FORWARD_THRESHOLD,
                                            ROBOTS_SWEEPER_RAT_HIT_FORWARD_ANIM_MODE,
                                            ROBOTS_SWEEPER_RAT_HIT_FORWARD_ANIM_SET,
                                            ROBOTS_SWEEPER_RAT_HIT_FORWARD_SCRIPT,
                                            ROBOTS_SWEEPER_RAT_HIT_FORWARD_SIGNAL_FRAME,
                                            ROBOTS_SWEEPER_RAT_HIT_BACK_ANIM_MODE,
                                            ROBOTS_SWEEPER_RAT_HIT_BACK_ANIM_SET,
                                            ROBOTS_SWEEPER_RAT_HIT_BACK_SCRIPT,
                                            ROBOTS_SWEEPER_RAT_HIT_BACK_SIGNAL_FRAME,
                                            ROBOTS_SWEEPER_RAT_DEATH_ANIM_MODE,
                                            ROBOTS_SWEEPER_RAT_DEATH_ANIM_SET,
                                            ROBOTS_SWEEPER_RAT_DEATH_SCRIPT,
                                            ROBOTS_SWEEPER_RAT_DEATH_SIGNAL_FRAME
                                        )
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Missile Producer",
                                        format!(
                                            "AnimMode 0x{:08X} -> AnimSet 0x{:08X} -> Script 0x{:08X} -> Event 0x{:08X}",
                                            ROBOTS_SWEEPER_ATTACK_ANIM_MODE,
                                            ROBOTS_SWEEPER_ATTACK_ANIM_SET,
                                            ROBOTS_SWEEPER_ATTACK_SCRIPT,
                                            ROBOTS_SWEEPER_MISSILE_EVENT
                                        )
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Missile Launch Timing",
                                        "same XItem update: Ratchet handler first refreshes current eye anchor + Player-facing yaw, then attached Script/Anim animator dispatches the missile Event; R_Hand query uses previous live animation sample frame 16.5, but XItem applies the already-current owner transform and returns world [x,y,z,0]".to_string()
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Missile Factory",
                                        format!(
                                            "file 0x{:08X}, Script 0x{:08X}, mask 0x01 / priority 0x14 / Projectile Physics",
                                            ROBOTS_SWEEPER_MISSILE_RESOURCE_FILE,
                                            ROBOTS_SWEEPER_MISSILE_SCRIPT
                                        )
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Launch Attachment",
                                        format!(
                                            "HT_AnimBone_R_Hand 0x{:08X}; XItem query -> Anim live SkinAnim matrix",
                                            ROBOTS_SWEEPER_MISSILE_LAUNCH_BONE
                                        )
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Exact Event Pose",
                                        format!(
                                            "firing Animation 0x{:08X} @ raw frame {:.1}; previous-live R_Hand={} decoded directly from nb11_rat.edb before pose flush",
                                            ROBOTS_SWEEPER_ATTACK_ANIMATION,
                                            ROBOTS_SWEEPER_MISSILE_LIVE_BONE_FRAME,
                                            ratchet_missile_hand_summary
                                        )
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Runtime Boundary",
                                        "missile launch transform and frame ordering are exact; automatic spawning still depends on replaying the recovered controller RNG/live AI+transporter/Player/eye state and exact Attack Script timeline, so no fake periodic firing is synthesized".to_string()
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

                            let trigger_index = self.selected_trigger.unwrap_or_default();
                            let object_audio_profile = if trig.ttype == 79 {
                                robots_direct_object_audio_profile(trig)
                            } else {
                                robots_object_audio_profile_for_source(map, trigger_index)
                            };
                            if let Some(profile) = object_audio_profile {
                                ui.separator();
                                ui.strong("Native Object Audio Profile");
                                quick_grid!(ui, "t_native_object_audio", |ui| {
                                    readonly_input!(
                                        ui,
                                        "Runtime Role",
                                        if trig.ttype == 79 {
                                            "data-only four-slot sound profile; it does not handle events itself"
                                                .to_string()
                                        } else {
                                            "native object consumes linked profile through vtable slot +0xEC"
                                                .to_string()
                                        }
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Profile Source",
                                        profile
                                            .linked_trigger_index
                                            .map(|index| format!("XTrigger_ObjectAudio #{index}"))
                                            .unwrap_or_else(|| {
                                                if trig.ttype == 79 {
                                                    format!("selected XTrigger_ObjectAudio #{trigger_index}")
                                                } else {
                                                    "native class fallback table".to_string()
                                                }
                                            })
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Audio Gate",
                                        if trig.ttype == 79 {
                                            "not applicable; controlled by linked consumer".to_string()
                                        } else {
                                            let state = if robots_object_audio_is_enabled(trig) {
                                                "enabled"
                                            } else {
                                                "disabled"
                                            };
                                            let proof = match trig.ttype {
                                                7 => "data[4] bit 0",
                                                34 => "data[5] bit 0",
                                                8 => "data[7] bit 0x100",
                                                32 => "data[3] bit 0x200",
                                                37 => "data[2] bit 0x800",
                                                55 | 80 => "unconditional native consumer",
                                                _ => "not a native ObjectAudio consumer",
                                            };
                                            format!("{state} ({proof})")
                                        }
                                    );
                                    ui.end_row();
                                    for (slot, role) in [
                                        (0, "data[0] Activate One-Shot"),
                                        (1, "data[1] Deactivate One-Shot"),
                                        (2, "data[2] Active Loop"),
                                        (3, "data[3] Inactive Loop"),
                                    ] {
                                        let raw_sound = if trig.ttype == 79 {
                                            trig.data.get(slot).copied().flatten()
                                        } else {
                                            profile.sound(slot)
                                        };
                                        let display = if trig.ttype == 79
                                            && raw_sound == Some(0x1AF0_0001)
                                        {
                                            "HT_Sound_SFX_AA_BLANK (use consumer native fallback)"
                                                .to_string()
                                        } else {
                                            format_object_audio_sound(&self.hashcodes, raw_sound)
                                        };
                                        readonly_input!(ui, role, display);
                                        ui.end_row();
                                    }
                                    if trig.ttype == 79 {
                                        let consumers = trig
                                            .incoming_links
                                            .iter()
                                            .filter_map(|link| usize::try_from(*link).ok())
                                            .filter_map(|index| {
                                                map.triggers
                                                    .get(index)
                                                    .filter(|consumer| {
                                                        robots_object_audio_is_consumer(consumer.ttype)
                                                    })
                                                    .map(|consumer| {
                                                    let name = self
                                                        .trigger_info
                                                        .triggers
                                                        .get(&consumer.ttype)
                                                        .map(|info| info.name.as_str())
                                                        .unwrap_or("Unknown");
                                                    format!("#{index} {name}")
                                                })
                                            })
                                            .collect::<Vec<_>>();
                                        readonly_input!(
                                            ui,
                                            "Incoming Consumers",
                                            if consumers.is_empty() {
                                                "none".to_string()
                                            } else {
                                                consumers.join(", ")
                                            }
                                        );
                                        ui.end_row();
                                    }
                                    readonly_input!(
                                        ui,
                                        "Native Events",
                                        if trig.ttype == 55 {
                                            "Clock: 0x100 starts Active Loop; 0x200 stops it; slots 0/1/3 are unused"
                                                .to_string()
                                        } else {
                                            "0x100: stop inactive loop, start active loop + activate one-shot; 0x200: stop active loop, start inactive loop + deactivate one-shot"
                                                .to_string()
                                        }
                                    );
                                    ui.end_row();
                                });
                            }

                            if Self::runtime_event_supported(map, trig) {
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
                                    if let Some(runtime_type) = robots_character_runtime_type(trig.ttype) {
                                        let ai_key = Self::runtime_event_key(map.hashcode, trigger_index);
                                        let ai_xitem_live = self
                                            .native_ai_trigger_lifecycle
                                            .get(&ai_key)
                                            .is_some_and(|state| state.xitem_exists);
                                        readonly_input!(
                                            ui,
                                            "AI XItem",
                                            if ai_xitem_live { "live" } else { "absent" }.to_string()
                                        );
                                        ui.end_row();
                                        readonly_input!(ui, "AI Runtime Type", runtime_type.to_string());
                                        ui.end_row();
                                    }
                                    if trig.ttype == 48 {
                                        let npc_key = Self::runtime_event_key(map.hashcode, trigger_index);
                                        let automatic_focus_owner = self
                                            .native_player_focus_runtime
                                            .current_owner
                                            .is_some_and(|owner| owner.key == npc_key);
                                        readonly_input!(
                                            ui,
                                            "Player Focus Owner",
                                            if automatic_focus_owner { "yes" } else { "no" }.to_string()
                                        );
                                        ui.end_row();
                                        let behavior = self.native_npc_behavior.get(&npc_key);
                                        readonly_input!(
                                            ui,
                                            "NPC Proximity",
                                            behavior
                                                .map(|runtime| if runtime.proximity.node_active { "active" } else { "idle" })
                                                .unwrap_or("unsupported")
                                                .to_string()
                                        );
                                        ui.end_row();
                                        readonly_input!(
                                            ui,
                                            "NPC Selector",
                                            behavior
                                                .and_then(|runtime| runtime.active_node)
                                                .map(|node| format!("{node:?}"))
                                                .unwrap_or_else(|| "none".to_string())
                                        );
                                        ui.end_row();
                                        readonly_input!(
                                            ui,
                                            "NPC Behavior Action",
                                            behavior
                                                .and_then(|runtime| runtime.last_action)
                                                .map(|action| format!("{action:?}"))
                                                .unwrap_or_else(|| "none".to_string())
                                        );
                                        ui.end_row();
                                        readonly_input!(
                                            ui,
                                            "NPC Flag2 Route",
                                            behavior
                                                .map(|runtime| format!(
                                                    "len {} index {} {} cooldown {:.3}",
                                                    runtime.flag2.route.len(),
                                                    runtime.flag2.route_index,
                                                    if runtime.flag2.reverse { "reverse" } else { "forward" },
                                                    runtime.flag2.rebuild_cooldown_seconds,
                                                ))
                                                .unwrap_or_else(|| "unsupported".to_string())
                                        );
                                        ui.end_row();
                                        readonly_input!(
                                            ui,
                                            "NPC NavMesh",
                                            self.native_monster_navigation
                                                .get(&npc_key)
                                                .map(|state| format!(
                                                    "region {:?} face {:?} group {:?}",
                                                    state.region_ordinal,
                                                    state.face_index,
                                                    state.group_flags0,
                                                ))
                                                .unwrap_or_else(|| "none".to_string())
                                        );
                                        ui.end_row();
                                        ui.label("NPC Runtime");
                                        ui.horizontal(|ui| {
                                            if ui.button("Focus").clicked() {
                                                let _ = self.native_npc_focus_update(
                                                    map,
                                                    trigger_index,
                                                    true,
                                                );
                                            }
                                            if ui.button("Blur").clicked() {
                                                let _ = self.native_npc_focus_update(
                                                    map,
                                                    trigger_index,
                                                    false,
                                                );
                                            }
                                            if ui.button("Interact").clicked() {
                                                if let Some(presentation) = self.native_npc_execute_interaction(
                                                    map,
                                                    trigger_index,
                                                    wall_time,
                                                ) {
                                                    ui.label(format!("{presentation:?}"));
                                                }
                                            }
                                        });
                                        ui.end_row();
                                    }
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

                            if trig.ttype == 50 {
                                let data_u32 = |index: usize| {
                                    trig.data.get(index).and_then(|value| *value)
                                };
                                let data_f32 = |index: usize| data_u32(index).map(f32::from_bits);
                                let width = data_u32(0).unwrap_or_default();
                                let height = data_u32(1).unwrap_or_default();
                                let cells = u64::from(width) * u64::from(height);
                                let initial_waves = data_u32(5).unwrap_or_default();
                                let flags = data_u32(6).unwrap_or_default();
                                let periodic_interval = data_u32(7).unwrap_or_default();

                                ui.separator();
                                ui.strong("Native Fluid Diagnostics");
                                quick_grid!(ui, "t_native_fluid", |ui| {
                                    readonly_input!(
                                        ui,
                                        "Runtime Class",
                                        "XTrigger_Fluid -> XItemHandler_Fluid".to_string()
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Water Grid",
                                        format!("{width} x {height} = {cells} cells")
                                    );
                                    ui.end_row();
                                    if let Some(quad_size) = data_f32(4) {
                                        readonly_input!(
                                            ui,
                                            "Entity Quad Size",
                                            format!("{quad_size:.6}")
                                        );
                                        ui.end_row();
                                    }
                                    readonly_input!(
                                        ui,
                                        "Initial Random Waves",
                                        initial_waves.to_string()
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Periodic Waves",
                                        if flags & 1 != 0 {
                                            format!("enabled, every {periodic_interval} ticks")
                                        } else {
                                            "disabled".to_string()
                                        }
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Native Solver",
                                        "0x005129A2 grid / 0x00512F1D local disturbance".to_string()
                                    );
                                    ui.end_row();
                                    readonly_input!(
                                        ui,
                                        "Runtime Boundary",
                                        "Native water deformation and gameplay body response are recovered diagnostically but are not yet simulated in Maps"
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
                            });
                        }
                    });
            });
    }
}
