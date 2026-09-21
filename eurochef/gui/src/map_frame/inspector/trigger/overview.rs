use super::super::super::*;
use super::super::widgets::{inspector_card, property_grid, property_row};

impl MapFrame {
    pub(super) fn draw_trigger_overview(
        &self,
        ui: &mut egui::Ui,
        map: &ProcessedMap,
        trig: &ProcessedTrigger,
    ) {
        let type_name = |value| {
            self.trigger_info
                .triggers
                .get(&value)
                .map(|info| format!("{} (0x{value:x})", info.name))
                .unwrap_or_else(|| format!("0x{value:x}"))
        };
        inspector_card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong(format!("{} Overview & Transform", font_awesome::CUBE));
                ui.colored_label(
                    egui::Color32::from_rgb(112, 186, 255),
                    type_name(trig.ttype),
                );
            });
            ui.add_space(4.0);
            property_grid("t_info", ui, |ui| {
                let mut row = |label: &str, value: String| {
                    property_row(ui, label, value);
                    ui.end_row();
                };
                row("Type", type_name(trig.ttype));
                row(
                    "Subtype",
                    trig.tsubtype
                        .map(type_name)
                        .unwrap_or_else(|| "None".to_owned()),
                );
                row(
                    "Type Index",
                    format!("{} / 0x{:x}", trig.type_index, trig.type_index),
                );
                row("File Offset", format!("0x{:08x}", trig.file_offset));
                row("Link Ref", trig.link_ref.to_string());
                row("Debug", format!("{} / 0x{:x}", trig.debug, trig.debug));
                row("Game Flags", format!("0x{:08x}", trig.game_flags));
                row("Trigger Flags", format!("0x{:08x}", trig.trig_flags));
                row(
                    "Position",
                    format!(
                        "{:.3}, {:.3}, {:.3}",
                        trig.position.x, trig.position.y, trig.position.z
                    ),
                );
                row(
                    "Rotation",
                    format!(
                        "{:.3}, {:.3}, {:.3}",
                        trig.rotation.x.to_degrees(),
                        trig.rotation.y.to_degrees(),
                        trig.rotation.z.to_degrees()
                    ),
                );
                row(
                    "Scale",
                    format!(
                        "{:.2}, {:.2}, {:.2}",
                        trig.scale.x, trig.scale.y, trig.scale.z
                    ),
                );
                if let Some(coll) = trig
                    .engine_options
                    .collision_index
                    .and_then(|index| map.trigger_collisions.get(index as usize))
                {
                    row(
                        "Collision",
                        match coll.dtype {
                            0 => "Box".to_owned(),
                            3 => "Cylinder".to_owned(),
                            value => format!(
                                "{} Unknown collision type {value}",
                                font_awesome::EXCLAMATION_TRIANGLE
                            ),
                        },
                    );
                }
            });
            ui.horizontal(|ui| {
                for (axis, value, colour) in [
                    ("X", trig.position.x, egui::Color32::from_rgb(240, 95, 95)),
                    ("Y", trig.position.y, egui::Color32::from_rgb(92, 210, 130)),
                    ("Z", trig.position.z, egui::Color32::from_rgb(98, 155, 255)),
                ] {
                    egui::Frame::new()
                        .fill(colour.gamma_multiply(0.16))
                        .corner_radius(egui::CornerRadius::same(5))
                        .inner_margin(egui::Margin::symmetric(6, 3))
                        .show(ui, |ui| {
                            ui.colored_label(colour, format!("{axis} {value:.3}"));
                        });
                }
            });
        });
    }
}
