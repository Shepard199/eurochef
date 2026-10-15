//! The original-style game shell uses the existing native runtime. Game menu,
//! save slots and transitions are NOT a second editor or a second AI simulator.
//! Original screen assets/flow are still reverse frontiers; use a clearly
//! labelled functional front-end rather than inventing game-accurate artwork.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum GamePresentationMode {
    #[default]
    Editor,
    TitleScreen,
    MainMenu,
    NewGameSlots,
    ConfirmOverwrite,
    Launching,
    Running,
    Paused,
    SaveSlots,
    LoadSlots,
    Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GameMenuAction {
    EnterMainMenu,
    NewGame,
    OpenNewGameSlots,
    ChooseNewGameSlot(u8),
    ConfirmOverwrite,
    Continue,
    Save(u8),
    Load(u8),
    MainMenu,
    Back,
    OpenSaveSlots,
    OpenLoadSlots,
    OpenSettings,
    ChooseData,
    Editor,
    Exit,
}

impl GamePresentationMode {
    pub(super) fn request_launch(&mut self) -> bool {
        if !matches!(self, Self::MainMenu | Self::Paused | Self::NewGameSlots | Self::ConfirmOverwrite) {
            return false;
        }
        *self = Self::Launching;
        true
    }

    pub(super) fn commit_player_entry(&mut self) -> bool {
        if self != &Self::Launching {
            return false;
        }
        *self = Self::Running;
        true
    }

    pub(super) fn editor(&mut self) {
        *self = Self::Editor;
    }

    pub(super) fn is_running(self) -> bool {
        self == Self::Running
    }

    pub(super) fn show_frontend(self) -> bool {
        !matches!(self, Self::Editor | Self::Running)
    }
}

/// Keyboard/controller-ready menu selection, independent of scene rendering.
#[derive(Debug, Default)]
pub(super) struct FrontendMenuNavigation {
    screen: Option<GamePresentationMode>,
    selected: usize,
    entered_at: f64,
    scene_started_at: Option<f64>,
    event_sampled_until: f64,
    recent_events: Vec<super::frontend_script::FrontendScheduledEvent>,
    observed_presentation_marker: Option<u32>,
    hud_selector: super::frontend_hud_dispatch::NativeHudMarkerSelector,
    /// Only expose the unverified M00 orbit as an F4 inspection layer.
    /// The original menu camera is not reconstructed yet.
    show_3d_preview: bool,
    debug_chrome_motion_preview: bool,
    show_script_diagnostics: bool,
}
impl FrontendMenuNavigation {
    /// Only auto-enable M00 when a measured original-camera track is present;
    /// F4 can still toggle it off for inspection of 2D HUD alone.
    pub(super) fn use_captured_camera(&mut self, has_capture: bool) {
        self.show_3d_preview = has_capture;
    }

    fn prepare(&mut self, screen: GamePresentationMode, now: f64) -> bool {
        if self.screen == Some(screen) { return false; }
        // The background M00 world persists across H04 frontend screens.
        // Only the screen-local HUD timeline and its input state restart.
        if self.scene_started_at.is_none() || screen == GamePresentationMode::TitleScreen {
            self.scene_started_at = Some(now);
        }
        self.screen = Some(screen);
        self.selected = 0;
        self.entered_at = now;
        self.event_sampled_until = -1.0 / 60.0;
        self.recent_events.clear();
        self.observed_presentation_marker = None;
        self.hud_selector.reset(Vec::new());
        true
    }

    fn move_selection(&mut self, step: isize, count: usize) {
        if count == 0 { self.selected = 0; return; }
        self.selected = (self.selected as isize + step).rem_euclid(count as isize) as usize;
    }

    fn clamp_selection(&mut self, count: usize) {
        if count == 0 { self.selected = 0; }
        else if self.selected >= count { self.selected = count - 1; }
    }

    fn choose(&self, actions: &[GameMenuAction]) -> Option<GameMenuAction> {
        actions.get(self.selected).copied()
    }

    fn scene_seconds(&self, now: f64) -> f64 {
        (now - self.entered_at).max(0.0)
    }

    /// Screen-local H04 playhead after the native 0x46 JumpToMarker.
    /// M00 world time remains independent and uninterrupted.
    fn hud_script_seconds(&self, now: f64) -> f64 {
        self.scene_seconds(now) +
            self.hud_selector.last_seek().map(|seek| seek.event_seconds).unwrap_or(0.0)
    }

    fn background_seconds(&self, now: f64) -> f64 {
        self.scene_started_at.map(|started| (now - started).max(0.0)).unwrap_or(0.0)
    }
}

