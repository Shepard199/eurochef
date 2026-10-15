use anyhow::Context;
use std::{
    collections::hash_map,
    fs::File,
    io::{BufReader, Cursor, Read, Seek},
    path::PathBuf,
    sync::Arc,
};

use crossbeam::atomic::AtomicCell;
use eframe::CreationContext;
use egui::{mutex::RwLock, Color32, FontData, FontDefinitions, NumExt};
use eurochef_edb::{
    binrw::{BinReaderExt, Endian},
    edb::EdbFile,
    particle::EXGeoParticle,
    versions::Platform,
    Hashcode, HashcodeUtils,
};
use eurochef_shared::filesystem::path::DissectedFilelistPath;
use eurochef_shared::{
    hashcodes::parse_hashcodes,
    script::{UXGeoScript, UXGeoScriptCommandData},
    spreadsheets::UXGeoSpreadsheet,
    textures::UXGeoTexture,
};
use instant::Instant;
use nohash_hasher::IntMap;

use crate::{
    animations,
    entities::{self},
    fileinfo, maps,
    render::{entity::EntityRenderer, RenderStore},
    scripts, spreadsheet, textures,
};

/// Basic app tracking state
pub enum AppState {
    Ready,
    SelectPlatform,
    Loading(String),
    Error(anyhow::Error),
}

#[derive(PartialEq)]
enum Panel {
    FileInfo,
    Maps,
    Entities,
    Textures,
    Spreadsheets,
    Scripts,
    Animations,
}

/// In-flight native level entry. The App owns this across the replacement of
/// MapViewerPanel by load_file(); no editor-selected context is carried over.
#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingNativeLevelEntry {
    target_edb_uid: u32,
    player_entry_context: u32,
    // Persistent gameplay globals only. World XItems are level-local and
    // must be recreated by destination TriggerManager, not copied.
    progress: Option<eurochef_shared::robots_runtime::change_level::RobotsLevelTransitionProgress>,
}

impl PendingNativeLevelEntry {
    fn matches_loaded_edb(&self, loaded_uid: Option<u32>) -> bool {
        loaded_uid == Some(self.target_edb_uid)
    }
}

#[cfg(test)]
mod native_level_entry_tests {
    use super::PendingNativeLevelEntry;

    #[test]
    fn transition_context_is_consumable_only_after_exact_target_file_header() {
        let pending = PendingNativeLevelEntry {
            target_edb_uid: 0x0100_001D,
            player_entry_context: 7,
            progress: None,
        };
        assert!(!pending.matches_loaded_edb(None));
        assert!(!pending.matches_loaded_edb(Some(0x0100_001E)));
        assert!(pending.matches_loaded_edb(Some(0x0100_001D)));
        assert_eq!(pending.player_entry_context, 7);
    }

