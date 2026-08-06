#[macro_use]
extern crate tracing;

mod edb;
mod filelist;

use anyhow::Context;
use clap::{Parser, Subcommand};
use clap_num::maybe_hex;
use eurochef_edb::versions::Platform;
use tracing::metadata::LevelFilter;
use tracing_subscriber::{
    prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt, EnvFilter,
};

#[derive(clap::ValueEnum, PartialEq, Debug, Clone)]
pub enum PlatformArg {
    Pc,
    Xb,
    Xbox,
    Xbox360,
    Ps2,
    Ps3,
    Gc,
    Gamecube,
    Wii,
    WiiU,
}

impl From<PlatformArg> for Platform {
    fn from(val: PlatformArg) -> Self {
        match val {
            PlatformArg::Pc => Platform::Pc,
            PlatformArg::Xbox | PlatformArg::Xb => Platform::Xbox,
            PlatformArg::Xbox360 => Platform::Xbox360,
            PlatformArg::Ps2 => Platform::Ps2,
            PlatformArg::Ps3 => Platform::Ps3,
            PlatformArg::Gamecube | PlatformArg::Gc => Platform::GameCube,
            PlatformArg::Wii => Platform::Wii,
            PlatformArg::WiiU => Platform::WiiU,
        }
    }
}

#[derive(Parser, Debug)]
struct Args {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Commands for working with filelists
    Filelist {
        #[command(subcommand)]
        subcommand: FilelistCommand,
    },
    Edb {
        #[command(subcommand)]
        subcommand: EdbCommand,
    },
}