fn native_frontend_script(screen: GamePresentationMode) -> Option<u32> {
    use super::frontend_script::*;
    match screen {
        GamePresentationMode::TitleScreen => Some(TITLE_ROOT),
        GamePresentationMode::MainMenu => Some(MAIN_MENU_PC),
        GamePresentationMode::NewGameSlots | GamePresentationMode::ConfirmOverwrite =>
            Some(NEW_GAME_MENU),
        GamePresentationMode::LoadSlots | GamePresentationMode::SaveSlots =>
            Some(LOAD_GAME_MENU),
        GamePresentationMode::Settings => Some(OPTIONS_PC),
        _ => None, // native pause menu script contract is still separate
    }
}

/// Scale the original H07 glyphs and interactive hit areas together, based on
/// logical viewport size (physical GPU pixels are handled by egui DPI).
/// Preserve a sane lower bound for handheld-sized Windows, without making
/// 4K monitors display 250-point controls in an empty ocean.
#[derive(Clone, Copy, Debug)]
struct FrontendUiDimensions {
    button_width: f32,
    button_height: f32,
    text_height: f32,
    gap: f32,
}

impl FrontendUiDimensions {
    fn from_viewport(size: egui::Vec2, original: bool) -> Self {
        let short = size.x.min(size.y).max(1.0);
        let scale = (short / 900.0).clamp(if original { 0.65 } else { 0.85 }, 1.7);
        Self {
            button_width: if original { (short * 0.41).clamp(225.0, 570.0) }
                else { (size.x * 0.33).clamp(250.0, 620.0) },
            button_height: if original { 21.5 * scale } else { 38.0 * scale },
            text_height: if original { 16.5 * scale } else { 17.0 * scale },
            gap: if original { 0.6 * scale } else { 5.0 * scale },
        }
    }
}

fn frontend_dimensions(ui: &egui::Ui, original: bool) -> FrontendUiDimensions {
    FrontendUiDimensions::from_viewport(ui.ctx().content_rect().size(), original)
}

/// Proven by Robots.exe 0x00499960: each menu state selects a 0x46-class
/// marker through 0x00440600. Exact marker IDs also occur in the shipped
/// H04 script's 0x16000008 Event payload. This fallback lookup is diagnostic;
/// runtime markers are decoded from the corresponding Script Event.
fn native_hud_marker_for_menu(screen: GamePresentationMode) -> Option<u32> {
    match screen {
        GamePresentationMode::MainMenu => Some(0x4600_0001),
        GamePresentationMode::NewGameSlots | GamePresentationMode::ConfirmOverwrite => Some(0x4600_000c),
        GamePresentationMode::LoadSlots | GamePresentationMode::SaveSlots => Some(0x4600_000d),
        GamePresentationMode::Settings => Some(0x4600_0022),
        _ => None,
    }
}

fn menu_button(
    ui: &mut egui::Ui, action: &mut Option<GameMenuAction>,
    original: Option<&super::menu_assets::RobotsMenuAssets>,
    navigation: &mut FrontendMenuNavigation,
    actions: &mut Vec<GameMenuAction>,
    enabled: bool, label: &str, next: GameMenuAction,
) {
    let dims = frontend_dimensions(ui, original.is_some());
    let native_text = original.is_some_and(|assets| assets.can_paint_original_label(label));
    // Keep the button's semantic label and interaction while H07 draws the
    // actual visible glyphs. Unsupported Unicode falls back to egui text.
    let rich_label = egui::RichText::new(label).size(dims.text_height)
        .strong().color(if native_text { egui::Color32::TRANSPARENT }
            else { egui::Color32::from_rgb(235, 237, 236) });
    let ordinal = if enabled {
        let index = actions.len();
        actions.push(next);
        Some(index)
    } else { None };
    let width = if original.is_some() && next == GameMenuAction::EnterMainMenu {
        // Title's solitary "Press Enter" highlight spans most of the stage.
        (ui.ctx().content_rect().width().min(ui.ctx().content_rect().height()) * 0.88)
            .clamp(280.0, 810.0)
    } else { dims.button_width };
    let response = if original.is_some() {
        // An invisible egui Button still reserves default label padding,
        // so the visible H07 text drifts into the metal teeth. Allocate an
        // exact hitbox and paint glyphs ourselves using the native atlas.
        ui.allocate_response(egui::vec2(width, dims.button_height),
            if enabled { egui::Sense::click() } else { egui::Sense::hover() })
    } else {
        ui.add_enabled(enabled,
            egui::Button::new(rich_label)
                .min_size(egui::vec2(width, dims.button_height)))
    };
    let pointer_active = response.hovered()
        && ui.input(|i| i.pointer.delta().length_sq() > 0.0 || i.pointer.any_pressed());
    if let Some(assets) = original {
        let selected = enabled && (pointer_active || ordinal == Some(navigation.selected));
        if selected { assets.paint_selection(ui, response.rect); }
        if native_text {
            let tint = if enabled { egui::Color32::WHITE }
                else { egui::Color32::from_gray(110) };
            assets.paint_original_label(ui, response.rect, label, dims.text_height, tint);
        } else if selected {
            // The highlight is above the button: restore fallback glyphs on top.
            ui.painter().text(response.rect.center(), egui::Align2::CENTER_CENTER,
                label, egui::FontId::proportional(dims.text_height), egui::Color32::WHITE);
        }
    }
    if pointer_active {
        if let Some(i) = ordinal { navigation.selected = i; }
    }
    if response.clicked() { *action = Some(next); }
    ui.add_space(dims.gap);
}

