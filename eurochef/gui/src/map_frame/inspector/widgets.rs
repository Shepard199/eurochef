use super::super::*;

pub(super) fn format_object_audio_sound(
    hashcodes: &IntMap<u32, String>,
    sound: Option<u32>,
) -> String {
    let Some(sound) = sound else {
        return "none / native fallback unavailable".to_string();
    };
    hashcodes
        .get(&sound)
        .map(|name| format!("{name} (0x{sound:08X})"))
        .unwrap_or_else(|| format!("0x{sound:08X}"))
}

/// Dense diagnostic rows with an explicit copy affordance for addresses and hashes.
pub(super) fn property_row(
    ui: &mut egui::Ui,
    label: impl Into<egui::WidgetText>,
    value: impl Into<String>,
) {
    let value = value.into();
    ui.label(label);
    ui.horizontal(|ui| {
        ui.monospace(&value);
        if ui.small_button(font_awesome::COPY.to_string()).clicked() {
            ui.ctx().copy_text(value.clone());
        }
    });
}

pub(super) fn property_grid(
    id: impl std::hash::Hash + std::fmt::Debug,
    ui: &mut egui::Ui,
    add_rows: impl FnOnce(&mut egui::Ui),
) {
    egui::Grid::new(id)
        .num_columns(2)
        .spacing([12.0, 3.0])
        .striped(true)
        .show(ui, add_rows);
}

pub(super) fn inspector_card(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(egui::Color32::from_rgb(22, 25, 43))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(65)))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(8))
        .show(ui, add_contents);
}