#[derive(Subcommand, Debug, Clone)]
enum EdbCommand {
    /// Extract entities
    Entities {
        /// .edb file to read
        filename: String,

        /// Output folder for textures (default: "./entities/{filename}/")
        output_folder: Option<String>,

        /// Override for platform detection
        #[arg(value_enum, short, long, ignore_case = true)]
        platform: Option<PlatformArg>,

        /// Don't embed textures into the output file
        #[arg(short = 'e', long)]
        no_embed: bool,

        /// Remove transparent surfaces
        #[arg(short = 't', long)]
        no_transparent: bool,
    },
    /// Extract spreadsheets
    Spreadsheets {
        /// .edb file to read
        filename: String,

        /// Output folder for spreadsheet (default: "./spreadsheets/{filename}/")
        output_folder: Option<String>,
    },
    /// Extract maps
    Maps {
        /// .edb file to read
        filename: String,

        /// Output folder for maps (default: "./maps/{filename}/")
        output_folder: Option<String>,

        /// Override for platform detection
        #[arg(value_enum, short, long, ignore_case = true)]
        platform: Option<PlatformArg>,

        /// File with trigger definitions (assets/triggers_*.yml)
        #[arg(short, long)]
        trigger_defs: Option<String>,
    },
    /// Build a shipped-corpus EXGeoParticle structural report from a pipeline manifest
    ParticleReport {
        /// Pipeline manifest.tsv containing EDB UID and source path columns
        manifest: String,

        /// Output folder (default: "./particle_corpus_report/")
        output_folder: Option<String>,
    },
    /// Build a canonical textures/animations/scripts/entities atlas from a pipeline manifest
    ResourceAtlas {
        /// Pipeline manifest.tsv containing EDB UID and source path columns
        manifest: String,

        /// Output folder (default: "./resource_atlas/")
        output_folder: Option<String>,
    },
    /// Build a shipped-corpus AnimScript health report from a pipeline manifest
    ScriptHealth {
        /// Pipeline manifest.tsv containing EDB UID and source path columns
        manifest: String,

        /// Output folder (default: "./script_health_report/")
        output_folder: Option<String>,
    },
    /// Build a shipped-corpus XTrigger report from a pipeline manifest
    TriggerReport {
        /// Pipeline manifest.tsv containing EDB UID and source path columns
        manifest: String,

        /// Output folder (default: "./xtrigger_corpus_report/")
        output_folder: Option<String>,

        /// File with trigger definitions (assets/triggers_*.yml)
        #[arg(short, long)]
        trigger_defs: Option<String>,
    },
    /// Build a shipped-corpus HT_Entity structural report from a pipeline manifest
    EntityReport {
        /// Pipeline manifest.tsv containing EDB UID and source path columns
        manifest: String,

        /// Output folder (default: "./ht_entity_corpus_report/")
        output_folder: Option<String>,
    },
    /// Build a shipped-corpus Animation -> AnimSkin -> Entity binding report
    AnimBindingReport {
        /// Pipeline manifest.tsv containing EDB UID and source path columns
        manifest: String,

        /// Output folder (default: "./anim_binding_corpus_report/")
        output_folder: Option<String>,
    },
    /// Export Robots character models and animation-only clips through Autodesk FBX SDK
    FbxCharacters {
        /// .edb file containing AnimSkin character resources
        filename: String,

        /// Output folder (default: "./fbx/{filename}/")
        output_folder: Option<String>,

        /// Override for platform detection; only the proved PC layout is supported
        #[arg(value_enum, short, long, ignore_case = true)]
        platform: Option<PlatformArg>,

        /// Explicit path to fbx_export_helper.exe
        #[arg(long)]
        exporter: Option<String>,

        /// Keep the canonical .ecfbx and .fbxscene.json intermediate files
        #[arg(long)]
        keep_ir: bool,

        /// Build and validate only the canonical character/animation IR; do not invoke Autodesk FBX SDK
        #[arg(long)]
        ir_only: bool,

        /// Manifest of all EDB files used to resolve map/Script Animation and AnimSkin references across files
        #[arg(long)]
        script_manifest: Option<String>,

        /// Explicit FPS for clips with no valid in-EDB AnimScript timing reference
        #[arg(long)]
        unreferenced_animation_fps: Option<f32>,

        /// Replace existing FBX and report files
        #[arg(long)]
        overwrite: bool,
    },
    /// Extract textures
    Textures {
        /// .edb file to read
        filename: String,

        /// Output folder for textures (default: "./textures/{filename}/")
        output_folder: Option<String>,

        /// Override for platform detection
        #[arg(value_enum, short, long, ignore_case = true)]
        platform: Option<PlatformArg>,

        /// Output file format to use (supported: tga, png, qoi)
        /// Selecting PNG will export animated textures as APNGs (unless disabled)
        #[arg(short, long, default_value("tga"))]
        format: String,

        /// Don't export APNGs when using PNG as output format
        #[arg(long)]
        no_apngs: bool,
    },
    /// Extract animations (!!MAJOR WIP!!)
    Animations {
        /// .edb file to read
        filename: String,

        /// Output folder for textures (default: "./entities/{filename}/")
        output_folder: Option<String>,

        // TODO(cohae): can we move this up to the edb command?
        /// Override for platform detection
        #[arg(value_enum, short, long, ignore_case = true)]
        platform: Option<PlatformArg>,
    },
}

#[derive(Subcommand, Debug, Clone)]
enum FilelistCommand {
    /// Extract a filelist
    Extract {
        /// .bin file to use (don't use a .000 file)
        filename: String,

        /// The folder to extract to (will be created if it doesnt exist)
        #[arg(default_value = "./")]
        output_folder: String,

        /// Create a .scr file in the output folder listing the contents in the right order, for future repacking
        #[arg(short = 's', long)]
        create_scr: bool,
    },
    /// Create a new filelist from a folder
    Create {
        /// Folder to read files from
        input_folder: String,

        /// Destination for the generated filelist (without filename extension)
        #[arg(default_value = "./Filelist")]
        output_file: String,

        #[arg(long, short = 'l', default_value_t = 'x')]
        drive_letter: char,

        /// Supported versions: 5, 6, 7
        #[arg(long, short, default_value_t = 7)]
        version: u32,

        #[arg(value_enum, short, long, ignore_case = true)]
        platform: PlatformArg,

        /// Maximum size per data file, might be overridden by a .scr file
        #[arg(long, short = 'z', default_value_t = 0x80000000, value_parser = maybe_hex::<u32>)]
        split_size: u32,

        /// .scr file to read options from (currently doesnt support wildcards)
        #[arg(long, short)]
        scr_file: Option<String>,
    },
}