/// A disabled frontend entry is not a navigable action, but it should still
/// use the same original H07 glyph pipeline as enabled Game Mode commands.
fn disabled_menu_button(
    ui: &mut egui::Ui,
    original: Option<&super::menu_assets::RobotsMenuAssets>,
    label: &str,
    hint: &str,
) {
    let native_text = original.is_some_and(|assets| assets.can_paint_original_label(label));
    let dims = frontend_dimensions(ui, original.is_some());
    let text = egui::RichText::new(label)
        .size(dims.text_height)
        .color(if native_text { egui::Color32::TRANSPARENT } else { egui::Color32::GRAY });
    let response = if original.is_some() {
        ui.allocate_response(egui::vec2(dims.button_width, dims.button_height),
            egui::Sense::hover())
    } else {
        ui.add_enabled(false, egui::Button::new(text)
            .min_size(egui::vec2(dims.button_width, dims.button_height)))
    };
    if native_text {
        if let Some(assets) = original {
            assets.paint_original_label(ui, response.rect, label, dims.text_height, egui::Color32::from_gray(110));
        }
    }
    response.on_hover_text(hint);
    ui.add_space(dims.gap);
}

/// A small self-contained frontend; renderer and AI come exclusively from
/// MapViewerPanel. These are host labels, not claimed original menu graphics.
pub(super) fn draw_game_menu(
    ui: &mut egui::Ui,
    mode: GamePresentationMode,
    loaded_edb: bool,
    session_alive: bool,
    slots: [bool; 3],
    overwrite_slot: Option<u8>,
    error: Option<&str>,
    audio_levels: &mut [f32; 3],
    original: Option<&super::menu_assets::RobotsMenuAssets>,
    scene3d: Option<&super::frontend_scene3d::FrontendScene3D>,
    navigation: &mut FrontendMenuNavigation,
) -> Option<GameMenuAction> {
    let mut action = None;
    let now = ui.ctx().input(|i| i.time);
    let new_screen = navigation.prepare(mode, now);
    if new_screen {
        let markers = native_frontend_script(mode)
            .and_then(|uid| original.map(|assets| assets.marker_evidence(uid)))
            .unwrap_or_default();
        navigation.hud_selector.reset(markers);
        if let Some(marker) = native_hud_marker_for_menu(mode) {
            let selected = navigation.hud_selector.select_uid(marker, false);
            if let Some(seek) = selected {
                // 0x0043D730 copies the *last payload float* to native
                // record+0x30, but this is descriptor metadata. The H04
                // Event.start is the script playhead after selecting the runtime record.
                navigation.event_sampled_until = seek.event_seconds - 1.0 / 60.0;
            }
            tracing::debug!(
                "H04 HUD marker selection: {marker:#010x}, event-frame evidence={selected:?}"
            );
        }
    }
    let _deferred_marker = navigation.hud_selector.tick();
    let mut choices = Vec::new();
    let scene_time = navigation.hud_script_seconds(now);
    let native_frame = native_frontend_script(mode).and_then(|uid|
        original.filter(|assets| assets.has_script(uid))
            .map(|assets| assets.sample_script(uid, scene_time))
    );
    // MainMenu_PC ships paired Entity 0x82000031 controllers; use their
    // actual keyed motion curve for the temporary 2D chrome stage.
    let chrome_motion = if mode == GamePresentationMode::MainMenu {
        original.and_then(|assets|
            assets.main_menu_chrome_motion(super::frontend_script::MAIN_MENU_PC, scene_time))
    } else { None };
    let projected_chrome_motion = if navigation.debug_chrome_motion_preview {
        chrome_motion
    } else { None };
    if let (Some(uid), Some(assets)) = (native_frontend_script(mode), original) {
        // Start-edge scheduling only. Native Event payload effects are not yet
        // interpreted; do not let an unknown opcode change menu/game state.
        let fired = assets.events_started_between(uid, navigation.event_sampled_until, scene_time);
        navigation.event_sampled_until = scene_time;
        if !fired.is_empty() {
            for event in &fired {
                if let Some(marker) = event.presentation_marker {
                    navigation.observed_presentation_marker = Some(marker);
                }
                tracing::trace!("H04 Event edge script={:#010x} type={:#010x} marker={:?} payload_scalar={:?} at={:.3}s",
                    event.source_script_uid, event.event_type,
                    event.presentation_marker, event.presentation_payload_scalar, event.at_seconds);
            }
            navigation.recent_events.extend(fired);
            let excess = navigation.recent_events.len().saturating_sub(16);
            navigation.recent_events.drain(..excess);
        }
    }
    // The shipped frontend HUD scripts advance with elapsed time even when
    // the mouse and keyboard are idle. Keep their clock ticking independently
    // of egui input events; otherwise the F3 inspector freezes on one frame.
    if native_frame.is_some() {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(16));
    }
    egui::CentralPanel::default().show(ui, |ui| {
        let rect = ui.max_rect();
        let preview_3d = navigation.show_3d_preview && scene3d.is_some();
        if preview_3d {
            if let Some(scene) = scene3d {
                scene.paint(ui, rect, navigation.background_seconds(now) as f32);
            }
        }
        if let Some(assets) = original {
            let entrance = match mode {
                GamePresentationMode::TitleScreen =>
                    native_frontend_script(mode).and_then(|uid|
                        assets.entrance_progress(uid, navigation.hud_script_seconds(now)))
                        .unwrap_or(1.0),
                _ => 1.0,
            };
            if mode == GamePresentationMode::TitleScreen {
                assets.paint_original_title(ui, entrance, preview_3d);
            } else {
                assets.paint_original_frame(ui, entrance, preview_3d, projected_chrome_motion);
            }
        }
        else { ui.painter().rect_filled(rect, 0.0, egui::Color32::from_rgb(15, 24, 36)); }
        ui.vertical_centered(|ui| {
            if original.is_some() {
                // Native HUD hitboxes have their own layout. egui's default
                // per-widget padding/spacing must not be layered on top.
                ui.spacing_mut().item_spacing.y = 0.0;
                // Original FE_MainMenu_PC displays the options in the inner
                // transparent metal-backed panel, not under a separate logo.
                if mode == GamePresentationMode::TitleScreen {
                    ui.add_space((ui.available_height() * 0.71).max(76.0));
                } else {
                    // Main Menu controls share the very same H04 art canvas:
                    // position them inside the top enamel panel, rather than
                    // across the exposed teeth and the second chrome panel.
                    let top = super::menu_assets::RobotsMenuAssets::main_menu_button_top(ui.max_rect())
                        + super::menu_assets::RobotsMenuAssets::chrome_motion_pixels(
                            ui.max_rect(), projected_chrome_motion.unwrap_or(0.0));
                    ui.add_space((top - ui.cursor().top()).max(0.0));
                }
            } else {
                ui.add_space((ui.available_height() * 0.12).max(24.0));
                ui.label(egui::RichText::new("ROBOTS").size(58.0).strong()
                    .color(egui::Color32::from_rgb(244, 190, 63)));
                ui.label(egui::RichText::new("EUROCHEF · GAME MODE (experimental)").size(13.0)
                    .color(egui::Color32::LIGHT_GRAY));
                ui.add_space(28.0);
            }

            match mode {
                GamePresentationMode::TitleScreen => {
                    menu_button(ui, &mut action, original, navigation, &mut choices, true,
                        "Press Enter / Click to Start", GameMenuAction::EnterMainMenu);
                }
                GamePresentationMode::MainMenu => {
                    menu_button(ui, &mut action, original, navigation, &mut choices, loaded_edb, "New Game", GameMenuAction::OpenNewGameSlots);
                    // Resume is a developer convenience on F6. The shipped
                    // Main Menu has five entries, not six; rendering this
                    // extra row would cross the original chrome's teeth.
                    menu_button(ui, &mut action, original, navigation, &mut choices, true, "Load Game", GameMenuAction::OpenLoadSlots);
                    menu_button(ui, &mut action, original, navigation, &mut choices, true, "Options", GameMenuAction::OpenSettings);
                    disabled_menu_button(ui, original, "Bonus Features",
                        "Original bonus content and unlock conditions not yet recovered");
                    // Developer-only Editor/EDB selection remains on F2.
                    // An extra on-screen item would break the original five-row
                    // Main Menu composition and overlap its mechanical chrome.
                    menu_button(ui, &mut action, original, navigation, &mut choices, true, "Quit To Desktop", GameMenuAction::Exit);
                }
                GamePresentationMode::NewGameSlots => {
                    ui.heading("New Game · Select Save Slot");
                    ui.label("Choose a slot or start without saving.");
                    ui.add_space(12.0);
                    for slot in 1..=3u8 {
                        menu_button(ui, &mut action, original, navigation, &mut choices, true,
                            &format!("Slot {slot}  {}", if slots[(slot - 1) as usize] { "[occupied]" } else { "[empty]" }),
                            GameMenuAction::ChooseNewGameSlot(slot));
                    }
                    menu_button(ui, &mut action, original, navigation, &mut choices, true, "Play Without Saving", GameMenuAction::NewGame);
                    menu_button(ui, &mut action, original, navigation, &mut choices, true, "Back", GameMenuAction::Back);
                }
                GamePresentationMode::ConfirmOverwrite => {
                    ui.heading("Overwrite Saved Game?");
                    ui.label(format!("Slot {} already contains a checkpoint.", overwrite_slot.unwrap_or_default()));
                    ui.label("The previous checkpoint will be replaced after the new Player starts.");
                    menu_button(ui, &mut action, original, navigation, &mut choices, overwrite_slot.is_some(), "Overwrite", GameMenuAction::ConfirmOverwrite);
                    menu_button(ui, &mut action, original, navigation, &mut choices, true, "Cancel", GameMenuAction::Back);
                }
                GamePresentationMode::Paused => {
                    menu_button(ui, &mut action, original, navigation, &mut choices, true, "Resume Game", GameMenuAction::Continue);
                    menu_button(ui, &mut action, original, navigation, &mut choices, true, "Save Game", GameMenuAction::OpenSaveSlots);
                    menu_button(ui, &mut action, original, navigation, &mut choices, true, "Load Game", GameMenuAction::OpenLoadSlots);
                    menu_button(ui, &mut action, original, navigation, &mut choices, true, "Main Menu", GameMenuAction::MainMenu);
                    menu_button(ui, &mut action, original, navigation, &mut choices, true, "Return to Editor", GameMenuAction::Editor);
                }
                GamePresentationMode::SaveSlots | GamePresentationMode::LoadSlots => {
                    ui.heading(if mode == GamePresentationMode::SaveSlots { "Save Game" } else { "Load Game" });
                    ui.label("Checkpoint format v1: Player/inventory/missions only; world XItems are not serialized.");
                    ui.add_space(12.0);
                    for slot in 1..=3 {
                        menu_button(ui, &mut action, original, navigation, &mut choices, mode == GamePresentationMode::SaveSlots || slots[(slot - 1) as usize],
                            &format!("Slot {slot}  {}", if slots[(slot - 1) as usize] {"[saved]"} else {"[empty]"}),
                            if mode == GamePresentationMode::SaveSlots {GameMenuAction::Save(slot)} else {GameMenuAction::Load(slot)});
                    }
                    menu_button(ui, &mut action, original, navigation, &mut choices, true, "Back", GameMenuAction::Back);
                }
                GamePresentationMode::Settings => {
                    ui.heading("Settings");
                    ui.add(egui::Slider::new(&mut audio_levels[0], 0.0..=1.0).text("Master Volume"));
                    ui.add(egui::Slider::new(&mut audio_levels[1], 0.0..=1.0).text("Music / Ambient Volume"));
                    ui.add(egui::Slider::new(&mut audio_levels[2], 0.0..=1.0).text("Sound Effects Volume"));
                    ui.label("Original subtitle, camera and controller options are not wired yet.");
                    menu_button(ui, &mut action, original, navigation, &mut choices, true, "Back", GameMenuAction::Back);
                }
                GamePresentationMode::Launching => {
                    ui.spinner();
                    ui.label("Loading level and creating Player...");
                }
                GamePresentationMode::Running | GamePresentationMode::Editor => {}
            }
            if let Some(error) = error {
                ui.add_space(8.0);
                ui.colored_label(egui::Color32::LIGHT_RED, error);
            }
        });
    });

    // Arrow keys navigate enabled commands in native menu order; disabled
    // Bonus Features / unavailable slots do not become phantom selections.
    // Never activate a new screen on the same Enter that dismissed the old.
    let (up, down, enter, back, toggle_debug, toggle_3d, toggle_motion, editor_shortcut, resume_shortcut) = ui.ctx().input(|i| (
        i.key_pressed(egui::Key::ArrowUp),
        i.key_pressed(egui::Key::ArrowDown),
        i.key_pressed(egui::Key::Enter),
        i.key_pressed(egui::Key::Escape),
        i.key_pressed(egui::Key::F3),
        i.key_pressed(egui::Key::F4),
        i.key_pressed(egui::Key::F5),
        i.key_pressed(egui::Key::F2),
        i.key_pressed(egui::Key::F6),
    ));
    if toggle_debug { navigation.show_script_diagnostics = !navigation.show_script_diagnostics; }
    if toggle_3d { navigation.show_3d_preview = !navigation.show_3d_preview; }
    if toggle_motion { navigation.debug_chrome_motion_preview = !navigation.debug_chrome_motion_preview; }
    if editor_shortcut && mode == GamePresentationMode::MainMenu {
        action = Some(if loaded_edb { GameMenuAction::Editor } else { GameMenuAction::ChooseData });
    }
    if resume_shortcut && mode == GamePresentationMode::MainMenu && session_alive {
        action = Some(GameMenuAction::Continue);
    }
    // Entries can disappear when EDB availability or session state changes
    // without a screen transition; never leave the cursor on a stale index.
    navigation.clamp_selection(choices.len());
    if !new_screen && action.is_none() {
        if up { navigation.move_selection(-1, choices.len()); }
        if down { navigation.move_selection(1, choices.len()); }
        if enter && !up && !down { action = navigation.choose(&choices); }
        if back {
            action = match mode {
                GamePresentationMode::MainMenu => Some(GameMenuAction::Exit),
                GamePresentationMode::Paused => Some(GameMenuAction::Continue),
                GamePresentationMode::TitleScreen => Some(GameMenuAction::EnterMainMenu),
                GamePresentationMode::NewGameSlots |
                GamePresentationMode::ConfirmOverwrite |
                GamePresentationMode::Settings |
                GamePresentationMode::LoadSlots |
                GamePresentationMode::SaveSlots => Some(GameMenuAction::Back),
                _ => None,
            }
        }
    }
    if up || down || toggle_debug || toggle_3d || toggle_motion { ui.ctx().request_repaint(); }
    if navigation.show_script_diagnostics {
        if let (Some(uid), Some(frame)) = (native_frontend_script(mode), native_frame.as_ref()) {
            egui::Window::new("Robots frontend HudScript (F3)")
                .resizable(true)
                .collapsible(true)
                .default_width(300.0)
                .show(ui.ctx(), |ui| {
                    ui.label(format!("Root UID {uid:#010x} · H04 playhead {:.3}s",
                        navigation.hud_script_seconds(now)));
                    ui.label(format!("M00 persistent background: {:.3}s", navigation.background_seconds(now)));
                    if let Some(motion) = chrome_motion {
                        ui.monospace(format!("H04 Entity Y keys: {motion:.3}; 2D motion preview F5: {} (unverified mapping)",
                            navigation.debug_chrome_motion_preview));
                    }
                    // Each serialized H04 Entity command is an independent
                    // controller instance, even when resource UIDs repeat.
                    // Inspect LOCAL TRS only; not projected GPU panel vertices.
                    if let Some(assets) = original {
                        let poses = assets.entity_poses(uid, navigation.hud_script_seconds(now));
                        ui.label(format!("Native H04 Entity instances active: {}", poses.len()));
                        for pose in poses.iter().take(10) {
                            ui.monospace(format!(
                                "#{} uid={:#010x} frames={:.0}..{:.0} pos=[{:.2},{:.2},{:.2}] scale=[{:.2},{:.2},{:.2}]",
                                pose.command_index, pose.entity_uid, pose.start_frame, pose.end_frame,
                                pose.position[0], pose.position[1], pose.position[2],
                                pose.scale[0], pose.scale[1], pose.scale[2],
                            ));
                            ui.small(format!(
                                "   local quat=[{:.3},{:.3},{:.3},{:.3}] · GPU HUD projection unresolved",
                                pose.rotation[0],pose.rotation[1],pose.rotation[2],pose.rotation[3]));
                        }
                    }
                    if let Some(scene) = scene3d {
                        ui.monospace(if scene.has_captured_camera() {
                            "Camera: measured original track/dump (NativeViewCamera)"
                        } else {
                            "Camera: M00 orbit fallback (no original capture; F4 preview only)"
                        });
                    }
                    if let Some(hud) = native_hud_marker_for_menu(mode) {
                        ui.monospace(format!("Native 0x46 marker expected: {hud:#010x}"));
                    }
                    if let Some(selected) = navigation.hud_selector.selected_uid() {
                        ui.monospace(format!("Native marker selected: {selected:#010x}, status={:?}",
                            navigation.hud_selector.selected_state()));
                    }
                    if let Some(seek) = navigation.hud_selector.last_seek() {
                        ui.monospace(format!("H04 marker event starts at frame {:.1} ({:.3}s)",
                            seek.event_frame, seek.event_seconds));
                        ui.monospace(format!("Native JumpToMarker: runtime record {} / script start {:.3}s",
                            seek.runtime_record_index, seek.event_seconds));
                    }
                    if let Some(pending) = navigation.hud_selector.pending_uid() {
                        ui.monospace(format!("Deferred marker pending: {pending:#010x}"));
                    }
                    ui.label(format!("Active commands {} · unresolved subscripts {} · cycles {}",
                        frame.commands.len(), frame.unresolved_subscripts.len(),
                        frame.cyclic_subscripts.len()));
                    ui.small("Shipped 0x16000008 events decode real 0x46 marker UIDs; marker playback side effects remain open.");
                    if let Some(marker) = navigation.observed_presentation_marker {
                        ui.monospace(format!("H04 event marker observed: {marker:#010x}"));
                    }
                    for command in frame.commands.iter()
                        .filter_map(|command| command.presentation_marker)
                        .take(8) {
                        if let Some(binding) = super::frontend_hud_dispatch::native_option_binding(command) {
                            ui.monospace(format!(
                                "Original HUD option {:#010x} -> saved setting {:#010x} (volume arrows: {})",
                                binding.option_uid, binding.saved_setting_uid, binding.volume_adjustable));
                        }
                    }
                    ui.label(format!("Recent native H04 Event starts: {}", navigation.recent_events.len()));
                    for event in navigation.recent_events.iter().rev().take(8) {
                        ui.monospace(format!("{:.2}s type={:#010x} marker={:?} metadata={:?} script={:#010x} cmd={}",
                            event.at_seconds, event.event_type,
                            event.presentation_marker, event.presentation_payload_scalar,
                            event.source_script_uid, event.command_index));
                    }
                    if let Some(assets) = original {
                        egui::CollapsingHeader::new("Original H07 font texture")
                            .show(ui, |ui| assets.show_font_atlas_diagnostic(ui));
                    }
                    for command in frame.commands.iter().take(18) {
                        ui.monospace(format!("{:#010x}: #{:02} {:?} ref={:?} ctl={}",
                            command.source_script_uid, command.command_index,
                            command.kind, command.resource_uid.or(command.presentation_marker), command.controller_index));
                    }
                });
        }
    }
    action
}

