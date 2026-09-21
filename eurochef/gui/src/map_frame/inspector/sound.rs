use super::super::*;

impl MapFrame {
    pub(crate) fn draw_sound_inspector(&mut self, ctx: &egui::Context, map: &ProcessedMap) {
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
        egui::Window::new("Sound Inspector").default_width(430.0).scroll([false, true]).show(ctx, |ui| {
            ui.heading(object_name);
            egui::Grid::new("sound_info").num_columns(2).striped(true).spacing([24.0, 4.0]).show(ui, |ui| {
                let mut row = |label: &str, value: String| { ui.label(label); ui.monospace(value); ui.end_row(); };
                row("Map sound index", index.to_string()); row("Object hash", format!("0x{:08x}", sound.hashcode));
                row("Sound reference", format!("{} [0x{:08x}]", sound_name, sound.sound_ref));
                row("Position", format!("{:.3}, {:.3}, {:.3}", sound.position.x, sound.position.y, sound.position.z));
                row("Flags", format!("0x{:08x}", sound.flags)); row("Volume", sound.volume.to_string()); row("Fade in", sound.fade_in.to_string()); row("Fade out", sound.fade_out.to_string());
                row("Tracking type", format!("{} [0x{:02x}, map-emitter semantics unresolved]", sound.tracking_type, sound.tracking_type));
                row("Inner radius", format!("{:.3}", sound.inner_radius)); row("Outer radius", format!("{:.3}", sound.outer_radius)); row("Base map on", format!("0x{:08x}", sound.base_map_on));
                row("Colour", format!("#{:02X}{:02X}{:02X}{:02X}", sound.color[0], sound.color[1], sound.color[2], sound.color[3]));
                row("MapZone indices", if zones.is_empty() { "none".to_string() } else { zones.clone() });
            });
            ui.separator(); ui.strong("EuroSound bank preview"); let mut preview = self.sound_preview.lock();
            if let Some(profile) = preview.native_sound_profile(sound.sound_ref) {
                egui::Grid::new("native_sound_profile").num_columns(2).striped(true).spacing([24.0, 4.0]).show(ui, |ui| {
                    let mut row = |label: &str, value: String| { ui.label(label); ui.monospace(value); ui.end_row(); };
                    row("Native master", format!("{:.3}", profile.master_volume)); row("Priority", profile.priority.to_string()); row("Max voices", profile.max_voices.to_string());
                    row("Voice group", format!("0x{:04X}, max {}", profile.group_hashcode, profile.group_max_channels)); row("Delay raw", format!("{}..{}", profile.min_delay, profile.max_delay)); row("Ducker", format!("{} / {}", profile.ducker, profile.ducker_length));
                    row("Reverb send raw", profile.reverb_send.to_string()); row("Doppler raw", profile.doppler_value.to_string()); row("Tracking", format!("0x{:02X}", profile.tracking_type)); row("Flags", format!("0x{:04X}", profile.flags)); row("User flags", format!("0x{:04X}", profile.user_flags)); row("User value", profile.user_value.to_string()); row("Duration / loop", format!("{:.4}s / {}", profile.duration_seconds, profile.looping));
                });
                ui.small("Priority, per-SFX MaxVoices and group channel limits are active. Delay, ducker, reverb, Doppler and user fields are preserved raw until their native consumers/units are proven.");
            }
            preview.draw_settings(ui); preview.draw_actions(ui, sound.sound_ref);
        });
    }
}