    #[test]
    fn level_entry_preserves_inventory_and_fresh_start_does_not() {
        use eurochef_shared::robots_runtime::script_host::RobotsScriptGameplayState;
        let mut globals = RobotsScriptGameplayState::default();
        globals.inventory.values.push(
            eurochef_shared::robots_runtime::inventory::RobotsInventoryValue {
                uid: 0x4700_0001,
                current: 12,
            },
        );
        globals.missions.set_status(
            0x5400_0001,
            eurochef_shared::robots_runtime::mission::RobotsMissionStatus::Completed,
        );
        let mut player_items =
            eurochef_shared::robots_runtime::player_items::RobotsPlayerItemState::default();
        assert!(player_items.seed_profile_value(
            eurochef_shared::robots_runtime::player_items::ROBOTS_PLAYER_ITEM_WATCHBOT_CONTROLLER,
            1,
        ));
        assert!(player_items.seed_profile_value(
            eurochef_shared::robots_runtime::player_items::ROBOTS_PLAYER_ITEM_WATCHBOT_UPGRADE,
            1,
        ));
        let carried = PendingNativeLevelEntry {
            target_edb_uid: 0x0100_001D,
            player_entry_context: 7,
            progress: Some(eurochef_shared::robots_runtime::change_level::RobotsLevelTransitionProgress {
                gameplay: globals.clone(),
                player_items: player_items.clone(),
                health_fraction_bits_58000027: Some(0.35f32.to_bits()),
                selected_action_uid_58000029: None,
                player_scalar_bits_58000028: Some(80.0f32.to_bits()),
            }),
        };
        assert_eq!(carried.progress.as_ref().map(|p| &p.gameplay), Some(&globals));
        assert_eq!(carried.progress.as_ref().map(|p| &p.player_items), Some(&player_items));
        assert_eq!(
            carried.progress.as_ref().unwrap().player_items.stored_current(
                eurochef_shared::robots_runtime::player_items::ROBOTS_PLAYER_ITEM_WATCHBOT_CONTROLLER,
            ),
            1,
        );
        assert!(carried.progress.as_ref().unwrap().player_items.is_enabled(
            eurochef_shared::robots_runtime::player_items::ROBOTS_PLAYER_ITEM_WATCHBOT_UPGRADE,
        ));
        assert!((carried.progress.as_ref().unwrap().restored_health(200.0).unwrap() - 70.0).abs() < 0.0001);
        assert_eq!(
            carried.progress.as_ref().unwrap().gameplay.missions.status(0x5400_0001),
            eurochef_shared::robots_runtime::mission::RobotsMissionStatus::Completed,
        );
        assert!(PendingNativeLevelEntry { progress: None, ..carried }.progress.is_none());
    }
}

pub struct EurochefApp {
    gl: Arc<glow::Context>,

    state: AppState,
    current_panel: Panel,
    game_mode: GamePresentationMode,
    auto_launch_game_once: bool,
    game_menu_return_mode: GamePresentationMode,
    game_menu_notice: Option<String>,
    game_session_live: bool,
    game_menu_assets: Option<menu_assets::RobotsMenuAssets>,
    frontend_scene3d: Option<frontend_scene3d::FrontendScene3D>,
    game_menu_navigation: game_mode::FrontendMenuNavigation,
    game_menu_assets_source: Option<PathBuf>,
    pending_new_game_slot: Option<u8>,
    initial_game_slot: Option<u8>,

    spreadsheetlist: Option<spreadsheet::TextItemList>,
    fileinfo: Option<fileinfo::FileInfoPanel>,
    textures: Option<textures::TextureList>,
    entities: Option<entities::EntityListPanel>,
    maps: Option<maps::MapViewerPanel>,
    scripts: Option<scripts::ScriptListPanel>,
    animations: Option<animations::AnimationListPanel>,
    sound_preview: Option<crate::sound_preview::SharedSoundPreview>,

    load_input: Arc<AtomicCell<Option<(Vec<u8>, String)>>>,
    pending_file: Option<(Vec<u8>, Option<Platform>)>,
    /// One-shot verified ChangeLevel entry, distinct from manual EDB opening.
    pending_native_level_entry: Option<PendingNativeLevelEntry>,
    current_source_path: Option<PathBuf>,
    selected_platform: Platform,

    ps2_warning: bool,
    about_window: bool,
    show_profiler: bool,

    hashcodes: Arc<IntMap<u32, String>>,
    path_cache: IntMap<Hashcode, String>,
    render_store: Arc<RwLock<RenderStore>>,
    game: String,
}

mod loading;
mod ui;
mod game_mode;
mod checkpoint;
mod menu_assets;
mod frontend_script;
mod frontend_hud_dispatch;
mod frontend_scene3d;
mod frontend_native_camera;
use game_mode::{GameMenuAction, GamePresentationMode};

impl EurochefApp {
    /// Called once before the first frame.
    pub fn new(path: Option<String>, hashcodes_path: Option<String>, cc: &CreationContext<'_>) -> Self {
        Self::new_with_launch(path, hashcodes_path, false, cc)
    }