#[cfg(test)]
mod tests {
    use super::{FrontendMenuNavigation, FrontendUiDimensions, GamePresentationMode};

    #[test]
    fn frontend_controls_scale_with_viewport_without_changing_gameplay_editor() {
        let compact = FrontendUiDimensions::from_viewport(egui::vec2(960.0, 540.0), true);
        let full_hd = FrontendUiDimensions::from_viewport(egui::vec2(1920.0, 1080.0), true);
        let ultra_hd = FrontendUiDimensions::from_viewport(egui::vec2(3840.0, 2160.0), true);
        assert!(compact.button_height < full_hd.button_height);
        assert!(full_hd.button_height < ultra_hd.button_height);
        assert!(compact.text_height < full_hd.text_height && full_hd.text_height < ultra_hd.text_height);
        assert!(compact.button_width < full_hd.button_width && full_hd.button_width <= ultra_hd.button_width);
        assert!(ultra_hd.button_width <= 620.0);
        for size in [egui::vec2(960.0, 540.0), egui::vec2(1920.0, 1080.0), egui::vec2(3840.0, 2160.0)] {
            let metrics = FrontendUiDimensions::from_viewport(size, true);
            assert!(metrics.button_width >= metrics.text_height * 7.0);
        }
    }

    #[test]
    fn captured_frontend_camera_auto_enables_3d_preview_only_when_measured() {
        let mut nav = FrontendMenuNavigation::default();
        assert!(!nav.show_3d_preview);
        assert!(!nav.debug_chrome_motion_preview);
        nav.use_captured_camera(true);
        assert!(nav.show_3d_preview);
        nav.use_captured_camera(false);
        assert!(!nav.show_3d_preview);
    }

