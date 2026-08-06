use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufReader, Read, Seek, SeekFrom, Write},
    mem::size_of,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{bail, ensure, Context};
use eurochef_edb::{
    anim::{EXGeoAnimSkinEntity, EXGeoBaseAnimSkin},
    binrw::BinReaderExt,
    edb::EdbFile,
    entity::EXGeoEntity,
    versions::Platform,
    HashcodeUtils,
};
use eurochef_shared::{
    entities::{read_entity, TriStrip, UXVertex},
    script::{UXGeoScript, UXGeoScriptCommandData},
};
use serde_json::json;

use crate::PlatformArg;

const IR_MAGIC: &[u8; 8] = b"ECFBX002";
const IR_VERSION: u32 = 2;
const MAX_ENTITY_DEPTH: u32 = 32;
const MAX_MOTION_BYTES: usize = 64 * 1024 * 1024;
const WEIGHT_EPSILON: f32 = 1.0e-4;
const POSE_CACHE_MAGIC: &[u8; 8] = b"RAPCV002";
const POSE_CACHE_HEADER_SIZE: usize = 40;
const POSE_CACHE_VALUES_PER_BONE: usize = 7;
const POSE_CACHE_BYTES_PER_BONE: usize = POSE_CACHE_VALUES_PER_BONE * size_of::<f32>();
const MAX_POSE_CACHE_BYTES: usize = 512 * 1024 * 1024;

#[derive(Debug, Clone)]
struct BoneIr {
    name: String,
    parent: i32,
    local_position: [f32; 3],
    global_position: [f32; 3],
}

#[derive(Debug, Clone, Copy)]
struct InfluenceIr {
    bone_indices: [u16; 4],
    weights: [f32; 4],
}

#[derive(Debug, Clone)]
struct MaterialIr {
    hashcode: u32,
    name: String,
}

#[derive(Debug, Clone)]
struct MeshIr {
    name: String,
    vertices: Vec<UXVertex>,
    influences: Vec<InfluenceIr>,
    indices: Vec<u32>,
    triangle_materials: Vec<u32>,
    materials: Vec<MaterialIr>,
}

#[derive(Debug, Clone, Copy)]
struct PoseIr {
    position: [f32; 3],
    rotation: [f32; 4],
    scale: [f32; 3],
}

impl PoseIr {
    const IDENTITY: Self = Self {
        position: [0.0; 3],
        rotation: [0.0, 0.0, 0.0, 1.0],
        scale: [1.0; 3],
    };
}

#[derive(Debug, Clone)]
struct AnimationClipIr {
    name: String,
    animation_uid: u32,
    animation_source_edb_uid: u32,
    animation_source_path: PathBuf,
    source_animation_index: u32,
    source_script_edb_uid: u32,
    source_script_uid: u32,
    source_script_command: u32,
    usage_count: u32,
    source_script_fps: f32,
    source_command_length: u32,
    sample_rate: f32,
    frame_count: u32,
    duration_seconds: f32,
    root_motion_mode: String,
    pose_cache_path: PathBuf,
    poses: Vec<PoseIr>,
}

#[derive(Debug, Clone)]
struct SkippedAnimationIr {
    animation_uid: u32,
    animation_source_edb_uid: u32,
    animation_source_path: PathBuf,
    source_animation_index: usize,
    reason: String,
}

#[derive(Debug, Clone)]
struct CharacterIr {
    name: String,
    source_edb_uid: u32,
    animskin_uid: u32,
    bones: Vec<BoneIr>,
    meshes: Vec<MeshIr>,
    clips: Vec<AnimationClipIr>,
    skipped_animations: Vec<SkippedAnimationIr>,
}

#[derive(Debug, Clone, Copy)]
struct AnimationUsageTiming {
    script_edb_uid: u32,
    script_uid: u32,
    command_index: usize,
    script_fps: f32,
    command_length: u16,
}

#[derive(Debug, Default)]
struct AnimationUsageCatalog {
    valid: HashMap<usize, Vec<AnimationUsageTiming>>,
    invalid: HashMap<usize, Vec<String>>,
}

#[derive(Debug, Clone)]
struct CorpusFileCatalog {
    uid: u32,
    source_path: PathBuf,
    animations: Vec<u32>,
    skins: Vec<u32>,
}

#[derive(Debug, Clone)]
struct ScriptAnimationBinding {
    script_edb_uid: u32,
    script_uid: u32,
    command_index: usize,
    script_fps: f32,
    command_length: u16,
    animation_source_edb_uid: u32,
    animation_source_path: PathBuf,
    animation_index: usize,
    animation_uid: u32,
    skin_source_edb_uid: u32,
    skin_uid: u32,
}

#[derive(Debug, Default)]
struct CorpusAnimationCatalog {
    target_animation_usages: AnimationUsageCatalog,
    explicit_skin_bindings: Vec<ScriptAnimationBinding>,
    files_scanned: usize,
    animation_commands: usize,
    unresolved_commands: usize,
}

#[derive(Debug, Clone)]
struct PoseCacheData {
    source_path: PathBuf,
    frame_count: usize,
    bone_count: usize,
    poses: Vec<PoseIr>,
}

#[derive(Debug, Clone, Copy)]
struct TimingVariant {
    source_script_edb_uid: u32,
    source_script_uid: u32,
    source_script_command: u32,
    usage_count: u32,
    source_script_fps: f32,
    source_command_length: u32,
    sample_rate: f32,
    duration_seconds: f32,
}

