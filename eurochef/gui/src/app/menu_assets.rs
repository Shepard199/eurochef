//! Windows Robots (2005) frontend textures, decoded directly from the
//! original h04_fron.edb with the existing EuroChef EngineX texture parser.
//! No screenshots or third-party artwork are embedded. The front-end's real
//! HudScript composition/animation is a separate reverse frontier.
use std::{fs::File, io::{BufReader, Read, Seek, SeekFrom}, path::Path};
use anyhow::{Context, Result};
use eurochef_edb::{edb::EdbFile, versions::Platform};
use eurochef_shared::textures::UXGeoTexture;
use super::frontend_script::{FrontendScriptCatalog, FrontendSceneFrame};

pub(super) const ROBOTS_FRONTEND_EDB_UID: u32 = 0x0100_000a;
pub(super) const ROBOTS_TITLE_LOGO: u32 = 0x8600_0035;
pub(super) const ROBOTS_FRAME_BOTTOM: u32 = 0x8600_003e;
pub(super) const ROBOTS_FRAME_TOP: u32 = 0x8600_003f;
pub(super) const ROBOTS_SELECTION: u32 = 0x8600_001e;
pub(super) const ROBOTS_BACKGROUND_STILL: u32 = 0x8600_001a;
const ROBOTS_FONT_EDB_UID: u32 = 0x0100_0042;
const ROBOTS_FONT_ATLAS_UID: u32 = 0x8600_0002;

#[derive(Clone, Debug)]
struct OriginalFontEntry {
    uid: u32,
    section: u16,
    address: u32,
    raw_prefix: Option<Vec<u8>>,
}

/// Native two-level glyph lookup from Robots.exe 0x00430620.
/// The shipped H07 corpus confirms font_list[].address as its serialized
/// table base (HT_Font_Main, Timer and Subtitles match real glyph records).
fn native_font_glyph_offset(data: &[u8], table_base: usize, character: u16) -> Option<usize> {
    fn rel(data: &[u8], pointer: usize) -> Option<usize> {
        let bytes: [u8; 4] = data.get(pointer..pointer.checked_add(4)?)?.try_into().ok()?;
        let delta = i32::from_le_bytes(bytes);
        if delta == 0 { return None; }
        let target = (pointer as i64).checked_add(i64::from(delta))?;
        let target = usize::try_from(target).ok()?;
        (target < data.len()).then_some(target)
    }
    let high = usize::from(character >> 8);
    let low = usize::from(character & 0xff);
    let first_ptr = table_base.checked_add(4)?.checked_add(high.checked_mul(4)?)?;
    let page = rel(data, first_ptr)?;
    let second_ptr = page.checked_add(low.checked_mul(4)?)?;
    let glyph = rel(data, second_ptr)?;
    (glyph.checked_add(0x16)? <= data.len()).then_some(glyph)
}

/// Four native float32 UV bounds copied to the rendered text element by
/// Robots.exe 0x004314F5..0x0043150A. Confirmed against shipped H07 glyphs
/// and atlas dimensions; draw size comes from the following two u16 fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NativeGlyphRenderRecord {
    render_words: [u32; 4],
    metrics: NativeGlyphMetrics,
}

