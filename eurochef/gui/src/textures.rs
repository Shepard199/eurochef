use std::sync::Arc;

use egui::{Color32, RichText, Widget};
use eurochef_edb::Hashcode;
use eurochef_shared::{
    maps::{
        format_hashcode_with_id, format_typed_hashcode_in_edb, format_typed_hashcode_with_id_in_edb,
    },
    textures::UXGeoTexture,
    IdentifiableResult,
};
use fnv::FnvHashMap;
use instant::Instant;
use nohash_hasher::IntMap;

use crate::strip_ansi_codes;

pub struct TextureList {
    file: Hashcode,
    textures: Vec<IdentifiableResult<UXGeoTexture>>,
    hashcodes: Arc<IntMap<Hashcode, String>>,

    // Each serialized Texture UID keeps its own identity, while exact TXG
    // duplicates may share the same GPU TextureHandles.
    egui_textures: FnvHashMap<u32, Vec<egui::TextureHandle>>,
    identity_group_count: usize,
    identity_reused_texture_count: usize,

    start_time: Instant,

    // Options/filters
    zoom: f32,
    filter_animated: bool,

    enlarged_texture: Option<(usize, u32)>,
    enlarged_zoom: f32,

    fallback_texture: egui::TextureHandle,
}

impl TextureList {
    const ENLARGED_ZOOM_DEFAULT: f32 = 2.5;

    pub fn new(
        ctx: &egui::Context,
        file: Hashcode,
        textures: Vec<IdentifiableResult<UXGeoTexture>>,
        hashcodes: Arc<IntMap<Hashcode, String>>,
    ) -> Self {
        let mut s = Self {
            file,
            textures,
            hashcodes,
            egui_textures: FnvHashMap::default(),
            identity_group_count: 0,
            identity_reused_texture_count: 0,
            start_time: Instant::now(),

            zoom: 1.0,
            filter_animated: false,

            enlarged_texture: None,
            enlarged_zoom: Self::ENLARGED_ZOOM_DEFAULT,

            fallback_texture: ctx.load_texture(
                "fallback",
                egui::ColorImage::from_rgba_unmultiplied([1, 1], &[0, 0, 0, 0]),
                egui::TextureOptions::default(),
            ),
        };

        s.load_textures(ctx);

        s
    }