pub fn execute_command(
    filename: String,
    platform: Option<PlatformArg>,
    output_folder: Option<String>,
    exporter: Option<String>,
    keep_ir: bool,
    ir_only: bool,
    script_manifest: Option<String>,
    unreferenced_animation_fps: Option<f32>,
    overwrite: bool,
) -> anyhow::Result<()> {
    let platform = platform
        .map(Into::into)
        .or_else(|| Platform::from_path(&filename))
        .context("failed to detect EDB platform")?;
    ensure!(
        platform == Platform::Pc,
        "FBX character export currently supports the proved Robots PC AnimSkin layout only"
    );
    if let Some(fps) = unreferenced_animation_fps {
        ensure!(
            fps.is_finite() && fps > f32::EPSILON,
            "--unreferenced-animation-fps must be finite and greater than zero"
        );
    }

    let source_path = PathBuf::from(&filename);
    let default_folder = source_path
        .file_name()
        .map(|name| PathBuf::from("./fbx").join(name))
        .unwrap_or_else(|| PathBuf::from("./fbx/characters"));
    let output_folder = output_folder.map(PathBuf::from).unwrap_or(default_folder);
    fs::create_dir_all(&output_folder)
        .with_context(|| format!("failed to create {}", output_folder.display()))?;

    let exporter = if ir_only {
        None
    } else {
        Some(resolve_exporter(exporter.as_deref())?)
    };
    let file = File::open(&source_path)
        .with_context(|| format!("failed to open {}", source_path.display()))?;
    let mut edb = EdbFile::new(Box::new(BufReader::new(file)), platform)?;
    let header = edb.header.clone();
    ensure!(
        header.animskin_list.len() != 0,
        "{} contains no AnimSkin records",
        source_path.display()
    );

    let scripts =
        UXGeoScript::read_all(&mut edb).context("failed to read AnimScripts for FBX timing")?;
    let mut usage_catalog = collect_animation_usages(&header, &scripts);
    let corpus_catalog = if let Some(manifest_path) = script_manifest.as_deref() {
        let catalog = collect_corpus_animation_bindings(Path::new(manifest_path), &header)?;
        merge_animation_usages(&mut usage_catalog, &catalog.target_animation_usages);
        info!(
            files = catalog.files_scanned,
            animation_commands = catalog.animation_commands,
            explicit_target_skin_bindings = catalog.explicit_skin_bindings.len(),
            unresolved_commands = catalog.unresolved_commands,
            "collected cross-EDB Script animation bindings"
        );
        catalog
    } else {
        CorpusAnimationCatalog::default()
    };
    let ir_folder = output_folder.join(".fbx-ir");
    fs::create_dir_all(&ir_folder)?;
    let mut exported_models = 0usize;
    let mut exported_clips = 0usize;
    let mut skipped_clips = 0usize;

    for (skin_index, skin_header) in header.animskin_list.iter().enumerate() {
        edb.seek(SeekFrom::Start(skin_header.common.address as u64))?;
        let skin = edb
            .read_type_args::<EXGeoBaseAnimSkin>(edb.endian, (header.version,))
            .with_context(|| format!("failed to parse AnimSkin index {skin_index}"))?;
        let ir = build_character_ir(
            &mut edb,
            &skin,
            skin_header.common.hashcode,
            skin_header.base_skin_num,
            &source_path,
            &usage_catalog,
            &corpus_catalog.explicit_skin_bindings,
            unreferenced_animation_fps,
        )
        .with_context(|| format!("failed to build AnimSkin index {skin_index}"))?;
        validate_character_ir(&ir)?;

        let character_base = format!("{}_[0x{:08X}]", sanitize_name(&ir.name), ir.animskin_uid);
        let model_stem = format!("{character_base}_SK");
        let ir_path = ir_folder.join(format!("{model_stem}.ecfbx"));
        let manifest_path = ir_folder.join(format!("{model_stem}.fbxscene.json"));
        let model_fbx_path = output_folder.join(format!("{model_stem}.fbx"));
        let model_report_path = output_folder.join(format!("{model_stem}.fbx.report.json"));
        let animation_outputs = ir
            .clips
            .iter()
            .enumerate()
            .map(|(clip_index, clip)| {
                let stem = animation_output_stem(
                    &character_base,
                    ir.source_edb_uid,
                    clip,
                    clip_index,
                    &ir.clips,
                );
                (
                    output_folder.join(format!("{stem}.fbx")),
                    output_folder.join(format!("{stem}.fbx.report.json")),
                )
            })
            .collect::<Vec<_>>();

        let would_overwrite = if ir_only {
            ir_path.exists() || manifest_path.exists()
        } else {
            model_fbx_path.exists()
                || model_report_path.exists()
                || animation_outputs
                    .iter()
                    .any(|(fbx_path, report_path)| fbx_path.exists() || report_path.exists())
        };
        if !overwrite && would_overwrite {
            bail!("refusing to overwrite output for {model_stem}; pass --overwrite");
        }

        write_ir(&ir_path, &ir)?;
        write_ir_manifest(
            &manifest_path,
            &source_path,
            &ir,
            &ir_path,
            &model_fbx_path,
            &animation_outputs,
        )?;

        skipped_clips += ir.skipped_animations.len();
        if ir_only {
            exported_models += 1;
            exported_clips += ir.clips.len();
            info!(
                animskin = format_args!("0x{:08X}", ir.animskin_uid),
                clips = ir.clips.len(),
                skipped = ir.skipped_animations.len(),
                ir = %ir_path.display(),
                "validated FBX character and animation IR"
            );
            continue;
        }

        let exporter = exporter.as_ref().context("FBX exporter was not resolved")?;
        invoke_helper(
            exporter,
            &["model".to_string()],
            &ir_path,
            &model_fbx_path,
            &model_report_path,
            ir.animskin_uid,
        )?;
        exported_models += 1;
        info!(
            animskin = format_args!("0x{:08X}", ir.animskin_uid),
            output = %model_fbx_path.display(),
            "exported FBX character model"
        );

        for (clip_index, (fbx_path, report_path)) in animation_outputs.iter().enumerate() {
            invoke_helper(
                exporter,
                &["animation".to_string(), clip_index.to_string()],
                &ir_path,
                fbx_path,
                report_path,
                ir.clips[clip_index].animation_uid,
            )?;
            exported_clips += 1;
            info!(
                animation = format_args!("0x{:08X}", ir.clips[clip_index].animation_uid),
                sample_rate = ir.clips[clip_index].sample_rate,
                frames = ir.clips[clip_index].frame_count,
                output = %fbx_path.display(),
                "exported animation-only FBX"
            );
        }

        if !keep_ir {
            let _ = fs::remove_file(&ir_path);
            let _ = fs::remove_file(&manifest_path);
        }
    }

    if !keep_ir && !ir_only {
        let _ = fs::remove_dir(&ir_folder);
    }
    if ir_only {
        info!(
            models = exported_models,
            animations = exported_clips,
            skipped_animations = skipped_clips,
            output = %ir_folder.display(),
            "FBX character and animation IR validation complete"
        );
    } else {
        info!(
            models = exported_models,
            animations = exported_clips,
            skipped_animations = skipped_clips,
            output = %output_folder.display(),
            "FBX character and animation export complete"
        );
    }
    Ok(())
}

fn invoke_helper(
    exporter: &Path,
    mode_arguments: &[String],
    ir_path: &Path,
    output_path: &Path,
    report_path: &Path,
    resource_uid: u32,
) -> anyhow::Result<()> {
    let mut command = Command::new(exporter);
    for argument in mode_arguments {
        command.arg(argument);
    }
    let status = command
        .arg(ir_path)
        .arg(output_path)
        .arg(report_path)
        .status()
        .with_context(|| format!("failed to launch {}", exporter.display()))?;
    ensure!(
        status.success(),
        "FBX helper failed for resource 0x{resource_uid:08X} with status {status}"
    );
    ensure!(
        output_path.is_file(),
        "FBX helper did not create {}",
        output_path.display()
    );
    ensure!(
        report_path.is_file(),
        "FBX helper did not create {}",
        report_path.display()
    );
    Ok(())
}

fn resolve_exporter(explicit: Option<&str>) -> anyhow::Result<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = explicit.filter(|value| !value.trim().is_empty()) {
        candidates.push(PathBuf::from(path));
    }
    if let Ok(path) = std::env::var("EUROCHEF_FBX_EXPORTER") {
        if !path.trim().is_empty() {
            candidates.push(PathBuf::from(path));
        }
    }

    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .context("could not resolve eurochef-main_legacy root")?;
    candidates.push(project_root.join("target/release/tools/fbx/fbx_export_helper.exe"));
    candidates.push(project_root.join("target/debug/tools/fbx/fbx_export_helper.exe"));

    candidates
        .into_iter()
        .find(|path| path.is_file())
        .ok_or_else(|| anyhow::anyhow!(
            "Autodesk FBX helper not found. Set EUROCHEF_FBX_EXPORTER or run BUILD_FBX_EXPORTER.cmd after setting FBX_SDK_ROOT to an external Autodesk FBX SDK 2020.x installation"
        ))
}

fn read_corpus_manifest(path: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read Script manifest {}", path.display()))?;
    let base = path.parent().unwrap_or_else(|| Path::new("."));
    let mut entries = Vec::new();
    for (line_index, line) in content.lines().enumerate() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let Some((_, source_text)) = line.split_once('\t') else {
            if line_index == 0 {
                continue;
            }
            continue;
        };
        let source_text = source_text.trim();
        if source_text.is_empty()
            || source_text.to_ascii_lowercase().contains("source edb")
            || source_text.eq_ignore_ascii_case("path")
        {
            continue;
        }
        let source_path = PathBuf::from(source_text);
        entries.push(if source_path.is_absolute() {
            source_path
        } else {
            base.join(source_path)
        });
    }
    ensure!(
        !entries.is_empty(),
        "Script manifest {} contains no EDB paths",
        path.display()
    );
    Ok(entries)
}

fn resolve_corpus_resource(
    files: &HashMap<u32, CorpusFileCatalog>,
    current_file: u32,
    serialized_file: u32,
    serialized_hash: u32,
    skin: bool,
) -> Option<(u32, PathBuf, usize, u32)> {
    let source_file = if serialized_hash.is_local() || serialized_file == u32::MAX {
        current_file
    } else {
        serialized_file
    };
    let source = files.get(&source_file)?;
    let list = if skin {
        source.skins.as_slice()
    } else {
        source.animations.as_slice()
    };
    let index = if serialized_hash.is_local() {
        serialized_hash.index() as usize
    } else {
        list.iter()
            .position(|hashcode| *hashcode == serialized_hash)?
    };
    let resolved_uid = *list.get(index)?;
    Some((source.uid, source.source_path.clone(), index, resolved_uid))
}

fn push_usage(
    catalog: &mut AnimationUsageCatalog,
    animation_index: usize,
    timing: AnimationUsageTiming,
) {
    let entries = catalog.valid.entry(animation_index).or_default();
    if !entries.iter().any(|entry| {
        entry.script_edb_uid == timing.script_edb_uid
            && entry.script_uid == timing.script_uid
            && entry.command_index == timing.command_index
            && entry.script_fps.to_bits() == timing.script_fps.to_bits()
            && entry.command_length == timing.command_length
    }) {
        entries.push(timing);
    }
}

fn merge_animation_usages(target: &mut AnimationUsageCatalog, source: &AnimationUsageCatalog) {
    for (animation_index, timings) in &source.valid {
        for timing in timings {
            push_usage(target, *animation_index, *timing);
        }
    }
    for (animation_index, errors) in &source.invalid {
        let target_errors = target.invalid.entry(*animation_index).or_default();
        for error in errors {
            if !target_errors.contains(error) {
                target_errors.push(error.clone());
            }
        }
    }
}

