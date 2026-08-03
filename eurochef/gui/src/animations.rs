use std::{
    collections::HashMap,
    fs,
    io::{Read, Seek, SeekFrom},
    mem::size_of,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Context;
use egui::{
    mutex::{Mutex, RwLock},
    RichText,
};
use eurochef_edb::{
    anim::EXGeoBaseAnimSkin, binrw::BinReaderExt, edb::EdbFile, entity::EXGeoEntity,
    header::EXGeoHeader, versions::Platform, Hashcode, HashcodeUtils,
};
use eurochef_shared::{
    entities::UXVertex,
    maps::format_hashcode,
    script::{UXGeoScript, UXGeoScriptCommandData},
    IdentifiableResult,
};
use glam::{Quat, Vec2, Vec3};
use glow::HasContext;
use instant::Instant;
use nohash_hasher::IntMap;

use crate::{
    entities::ProcessedEntityMesh,
    render::{
        camera::ArcBallCamera,
        entity::EntityRenderer,
        viewer::{BaseViewer, RenderContext},
        RenderStore,
    },
};

mod skinning;

use skinning::{
    bind_pose_skin_matrices, build_skin_matrices, matrix_max_abs_difference, skin_vertices,
    AnimationBonePose,
};

const MAX_CAPTURED_MOTION_BYTES: usize = 16 * 1024 * 1024;
const DEFAULT_PREVIEW_SECONDS: f32 = 1.0;
const POSE_CACHE_MAGIC: &[u8; 8] = b"RAPCV002";
const POSE_CACHE_HEADER_SIZE: usize = 40;
const POSE_CACHE_VALUES_PER_BONE: usize = 7;
const POSE_CACHE_BYTES_PER_BONE: usize = POSE_CACHE_VALUES_PER_BONE * size_of::<f32>();
const MAX_POSE_CACHE_BYTES: usize = 512 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct AnimationMotionData {
    pub expected_size: usize,
    pub bytes: Vec<u8>,
    pub truncated: bool,
    pub read_error: Option<String>,
    pub checksum: u64,
}

#[derive(Debug, Clone)]
pub struct AnimationPoseCache {
    pub source_path: PathBuf,
    pub frame_count: usize,
    pub bone_count: usize,
    pub motion_checksum: u64,
    poses: Vec<AnimationBonePose>,
}

impl AnimationPoseCache {
    fn frame_at_phase(&self, phase: f32) -> f32 {
        if self.frame_count <= 1 {
            return 0.0;
        }
        phase.clamp(0.0, 1.0) * (self.frame_count - 1) as f32
    }

    fn sample_phase(&self, phase: f32) -> Option<Vec<AnimationBonePose>> {
        self.sample_frame(self.frame_at_phase(phase))
    }

    fn sample_frame(&self, raw_frame: f32) -> Option<Vec<AnimationBonePose>> {
        if self.frame_count == 0
            || self.bone_count == 0
            || self.poses.len() != self.frame_count.checked_mul(self.bone_count)?
        {
            return None;
        }
        let clamped = raw_frame
            .max(0.0)
            .min((self.frame_count.saturating_sub(1)) as f32);
        let current_frame = clamped.floor() as usize;
        let next_frame = (current_frame + 1).min(self.frame_count - 1);
        let fraction = clamped.fract();

        let current_start = current_frame.checked_mul(self.bone_count)?;
        let next_start = next_frame.checked_mul(self.bone_count)?;
        let current = self
            .poses
            .get(current_start..current_start.checked_add(self.bone_count)?)?;
        let next = self
            .poses
            .get(next_start..next_start.checked_add(self.bone_count)?)?;
        Some(
            current
                .iter()
                .zip(next)
                .map(|(current, next)| {
                    let mut next_rotation = next.rotation;
                    if current.rotation.dot(next_rotation) < 0.0 {
                        next_rotation = Quat::from_xyzw(
                            -next_rotation.x,
                            -next_rotation.y,
                            -next_rotation.z,
                            -next_rotation.w,
                        );
                    }
                    AnimationBonePose {
                        position: current.position.lerp(next.position, fraction),
                        rotation: current.rotation.slerp(next_rotation, fraction).normalize(),
                    }
                })
                .collect(),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimationVertexInfluence {
    pub bone_indices: [u8; 4],
    pub weights: [f32; 4],
}

#[derive(Debug, Clone)]
pub struct AnimationPartSkin {
    pub part_index: usize,
    pub vertex_count: usize,
    pub influences: Vec<AnimationVertexInfluence>,
}

#[derive(Debug, Clone)]
pub struct AnimationComponent {
    pub group: &'static str,
    pub component_index: usize,
    pub raw_entity_index: u32,
    pub entity_index: usize,
    pub entity_hashcode: Option<Hashcode>,
    pub section_index: u32,
    pub parts_count: u32,
    pub morph_index: i32,
    pub part_skins: Vec<AnimationPartSkin>,
}

#[derive(Debug, Clone)]
pub struct AnimationSkinRecord {
    pub index: usize,
    pub hashcode: Hashcode,
    pub base_skin_num: u32,
    pub mip_ref: u32,
    pub parsed: Option<EXGeoBaseAnimSkin>,
    pub parse_error: Option<String>,
    pub components: Vec<AnimationComponent>,
    pub center: Vec3,
    pub maximum_extent: f32,
    pub bind_pose_identity_error: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct AnimationUsage {
    pub script_hashcode: Hashcode,
    pub command_index: usize,
    pub start_frame: i16,
    pub length_frames: u16,
    pub script_fps: f32,
    pub skin_file: Hashcode,
    pub skin_hashcode: Hashcode,
}

#[derive(Debug, Clone)]
pub struct AnimationClipRecord {
    pub index: usize,
    pub hashcode: Hashcode,
    pub file_offset: u32,
    pub motiondata_info_addr: u32,
    pub data_size: u32,
    pub skin_num: u32,
    pub skin_index: Option<usize>,
    pub motion: AnimationMotionData,
    pub pose_cache: Option<AnimationPoseCache>,
    pub pose_cache_error: Option<String>,
    pub usages: Vec<AnimationUsage>,
    pub preview_duration: f32,
}

#[derive(Debug, Clone, Default)]
pub struct AnimationCatalog {
    pub clips: Vec<AnimationClipRecord>,
    pub skins: Vec<AnimationSkinRecord>,
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
    if let Ok(executable) = std::env::current_exe() {
        if let Some(parent) = executable.parent() {
            roots.push(parent.join("../robots_animation_pose_cache"));
            roots.push(parent.join("../../robots_animation_pose_cache"));
        }
    }
    roots.sort();
    roots.dedup();
    roots
}

fn parse_pose_cache(
    source_path: &Path,
    bytes: &[u8],
    expected_edb_uid: Hashcode,
    expected_animation_index: usize,
    expected_animation_hashcode: Hashcode,
    expected_animskin_hashcode: Hashcode,
    expected_motion_checksum: u64,
) -> Result<AnimationPoseCache, String> {
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
    let animation_hashcode = read_u32_le(bytes, 0x10)?;
    let animskin_hashcode = read_u32_le(bytes, 0x14)?;
    let frame_count = read_u32_le(bytes, 0x18)? as usize;
    let bone_count = read_u32_le(bytes, 0x1C)? as usize;
    let motion_checksum = read_u64_le(bytes, 0x20)?;

    if edb_uid != expected_edb_uid {
        return Err(format!(
            "EDB UID mismatch 0x{edb_uid:08X} != 0x{expected_edb_uid:08X}"
        ));
    }
    if animation_index != expected_animation_index {
        return Err(format!(
            "Animation index mismatch {animation_index} != {expected_animation_index}"
        ));
    }
    if animation_hashcode != expected_animation_hashcode {
        return Err(format!(
            "Animation hash mismatch 0x{animation_hashcode:08X} != 0x{expected_animation_hashcode:08X}"
        ));
    }
    if animskin_hashcode != expected_animskin_hashcode {
        return Err(format!(
            "AnimSkin hash mismatch 0x{animskin_hashcode:08X} != 0x{expected_animskin_hashcode:08X}"
        ));
    }
    if motion_checksum != expected_motion_checksum {
        return Err(format!(
            "motion checksum mismatch 0x{motion_checksum:016X} != 0x{expected_motion_checksum:016X}"
        ));
    }
    if frame_count == 0 || bone_count == 0 || bone_count > u8::MAX as usize {
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
        let position = Vec3::new(
            read_f32_le(bytes, offset)?,
            read_f32_le(bytes, offset + 4)?,
            read_f32_le(bytes, offset + 8)?,
        );
        let rotation = Quat::from_xyzw(
            read_f32_le(bytes, offset + 12)?,
            read_f32_le(bytes, offset + 16)?,
            read_f32_le(bytes, offset + 20)?,
            read_f32_le(bytes, offset + 24)?,
        );
        if !position.is_finite() || !rotation.is_finite() {
            return Err(format!("non-finite pose at flattened index {pose_index}"));
        }
        let length = rotation.length();
        if (length - 1.0).abs() > 2.0e-3 {
            return Err(format!(
                "non-unit quaternion at flattened index {pose_index}: {length}"
            ));
        }
        poses.push(AnimationBonePose {
            position,
            rotation: rotation.normalize(),
        });
    }

    Ok(AnimationPoseCache {
        source_path: source_path.to_path_buf(),
        frame_count,
        bone_count,
        motion_checksum,
        poses,
    })
}

fn load_pose_cache(
    edb_uid: Hashcode,
    animation_index: usize,
    animation_hashcode: Hashcode,
    animskin_hashcode: Hashcode,
    motion_checksum: u64,
) -> (Option<AnimationPoseCache>, Option<String>) {
    let relative =
        PathBuf::from(format!("{edb_uid:08X}")).join(format!("{animation_index:04}.rapc"));
    for root in pose_cache_roots() {
        let path = root.join(&relative);
        if !path.is_file() {
            continue;
        }
        return match fs::read(&path) {
            Ok(bytes) => match parse_pose_cache(
                &path,
                &bytes,
                edb_uid,
                animation_index,
                animation_hashcode,
                animskin_hashcode,
                motion_checksum,
            ) {
                Ok(cache) => (Some(cache), None),
                Err(error) => (None, Some(format!("{}: {error}", path.display()))),
            },
            Err(error) => (
                None,
                Some(format!("could not read {}: {error}", path.display())),
            ),
        };
    }
    (None, None)
}

pub fn read_from_file(edb: &mut EdbFile) -> anyhow::Result<AnimationCatalog> {
    let header = edb.header.clone();
    let saved_position = edb.stream_position()?;
    let mut skins = Vec::with_capacity(header.animskin_list.len());
    let mut entity_part_vertex_counts: HashMap<usize, Vec<usize>> = HashMap::new();

    for (index, skin_header) in header.animskin_list.iter().enumerate() {
        edb.seek(SeekFrom::Start(skin_header.common.address as u64))?;
        let parsed = edb.read_type_args::<EXGeoBaseAnimSkin>(edb.endian, (header.version,));
        let (parsed, parse_error) = match parsed {
            Ok(skin) => (Some(skin), None),
            Err(error) => (None, Some(error.to_string())),
        };

        let components = if let Some(skin) = parsed.as_ref() {
            collect_components(edb, &header, skin, &mut entity_part_vertex_counts)?
        } else {
            Vec::new()
        };

        let bind_pose_identity_error = parsed.as_ref().and_then(|skin| {
            bind_pose_skin_matrices(skin).map(|matrices| {
                matrices
                    .into_iter()
                    .map(|matrix| matrix_max_abs_difference(matrix, glam::Mat4::IDENTITY))
                    .fold(0.0, f32::max)
            })
        });

        skins.push(AnimationSkinRecord {
            index,
            hashcode: skin_header.common.hashcode,
            base_skin_num: skin_header.base_skin_num,
            mip_ref: skin_header.mip_ref,
            parsed,
            parse_error,
            components,
            center: Vec3::ZERO,
            maximum_extent: 1.0,
            bind_pose_identity_error,
        });
    }

    let mut clips = Vec::with_capacity(header.anim_list.len());
    for (index, animation) in header.anim_list.iter().enumerate() {
        let skin_index = skins
            .iter()
            .position(|skin| skin.base_skin_num == animation.skin_num);
        let motion = read_motion_data(
            edb,
            &header,
            animation.motiondata_info_addr,
            animation.datasize,
        );
        let (pose_cache, pose_cache_error) = skin_index
            .and_then(|skin_index| skins.get(skin_index))
            .map(|skin| {
                load_pose_cache(
                    header.hashcode,
                    index,
                    animation.common.hashcode,
                    skin.hashcode,
                    motion.checksum,
                )
            })
            .unwrap_or((None, None));

        clips.push(AnimationClipRecord {
            index,
            hashcode: animation.common.hashcode,
            file_offset: animation.common.address,
            motiondata_info_addr: animation.motiondata_info_addr,
            data_size: animation.datasize,
            skin_num: animation.skin_num,
            skin_index,
            motion,
            pose_cache,
            pose_cache_error,
            usages: Vec::new(),
            preview_duration: DEFAULT_PREVIEW_SECONDS,
        });
    }

    edb.seek(SeekFrom::Start(saved_position))?;
    Ok(AnimationCatalog { clips, skins })
}

fn collect_components(
    edb: &mut EdbFile,
    header: &EXGeoHeader,
    skin: &EXGeoBaseAnimSkin,
    vertex_count_cache: &mut HashMap<usize, Vec<usize>>,
) -> anyhow::Result<Vec<AnimationComponent>> {
    let mut components = Vec::new();
    let endian = edb.endian;
    for (group, entries) in [
        ("primary", skin.entities.data().as_slice()),
        ("secondary", skin.more_entities.data().as_slice()),
    ] {
        for (component_index, component) in entries.iter().enumerate() {
            let entity_index = (component.entity_index & 0x00ff_ffff) as usize;
            let entity_hashcode = header
                .entity_list
                .data()
                .get(entity_index)
                .map(|entity| entity.common.hashcode);
            let vertex_counts = if let Some(counts) = vertex_count_cache.get(&entity_index) {
                counts.clone()
            } else {
                let counts = read_entity_mesh_vertex_counts(edb, header, entity_index)?;
                vertex_count_cache.insert(entity_index, counts.clone());
                counts
            };

            anyhow::ensure!(
                component.skin_data.len() == vertex_counts.len(),
                "AnimSkin component {}:{} has {} weight payloads but Entity {} has {} mesh parts",
                group,
                component_index,
                component.skin_data.len(),
                entity_index,
                vertex_counts.len()
            );

            let mut part_skins = Vec::with_capacity(vertex_counts.len());
            for (part_index, (payload, vertex_count)) in component
                .skin_data
                .iter()
                .zip(vertex_counts.iter().copied())
                .enumerate()
            {
                let palette = payload.bone_palette.as_slice();
                let influences = payload
                    .read_vertex_influences(edb, endian, vertex_count)?
                    .into_iter()
                    .enumerate()
                    .map(|(vertex_index, influence)| {
                        let bone_indices = influence.bone_indices(palette).with_context(|| {
                            format!(
                                "invalid skin selector in component {}:{}, part {}, vertex {}",
                                group, component_index, part_index, vertex_index
                            )
                        })?;
                        anyhow::ensure!(
                            bone_indices
                                .iter()
                                .all(|bone_index| usize::from(*bone_index) < skin.bone_count as usize),
                            "skin influence references bone outside AnimSkin at component {}:{}, part {}, vertex {}",
                            group,
                            component_index,
                            part_index,
                            vertex_index
                        );
                        anyhow::ensure!(
                            influence.weights.iter().all(|weight| weight.is_finite())
                                && (influence.weights.iter().sum::<f32>() - 1.0).abs() <= 1.0e-4,
                            "invalid skin weights at component {}:{}, part {}, vertex {}",
                            group,
                            component_index,
                            part_index,
                            vertex_index
                        );
                        Ok(AnimationVertexInfluence {
                            bone_indices,
                            weights: influence.weights,
                        })
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?;
                part_skins.push(AnimationPartSkin {
                    part_index,
                    vertex_count,
                    influences,
                });
            }

            components.push(AnimationComponent {
                group,
                component_index,
                raw_entity_index: component.entity_index,
                entity_index,
                entity_hashcode,
                section_index: component.section_index,
                parts_count: component.parts_count,
                morph_index: component.morph_index,
                part_skins,
            });
        }
    }
    Ok(components)
}

fn read_entity_mesh_vertex_counts(
    edb: &mut EdbFile,
    header: &EXGeoHeader,
    entity_index: usize,
) -> anyhow::Result<Vec<usize>> {
    let entity_header = header
        .entity_list
        .data()
        .get(entity_index)
        .context("AnimSkin Entity index outside Entity list")?;
    let saved_position = edb.stream_position()?;
    edb.seek(SeekFrom::Start(entity_header.common.address as u64))?;
    let entity: EXGeoEntity = edb.read_type_args(edb.endian, (header.version, edb.platform))?;
    edb.seek(SeekFrom::Start(saved_position))?;

    let mut counts = Vec::new();
    collect_entity_mesh_vertex_counts(&entity, &mut counts);
    Ok(counts)
}

fn collect_entity_mesh_vertex_counts(entity: &EXGeoEntity, counts: &mut Vec<usize>) {
    match entity {
        EXGeoEntity::Mesh(mesh) => counts.push(mesh.vertices.len()),
        EXGeoEntity::Split(split) => {
            for child in &split.entities {
                collect_entity_mesh_vertex_counts(child, counts);
            }
        }
        _ => {}
    }
}

fn read_motion_data(
    edb: &mut EdbFile,
    header: &EXGeoHeader,
    address: u32,
    expected_size: u32,
) -> AnimationMotionData {
    let expected_size = expected_size as usize;
    if expected_size == 0 {
        return AnimationMotionData {
            expected_size,
            bytes: Vec::new(),
            truncated: false,
            read_error: None,
            checksum: fnv1a64(&[]),
        };
    }

    let file_remaining = (header.file_size as usize).saturating_sub(address as usize);
    let capture_size = expected_size
        .min(file_remaining)
        .min(MAX_CAPTURED_MOTION_BYTES);
    let truncated = capture_size != expected_size;
    let mut bytes = vec![0; capture_size];
    let read_error = match edb
        .seek(SeekFrom::Start(address as u64))
        .and_then(|_| edb.read_exact(&mut bytes))
    {
        Ok(()) => None,
        Err(error) => {
            bytes.clear();
            Some(error.to_string())
        }
    };
    let checksum = fnv1a64(&bytes);

    AnimationMotionData {
        expected_size,
        bytes,
        truncated,
        read_error,
        checksum,
    }
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn attach_mesh_bounds(
    catalog: &mut AnimationCatalog,
    entities: &[(
        usize,
        IdentifiableResult<(EXGeoEntity, ProcessedEntityMesh)>,
    )],
) {
    for skin in &mut catalog.skins {
        let mut minimum = Vec3::splat(f32::MAX);
        let mut maximum = Vec3::splat(f32::MIN);
        let mut found = false;

        for component in &skin.components {
            let Some((_, entity)) = entities
                .iter()
                .find(|(entity_index, _)| *entity_index == component.entity_index)
            else {
                continue;
            };
            let Ok((_, mesh)) = &entity.data else {
                continue;
            };
            let bounds = mesh.bounding_box();
            minimum = minimum.min(bounds.0);
            maximum = maximum.max(bounds.1);
            found = true;
        }

        if found {
            skin.center = (minimum + maximum) * 0.5;
            skin.maximum_extent = (maximum - minimum).max_element().abs().max(0.1);
        }
    }
}

fn attach_script_usages(catalog: &mut AnimationCatalog, file: Hashcode, scripts: &[UXGeoScript]) {
    for script in scripts {
        let fps = script.timeline_framerate();
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

            if !anim_hashcode.is_local() && *anim_file != file {
                continue;
            }
            let Some(clip_index) = resolve_clip_index(&catalog.clips, *anim_hashcode) else {
                continue;
            };

            let usage = AnimationUsage {
                script_hashcode: script.hashcode,
                command_index,
                start_frame: command.start,
                length_frames: command.length,
                script_fps: fps,
                skin_file: *skin_file,
                skin_hashcode: *skin_hashcode,
            };
            let duration = f32::from(command.length.max(1)) / fps.max(f32::EPSILON);
            let clip = &mut catalog.clips[clip_index];
            clip.preview_duration = clip.preview_duration.max(duration);
            clip.usages.push(usage);
        }
    }
}

fn resolve_clip_index(clips: &[AnimationClipRecord], hashcode: Hashcode) -> Option<usize> {
    if hashcode.is_local() {
        clips.get(hashcode.index() as usize).map(|clip| clip.index)
    } else {
        clips.iter().position(|clip| clip.hashcode == hashcode)
    }
}

struct AnimationSkinnedEntity {
    renderer: EntityRenderer,
    original_vertices: Vec<UXVertex>,
    skinned_vertices: Vec<UXVertex>,
    part_vertex_ranges: Vec<std::ops::Range<usize>>,
    part_skins: Vec<AnimationPartSkin>,
}

fn build_skin_renderers(
    file: Hashcode,
    gl: &glow::Context,
    platform: Platform,
    catalog: &AnimationCatalog,
    entities: &[(
        usize,
        IdentifiableResult<(EXGeoEntity, ProcessedEntityMesh)>,
    )],
) -> Vec<Vec<AnimationSkinnedEntity>> {
    catalog
        .skins
        .iter()
        .map(|skin| {
            skin.components
                .iter()
                .filter_map(|component| {
                    let (_, entity) = entities
                        .iter()
                        .find(|(entity_index, _)| *entity_index == component.entity_index)?;
                    let (_, mesh) = entity.data.as_ref().ok()?;
                    if mesh.part_vertex_ranges.len() != component.part_skins.len() {
                        warn!(
                            "Animation skin component {}:{} has {} mesh ranges and {} influence parts",
                            component.group,
                            component.component_index,
                            mesh.part_vertex_ranges.len(),
                            component.part_skins.len()
                        );
                        return None;
                    }
                    let mut renderer = EntityRenderer::new(file, platform);
                    unsafe {
                        renderer.load_mesh(gl, mesh);
                    }
                    Some(AnimationSkinnedEntity {
                        renderer,
                        original_vertices: mesh.vertex_data.clone(),
                        skinned_vertices: mesh.vertex_data.clone(),
                        part_vertex_ranges: mesh.part_vertex_ranges.clone(),
                        part_skins: component.part_skins.clone(),
                    })
                })
                .collect()
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationRuntimeStatus {
    Rendered,
    MissingAnimation,
    MissingSkin,
    SkinMismatch,
    MissingPoseCache,
    InvalidPose,
    MissingGeometry,
}

pub struct AnimationRuntime {
    catalog: AnimationCatalog,
    skin_renderers: Arc<RwLock<Vec<Vec<AnimationSkinnedEntity>>>>,
}

impl AnimationRuntime {
    pub fn new(
        file: Hashcode,
        gl: &glow::Context,
        platform: Platform,
        catalog: AnimationCatalog,
        entities: &[(
            usize,
            IdentifiableResult<(EXGeoEntity, ProcessedEntityMesh)>,
        )],
    ) -> Self {
        Self {
            skin_renderers: Arc::new(RwLock::new(build_skin_renderers(
                file, gl, platform, &catalog, entities,
            ))),
            catalog,
        }
    }

    fn resolve_skin_index(&self, hashcode: Hashcode) -> Option<usize> {
        if hashcode.is_local() {
            let index = hashcode.index() as usize;
            self.catalog.skins.get(index).map(|_| index)
        } else {
            self.catalog
                .skins
                .iter()
                .position(|skin| skin.hashcode == hashcode)
        }
    }

    fn resolve_clip_skin_index(
        &self,
        clip: &AnimationClipRecord,
        requested_skin_hashcode: Hashcode,
    ) -> Option<usize> {
        // Shipped Animation commands commonly serialize 0xFFFFFFFF for the
        // skin reference. Robots treats that sentinel as "use the AnimSkin
        // bound by the Animation asset", not as local object index 65535.
        if requested_skin_hashcode == u32::MAX {
            clip.skin_index
        } else {
            self.resolve_skin_index(requested_skin_hashcode)
        }
    }

    pub fn status(
        &self,
        animation_hashcode: Hashcode,
        skin_hashcode: Hashcode,
    ) -> AnimationRuntimeStatus {
        let Some(clip_index) = resolve_clip_index(&self.catalog.clips, animation_hashcode) else {
            return AnimationRuntimeStatus::MissingAnimation;
        };
        let clip = &self.catalog.clips[clip_index];
        let Some(skin_index) = self.resolve_clip_skin_index(clip, skin_hashcode) else {
            return AnimationRuntimeStatus::MissingSkin;
        };
        if clip.skin_index != Some(skin_index) {
            return AnimationRuntimeStatus::SkinMismatch;
        }
        if clip.pose_cache.is_none() {
            return AnimationRuntimeStatus::MissingPoseCache;
        }
        if self
            .skin_renderers
            .read()
            .get(skin_index)
            .is_none_or(Vec::is_empty)
        {
            return AnimationRuntimeStatus::MissingGeometry;
        }
        AnimationRuntimeStatus::Rendered
    }

    #[allow(clippy::too_many_arguments)]
    pub unsafe fn draw(
        &self,
        gl: &glow::Context,
        render_context: &RenderContext<'_>,
        render_store: &RenderStore,
        animation_hashcode: Hashcode,
        skin_hashcode: Hashcode,
        phase: f32,
        position: Vec3,
        rotation: Quat,
        scale: Vec3,
        time: f64,
    ) -> AnimationRuntimeStatus {
        let Some(clip_index) = resolve_clip_index(&self.catalog.clips, animation_hashcode) else {
            return AnimationRuntimeStatus::MissingAnimation;
        };
        let clip = &self.catalog.clips[clip_index];
        let Some(skin_index) = self.resolve_clip_skin_index(clip, skin_hashcode) else {
            return AnimationRuntimeStatus::MissingSkin;
        };
        if clip.skin_index != Some(skin_index) {
            return AnimationRuntimeStatus::SkinMismatch;
        }
        let Some(cache) = clip.pose_cache.as_ref() else {
            return AnimationRuntimeStatus::MissingPoseCache;
        };
        let Some(poses) = cache.sample_phase(phase) else {
            return AnimationRuntimeStatus::InvalidPose;
        };
        let Some(skin) = self
            .catalog
            .skins
            .get(skin_index)
            .and_then(|skin| skin.parsed.as_ref())
        else {
            return AnimationRuntimeStatus::MissingSkin;
        };
        let Some(skin_matrices) = build_skin_matrices(skin, &poses) else {
            return AnimationRuntimeStatus::InvalidPose;
        };
        let mut all_skin_renderers = self.skin_renderers.write();
        let Some(entities) = all_skin_renderers.get_mut(skin_index) else {
            return AnimationRuntimeStatus::MissingGeometry;
        };
        if entities.is_empty() {
            return AnimationRuntimeStatus::MissingGeometry;
        }

        for entity in entities.iter_mut() {
            if skin_vertices(
                &entity.original_vertices,
                &mut entity.skinned_vertices,
                &entity.part_vertex_ranges,
                &entity.part_skins,
                &skin_matrices,
            )
            .is_some()
            {
                entity.renderer.update_vertices(gl, &entity.skinned_vertices);
            }
            entity.renderer.draw_opaque(
                gl,
                render_context,
                position,
                rotation,
                scale,
                time,
                render_store,
            );
        }

        gl.depth_mask(false);
        for entity in entities.iter() {
            entity.renderer.draw_transparent(
                gl,
                render_context,
                position,
                rotation,
                scale,
                time,
                render_store,
            );
        }
        gl.depth_mask(true);
        AnimationRuntimeStatus::Rendered
    }
}

fn semantic_script_reference(
    hashcodes: &IntMap<Hashcode, String>,
    script_hashcode: Hashcode,
) -> String {
    if script_hashcode.is_local() {
        format!("Script #{}", script_hashcode.index())
    } else {
        format_hashcode(hashcodes, script_hashcode)
    }
}

fn semantic_animation_label(
    catalog: &AnimationCatalog,
    clip: &AnimationClipRecord,
    hashcodes: &IntMap<Hashcode, String>,
) -> String {
    if !clip.hashcode.is_local() {
        let name = format_hashcode(hashcodes, clip.hashcode);
        if !name.contains("_Unknown_") && !name.contains("HT_Invalid") {
            return name;
        }
    }

    let mut parts = vec![format!("Animation #{}", clip.index)];
    if let Some(cache) = clip.pose_cache.as_ref() {
        parts.push(format!("{} asset frames", cache.frame_count));
    }
    if let Some(skin_index) = clip.skin_index {
        let skin_label = catalog
            .skins
            .get(skin_index)
            .map(|skin| {
                if skin.hashcode.is_local() {
                    format!("AnimSkin #{}", skin.index)
                } else {
                    format_hashcode(hashcodes, skin.hashcode)
                }
            })
            .unwrap_or_else(|| format!("AnimSkin index {skin_index}"));
        parts.push(skin_label);
    } else {
        parts.push("no AnimSkin binding".to_string());
    }

    match clip.usages.as_slice() {
        [] => parts.push("not referenced by Scripts".to_string()),
        [usage] => parts.push(format!(
            "used by {} command {}",
            semantic_script_reference(hashcodes, usage.script_hashcode),
            usage.command_index
        )),
        usages => parts.push(format!("used by {} Script commands", usages.len())),
    }
    parts.join(" · ")
}

pub struct AnimationListPanel {
    catalog: AnimationCatalog,
    selected_clip: usize,
    filter: String,
    viewer: Arc<Mutex<BaseViewer>>,
    render_store: Arc<RwLock<RenderStore>>,
    skin_renderers: Arc<RwLock<Vec<Vec<AnimationSkinnedEntity>>>>,
    hashcodes: Arc<IntMap<Hashcode, String>>,
    current_time: f32,
    playback_speed: f32,
    is_playing: bool,
    loop_animation: bool,
    last_frame: Instant,
}

impl AnimationListPanel {
    pub fn new(
        file: Hashcode,
        gl: &glow::Context,
        mut catalog: AnimationCatalog,
        entities: &[(
            usize,
            IdentifiableResult<(EXGeoEntity, ProcessedEntityMesh)>,
        )],
        scripts: &[UXGeoScript],
        platform: Platform,
        render_store: Arc<RwLock<RenderStore>>,
        hashcodes: Arc<IntMap<Hashcode, String>>,
    ) -> Self {
        attach_mesh_bounds(&mut catalog, entities);
        attach_script_usages(&mut catalog, file, scripts);
        let skin_renderers = Arc::new(RwLock::new(build_skin_renderers(
            file, gl, platform, &catalog, entities,
        )));
        let selected_clip = {
            let renderers = skin_renderers.read();
            catalog
                .clips
                .iter()
                .position(|clip| {
                    clip.skin_index
                        .and_then(|skin_index| renderers.get(skin_index))
                        .is_some_and(|entities| !entities.is_empty())
                })
                .unwrap_or(0)
        };
        let viewer = Arc::new(Mutex::new(BaseViewer::new(gl)));
        if let Some(skin) = catalog
            .clips
            .get(selected_clip)
            .and_then(|clip| clip.skin_index)
            .and_then(|skin_index| catalog.skins.get(skin_index))
        {
            viewer.lock().camera_orbit = ArcBallCamera::new(
                Vec3::ZERO,
                Vec2::new(15.0, 140.0),
                skin.maximum_extent * 1.25,
                false,
            );
        }

        Self {
            catalog,
            selected_clip,
            filter: String::new(),
            viewer,
            render_store,
            skin_renderers,
            hashcodes,
            current_time: 0.0,
            playback_speed: 1.0,
            is_playing: false,
            loop_animation: false,
            last_frame: Instant::now(),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let delta_time = self.last_frame.elapsed().as_secs_f32();
        self.last_frame = Instant::now();

        let available = ui.available_size();
        let sidebar_width = (available.x * 0.30).clamp(240.0, 340.0);
        let content_width = (available.x - sidebar_width - 12.0).max(1.0);
        let content_height = available.y.max(1.0);

        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(sidebar_width, content_height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_min_width(sidebar_width);
                    ui.set_max_width(sidebar_width);
                    ui.heading("Animations");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.filter)
                            .hint_text("Filter name or hashcode"),
                    );
                    ui.separator();

                    let filter = self.filter.trim().to_ascii_lowercase();
                    egui::ScrollArea::vertical()
                        .id_salt("animation_list_scroll")
                        .auto_shrink([false, false])
                        .max_height(ui.available_height())
                        .show(ui, |ui| {
                            for index in 0..self.catalog.clips.len() {
                                let clip = &self.catalog.clips[index];
                                let name = semantic_animation_label(
                                    &self.catalog,
                                    clip,
                                    &self.hashcodes,
                                );
                                let label = format!("{name}  [0x{:08X}]", clip.hashcode);
                                if !filter.is_empty()
                                    && !label.to_ascii_lowercase().contains(&filter)
                                {
                                    continue;
                                }

                                let hover_text = format!(
                                    "Index {}\nMotion bytes {}\nAnimSkin {}",
                                    clip.index,
                                    clip.data_size,
                                    clip.skin_index
                                        .and_then(|skin_index| self.catalog.skins.get(skin_index))
                                        .map(|skin| format!("0x{:08X}", skin.hashcode))
                                        .unwrap_or_else(|| "none".to_string())
                                );
                                let response =
                                    ui.selectable_label(self.selected_clip == index, label);
                                let clicked = response.clicked();
                                response.on_hover_text(hover_text);
                                if clicked {
                                    self.select_clip(index);
                                }
                            }
                        });
                },
            );

            ui.separator();
            ui.allocate_ui_with_layout(
                egui::vec2(content_width, content_height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_min_width(content_width);
                    ui.set_max_width(content_width);
                    ui.heading("Animation preview");
                    ui.horizontal_wrapped(|ui| {
                        self.viewer.lock().show_toolbar(ui);
                        ui.separator();
                        ui.label("Speed");
                        ui.add(
                            egui::DragValue::new(&mut self.playback_speed)
                                .range(0.05..=4.0)
                                .speed(0.01),
                        );
                        ui.checkbox(&mut self.loop_animation, "Loop");
                    });

                    let preview_height = (ui.available_height() * 0.64)
                        .clamp(240.0, 720.0)
                        .min(ui.available_height().max(1.0));
                    egui::Frame::canvas(ui.style())
                        .show(ui, |ui| self.show_canvas(ui, preview_height));
                    self.show_playback_controls(ui);
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .id_salt("animation_details_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| self.show_selected_details(ui));
                },
            );
        });

        if self.is_playing {
            self.current_time += delta_time * self.playback_speed;
            ui.ctx().request_repaint();
        }
        let duration = self.selected_duration();
        if self.current_time > duration {
            if self.loop_animation {
                self.current_time = if duration > f32::EPSILON {
                    self.current_time % duration
                } else {
                    0.0
                };
            } else {
                self.current_time = duration;
                self.is_playing = false;
            }
        }
    }

    fn select_clip(&mut self, index: usize) {
        self.selected_clip = index;
        self.current_time = 0.0;
        self.is_playing = false;
        if let Some(skin) = self.selected_skin() {
            self.viewer.lock().camera_orbit = ArcBallCamera::new(
                Vec3::ZERO,
                Vec2::new(15.0, 140.0),
                skin.maximum_extent * 1.25,
                false,
            );
        }
    }

    fn selected_clip(&self) -> Option<&AnimationClipRecord> {
        self.catalog.clips.get(self.selected_clip)
    }

    fn selected_skin(&self) -> Option<&AnimationSkinRecord> {
        let clip = self.selected_clip()?;
        self.catalog.skins.get(clip.skin_index?)
    }

    fn selected_duration(&self) -> f32 {
        self.selected_clip()
            .map(|clip| {
                clip.usages
                    .iter()
                    .filter_map(|usage| {
                        (usage.script_fps.is_finite()
                            && usage.script_fps > f32::EPSILON
                            && usage.length_frames > 0)
                            .then_some(usage.length_frames as f32 / usage.script_fps)
                    })
                    .fold(None::<f32>, |duration, value| {
                        Some(duration.map_or(value, |current| current.max(value)))
                    })
                    .or_else(|| {
                        clip.pose_cache.as_ref().map(|cache| {
                            cache.frame_count.saturating_sub(1).max(1) as f32 / 30.0
                        })
                    })
                    .unwrap_or(DEFAULT_PREVIEW_SECONDS)
                    .max(f32::EPSILON)
            })
            .unwrap_or(DEFAULT_PREVIEW_SECONDS)
    }

    fn selected_phase(&self) -> f32 {
        (self.current_time / self.selected_duration()).clamp(0.0, 1.0)
    }

    fn selected_asset_frame(&self) -> f32 {
        self.selected_clip()
            .and_then(|clip| clip.pose_cache.as_ref())
            .map(|cache| cache.frame_at_phase(self.selected_phase()))
            .unwrap_or(0.0)
    }

    fn selected_frame_step_seconds(&self) -> f32 {
        let duration = self.selected_duration();
        self.selected_clip()
            .and_then(|clip| clip.pose_cache.as_ref())
            .map(|cache| duration / cache.frame_count.saturating_sub(1).max(1) as f32)
            .unwrap_or_else(|| 1.0 / self.selected_usage_fps())
    }

    fn show_canvas(&mut self, ui: &mut egui::Ui, height: f32) {
        let canvas_size = egui::vec2(ui.available_width().max(1.0), height.max(1.0));
        let (rect, response) = ui.allocate_exact_size(canvas_size, egui::Sense::click_and_drag());
        ui.painter()
            .rect_filled(rect, 0.0, egui::Color32::from_rgb(18, 20, 24));
        self.viewer.lock().update(ui, &response);

        let selected_skin_index = self.selected_clip().and_then(|clip| clip.skin_index);
        let playback_phase = self.selected_phase();
        let sampled_poses = self.selected_clip().and_then(|clip| {
            clip.pose_cache
                .as_ref()
                .and_then(|cache| cache.sample_phase(playback_phase))
        });
        let skin_matrices = self
            .selected_skin()
            .and_then(|skin| skin.parsed.as_ref())
            .and_then(|skin| {
                sampled_poses
                    .as_deref()
                    .and_then(|poses| build_skin_matrices(skin, poses))
                    .or_else(|| bind_pose_skin_matrices(skin))
            })
            .unwrap_or_default();
        let has_components = selected_skin_index
            .and_then(|skin_index| {
                self.skin_renderers
                    .read()
                    .get(skin_index)
                    .map(|entities| !entities.is_empty())
            })
            .unwrap_or(false);
        let center = self
            .selected_skin()
            .map(|skin| skin.center)
            .unwrap_or(Vec3::ZERO);
        let time = self.current_time as f64;
        let render_store = self.render_store.clone();
        let skin_renderers = self.skin_renderers.clone();
        let viewer = self.viewer.clone();
        let callback = egui_glow::CallbackFn::new(move |info, painter| unsafe {
            let Some(skin_index) = selected_skin_index else {
                return;
            };
            let mut viewer = viewer.lock();
            viewer.start_render(painter.gl(), info.viewport.aspect_ratio(), time as f32);
            let render_context = viewer.render_context();
            let store = render_store.read();
            let mut all_skin_renderers = skin_renderers.write();
            let Some(entities) = all_skin_renderers.get_mut(skin_index) else {
                return;
            };
            let position = -center;

            for entity in entities.iter_mut() {
                if skin_vertices(
                    &entity.original_vertices,
                    &mut entity.skinned_vertices,
                    &entity.part_vertex_ranges,
                    &entity.part_skins,
                    &skin_matrices,
                )
                .is_some()
                {
                    entity
                        .renderer
                        .update_vertices(painter.gl(), &entity.skinned_vertices);
                }
                entity.renderer.draw_opaque(
                    painter.gl(),
                    &render_context,
                    position,
                    Quat::IDENTITY,
                    Vec3::ONE,
                    time,
                    &store,
                );
            }
            painter.gl().depth_mask(false);
            for entity in entities.iter() {
                entity.renderer.draw_transparent(
                    painter.gl(),
                    &render_context,
                    position,
                    Quat::IDENTITY,
                    Vec3::ONE,
                    time,
                    &store,
                );
            }
            painter.gl().depth_mask(true);
        });
        ui.painter().add(egui::PaintCallback {
            rect,
            callback: Arc::new(callback),
        });

        let frame_count = self
            .selected_clip()
            .and_then(|clip| clip.pose_cache.as_ref())
            .map(|cache| cache.frame_count)
            .unwrap_or(0);
        let current_frame = if frame_count > 0 {
            self.selected_asset_frame().floor() as usize
        } else {
            0
        };
        ui.painter().text(
            rect.left_top() + egui::vec2(10.0, 8.0),
            egui::Align2::LEFT_TOP,
            if frame_count > 0 {
                format!("Frame {current_frame} / {}", frame_count - 1)
            } else {
                "Bind pose preview".to_string()
            },
            egui::FontId::monospace(13.0),
            egui::Color32::LIGHT_GRAY,
        );

        if !has_components {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "No resolved AnimSkin component geometry",
                egui::FontId::proportional(16.0),
                egui::Color32::GRAY,
            );
        }
    }

    fn show_playback_controls(&mut self, ui: &mut egui::Ui) {
        let duration = self.selected_duration();
        ui.horizontal(|ui| {
            if ui
                .button(RichText::new(if self.is_playing {
                    "Pause"
                } else {
                    "Play"
                }))
                .clicked()
                || ui.input(|input| input.key_pressed(egui::Key::Space))
            {
                if self.current_time >= duration {
                    self.current_time = 0.0;
                }
                self.is_playing = !self.is_playing;
            }
            if ui.button("|<").clicked() {
                self.current_time = 0.0;
                self.is_playing = false;
            }
            if ui.button("< Frame").clicked() {
                let step = self.selected_frame_step_seconds();
                self.current_time = (self.current_time - step).max(0.0);
            }
            if ui.button("Frame >").clicked() {
                let step = self.selected_frame_step_seconds();
                self.current_time = (self.current_time + step).min(duration);
            }
            ui.add(
                egui::Slider::new(&mut self.current_time, 0.0..=duration)
                    .show_value(false)
                    .text("Timeline"),
            );
            ui.monospace(format!(
                "{:.3} / {:.3} s  asset frame {:.2}",
                self.current_time,
                duration,
                self.selected_asset_frame()
            ));
        });
    }

    fn selected_usage_fps(&self) -> f32 {
        self.selected_clip()
            .and_then(|clip| clip.usages.first())
            .map(|usage| usage.script_fps)
            .filter(|fps| fps.is_finite() && *fps > f32::EPSILON)
            .unwrap_or(30.0)
    }

    fn show_selected_details(&self, ui: &mut egui::Ui) {
        let Some(clip) = self.selected_clip() else {
            ui.label("No animation selected");
            return;
        };
        let name = semantic_animation_label(&self.catalog, clip, &self.hashcodes);

        ui.heading(name);
        ui.horizontal_wrapped(|ui| {
            ui.monospace(format!("hash=0x{:08X}", clip.hashcode));
            ui.separator();
            ui.monospace(format!("index={}", clip.index));
            ui.separator();
            ui.monospace(format!("record=0x{:08X}", clip.file_offset));
            ui.separator();
            ui.monospace(format!("motion=0x{:08X}", clip.motiondata_info_addr));
            ui.separator();
            ui.monospace(format!("size={}", clip.data_size));
            ui.separator();
            ui.monospace(format!("skin_num=0x{:08X}", clip.skin_num));
        });

        if let Some(cache) = &clip.pose_cache {
            ui.colored_label(
                egui::Color32::LIGHT_GREEN,
                format!(
                    "Native pose cache active: {} frames, {} bones. CPU skinning and frame interpolation are enabled.",
                    cache.frame_count, cache.bone_count
                ),
            );
            ui.monospace(format!(
                "cache={} motion_fnv=0x{:016X}",
                cache.source_path.display(),
                cache.motion_checksum
            ));
        } else if let Some(error) = &clip.pose_cache_error {
            ui.colored_label(
                egui::Color32::LIGHT_RED,
                format!("Native pose cache rejected: {error}"),
            );
        } else {
            ui.colored_label(
                egui::Color32::YELLOW,
                "Native pose cache is not available for this clip. Exact Animation/AnimSkin binding and bind-pose CPU skinning remain active.",
            );
        }

        egui::CollapsingHeader::new("Motion payload")
            .default_open(true)
            .show(ui, |ui| {
                ui.label(format!(
                    "Captured {} of {} bytes; FNV-1a 0x{:016X}{}",
                    clip.motion.bytes.len(),
                    clip.motion.expected_size,
                    clip.motion.checksum,
                    if clip.motion.truncated {
                        " (truncated)"
                    } else {
                        ""
                    }
                ));
                if let Some(error) = &clip.motion.read_error {
                    ui.colored_label(egui::Color32::LIGHT_RED, format!("Read error: {error}"));
                }
                let preview = clip
                    .motion
                    .bytes
                    .iter()
                    .take(96)
                    .map(|byte| format!("{byte:02X}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                ui.add(
                    egui::Label::new(
                        RichText::new(if preview.is_empty() {
                            "<empty>".to_string()
                        } else {
                            preview
                        })
                        .monospace(),
                    )
                    .wrap(),
                );
            });

        if let Some(skin) = self.selected_skin() {
            egui::CollapsingHeader::new("AnimSkin and bind hierarchy")
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.monospace(format!("skin=0x{:08X}", skin.hashcode));
                        ui.separator();
                        ui.monospace(format!("index={}", skin.index));
                        ui.separator();
                        ui.monospace(format!("base_skin_num=0x{:08X}", skin.base_skin_num));
                        ui.separator();
                        ui.monospace(format!("mip_ref=0x{:08X}", skin.mip_ref));
                    });
                    if let Some(identity_error) = skin.bind_pose_identity_error {
                        ui.monospace(format!(
                            "bind-pose identity max error={identity_error:.3e}"
                        ));
                    }
                    if let Some(error) = &skin.parse_error {
                        ui.colored_label(egui::Color32::LIGHT_RED, error);
                    }
                    if let Some(parsed) = &skin.parsed {
                        ui.label(format!(
                            "Object type 0x{:08X}; bones {}; primary components {}; secondary components {}; absolute bind positions {}; relative bind positions {}; hierarchy records {}",
                            parsed.object_type,
                            parsed.bone_count,
                            parsed.entities.len(),
                            parsed.more_entities.len(),
                            parsed.absolute_bind_positions.len(),
                            parsed.relative_bind_positions.len(),
                            parsed.hier_data.len(),
                        ));
                        egui::ScrollArea::vertical()
                            .id_salt("animation_bone_hierarchy")
                            .max_height(150.0)
                            .show(ui, |ui| {
                                egui::Grid::new("animation_bone_hierarchy_grid")
                                    .striped(true)
                                    .show(ui, |ui| {
                                        ui.strong("Bone");
                                        ui.strong("Link");
                                        ui.strong("Max");
                                        ui.strong("Flags");
                                        ui.strong("Absolute bind");
                                        ui.strong("Relative bind");
                                        ui.end_row();
                                        for (bone_index, hierarchy) in
                                            parsed.hier_data.iter().enumerate()
                                        {
                                            ui.monospace(bone_index.to_string());
                                            ui.monospace(hierarchy.link_index.to_string());
                                            ui.monospace(hierarchy.max_index.to_string());
                                            ui.monospace(format!("0x{:04X}", hierarchy.flags));
                                            ui.monospace(
                                                parsed
                                                    .absolute_bind_positions
                                                    .get(bone_index)
                                                    .map(|value| format!("{value:?}"))
                                                    .unwrap_or_default(),
                                            );
                                            ui.monospace(
                                                parsed
                                                    .relative_bind_positions
                                                    .get(bone_index)
                                                    .map(|value| format!("{value:?}"))
                                                    .unwrap_or_default(),
                                            );
                                            ui.end_row();
                                        }
                                    });
                            });
                    }
                });

            egui::CollapsingHeader::new(format!("Component geometry ({})", skin.components.len()))
                .default_open(false)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("animation_component_table")
                        .max_height(180.0)
                        .show(ui, |ui| {
                            egui::Grid::new("animation_component_grid")
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.strong("Group");
                                    ui.strong("#");
                                    ui.strong("Entity");
                                    ui.strong("Section");
                                    ui.strong("Parts");
                                    ui.strong("Morph");
                                    ui.end_row();
                                    for component in &skin.components {
                                        ui.label(component.group);
                                        ui.monospace(component.component_index.to_string());
                                        ui.monospace(
                                            component
                                                .entity_hashcode
                                                .map(|hashcode| {
                                                    format!(
                                                        "{} [0x{hashcode:08X}]",
                                                        format_hashcode(&self.hashcodes, hashcode)
                                                    )
                                                })
                                                .unwrap_or_else(|| {
                                                    format!(
                                                        "entity_index={} raw=0x{:08X}",
                                                        component.entity_index,
                                                        component.raw_entity_index
                                                    )
                                                }),
                                        );
                                        ui.monospace(component.section_index.to_string());
                                        ui.monospace(component.parts_count.to_string());
                                        ui.monospace(component.morph_index.to_string());
                                        ui.end_row();
                                    }
                                });
                        });
                });
        } else if clip.skin_num == u32::MAX {
            ui.colored_label(
                egui::Color32::GRAY,
                "Animation has the explicit no-skin sentinel.",
            );
        } else {
            ui.colored_label(
                egui::Color32::LIGHT_RED,
                format!("No AnimSkin matches base_skin_num 0x{:08X}", clip.skin_num),
            );
        }

        egui::CollapsingHeader::new(format!("AnimScript usages ({})", clip.usages.len()))
            .default_open(false)
            .show(ui, |ui| {
                if clip.usages.is_empty() {
                    ui.label("No command in this EDB references this animation.");
                }
                for usage in &clip.usages {
                    ui.monospace(format!(
                        "script 0x{:08X}, command {}, start {}, length {}, {:.3} fps, skin {}",
                        usage.script_hashcode,
                        usage.command_index,
                        usage.start_frame,
                        usage.length_frames,
                        usage.script_fps,
                        if usage.skin_hashcode == u32::MAX {
                            "implicit Animation binding".to_string()
                        } else if usage.skin_hashcode.is_local() {
                            format!("AnimSkin #{}", usage.skin_hashcode.index())
                        } else {
                            format!("0x{:08X}@0x{:08X}", usage.skin_hashcode, usage.skin_file)
                        }
                    ));
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(index: usize, hashcode: Hashcode) -> AnimationClipRecord {
        AnimationClipRecord {
            index,
            hashcode,
            file_offset: 0,
            motiondata_info_addr: 0,
            data_size: 0,
            skin_num: u32::MAX,
            skin_index: None,
            motion: AnimationMotionData {
                expected_size: 0,
                bytes: Vec::new(),
                truncated: false,
                read_error: None,
                checksum: fnv1a64(&[]),
            },
            pose_cache: None,
            pose_cache_error: None,
            usages: Vec::new(),
            preview_duration: DEFAULT_PREVIEW_SECONDS,
        }
    }

    #[test]
    fn resolves_local_animation_by_serialized_index() {
        let clips = vec![clip(0, 0x0300_0010), clip(1, 0x0300_0020)];
        assert_eq!(resolve_clip_index(&clips, 0x8300_0001), Some(1));
    }

    #[test]
    fn resolves_global_animation_by_hashcode() {
        let clips = vec![clip(0, 0x0300_0010), clip(1, 0x0300_0020)];
        assert_eq!(resolve_clip_index(&clips, 0x0300_0020), Some(1));
    }

    #[test]
    fn animation_skin_sentinel_uses_the_clips_serialized_binding() {
        let mut bound_clip = clip(0, 0x0300_0010);
        bound_clip.skin_index = Some(0);
        let runtime = AnimationRuntime {
            catalog: AnimationCatalog {
                clips: vec![bound_clip],
                skins: vec![AnimationSkinRecord {
                    index: 0,
                    hashcode: 0x0D00_0001,
                    base_skin_num: 0,
                    mip_ref: 0,
                    parsed: None,
                    parse_error: None,
                    components: Vec::new(),
                    center: Vec3::ZERO,
                    maximum_extent: 0.0,
                    bind_pose_identity_error: None,
                }],
            },
            skin_renderers: Arc::new(RwLock::new(vec![Vec::new()])),
        };
        let clip = &runtime.catalog.clips[0];
        assert_eq!(runtime.resolve_clip_skin_index(clip, u32::MAX), Some(0));
        assert_eq!(runtime.resolve_clip_skin_index(clip, 0x8D00_0000), Some(0));
    }

    #[test]
    fn motion_checksum_is_stable() {
        assert_eq!(fnv1a64(b"Robots"), 0x14BA_513B_C889_6C24);
    }

    #[test]
    fn pose_cache_parser_interpolates_position_and_shortest_quaternion_path() {
        let edb_uid: u32 = 0x0100_0086;
        let animation_index = 7usize;
        let animation_hashcode: u32 = 0x8300_0007;
        let animskin_hashcode: u32 = 0x0D00_0001;
        let checksum: u64 = 0x1122_3344_5566_7788;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(POSE_CACHE_MAGIC);
        bytes.extend_from_slice(&edb_uid.to_le_bytes());
        bytes.extend_from_slice(&(animation_index as u32).to_le_bytes());
        bytes.extend_from_slice(&animation_hashcode.to_le_bytes());
        bytes.extend_from_slice(&animskin_hashcode.to_le_bytes());
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&checksum.to_le_bytes());
        for values in [
            [0.0f32, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0],
            [2.0f32, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0],
        ] {
            for value in values {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }

        let cache = parse_pose_cache(
            Path::new("fixture.rapc"),
            &bytes,
            edb_uid,
            animation_index,
            animation_hashcode,
            animskin_hashcode,
            checksum,
        )
        .expect("valid synthetic pose cache");
        let sample = cache
            .sample_frame(0.5)
            .expect("interpolated pose sample");
        assert_eq!(sample.len(), 1);
        assert!((sample[0].position.x - 1.0).abs() < 1.0e-6);
        assert!((sample[0].rotation.length() - 1.0).abs() < 1.0e-6);
        assert!((sample[0].rotation.z.abs() - std::f32::consts::FRAC_1_SQRT_2).abs() < 1.0e-5);
        assert!((sample[0].rotation.w.abs() - std::f32::consts::FRAC_1_SQRT_2).abs() < 1.0e-5);

        let last = cache.sample_phase(1.0).expect("last pose sample");
        assert!((last[0].position.x - 2.0).abs() < 1.0e-6);
        let after_end = cache.sample_phase(2.0).expect("clamped last pose sample");
        assert!((after_end[0].position.x - 2.0).abs() < 1.0e-6);
    }

    #[test]
    fn real_animation_catalog_when_fixture_is_requested() {
        let Ok(path) = std::env::var("ROBOTS_ANIMATION_FIXTURE") else {
            return;
        };
        let platform = eurochef_edb::versions::Platform::from_path(&path)
            .expect("fixture platform should be detectable");
        let file = std::fs::File::open(&path).expect("open animation fixture");
        let reader = std::io::BufReader::new(file);
        let mut edb = EdbFile::new(Box::new(reader), platform).expect("parse animation fixture");
        let catalog = read_from_file(&mut edb).expect("read animation catalog");

        assert!(!catalog.clips.is_empty(), "fixture contains no animations");
        assert!(
            catalog.skins.iter().all(|skin| skin.parse_error.is_none()),
            "fixture contains an AnimSkin parse failure"
        );
        assert!(
            catalog
                .clips
                .iter()
                .all(|clip| clip.motion.read_error.is_none()),
            "fixture contains an unreadable motion payload"
        );
        if std::env::var_os("ROBOTS_ANIMATION_POSE_CACHE").is_some() {
            let missing_caches = catalog
                .clips
                .iter()
                .filter(|clip| clip.skin_index.is_some() && clip.pose_cache.is_none())
                .map(|clip| clip.index)
                .collect::<Vec<_>>();
            assert!(
                missing_caches.is_empty(),
                "fixture UID=0x{:08X} did not load every bound native pose cache; missing={missing_caches:?}; roots={:?}; errors={:?}",
                edb.header.hashcode,
                pose_cache_roots(),
                catalog
                    .clips
                    .iter()
                    .filter_map(|clip| clip.pose_cache_error.as_deref())
                    .collect::<Vec<_>>()
            );
            assert!(
                catalog
                    .clips
                    .iter()
                    .all(|clip| clip.pose_cache_error.is_none()),
                "fixture rejected a native pose cache"
            );
        }
    }
    #[test]
    fn real_animation_manifest_when_requested() {
        let Ok(manifest_path) = std::env::var("ROBOTS_ANIMATION_MANIFEST") else {
            return;
        };
        let manifest = std::fs::read_to_string(&manifest_path).expect("read animation manifest");
        let mut files = 0usize;
        let mut clips = 0usize;
        let mut skins = 0usize;
        let mut motion_failures = Vec::new();
        let mut skin_failures = Vec::new();
        let mut bind_pose_failures = Vec::new();

        for line in manifest.lines().skip(1) {
            let Some((_, path)) = line.split_once('\t') else {
                continue;
            };
            let path = path.trim();
            if path.is_empty() {
                continue;
            }
            let platform = eurochef_edb::versions::Platform::from_path(path)
                .expect("manifest EDB platform should be detectable");
            let file = std::fs::File::open(path).expect("open manifest EDB");
            let reader = std::io::BufReader::new(file);
            let mut edb = EdbFile::new(Box::new(reader), platform).expect("parse manifest EDB");
            let catalog = read_from_file(&mut edb).expect("read manifest animation catalog");
            files += 1;
            clips += catalog.clips.len();
            skins += catalog.skins.len();

            for clip in catalog.clips {
                if let Some(error) = clip.motion.read_error {
                    motion_failures.push(format!("{path}: animation {}: {error}", clip.index));
                }
            }
            for skin in catalog.skins {
                if let Some(error) = skin.parse_error {
                    skin_failures.push(format!("{path}: AnimSkin {}: {error}", skin.index));
                }
                if let Some(parsed) = &skin.parsed {
                    match bind_pose_skin_matrices(parsed) {
                        Some(matrices) => {
                            let error = matrices
                                .into_iter()
                                .map(|matrix| {
                                    matrix_max_abs_difference(matrix, glam::Mat4::IDENTITY)
                                })
                                .fold(0.0, f32::max);
                            if error > 1.0e-4 {
                                bind_pose_failures.push(format!(
                                    "{path}: AnimSkin {}: max identity error {error}",
                                    skin.index
                                ));
                            }
                        }
                        None => bind_pose_failures.push(format!(
                            "{path}: AnimSkin {}: invalid bind hierarchy",
                            skin.index
                        )),
                    }
                }
            }
        }

        assert_eq!(files, 179);
        assert_eq!(clips, 1744);
        assert_eq!(skins, 234);
        assert!(motion_failures.is_empty(), "{motion_failures:#?}");
        assert!(skin_failures.is_empty(), "{skin_failures:#?}");
        assert!(bind_pose_failures.is_empty(), "{bind_pose_failures:#?}");
    }
}