    pub fn load_textures(&mut self, ctx: &egui::Context) {
        self.egui_textures.clear();
        self.identity_group_count = 0;
        self.identity_reused_texture_count = 0;
        let mut exact_group_handles: FnvHashMap<String, Vec<egui::TextureHandle>> =
            FnvHashMap::default();

        for it in &self.textures {
            if let Ok(t) = &it.data {
                let texture_identity =
                    eurochef_edb::robots_texture_identity::active_record(self.file, it.hashcode);
                if let Some(identity) = &texture_identity {
                    if let Some(handles) = exact_group_handles.get(&identity.exact_group) {
                        self.egui_textures.insert(it.hashcode, handles.clone());
                        self.identity_reused_texture_count += 1;
                        continue;
                    }
                }

                let frames: Vec<egui::TextureHandle> = t
                    .frames
                    .iter()
                    .map(|f| {
                        ctx.load_texture(
                            format_typed_hashcode_in_edb(
                                &self.hashcodes,
                                self.file,
                                "Texture",
                                it.hashcode,
                            ),
                            egui::ColorImage::from_rgba_unmultiplied(
                                [t.width as usize, t.height as usize],
                                f,
                            ),
                            egui::TextureOptions::default(),
                        )
                    })
                    .collect();

                if let Some(identity) = texture_identity {
                    exact_group_handles.insert(identity.exact_group, frames.clone());
                }
                self.egui_textures.insert(it.hashcode, frames);
            }
        }
        self.identity_group_count = exact_group_handles.len();
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Zoom: ");
            egui::Slider::new(&mut self.zoom, 0.25..=3.0)
                .show_value(true)
                .ui(ui);

            egui::Checkbox::new(&mut self.filter_animated, "Animated only").ui(ui);
        });
        if let Some(path) = eurochef_edb::robots_texture_identity::active_catalog_path() {
            ui.small(format!(
                "Texture identity: {} · exact groups in this EDB: {} · GPU uploads reused: {}",
                path.display(),
                self.identity_group_count,
                self.identity_reused_texture_count
            ));
        } else {
            ui.colored_label(
                Color32::YELLOW,
                "Texture identity catalog: not loaded; owner-local IDs remain ungrouped",
            );
        }

        ui.separator();

        egui::ScrollArea::vertical()
            .id_salt("section_scroll_area")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Match EntityListPanel: a fixed tile footprint keeps normal,
                // linked, and failed textures aligned in one predictable grid.
                let preview_size = egui::vec2(128., 128.) * self.zoom;
                let card_size = preview_size + egui::vec2(16., 54.);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = [4. * self.zoom; 2].into();
                    for (i, it) in self.textures.iter().enumerate() {
                        let resource_label =
                            format_typed_hashcode_with_id_in_edb(
                                &self.hashcodes,
                                self.file,
                                "Texture",
                                it.hashcode,
                            );
                        let texture_identity =
                            eurochef_edb::robots_texture_identity::active_record(
                                self.file,
                                it.hashcode,
                            );

                        // Skip null texture
                        if it.hashcode == 0x06000000 {
                            continue;
                        }
                        // The old branches filtered here; do it before the
                        // card closure so Rust can keep the loop control flow.
                        if self.filter_animated
                            && !matches!(
                                &it.data,
                                Ok(texture)
                                    if texture.external_texture.is_none() && texture.frame_count > 1
                            )
                        {
                            continue;
                        }

                        ui.allocate_ui_with_layout(
                            card_size,
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                egui::Frame::new()
                                    .fill(ui.visuals().widgets.noninteractive.weak_bg_fill)
                                    .stroke(egui::Stroke::new(
                                        1.0,
                                        ui.visuals().widgets.noninteractive.bg_stroke.color,
                                    ))
                                    .corner_radius(egui::CornerRadius::same(8))
                                    .inner_margin(egui::Margin::same(8))
                                    .show(ui, |ui| {
                        match (&it.data, it.data.as_ref().map(|d| d.external_texture).ok().flatten()) {
                            (Ok(t), None) => {
                                ui.vertical(|ui| {
                                    ui.set_width(preview_size.x);
                                    let time = self.start_time.elapsed().as_secs_f32();
                                    let frametime_scale =
                                        t.frame_count as f32 / t.frames.len() as f32;
                                    let frame_time = (1. / t.framerate as f32) * frametime_scale;

                                    let frames = &self.egui_textures[&it.hashcode];
                                    let current = if frames.is_empty() {
                                        &self.fallback_texture
                                    } else if frames.len() > 1 {
                                        &frames[(time / frame_time) as usize % frames.len()]
                                    } else {
                                        &frames[0]
                                    };

                                    let diagnostics = t.diagnostics.to_strings();

                                    let response = egui::Image::new((
                                        current.id(),
                                        preview_size,
                                    ))
                                    .sense(egui::Sense::click())
                                    .ui(ui)
                                    .on_hover_ui(|ui| {
                                        ui.label(format!(
                                            "{resource_label}\nHashcode: 0x{:08X}\nFormat (internal): 0x{:x}\nDimensions: {}x{}{}\nScroll: {} {}\nFlags: 0x{:x}\nGameflags: 0x{:x}\nIndex: {i}\n",
                                            it.hashcode, t.format_internal, t.width, t.height, if t.depth <= 1 { String::new() } else { format!("x{}", t.depth) }, t.scroll[0], t.scroll[1], t.flags, t.game_flags
                                        ));

                                        if let Some(identity) = &texture_identity {
                                            ui.separator();
                                            ui.strong(format!(
                                                "Exact identity: {}",
                                                identity.exact_group
                                            ));
                                            ui.label(format!(
                                                "Exact group size: {}\nAlias status: {}\nOwner EDB: {} [0x{:08X}]",
                                                identity.exact_group_size,
                                                identity.alias_status,
                                                identity.owner_edb_label,
                                                identity.owner_edb_uid
                                            ));
                                            if let Some(global_uid) = identity.recovered_global_uid {
                                                ui.label(format!(
                                                    "Recovered global Texture: {} [0x{global_uid:08X}]",
                                                    identity
                                                        .recovered_global_label
                                                        .as_deref()
                                                        .unwrap_or("HT_Texture_Unknown")
                                                ));
                                            } else {
                                                ui.label("Recovered global Texture: none (not guessed)");
                                            }
                                        }

                                        if frames.len() > 1 {
                                            ui.label(format!(
                                                "{} frames ({} fps)\n",
                                                frames.len(),
                                                t.framerate
                                            ));
                                        }

                                        for d in &diagnostics {
                                            ui.colored_label(Color32::YELLOW, *d);
                                        }

                                        if !diagnostics.is_empty() {
                                            ui.label("");
                                        }

                                        ui.strong("Click to enlarge");
                                    })
                                    .on_hover_cursor(egui::CursorIcon::PointingHand);

                                    if !diagnostics.is_empty() {
                                        ui.painter().text(
                                            response.rect.left_top() + egui::vec2(24., 24.),
                                            egui::Align2::CENTER_CENTER,
                                            font_awesome::EXCLAMATION_TRIANGLE,
                                            egui::FontId::proportional(24.),
                                            Color32::YELLOW,
                                        );
                                    }

                                    if response.clicked() {
                                        self.enlarged_texture = Some((i, it.hashcode));
                                    }

                                    ui.add(
                                        egui::Label::new(RichText::new(&resource_label).strong())
                                            .truncate()
                                            .show_tooltip_when_elided(true),
                                    );
                                });
                            }
                            (_, Some((ext_file, ext_texture))) => {
                                let external_identity =
                                    eurochef_edb::robots_texture_identity::active_record(
                                        ext_file,
                                        ext_texture,
                                    );

                                ui.vertical(|ui| {
                                    ui.set_width(preview_size.x);
                                    let (rect, response) = ui.allocate_exact_size(
                                        preview_size,
                                        egui::Sense::click(),
                                    );
                                    ui.painter().rect_filled(
                                        rect,
                                        egui::CornerRadius::ZERO,
                                        Color32::BLACK,
                                    );

                                    ui.painter().text(
                                        rect.left_top() + egui::vec2(24., 24.),
                                        egui::Align2::CENTER_CENTER,
                                        font_awesome::LINK,
                                        egui::FontId::proportional(24.),
                                        Color32::RED,
                                    );

                                    response.on_hover_ui(|ui| {
                                        ui.colored_label(
                                            Color32::LIGHT_RED,
                                            format!(
                                                "{resource_label} is a reference to {} in {}",
                                                format_typed_hashcode_with_id_in_edb(
                                                    &self.hashcodes,
                                                    ext_file,
                                                    "Texture",
                                                    ext_texture,
                                                ),
                                                format_hashcode_with_id(&self.hashcodes, ext_file)
                                            ),
                                        );
                                        if let Some(identity) = &external_identity {
                                            ui.separator();
                                            ui.label(format!(
                                                "Exact identity: {}\nExact group size: {}\nAlias status: {}",
                                                identity.exact_group,
                                                identity.exact_group_size,
                                                identity.alias_status
                                            ));
                                        }
                                    });
                                    ui.add(
                                        egui::Label::new(RichText::new(&resource_label).strong())
                                            .truncate()
                                            .show_tooltip_when_elided(true),
                                    );
                                });
                            }
                            (Err(e), _) => {
                                ui.vertical(|ui| {
                                    ui.set_width(preview_size.x);
                                    let (rect, response) = ui.allocate_exact_size(
                                        preview_size,
                                        egui::Sense::click(),
                                    );
                                    ui.painter().rect_filled(
                                        rect,
                                        egui::CornerRadius::ZERO,
                                        Color32::BLACK,
                                    );

                                    ui.painter().text(
                                        rect.left_top() + egui::vec2(24., 24.),
                                        egui::Align2::CENTER_CENTER,
                                        font_awesome::EXCLAMATION_TRIANGLE,
                                        egui::FontId::proportional(24.),
                                        Color32::RED,
                                    );

                                    response.on_hover_ui(|ui| {
                                        ui.label(format!("{resource_label} failed:"));
                                        ui.colored_label(
                                            Color32::LIGHT_RED,
                                            cutoff_string(
                                                strip_ansi_codes(&format!("{e:?}")),
                                                1024,
                                            ),
                                        );
                                    });
                                    ui.add(
                                        egui::Label::new(RichText::new(&resource_label).strong())
                                            .truncate()
                                            .show_tooltip_when_elided(true),
                                    );
                                });
                            },
                        }
                                    });
                            },
                        );
                    }
                });
            });
    }

    pub fn show_enlarged_window(&mut self, ctx: &egui::Context) {
        let mut window_open = self.enlarged_texture.is_some();
        if let Some(enlarged_texture) = self.enlarged_texture {
            let (i, _hashcode) = enlarged_texture;
            let it = &self.textures[i];
            let resource_label = format_typed_hashcode_with_id_in_edb(
                &self.hashcodes,
                self.file,
                "Texture",
                it.hashcode,
            );
            let texture_identity =
                eurochef_edb::robots_texture_identity::active_record(self.file, it.hashcode);

            if let Ok(t) = &it.data {
                // TODO(cohae): Fix resizing window
                egui::Window::new(format!("Texture Viewer · {resource_label}"))
                    .open(&mut window_open)
                    .collapsible(false)
                    .default_height(ctx.content_rect().height() * 0.70_f32)
                    .show(ctx, |ui| {
                        ui.strong(&resource_label);
                        if let Some(identity) = &texture_identity {
                            ui.label(format!(
                                "Exact identity: {} · copies: {} · status: {}",
                                identity.exact_group,
                                identity.exact_group_size,
                                identity.alias_status
                            ));
                            if let Some(global_uid) = identity.recovered_global_uid {
                                ui.label(format!(
                                    "Recovered global: {} [0x{global_uid:08X}]",
                                    identity
                                        .recovered_global_label
                                        .as_deref()
                                        .unwrap_or("HT_Texture_Unknown")
                                ));
                            }
                        }
                        let time = self.start_time.elapsed().as_secs_f32();
                        let frametime_scale = t.frame_count as f32 / t.frames.len() as f32;
                        let frame_time = (1. / t.framerate as f32) * frametime_scale;

                        let frames = &self.egui_textures[&it.hashcode];
                        let current = if !frames.is_empty() {
                            &frames[(time / frame_time) as usize % frames.len()]
                        } else {
                            &frames[0]
                        };

                        self.enlarged_zoom *= ctx.input(|i| i.zoom_delta());

                        egui::Image::new((current.id(), current.size_vec2() * self.enlarged_zoom))
                            .ui(ui);

                        // TODO(cohae): Animation checkbox, when unticked, show frame slider
                    });
            }
        }

        if !window_open {
            self.enlarged_texture = None;
            self.enlarged_zoom = Self::ENLARGED_ZOOM_DEFAULT; // swy: reset the zoom level each time we close a preview
        }
    }
}

pub fn cutoff_string(string: String, max_len: usize) -> String {
    if string.len() > max_len {
        let new_string = String::from_utf8_lossy(&string.as_bytes()[..max_len]).to_string();
        new_string + "..."
    } else {
        string
    }
}