fn native_font_glyph_render_record(data: &[u8], glyph: usize) -> Option<NativeGlyphRenderRecord> {
    let mut render_words = [0u32; 4];
    for (i, word) in render_words.iter_mut().enumerate() {
        let start = glyph.checked_add(i.checked_mul(4)?)?;
        *word = u32::from_le_bytes(data.get(start..start.checked_add(4)?)?.try_into().ok()?);
    }
    Some(NativeGlyphRenderRecord {
        render_words,
        metrics: native_font_glyph_metrics(data, glyph)?,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NativeGlyphMetrics {
    width: u16,
    height: u16,
}

fn native_font_glyph_metrics(data: &[u8], glyph: usize) -> Option<NativeGlyphMetrics> {
    let word = |offset: usize| -> Option<u16> {
        let start = glyph.checked_add(offset)?;
        Some(u16::from_le_bytes(data.get(start..start.checked_add(2)?)?.try_into().ok()?))
    };
    let right = word(0x10)?;
    let vertical = word(0x12)?;
    let left = word(0x14)?;
    Some(NativeGlyphMetrics {
        width: right.checked_sub(left)?,
        height: vertical.checked_sub(3)?,
    })
}

#[derive(Debug, Clone, Copy)]
struct NativeGlyphQuad {
    uv: egui::Rect,
    pixels: egui::Vec2,
    advance: f32,
}

struct NativeBitmapFont {
    bytes: Vec<u8>,
    table_base: usize,
}

impl NativeBitmapFont {
    fn glyph(&self, character: char) -> Option<NativeGlyphQuad> {
        let code = u16::try_from(u32::from(character)).ok()?;
        let offset = native_font_glyph_offset(&self.bytes, self.table_base, code)?;
        let record = native_font_glyph_render_record(&self.bytes, offset)?;
        let [u0, v0, u1, v1] = record.render_words.map(f32::from_bits);
        if ![u0, v0, u1, v1].iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v))
            || u0 >= u1 || v0 >= v1
        {
            return None;
        }
        let read_u16 = |rel: usize| -> Option<u16> {
            let start = offset.checked_add(rel)?;
            Some(u16::from_le_bytes(
                self.bytes.get(start..start.checked_add(2)?)?.try_into().ok()?
            ))
        };
        let width = f32::from(read_u16(0x10)?);
        let height = f32::from(read_u16(0x12)?);
        if width == 0.0 || height == 0.0 { return None; }
        Some(NativeGlyphQuad {
            uv: egui::Rect::from_min_max(egui::pos2(u0, v0), egui::pos2(u1, v1)),
            pixels: egui::vec2(width, height),
            advance: f32::from(record.metrics.width),
        })
    }

    fn layout_width(&self, label: &str, scale: f32) -> Option<f32> {
        label.chars().try_fold(0.0, |sum, ch| Some(sum + self.glyph(ch)?.advance * scale))
    }
}

/// Bind the exact texture referenced by HT_Font_Main's serialized index.
/// In the shipped H07 corpus that index is 2 -> 0x86000002.
fn read_original_main_font(path: &Path) -> Result<NativeBitmapFont> {
    let file = File::open(path)?;
    let edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)?;
    anyhow::ensure!(edb.header.hashcode == ROBOTS_FONT_EDB_UID, "Unexpected H07 EDB UID");
    let entry = edb.header.font_list.iter()
        .find(|entry| entry.hashcode == 0x0700_0001)
        .context("HT_Font_Main missing")?;
    anyhow::ensure!(entry.section == 0, "Unsupported EXGeoFont section");
    let bytes = std::fs::read(path)?;
    let table_base = usize::try_from(entry.address)?;
    let selector_bytes: [u8; 4] = bytes
        .get(table_base..table_base.checked_add(4).context("Font address overflow")?)
        .context("EXGeoFont address outside file")?.try_into()?;
    let selector = u32::from_le_bytes(selector_bytes) as usize;
    let referenced = edb.header.texture_list.data().get(selector)
        .context("EXGeoFont texture index outside EDB")?;
    anyhow::ensure!(referenced.common.hashcode == ROBOTS_FONT_ATLAS_UID,
        "HT_Font_Main references a different atlas");
    let font = NativeBitmapFont { bytes, table_base };
    anyhow::ensure!(font.glyph('A').is_some() && font.glyph('0').is_some(),
        "Font glyph lookup or UV data invalid");
    Ok(font)
}

/// A bounded candidate file offset, not a claim that EXGeoFont uses absolute
/// addressing. Rejects zero, overflow and truncated records before seeking.
fn font_raw_prefix_range(address: u32, file_len: u64) -> Option<std::ops::Range<u64>> {
    let start = u64::from(address);
    let end = start.checked_add(32)?;
    (start != 0 && end <= file_len).then_some(start..end)
}

/// Read actual EDB font directory identity and offsets, not guessed glyph data.
fn read_original_font_directory(path: &Path) -> Result<Vec<OriginalFontEntry>> {
    let file = File::open(path)?;
    let edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)?;
    anyhow::ensure!(edb.header.hashcode == ROBOTS_FONT_EDB_UID, "Unexpected font EDB UID");
    let mut source = File::open(path)?;
    let file_len = source.metadata()?.len();
    let mut entries = Vec::new();
    for entry in edb.header.font_list.iter() {
        // Do not assign meaning to the section or address before the native
        // lookup path is recovered. Only retain bounded diagnostic bytes.
        let raw_prefix = if let Some(range) = font_raw_prefix_range(entry.address, file_len) {
            let mut data = vec![0u8; (range.end - range.start) as usize];
            source.seek(SeekFrom::Start(range.start))?;
            source.read_exact(&mut data)?;
            Some(data)
        } else { None };
        entries.push(OriginalFontEntry {
            uid: entry.hashcode, section: entry.section,
            address: entry.address, raw_prefix,
        });
    }
    Ok(entries)
}