pub fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().without_time())
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .with_env_var("EUROCHEF_LOG")
                .from_env_lossy(),
        )
        .init();

    let args = Args::parse();

    match &args.cmd {
        Command::Filelist { subcommand } => handle_filelist(subcommand.clone()),
        Command::Edb { subcommand } => handle_edb(subcommand.clone()),
    }
}

fn handle_edb(cmd: EdbCommand) -> anyhow::Result<()> {
    match cmd {
        EdbCommand::Entities {
            filename,
            output_folder,
            platform,
            no_embed,
            no_transparent,
        } => edb::entities::execute_command(
            filename,
            platform,
            output_folder,
            no_embed,
            no_transparent,
        ),
        EdbCommand::Maps {
            filename,
            platform,
            output_folder,
            trigger_defs,
        } => edb::maps::execute_command(filename, platform, output_folder, trigger_defs),
        EdbCommand::ParticleReport {
            manifest,
            output_folder,
        } => edb::particle_report::execute_command(manifest, output_folder),
        EdbCommand::ResourceAtlas {
            manifest,
            output_folder,
        } => edb::resource_atlas::execute_command(manifest, output_folder),
        EdbCommand::ScriptHealth {
            manifest,
            output_folder,
        } => edb::script_health::execute_command(manifest, output_folder),
        EdbCommand::TriggerReport {
            manifest,
            output_folder,
            trigger_defs,
        } => edb::trigger_report::execute_command(manifest, output_folder, trigger_defs),
        EdbCommand::EntityReport {
            manifest,
            output_folder,
        } => edb::entity_report::execute_command(manifest, output_folder),
        EdbCommand::AnimBindingReport {
            manifest,
            output_folder,
        } => edb::anim_binding_report::execute_command(manifest, output_folder),
        EdbCommand::FbxCharacters {
            filename,
            output_folder,
            platform,
            exporter,
            keep_ir,
            ir_only,
            script_manifest,
            unreferenced_animation_fps,
            overwrite,
        } => edb::fbx_characters::execute_command(
            filename,
            platform,
            output_folder,
            exporter,
            keep_ir,
            ir_only,
            script_manifest,
            unreferenced_animation_fps,
            overwrite,
        ),
        EdbCommand::Spreadsheets {
            filename,
            output_folder,
        } => edb::spreadsheets::execute_command(filename, output_folder),
        EdbCommand::Textures {
            filename,
            platform,
            output_folder,
            format,
            no_apngs,
        } => edb::textures::execute_command(filename, platform, output_folder, format, no_apngs),
        EdbCommand::Animations {
            filename,
            platform,
            output_folder,
        } => edb::animations::execute_command(filename, platform, output_folder),
    }
}

fn handle_filelist(cmd: FilelistCommand) -> anyhow::Result<()> {
    match cmd {
        FilelistCommand::Extract {
            filename,
            output_folder,
            create_scr,
        } => filelist::extract::execute_command(filename, output_folder, create_scr)
            .context("Failed to extract filelist"),
        FilelistCommand::Create {
            input_folder,
            output_file,
            drive_letter,
            version,
            platform,
            split_size,
            scr_file,
        } => filelist::create::execute_command(
            input_folder,
            output_file,
            drive_letter,
            version,
            platform,
            split_size,
            scr_file,
        )
        .context("Failed to create filelist"),
    }
}