fn collect_corpus_animation_bindings(
    manifest_path: &Path,
    target_header: &eurochef_edb::header::EXGeoHeader,
) -> anyhow::Result<CorpusAnimationCatalog> {
    let paths = read_corpus_manifest(manifest_path)?;
    let mut files = HashMap::<u32, CorpusFileCatalog>::new();
    for source_path in &paths {
        let platform = Platform::from_path(source_path)
            .with_context(|| format!("failed to detect platform for {}", source_path.display()))?;
        if platform != Platform::Pc {
            continue;
        }
        let file = File::open(source_path)
            .with_context(|| format!("failed to open corpus EDB {}", source_path.display()))?;
        let edb = EdbFile::new(Box::new(BufReader::new(file)), platform)
            .with_context(|| format!("failed to parse corpus EDB {}", source_path.display()))?;
        let header = edb.header;
        let entry = CorpusFileCatalog {
            uid: header.hashcode,
            source_path: source_path.clone(),
            animations: header
                .anim_list
                .iter()
                .map(|animation| animation.common.hashcode)
                .collect(),
            skins: header
                .animskin_list
                .iter()
                .map(|skin| skin.common.hashcode)
                .collect(),
        };
        if let Some(existing) = files.get(&entry.uid) {
            ensure!(
                existing.source_path == entry.source_path,
                "duplicate EDB UID 0x{:08X} in {} and {}",
                entry.uid,
                existing.source_path.display(),
                entry.source_path.display()
            );
        } else {
            files.insert(entry.uid, entry);
        }
    }

    let mut result = CorpusAnimationCatalog {
        files_scanned: files.len(),
        ..Default::default()
    };
    for source in files.values() {
        let file = File::open(&source.source_path)
            .with_context(|| format!("failed to reopen {}", source.source_path.display()))?;
        let mut edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)?;
        let scripts = UXGeoScript::read_all(&mut edb).with_context(|| {
            format!(
                "failed to read Scripts from {}",
                source.source_path.display()
            )
        })?;
        for script in &scripts {
            for (command_index, command) in script.commands.iter().enumerate() {
                let UXGeoScriptCommandData::Animation {
                    skin_file,
                    skin_hashcode,
                    anim_file,
                    anim_hashcode,
                } = &command.data
                else {
                    continue;
                };
                result.animation_commands += 1;
                let Some((
                    animation_source_edb_uid,
                    animation_source_path,
                    animation_index,
                    animation_uid,
                )) = resolve_corpus_resource(&files, source.uid, *anim_file, *anim_hashcode, false)
                else {
                    result.unresolved_commands += 1;
                    continue;
                };
                if !script.framerate.is_finite()
                    || script.framerate <= f32::EPSILON
                    || command.length == 0
                {
                    result.unresolved_commands += 1;
                    continue;
                }
                let timing = AnimationUsageTiming {
                    script_edb_uid: source.uid,
                    script_uid: script.hashcode,
                    command_index,
                    script_fps: script.framerate,
                    command_length: command.length,
                };

                let explicit_skin = if matches!(*skin_hashcode, 0 | u32::MAX) {
                    None
                } else {
                    resolve_corpus_resource(&files, source.uid, *skin_file, *skin_hashcode, true)
                };

                if animation_source_edb_uid == target_header.hashcode {
                    let native_skin_uid = target_header
                        .anim_list
                        .data()
                        .get(animation_index)
                        .and_then(|animation| {
                            target_header
                                .animskin_list
                                .iter()
                                .find(|skin| skin.base_skin_num == animation.skin_num)
                        })
                        .map(|skin| skin.common.hashcode);
                    let usage_matches_native_skin = explicit_skin
                        .as_ref()
                        .map(|(skin_source_uid, _, _, skin_uid)| {
                            *skin_source_uid == target_header.hashcode
                                && native_skin_uid == Some(*skin_uid)
                        })
                        .unwrap_or(true);
                    if usage_matches_native_skin {
                        push_usage(&mut result.target_animation_usages, animation_index, timing);
                    }
                }

                let Some((skin_source_edb_uid, _, _, skin_uid)) = explicit_skin else {
                    continue;
                };
                if skin_source_edb_uid != target_header.hashcode {
                    continue;
                }
                result.explicit_skin_bindings.push(ScriptAnimationBinding {
                    script_edb_uid: source.uid,
                    script_uid: script.hashcode,
                    command_index,
                    script_fps: script.framerate,
                    command_length: command.length,
                    animation_source_edb_uid,
                    animation_source_path,
                    animation_index,
                    animation_uid,
                    skin_source_edb_uid,
                    skin_uid,
                });
            }
        }
    }
    Ok(result)
}

fn collect_animation_usages(
    header: &eurochef_edb::header::EXGeoHeader,
    scripts: &[UXGeoScript],
) -> AnimationUsageCatalog {
    let mut catalog = AnimationUsageCatalog::default();
    for script in scripts {
        for (command_index, command) in script.commands.iter().enumerate() {
            let UXGeoScriptCommandData::Animation {
                skin_file,
                skin_hashcode,
                anim_file,
                anim_hashcode,
            } = &command.data
            else {
                continue;
            };
            let Some(animation_index) = resolve_animation_index(header, *anim_file, *anim_hashcode)
            else {
                continue;
            };
            if !matches!(*skin_hashcode, 0 | u32::MAX) {
                let skin_source_file = if skin_hashcode.is_local() || *skin_file == u32::MAX {
                    header.hashcode
                } else {
                    *skin_file
                };
                if skin_source_file != header.hashcode {
                    continue;
                }
                let resolved_skin_uid = if skin_hashcode.is_local() {
                    header
                        .animskin_list
                        .data()
                        .get(skin_hashcode.index() as usize)
                        .map(|skin| skin.common.hashcode)
                } else {
                    header
                        .animskin_list
                        .iter()
                        .find(|skin| skin.common.hashcode == *skin_hashcode)
                        .map(|skin| skin.common.hashcode)
                };
                let native_skin_uid = header
                    .anim_list
                    .data()
                    .get(animation_index)
                    .and_then(|animation| {
                        header
                            .animskin_list
                            .iter()
                            .find(|skin| skin.base_skin_num == animation.skin_num)
                    })
                    .map(|skin| skin.common.hashcode);
                if resolved_skin_uid.is_none() || resolved_skin_uid != native_skin_uid {
                    continue;
                }
            }
            if !script.framerate.is_finite() || script.framerate <= f32::EPSILON {
                catalog
                    .invalid
                    .entry(animation_index)
                    .or_default()
                    .push(format!(
                        "Script 0x{:08X} command {command_index} has invalid serialized FPS {}",
                        script.hashcode, script.framerate
                    ));
                continue;
            }
            if command.length == 0 {
                catalog
                    .invalid
                    .entry(animation_index)
                    .or_default()
                    .push(format!(
                        "Script 0x{:08X} command {command_index} has zero animation length",
                        script.hashcode
                    ));
                continue;
            }
            catalog
                .valid
                .entry(animation_index)
                .or_default()
                .push(AnimationUsageTiming {
                    script_edb_uid: header.hashcode,
                    script_uid: script.hashcode,
                    command_index,
                    script_fps: script.framerate,
                    command_length: command.length,
                });
        }
    }
    catalog
}

fn resolve_animation_index(
    header: &eurochef_edb::header::EXGeoHeader,
    anim_file: u32,
    anim_hashcode: u32,
) -> Option<usize> {
    if anim_hashcode.is_local() {
        let index = anim_hashcode.index() as usize;
        return header.anim_list.data().get(index).map(|_| index);
    }
    if anim_file != u32::MAX && anim_file != header.hashcode {
        return None;
    }
    header
        .anim_list
        .iter()
        .position(|animation| animation.common.hashcode == anim_hashcode)
}

fn build_character_ir(
    edb: &mut EdbFile,
    skin: &EXGeoBaseAnimSkin,
    animskin_uid: u32,
    base_skin_num: u32,
    source_path: &Path,
    usage_catalog: &AnimationUsageCatalog,
    corpus_bindings: &[ScriptAnimationBinding],
    unreferenced_animation_fps: Option<f32>,
) -> anyhow::Result<CharacterIr> {
    let header = edb.header.clone();
    let bone_count = skin.bone_count as usize;
    ensure!(bone_count > 0, "AnimSkin has no bones");
    ensure!(
        skin.absolute_bind_positions.len() == bone_count
            && skin.relative_bind_positions.len() == bone_count
            && skin.hier_data.len() == bone_count,
        "AnimSkin bone arrays do not match bone_count={bone_count}"
    );

    let source_root_count = skin
        .hier_data
        .iter()
        .filter(|hierarchy| hierarchy.link_index == u16::MAX)
        .count();
    ensure!(
        source_root_count != 0,
        "AnimSkin hierarchy has no root bone"
    );
    let synthetic_root = source_root_count > 1;
    let source_bone_offset = usize::from(synthetic_root);
    let mut bones = Vec::with_capacity(bone_count + source_bone_offset);
    if synthetic_root {
        bones.push(BoneIr {
            name: "EuroChefRoot".to_string(),
            parent: -1,
            local_position: [0.0; 3],
            global_position: [0.0; 3],
        });
    }
    for index in 0..bone_count {
        let hierarchy = &skin.hier_data[index];
        let parent = if hierarchy.link_index == u16::MAX {
            if synthetic_root {
                0
            } else {
                -1
            }
        } else {
            i32::from(hierarchy.link_index) + source_bone_offset as i32
        };
        bones.push(BoneIr {
            name: format!("bone_{index:03}"),
            parent,
            local_position: vector3(&skin.relative_bind_positions[index]),
            global_position: vector3(&skin.absolute_bind_positions[index]),
        });
    }

    let name = eurochef_edb::robots_hashdb::format_or_invalid(animskin_uid);
    let mut meshes = Vec::new();
    for (group_name, components) in [
        ("primary", skin.entities.data().as_slice()),
        ("secondary", skin.more_entities.data().as_slice()),
    ] {
        for (component_index, component) in components.iter().enumerate() {
            meshes.push(build_component_mesh(
                edb,
                skin,
                component,
                group_name,
                component_index,
                source_bone_offset,
            )?);
        }
    }
    ensure!(!meshes.is_empty(), "AnimSkin contains no component meshes");

    let (clips, skipped_animations) = build_animation_clips(
        edb,
        base_skin_num,
        animskin_uid,
        source_path,
        bone_count,
        source_bone_offset,
        usage_catalog,
        corpus_bindings,
        unreferenced_animation_fps,
    )?;

    Ok(CharacterIr {
        name,
        source_edb_uid: header.hashcode,
        animskin_uid,
        bones,
        meshes,
        clips,
        skipped_animations,
    })
}