    pub fn new_with_launch(
        path: Option<String>,
        hashcodes_path: Option<String>,
        auto_launch_game_once: bool,
        cc: &CreationContext<'_>,
    ) -> Self {
        // Install FontAwesome font and place it second
        let mut fonts = FontDefinitions::default();
        fonts.font_data.insert(
            "font_awesome".to_owned(),
            FontData::from_static(include_bytes!("../assets/FontAwesomeSolid.ttf")).into(),
        );

        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(1, "font_awesome".to_owned());

        cc.egui_ctx.set_fonts(fonts);

        let hashcodes = if let Some(hashcodes_path) = hashcodes_path {
            let hfs = std::fs::read_to_string(hashcodes_path).unwrap();
            parse_hashcodes(&hfs)
        } else {
            Default::default()
        };

        let mut s = Self {
            gl: cc.gl.clone().unwrap(),
            state: AppState::Ready,
            current_panel: Panel::FileInfo,
            game_mode: if auto_launch_game_once { GamePresentationMode::TitleScreen } else { GamePresentationMode::Editor },
            auto_launch_game_once,
            game_menu_return_mode: GamePresentationMode::MainMenu,
            game_menu_notice: None,
            game_session_live: false,
            game_menu_assets: None,
            frontend_scene3d: None,
            game_menu_navigation: game_mode::FrontendMenuNavigation::default(),
            game_menu_assets_source: None,
            pending_new_game_slot: None,
            initial_game_slot: None,
            spreadsheetlist: None,
            fileinfo: None,
            textures: None,
            entities: None,
            maps: None,
            scripts: None,
            animations: None,
            sound_preview: None,
            load_input: Arc::new(AtomicCell::new(None)),
            pending_file: None,
            pending_native_level_entry: None,
            current_source_path: None,
            selected_platform: Platform::Ps2,
            ps2_warning: false,
            about_window: false,
            path_cache: Default::default(),
            render_store: Arc::new(RwLock::new(RenderStore::new())),
            hashcodes: Arc::new(hashcodes),
            game: String::new(),
            show_profiler: false,
        };

        if let Some(path) = path {
            match s.load_file_with_path(path) {
                Ok(_) => {}
                Err(e) => {
                    s.state = AppState::Error(e);
                }
            }
        }

        s
    }

    /// Resolve menu art from the indexed original H04 frontend EDB; never
    /// substitute a screenshot or serialize copies into the workspace.
    fn ensure_original_menu_assets(&mut self, ctx: &egui::Context) {
        let source = self.path_cache.get(&menu_assets::ROBOTS_FRONTEND_EDB_UID)
            .map(PathBuf::from)
            .filter(|candidate| candidate.is_file())
            .or_else(|| self.current_source_path.as_ref()
                .and_then(|path| path.parent().map(|parent| parent.join("h04_fron.edb")))
                .filter(|candidate| candidate.is_file()));
        let Some(source) = source else { return; };
        if self.game_menu_assets_source.as_ref() == Some(&source) {
            return;
        }
        if !source.is_file() {
            return;
        }
        self.game_menu_assets_source = Some(source.clone());
        // The frontend scene is a separate GL resource owner: gameplay loads
        // may purge the game's RenderStore without destroying menu rendering.
        self.frontend_scene3d = None;
        let scene_file = source.with_file_name("m00_fron.edb");
        if scene_file.is_file() {
            match frontend_scene3d::FrontendScene3D::from_original_edb(&self.gl, &scene_file) {
                Ok(scene) => {
                    tracing::info!("Original M00 frontend scene loaded: {} script roots", scene.script_count());
                    self.game_menu_navigation.use_captured_camera(scene.has_captured_camera());
                    self.frontend_scene3d = Some(scene);
                }
                Err(error) => tracing::warn!("Unable to load M00 frontend scene {}: {error:#}", scene_file.display()),
            }
        }
        match menu_assets::RobotsMenuAssets::from_original_edb(ctx, &source) {
            Ok(assets) => {
                tracing::info!("Robots Game Mode art loaded from original {}", source.display());
                self.game_menu_assets = Some(assets);
            }
            Err(error) => {
                self.game_menu_assets = None;
                tracing::warn!("Cannot decode original Robots menu {}: {error:#}", source.display());
                self.game_menu_notice = Some(
                    format!("Original menu textures unavailable: {error:#}")
                );
            }
        }
    }