/// Decode the original atlas without inventing unsupported character UVs or advances.
/// Glyph layout remains a separate EXGeoFont reverse-engineering task.
fn read_original_font_atlas(path: &Path) -> Result<egui::ColorImage> {
    let file = File::open(path).with_context(|| format!("opening Robots font {}", path.display()))?;
    let mut edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)?;
    anyhow::ensure!(edb.header.hashcode == ROBOTS_FONT_EDB_UID,
        "Expected font EDB {ROBOTS_FONT_EDB_UID:#010x}, got {:#010x}", edb.header.hashcode);
    // The EDB header has a distinct EXGeoFont directory. A decoded texture
    // alone is insufficient evidence of glyph IDs, kerning or advances.
    anyhow::ensure!(edb.header.font_list.len() > 0,
        "Original font EDB has no EXGeoFont directory entries");
    let textures = UXGeoTexture::read_hashcodes(&mut edb, &[ROBOTS_FONT_ATLAS_UID]);
    let (_, texture) = textures.into_iter().find(|(_, t)| t.hashcode == ROBOTS_FONT_ATLAS_UID)
        .context("Original font atlas is absent")?;
    let decoded = texture.data.context("Decoding original font atlas")?;
    anyhow::ensure!((decoded.width, decoded.height) == (256, 256),
        "Unexpected original font atlas dimensions");
    let rgba = decoded.frames.first().context("Font atlas contains no frames")?;
    anyhow::ensure!(rgba.len() == 256 * 256 * 4, "Invalid font atlas RGBA length");
    Ok(egui::ColorImage::from_rgba_unmultiplied([256, 256], rgba))
}

pub(super) struct RobotsMenuAssets {
    logo: egui::TextureHandle,
    frame_top: egui::TextureHandle,
    frame_bottom: egui::TextureHandle,
    selection: egui::TextureHandle,
    background_still: egui::TextureHandle,
    font_atlas: Option<egui::TextureHandle>,
    main_font: Option<NativeBitmapFont>,
    font_directory: Vec<OriginalFontEntry>,
    scripts: FrontendScriptCatalog,
}

