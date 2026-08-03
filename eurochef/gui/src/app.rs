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

pub struct EurochefApp {
    gl: Arc<glow::Context>,

    state: AppState,
    current_panel: Panel,

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

impl EurochefApp {
    /// Called once before the first frame.
    pub fn new(
        path: Option<String>,
        hashcodes_path: Option<String>,
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

    // TODO: Error handling
    pub fn load_file_with_path<P: AsRef<std::path::Path>>(
        &mut self,
        path: P,
    ) -> anyhow::Result<()> {
        let path = path.as_ref().to_path_buf();
        let platform = Platform::from_path(&path);
        self.current_source_path = Some(path.clone());

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