fn build_animation_clips(
    edb: &mut EdbFile,
    base_skin_num: u32,
    animskin_uid: u32,
    source_path: &Path,
    source_bone_count: usize,
    source_bone_offset: usize,
    usage_catalog: &AnimationUsageCatalog,
    corpus_bindings: &[ScriptAnimationBinding],
    unreferenced_animation_fps: Option<f32>,
) -> anyhow::Result<(Vec<AnimationClipIr>, Vec<SkippedAnimationIr>)> {
    let header = edb.header.clone();
    let mut clips = Vec::new();
    let mut skipped = Vec::new();

    for (animation_index, animation) in header.anim_list.iter().enumerate() {
        if animation.skin_num != base_skin_num {
            continue;
        }
        let animation_uid = animation.common.hashcode;
        let motion_checksum =
            match read_motion_checksum(edb, animation.motiondata_info_addr, animation.datasize) {
                Ok(checksum) => checksum,
                Err(error) => {
                    skipped.push(SkippedAnimationIr {
                        animation_uid,
                        animation_source_edb_uid: header.hashcode,
                        animation_source_path: source_path.to_path_buf(),
                        source_animation_index: animation_index,
                        reason: format!("motion payload could not be checksummed: {error}"),
                    });
                    continue;
                }
            };
        let cache = match load_pose_cache(
            header.hashcode,
            animation_index,
            animation_uid,
            animskin_uid,
            motion_checksum,
        ) {
            Ok(cache) => cache,
            Err(error) => {
                skipped.push(SkippedAnimationIr {
                    animation_uid,
                    animation_source_edb_uid: header.hashcode,
                    animation_source_path: source_path.to_path_buf(),
                    source_animation_index: animation_index,
                    reason: error,
                });
                continue;
            }
        };
        if cache.bone_count != source_bone_count {
            skipped.push(SkippedAnimationIr {
                animation_uid,
                animation_source_edb_uid: header.hashcode,
                animation_source_path: source_path.to_path_buf(),
                source_animation_index: animation_index,
                reason: format!(
                    "pose cache bone count {} does not match AnimSkin {}",
                    cache.bone_count, source_bone_count
                ),
            });
            continue;
        }

        let valid_usages = usage_catalog
            .valid
            .get(&animation_index)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let mut variants = timing_variants(cache.frame_count, valid_usages);
        if variants.is_empty() {
            if let Some(fps) = unreferenced_animation_fps {
                variants.push(TimingVariant {
                    source_script_edb_uid: u32::MAX,
                    source_script_uid: u32::MAX,
                    source_script_command: u32::MAX,
                    usage_count: 0,
                    source_script_fps: fps,
                    source_command_length: 0,
                    sample_rate: fps,
                    duration_seconds: if cache.frame_count > 1 {
                        (cache.frame_count - 1) as f32 / fps
                    } else {
                        0.0
                    },
                });
            } else {
                let mut reason = "no valid in-EDB AnimScript timing reference; exact FPS is not serialized in Animation".to_string();
                if let Some(errors) = usage_catalog.invalid.get(&animation_index) {
                    reason.push_str(&format!("; invalid usages: {}", errors.join(" | ")));
                }
                skipped.push(SkippedAnimationIr {
                    animation_uid,
                    animation_source_edb_uid: header.hashcode,
                    animation_source_path: source_path.to_path_buf(),
                    source_animation_index: animation_index,
                    reason,
                });
                continue;
            }
        }

        let decoded_name = eurochef_edb::robots_hashdb::format_or_invalid(animation_uid);
        for (variant_index, timing) in variants.iter().enumerate() {
            let mut poses = add_synthetic_root_poses(
                &cache.poses,
                cache.frame_count,
                source_bone_count,
                source_bone_offset,
            )?;
            enforce_quaternion_sign_continuity(
                &mut poses,
                cache.frame_count,
                source_bone_count + source_bone_offset,
            );
            let name = if variants.len() == 1 {
                decoded_name.clone()
            } else {
                format!("{decoded_name}_Timing{variant_index:02}")
            };
            clips.push(AnimationClipIr {
                name,
                animation_uid,
                animation_source_edb_uid: header.hashcode,
                animation_source_path: source_path.to_path_buf(),
                source_animation_index: checked_u32(animation_index, "animation index")?,
                source_script_edb_uid: timing.source_script_edb_uid,
                source_script_uid: timing.source_script_uid,
                source_script_command: timing.source_script_command,
                usage_count: timing.usage_count,
                source_script_fps: timing.source_script_fps,
                source_command_length: timing.source_command_length,
                sample_rate: timing.sample_rate,
                frame_count: checked_u32(cache.frame_count, "animation frame count")?,
                duration_seconds: timing.duration_seconds,
                root_motion_mode: "preserve_local_pose_cache_tracks".to_string(),
                pose_cache_path: cache.source_path.clone(),
                poses,
            });
        }
    }

    let mut grouped_bindings = HashMap::<(u32, usize, u32), Vec<&ScriptAnimationBinding>>::new();
    for binding in corpus_bindings {
        if binding.skin_source_edb_uid != header.hashcode || binding.skin_uid != animskin_uid {
            continue;
        }
        grouped_bindings
            .entry((
                binding.animation_source_edb_uid,
                binding.animation_index,
                binding.animation_uid,
            ))
            .or_default()
            .push(binding);
    }

    for ((animation_source_edb_uid, animation_index, animation_uid), bindings) in grouped_bindings {
        let source_path = &bindings[0].animation_source_path;
        let file = File::open(source_path).with_context(|| {
            format!("failed to open Animation source {}", source_path.display())
        })?;
        let mut animation_edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
            .with_context(|| {
                format!("failed to parse Animation source {}", source_path.display())
            })?;
        let animation_header = animation_edb.header.clone();
        ensure!(
            animation_header.hashcode == animation_source_edb_uid,
            "Animation source UID mismatch 0x{:08X} != 0x{:08X}",
            animation_header.hashcode,
            animation_source_edb_uid
        );
        let Some(animation) = animation_header.anim_list.data().get(animation_index) else {
            skipped.push(SkippedAnimationIr {
                animation_uid,
                animation_source_edb_uid,
                animation_source_path: source_path.clone(),
                source_animation_index: animation_index,
                reason: "Script-resolved Animation index is outside source EDB".to_string(),
            });
            continue;
        };
        ensure!(
            animation.common.hashcode == animation_uid,
            "Script-resolved Animation UID mismatch at 0x{animation_source_edb_uid:08X}:{animation_index}"
        );
        if animation_source_edb_uid == header.hashcode && animation.skin_num == base_skin_num {
            continue;
        }

        let motion_checksum = match read_motion_checksum(
            &mut animation_edb,
            animation.motiondata_info_addr,
            animation.datasize,
        ) {
            Ok(checksum) => checksum,
            Err(error) => {
                skipped.push(SkippedAnimationIr {
                    animation_uid,
                    animation_source_edb_uid,
                    animation_source_path: source_path.clone(),
                    source_animation_index: animation_index,
                    reason: format!(
                        "Script-bound motion payload could not be checksummed: {error}"
                    ),
                });
                continue;
            }
        };
        let cache = match load_pose_cache(
            animation_source_edb_uid,
            animation_index,
            animation_uid,
            animskin_uid,
            motion_checksum,
        ) {
            Ok(cache) => cache,
            Err(error) => {
                skipped.push(SkippedAnimationIr {
                    animation_uid,
                    animation_source_edb_uid,
                    animation_source_path: source_path.clone(),
                    source_animation_index: animation_index,
                    reason: format!(
                        "cache_missing_for_script_bound_skin: Animation 0x{animation_uid:08X} from EDB 0x{animation_source_edb_uid:08X} is bound by Script to AnimSkin 0x{animskin_uid:08X}; regenerate RAPCV002 for this exact pair: {error}"
                    ),
                });
                continue;
            }
        };
        if cache.bone_count != source_bone_count {
            skipped.push(SkippedAnimationIr {
                animation_uid,
                animation_source_edb_uid,
                animation_source_path: source_path.clone(),
                source_animation_index: animation_index,
                reason: format!(
                    "Script-bound pose cache bone count {} does not match AnimSkin {}",
                    cache.bone_count, source_bone_count
                ),
            });
            continue;
        }

        let usages = bindings
            .iter()
            .map(|binding| AnimationUsageTiming {
                script_edb_uid: binding.script_edb_uid,
                script_uid: binding.script_uid,
                command_index: binding.command_index,
                script_fps: binding.script_fps,
                command_length: binding.command_length,
            })
            .collect::<Vec<_>>();
        let variants = timing_variants(cache.frame_count, &usages);
        let decoded_name = eurochef_edb::robots_hashdb::format_or_invalid(animation_uid);
        for (variant_index, timing) in variants.iter().enumerate() {
            let mut poses = add_synthetic_root_poses(
                &cache.poses,
                cache.frame_count,
                source_bone_count,
                source_bone_offset,
            )?;
            enforce_quaternion_sign_continuity(
                &mut poses,
                cache.frame_count,
                source_bone_count + source_bone_offset,
            );
            let source_suffix = if animation_source_edb_uid == header.hashcode {
                String::new()
            } else {
                format!("_SourceEDB_{animation_source_edb_uid:08X}")
            };
            let timing_suffix = if variants.len() == 1 {
                String::new()
            } else {
                format!("_Timing{variant_index:02}")
            };
            clips.push(AnimationClipIr {
                name: format!("{decoded_name}{source_suffix}{timing_suffix}"),
                animation_uid,
                animation_source_edb_uid,
                animation_source_path: source_path.clone(),
                source_animation_index: checked_u32(animation_index, "animation index")?,
                source_script_edb_uid: timing.source_script_edb_uid,
                source_script_uid: timing.source_script_uid,
                source_script_command: timing.source_script_command,
                usage_count: timing.usage_count,
                source_script_fps: timing.source_script_fps,
                source_command_length: timing.source_command_length,
                sample_rate: timing.sample_rate,
                frame_count: checked_u32(cache.frame_count, "animation frame count")?,
                duration_seconds: timing.duration_seconds,
                root_motion_mode: "preserve_local_pose_cache_tracks".to_string(),
                pose_cache_path: cache.source_path.clone(),
                poses,
            });
        }
    }

    Ok((clips, skipped))
}

