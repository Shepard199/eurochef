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
    anim::EXGeoBaseAnimSkin,
    binrw::BinReaderExt,
    common::EXGeoAnimHeader,
    edb::EdbFile,
    entity::{read_robots_v248_morph_shape_deltas, EXGeoEntity},
    header::EXGeoHeader,
    versions::Platform,
    Hashcode, HashcodeUtils,
};
use eurochef_shared::{
    entities::UXVertex,
    maps::{format_hashcode_with_id, format_typed_hashcode_with_id},
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
        script::resolve_animation_skin_target,
        viewer::{BaseViewer, RenderContext},
        RenderStore,
    },
};

mod skinning;

use skinning::{
    bind_pose_global_bone_matrices, bind_pose_skin_matrices, build_global_bone_matrices,
    build_native_bone_remap, build_skin_matrices, matrix_max_abs_difference, skin_vertices,
    skin_vertices_with_morph, transform_vertices_rigid, AnimationBonePose, NativeBoneRemap,
};

const MAX_CAPTURED_MOTION_BYTES: usize = 16 * 1024 * 1024;
const DEFAULT_PREVIEW_SECONDS: f32 = 1.0;
const POSE_CACHE_MAGIC: &[u8; 8] = b"RAPCV003";
const POSE_CACHE_HEADER_SIZE: usize = 44;
const POSE_CACHE_VALUES_PER_BONE: usize = 7;
const POSE_CACHE_BYTES_PER_BONE: usize = POSE_CACHE_VALUES_PER_BONE * size_of::<f32>();
const MAX_POSE_CACHE_BYTES: usize = 512 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationPoseSource {
    Rapc,
    EmbeddedEdb,
}

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
    pub source: AnimationPoseSource,
    pub source_path: PathBuf,
    pub frame_count: usize,
    pub bone_count: usize,
    pub scalar_count: usize,
    pub motion_checksum: u64,
    poses: Vec<AnimationBonePose>,
    scalars: Vec<f32>,
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

    pub fn sample_scalar_phase(&self, phase: f32) -> Option<Vec<f32>> {
        self.sample_scalar_frame(self.frame_at_phase(phase))
    }

    fn sample_scalar_frame(&self, raw_frame: f32) -> Option<Vec<f32>> {
        if self.scalar_count == 0 {
            return Some(Vec::new());
        }
        if self.frame_count == 0
            || self.scalars.len() != self.frame_count.checked_mul(self.scalar_count)?
        {
            return None;
        }
        let clamped = raw_frame
            .max(0.0)
            .min((self.frame_count.saturating_sub(1)) as f32);
        let current_frame = clamped.floor() as usize;
        let next_frame = (current_frame + 1).min(self.frame_count - 1);
        let fraction = clamped.fract();
        let current_start = current_frame.checked_mul(self.scalar_count)?;
        let next_start = next_frame.checked_mul(self.scalar_count)?;
        let current = self
            .scalars
            .get(current_start..current_start.checked_add(self.scalar_count)?)?;
        let next = self
            .scalars
            .get(next_start..next_start.checked_add(self.scalar_count)?)?;
        Some(
            current
                .iter()
                .zip(next)
                .map(|(current, next)| current + (next - current) * fraction)
                .collect(),
        )
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
    /// Robots v248 additive position deltas, one complete vertex array per native morph scalar.
    pub morph_shapes: Vec<Vec<Vec3>>,
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
    /// First native scalar channel used by this component's additive morph group.
    pub morph_scalar_base: Option<usize>,
    pub part_skins: Vec<AnimationPartSkin>,
}

#[derive(Debug, Clone)]
pub struct AnimationBoneAttachmentRecord {
    pub attachment_index: usize,
    pub bone_selector: usize,
    pub bone_hashcode: Option<Hashcode>,
    pub raw_entity_index: u32,
    pub entity_index: usize,
    pub entity_hashcode: Option<Hashcode>,
}

#[derive(Debug, Clone)]
pub struct AnimationSkinRecord {
    pub index: usize,
    pub hashcode: Hashcode,
    pub base_skin_num: u32,
    pub mip_ref: u32,
    /// Sparse Robots v248 serialized bone slot -> exact HT_AnimBone hash mapping.
    pub bone_hashcodes: Vec<Option<Hashcode>>,
    pub bone_attachments: Vec<AnimationBoneAttachmentRecord>,
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

impl AnimationCatalog {
    pub fn bound_skin_hashcode(&self, animation_hashcode: Hashcode) -> Option<Hashcode> {
        let clip_index = resolve_clip_index(&self.clips, animation_hashcode)?;
        let skin_index = self.clips.get(clip_index)?.skin_index?;
        self.skins.get(skin_index).map(|skin| skin.hashcode)
    }
}

/// Samples one native bound AnimBone in animation-local space at an exact raw
/// asset frame. This deliberately bypasses UI phase normalization: gameplay
/// callers such as the Sweeper Ratchet missile event require the native 16.5
/// frame sample rather than `phase * (frame_count - 1)`.
pub(crate) fn sample_bound_animation_bone_position(
    catalog: &AnimationCatalog,
    animation_hashcode: Hashcode,
    bone_hashcode: Hashcode,
    raw_frame: f32,
) -> Option<Vec3> {
    let clip_index = resolve_clip_index(&catalog.clips, animation_hashcode)?;
    let clip = catalog.clips.get(clip_index)?;
    let cache = clip.pose_cache.as_ref()?;
    let skin_index = clip.skin_index?;
    let skin_record = catalog.skins.get(skin_index)?;
    let skin = skin_record.parsed.as_ref()?;
    if cache.bone_count != skin.bone_count as usize {
        return None;
    }
    let bone_selector = skin_record
        .bone_hashcodes
        .iter()
        .position(|hashcode| *hashcode == Some(bone_hashcode))?;
    let poses = cache.sample_frame(raw_frame)?;
    let globals = build_global_bone_matrices(skin, &poses)?;
    globals
        .get(bone_selector)
        .map(|matrix| matrix.transform_point3(Vec3::ZERO))
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
        return Err("pose cache magic is not RAPCV003".to_string());
    }