    /// Game Mode actions share the existing file/Player/TriggerManager host.
    /// A checkpoint is intentionally a progress-only save, not an invented
    /// serialization of dormant NPC/XItem ownership.
    fn handle_game_menu_action(
        &mut self, action: GameMenuAction, ctx: &egui::Context,
    ) -> anyhow::Result<()> {
        use eurochef_shared::robots_runtime::change_level::{
            RobotsChangeLevelRequest, RobotsChangeLevelTarget,
        };
        match action {
            GameMenuAction::EnterMainMenu => {
                self.game_mode = GamePresentationMode::MainMenu;
            }
            GameMenuAction::OpenNewGameSlots => {
                self.game_mode = GamePresentationMode::NewGameSlots;
            }
            GameMenuAction::ChooseNewGameSlot(slot) => {
                if checkpoint::slot_exists(slot) {
                    self.pending_new_game_slot = Some(slot);
                    self.game_mode = GamePresentationMode::ConfirmOverwrite;
                } else {
                    self.handle_game_menu_action(GameMenuAction::NewGame, ctx)?;
                    self.initial_game_slot = Some(slot);
                }
            }
            GameMenuAction::ConfirmOverwrite => {
                let slot = self.pending_new_game_slot
                    .context("No pending save slot selected for overwrite")?;
                self.handle_game_menu_action(GameMenuAction::NewGame, ctx)?;
                self.pending_new_game_slot = None;
                self.initial_game_slot = Some(slot);
            }
            GameMenuAction::NewGame => {
                self.initial_game_slot = None;
                anyhow::ensure!(
                    self.maps.as_ref().is_some_and(|panel| panel.has_unique_player_entry_context(0)),
                    "New Game requires one verified context-0 Player in the loaded level"
                );
                self.queue_native_change_level(
                    RobotsChangeLevelRequest {
                        target: RobotsChangeLevelTarget::CurrentLevel,
                        player_entry_context: 0,
                    },
                    None,
                    ctx.input(|input| input.time),
                )?;
                self.game_session_live = false;
                self.game_mode = GamePresentationMode::Launching;
            }
            GameMenuAction::Continue => {
                anyhow::ensure!(self.game_session_live, "No running Player session");
                self.game_mode = GamePresentationMode::Running;
            }
            GameMenuAction::Save(slot) => {
                anyhow::ensure!(self.game_session_live, "No live game to save");
                let level_edb_uid = self.fileinfo.as_ref().context("No active level")?.header.hashcode;
                let (player_entry_context, progress) = self.maps.as_ref()
                    .and_then(|maps| maps.game_checkpoint_progress())
                    .context("Checkpoint requires an active gameplay Player and idle Mission owner")?;
                checkpoint::save_slot(slot, &checkpoint::RobotsCheckpoint {
                    format_version: checkpoint::FORMAT_VERSION,
                    level_edb_uid,
                    player_entry_context,
                    progress,
                })?;
                self.game_menu_notice = Some(format!(
                    "Checkpoint slot {slot} saved (persistent progress only; NPC/XItem states not saved)."
                ));
            }
            GameMenuAction::Load(slot) => {
                let loaded = checkpoint::load_slot(slot)?;
                let active_uid = self.fileinfo.as_ref().context("No active EDB")?.header.hashcode;
                let target = if loaded.level_edb_uid == active_uid {
                    RobotsChangeLevelTarget::CurrentLevel
                } else {
                    RobotsChangeLevelTarget::Level(loaded.level_edb_uid)
                };
                self.queue_native_change_level(
                    RobotsChangeLevelRequest {
                        target,
                        player_entry_context: loaded.player_entry_context,
                    },
                    Some(loaded.progress),
                    ctx.input(|input| input.time),
                )?;
                self.game_session_live = false;
                self.game_mode = GamePresentationMode::Launching;
            }
            GameMenuAction::MainMenu => self.game_mode = GamePresentationMode::MainMenu,
            GameMenuAction::Back => {
                self.game_mode = match self.game_mode {
                    GamePresentationMode::NewGameSlots => GamePresentationMode::MainMenu,
                    GamePresentationMode::ConfirmOverwrite => GamePresentationMode::NewGameSlots,
                    _ => self.game_menu_return_mode,
                };
            },
            GameMenuAction::OpenSaveSlots => {
                self.game_menu_return_mode = self.game_mode;
                self.game_mode = GamePresentationMode::SaveSlots;
            }
            GameMenuAction::OpenLoadSlots => {
                self.game_menu_return_mode = self.game_mode;
                self.game_mode = GamePresentationMode::LoadSlots;
            }
            GameMenuAction::OpenSettings => {
                self.game_menu_return_mode = self.game_mode;
                self.game_mode = GamePresentationMode::Settings;
            }
            GameMenuAction::ChooseData => {
                #[cfg(not(target_arch = "wasm32"))]
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Robots EDB", &["edb"])
                    .pick_file()
                {
                    self.load_file_with_path(path)?;
                    self.game_menu_notice = Some("Loading selected EDB resources...".to_string());
                }
            }
            GameMenuAction::Editor => {
                self.game_mode.editor();
                self.current_panel = Panel::Maps;
            }
            GameMenuAction::Exit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
        Ok(())
    }

    /// The chosen new-game slot receives its first checkpoint only after a
    /// live native Player/Camera/TriggerManager frame has been admitted.
    fn try_write_initial_game_slot(&mut self) {
        let Some(slot) = self.initial_game_slot else { return; };
        let Some(level_edb_uid) = self.fileinfo.as_ref().map(|file| file.header.hashcode) else { return; };
        let Some((player_entry_context, progress)) = self.maps.as_ref()
            .and_then(|panel| panel.game_checkpoint_progress()) else { return; };
        self.initial_game_slot = None;
        match checkpoint::save_slot(slot, &checkpoint::RobotsCheckpoint {
            format_version: checkpoint::FORMAT_VERSION,
            level_edb_uid, player_entry_context, progress,
        }) {
            Ok(()) => {
                self.game_menu_notice = Some(format!("New game checkpoint created in slot {slot}"));
            }
            Err(error) => {
                self.game_menu_notice = Some(format!("Cannot save new-game slot {slot}: {error:#}"));
            }
        }
    }

    /// Consume a real ChangeLevel event. Only an exactly indexed EDB UID can
    /// be opened. A Level UID that isn't an EDB identity must fail closed until
    /// the native Level->EDB lookup is reconstructed, never guess a sub-map.
    fn queue_native_change_level(
        &mut self,
        request: eurochef_shared::robots_runtime::change_level::RobotsChangeLevelRequest,
        progress: Option<eurochef_shared::robots_runtime::change_level::RobotsLevelTransitionProgress>,
        wall_time: f64,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.pending_native_level_entry.is_none() && self.pending_file.is_none(),
            "A level/file load is already pending"
        );
        let current_uid = self
            .fileinfo
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No active EDB for ChangeLevel"))?
            .header.hashcode;
        let target_uid = request
            .target_level_uid(Some(current_uid))
            .ok_or_else(|| anyhow::anyhow!("ChangeLevel target UID is unresolved"))?;
        // Native shipped 0x004DDEF0 resolves Level UIDs by exact EDB file UID.
        // Do not make that identity assumption for custom/editor EDBs.
        if progress.is_some() {
            anyhow::ensure!(
                eurochef_shared::robots_runtime::change_level::robots_shipped_level_edb_uid(target_uid)
                    == Some(target_uid),
                "ChangeLevel UID {target_uid:#010x} is absent from the shipped Level catalog"
            );
        }
        // 0x0048C6A0 does not reload EDB for non-forced same-level events.
        // Preserve the existing Player for the active sub-map; a unique
        // destination on a different sub-map performs a bounded owner reset.
        if request.needs_full_level_reload(Some(current_uid)) == Some(false) {
            anyhow::ensure!(
                progress.is_some(),
                "Same-level ChangeLevel requires an admitted gameplay event"
            );
            let (map_index, player_index) = self.maps.as_mut()
                .and_then(|maps| maps.change_same_level_player_context(
                    request.player_entry_context, wall_time, progress.clone()?,
                ))
                .ok_or_else(|| anyhow::anyhow!(
                    "Same-level context {} is ambiguous or has no valid live Player destination",
                    request.player_entry_context,
                ))?;
            tracing::info!(
                "Same-level ChangeLevel retained EDB {current_uid:#010x}, sub-map #{map_index}, Player #{player_index}, context {}",
                request.player_entry_context,
            );
            return Ok(());
        }

        // 0x0048C6A0 has a distinct same-level non-forced branch:
        // GameWnd setter + current-level owner callback, NOT a level reload.
        // Until that owner-reset handshake is ported, do not silently destroy
        // the loaded map and call it an equivalent level transition.
        anyhow::ensure!(
            request.needs_full_level_reload(Some(current_uid)) == Some(true),
            "Non-forced same-level ChangeLevel requires a GameWnd owner reset, not an EDB reload"
        );
        let target_path = if target_uid == current_uid {
            self.current_source_path.clone()
        } else {
            self.path_cache.get(&target_uid).map(PathBuf::from)
        }
        .ok_or_else(|| anyhow::anyhow!(
            "ChangeLevel target {target_uid:#010x} has no indexed EDB; not guessing level mapping"
        ))?;
        self.pending_native_level_entry = Some(PendingNativeLevelEntry {
            target_edb_uid: target_uid,
            player_entry_context: request.player_entry_context,
            progress,
        });
        if let Err(error) = self.load_file_with_path(&target_path) {
            self.pending_native_level_entry = None;
            return Err(error);
        }
        Ok(())
    }