fn timing_variants(_frame_count: usize, usages: &[AnimationUsageTiming]) -> Vec<TimingVariant> {
    let mut variants = Vec::<TimingVariant>::new();
    for usage in usages {
        let duration_seconds = f32::from(usage.command_length) / usage.script_fps;
        if !duration_seconds.is_finite() || duration_seconds <= f32::EPSILON {
            continue;
        }
        let sample_rate = usage.script_fps;
        if let Some(existing) = variants.iter_mut().find(|variant| {
            (variant.sample_rate - sample_rate).abs() <= 1.0e-6
                && (variant.duration_seconds - duration_seconds).abs() <= 1.0e-6
        }) {
            existing.usage_count = existing.usage_count.saturating_add(1);
            continue;
        }
        variants.push(TimingVariant {
            source_script_edb_uid: usage.script_edb_uid,
            source_script_uid: usage.script_uid,
            source_script_command: usage.command_index as u32,
            usage_count: 1,
            source_script_fps: usage.script_fps,
            source_command_length: u32::from(usage.command_length),
            sample_rate,
            duration_seconds,
        });
    }
    variants
}

fn add_synthetic_root_poses(
    source: &[PoseIr],
    frame_count: usize,
    source_bone_count: usize,
    source_bone_offset: usize,
) -> anyhow::Result<Vec<PoseIr>> {
    ensure!(
        source.len() == frame_count.saturating_mul(source_bone_count),
        "pose cache dimensions do not match pose payload"
    );
    if source_bone_offset == 0 {
        return Ok(source.to_vec());
    }
    ensure!(source_bone_offset == 1, "unsupported synthetic bone offset");
    let mut output = Vec::with_capacity(frame_count * (source_bone_count + 1));
    for frame in 0..frame_count {
        output.push(PoseIr::IDENTITY);
        let start = frame * source_bone_count;
        output.extend_from_slice(&source[start..start + source_bone_count]);
    }
    Ok(output)
}

fn enforce_quaternion_sign_continuity(poses: &mut [PoseIr], frame_count: usize, bone_count: usize) {
    if frame_count <= 1 || bone_count == 0 {
        return;
    }
    for bone_index in 0..bone_count {
        for frame in 1..frame_count {
            let previous_index = (frame - 1) * bone_count + bone_index;
            let current_index = frame * bone_count + bone_index;
            let previous = poses[previous_index].rotation;
            let current = poses[current_index].rotation;
            let dot = previous
                .iter()
                .zip(current)
                .map(|(left, right)| left * right)
                .sum::<f32>();
            if dot < 0.0 {
                poses[current_index].rotation = current.map(|value| -value);
            }
        }
    }
}

fn pose_cache_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf);
    if let Ok(path) = std::env::var("ROBOTS_ANIMATION_POSE_CACHE") {
        if !path.trim().is_empty() {
            let configured = PathBuf::from(path);
            roots.push(configured.clone());
            if configured.is_relative() {
                if let Some(project_root) = project_root.as_ref() {
                    roots.push(project_root.join(configured));
                }
            }
        }
    }
    if let Some(project_root) = project_root.as_ref() {
        roots.push(project_root.join("target/robots_animation_pose_cache"));
    }
    if let Ok(current_dir) = std::env::current_dir() {
        roots.push(current_dir.join("target/robots_animation_pose_cache"));
        roots.push(
            current_dir.join("_tools/eurochef-main_legacy/target/robots_animation_pose_cache"),
        );
    }
    roots.sort();
    roots.dedup();
    roots
}