    #[test]
    fn persistent_m00_clock_survives_menu_navigation_but_new_title_resets() {
        use super::FrontendMenuNavigation;
        let mut nav = FrontendMenuNavigation::default();
        assert!(nav.prepare(GamePresentationMode::TitleScreen, 12.0));
        assert_eq!(nav.background_seconds(15.0), 3.0);
        assert!(nav.prepare(GamePresentationMode::MainMenu, 15.0));
        assert_eq!(nav.scene_seconds(16.0), 1.0);
        assert_eq!(nav.background_seconds(16.0), 4.0);
        assert!(nav.prepare(GamePresentationMode::Settings, 17.0));
        assert_eq!(nav.background_seconds(18.0), 6.0);
        assert!(nav.prepare(GamePresentationMode::TitleScreen, 19.0));
        assert_eq!(nav.background_seconds(19.0), 0.0);
    }

    #[test]
    fn native_marker_seek_offsets_only_h04_screen_playhead() {
        use super::super::frontend_hud_dispatch::HudMarkerDescriptor;
        let mut nav = FrontendMenuNavigation::default();
        nav.prepare(GamePresentationMode::TitleScreen, 10.0);
        assert_eq!(nav.background_seconds(12.0), 2.0);
        nav.prepare(GamePresentationMode::MainMenu, 12.0);
        nav.hud_selector.reset(vec![HudMarkerDescriptor {
            uid: 0x4600_0001, event_frame: 56.0, runtime_record_index: 1, payload_scalar: 0.0, fps: 30.0,
        }]);
        let seek = nav.hud_selector.select_uid(0x4600_0001, false).unwrap();
        assert_eq!(seek.runtime_record_index, 1);
        nav.event_sampled_until = seek.event_seconds - 1.0 / 60.0;
        assert!((nav.hud_script_seconds(12.0) - 56.0 / 30.0).abs() < 1e-9);
        assert!((nav.hud_script_seconds(13.0) - (1.0 + 56.0 / 30.0)).abs() < 1e-9);
        assert_eq!(nav.scene_seconds(13.0), 1.0);
        assert_eq!(nav.background_seconds(13.0), 3.0);
        nav.prepare(GamePresentationMode::TitleScreen, 14.0);
        assert_eq!(nav.hud_script_seconds(14.0), 0.0);
        assert_eq!(nav.background_seconds(14.0), 0.0);
    }

