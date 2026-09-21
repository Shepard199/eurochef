use super::super::super::*;
use super::super::widgets::{property_grid, property_row};

impl MapFrame {
    pub(super) fn draw_character_body_diagnostics(
        &self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        map: &ProcessedMap,
        trig: &ProcessedTrigger,
    ) {
        let Some(trigger_index) = self.selected_trigger else {
            return;
        };
        macro_rules! readonly_input {
            ($ui:expr, $label:expr, $value:expr) => {{
                property_row($ui, $label, $value);
            }};
        }
        macro_rules! quick_grid {
            ($ui:expr, $label:expr, $contents:expr) => {{
                property_grid($label, $ui, $contents);
            }};
        }
        if let Some(body) = self.runtime_character_body(map.hashcode, trigger_index) {
            ui.separator();
            ui.strong("Native Gameplay Body");
            quick_grid!(ui, "t_native_character_body", |ui| {
                if let Some(visual) = trig.character_visual.as_ref() {
                    readonly_input!(
                        ui,
                        "Native handler",
                        format!(
                            "{}  descriptor=0x{:08X}  vtable=0x{:08X}",
                            visual.handler_class.native_name(),
                            visual.handler_class.descriptor_address(),
                            visual.handler_class.vtable_address(),
                        )
                    );
                    ui.end_row();
                }
                readonly_input!(
                    ui,
                    "Registration mask",
                    format!(
                        "0x{:08X} (Physics contact channel bit0)",
                        body.registration_mask
                    )
                );
                ui.end_row();
                readonly_input!(ui, "AnimSkin", format!("0x{:08X}", body.collision.animskin));
                ui.end_row();
                readonly_input!(
                    ui,
                    "Shape",
                    match body.collision.shape {
                        crate::maps::ProcessedCharacterCollisionShape::Sphere { radius } =>
                            format!("Sphere  radius={radius:.6}"),
                        crate::maps::ProcessedCharacterCollisionShape::Capsule {
                            half_segment,
                            radius,
                        } => format!("Capsule  half-segment={half_segment:.6}  radius={radius:.6}"),
                    }
                );
                ui.end_row();
                readonly_input!(
                    ui,
                    "Local center",
                    format!(
                        "{:.6}, {:.6}, {:.6}",
                        body.collision.local_center.x,
                        body.collision.local_center.y,
                        body.collision.local_center.z
                    )
                );
                ui.end_row();
                readonly_input!(
                    ui,
                    "Transform selector",
                    format!(
                        "{} (AnimDatum +0x30 bone matrix)",
                        body.collision.transform_selector
                    )
                );
                ui.end_row();
                readonly_input!(
                    ui,
                    "Owner position",
                    format!(
                        "{:.6}, {:.6}, {:.6}",
                        body.owner_position.x, body.owner_position.y, body.owner_position.z
                    )
                );
                ui.end_row();
                if let Some(track) = body.initial_animation.as_ref() {
                    readonly_input!(
                                                ui,
                                                "Native initial animation",
                                                format!(
                                                    "0x{:08X} Idle_Attack, AnimSkin=0x{:08X}, frames={}, effective rate={} frames/s (serialized /60 per 60-Hz tick), loop flags=0x3, chain={} bones",
                                                    track.animation,
                                                    track.animskin,
                                                    track.frame_count,
                                                    track.clip_rate,
                                                    track.bone_chain.len()
                                                )
                                            );
                    ui.end_row();
                    readonly_input!(
                        ui,
                        "Initial clip clock",
                        format!(
                            "{:.3}s; frame position {:.3}",
                            body.initial_animation_seconds,
                            (body.initial_animation_seconds * f32::from(track.clip_rate))
                                % track.frame_count.max(1) as f32
                        )
                    );
                    ui.end_row();
                }
                readonly_input!(
                                            ui,
                                            "Resolved Move animation",
                                            match body.move_animation.as_ref() {
                                                Some(track) => format!(
                                                    "0x{:08X}, AnimSkin=0x{:08X}, frames={}, rate={} frames/s, chain={} bones; predecoded only, gameplay Move state not activated",
                                                    track.animation,
                                                    track.animskin,
                                                    track.frame_count,
                                                    track.clip_rate,
                                                    track.bone_chain.len()
                                                ),
                                                None => "not present/resolved for this character EDB".to_owned(),
                                            }
                                        );
                ui.end_row();
                if trig
                    .character_visual
                    .as_ref()
                    .is_some_and(|visual| {
                        visual.handler_class
                            == eurochef_shared::robots_runtime::ai_character::RobotsAiHandlerClass::Eq02MineBot
                    })
                {
                    readonly_input!(
                        ui,
                        "MineBot gameplay target",
                        match crate::map_runtime::runtime_player_spawn_state(map) {
                            Some(player) => format!(
                                "XTrigger_Player #{} mode={} spawn=[{:.4}, {:.4}, {:.4}]",
                                player.trigger_index,
                                player.mode,
                                player.position.x,
                                player.position.y,
                                player.position.z
                            ),
                            None => "no XTrigger_Player (ttype 0) in this map".to_owned(),
                        }
                    );
                    ui.end_row();
                    readonly_input!(
                        ui,
                        "MineBot Move gate",
                        match crate::map_runtime::runtime_player_spawn_state(map) {
                            Some(player) => {
                                let distance = trig.position.distance(player.position);
                                let enter = crate::map_runtime::runtime_minebot_move_gate(
                                    trig.position,
                                    player.position,
                                    false,
                                );
                                let keep = crate::map_runtime::runtime_minebot_move_gate(
                                    trig.position,
                                    player.position,
                                    true,
                                );
                                format!(
                                                            "spawn distance={distance:.4}; enter(<12)={enter}; continue(<16)={keep}; diagnostic only"
                                                        )
                            }
                            None => "unavailable without gameplay Player spawn owner".to_owned(),
                        }
                    );
                    ui.end_row();
                    readonly_input!(
                        ui,
                        "MineBot Attack vs Move",
                        match crate::map_runtime::runtime_player_spawn_state(map) {
                            Some(player) => {
                                let move_enter = crate::map_runtime::runtime_minebot_move_gate(
                                    trig.position,
                                    player.position,
                                    false,
                                );
                                let attack_geometry =
                                    crate::map_runtime::runtime_minebot_attack_gate(
                                        trig.position,
                                        player.position,
                                        true,
                                    );
                                match crate::map_runtime::runtime_map_line_of_sight_clear(
                                                            map,
                                                            trig.position,
                                                            player.position,
                                                        ) {
                                                            Some(false) => {
                                                                let winner = crate::map_runtime::runtime_minebot_move_attack_winner(
                                                                    move_enter,
                                                                    false,
                                                                );
                                                                format!(
                                                                    "Attack ring [5,19], |dy|<=1000, priority=0x51; Move priority=0x50; native static Map LOS=blocked; attack_eligible=false; local winner={winner:?}."
                                                                )
                                                            }
                                                            Some(true) => match self.live_dynamic_script_los_state(
                                                                ctx,
                                                                map,
                                                                trig.position,
                                                                player.position,
                                                            ) {
                                                                Some((dynamic_clear, live_scripts, entity_instances)) => {
                                                                    let attack_eligible = attack_geometry && dynamic_clear;
                                                                    let winner = crate::map_runtime::runtime_minebot_move_attack_winner(
                                                                        move_enter,
                                                                        attack_eligible,
                                                                    );
                                                                    format!(
                                                                        "Attack ring [5,19], |dy|<=1000, priority=0x51; Move priority=0x50; native static Map LOS=clear; live registration-mask-0x02 Script LOS={dynamic_clear}; live_scripts={live_scripts}; entity_instances={entity_instances}; attack_eligible={attack_eligible}; local winner={winner:?}."
                                                                    )
                                                                }
                                                                None => format!(
                                                                    "Attack ring [5,19], |dy|<=1000, priority=0x51; Move priority=0x50; native static Map LOS=clear; live Script LOS unavailable until a native Camera viewpoint has produced MapZone +0x6A/+0x6B state; geometry_if_visible={attack_geometry}; final winner unavailable."
                                                                ),
                                                            },
                                                            None => format!(
                                                                "Attack ring [5,19], |dy|<=1000, priority=0x51; Move priority=0x50; static Map LOS unresolved; geometry_if_visible={attack_geometry}; final visibility/winner unavailable."
                                                            ),
                                                        }
                            }
                            None => "unavailable without gameplay Player spawn owner".to_owned(),
                        }
                    );
                    ui.end_row();
                }
                let current_world_shape = body.current_world_shape();
                readonly_input!(
                                            ui,
                                            "Current world shape",
                                            match current_world_shape {
                                                crate::map_runtime::RuntimeCharacterWorldShape::Sphere {
                                                    center_xyz,
                                                    radius,
                                                } => format!(
                                                    "animated sphere center=[{:.4}, {:.4}, {:.4}] r={radius:.4}",
                                                    center_xyz[0], center_xyz[1], center_xyz[2]
                                                ),
                                                crate::map_runtime::RuntimeCharacterWorldShape::Capsule {
                                                    start_xyz,
                                                    delta_xyz,
                                                    radius,
                                                } => format!(
                                                    "animated capsule start=[{:.4}, {:.4}, {:.4}] delta=[{:.4}, {:.4}, {:.4}] r={radius:.4}",
                                                    start_xyz[0],
                                                    start_xyz[1],
                                                    start_xyz[2],
                                                    delta_xyz[0],
                                                    delta_xyz[1],
                                                    delta_xyz[2]
                                                ),
                                            }
                                        );
                ui.end_row();
                readonly_input!(
                                            ui,
                                            "Runtime boundary",
                                            "Freshly-created AI body now samples the native-proven layer-0 Idle_Attack EDB pose and applies the selected AnimDatum bone matrix to world collision. Later AI state/clip transitions remain unresolved; camera/player preview is not used."
                                                .to_string()
                                        );
                ui.end_row();
            });
        }
    }
}