fn load_pose_cache(
    edb_uid: u32,
    animation_index: usize,
    animation_uid: u32,
    animskin_uid: u32,
    motion_checksum: u64,
) -> Result<PoseCacheData, String> {
    let folder = PathBuf::from(format!("{edb_uid:08X}"));
    let relative_candidates = [
        folder.join(format!("{animation_index:04}_[0x{animskin_uid:08X}].rapc")),
        folder.join(format!("{animation_index:04}.rapc")),
    ];
    let mut searched = Vec::new();
    for root in pose_cache_roots() {
        for relative in &relative_candidates {
            let path = root.join(relative);
            searched.push(path.clone());
            if !path.is_file() {
                continue;
            }
            let bytes = fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
            match parse_pose_cache(
                &path,
                &bytes,
                edb_uid,
                animation_index,
                animation_uid,
                animskin_uid,
                motion_checksum,
            ) {
                Ok(cache) => return Ok(cache),
                Err(error) if relative == &relative_candidates[0] => return Err(error),
                Err(_) => continue,
            }
        }
    }
    Err(format!(
        "RAPCV002 pose cache is missing; searched {}",
        searched
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

fn parse_pose_cache(
    source_path: &Path,
    bytes: &[u8],
    expected_edb_uid: u32,
    expected_animation_index: usize,
    expected_animation_uid: u32,
    expected_animskin_uid: u32,
    expected_motion_checksum: u64,
) -> Result<PoseCacheData, String> {
    if bytes.len() < POSE_CACHE_HEADER_SIZE {
        return Err(format!(
            "pose cache is {} bytes; header requires {POSE_CACHE_HEADER_SIZE}",
            bytes.len()
        ));
    }
    if bytes.len() > MAX_POSE_CACHE_BYTES {
        return Err(format!(
            "pose cache is {} bytes; safety limit is {MAX_POSE_CACHE_BYTES}",
            bytes.len()
        ));
    }
    if bytes.get(..POSE_CACHE_MAGIC.len()) != Some(POSE_CACHE_MAGIC) {
        return Err("pose cache magic is not RAPCV002".to_string());
    }
    let edb_uid = read_u32_le(bytes, 0x08)?;
    let animation_index = read_u32_le(bytes, 0x0C)? as usize;
    let animation_uid = read_u32_le(bytes, 0x10)?;
    let animskin_uid = read_u32_le(bytes, 0x14)?;
    let frame_count = read_u32_le(bytes, 0x18)? as usize;
    let bone_count = read_u32_le(bytes, 0x1C)? as usize;
    let motion_checksum = read_u64_le(bytes, 0x20)?;
    if edb_uid != expected_edb_uid
        || animation_index != expected_animation_index
        || animation_uid != expected_animation_uid
        || animskin_uid != expected_animskin_uid
        || motion_checksum != expected_motion_checksum
    {
        return Err(format!(
            "pose cache identity mismatch: edb=0x{edb_uid:08X}/0x{expected_edb_uid:08X}, animation={animation_index}/{expected_animation_index} uid=0x{animation_uid:08X}/0x{expected_animation_uid:08X}, skin=0x{animskin_uid:08X}/0x{expected_animskin_uid:08X}, motion=0x{motion_checksum:016X}/0x{expected_motion_checksum:016X}"
        ));
    }
    if frame_count == 0 || bone_count == 0 || bone_count > u16::MAX as usize {
        return Err(format!(
            "invalid pose dimensions frames={frame_count} bones={bone_count}"
        ));
    }
    let pose_count = frame_count
        .checked_mul(bone_count)
        .ok_or_else(|| "pose cache dimensions overflow".to_string())?;
    let payload_size = pose_count
        .checked_mul(POSE_CACHE_BYTES_PER_BONE)
        .ok_or_else(|| "pose cache payload size overflows".to_string())?;
    let expected_size = POSE_CACHE_HEADER_SIZE
        .checked_add(payload_size)
        .ok_or_else(|| "pose cache file size overflows".to_string())?;
    if bytes.len() != expected_size {
        return Err(format!(
            "pose cache size mismatch {} != {expected_size}",
            bytes.len()
        ));
    }

    let mut poses = Vec::with_capacity(pose_count);
    for pose_index in 0..pose_count {
        let offset = POSE_CACHE_HEADER_SIZE + pose_index * POSE_CACHE_BYTES_PER_BONE;
        let position = [
            read_f32_le(bytes, offset)?,
            read_f32_le(bytes, offset + 4)?,
            read_f32_le(bytes, offset + 8)?,
        ];
        let rotation = [
            read_f32_le(bytes, offset + 12)?,
            read_f32_le(bytes, offset + 16)?,
            read_f32_le(bytes, offset + 20)?,
            read_f32_le(bytes, offset + 24)?,
        ];
        if !position
            .iter()
            .chain(rotation.iter())
            .all(|value| value.is_finite())
        {
            return Err(format!("non-finite pose at flattened index {pose_index}"));
        }
        let rotation_length = rotation
            .iter()
            .map(|value| value * value)
            .sum::<f32>()
            .sqrt();
        if (rotation_length - 1.0).abs() > 2.0e-3 {
            return Err(format!(
                "non-unit quaternion at flattened index {pose_index}: {rotation_length}"
            ));
        }
        poses.push(PoseIr {
            position,
            rotation,
            scale: [1.0; 3],
        });
    }
    Ok(PoseCacheData {
        source_path: source_path.to_path_buf(),
        frame_count,
        bone_count,
        poses,
    })
}

fn read_motion_checksum(edb: &mut EdbFile, address: u32, size: u32) -> anyhow::Result<u64> {
    let size = size as usize;
    ensure!(
        size <= MAX_MOTION_BYTES,
        "motion payload exceeds safety limit"
    );
    let header = edb.header.clone();
    ensure!(
        (address as usize).saturating_add(size) <= header.file_size as usize,
        "motion payload is outside EDB file bounds"
    );
    let saved_position = edb.stream_position()?;
    edb.seek(SeekFrom::Start(address as u64))?;
    let mut bytes = vec![0u8; size];
    let result = edb.read_exact(&mut bytes);
    edb.seek(SeekFrom::Start(saved_position))?;
    result?;
    Ok(fnv1a64(&bytes))
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let value = bytes
        .get(offset..offset + size_of::<u32>())
        .ok_or_else(|| format!("pose cache u32 at 0x{offset:X} is truncated"))?;
    Ok(u32::from_le_bytes(value.try_into().unwrap()))
}

fn read_u64_le(bytes: &[u8], offset: usize) -> Result<u64, String> {
    let value = bytes
        .get(offset..offset + size_of::<u64>())
        .ok_or_else(|| format!("pose cache u64 at 0x{offset:X} is truncated"))?;
    Ok(u64::from_le_bytes(value.try_into().unwrap()))
}

fn read_f32_le(bytes: &[u8], offset: usize) -> Result<f32, String> {
    let value = bytes
        .get(offset..offset + size_of::<f32>())
        .ok_or_else(|| format!("pose cache f32 at 0x{offset:X} is truncated"))?;
    Ok(f32::from_le_bytes(value.try_into().unwrap()))
}

fn build_component_mesh(
    edb: &mut EdbFile,
    skin: &EXGeoBaseAnimSkin,
    component: &EXGeoAnimSkinEntity,
    group_name: &str,
    component_index: usize,
    source_bone_offset: usize,
) -> anyhow::Result<MeshIr> {
    let header = edb.header.clone();
    let entity_index = (component.entity_index & 0x00ff_ffff) as usize;
    let entity_header = header
        .entity_list
        .data()
        .get(entity_index)
        .with_context(|| format!("component Entity index {entity_index} is outside Entity list"))?;
    edb.seek(SeekFrom::Start(entity_header.common.address as u64))?;
    let entity: EXGeoEntity = edb.read_type_args(edb.endian, (header.version, edb.platform))?;

    let mut part_vertex_counts = Vec::new();
    collect_mesh_vertex_counts(&entity, &mut part_vertex_counts);
    ensure!(
        component.skin_data.len() == part_vertex_counts.len(),
        "component {group_name}:{component_index} has {} weight parts but Entity has {} mesh parts",
        component.skin_data.len(),
        part_vertex_counts.len()
    );

    let mut vertices = Vec::new();
    let mut source_indices = Vec::new();
    let mut strips = Vec::new();
    read_entity(
        &entity,
        &mut vertices,
        &mut source_indices,
        &mut strips,
        edb,
        MAX_ENTITY_DEPTH,
        false,
        true,
    )?;
    ensure!(
        !vertices.is_empty(),
        "component Entity has no render vertices"
    );

    let mut influences = Vec::with_capacity(vertices.len());
    for (part_index, (weights, vertex_count)) in component
        .skin_data
        .iter()
        .zip(part_vertex_counts.iter().copied())
        .enumerate()
    {
        let palette = weights.bone_palette.as_slice();
        let part_influences = weights
            .read_vertex_influences(edb, edb.endian, vertex_count)
            .with_context(|| format!("failed to read weights for mesh part {part_index}"))?;
        for (vertex_index, influence) in part_influences.into_iter().enumerate() {
            let bone_indices = influence.bone_indices(palette).with_context(|| {
                format!("invalid palette selector in part {part_index}, vertex {vertex_index}")
            })?;
            let weight_sum = influence.weights.iter().sum::<f32>();
            ensure!(
                influence
                    .weights
                    .iter()
                    .all(|weight| weight.is_finite() && *weight >= 0.0)
                    && (weight_sum - 1.0).abs() <= WEIGHT_EPSILON,
                "invalid weights in part {part_index}, vertex {vertex_index}: sum={weight_sum}"
            );
            ensure!(
                bone_indices
                    .iter()
                    .all(|bone| usize::from(*bone) < skin.bone_count as usize),
                "bone index outside AnimSkin in part {part_index}, vertex {vertex_index}"
            );
            let mut exported_bones = [0u16; 4];
            for (lane, source_bone) in bone_indices.into_iter().enumerate() {
                exported_bones[lane] = u16::try_from(usize::from(source_bone) + source_bone_offset)
                    .context("exported bone index exceeds u16")?;
            }
            influences.push(InfluenceIr {
                bone_indices: exported_bones,
                weights: influence.weights,
            });
        }
    }
    ensure!(
        influences.len() == vertices.len(),
        "component has {} vertices but {} skin influence records",
        vertices.len(),
        influences.len()
    );

    let mut materials = Vec::<MaterialIr>::new();
    let mut material_slots = HashMap::<u32, u32>::new();
    let mut indices = Vec::new();
    let mut triangle_materials = Vec::new();
    for strip in &strips {
        let material_hash = texture_hash(&header, strip);
        let material_slot = *material_slots.entry(material_hash).or_insert_with(|| {
            let slot = materials.len() as u32;
            materials.push(MaterialIr {
                hashcode: material_hash,
                name: if material_hash == u32::MAX {
                    "HT_Texture_None_[0xFFFFFFFF]".to_string()
                } else {
                    format!(
                        "{}_[0x{material_hash:08X}]",
                        eurochef_edb::robots_hashdb::format_or_invalid(material_hash)
                    )
                },
            });
            slot
        });
        let start = strip.start_index as usize;
        let end = start
            .checked_add(strip.index_count as usize)
            .context("strip index range overflow")?;
        let strip_indices = source_indices
            .get(start..end)
            .context("strip index range is outside Entity index buffer")?;
        ensure!(
            strip_indices.len() % 3 == 0,
            "triangulated strip index count is not divisible by 3"
        );
        for triangle in strip_indices.chunks_exact(3) {
            ensure!(
                triangle
                    .iter()
                    .all(|index| (*index as usize) < vertices.len()),
                "triangle references vertex outside component mesh"
            );
            indices.extend_from_slice(triangle);
            triangle_materials.push(material_slot);
        }
    }
    ensure!(!indices.is_empty(), "component Entity has no triangles");

    Ok(MeshIr {
        name: format!(
            "{}_[0x{:08X}]_{group_name}_{component_index:03}",
            sanitize_name(&eurochef_edb::robots_hashdb::format_or_invalid(
                entity_header.common.hashcode
            )),
            entity_header.common.hashcode
        ),
        vertices,
        influences,
        indices,
        triangle_materials,
        materials,
    })
}

fn collect_mesh_vertex_counts(entity: &EXGeoEntity, counts: &mut Vec<usize>) {
    match entity {
        EXGeoEntity::Mesh(mesh) => counts.push(mesh.vertices.len()),
        EXGeoEntity::Split(split) => {
            for child in &split.entities {
                collect_mesh_vertex_counts(child, counts);
            }
        }
        _ => {}
    }
}

fn texture_hash(header: &eurochef_edb::header::EXGeoHeader, strip: &TriStrip) -> u32 {
    if strip.texture_index == u32::MAX {
        return u32::MAX;
    }
    header
        .texture_list
        .data()
        .get(strip.texture_index as usize)
        .map(|texture| texture.common.hashcode)
        .unwrap_or(u32::MAX)
}

fn validate_character_ir(ir: &CharacterIr) -> anyhow::Result<()> {
    ensure!(!ir.bones.is_empty(), "character has no bones");
    let roots = ir.bones.iter().filter(|bone| bone.parent < 0).count();
    ensure!(
        roots == 1,
        "character must have exactly one root bone, found {roots}"
    );
    let mut bone_names = std::collections::HashSet::new();
    for (index, bone) in ir.bones.iter().enumerate() {
        ensure!(!bone.name.is_empty(), "bone {index} has an empty name");
        ensure!(
            bone_names.insert(&bone.name),
            "duplicate bone name {}",
            bone.name
        );
        ensure!(
            bone.local_position.iter().all(|value| value.is_finite())
                && bone.global_position.iter().all(|value| value.is_finite()),
            "bone {index} contains non-finite bind coordinates"
        );
        if bone.parent >= 0 {
            ensure!(
                (bone.parent as usize) < index,
                "bone {index} parent {} must precede the child",
                bone.parent
            );
        }
    }
    for mesh in &ir.meshes {
        ensure!(
            mesh.vertices.len() == mesh.influences.len(),
            "mesh vertex/weight mismatch"
        );
        ensure!(
            mesh.indices.len() % 3 == 0,
            "mesh index count is not triangular"
        );
        ensure!(
            mesh.triangle_materials.len() == mesh.indices.len() / 3,
            "mesh triangle/material mismatch"
        );
        for (index, vertex) in mesh.vertices.iter().enumerate() {
            ensure!(
                vertex
                    .pos
                    .iter()
                    .chain(vertex.norm.iter())
                    .chain(vertex.uv.iter())
                    .chain(vertex.color.iter())
                    .all(|value| value.is_finite()),
                "mesh {} vertex {index} contains non-finite data",
                mesh.name
            );
            let influence = mesh.influences[index];
            ensure!(
                influence
                    .bone_indices
                    .iter()
                    .all(|bone| usize::from(*bone) < ir.bones.len()),
                "mesh {} vertex {index} references an invalid bone",
                mesh.name
            );
            ensure!(
                influence
                    .weights
                    .iter()
                    .all(|weight| *weight >= 0.0 && weight.is_finite())
                    && (influence.weights.iter().sum::<f32>() - 1.0).abs() <= WEIGHT_EPSILON,
                "mesh {} vertex {index} has invalid weights",
                mesh.name
            );
        }
    }
    let mut clip_names = std::collections::HashSet::new();
    for clip in &ir.clips {
        ensure!(!clip.name.is_empty(), "animation clip has an empty name");
        ensure!(
            clip_names.insert(&clip.name),
            "duplicate animation clip name {}",
            clip.name
        );
        ensure!(
            clip.sample_rate.is_finite() && clip.sample_rate > f32::EPSILON,
            "animation {} has invalid sample rate {}",
            clip.name,
            clip.sample_rate
        );
        ensure!(
            clip.frame_count > 0,
            "animation {} has zero frames",
            clip.name
        );
        let expected_duration = if clip.source_command_length != 0 {
            clip.source_command_length as f32 / clip.source_script_fps
        } else if clip.frame_count > 1 {
            (clip.frame_count - 1) as f32 / clip.sample_rate
        } else {
            0.0
        };
        ensure!(
            clip.duration_seconds.is_finite()
                && clip.duration_seconds >= 0.0
                && (clip.duration_seconds - expected_duration).abs() <= 1.0e-5,
            "animation {} duration {} does not match serialized timing {}",
            clip.name,
            clip.duration_seconds,
            expected_duration
        );
        ensure!(
            clip.poses.len() == clip.frame_count as usize * ir.bones.len(),
            "animation {} pose dimensions do not match frames and bones",
            clip.name
        );
        for (pose_index, pose) in clip.poses.iter().enumerate() {
            ensure!(
                pose.position
                    .iter()
                    .chain(pose.rotation.iter())
                    .chain(pose.scale.iter())
                    .all(|value| value.is_finite()),
                "animation {} pose {pose_index} contains non-finite data",
                clip.name
            );
            let quaternion_length = pose
                .rotation
                .iter()
                .map(|value| value * value)
                .sum::<f32>()
                .sqrt();
            ensure!(
                (quaternion_length - 1.0).abs() <= 2.0e-3,
                "animation {} pose {pose_index} has non-unit quaternion {}",
                clip.name,
                quaternion_length
            );
            ensure!(
                pose.scale.iter().all(|value| *value > 0.0),
                "animation {} pose {pose_index} has non-positive scale",
                clip.name
            );
        }
    }
    Ok(())
}

fn write_ir(path: &Path, ir: &CharacterIr) -> anyhow::Result<()> {
    let mut out =
        File::create(path).with_context(|| format!("failed to create {}", path.display()))?;
    out.write_all(IR_MAGIC)?;
    write_u32(&mut out, IR_VERSION)?;
    write_string(&mut out, &ir.name)?;
    write_u32(&mut out, ir.source_edb_uid)?;
    write_u32(&mut out, ir.animskin_uid)?;
    write_u32(&mut out, checked_u32(ir.bones.len(), "bone count")?)?;
    for bone in &ir.bones {
        write_string(&mut out, &bone.name)?;
        write_i32(&mut out, bone.parent)?;
        write_f32_slice(&mut out, &bone.local_position)?;
        write_f32_slice(&mut out, &bone.global_position)?;
    }
    write_u32(&mut out, checked_u32(ir.meshes.len(), "mesh count")?)?;
    for mesh in &ir.meshes {
        write_string(&mut out, &mesh.name)?;
        write_u32(&mut out, checked_u32(mesh.vertices.len(), "vertex count")?)?;
        for (vertex, influence) in mesh.vertices.iter().zip(&mesh.influences) {
            write_f32_slice(&mut out, &vertex.pos)?;
            write_f32_slice(&mut out, &vertex.norm)?;
            write_f32_slice(&mut out, &vertex.uv)?;
            write_f32_slice(&mut out, &vertex.color)?;
            for bone in influence.bone_indices {
                out.write_all(&bone.to_le_bytes())?;
            }
            write_f32_slice(&mut out, &influence.weights)?;
        }
        write_u32(&mut out, checked_u32(mesh.indices.len(), "index count")?)?;
        for index in &mesh.indices {
            write_u32(&mut out, *index)?;
        }
        write_u32(
            &mut out,
            checked_u32(mesh.triangle_materials.len(), "triangle count")?,
        )?;
        for material in &mesh.triangle_materials {
            write_u32(&mut out, *material)?;
        }
        write_u32(
            &mut out,
            checked_u32(mesh.materials.len(), "material count")?,
        )?;
        for material in &mesh.materials {
            write_u32(&mut out, material.hashcode)?;
            write_string(&mut out, &material.name)?;
        }
    }
    write_u32(
        &mut out,
        checked_u32(ir.clips.len(), "animation clip count")?,
    )?;
    for clip in &ir.clips {
        write_string(&mut out, &clip.name)?;
        write_u32(&mut out, clip.animation_uid)?;
        write_u32(&mut out, clip.source_animation_index)?;
        write_u32(&mut out, clip.source_script_uid)?;
        write_u32(&mut out, clip.source_script_command)?;
        write_u32(&mut out, clip.usage_count)?;
        write_f32(&mut out, clip.source_script_fps)?;
        write_u32(&mut out, clip.source_command_length)?;
        write_f32(&mut out, clip.sample_rate)?;
        write_u32(&mut out, clip.frame_count)?;
        write_f32(&mut out, clip.duration_seconds)?;
        write_string(&mut out, &clip.root_motion_mode)?;
        write_u32(
            &mut out,
            checked_u32(clip.poses.len(), "animation pose count")?,
        )?;
        for pose in &clip.poses {
            write_f32_slice(&mut out, &pose.position)?;
            write_f32_slice(&mut out, &pose.rotation)?;
            write_f32_slice(&mut out, &pose.scale)?;
        }
    }
    out.flush()?;
    Ok(())
}

fn write_ir_manifest(
    path: &Path,
    source_path: &Path,
    ir: &CharacterIr,
    ir_path: &Path,
    model_fbx_path: &Path,
    animation_outputs: &[(PathBuf, PathBuf)],
) -> anyhow::Result<()> {
    let triangle_count = ir
        .meshes
        .iter()
        .map(|mesh| mesh.indices.len() / 3)
        .sum::<usize>();
    let vertex_count = ir
        .meshes
        .iter()
        .map(|mesh| mesh.vertices.len())
        .sum::<usize>();
    let animations = ir
        .clips
        .iter()
        .zip(animation_outputs)
        .map(|(clip, (fbx_path, report_path))| {
            json!({
                "animation_uid": format!("0x{:08X}", clip.animation_uid),
                "decoded_name": clip.name,
                "animation_source_edb_uid": format!("0x{:08X}", clip.animation_source_edb_uid),
                "animation_source_file": clip.animation_source_path,
                "source_animation_index": clip.source_animation_index,
                "source_script_edb_uid": format!("0x{:08X}", clip.source_script_edb_uid),
                "source_script_uid": format!("0x{:08X}", clip.source_script_uid),
                "source_script_command": clip.source_script_command,
                "usage_count": clip.usage_count,
                "source_script_fps": clip.source_script_fps,
                "source_command_length": clip.source_command_length,
                "sample_rate": clip.sample_rate,
                "frame_count": clip.frame_count,
                "duration_seconds": clip.duration_seconds,
                "root_motion_mode": clip.root_motion_mode,
                "pose_cache": clip.pose_cache_path,
                "output_file": fbx_path,
                "report_file": report_path,
            })
        })
        .collect::<Vec<_>>();
    let skipped_animations = ir
        .skipped_animations
        .iter()
        .map(|clip| {
            json!({
                "animation_uid": format!("0x{:08X}", clip.animation_uid),
                "animation_source_edb_uid": format!("0x{:08X}", clip.animation_source_edb_uid),
                "animation_source_file": clip.animation_source_path,
                "source_animation_index": clip.source_animation_index,
                "reason": clip.reason,
            })
        })
        .collect::<Vec<_>>();
    let manifest = json!({
        "schema": "eurochef-fbx-character-ir-v2",
        "source_file": source_path,
        "source_edb_uid": format!("0x{:08X}", ir.source_edb_uid),
        "animskin_uid": format!("0x{:08X}", ir.animskin_uid),
        "decoded_name": ir.name,
        "ir_file": ir_path,
        "model_output_file": model_fbx_path,
        "source_units": "EuroChef world units (meters in existing glTF path)",
        "target_units": "centimeters",
        "target_axis_system": "MayaZUp (+Z up, -Y front, right-handed)",
        "coordinate_transform": "(-x, -z, y) * 100",
        "bone_count": ir.bones.len(),
        "mesh_count": ir.meshes.len(),
        "vertex_count": vertex_count,
        "triangle_count": triangle_count,
        "material_slots": ir.meshes.iter().map(|mesh| mesh.materials.len()).sum::<usize>(),
        "contains_animation": !ir.clips.is_empty(),
        "animation_clip_count": ir.clips.len(),
        "animations": animations,
        "skipped_animations": skipped_animations,
        "animation_timing_policy": "one animation-only FBX per unique AnimScript FPS/duration; key times span command_length/script_fps including first and last pose frame; no implicit 30 FPS",
    });
    fs::write(path, serde_json::to_vec_pretty(&manifest)?)?;
    Ok(())
}

fn animation_output_stem(
    character_base: &str,
    target_edb_uid: u32,
    clip: &AnimationClipIr,
    clip_index: usize,
    all_clips: &[AnimationClipIr],
) -> String {
    let same_uid_count = all_clips
        .iter()
        .filter(|candidate| {
            candidate.animation_source_edb_uid == clip.animation_source_edb_uid
                && candidate.animation_uid == clip.animation_uid
        })
        .count();
    let source_suffix = if clip.animation_source_edb_uid == target_edb_uid {
        String::new()
    } else {
        format!("_SRC_{:08X}", clip.animation_source_edb_uid)
    };
    let timing_suffix = if same_uid_count > 1 {
        format!("_T{clip_index:03}")
    } else {
        String::new()
    };
    format!(
        "{character_base}__{}_[0x{:08X}]{source_suffix}{timing_suffix}",
        sanitize_name(&clip.name),
        clip.animation_uid
    )
}

fn vector3(value: &[f32; 4]) -> [f32; 3] {
    [value[0], value[1], value[2]]
}

fn sanitize_name(value: &str) -> String {
    let mut output = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    while output.contains("__") {
        output = output.replace("__", "_");
    }
    output.trim_matches('_').to_string()
}

fn checked_u32(value: usize, field: &str) -> anyhow::Result<u32> {
    u32::try_from(value).with_context(|| format!("{field} exceeds u32"))
}

fn write_u32(out: &mut impl Write, value: u32) -> std::io::Result<()> {
    out.write_all(&value.to_le_bytes())
}

fn write_i32(out: &mut impl Write, value: i32) -> std::io::Result<()> {
    out.write_all(&value.to_le_bytes())
}

fn write_f32(out: &mut impl Write, value: f32) -> std::io::Result<()> {
    out.write_all(&value.to_le_bytes())
}

fn write_f32_slice(out: &mut impl Write, values: &[f32]) -> std::io::Result<()> {
    for value in values {
        write_f32(out, *value)?;
    }
    Ok(())
}

fn write_string(out: &mut impl Write, value: &str) -> anyhow::Result<()> {
    let bytes = value.as_bytes();
    write_u32(out, checked_u32(bytes.len(), "string length")?)?;
    out.write_all(bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filename_sanitizer_is_deterministic() {
        assert_eq!(
            sanitize_name("HT_AnimSkin Foo [0x0D000001]"),
            "HT_AnimSkin_Foo_0x0D000001"
        );
    }

    #[test]
    fn character_validator_rejects_non_preceding_parent() {
        let character = CharacterIr {
            name: "test".to_string(),
            source_edb_uid: 1,
            animskin_uid: 2,
            bones: vec![BoneIr {
                name: "bone_000".to_string(),
                parent: 0,
                local_position: [0.0; 3],
                global_position: [0.0; 3],
            }],
            meshes: Vec::new(),
            clips: Vec::new(),
            skipped_animations: Vec::new(),
        };
        assert!(validate_character_ir(&character).is_err());
    }

    #[test]
    fn ir_header_is_stable() {
        let character = CharacterIr {
            name: "test".to_string(),
            source_edb_uid: 1,
            animskin_uid: 2,
            bones: vec![BoneIr {
                name: "bone_000".to_string(),
                parent: -1,
                local_position: [0.0; 3],
                global_position: [0.0; 3],
            }],
            meshes: Vec::new(),
            clips: Vec::new(),
            skipped_animations: Vec::new(),
        };
        let mut path = std::env::temp_dir();
        path.push(format!("eurochef-fbx-ir-{}.bin", std::process::id()));
        write_ir(&path, &character).expect("write IR");
        let bytes = fs::read(&path).expect("read IR");
        let _ = fs::remove_file(path);
        assert_eq!(&bytes[..8], IR_MAGIC);
        assert_eq!(
            u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
            IR_VERSION
        );
    }

    #[test]
    fn timing_preserves_pose_frame_count_and_script_duration() {
        let variants = timing_variants(
            61,
            &[AnimationUsageTiming {
                script_edb_uid: 1,
                script_uid: 1,
                command_index: 2,
                script_fps: 24.0,
                command_length: 48,
            }],
        );
        assert_eq!(variants.len(), 1);
        assert!((variants[0].duration_seconds - 2.0).abs() <= 1.0e-6);
        assert!((variants[0].sample_rate - 24.0).abs() <= 1.0e-6);
        assert_eq!(variants[0].source_command_length, 48);
    }

    #[test]
    fn quaternion_sign_continuity_preserves_last_frame_pose() {
        let mut poses = vec![
            PoseIr::IDENTITY,
            PoseIr {
                rotation: [0.0, 0.0, 0.0, -1.0],
                ..PoseIr::IDENTITY
            },
        ];
        enforce_quaternion_sign_continuity(&mut poses, 2, 1);
        assert_eq!(poses[1].rotation, PoseIr::IDENTITY.rotation);
    }

    #[test]
    fn synthetic_root_is_inserted_for_every_frame() {
        let source = vec![
            PoseIr {
                position: [1.0, 0.0, 0.0],
                ..PoseIr::IDENTITY
            },
            PoseIr {
                position: [2.0, 0.0, 0.0],
                ..PoseIr::IDENTITY
            },
        ];
        let output = add_synthetic_root_poses(&source, 2, 1, 1).expect("synthetic root");
        assert_eq!(output.len(), 4);
        assert_eq!(output[0].position, [0.0; 3]);
        assert_eq!(output[1].position, [1.0, 0.0, 0.0]);
        assert_eq!(output[2].position, [0.0; 3]);
        assert_eq!(output[3].position, [2.0, 0.0, 0.0]);
    }
}