    // TODO: Error handling
    pub fn load_file_with_path<P: AsRef<std::path::Path>>(
        &mut self,
        path: P,
    ) -> anyhow::Result<()> {
        let path = path.as_ref().to_path_buf();
        let platform = Platform::from_path(&path);
        self.current_source_path = Some(path.clone());

        match eurochef_edb::robots_texture_identity::install_discovered(&path) {
            Ok(Some(catalog_path)) => tracing::info!(
                "Loaded Robots Texture identity catalog {}",
                catalog_path.display()
            ),
            Ok(None) => tracing::debug!(
                "No Robots Texture identity catalog discovered for {}",
                path.display()
            ),
            Err(error) => {
                eurochef_edb::robots_texture_identity::clear();
                tracing::warn!(
                    "Failed to load Robots Texture identity catalog for {}: {error:#}",
                    path.display()
                );
            }
        }

        // ROBOTS_PATCH_0023_PIPELINE_REFERENCE_INDEX
        // Rebuild the cache for every explicitly opened EDB. Otherwise opening a
        // file from _eurotools_out can leave no usable external-reference index.
        self.path_cache.clear();

        if let Some(dissected_path) = DissectedFilelistPath::dissect(&path) {
            self.game = dissected_path.game.clone();

            self.hashcodes = Arc::new(eurochef_shared::filesystem::load_hashcodes(
                &dissected_path,
                true,
            ));

            // Index the folder and load it into the path cache
            info!(
                "Indexing game folder {}",
                dissected_path.dir_relative().to_string_lossy()
            );
            self.path_cache.clear();

            for entry in glob::glob(&format!(
                "{}/*.edb",
                dissected_path.dir_absolute().to_string_lossy()
            ))? {
                match entry {
                    Ok(path) => {
                        let file = File::open(&path)?;
                        let mut reader = BufReader::new(file);
                        let endian = if reader.read_ne::<u8>()? == 0x47 {
                            Endian::Big
                        } else {
                            Endian::Little
                        };
                        reader.seek(std::io::SeekFrom::Start(4))?;
                        let hashcode: Hashcode = reader.read_type(endian)?;
                        self.path_cache
                            .insert(hashcode, path.to_string_lossy().to_string());
                    }
                    Err(e) => println!("{:?}", e),
                }
            }

            info!("Indexed {} EDBs", self.path_cache.len());
        }

        // ROBOTS_PATCH_0023_PIPELINE_REFERENCE_INDEX_MANIFEST
        // ROBOTS_PATCH_0023_REV3_EXPLICIT_MANIFEST_ENV
        let manifest_path = std::env::var_os("ROBOTS_EDB_MANIFEST")
            .map(std::path::PathBuf::from)
            .filter(|p| p.is_file())
            .or_else(|| {
                path.ancestors()
                    .find(|p| {
                        p.file_name()
                            .and_then(|n| n.to_str())
                            .map(|n| n.eq_ignore_ascii_case("_eurotools_out"))
                            .unwrap_or(false)
                    })
                    .map(|out_root| out_root.join("edb").join("manifest.tsv"))
                    .filter(|p| p.is_file())
            });

        if let Some(manifest_path) = manifest_path {
            match std::fs::read_to_string(&manifest_path) {
                Ok(manifest) => {
                    let mut indexed_from_manifest = 0usize;
                    let mut missing_sources = 0usize;

                    for line in manifest.lines().skip(1) {
                        let Some((_, source_path)) = line.split_once('\t') else {
                            continue;
                        };

                        let source_path = source_path.trim();
                        if source_path.is_empty() {
                            continue;
                        }

                        let source = std::path::Path::new(source_path);
                        if !source.is_file() {
                            missing_sources += 1;
                            continue;
                        }

                        let file = match File::open(source) {
                            Ok(file) => file,
                            Err(e) => {
                                warn!(
                                    "Could not open EDB from pipeline manifest '{}': {}",
                                    source.display(),
                                    e
                                );
                                continue;
                            }
                        };

                        let mut reader = BufReader::new(file);
                        let endian = match reader.read_ne::<u8>() {
                            Ok(0x47) => Endian::Big,
                            Ok(_) => Endian::Little,
                            Err(e) => {
                                warn!(
                                    "Could not read EDB endian marker from '{}': {}",
                                    source.display(),
                                    e
                                );
                                continue;
                            }
                        };

                        if let Err(e) = reader.seek(std::io::SeekFrom::Start(4)) {
                            warn!("Could not seek EDB header in '{}': {}", source.display(), e);
                            continue;
                        }

                        let hashcode: Hashcode = match reader.read_type(endian) {
                            Ok(hashcode) => hashcode,
                            Err(e) => {
                                warn!(
                                    "Could not read EDB hashcode from '{}': {}",
                                    source.display(),
                                    e
                                );
                                continue;
                            }
                        };

                        self.path_cache
                            .insert(hashcode, source.to_string_lossy().to_string());
                        indexed_from_manifest += 1;
                    }

                    info!(
                        "Robots manifest index: path='{}' indexed={} missing={} cache_total={}",
                        manifest_path.display(),
                        indexed_from_manifest,
                        missing_sources,
                        self.path_cache.len()
                    );
                    println!(
                        "[Robots] manifest index: {} EDBs, {} missing, cache {}",
                        indexed_from_manifest,
                        missing_sources,
                        self.path_cache.len()
                    );
                }
                Err(e) => {
                    warn!(
                        "Robots pipeline manifest not readable '{}': {}",
                        manifest_path.display(),
                        e
                    );
                    println!(
                        "[Robots] manifest read failed: {} ({})",
                        manifest_path.display(),
                        e
                    );
                }
            }
        } else {
            warn!("Robots EDB manifest not discovered; external references may remain unresolved.");
            println!(
                "[Robots] manifest not discovered; external references may remain unresolved."
            );
        }

        let mut f = File::open(path)?;
        let mut data = vec![];
        f.read_to_end(&mut data)?;
        self.pending_file = Some((data, platform));

        Ok(())
    }
}