    #[test]
    fn native_hud_resource_selection_matches_robots_frontend_state_machine() {
        use super::native_hud_marker_for_menu;
        assert_eq!(native_hud_marker_for_menu(GamePresentationMode::MainMenu), Some(0x4600_0001));
        assert_eq!(native_hud_marker_for_menu(GamePresentationMode::NewGameSlots), Some(0x4600_000c));
        assert_eq!(native_hud_marker_for_menu(GamePresentationMode::LoadSlots), Some(0x4600_000d));
        assert_eq!(native_hud_marker_for_menu(GamePresentationMode::Settings), Some(0x4600_0022));
        assert_eq!(native_hud_marker_for_menu(GamePresentationMode::TitleScreen), None);
        assert_eq!(native_hud_marker_for_menu(GamePresentationMode::Running), None);
    }

    #[test]
    fn game_mode_starts_only_after_verified_player_entry() {
        let mut mode = GamePresentationMode::default();
        assert!(!mode.is_running());
        assert!(!mode.commit_player_entry(), "no live Player, no Game Mode");
        mode = GamePresentationMode::MainMenu;
        assert!(mode.request_launch());
        assert!(!mode.request_launch(), "do not enqueue a second launch");
        assert!(!mode.is_running(), "an EDB request is not a Player");
        assert!(mode.commit_player_entry());
        assert!(mode.is_running());
        assert!(!mode.commit_player_entry(), "duplicate commit refused");
        mode.editor();
        assert!(!mode.is_running());
        mode = GamePresentationMode::MainMenu;
        assert!(mode.request_launch());
    }
    #[test]
    fn keyboard_navigation_wraps_and_ignores_disabled_entries() {
        use super::{FrontendMenuNavigation, GameMenuAction};
        let mut navigation = FrontendMenuNavigation::default();
        assert!(navigation.prepare(GamePresentationMode::MainMenu, 1.0));
        let choices = [
            GameMenuAction::OpenNewGameSlots, GameMenuAction::OpenLoadSlots,
            GameMenuAction::OpenSettings, GameMenuAction::Exit,
        ];
        assert_eq!(navigation.choose(&choices), Some(GameMenuAction::OpenNewGameSlots));
        navigation.move_selection(-1, choices.len());
        assert_eq!(navigation.choose(&choices), Some(GameMenuAction::Exit));
        navigation.move_selection(1, choices.len());
        assert_eq!(navigation.choose(&choices), Some(GameMenuAction::OpenNewGameSlots));
        navigation.move_selection(2, choices.len());
        assert_eq!(navigation.choose(&choices), Some(GameMenuAction::OpenSettings));
        // A different screen must reset the cursor; the first key press
        // cannot silently activate an option from the previous screen.
        assert!(navigation.prepare(GamePresentationMode::Settings, 4.0));
        assert_eq!(navigation.choose(&[GameMenuAction::Back]), Some(GameMenuAction::Back));
        assert_eq!(navigation.scene_seconds(4.5), 0.5);
        assert_eq!(navigation.background_seconds(4.5), 3.5);
        navigation.move_selection(3, choices.len());
        navigation.clamp_selection(1);
        assert_eq!(navigation.choose(&[GameMenuAction::Back]), Some(GameMenuAction::Back));
        navigation.move_selection(-1, 0);
        navigation.clamp_selection(0);
        assert_eq!(navigation.choose(&[]), None);
        assert!(!navigation.prepare(GamePresentationMode::Settings, 5.0));
        assert_eq!(navigation.scene_seconds(5.5), 1.5);
    }

    #[test]
    fn each_frontend_screen_selects_the_shipped_native_hudscript() {
        use super::native_frontend_script;
        use super::super::frontend_script::*;
        assert_eq!(native_frontend_script(GamePresentationMode::TitleScreen), Some(TITLE_ROOT));
        assert_eq!(native_frontend_script(GamePresentationMode::MainMenu), Some(MAIN_MENU_PC));
        assert_eq!(native_frontend_script(GamePresentationMode::NewGameSlots), Some(NEW_GAME_MENU));
        assert_eq!(native_frontend_script(GamePresentationMode::Settings), Some(OPTIONS_PC));
        assert_eq!(native_frontend_script(GamePresentationMode::Paused), None);
        assert_eq!(native_frontend_script(GamePresentationMode::Running), None);
    }

    #[test]
    fn failed_player_entry_aborts_launch() {
        let mut mode = GamePresentationMode::MainMenu;
        assert!(mode.request_launch());
        mode = GamePresentationMode::MainMenu;
        assert!(!mode.commit_player_entry());
    }
}