impl RobotsMenuAssets {
    pub(super) fn from_original_edb(ctx: &egui::Context, path: &Path) -> Result<Self> {
        let source = File::open(path).with_context(|| format!("opening Robots frontend {}", path.display()))?;
        let mut edb = EdbFile::new(Box::new(BufReader::new(source)), Platform::Pc)
            .context("parsing Robots frontend EDB")?;
        anyhow::ensure!(
            edb.header.hashcode == ROBOTS_FRONTEND_EDB_UID,
            "Expected h04_fron EDB 0x{ROBOTS_FRONTEND_EDB_UID:08X}, found 0x{:08X}",
            edb.header.hashcode,
        );
        let scripts = FrontendScriptCatalog::from_original_edb(&mut edb)?;
        // Font data is optional for installs without H07. Only bind the
        // original UI glyphs when its native selector matches the decoded atlas.
        let font_path = path.with_file_name("h07_font.edb");
        let font_directory = read_original_font_directory(&font_path).unwrap_or_default();
        let main_font = read_original_main_font(&font_path).ok();
        let font_atlas = read_original_font_atlas(&font_path).ok().map(|image|
            // Keep the shipped bitmap/UVs, but filter sampling when HUD scales
            // above the original 640-point frontend stage on HiDPI monitors.
            ctx.load_texture("robots-h07-font-atlas", image, egui::TextureOptions::LINEAR));
        let mut textures = UXGeoTexture::read_hashcodes(
            &mut edb,
            &[ROBOTS_TITLE_LOGO, ROBOTS_FRAME_BOTTOM, ROBOTS_FRAME_TOP,
              ROBOTS_SELECTION, ROBOTS_BACKGROUND_STILL],
        );
        let mut load = |uid: u32| -> Result<egui::TextureHandle> {
            let pos = textures.iter().position(|(_, tex)| tex.hashcode == uid)
                .with_context(|| format!("Frontend texture {uid:#010x} absent"))?;
            let (_, result) = textures.swap_remove(pos);
            let decoded = result.data.with_context(|| format!("Decoding frontend texture {uid:#010x}"))?;
            let bytes = decoded.frames.first().context("Texture has no decoded RGBA frame")?;
            anyhow::ensure!(bytes.len() == decoded.width as usize * decoded.height as usize * 4,
                "Unexpected frontend texture buffer size for {uid:#010x}");
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [decoded.width as usize, decoded.height as usize], bytes,
            );
            Ok(ctx.load_texture(
                format!("robots-h04-{uid:#010x}"),
                image,
                egui::TextureOptions::LINEAR,
            ))
        };
        Ok(Self {
            logo: load(ROBOTS_TITLE_LOGO)?,
            frame_top: load(ROBOTS_FRAME_TOP)?,
            frame_bottom: load(ROBOTS_FRAME_BOTTOM)?,
            selection: load(ROBOTS_SELECTION)?,
            background_still: load(ROBOTS_BACKGROUND_STILL)?,
            font_atlas,
            main_font,
            font_directory,
            scripts,
        })
    }

    pub(super) fn entity_poses(&self, uid: u32, elapsed_seconds: f64)
        -> Vec<super::frontend_script::FrontendEntityPose> {
        self.scripts.entity_poses(uid, elapsed_seconds)
    }

    pub(super) fn sample_script(&self, uid: u32, elapsed_seconds: f64) -> FrontendSceneFrame {
        self.scripts.sample(uid, elapsed_seconds)
    }

    pub(super) fn marker_evidence(&self, script_uid: u32)
        -> Vec<super::frontend_hud_dispatch::HudMarkerDescriptor> {
        self.scripts.marker_evidence(script_uid)
    }

    pub(super) fn entrance_progress(&self, uid: u32, elapsed_seconds: f64) -> Option<f32> {
        self.scripts.entrance_progress(uid, elapsed_seconds)
    }

    pub(super) fn main_menu_chrome_motion(&self, uid: u32, elapsed_seconds: f64) -> Option<f32> {
        self.scripts.main_menu_chrome_motion(uid, elapsed_seconds)
    }

    pub(super) fn events_started_between(&self, uid: u32, previous: f64, current: f64)
        -> Vec<super::frontend_script::FrontendScheduledEvent> {
        self.scripts.events_started_between(uid, previous, current)
    }

    pub(super) fn has_script(&self, uid: u32) -> bool {
        self.scripts.contains(uid)
    }

    pub(super) fn can_paint_original_label(&self, label: &str) -> bool {
        self.font_atlas.is_some() && self.main_font.as_ref()
            .is_some_and(|font| font.layout_width(label, 1.0).is_some())
    }

    /// Draw genuine H07 glyphs with their serialized normalized UV bounds.
    /// Button positioning remains an approximate host layout, not a claim
    /// of pixel-exact FE_HudScript transform parity.
    pub(super) fn paint_original_label(
        &self, ui: &egui::Ui, rect: egui::Rect, label: &str,
        text_height: f32, tint: egui::Color32,
    ) -> bool {
        let (Some(font), Some(atlas)) = (&self.main_font, &self.font_atlas) else {
            return false;
        };
        let Some(reference) = font.glyph('A') else { return false; };
        if reference.pixels.y <= 0.0 || text_height <= 0.0 { return false; }
        let scale = text_height / reference.pixels.y;
        let Some(width) = font.layout_width(label, scale) else { return false; };
        let mut x = rect.center().x - width * 0.5;
        let y = rect.center().y - text_height * 0.5;
        for ch in label.chars() {
            // Validated above, so no partial text can be submitted.
            let glyph = font.glyph(ch).expect("validated original glyph");
            let glyph_rect = egui::Rect::from_min_size(
                egui::pos2(x, y), glyph.pixels * scale,
            );
            ui.painter().image(atlas.id(), glyph_rect, glyph.uv, tint);
            x += glyph.advance * scale;
        }
        true
    }

    pub(super) fn show_font_atlas_diagnostic(&self, ui: &mut egui::Ui) {
        ui.label(format!("EXGeoFont directory: {} entries", self.font_directory.len()));
        for entry in &self.font_directory {
            ui.monospace(format!("uid={:#010x} section={} address={:#010x}",
                entry.uid, entry.section, entry.address));
            if let Some(bytes) = &entry.raw_prefix {
                ui.small(format!("raw at address (uninterpreted): {}",
                    bytes.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")));
            }
        }
        if let Some(atlas) = &self.font_atlas {
            ui.label(if self.main_font.is_some() {
                "Original H07 atlas · HT_Font_Main glyph UVs active in Game Mode"
            } else {
                "Original H07 atlas · glyph binding unavailable"
            });
            ui.image((atlas.id(), egui::vec2(256.0, 256.0)));
        } else {
            ui.label("h07_font.edb unavailable or font atlas could not be decoded");
        }
    }

    /// Draw source art as a backdrop on any monitor without distorting its
    /// aspect or exposing a square edge at widescreen viewports. Cropping is
    /// centered and only changes UVs, not the original decoded RGBA pixels.
    fn cover_uv(image_size: egui::Vec2, bounds: egui::Vec2) -> egui::Rect {
        if image_size.x <= 0.0 || image_size.y <= 0.0 ||
            bounds.x <= 0.0 || bounds.y <= 0.0 {
            return egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0));
        }
        let image_aspect = image_size.x / image_size.y;
        let bounds_aspect = bounds.x / bounds.y;
        if bounds_aspect > image_aspect {
            let fraction = (image_aspect / bounds_aspect).clamp(0.0, 1.0);
            let pad = (1.0 - fraction) * 0.5;
            egui::Rect::from_min_max(egui::pos2(0.0, pad),
                egui::pos2(1.0, 1.0 - pad))
        } else {
            let fraction = (bounds_aspect / image_aspect).clamp(0.0, 1.0);
            let pad = (1.0 - fraction) * 0.5;
            egui::Rect::from_min_max(egui::pos2(pad, 0.0),
                egui::pos2(1.0 - pad, 1.0))
        }
    }

    fn paint_background_cover(&self, painter: &egui::Painter,
        outer: egui::Rect, tint: egui::Color32) {
        let [w, h] = self.background_still.size();
        let uv = Self::cover_uv(egui::vec2(w as f32, h as f32), outer.size());
        painter.image(self.background_still.id(), outer, uv, tint);
    }

    fn sprite(
        painter: &egui::Painter,
        texture: &egui::TextureHandle,
        rect: egui::Rect,
        mirrored: bool,
        tint: egui::Color32,
    ) {
        let uv = if mirrored {
            egui::Rect::from_min_max(egui::pos2(1.0, 0.0), egui::pos2(0.0, 1.0))
        } else {
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0))
        };
        painter.image(texture.id(), rect, uv, tint);
    }

    /// Temporary host chrome stage until native HUD matrices are recovered.
    /// Use the *same* stage bounds for painted H04 chrome and the interactive
    /// button hitboxes. Keep it contained within the viewport at any DPI.
    pub(super) fn frame_canvas(outer: egui::Rect) -> egui::Rect {
        let side = (outer.width().min(outer.height()) * 0.82).max(1.0);
        egui::Rect::from_center_size(outer.center(), egui::vec2(side, side))
    }

    /// The keyframed world-space Y motion is real; the conversion to
    /// temporary 2D chrome-stage pixels is not the recovered HUD projection.
    pub(super) fn chrome_motion_pixels(outer: egui::Rect, normalized: f32) -> f32 {
        normalized * Self::frame_canvas(outer).height() * 0.5
    }

    pub(super) fn main_menu_button_top(outer: egui::Rect) -> f32 {
        let canvas = Self::frame_canvas(outer);
        canvas.min.y + canvas.height() * 0.295
    }

    pub(super) fn paint_original_title(&self, ui: &egui::Ui, entrance: f32, scene3d: bool) {
        let outer = ui.max_rect();
        let side = outer.width().min(outer.height()).max(1.0);
        let canvas = egui::Rect::from_center_size(outer.center(), egui::vec2(side, side));
        if !scene3d {
            // The selected 3D camera is currently only a geometry viewer.
            // Keep the Title Screen clean instead of compositing a stray M00
            // map chunk through the original transparent logo.
            ui.painter().rect_filled(outer, 0.0, egui::Color32::from_rgb(14, 15, 17));
        } else {
            ui.painter().rect_filled(outer, 0.0, egui::Color32::from_black_alpha(75));
        }
        let size = egui::vec2(side * 0.78, side * 0.39);
        let phase = entrance.clamp(0.0, 1.0);
        // Provisional image transform paced by the shipped TitleScreenRoot
        // Entity activation frames, NOT the original controller matrix.
        let logo = egui::Rect::from_center_size(
            canvas.center() - egui::vec2(0.0, side * 0.12 - side * 0.08 * (1.0 - phase)), size);
        Self::sprite(ui.painter(), &self.logo, logo, false,
            egui::Color32::from_white_alpha((255.0 * phase).round() as u8));
    }

    pub(super) fn paint_original_frame(&self, ui: &egui::Ui, entrance: f32, scene3d: bool,
        native_chrome_motion: Option<f32>) {
        let outer = ui.max_rect();
        let canvas = Self::frame_canvas(outer);
        let side = canvas.height();
        let painter = ui.painter();
        if !scene3d {
            painter.rect_filled(outer, 0.0, egui::Color32::from_rgb(17, 19, 21));
            // Virtual 640x640 HUD stage is independent of the fullscreen
            // backdrop. Render the latter as a cover image to avoid the
            // square edges visible on widescreen Windows monitors.
            self.paint_background_cover(painter, outer,
                egui::Color32::from_white_alpha(95));
        }
        let half = side * 0.5;
        let halfsize = egui::vec2(half, half);
        let phase = entrance.clamp(0.0, 1.0);
        // On MainMenu_PC follow original controller vector_0 Y keyframe
        // shapes (0,16,36,39,54), including their overshoot and settling.
        // World-to-screen scaling is provisional: half of the HUD stage.
        let y_offset = Self::chrome_motion_pixels(outer, native_chrome_motion.unwrap_or(0.0));
        let shift = if native_chrome_motion.is_some() { 0.0 }
            else { side * 0.06 * (1.0 - phase) };
        let tint = if native_chrome_motion.is_some() { egui::Color32::WHITE }
            else { egui::Color32::from_white_alpha((255.0 * phase).round() as u8) };
        // The F5 diagnostic translation is not a proven Entity-to-sprite mapping.
        // until actual HUD mesh projection is recovered, pixels/unit is host-calibrated.
        Self::sprite(painter, &self.frame_top,
            egui::Rect::from_min_size(canvas.min + egui::vec2(-shift, y_offset), halfsize),
            false, tint);
        Self::sprite(painter, &self.frame_top,
            egui::Rect::from_min_size(canvas.min + egui::vec2(half + shift, y_offset), halfsize),
            true, tint);
        Self::sprite(painter, &self.frame_bottom,
            egui::Rect::from_min_size(canvas.min + egui::vec2(-shift, half + y_offset), halfsize),
            false, tint);
        Self::sprite(painter, &self.frame_bottom,
            egui::Rect::from_min_size(canvas.min + egui::vec2(half + shift, half + y_offset), halfsize),
            true, tint);
    }

    pub(super) fn paint_selection(&self, ui: &egui::Ui, rect: egui::Rect) {
        let painter = ui.painter();
        painter.image(
            self.selection.id(), rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            egui::Color32::from_white_alpha(160),
        );
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shipped_windows_frontend_decodes_real_art_when_corpus_present() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let source = root.join(
            "../../target/attachment_game_root/_eurotools_out/extracted_main/robots/binary/_bin_pc/h04_fron.edb"
        );
        if !source.is_file() {
            // The tests run without redistributing copyrighted game data.
            return;
        }
        let ctx = egui::Context::default();
        let art = RobotsMenuAssets::from_original_edb(&ctx, &source).unwrap();
        assert_eq!(art.frame_top.size(), [256, 256]);
        assert_eq!(art.frame_bottom.size(), [256, 256]);
        assert_eq!(art.logo.size(), [512, 256]);
        assert_eq!(art.selection.size(), [64, 32]);
        if source.with_file_name("h07_font.edb").is_file() {
            assert!(art.can_paint_original_label("New Game"));
            assert!(art.can_paint_original_label("Bonus Features"));
            let mut painted = false;
            let frame = ctx.run_ui(egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO, egui::vec2(500.0, 300.0))),
                ..Default::default()
            }, |ui| {
                let rect = egui::Rect::from_min_size(
                    egui::pos2(100.0, 100.0), egui::vec2(250.0, 28.0));
                painted = art.paint_original_label(ui, rect, "New Game", 20.0, egui::Color32::WHITE);
            });
            assert!(painted);
            let atlas_id = art.font_atlas.as_ref().unwrap().id();
            assert!(frame.shapes.iter().any(|shape| matches!(
                &shape.shape, egui::epaint::Shape::Mesh(mesh) if mesh.texture_id == atlas_id
            )), "Original H07 glyphs must submit textured geometry");
        }
    }

    #[test]
    fn h04_backdrop_cover_uv_preserves_aspect_at_widescreen_and_tall_sizes() {
        let square = egui::vec2(512.0, 512.0);
        let wide = RobotsMenuAssets::cover_uv(square, egui::vec2(1920.0, 1080.0));
        assert_eq!(wide.min.x, 0.0);
        assert_eq!(wide.max.x, 1.0);
        assert!((wide.min.y - 0.21875).abs() < 1e-6);
        assert!((wide.max.y - 0.78125).abs() < 1e-6);
        let portrait = RobotsMenuAssets::cover_uv(square, egui::vec2(540.0, 960.0));
        assert_eq!(portrait.min.y, 0.0);
        assert_eq!(portrait.max.y, 1.0);
        assert!((portrait.min.x - 0.21875).abs() < 1e-6);
        assert!((portrait.max.x - 0.78125).abs() < 1e-6);
        let square_uv = RobotsMenuAssets::cover_uv(square, square);
        assert_eq!(square_uv.min, egui::Pos2::ZERO);
        assert_eq!(square_uv.max, egui::pos2(1.0, 1.0));
        let invalid_uv = RobotsMenuAssets::cover_uv(egui::Vec2::ZERO, egui::vec2(1920.0, 1080.0));
        assert_eq!(invalid_uv, square_uv);
    }

    #[test]
    fn native_chrome_curve_keeps_text_and_panels_in_same_stage() {
        for size in [egui::vec2(960.0, 540.0), egui::vec2(1920.0, 1080.0),
            egui::vec2(3840.0, 2160.0)] {
            let outer = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
            let rest = RobotsMenuAssets::main_menu_button_top(outer);
            let offset0 = RobotsMenuAssets::chrome_motion_pixels(outer, 1.0);
            let overshoot = RobotsMenuAssets::chrome_motion_pixels(outer, -0.46);
            assert_eq!(offset0, RobotsMenuAssets::frame_canvas(outer).height() * 0.5);
            assert!(rest + overshoot > outer.top());
            assert!(rest + offset0 < outer.bottom());
            assert_eq!(RobotsMenuAssets::chrome_motion_pixels(outer, 0.0), 0.0);
        }
    }

    #[test]
    fn chrome_and_button_stage_share_bounded_viewport_coordinates() {
        for dims in [egui::vec2(960.0, 540.0), egui::vec2(1280.0, 1024.0),
                     egui::vec2(1920.0, 1080.0), egui::vec2(3840.0, 2160.0)] {
            let outer = egui::Rect::from_min_size(egui::Pos2::ZERO, dims);
            let stage = RobotsMenuAssets::frame_canvas(outer);
            let top = RobotsMenuAssets::main_menu_button_top(outer);
            assert!(outer.contains(stage.min) && outer.contains(stage.max));
            assert!(top > stage.min.y && top < stage.center().y);
            let scale = (dims.x.min(dims.y) / 900.0).clamp(0.65, 1.7);
            let five_rows = 5.0 * (21.5 + 0.6) * scale;
            assert!(top + five_rows < stage.min.y + stage.height() * 0.50,
                "the five original commands must not cross the chrome's central teeth");
        }
    }

    #[test]
    fn shipped_h04_sprite_alpha_is_preserved_without_fake_black_mattes() {
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/attachment_game_root/_eurotools_out/extracted_main/robots/binary/_bin_pc/h04_fron.edb");
        if !source.is_file() { return; }
        let mut edb = EdbFile::new(Box::new(BufReader::new(File::open(source).unwrap())), Platform::Pc).unwrap();
        let decoded = UXGeoTexture::read_hashcodes(&mut edb, &[
            ROBOTS_TITLE_LOGO, ROBOTS_FRAME_TOP, ROBOTS_FRAME_BOTTOM, ROBOTS_SELECTION,
            ROBOTS_BACKGROUND_STILL,
        ]);
        for (_, result) in decoded {
            let uid = result.hashcode;
            let Ok(texture) = result.data else { continue; };
            let Some(bytes) = texture.frames.first() else { continue; };
            let n = bytes.len() / 4;
            let alpha_zero = bytes.chunks_exact(4).filter(|px| px[3] == 0).count();
            let alpha_partial = bytes.chunks_exact(4).filter(|px| px[3] > 0 && px[3] < 255).count();
            let corner = [0, (texture.width as usize).saturating_sub(1),
                (texture.height as usize - 1) * texture.width as usize,
                n.saturating_sub(1)];
            let corners: Vec<_> = corner.iter().map(|&pos| &bytes[pos*4..pos*4+4]).collect();
            if matches!(uid, ROBOTS_TITLE_LOGO | ROBOTS_FRAME_TOP | ROBOTS_FRAME_BOTTOM) {
                assert!(alpha_zero > n / 4, "shipped chrome has substantial real transparency");
                assert!(corners.iter().all(|rgba| rgba[3] == 0),
                    "original H04 chrome texture must have transparent corners");
            } else if uid == ROBOTS_BACKGROUND_STILL {
                assert_eq!(alpha_zero, 0, "the factory fallback is an opaque still");
                assert_eq!(alpha_partial, 0);
            } else if uid == ROBOTS_SELECTION {
                assert!(alpha_zero > 0 && alpha_partial > 0);
            }
        }
    }

    #[test]
    fn native_glyph_two_level_relative_lookup_is_bounded() {
        let mut bytes = vec![0u8; 1024];
        // table_base=16, first-level pointer at 20 (page 0), target=128.
        bytes[20..24].copy_from_slice(&108i32.to_le_bytes());
        // ASCII 'A'=0x41: second-level pointer at 128 + 65*4 = 388.
        bytes[388..392].copy_from_slice(&124i32.to_le_bytes());
        assert_eq!(native_font_glyph_offset(&bytes, 16, 0x0041), Some(512));
        assert_eq!(native_font_glyph_offset(&bytes, 16, 0x0042), None);
        assert_eq!(native_font_glyph_offset(&bytes, 16, 0x0141), None);
        assert_eq!(native_font_glyph_offset(&bytes, usize::MAX, 0x0041), None);
        bytes[388..392].copy_from_slice(&i32::MAX.to_le_bytes());
        assert_eq!(native_font_glyph_offset(&bytes, 16, 0x0041), None);
    }

    #[test]
    fn native_glyph_render_record_preserves_opaque_sprite_data() {
        let mut data = vec![0u8; 64];
        for (index, word) in [0x11223344u32, 0x55667788, 0x99aabbcc, 0xddeeff00].iter().enumerate() {
            let start = 8 + index * 4;
            data[start..start + 4].copy_from_slice(&word.to_le_bytes());
        }
        data[24..26].copy_from_slice(&14u16.to_le_bytes());
        data[26..28].copy_from_slice(&21u16.to_le_bytes());
        data[28..30].copy_from_slice(&4u16.to_le_bytes());
        let glyph = native_font_glyph_render_record(&data, 8).unwrap();
        assert_eq!(glyph.render_words, [0x11223344, 0x55667788, 0x99aabbcc, 0xddeeff00]);
        assert_eq!(glyph.metrics, NativeGlyphMetrics { width: 10, height: 18 });
        assert_eq!(native_font_glyph_render_record(&data, 49), None);
    }

    #[test]
    fn native_glyph_metrics_follow_shipped_layout_and_fail_closed() {
        let mut data = vec![0u8; 64];
        data[32..34].copy_from_slice(&17u16.to_le_bytes());
        data[34..36].copy_from_slice(&23u16.to_le_bytes());
        data[36..38].copy_from_slice(&5u16.to_le_bytes());
        assert_eq!(native_font_glyph_metrics(&data, 16), Some(NativeGlyphMetrics { width: 12, height: 20 }));
        assert_eq!(native_font_glyph_metrics(&data, 48), None);
        data[36..38].copy_from_slice(&18u16.to_le_bytes());
        assert_eq!(native_font_glyph_metrics(&data, 16), None);
        data[36..38].copy_from_slice(&5u16.to_le_bytes());
        data[34..36].copy_from_slice(&2u16.to_le_bytes());
        assert_eq!(native_font_glyph_metrics(&data, 16), None);
    }

    #[test]
    fn font_offset_probe_rejects_missing_or_truncated_data() {
        assert_eq!(font_raw_prefix_range(0, 1024), None);
        assert_eq!(font_raw_prefix_range(100, 131), None);
        assert_eq!(font_raw_prefix_range(100, 132), Some(100..132));
        assert_eq!(font_raw_prefix_range(u32::MAX, 1024), None);
    }

    #[test]
    fn shipped_windows_font_atlas_decodes_when_corpus_present() {
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../target/attachment_game_root/_eurotools_out/extracted_main/robots/binary/_bin_pc/h07_font.edb"
        );
        if !source.is_file() { return; }
        let directory = read_original_font_directory(&source).unwrap();
        assert!(!directory.is_empty());
        assert!(directory.iter().any(|entry| entry.uid == 0x0700_0001));
        // All shipped PC font entries resolve to bounded in-file offsets.
        // This establishes safe access, not the semantics of the fields.
        assert!(directory.iter().all(|entry| entry.section == 0 && entry.raw_prefix.is_some()));
        assert!(directory.iter().all(|entry| entry.raw_prefix.as_ref().unwrap().len() == 32));
        let atlas = read_original_font_atlas(&source).unwrap();
        assert_eq!(atlas.size, [256, 256]);
        let font = read_original_main_font(&source).unwrap();
        let a = font.glyph('A').unwrap();
        assert_eq!(a.pixels, egui::vec2(16.0, 28.0));
        assert!((a.uv.min.x - 144.0 / 255.0).abs() < 0.0001);
        assert!((a.uv.max.y - 56.0 / 255.0).abs() < 0.0001);
        assert!(font.layout_width("New Game", 1.0).is_some());
    }

    #[test]
    fn original_frontend_uses_confirmed_h04_texture_uids() {
        assert_eq!(ROBOTS_FRONTEND_EDB_UID, 0x0100_000a);
        assert_eq!(ROBOTS_TITLE_LOGO, 0x8600_0035);
        assert_eq!(ROBOTS_FRAME_BOTTOM, 0x8600_003e);
        assert_eq!(ROBOTS_FRAME_TOP, 0x8600_003f);
    }
}