    let edb_uid = read_u32_le(bytes, 0x08)?;
    let animation_index = read_u32_le(bytes, 0x0C)? as usize;
    let animation_hashcode = read_u32_le(bytes, 0x10)?;
    let animskin_hashcode = read_u32_le(bytes, 0x14)?;
    let frame_count = read_u32_le(bytes, 0x18)? as usize;
    let bone_count = read_u32_le(bytes, 0x1C)? as usize;
    let scalar_count = read_u32_le(bytes, 0x20)? as usize;
    let motion_checksum = read_u64_le(bytes, 0x24)?;

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
    let pose_frame_size = bone_count
        .checked_mul(POSE_CACHE_BYTES_PER_BONE)
        .ok_or_else(|| "pose cache pose frame size overflows".to_string())?;
    let scalar_frame_size = scalar_count
        .checked_mul(size_of::<f32>())
        .ok_or_else(|| "pose cache scalar frame size overflows".to_string())?;
    let frame_stride = pose_frame_size
        .checked_add(scalar_frame_size)
        .ok_or_else(|| "pose cache frame stride overflows".to_string())?;
    let payload_size = frame_count
        .checked_mul(frame_stride)
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
    let mut scalars = Vec::with_capacity(frame_count.saturating_mul(scalar_count));
    for frame_index in 0..frame_count {
        let frame_offset = POSE_CACHE_HEADER_SIZE + frame_index * frame_stride;
        for bone_index in 0..bone_count {
            let pose_index = frame_index * bone_count + bone_index;
            let offset = frame_offset + bone_index * POSE_CACHE_BYTES_PER_BONE;
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
        let scalar_offset = frame_offset + pose_frame_size;
        for scalar_index in 0..scalar_count {
            let value = read_f32_le(bytes, scalar_offset + scalar_index * size_of::<f32>())?;
            if !value.is_finite() {
                return Err(format!(
                    "non-finite morph scalar at frame {frame_index} index {scalar_index}"
                ));
            }
            scalars.push(value);
        }
    }

    Ok(AnimationPoseCache {
        source: AnimationPoseSource::Rapc,
        source_path: source_path.to_path_buf(),
        frame_count,
        bone_count,
        scalar_count,
        motion_checksum,
        poses,
        scalars,
    })
}

fn build_embedded_edb_pose_cache(
    edb: &mut EdbFile,
    edb_uid: Hashcode,
    animation_index: usize,
    animation: &EXGeoAnimHeader,
    skin: &EXGeoBaseAnimSkin,
    motion: &AnimationMotionData,
) -> Result<AnimationPoseCache, String> {
    if edb.platform != Platform::Pc || edb.header.version != 248 {
        return Err("embedded native pose decoder is only proven for Robots PC v248".to_string());
    }
    if motion.truncated || motion.bytes.len() != motion.expected_size {
        return Err(format!(
            "compressed motion stream is incomplete: captured={} expected={}",
            motion.bytes.len(),
            motion.expected_size
        ));
    }
    if let Some(error) = motion.read_error.as_deref() {
        return Err(format!("compressed motion stream read failed: {error}"));
    }

    let endian = edb.endian;
    let serialized = animation
        .read_robots_v248_exgeoanim(edb, endian)
        .map_err(|error| format!("read serialized EXGeoAnim: {error}"))?;
    let block_table = animation
        .read_robots_v248_motion_block_table(edb, endian)
        .map_err(|error| format!("read native motion block table: {error}"))?;
    let root_correction = animation
        .read_robots_v248_root_correction_transform(edb, endian)
        .map_err(|error| format!("read native root correction transform: {error}"))?;
    let frame_count = usize::from(serialized.frame_count);
    let bone_count = usize::from(serialized.bone_count);
    if frame_count == 0 || bone_count == 0 {
        return Err(format!(
            "invalid embedded pose dimensions frames={frame_count} bones={bone_count}"
        ));
    }
    if bone_count != skin.bone_count as usize {
        return Err(format!(
            "EXGeoAnim/AnimSkin bone mismatch {bone_count} != {}",
            skin.bone_count
        ));
    }

    let pose_count = frame_count
        .checked_mul(bone_count)
        .ok_or_else(|| "embedded pose dimensions overflow".to_string())?;
    let mut poses = Vec::with_capacity(pose_count);
    for frame_index in 0..frame_count {
        let raw = match block_table.as_ref() {
            Some(block_table) => serialized.decode_skeletal_frame_with_block_table(
                &motion.bytes,
                frame_index as u16,
                block_table,
            ),
            None => serialized.decode_skeletal_frame(&motion.bytes, frame_index as u16),
        }
        .map_err(|error| format!("decode frame {frame_index}: {error}"))?;
        let assembled = serialized
            .assemble_same_skin_pose_with_root_correction(
                &raw,
                &skin.relative_bind_positions,
                root_correction.as_ref(),
            )
            .map_err(|error| format!("assemble frame {frame_index}: {error}"))?;
        for (bone_index, bone) in assembled.bones.into_iter().enumerate() {
            let position = Vec3::new(bone.position[0], bone.position[1], bone.position[2]);
            let rotation = Quat::from_xyzw(
                bone.rotation[0],
                bone.rotation[1],
                bone.rotation[2],
                bone.rotation[3],
            );
            if !position.is_finite() || !rotation.is_finite() {
                return Err(format!(
                    "non-finite embedded pose at frame {frame_index} bone {bone_index}"
                ));
            }
            let length = rotation.length();
            if (length - 1.0).abs() > 2.0e-3 {
                return Err(format!(
                    "non-unit embedded quaternion at frame {frame_index} bone {bone_index}: {length}"
                ));
            }
            poses.push(AnimationBonePose {
                position,
                rotation: rotation.normalize(),
            });
        }
    }

    let source = AnimationPoseSource::EmbeddedEdb;
    let source_path = PathBuf::from(format!(
        "EDB 0x{edb_uid:08X} animation {animation_index} @ 0x{:08X}",
        animation.common.address
    ));
    let scalar_count = usize::from(serialized.scalar_channel_count);
    let mut scalars = Vec::with_capacity(frame_count.saturating_mul(scalar_count));
    if scalar_count != 0 {
        let scalar_stream = animation
            .read_robots_v248_scalar_stream(edb, endian)
            .map_err(|error| format!("read native scalar stream: {error}"))?;
        for frame_index in 0..frame_count {
            let frame_scalars = serialized
                .decode_scalar_frame(&scalar_stream, frame_index as u16)
                .map_err(|error| format!("decode scalar frame {frame_index}: {error}"))?;
            if frame_scalars.len() != scalar_count {
                return Err(format!(
                    "scalar frame {frame_index} has {} channels, expected {scalar_count}",
                    frame_scalars.len()
                ));
            }
            for (scalar_index, value) in frame_scalars.into_iter().enumerate() {
                if !value.is_finite() {
                    return Err(format!(
                        "non-finite embedded morph scalar at frame {frame_index} index {scalar_index}"
                    ));
                }
                scalars.push(value);
            }
        }
    }

    Ok(AnimationPoseCache {
        source,
        source_path,
        frame_count,
        bone_count,
        scalar_count,
        motion_checksum: motion.checksum,
        poses,
        scalars,
    })
}

fn load_pose_cache(
    edb_uid: Hashcode,
    animation_index: usize,
    animation_hashcode: Hashcode,
    animskin_hashcode: Hashcode,
    motion_checksum: u64,
) -> (Option<AnimationPoseCache>, Option<String>) {
    let folder = PathBuf::from(format!("{edb_uid:08X}"));
    let relative_candidates = [
        folder.join(format!(
            "{animation_index:04}_[0x{animskin_hashcode:08X}].rapc"
        )),
        folder.join(format!("{animation_index:04}.rapc")),
    ];
    let mut errors = Vec::new();
    for root in pose_cache_roots() {
        for relative in &relative_candidates {
            let path = root.join(relative);
            if !path.is_file() {
                continue;
            }
            match fs::read(&path) {
                Ok(bytes) => match parse_pose_cache(
                    &path,
                    &bytes,
                    edb_uid,
                    animation_index,
                    animation_hashcode,
                    animskin_hashcode,
                    motion_checksum,
                ) {
                    Ok(cache) => return (Some(cache), None),
                    Err(error) => {
                        errors.push(format!("{}: {error}", path.display()));
                        continue;
                    }
                },
                Err(error) => errors.push(format!("could not read {}: {error}", path.display())),
            }
        }
    }
    if errors.is_empty() {
        (None, None)
    } else {
        (None, Some(errors.join("; ")))
    }
}

pub fn read_from_file(edb: &mut EdbFile) -> anyhow::Result<AnimationCatalog> {
    let header = edb.header.clone();
    let saved_position = edb.stream_position()?;
    let mut skins = Vec::with_capacity(header.animskin_list.len());
    let mut entity_mesh_parts: HashMap<usize, Vec<AnimationEntityMeshPartInfo>> = HashMap::new();

    for (index, skin_header) in header.animskin_list.iter().enumerate() {
        edb.seek(SeekFrom::Start(skin_header.common.address as u64))?;
        let parsed = edb.read_type_args::<EXGeoBaseAnimSkin>(edb.endian, (header.version,));
        let (parsed, parse_error) = match parsed {
            Ok(skin) => (Some(skin), None),
            Err(error) => (None, Some(error.to_string())),
        };

        let bone_hashcodes = if let Some(skin) = parsed.as_ref() {
            if header.version == 248 && edb.platform == Platform::Pc {
                let endian = edb.endian;
                skin.read_robots_v248_bone_hashcodes(edb, endian)
                    .context("read Robots v248 AnimBone selector table")?
            } else {
                vec![None; skin.bone_count as usize]
            }
        } else {
            Vec::new()
        };

        let bone_attachments = parsed
            .as_ref()
            .and_then(|skin| skin.bone_attachments.as_ref())
            .map(|attachments| {
                attachments
                    .iter()
                    .enumerate()
                    .map(|(attachment_index, attachment)| {
                        let bone_selector = attachment.bone_selector as usize;
                        let entity_index = attachment.entity_list_index();
                        AnimationBoneAttachmentRecord {
                            attachment_index,
                            bone_selector,
                            bone_hashcode: bone_hashcodes
                                .get(bone_selector)
                                .and_then(|hashcode| *hashcode),
                            raw_entity_index: attachment.entity_index,
                            entity_index,
                            entity_hashcode: header
                                .entity_list
                                .data()
                                .get(entity_index)
                                .map(|entity| entity.common.hashcode),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        let components = if let Some(skin) = parsed.as_ref() {
            collect_components(edb, &header, skin, &mut entity_mesh_parts)?
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
            bone_hashcodes,
            bone_attachments,
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
        let (pose_cache, pose_cache_error) = if let Some(skin_index) = skin_index {
            let skin = &skins[skin_index];
            let (rapc_cache, rapc_error) = load_pose_cache(
                header.hashcode,
                index,
                animation.common.hashcode,
                skin.hashcode,
                motion.checksum,
            );
            if header.version == 248 && edb.platform == Platform::Pc {
                if let Some(parsed_skin) = skin.parsed.as_ref() {
                    match build_embedded_edb_pose_cache(
                        edb,
                        header.hashcode,
                        index,
                        animation,
                        parsed_skin,
                        &motion,
                    ) {
                        Ok(cache) => (Some(cache), None),
                        Err(embedded_error) => {
                            let error = rapc_error.map_or_else(
                                || format!("embedded EDB pose decode failed: {embedded_error}"),
                                |rapc_error| {
                                    format!(
                                        "embedded EDB pose decode failed: {embedded_error}; RAPCV003: {rapc_error}"
                                    )
                                },
                            );
                            (rapc_cache, Some(error))
                        }
                    }
                } else {
                    (rapc_cache, rapc_error)
                }
            } else {
                (rapc_cache, rapc_error)
            }
        } else {
            (None, None)
        };

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

#[derive(Debug, Clone, Copy)]
struct AnimationEntityMeshPartInfo {
    object_address: u64,
    vertex_count: usize,
}

fn collect_components(
    edb: &mut EdbFile,
    header: &EXGeoHeader,
    skin: &EXGeoBaseAnimSkin,
    mesh_part_cache: &mut HashMap<usize, Vec<AnimationEntityMeshPartInfo>>,
) -> anyhow::Result<Vec<AnimationComponent>> {
    let mut components = Vec::new();
    let endian = edb.endian;
    for (group, entries) in [
        ("group A (+0x68)", skin.entities.data().as_slice()),
        (
            "group B (+0x70, morph-bearing)",
            skin.more_entities.data().as_slice(),
        ),
    ] {
        for (component_index, component) in entries.iter().enumerate() {
            let entity_index = component.entity_list_index();
            let entity_hashcode = header
                .entity_list
                .data()
                .get(entity_index)
                .map(|entity| entity.common.hashcode);
            let mesh_parts = if let Some(parts) = mesh_part_cache.get(&entity_index) {
                parts.clone()
            } else {
                let parts = read_entity_mesh_parts(edb, header, entity_index)?;
                mesh_part_cache.insert(entity_index, parts.clone());
                parts
            };
            let morph_group = if header.version == 248
                && edb.platform == Platform::Pc
                && component.morph_index >= 0
            {
                let groups = skin
                    .robots_scalar_groups
                    .as_ref()
                    .context("Robots morph component has no scalar-group table")?;
                let morph_group = groups
                    .data()
                    .get(component.morph_index as usize)
                    .context("Robots morph index outside scalar-group table")?;
                anyhow::ensure!(
                    morph_group.mode == 0,
                    "unsupported shipped Robots morph mode {} at component {}:{}",
                    morph_group.mode,
                    group,
                    component_index
                );
                anyhow::ensure!(
                    usize::from(morph_group.scalar_base) + usize::from(morph_group.scalar_count)
                        <= skin.robots_scalar_value_count as usize,
                    "Robots morph scalar range outside AnimSkin buffer"
                );
                Some(morph_group)
            } else {
                None
            };
            let morph_scalar_base = morph_group.map(|group| usize::from(group.scalar_base));

            anyhow::ensure!(
                component.skin_data.len() == mesh_parts.len(),
                "AnimSkin component {}:{} has {} weight payloads but Entity {} has {} mesh parts",
                group,
                component_index,
                component.skin_data.len(),
                entity_index,
                mesh_parts.len()
            );

            let mut part_skins = Vec::with_capacity(mesh_parts.len());
            for (part_index, (payload, mesh_part)) in component
                .skin_data
                .iter()
                .zip(mesh_parts.iter().copied())
                .enumerate()
            {
                let vertex_count = mesh_part.vertex_count;
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
                let morph_shapes = if let Some(morph_group) = morph_group {
                    read_robots_v248_morph_shape_deltas(
                        edb,
                        endian,
                        mesh_part.object_address,
                        vertex_count,
                        usize::from(morph_group.scalar_count),
                    )?
                    .into_iter()
                    .map(|shape| shape.into_iter().map(Vec3::from_array).collect::<Vec<_>>())
                    .collect()
                } else {
                    Vec::new()
                };
                part_skins.push(AnimationPartSkin {
                    part_index,
                    vertex_count,
                    influences,
                    morph_shapes,
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
                morph_scalar_base,
                part_skins,
            });
        }
    }
    Ok(components)
}

fn read_entity_mesh_parts(
    edb: &mut EdbFile,
    header: &EXGeoHeader,
    entity_index: usize,
) -> anyhow::Result<Vec<AnimationEntityMeshPartInfo>> {
    let entity_header = header
        .entity_list
        .data()
        .get(entity_index)
        .context("AnimSkin Entity index outside Entity list")?;
    let saved_position = edb.stream_position()?;
    let object_address = entity_header.common.address as u64;
    edb.seek(SeekFrom::Start(object_address))?;
    let entity: EXGeoEntity = edb.read_type_args(edb.endian, (header.version, edb.platform))?;
    edb.seek(SeekFrom::Start(saved_position))?;

    let mut parts = Vec::new();
    collect_entity_mesh_parts(&entity, object_address, &mut parts);
    Ok(parts)
}

fn collect_entity_mesh_parts(
    entity: &EXGeoEntity,
    object_address: u64,
    parts: &mut Vec<AnimationEntityMeshPartInfo>,
) {
    match entity {
        EXGeoEntity::Mesh(mesh) => parts.push(AnimationEntityMeshPartInfo {
            object_address,
            vertex_count: mesh.vertices.len(),
        }),
        EXGeoEntity::Split(split) => {
            for child in &split.entities {
                collect_entity_mesh_parts(child, child.offset_absolute(), parts);
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
        if let Some(attachments) = skin
            .parsed
            .as_ref()
            .and_then(|parsed| parsed.bone_attachments.as_ref())
        {
            for attachment in attachments.iter() {
                let Some((_, entity)) = entities
                    .iter()
                    .find(|(entity_index, _)| *entity_index == attachment.entity_list_index())
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

enum AnimationEntityBinding {
    Skinned {
        part_vertex_ranges: Vec<std::ops::Range<usize>>,
        part_skins: Vec<AnimationPartSkin>,
        morph_scalar_base: Option<usize>,
    },
    RigidBone {
        bone_selector: usize,
    },
}

struct AnimationSkinnedEntity {
    renderer: EntityRenderer,
    original_vertices: Vec<UXVertex>,
    skinned_vertices: Vec<UXVertex>,
    binding: AnimationEntityBinding,
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
            let mut renderers = skin
                .components
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
                        binding: AnimationEntityBinding::Skinned {
                            part_vertex_ranges: mesh.part_vertex_ranges.clone(),
                            part_skins: component.part_skins.clone(),
                            morph_scalar_base: component.morph_scalar_base,
                        },
                    })
                })
                .collect::<Vec<_>>();

            if let Some(attachments) = skin
                .parsed
                .as_ref()
                .and_then(|parsed| parsed.bone_attachments.as_ref())
            {
                for attachment in attachments.iter() {
                    let bone_selector = attachment.bone_selector as usize;
                    if bone_selector >= skin.bone_hashcodes.len() {
                        warn!(
                            "Animation rigid attachment references bone {} outside {} slots",
                            bone_selector,
                            skin.bone_hashcodes.len()
                        );
                        continue;
                    }
                    let entity_index = attachment.entity_list_index();
                    let Some((_, entity)) = entities
                        .iter()
                        .find(|(candidate, _)| *candidate == entity_index)
                    else {
                        continue;
                    };
                    let Ok((_, mesh)) = entity.data.as_ref() else {
                        continue;
                    };
                    let mut renderer = EntityRenderer::new(file, platform);
                    unsafe {
                        renderer.load_mesh(gl, mesh);
                    }
                    renderers.push(AnimationSkinnedEntity {
                        renderer,
                        original_vertices: mesh.vertex_data.clone(),
                        skinned_vertices: mesh.vertex_data.clone(),
                        binding: AnimationEntityBinding::RigidBone { bone_selector },
                    });
                }
            }
            renderers
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationRuntimeStatus {
    Rendered,
    MissingAnimation,
    MissingSkin,
    /// Source Animation and requested visual AnimSkin differ, but the native
    /// FUN_004FCE8C selector remap can reconcile their serialized AnimBone maps.
    SkinMismatchRemappable,
    SkinMismatch,
    MissingPoseCache,
    InvalidPose,
    MissingGeometry,
}

pub struct AnimationRuntime {
    file: Hashcode,
    catalog: AnimationCatalog,
    pair_pose_caches: RwLock<HashMap<(usize, Hashcode), Option<AnimationPoseCache>>>,
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
            file,
            skin_renderers: Arc::new(RwLock::new(build_skin_renderers(
                file, gl, platform, &catalog, entities,
            ))),
            pair_pose_caches: RwLock::new(HashMap::new()),
            catalog,
        }
    }

    pub fn bound_skin_hashcode(&self, animation_hashcode: Hashcode) -> Option<Hashcode> {
        self.catalog.bound_skin_hashcode(animation_hashcode)
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

    /// Robots opcode-2 target-skin resource semantics from FUN_004F9E70 ->
    /// FUN_004F26AA: the serialized target may name an AnimSkin directly or
    /// another resource (shipped corpus: Animation) that resolves to its bound
    /// AnimSkin. Return the canonical target AnimSkin identity.
    pub(crate) fn resolve_animskin_reference(
        &self,
        hashcode: Hashcode,
    ) -> Option<(usize, Hashcode)> {
        if hashcode & 0x7f00_0000 == 0x0300_0000 {
            let bound_skin = self.bound_skin_hashcode(hashcode)?;
            let index = self.resolve_skin_index(bound_skin)?;
            let canonical = self.catalog.skins.get(index)?.hashcode;
            return Some((index, canonical));
        }
        let index = self.resolve_skin_index(hashcode)?;
        self.catalog
            .skins
            .get(index)
            .map(|skin| (index, skin.hashcode))
    }

    #[cfg(test)]
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

    fn pose_cache_for_skin(
        &self,
        clip_index: usize,
        animskin_hashcode: Hashcode,
    ) -> Option<AnimationPoseCache> {
        let key = (clip_index, animskin_hashcode);
        if let Some(cached) = self.pair_pose_caches.read().get(&key).cloned() {
            return cached;
        }
        let clip = self.catalog.clips.get(clip_index)?;
        let default_skin_hashcode = clip
            .skin_index
            .and_then(|skin_index| self.catalog.skins.get(skin_index))
            .map(|skin| skin.hashcode);
        let loaded = if default_skin_hashcode == Some(animskin_hashcode) {
            clip.pose_cache.clone().or_else(|| {
                load_pose_cache(
                    self.file,
                    clip.index,
                    clip.hashcode,
                    animskin_hashcode,
                    clip.motion.checksum,
                )
                .0
            })
        } else {
            load_pose_cache(
                self.file,
                clip.index,
                clip.hashcode,
                animskin_hashcode,
                clip.motion.checksum,
            )
            .0
        };
        self.pair_pose_caches.write().insert(key, loaded.clone());
        loaded
    }

    fn resolve_target_skin(
        &self,
        render_store: &RenderStore,
        clip: &AnimationClipRecord,
        requested_skin_file: Hashcode,
        requested_skin_hashcode: Hashcode,
    ) -> Option<(Arc<AnimationRuntime>, usize, Hashcode)> {
        if requested_skin_hashcode == u32::MAX {
            let skin_index = clip.skin_index?;
            let skin_hashcode = self.catalog.skins.get(skin_index)?.hashcode;
            let runtime = render_store.get_animation_runtime(self.file)?;
            return Some((runtime, skin_index, skin_hashcode));
        }

        let runtime = render_store.get_animation_runtime(requested_skin_file)?;
        let (skin_index, skin_hashcode) =
            runtime.resolve_animskin_reference(requested_skin_hashcode)?;
        Some((runtime, skin_index, skin_hashcode))
    }

    fn native_bone_remap_to(
        &self,
        visual_runtime: &AnimationRuntime,
        visual_skin_index: usize,
        animation_skin_index: usize,
    ) -> Option<NativeBoneRemap> {
        let visual_skin = visual_runtime.catalog.skins.get(visual_skin_index)?;
        let animation_skin = self.catalog.skins.get(animation_skin_index)?;
        let visual_parents = visual_skin
            .parsed
            .as_ref()?
            .hier_data
            .iter()
            .map(|hierarchy| hierarchy.link_index)
            .collect::<Vec<_>>();
        let animation_parents = animation_skin
            .parsed
            .as_ref()?
            .hier_data
            .iter()
            .map(|hierarchy| hierarchy.link_index)
            .collect::<Vec<_>>();
        build_native_bone_remap(
            &visual_skin.bone_hashcodes,
            &visual_parents,
            &animation_skin.bone_hashcodes,
            &animation_parents,
        )
    }

    pub fn status(
        &self,
        render_store: &RenderStore,
        skin_file: Hashcode,
        animation_hashcode: Hashcode,
        skin_hashcode: Hashcode,
    ) -> AnimationRuntimeStatus {
        let Some(clip_index) = resolve_clip_index(&self.catalog.clips, animation_hashcode) else {
            return AnimationRuntimeStatus::MissingAnimation;
        };
        let clip = &self.catalog.clips[clip_index];
        let Some((target_runtime, target_skin_index, target_skin_hashcode)) =
            self.resolve_target_skin(render_store, clip, skin_file, skin_hashcode)
        else {
            return AnimationRuntimeStatus::MissingSkin;
        };
        let target_skin = target_runtime.catalog.skins.get(target_skin_index);
        let target_bone_count = target_skin
            .and_then(|skin| skin.parsed.as_ref())
            .map(|skin| skin.bone_count as usize);

        if let Some(cache) = self.pose_cache_for_skin(clip_index, target_skin_hashcode) {
            if target_bone_count != Some(cache.bone_count) {
                return AnimationRuntimeStatus::InvalidPose;
            }
            if target_runtime
                .skin_renderers
                .read()
                .get(target_skin_index)
                .is_none_or(Vec::is_empty)
            {
                return AnimationRuntimeStatus::MissingGeometry;
            }
            return AnimationRuntimeStatus::Rendered;
        }

        let target_is_bound_source_skin =
            target_runtime.file == self.file && clip.skin_index == Some(target_skin_index);
        if target_is_bound_source_skin || clip.skin_index.is_none() {
            return AnimationRuntimeStatus::MissingPoseCache;
        }
        clip.skin_index
            .and_then(|animation_skin_index| {
                self.native_bone_remap_to(
                    target_runtime.as_ref(),
                    target_skin_index,
                    animation_skin_index,
                )
            })
            .filter(|remap| remap.selectors.iter().all(|selector| *selector != u8::MAX))
            .map(|_| AnimationRuntimeStatus::SkinMismatchRemappable)
            .unwrap_or(AnimationRuntimeStatus::SkinMismatch)
    }

    #[allow(clippy::too_many_arguments)]
    pub unsafe fn draw(
        &self,
        gl: &glow::Context,
        render_context: &RenderContext<'_>,
        render_store: &RenderStore,
        animation_hashcode: Hashcode,
        skin_file: Hashcode,
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
        let Some((target_runtime, target_skin_index, target_skin_hashcode)) =
            self.resolve_target_skin(render_store, clip, skin_file, skin_hashcode)
        else {
            return AnimationRuntimeStatus::MissingSkin;
        };
        let Some(cache) = self.pose_cache_for_skin(clip_index, target_skin_hashcode) else {
            let target_is_bound_source_skin =
                target_runtime.file == self.file && clip.skin_index == Some(target_skin_index);
            if target_is_bound_source_skin || clip.skin_index.is_none() {
                return AnimationRuntimeStatus::MissingPoseCache;
            }
            return clip
                .skin_index
                .and_then(|animation_skin_index| {
                    self.native_bone_remap_to(
                        target_runtime.as_ref(),
                        target_skin_index,
                        animation_skin_index,
                    )
                })
                .filter(|remap| remap.selectors.iter().all(|selector| *selector != u8::MAX))
                .map(|_| AnimationRuntimeStatus::SkinMismatchRemappable)
                .unwrap_or(AnimationRuntimeStatus::SkinMismatch);
        };
        let Some(poses) = cache.sample_phase(phase) else {
            return AnimationRuntimeStatus::InvalidPose;
        };
        let Some(morph_scalars) = cache.sample_scalar_phase(phase) else {
            return AnimationRuntimeStatus::InvalidPose;
        };
        let Some(skin) = target_runtime
            .catalog
            .skins
            .get(target_skin_index)
            .and_then(|skin| skin.parsed.as_ref())
        else {
            return AnimationRuntimeStatus::MissingSkin;
        };
        if cache.bone_count != skin.bone_count as usize {
            return AnimationRuntimeStatus::InvalidPose;
        }
        let Some(global_bone_matrices) = build_global_bone_matrices(skin, &poses) else {
            return AnimationRuntimeStatus::InvalidPose;
        };
        let Some(skin_matrices) = build_skin_matrices(skin, &poses) else {
            return AnimationRuntimeStatus::InvalidPose;
        };
        let mut all_skin_renderers = target_runtime.skin_renderers.write();
        let Some(entities) = all_skin_renderers.get_mut(target_skin_index) else {
            return AnimationRuntimeStatus::MissingGeometry;
        };
        if entities.is_empty() {
            return AnimationRuntimeStatus::MissingGeometry;
        }

        for entity in entities.iter_mut() {
            let updated = match &entity.binding {
                AnimationEntityBinding::Skinned {
                    part_vertex_ranges,
                    part_skins,
                    morph_scalar_base,
                } => skin_vertices_with_morph(
                    &entity.original_vertices,
                    &mut entity.skinned_vertices,
                    part_vertex_ranges,
                    part_skins,
                    &skin_matrices,
                    &morph_scalars,
                    *morph_scalar_base,
                ),
                AnimationEntityBinding::RigidBone { bone_selector } => global_bone_matrices
                    .get(*bone_selector)
                    .and_then(|bone_global| {
                        transform_vertices_rigid(
                            &entity.original_vertices,
                            &mut entity.skinned_vertices,
                            *bone_global,
                        )
                    }),
            };
            if updated.is_some() {
                entity
                    .renderer
                    .update_vertices(gl, &entity.skinned_vertices);
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
    format_typed_hashcode_with_id(hashcodes, "Script", script_hashcode)
}

fn semantic_animation_label(
    catalog: &AnimationCatalog,
    clip: &AnimationClipRecord,
    hashcodes: &IntMap<Hashcode, String>,
) -> String {
    let mut parts = vec![
        format_typed_hashcode_with_id(hashcodes, "Animation", clip.hashcode),
        format!("index {}", clip.index),
    ];
    if let Some(cache) = clip.pose_cache.as_ref() {
        parts.push(format!("{} asset frames", cache.frame_count));
        if cache.scalar_count != 0 {
            parts.push(format!("{} native morph scalars", cache.scalar_count));
        }
    }
    if let Some(skin_index) = clip.skin_index {
        let skin_label = catalog
            .skins
            .get(skin_index)
            .map(|skin| format_typed_hashcode_with_id(hashcodes, "AnimSkin", skin.hashcode))
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
    file: Hashcode,
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
            file,
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
                                let label =
                                    semantic_animation_label(&self.catalog, clip, &self.hashcodes);
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
                        clip.pose_cache
                            .as_ref()
                            .map(|cache| cache.frame_count.saturating_sub(1).max(1) as f32 / 30.0)
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
        let global_bone_matrices = self
            .selected_skin()
            .and_then(|skin| skin.parsed.as_ref())
            .and_then(|skin| {
                sampled_poses
                    .as_deref()
                    .and_then(|poses| build_global_bone_matrices(skin, poses))
                    .or_else(|| bind_pose_global_bone_matrices(skin))
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
                let updated = match &entity.binding {
                    AnimationEntityBinding::Skinned {
                        part_vertex_ranges,
                        part_skins,
                        ..
                    } => skin_vertices(
                        &entity.original_vertices,
                        &mut entity.skinned_vertices,
                        part_vertex_ranges,
                        part_skins,
                        &skin_matrices,
                    ),
                    AnimationEntityBinding::RigidBone { bone_selector } => global_bone_matrices
                        .get(*bone_selector)
                        .and_then(|bone_global| {
                            transform_vertices_rigid(
                                &entity.original_vertices,
                                &mut entity.skinned_vertices,
                                *bone_global,
                            )
                        }),
                };
                if updated.is_some() {
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
        let heading = semantic_animation_label(&self.catalog, clip, &self.hashcodes);

        ui.heading(heading);
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
            let source_summary = match cache.source {
                AnimationPoseSource::Rapc => format!(
                    "Native RAPCV003 compatibility cache active: {} frames, {} bones, {} morph scalars.",
                    cache.frame_count, cache.bone_count, cache.scalar_count
                ),
                AnimationPoseSource::EmbeddedEdb => format!(
                    "Native EDB decoder active: {} frames, {} bones, {} morph scalars. Skeletal and scalar playback are decoded directly from PC-v248 EDB data; RAPCV003 is not required.",
                    cache.frame_count, cache.bone_count, cache.scalar_count
                ),
            };
            ui.colored_label(egui::Color32::LIGHT_GREEN, source_summary);
            ui.monospace(format!(
                "source={} motion_fnv=0x{:016X}",
                cache.source_path.display(),
                cache.motion_checksum
            ));
            if cache.scalar_count != 0 {
                if let Some(values) = cache.sample_scalar_frame(self.selected_asset_frame()) {
                    let preview = values
                        .iter()
                        .enumerate()
                        .take(16)
                        .map(|(index, value)| format!("s{index}={value:.4}"))
                        .collect::<Vec<_>>()
                        .join("  ");
                    ui.monospace(format!("native morph scalars: {preview}"));
                    if values.len() > 16 {
                        ui.label(format!(
                            "… {} additional scalar channels",
                            values.len() - 16
                        ));
                    }
                }
            }
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
                                        ui.strong("Name");
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
                                            ui.monospace(
                                                skin.bone_hashcodes
                                                    .get(bone_index)
                                                    .and_then(|hashcode| *hashcode)
                                                    .map(|hashcode| {
                                                        format_typed_hashcode_with_id(
                                                            &self.hashcodes,
                                                            "AnimBone",
                                                            hashcode,
                                                        )
                                                    })
                                                    .unwrap_or_else(|| format!("bone_{bone_index:03}")),
                                            );
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
                                                    format_typed_hashcode_with_id(
                                                        &self.hashcodes,
                                                        "Entity",
                                                        hashcode,
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

            if !skin.bone_attachments.is_empty() {
                egui::CollapsingHeader::new(format!(
                    "Rigid bone attachments ({})",
                    skin.bone_attachments.len()
                ))
                .default_open(false)
                .show(ui, |ui| {
                    egui::Grid::new("animation_bone_attachment_grid")
                        .striped(true)
                        .show(ui, |ui| {
                            ui.strong("#");
                            ui.strong("Bone");
                            ui.strong("Entity");
                            ui.end_row();
                            for attachment in &skin.bone_attachments {
                                ui.monospace(attachment.attachment_index.to_string());
                                ui.monospace(
                                    attachment
                                        .bone_hashcode
                                        .map(|hashcode| {
                                            format_typed_hashcode_with_id(
                                                &self.hashcodes,
                                                "AnimBone",
                                                hashcode,
                                            )
                                        })
                                        .unwrap_or_else(|| {
                                            format!("bone_{:03}", attachment.bone_selector)
                                        }),
                                );
                                ui.monospace(
                                    attachment
                                        .entity_hashcode
                                        .map(|hashcode| {
                                            format_typed_hashcode_with_id(
                                                &self.hashcodes,
                                                "Entity",
                                                hashcode,
                                            )
                                        })
                                        .unwrap_or_else(|| {
                                            format!(
                                                "entity_index={} raw=0x{:08X}",
                                                attachment.entity_index,
                                                attachment.raw_entity_index
                                            )
                                        }),
                                );
                                ui.end_row();
                            }
                        });
                });
            }

            if let Some(post_pair) = skin
                .parsed
                .as_ref()
                .and_then(|parsed| parsed.robots_post_pair_block.as_ref())
            {
                let block = post_pair.data_ref();
                egui::CollapsingHeader::new(format!(
                    "Post-pair bone metadata ({} records, {} sparse pairs)",
                    block.bone_records.len(),
                    block.sparse_pairs.len()
                ))
                .default_open(false)
                .show(ui, |ui| {
                    ui.colored_label(
                        egui::Color32::GRAY,
                        "Robots v248 +0x5C structure is proven; sparse-pair and eight-float record semantics remain intentionally unnamed.",
                    );
                    ui.monospace(format!(
                        "Address 0x{:08X}, parsed size {} bytes",
                        post_pair.offset_absolute(),
                        block.serialized_size()
                    ));
                    if !block.sparse_pairs.is_empty() {
                        egui::Grid::new("animation_post_pair_sparse_grid")
                            .striped(true)
                            .show(ui, |ui| {
                                ui.strong("#");
                                ui.strong("Left");
                                ui.strong("Right");
                                ui.end_row();
                                for (pair_index, [left, right]) in
                                    block.sparse_pairs.iter().copied().enumerate()
                                {
                                    ui.monospace(pair_index.to_string());
                                    ui.monospace(format!("{} [0x{:04X}]", left, left));
                                    ui.monospace(format!("{} [0x{:04X}]", right, right));
                                    ui.end_row();
                                }
                            });
                    }
                });
            }

            if let Some(animdatums) = skin
                .parsed
                .as_ref()
                .and_then(|parsed| parsed.robots_animdatum_section.as_ref())
                .filter(|animdatums| animdatums.serialized_len() != 0)
            {
                egui::CollapsingHeader::new(format!(
                    "AnimDatum collision/query records ({})",
                    animdatums.serialized_len()
                ))
                .default_open(false)
                .show(ui, |ui| {
                    ui.colored_label(
                        egui::Color32::GRAY,
                        "Native AnimSkin +0x60/+0x64 channel used by 0x00500569. Search heads follow skip_count; continuation records remain visible for provenance.",
                    );
                    if let Some(header) = animdatums.header.as_ref() {
                        ui.monospace(format!(
                            "Index: 0x{:08X}  First datum: 0x{:08X}",
                            header.index_offset_absolute(),
                            header.first_datum_offset_absolute()
                        ));
                    }
                    egui::Grid::new("animation_animdatum_grid")
                        .striped(true)
                        .show(ui, |ui| {
                            ui.strong("#");
                            ui.strong("Search");
                            ui.strong("AnimDatum");
                            ui.strong("Skip");
                            ui.strong("Mode");
                            ui.strong("Shape / scalars");
                            ui.strong("Transform");
                            ui.strong("Datum");
                            ui.end_row();
                            for (entry_index, entry) in animdatums.entries.iter().enumerate() {
                                let datum = entry.datum();
                                let hierarchy_chain = datum
                                    .hierarchy_chain
                                    .iter()
                                    .map(|selector| selector.to_string())
                                    .collect::<Vec<_>>()
                                    .join("→");
                                let transform_bone = skin
                                    .bone_hashcodes
                                    .get(datum.transform_selector as usize)
                                    .and_then(|hashcode| *hashcode)
                                    .map(|hashcode| {
                                        format_typed_hashcode_with_id(
                                            &self.hashcodes,
                                            "AnimBone",
                                            hashcode,
                                        )
                                    })
                                    .unwrap_or_else(|| format!("bone_{:03}", datum.transform_selector));
                                let shape = if entry.hashcode == 0x1000_0004 {
                                    if datum.header.shape_mode == 3 {
                                        format!(
                                            "capsule half={:.4} r={:.4}",
                                            datum.map_collision_half_segment().unwrap_or_default(),
                                            datum.map_collision_radius().unwrap_or_default()
                                        )
                                    } else {
                                        format!(
                                            "sphere r={:.4}",
                                            datum.map_collision_radius().unwrap_or_default()
                                        )
                                    }
                                } else {
                                    format!(
                                        "[{:.4}, {:.4}, {:.4}]",
                                        datum.shape_scalars[0],
                                        datum.shape_scalars[1],
                                        datum.shape_scalars[2]
                                    )
                                };
                                ui.monospace(entry_index.to_string());
                                ui.monospace(if entry.searchable_head { "head" } else { "cont" });
                                ui.monospace(format_hashcode_with_id(&self.hashcodes, entry.hashcode));
                                ui.monospace(entry.skip_count.to_string());
                                ui.monospace(format!("0x{:02X}", datum.header.shape_mode));
                                ui.monospace(shape);
                                ui.monospace(format!("{} [{}]", transform_bone, hierarchy_chain));
                                ui.monospace(format!("0x{:08X}", entry.datum_offset_absolute()))
                                    .on_hover_text(format!(
                                        "raw +04 = 0x{:04X}\nraw +07 = 0x{:02X}\ncenter = [{:.6}, {:.6}, {:.6}]\nquaternion = [{:.6}, {:.6}, {:.6}, {:.6}]\nserialized_size = {} bytes",
                                        datum.header.raw_word_04,
                                        datum.header.raw_byte_07,
                                        datum.local_center[0],
                                        datum.local_center[1],
                                        datum.local_center[2],
                                        datum.local_orientation[0],
                                        datum.local_orientation[1],
                                        datum.local_orientation[2],
                                        datum.local_orientation[3],
                                        datum.serialized_size(),
                                    ));
                                ui.end_row();
                            }
                        });
                });
            }
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
                    let resolved_target = {
                        let render_store = self.render_store.read();
                        resolve_animation_skin_target(
                            self.file,
                            usage.skin_file,
                            usage.skin_hashcode,
                            self.file,
                            clip.hashcode,
                            &render_store,
                        )
                    };
                    let skin_label = if usage.skin_hashcode == u32::MAX {
                        resolved_target
                            .map(|(file, skin)| {
                                format!(
                                    "implicit -> {} @ {}",
                                    format_typed_hashcode_with_id(
                                        &self.hashcodes,
                                        "AnimSkin",
                                        skin,
                                    ),
                                    format_hashcode_with_id(&self.hashcodes, file)
                                )
                            })
                            .unwrap_or_else(|| {
                                "implicit Animation binding [0xFFFFFFFF]".to_string()
                            })
                    } else if usage.skin_hashcode & 0x7f00_0000 == 0x0300_0000 {
                        let indirect = format_typed_hashcode_with_id(
                            &self.hashcodes,
                            "Animation",
                            usage.skin_hashcode,
                        );
                        resolved_target
                            .map(|(file, skin)| {
                                format!(
                                    "indirect via {indirect} -> {} @ {}",
                                    format_typed_hashcode_with_id(
                                        &self.hashcodes,
                                        "AnimSkin",
                                        skin,
                                    ),
                                    format_hashcode_with_id(&self.hashcodes, file)
                                )
                            })
                            .unwrap_or_else(|| {
                                format!("indirect via {indirect} -> unresolved AnimSkin")
                            })
                    } else if let Some((file, skin)) = resolved_target {
                        format!(
                            "{} @ {}",
                            format_typed_hashcode_with_id(&self.hashcodes, "AnimSkin", skin),
                            format_hashcode_with_id(&self.hashcodes, file)
                        )
                    } else {
                        format_typed_hashcode_with_id(
                            &self.hashcodes,
                            "AnimSkin target",
                            usage.skin_hashcode,
                        )
                    };
                    ui.monospace(format!(
                        "script {}, command {}, start {}, length {}, {:.3} fps, skin {}",
                        semantic_script_reference(&self.hashcodes, usage.script_hashcode),
                        usage.command_index,
                        usage.start_frame,
                        usage.length_frames,
                        usage.script_fps,
                        skin_label,
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
            file: 0x0100_0001,
            catalog: AnimationCatalog {
                clips: vec![bound_clip],
                skins: vec![AnimationSkinRecord {
                    index: 0,
                    hashcode: 0x0D00_0001,
                    base_skin_num: 0,
                    mip_ref: 0,
                    bone_hashcodes: Vec::new(),
                    bone_attachments: Vec::new(),
                    parsed: None,
                    parse_error: None,
                    components: Vec::new(),
                    center: Vec3::ZERO,
                    maximum_extent: 0.0,
                    bind_pose_identity_error: None,
                }],
            },
            pair_pose_caches: RwLock::new(HashMap::new()),
            skin_renderers: Arc::new(RwLock::new(vec![Vec::new()])),
        };
        let clip = &runtime.catalog.clips[0];
        assert_eq!(runtime.resolve_clip_skin_index(clip, u32::MAX), Some(0));
        assert_eq!(runtime.resolve_clip_skin_index(clip, 0x8D00_0000), Some(0));
    }

    #[test]
    fn local_animation_reference_in_skin_slot_beats_same_index_animskin() {
        let mut indirect_clip = clip(1, 0x0300_0020);
        indirect_clip.skin_index = Some(0);
        let make_skin = |index, hashcode| AnimationSkinRecord {
            index,
            hashcode,
            base_skin_num: index as u32,
            mip_ref: 0,
            bone_hashcodes: Vec::new(),
            bone_attachments: Vec::new(),
            parsed: None,
            parse_error: None,
            components: Vec::new(),
            center: Vec3::ZERO,
            maximum_extent: 0.0,
            bind_pose_identity_error: None,
        };
        let runtime = AnimationRuntime {
            file: 0x0100_0001,
            catalog: AnimationCatalog {
                clips: vec![clip(0, 0x0300_0010), indirect_clip],
                skins: vec![make_skin(0, 0x0D00_0010), make_skin(1, 0x0D00_0020)],
            },
            pair_pose_caches: RwLock::new(HashMap::new()),
            skin_renderers: Arc::new(RwLock::new(vec![Vec::new(), Vec::new()])),
        };

        assert_eq!(
            runtime.resolve_animskin_reference(0x8300_0001),
            Some((0, 0x0D00_0010))
        );
        assert_eq!(
            runtime.resolve_animskin_reference(0x8D00_0001),
            Some((1, 0x0D00_0020))
        );
    }

    #[test]
    fn generic_animation_resolves_explicit_cross_file_skin_before_pose_cache() {
        let source_file = 0x0100_00A0;
        let target_file = 0x0100_00A1;
        let animation_hashcode = 0x0300_00A0;
        let target_skin_hashcode = 0x0D00_00A1;

        let source_runtime = Arc::new(AnimationRuntime {
            file: source_file,
            catalog: AnimationCatalog {
                clips: vec![clip(0, animation_hashcode)],
                skins: Vec::new(),
            },
            pair_pose_caches: RwLock::new(HashMap::new()),
            skin_renderers: Arc::new(RwLock::new(Vec::new())),
        });
        let target_runtime = Arc::new(AnimationRuntime {
            file: target_file,
            catalog: AnimationCatalog {
                clips: Vec::new(),
                skins: vec![AnimationSkinRecord {
                    index: 0,
                    hashcode: target_skin_hashcode,
                    base_skin_num: 0x8D00_00A1,
                    mip_ref: 0,
                    bone_hashcodes: Vec::new(),
                    bone_attachments: Vec::new(),
                    parsed: None,
                    parse_error: None,
                    components: Vec::new(),
                    center: Vec3::ZERO,
                    maximum_extent: 0.0,
                    bind_pose_identity_error: None,
                }],
            },
            pair_pose_caches: RwLock::new(HashMap::new()),
            skin_renderers: Arc::new(RwLock::new(vec![Vec::new()])),
        });
        let mut store = RenderStore::new();
        store.insert_animation_runtime(source_file, source_runtime.clone());
        store.insert_animation_runtime(target_file, target_runtime);

        assert_eq!(
            source_runtime.status(
                &store,
                target_file,
                animation_hashcode,
                target_skin_hashcode,
            ),
            AnimationRuntimeStatus::MissingPoseCache
        );
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
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&checksum.to_le_bytes());
        for (values, scalar) in [
            ([0.0f32, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0], 0.0f32),
            ([2.0f32, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0], 1.0f32),
        ] {
            for value in values {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
            bytes.extend_from_slice(&scalar.to_le_bytes());
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
        let sample = cache.sample_frame(0.5).expect("interpolated pose sample");
        assert_eq!(sample.len(), 1);
        assert!((sample[0].position.x - 1.0).abs() < 1.0e-6);
        assert!((sample[0].rotation.length() - 1.0).abs() < 1.0e-6);
        assert!((sample[0].rotation.z.abs() - std::f32::consts::FRAC_1_SQRT_2).abs() < 1.0e-5);
        assert!((sample[0].rotation.w.abs() - std::f32::consts::FRAC_1_SQRT_2).abs() < 1.0e-5);
        let scalars = cache
            .sample_scalar_frame(0.5)
            .expect("interpolated morph scalar sample");
        assert_eq!(scalars.len(), 1);
        assert!((scalars[0] - 0.5).abs() < 1.0e-6);

        let last = cache.sample_phase(1.0).expect("last pose sample");
        assert!((last[0].position.x - 2.0).abs() < 1.0e-6);
        let after_end = cache.sample_phase(2.0).expect("clamped last pose sample");
        assert!((after_end[0].position.x - 2.0).abs() < 1.0e-6);
    }

    #[test]
    fn real_robots_v248_sweeper_attack_r_hand_frame_16_5_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/nb11_rat.edb");
        let file = std::fs::File::open(&path).expect("open nb11_rat.edb");
        let mut edb = EdbFile::new(
            Box::new(std::io::BufReader::new(file)),
            eurochef_edb::versions::Platform::Pc,
        )
        .expect("parse nb11_rat.edb");
        assert_eq!(edb.header.hashcode, 0x0100_0051);
        let catalog = read_from_file(&mut edb).expect("decode nb11_rat animation catalog");
        assert_eq!(catalog.bound_skin_hashcode(0x8300_0009), Some(0x0D00_0001));
        let hand = sample_bound_animation_bone_position(&catalog, 0x8300_0009, 0x0E00_0015, 16.5)
            .expect("sample Attack R_Hand at native missile callback frame");
        let expected = Vec3::new(-0.101_767_67, 0.788_267_73, 0.772_494_4);
        assert!(
            hand.distance(expected) < 1.0e-6,
            "native R_Hand frame16.5 mismatch: {hand:?} != {expected:?}"
        );
    }

    #[test]
    fn real_robots_v248_embedded_pose_corpus_when_binding_report_is_configured() {
        let Ok(report_path) = std::env::var("ROBOTS_ANIMATION_BINDING_CORPUS") else {
            return;
        };
        let report = std::fs::read_to_string(&report_path).expect("read animation binding corpus");
        let mut lines = report.lines();
        let header = lines.next().expect("animation binding corpus header");
        let columns = header.split('\t').collect::<Vec<_>>();
        let path_column = columns
            .iter()
            .position(|column| *column == "edb_path")
            .expect("edb_path column");
        let status_column = columns
            .iter()
            .position(|column| *column == "skin_binding_status")
            .expect("skin_binding_status column");
        let mut expected_bound_clips = 0usize;
        let mut paths = std::collections::BTreeSet::new();
        for line in lines {
            let fields = line.split('\t').collect::<Vec<_>>();
            if fields.get(status_column) != Some(&"resolved_by_base_skin_num") {
                continue;
            }
            expected_bound_clips += 1;
            let raw_path = fields.get(path_column).expect("binding EDB path");
            paths.insert(raw_path.replace("\\\\", "\\"));
        }
        assert!(
            !paths.is_empty(),
            "binding corpus has no resolved EDB paths"
        );

        let mut decoded_bound_clips = 0usize;
        let mut oracle_compared_clips = 0usize;
        let mut oracle_missing_clips = 0usize;
        let mut max_position_error = 0.0f32;
        let mut max_quaternion_component_error = 0.0f32;
        let mut max_scalar_error = 0.0f32;
        let mut worst_position = None;
        let mut worst_quaternion = None;
        let mut worst_scalar = None;
        let mut failures = Vec::new();
        for path in &paths {
            let platform = eurochef_edb::versions::Platform::from_path(path)
                .unwrap_or(eurochef_edb::versions::Platform::Pc);
            let file = match std::fs::File::open(path) {
                Ok(file) => file,
                Err(error) => {
                    failures.push(format!("open {path}: {error}"));
                    continue;
                }
            };
            let reader = std::io::BufReader::new(file);
            let mut edb = match EdbFile::new(Box::new(reader), platform) {
                Ok(edb) => edb,
                Err(error) => {
                    failures.push(format!("parse {path}: {error}"));
                    continue;
                }
            };
            let catalog = match read_from_file(&mut edb) {
                Ok(catalog) => catalog,
                Err(error) => {
                    failures.push(format!("catalog {path}: {error}"));
                    continue;
                }
            };
            for clip in catalog
                .clips
                .iter()
                .filter(|clip| clip.skin_index.is_some())
            {
                decoded_bound_clips += 1;
                let Some(embedded) = clip.pose_cache.as_ref() else {
                    failures.push(format!(
                        "{path} clip {} 0x{:08X}: {:?}",
                        clip.index, clip.hashcode, clip.pose_cache_error
                    ));
                    continue;
                };
                if embedded.source == AnimationPoseSource::Rapc {
                    failures.push(format!(
                        "{path} clip {} 0x{:08X}: RAPC fallback {:?}",
                        clip.index, clip.hashcode, clip.pose_cache_error
                    ));
                    continue;
                }

                let skin = &catalog.skins[clip.skin_index.expect("bound skin index")];
                let (oracle, oracle_error) = load_pose_cache(
                    edb.header.hashcode,
                    clip.index,
                    clip.hashcode,
                    skin.hashcode,
                    clip.motion.checksum,
                );
                let Some(oracle) = oracle else {
                    oracle_missing_clips += 1;
                    if let Some(error) = oracle_error {
                        eprintln!(
                            "RAPCV003 oracle missing for {path} clip {}: {error}",
                            clip.index
                        );
                    }
                    continue;
                };
                oracle_compared_clips += 1;
                if embedded.frame_count != oracle.frame_count
                    || embedded.bone_count != oracle.bone_count
                    || embedded.scalar_count != oracle.scalar_count
                    || embedded.poses.len() != oracle.poses.len()
                    || embedded.scalars.len() != oracle.scalars.len()
                {
                    failures.push(format!(
                        "{path} clip {} cache dimensions embedded={}/{}/{}/{}/{} oracle={}/{}/{}/{}/{}",
                        clip.index,
                        embedded.frame_count,
                        embedded.bone_count,
                        embedded.scalar_count,
                        embedded.poses.len(),
                        embedded.scalars.len(),
                        oracle.frame_count,
                        oracle.bone_count,
                        oracle.scalar_count,
                        oracle.poses.len(),
                        oracle.scalars.len()
                    ));
                    continue;
                }
                for (pose_index, (actual, expected)) in
                    embedded.poses.iter().zip(&oracle.poses).enumerate()
                {
                    let position_error = (actual.position.x - expected.position.x)
                        .abs()
                        .max((actual.position.y - expected.position.y).abs())
                        .max((actual.position.z - expected.position.z).abs());
                    if position_error > max_position_error {
                        max_position_error = position_error;
                        worst_position = Some((
                            path.clone(),
                            clip.index,
                            pose_index / embedded.bone_count,
                            pose_index % embedded.bone_count,
                        ));
                    }
                    let direct = (actual.rotation.x - expected.rotation.x)
                        .abs()
                        .max((actual.rotation.y - expected.rotation.y).abs())
                        .max((actual.rotation.z - expected.rotation.z).abs())
                        .max((actual.rotation.w - expected.rotation.w).abs());
                    let negated = (actual.rotation.x + expected.rotation.x)
                        .abs()
                        .max((actual.rotation.y + expected.rotation.y).abs())
                        .max((actual.rotation.z + expected.rotation.z).abs())
                        .max((actual.rotation.w + expected.rotation.w).abs());
                    let quaternion_error = direct.min(negated);
                    if quaternion_error > max_quaternion_component_error {
                        max_quaternion_component_error = quaternion_error;
                        worst_quaternion = Some((
                            path.clone(),
                            clip.index,
                            pose_index / embedded.bone_count,
                            pose_index % embedded.bone_count,
                        ));
                    }
                }
                for (scalar_index, (actual, expected)) in
                    embedded.scalars.iter().zip(&oracle.scalars).enumerate()
                {
                    let scalar_error = (actual - expected).abs();
                    if scalar_error > max_scalar_error {
                        max_scalar_error = scalar_error;
                        worst_scalar = Some((
                            path.clone(),
                            clip.index,
                            scalar_index / embedded.scalar_count.max(1),
                            scalar_index % embedded.scalar_count.max(1),
                        ));
                    }
                }
            }
        }

        assert_eq!(
            decoded_bound_clips, expected_bound_clips,
            "binding corpus/catalog bound-clip count mismatch"
        );
        assert!(
            failures.is_empty(),
            "embedded EDB animation decoder failed corpus entries:\n{}",
            failures.join("\n")
        );
        assert_eq!(
            oracle_missing_clips, 0,
            "RAPCV003 oracle coverage is incomplete for the bound corpus"
        );
        assert!(
            max_position_error < 1.0e-3,
            "embedded/native position mismatch max={max_position_error} worst={worst_position:?}"
        );
        assert!(
            max_quaternion_component_error < 1.0e-3,
            "embedded/native quaternion mismatch max={max_quaternion_component_error} worst={worst_quaternion:?}"
        );
        assert!(
            max_scalar_error < 1.0e-3,
            "embedded/native scalar mismatch max={max_scalar_error} worst={worst_scalar:?}"
        );
        eprintln!(
            "Robots embedded EDB animation corpus: files={} bound_clips={decoded_bound_clips} oracle_clips={oracle_compared_clips} max_position_error={max_position_error:.9} max_quaternion_component_error={max_quaternion_component_error:.9} max_scalar_error={max_scalar_error:.9}",
            paths.len()
        );
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
        if edb.header.version == 248 && platform == Platform::Pc {
            let rapc_only = catalog
                .clips
                .iter()
                .filter(|clip| clip.skin_index.is_some())
                .filter(|clip| {
                    clip.pose_cache
                        .as_ref()
                        .is_none_or(|cache| cache.source == AnimationPoseSource::Rapc)
                })
                .map(|clip| (clip.index, clip.pose_cache_error.clone()))
                .collect::<Vec<_>>();
            assert!(
                rapc_only.is_empty(),
                "Robots PC v248 bound clips must use the embedded EDB skeletal decoder; RAPC-only/missing={rapc_only:#?}"
            );
        }
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
    fn collect_test_edb_paths(root: &Path, output: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(root) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_test_edb_paths(&path, output);
            } else if path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("edb"))
            {
                output.push(path);
            }
        }
    }

    #[test]
    fn real_animation_manifest_when_requested() {
        let Ok(manifest_path) = std::env::var("ROBOTS_ANIMATION_MANIFEST") else {
            return;
        };
        let manifest = std::fs::read_to_string(&manifest_path).expect("read animation manifest");
        let mut edb_paths = manifest
            .lines()
            .skip(1)
            .filter_map(|line| {
                line.split_once('\t')
                    .map(|(_, path)| PathBuf::from(path.trim()))
            })
            .filter(|path| {
                path.is_file()
                    && path
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("edb"))
            })
            .collect::<Vec<_>>();
        if edb_paths.is_empty() {
            let manifest_path = Path::new(&manifest_path);
            let source_root = manifest_path
                .parent()
                .and_then(Path::parent)
                .expect("canonical manifest should live below _eurotools_out")
                .join("extracted_main/robots/binary/_bin_pc");
            collect_test_edb_paths(&source_root, &mut edb_paths);
        }
        edb_paths.sort();
        edb_paths.dedup();

        let mut corpus_headers = std::collections::BTreeMap::new();
        for path in &edb_paths {
            let platform = eurochef_edb::versions::Platform::from_path(path)
                .expect("manifest EDB platform should be detectable");
            let file = std::fs::File::open(path).expect("open manifest EDB for header catalog");
            let reader = std::io::BufReader::new(file);
            let edb = EdbFile::new(Box::new(reader), platform)
                .expect("parse manifest EDB for header catalog");
            corpus_headers.insert(edb.header.hashcode, (path.clone(), edb.header.clone()));
        }

        let mut files = 0usize;
        let mut clips = 0usize;
        let mut skins = 0usize;
        let mut rigid_bone_attachments = 0usize;
        let mut animdatum_records = 0usize;
        let mut native_bound_clips = 0usize;
        let mut native_pose_caches_loaded = 0usize;
        let mut native_pose_cache_errors = Vec::new();
        let mut motion_failures = Vec::new();
        let mut skin_failures = Vec::new();
        let mut bind_pose_failures = Vec::new();

        for path in edb_paths {
            let platform = eurochef_edb::versions::Platform::from_path(&path)
                .expect("manifest EDB platform should be detectable");
            let file = std::fs::File::open(&path).expect("open manifest EDB");
            let reader = std::io::BufReader::new(file);
            let mut edb = EdbFile::new(Box::new(reader), platform).expect("parse manifest EDB");
            let catalog = read_from_file(&mut edb).expect("read manifest animation catalog");
            files += 1;
            clips += catalog.clips.len();
            skins += catalog.skins.len();
            rigid_bone_attachments += catalog
                .skins
                .iter()
                .map(|skin| skin.bone_attachments.len())
                .sum::<usize>();
            animdatum_records += catalog
                .skins
                .iter()
                .filter_map(|skin| skin.parsed.as_ref())
                .filter_map(|skin| skin.robots_animdatum_section.as_ref())
                .map(|auxiliary| auxiliary.entries.len())
                .sum::<usize>();
            for clip in &catalog.clips {
                if clip.skin_index.is_some() {
                    native_bound_clips += 1;
                    if clip.pose_cache.is_some() {
                        native_pose_caches_loaded += 1;
                    } else if let Some(error) = clip.pose_cache_error.as_deref() {
                        if native_pose_cache_errors.len() < 16 {
                            native_pose_cache_errors.push(format!(
                                "{} animation={} 0x{:08X}: {error}",
                                path.display(),
                                clip.index,
                                clip.hashcode
                            ));
                        }
                    }
                }
            }

            for clip in catalog.clips {
                if let Some(error) = clip.motion.read_error {
                    motion_failures.push(format!(
                        "{}: animation {}: {error}",
                        path.display(),
                        clip.index
                    ));
                }
            }
            for skin in catalog.skins {
                if let Some(error) = skin.parse_error {
                    skin_failures.push(format!(
                        "{}: AnimSkin {}: {error}",
                        path.display(),
                        skin.index
                    ));
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
                                    "{}: AnimSkin {}: max identity error {error}",
                                    path.display(),
                                    skin.index
                                ));
                            }
                        }
                        None => bind_pose_failures.push(format!(
                            "{}: AnimSkin {}: invalid bind hierarchy",
                            path.display(),
                            skin.index
                        )),
                    }
                }
            }
        }

        let resolve_corpus_object = |current_file: Hashcode,
                                     file_ref: Hashcode,
                                     object_ref: Hashcode,
                                     is_skin: bool|
         -> Option<(Hashcode, usize, Hashcode)> {
            let resolved_file = if file_ref == u32::MAX || object_ref.is_local() {
                current_file
            } else {
                file_ref
            };
            let (_, header) = corpus_headers.get(&resolved_file)?;
            if is_skin {
                if object_ref & 0x7f00_0000 == 0x0300_0000 {
                    let animation = if object_ref.is_local() {
                        header.anim_list.data().get(object_ref.index() as usize)
                    } else {
                        header
                            .anim_list
                            .iter()
                            .find(|entry| entry.common.hashcode == object_ref)
                    }?;
                    header
                        .animskin_list
                        .iter()
                        .enumerate()
                        .find(|(_, entry)| entry.base_skin_num == animation.skin_num)
                        .map(|(index, entry)| (resolved_file, index, entry.common.hashcode))
                } else if object_ref.is_local() {
                    let index = object_ref.index() as usize;
                    header
                        .animskin_list
                        .data()
                        .get(index)
                        .map(|entry| (resolved_file, index, entry.common.hashcode))
                } else {
                    header
                        .animskin_list
                        .iter()
                        .enumerate()
                        .find(|(_, entry)| entry.common.hashcode == object_ref)
                        .map(|(index, entry)| (resolved_file, index, entry.common.hashcode))
                }
            } else if object_ref.is_local() {
                let index = object_ref.index() as usize;
                header
                    .anim_list
                    .data()
                    .get(index)
                    .map(|entry| (resolved_file, index, entry.common.hashcode))
            } else {
                header
                    .anim_list
                    .iter()
                    .enumerate()
                    .find(|(_, entry)| entry.common.hashcode == object_ref)
                    .map(|(index, entry)| (resolved_file, index, entry.common.hashcode))
            }
        };
        let mut exact_script_pose_pairs = std::collections::BTreeSet::new();
        let mut pose_key_target_files = std::collections::BTreeMap::<
            (Hashcode, usize, Hashcode),
            std::collections::BTreeSet<Hashcode>,
        >::new();
        let mut corpus_animation_commands = 0usize;
        let mut corpus_explicit_skin_commands = 0usize;
        let mut corpus_unresolved_animation_commands = 0usize;
        let mut corpus_unresolved_skin_commands = 0usize;
        for (script_file_uid, (path, _)) in &corpus_headers {
            let platform = eurochef_edb::versions::Platform::from_path(path)
                .expect("script EDB platform should be detectable");
            let file = std::fs::File::open(path).expect("open EDB for script binding census");
            let mut edb = EdbFile::new(Box::new(std::io::BufReader::new(file)), platform)
                .expect("parse EDB for script binding census");
            let scripts = UXGeoScript::read_all(&mut edb).expect("read scripts for binding census");
            for script in scripts {
                for command in script.commands {
                    let UXGeoScriptCommandData::Animation {
                        skin_file,
                        skin_hashcode,
                        anim_file,
                        anim_hashcode,
                    } = command.data
                    else {
                        continue;
                    };
                    corpus_animation_commands += 1;
                    let Some((animation_file_uid, animation_index, animation_uid)) =
                        resolve_corpus_object(*script_file_uid, anim_file, anim_hashcode, false)
                    else {
                        corpus_unresolved_animation_commands += 1;
                        continue;
                    };
                    if matches!(skin_hashcode, 0 | u32::MAX) {
                        continue;
                    }
                    corpus_explicit_skin_commands += 1;
                    let Some((skin_source_file_uid, _, skin_uid)) =
                        resolve_corpus_object(*script_file_uid, skin_file, skin_hashcode, true)
                    else {
                        corpus_unresolved_skin_commands += 1;
                        continue;
                    };
                    exact_script_pose_pairs.insert((
                        animation_file_uid,
                        animation_index,
                        animation_uid,
                        skin_source_file_uid,
                        skin_uid,
                    ));
                    pose_key_target_files
                        .entry((animation_file_uid, animation_index, skin_uid))
                        .or_default()
                        .insert(skin_source_file_uid);
                }
            }
        }
        let pose_key_cross_file_collisions = pose_key_target_files
            .iter()
            .filter(|(_, target_files)| target_files.len() > 1)
            .map(|(key, target_files)| (*key, target_files.clone()))
            .collect::<Vec<_>>();
        eprintln!(
            "Robots exact Script pose-pair corpus: animation_commands={corpus_animation_commands} explicit_skin_commands={corpus_explicit_skin_commands} unresolved_animation={corpus_unresolved_animation_commands} unresolved_skin={corpus_unresolved_skin_commands} exact_pairs={} legacy_pose_keys={} cross_target_file_collisions={} collision_samples={pose_key_cross_file_collisions:#?}",
            exact_script_pose_pairs.len(),
            pose_key_target_files.len(),
            pose_key_cross_file_collisions.len(),
        );
        assert_eq!(corpus_animation_commands, 1573);
        assert_eq!(corpus_explicit_skin_commands, 411);
        assert_eq!(corpus_unresolved_animation_commands, 0);
        assert_eq!(corpus_unresolved_skin_commands, 0);
        assert_eq!(exact_script_pose_pairs.len(), 365);
        assert_eq!(pose_key_target_files.len(), 365);
        assert!(pose_key_cross_file_collisions.is_empty());

        if std::env::var_os("ROBOTS_ANIMATION_POSE_CACHE").is_some() {
            let mut cache_pairs_checked = 0usize;
            let mut cache_native_pairs = 0usize;
            let mut cache_variant_pairs = 0usize;
            let mut cache_failures = Vec::new();
            for (animation_file_uid, animation_index, animation_uid, skin_file_uid, skin_uid) in
                &exact_script_pose_pairs
            {
                let Some((animation_path, animation_header)) =
                    corpus_headers.get(animation_file_uid)
                else {
                    cache_failures
                        .push(format!("missing Animation EDB 0x{animation_file_uid:08X}"));
                    continue;
                };
                let Some(animation) = animation_header.anim_list.data().get(*animation_index)
                else {
                    cache_failures.push(format!(
                        "Animation 0x{animation_uid:08X} index {animation_index} missing in 0x{animation_file_uid:08X}"
                    ));
                    continue;
                };
                if animation.common.hashcode != *animation_uid {
                    cache_failures.push(format!(
                        "Animation index/hash mismatch in 0x{animation_file_uid:08X}:{animation_index}"
                    ));
                    continue;
                }
                let platform = eurochef_edb::versions::Platform::from_path(animation_path)
                    .expect("Animation cache EDB platform should be detectable");
                let file = std::fs::File::open(animation_path)
                    .expect("open Animation EDB for cache validation");
                let mut animation_edb =
                    EdbFile::new(Box::new(std::io::BufReader::new(file)), platform)
                        .expect("parse Animation EDB for cache validation");
                let motion = read_motion_data(
                    &mut animation_edb,
                    animation_header,
                    animation.motiondata_info_addr,
                    animation.datasize,
                );

                let Some((skin_path, skin_header)) = corpus_headers.get(skin_file_uid) else {
                    cache_failures.push(format!("missing AnimSkin EDB 0x{skin_file_uid:08X}"));
                    continue;
                };
                let Some(serialized_skin) = skin_header
                    .animskin_list
                    .iter()
                    .find(|entry| entry.common.hashcode == *skin_uid)
                else {
                    cache_failures.push(format!(
                        "AnimSkin 0x{skin_uid:08X} missing in 0x{skin_file_uid:08X}"
                    ));
                    continue;
                };
                let platform = eurochef_edb::versions::Platform::from_path(skin_path)
                    .expect("AnimSkin cache EDB platform should be detectable");
                let file =
                    std::fs::File::open(skin_path).expect("open AnimSkin EDB for cache validation");
                let mut skin_edb = EdbFile::new(Box::new(std::io::BufReader::new(file)), platform)
                    .expect("parse AnimSkin EDB for cache validation");
                skin_edb
                    .seek(SeekFrom::Start(serialized_skin.common.address as u64))
                    .expect("seek AnimSkin for cache validation");
                let parsed_skin = skin_edb
                    .read_type_args::<EXGeoBaseAnimSkin>(skin_edb.endian, (skin_header.version,))
                    .expect("parse AnimSkin for cache validation");

                let native_skin = animation_header
                    .animskin_list
                    .iter()
                    .find(|entry| entry.base_skin_num == animation.skin_num);
                let native_pair = *animation_file_uid == *skin_file_uid
                    && native_skin
                        .map(|entry| entry.common.hashcode == *skin_uid)
                        .unwrap_or(false);
                if native_pair {
                    cache_native_pairs += 1;
                } else {
                    cache_variant_pairs += 1;
                }

                match load_pose_cache(
                    *animation_file_uid,
                    *animation_index,
                    *animation_uid,
                    *skin_uid,
                    motion.checksum,
                ) {
                    (Some(cache), None) => {
                        cache_pairs_checked += 1;
                        if cache.bone_count != parsed_skin.bone_count as usize {
                            cache_failures.push(format!(
                                "cache bone count mismatch {} != {} for 0x{animation_file_uid:08X}:{animation_index} -> 0x{skin_file_uid:08X}/0x{skin_uid:08X}",
                                cache.bone_count,
                                parsed_skin.bone_count
                            ));
                        }
                    }
                    (cache, error) => cache_failures.push(format!(
                        "cache missing/rejected for 0x{animation_file_uid:08X}:{animation_index} -> 0x{skin_file_uid:08X}/0x{skin_uid:08X}: cache={} error={:?}",
                        cache.is_some(),
                        error
                    )),
                }
            }
            eprintln!(
                "Robots Script RAPCV003 coverage: checked={cache_pairs_checked} native_pairs={cache_native_pairs} variant_pairs={cache_variant_pairs} failures={} roots={:?} failure_samples={:#?}",
                cache_failures.len(),
                pose_cache_roots(),
                cache_failures.iter().take(8).collect::<Vec<_>>()
            );
            assert_eq!(cache_native_pairs, 11);
            assert_eq!(cache_variant_pairs, 354);
            assert!(cache_failures.is_empty(), "{cache_failures:#?}");
            assert_eq!(cache_pairs_checked, exact_script_pose_pairs.len());
        }

        assert_eq!(files, 179);
        assert_eq!(clips, 1744);
        assert_eq!(skins, 234);
        assert_eq!(rigid_bone_attachments, 41);
        assert_eq!(animdatum_records, 669);
        assert_eq!(native_bound_clips, 1390);
        if std::env::var_os("ROBOTS_ANIMATION_POSE_CACHE").is_some() {
            eprintln!(
                "Robots native RAPCV003 coverage: loaded={native_pose_caches_loaded}/{native_bound_clips} error_samples={native_pose_cache_errors:#?}"
            );
            assert_eq!(native_pose_caches_loaded, native_bound_clips);
            assert!(
                native_pose_cache_errors.is_empty(),
                "{native_pose_cache_errors:#?}"
            );
        }
        assert!(motion_failures.is_empty(), "{motion_failures:#?}");
        assert!(skin_failures.is_empty(), "{skin_failures:#?}");
        assert!(bind_pose_failures.is_empty(), "{bind_pose_failures:#?}");
    }
}
