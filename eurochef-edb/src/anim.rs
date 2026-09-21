use std::{
    fmt,
    io::{Read, Seek},
};

use binrw::{binrw, BinRead, BinReaderExt, BinResult, BinWrite, VecArgs};
use serde::Serialize;

use crate::{
    array::EXRelArray,
    common::{
        EXGeoAnimHeader, EXGeoAnimModeHeader, EXGeoAnimSetHeader, EXRelPtr, EXRelPtr16, EXVector,
        EXVector3,
    },
};

/// One native Robots PC-v248 AnimMode transition entry. `0x004F2F96` walks
/// `EXGeoAnimModeHeader.common._ptr` as an array of these 8-byte entries and
/// compares `new_mode_index` with the local index returned by `0x005048A5`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RobotsV248AnimModeTransition {
    pub new_mode_index: u32,
    pub control_list_offset: u64,
    pub controls: Vec<RobotsV248AnimModeControl>,
}

/// Fixed native prefix of one AnimMode control record. `0x004F2F96` dispatches
/// opcodes `0x0C000002..0x0C000006`. Opcodes 2/3 additionally use the u16 mask
/// at `+0x0A` through `0x004F30F9`; their packed DWORD values start at `+0x0C`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RobotsV248AnimModeControl {
    pub offset: u64,
    pub opcode: u32,
    pub resource_key: u32,
    pub raw_word_08: u16,
    pub sparse_mask: u16,
    pub sparse_values: Vec<u32>,
}

/// One native Robots PC-v248 AnimSet group consumed by `0x004F2D0B` and
/// `0x004F2DFC`. The serialized group is an 8-byte header followed by
/// `contribution_count` fixed 12-byte contribution records.
#[derive(Debug, Clone, PartialEq)]
pub struct RobotsV248AnimSetGroup {
    pub layer: u16,
    pub contribution_count: i16,
    pub weight: f32,
    pub contributions: Vec<RobotsV248AnimSetContribution>,
}

/// One fixed 12-byte AnimSet contribution. Native `0x004F2D0B` passes
/// `resource_hashcode` into `0x004F2A67`; the remaining lanes stay raw until
/// their downstream consumers are named instruction-by-instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RobotsV248AnimSetContribution {
    pub resource_hashcode: u32,
    pub raw_u16_04: u16,
    pub raw_u16_06: u16,
    pub raw_u32_08: u32,
}

impl EXGeoAnimModeHeader {
    /// Read the exact PC-v248 transition list consumed by `0x004F2F96`.
    ///
    /// Native layout:
    /// - `common.address` / fixed-up `common._ptr`: transition array base;
    /// - `num_anim_modes`: number of 8-byte transition entries;
    /// - entry `+0x00`: local `new_mode_index`;
    /// - entry `+0x04`: self-relative pointer to a control-list;
    /// - control-list: `u32 count` followed by `count` self-relative pointers.
    pub fn read_robots_v248_transitions<R: Read + Seek>(
        &self,
        reader: &mut R,
        endian: binrw::Endian,
    ) -> BinResult<Vec<RobotsV248AnimModeTransition>> {
        let saved = reader.stream_position()?;
        let mut transitions = Vec::with_capacity(self.num_anim_modes as usize);

        for index in 0..self.num_anim_modes as u64 {
            let entry_offset = self.common.address as u64 + index * 8;
            reader.seek(std::io::SeekFrom::Start(entry_offset))?;
            let new_mode_index: u32 = reader.read_type(endian)?;
            let rel: i32 = reader.read_type(endian)?;
            let control_list_offset = if rel == 0 {
                0
            } else {
                (entry_offset as i64 + 4 + i64::from(rel)) as u64
            };

            let mut controls = Vec::new();
            if control_list_offset != 0 {
                reader.seek(std::io::SeekFrom::Start(control_list_offset))?;
                let control_count: u32 = reader.read_type(endian)?;
                for control_index in 0..control_count as u64 {
                    let pointer_offset = control_list_offset + 4 + control_index * 4;
                    reader.seek(std::io::SeekFrom::Start(pointer_offset))?;
                    let control_rel: i32 = reader.read_type(endian)?;
                    if control_rel == 0 {
                        continue;
                    }
                    let control_offset = (pointer_offset as i64 + i64::from(control_rel)) as u64;
                    reader.seek(std::io::SeekFrom::Start(control_offset))?;
                    let opcode: u32 = reader.read_type(endian)?;
                    let resource_key: u32 = reader.read_type(endian)?;
                    let raw_word_08: u16 = reader.read_type(endian)?;
                    let sparse_mask: u16 = reader.read_type(endian)?;
                    let sparse_values = if matches!(opcode, 0x0C00_0002 | 0x0C00_0003) {
                        let mut values = Vec::with_capacity(sparse_mask.count_ones() as usize);
                        for _ in 0..sparse_mask.count_ones() {
                            values.push(reader.read_type(endian)?);
                        }
                        values
                    } else {
                        Vec::new()
                    };
                    controls.push(RobotsV248AnimModeControl {
                        offset: control_offset,
                        opcode,
                        resource_key,
                        raw_word_08,
                        sparse_mask,
                        sparse_values,
                    });
                }
            }

            transitions.push(RobotsV248AnimModeTransition {
                new_mode_index,
                control_list_offset,
                controls,
            });
        }

        reader.seek(std::io::SeekFrom::Start(saved))?;
        Ok(transitions)
    }
}

impl EXGeoAnimSetHeader {
    /// Read the exact PC-v248 AnimSet group stream consumed by native
    /// `0x004F2D0B` / `0x004F2DFC`.
    pub fn read_robots_v248_groups<R: Read + Seek>(
        &self,
        reader: &mut R,
        endian: binrw::Endian,
    ) -> BinResult<Vec<RobotsV248AnimSetGroup>> {
        let saved = reader.stream_position()?;
        let mut groups = Vec::with_capacity(self.num_anim_sets as usize);
        let mut group_offset = self.common.address as u64;

        for _ in 0..self.num_anim_sets {
            reader.seek(std::io::SeekFrom::Start(group_offset))?;
            let layer: u16 = reader.read_type(endian)?;
            let contribution_count: i16 = reader.read_type(endian)?;
            let weight: f32 = reader.read_type(endian)?;
            if contribution_count < 0 {
                reader.seek(std::io::SeekFrom::Start(saved))?;
                return Err(binrw::Error::AssertFail {
                    pos: group_offset,
                    message: format!(
                        "negative Robots v248 AnimSet contribution count {contribution_count}"
                    ),
                });
            }

            let mut contributions = Vec::with_capacity(contribution_count as usize);
            for _ in 0..contribution_count {
                contributions.push(RobotsV248AnimSetContribution {
                    resource_hashcode: reader.read_type(endian)?,
                    raw_u16_04: reader.read_type(endian)?,
                    raw_u16_06: reader.read_type(endian)?,
                    raw_u32_08: reader.read_type(endian)?,
                });
            }

            groups.push(RobotsV248AnimSetGroup {
                layer,
                contribution_count,
                weight,
                contributions,
            });
            group_offset += 8 + contribution_count as u64 * 12;
        }

        reader.seek(std::io::SeekFrom::Start(saved))?;
        Ok(groups)
    }
}

/// Serialized Robots PC v248 `EXGeoAnim` object at
/// `EXGeoAnimHeader.common.address` (native descriptor `0x005F3D68`, size
/// `0x9C`, class name `EXGeoAnim`). This is deliberately separate from
/// `motiondata_info_addr`: the latter points at the compressed motion stream,
/// while the object below stores the dimensions/masks/base pose consumed by
/// the native sampler before it enters `0x00567B30`.
///
/// Only fields with direct native/corpus evidence are named. The remaining
/// lanes stay raw so later RE can refine them without turning a plausible bit
/// pattern into an API promise.
#[binrw]
#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct RobotsV248EXGeoAnim {
    pub raw_00: u32,
    /// Serialized `+0x04`. Loader `0x0050268A` replaces runtime `+0x04/+0x08`
    /// with loaded-blob state, so this value is intentionally not treated as a
    /// relative pointer.
    pub raw_04: u32,
    pub raw_08: u32,
    pub raw_byte_0c: u8,
    /// Native frame-cache code also reads this byte, but its exact role is not
    /// yet proven.
    pub raw_byte_0d: u8,
    /// Native `0x00567B30` reads this exact u16 from `EXGeoAnim+0x0E` as the
    /// animation frame bound/count.
    pub frame_count: u16,
    pub raw_10: u32,
    /// Native `0x00503412` passes this byte from `EXGeoAnim+0x14` as the bone
    /// count. It matches the bound AnimSkin bone count across the native oracle
    /// corpus; each bone then contributes three or six compressed scalar channels.
    pub bone_count: u8,
    pub raw_byte_15: u8,
    /// Native frame-cache/morph consumers use `EXGeoAnim+0x16` as the scalar
    /// channel count. 1,750 RAPCV003 oracle clips match this serialized value
    /// exactly.
    pub scalar_channel_count: u16,
    #[serde(skip)]
    pub raw_18_5f: [u8; 0x48],
    /// Four per-bone translation-channel mask DWORDs copied by `0x00503412`
    /// before it enters native skeletal decoder `0x00567B30`. A clear bit still
    /// has the three compressed quaternion channels; a set bit adds three
    /// translation channels for that bone.
    pub translation_channel_masks: [u32; 4],
    pub raw_70_8b: [u8; 0x1c],
    /// Base/root translation added after the skeletal decoder returns.
    pub base_translation: EXVector3,
    pub raw_98_9b: [u8; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub struct RobotsV248SkeletalBoneFrame {
    /// Raw translation delta emitted by `0x00567B30` for bones selected by
    /// `EXGeoAnim+0x60..+0x6C`. The native pose assembler applies final
    /// base/bind placement afterwards.
    pub translation_delta: Option<EXVector3>,
    /// Native reconstructed quaternion `(x, y, z, w)`.
    pub rotation: [f32; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub struct RobotsV248SkeletalFrame {
    pub frame_index: u16,
    pub bones: Vec<RobotsV248SkeletalBoneFrame>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RobotsV248MotionBlockTable {
    pub block_stride: usize,
    pub end_frames: Vec<u16>,
}

/// Serialized EXGeoAnim packed-directory ID1 transform consumed by native
/// root-pose correction `0x00503AD8 -> 0x0050273F`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RobotsV248RootCorrectionTransform {
    pub translation: EXVector3,
    pub rotation: [f32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RobotsV248MotionBlockSelection {
    stream_offset: usize,
    start_frame: u16,
    frame_count: u16,
    relative_frame: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RobotsV248SkeletalBonePose {
    pub position: EXVector3,
    pub rotation: [f32; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub struct RobotsV248SkeletalPose {
    pub frame_index: u16,
    pub bones: Vec<RobotsV248SkeletalBonePose>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RobotsV248MotionDecodeError {
    message: String,
}

impl RobotsV248MotionDecodeError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for RobotsV248MotionDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for RobotsV248MotionDecodeError {}

impl RobotsV248MotionBlockTable {
    fn select(
        &self,
        frame_index: u16,
        total_frame_count: u16,
        motion_len: usize,
    ) -> Result<RobotsV248MotionBlockSelection, RobotsV248MotionDecodeError> {
        if self.block_stride == 0 || self.end_frames.is_empty() {
            return Err(RobotsV248MotionDecodeError::new(
                "motion block table has no usable blocks",
            ));
        }
        let last_frame = total_frame_count.saturating_sub(1);
        let frame = frame_index.min(last_frame);
        let mut start_frame = 0u16;
        for (block_index, &serialized_end) in self.end_frames.iter().enumerate() {
            let serialized_end_frame = serialized_end.min(last_frame);
            if frame <= serialized_end_frame {
                // Native 0x005563F4 decodes one overlap frame from every
                // non-final block so frame interpolation can cross the block
                // boundary without a second pose lookup.
                let decode_end_frame = if serialized_end_frame < last_frame {
                    serialized_end_frame.saturating_add(1)
                } else {
                    serialized_end_frame
                };
                let block_frame_count = decode_end_frame
                    .saturating_sub(start_frame)
                    .saturating_add(1);
                let stream_offset =
                    block_index.checked_mul(self.block_stride).ok_or_else(|| {
                        RobotsV248MotionDecodeError::new("motion block offset overflow")
                    })?;
                if stream_offset >= motion_len {
                    return Err(RobotsV248MotionDecodeError::new(format!(
                        "motion block {block_index} starts at 0x{stream_offset:X} outside {motion_len}-byte stream"
                    )));
                }
                return Ok(RobotsV248MotionBlockSelection {
                    stream_offset,
                    start_frame,
                    frame_count: block_frame_count,
                    relative_frame: frame.saturating_sub(start_frame),
                });
            }
            start_frame = serialized_end_frame.saturating_add(1);
        }
        Err(RobotsV248MotionDecodeError::new(format!(
            "motion block table does not cover frame {frame_index}"
        )))
    }
}

fn robots_v248_quat_mul(left: [f32; 4], right: [f32; 4]) -> [f32; 4] {
    let [lx, ly, lz, lw] = left;
    let [rx, ry, rz, rw] = right;
    [
        lw * rx + lx * rw + ly * rz - lz * ry,
        lw * ry - lx * rz + ly * rw + lz * rx,
        lw * rz + lx * ry - ly * rx + lz * rw,
        lw * rw - lx * rx - ly * ry - lz * rz,
    ]
}

fn robots_v248_axis_quat(axis: usize, angle: f32) -> [f32; 4] {
    let half = angle * 0.5;
    let (sin, cos) = half.sin_cos();
    match axis {
        0 => [sin, 0.0, 0.0, cos],
        1 => [0.0, sin, 0.0, cos],
        _ => [0.0, 0.0, sin, cos],
    }
}

fn robots_v248_quat_rotate_vector(quaternion: [f32; 4], vector: EXVector3) -> EXVector3 {
    let [x, y, z, w] = quaternion;
    let [vx, vy, vz] = vector;
    let tx = 2.0 * (y * vz - z * vy);
    let ty = 2.0 * (z * vx - x * vz);
    let tz = 2.0 * (x * vy - y * vx);
    [
        vx + w * tx + (y * tz - z * ty),
        vy + w * ty + (z * tx - x * tz),
        vz + w * tz + (x * ty - y * tx),
    ]
}

fn robots_v248_quat_to_fixed_euler(
    quaternion: [f32; 4],
    order: u8,
) -> Result<EXVector3, RobotsV248MotionDecodeError> {
    let [x, y, z, w] = quaternion;
    let xx = x * x;
    let yy = y * y;
    let zz = z * z;
    let xy = x * y;
    let xz = x * z;
    let yz = y * z;
    let xw = x * w;
    let yw = y * w;
    let zw = z * w;
    let m00 = 1.0 - 2.0 * (yy + zz);
    let m02 = 2.0 * (xz + yw);
    let m10 = 2.0 * (xy + zw);
    let m11 = 1.0 - 2.0 * (xx + zz);
    let m12 = 2.0 * (yz - xw);
    let m20 = 2.0 * (xz - yw);
    let m21 = 2.0 * (yz + xw);
    let m22 = 1.0 - 2.0 * (xx + yy);
    let clamp_unit = |value: f32| value.clamp(-1.0, 1.0);

    match order {
        // Native order 4 == fixed XYZ slots with ZXY composition.
        4 => Ok([clamp_unit(-m12).asin(), m02.atan2(m22), m10.atan2(m11)]),
        // Native order 5 == fixed XYZ slots with XZY composition.
        5 => Ok([(-m12).atan2(m11), (-m20).atan2(m00), clamp_unit(m10).asin()]),
        // Native order 7 == fixed XYZ slots with XYZ composition.
        7 => Ok([m21.atan2(m22), clamp_unit(-m20).asin(), m10.atan2(m00)]),
        _ => Err(RobotsV248MotionDecodeError::new(format!(
            "unsupported Robots root-correction Euler order {order}"
        ))),
    }
}

fn robots_v248_fixed_euler_to_quat(
    euler: EXVector3,
    order: u8,
) -> Result<[f32; 4], RobotsV248MotionDecodeError> {
    let qx = robots_v248_axis_quat(0, euler[0]);
    let qy = robots_v248_axis_quat(1, euler[1]);
    let qz = robots_v248_axis_quat(2, euler[2]);
    match order {
        4 => Ok(robots_v248_quat_mul(robots_v248_quat_mul(qy, qx), qz)),
        5 => Ok(robots_v248_quat_mul(robots_v248_quat_mul(qy, qz), qx)),
        7 => Ok(robots_v248_quat_mul(robots_v248_quat_mul(qz, qy), qx)),
        _ => Err(RobotsV248MotionDecodeError::new(format!(
            "unsupported Robots root-correction Euler order {order}"
        ))),
    }
}

impl RobotsV248EXGeoAnim {
    pub fn has_translation_channel(&self, bone_index: usize) -> bool {
        let word_index = bone_index / 32;
        let bit_index = bone_index % 32;
        self.translation_channel_masks
            .get(word_index)
            .is_some_and(|word| (word & (1u32 << bit_index)) != 0)
    }

    /// Assemble the decoded frame for the normal same-AnimSkin path recovered
    /// from native pose assembler `0x004FDB2A`. Rotation-only bones retain the
    /// AnimSkin relative bind position. Translation-channel bones use the
    /// decoded sparse translation directly; bone 0 first receives
    /// `EXGeoAnim.base_translation`, matching `0x00503412`, then the correction
    /// key at `EXGeoAnim+0x10` is applied exactly like cache refresh `0x00503AD8`.
    /// Cross-skin retargeting applies an additional root-Y scale and is
    /// intentionally not folded into this same-skin helper.
    pub fn assemble_same_skin_pose(
        &self,
        frame: &RobotsV248SkeletalFrame,
        relative_bind_positions: &[EXVector],
    ) -> Result<RobotsV248SkeletalPose, RobotsV248MotionDecodeError> {
        self.assemble_same_skin_pose_with_root_correction(frame, relative_bind_positions, None)
    }

    pub fn assemble_same_skin_pose_with_root_correction(
        &self,
        frame: &RobotsV248SkeletalFrame,
        relative_bind_positions: &[EXVector],
        root_correction: Option<&RobotsV248RootCorrectionTransform>,
    ) -> Result<RobotsV248SkeletalPose, RobotsV248MotionDecodeError> {
        let bone_count = usize::from(self.bone_count);
        if frame.bones.len() != bone_count {
            return Err(RobotsV248MotionDecodeError::new(format!(
                "decoded frame has {} bones, expected {bone_count}",
                frame.bones.len()
            )));
        }
        if relative_bind_positions.len() < bone_count {
            return Err(RobotsV248MotionDecodeError::new(format!(
                "AnimSkin has {} relative bind positions, expected at least {bone_count}",
                relative_bind_positions.len()
            )));
        }

        let mut bones = Vec::with_capacity(bone_count);
        for (bone_index, decoded) in frame.bones.iter().enumerate() {
            let mut position = match decoded.translation_delta {
                Some(mut translation) => {
                    if bone_index == 0 {
                        for (value, base) in translation.iter_mut().zip(self.base_translation) {
                            *value += base;
                        }
                    }
                    translation
                }
                None => {
                    let bind = relative_bind_positions[bone_index];
                    [bind[0], bind[1], bind[2]]
                }
            };
            let mut rotation = decoded.rotation;
            if bone_index == 0 {
                self.apply_root_cache_correction(&mut position, &mut rotation, root_correction)?;
            }
            bones.push(RobotsV248SkeletalBonePose { position, rotation });
        }

        Ok(RobotsV248SkeletalPose {
            frame_index: frame.frame_index,
            bones,
        })
    }

    /// Extract the native root-motion transform used by
    /// `0x005035A7/0x0050376D -> 0x005039D5` from decoded bone 0.
    ///
    /// This is intentionally the complement of `apply_root_cache_correction`:
    /// pose assembly removes the channels owned by root motion from the rendered
    /// skeleton, while `0x005039D5` keeps exactly those channels for locomotion.
    /// Translation bits `0x100/0x200/0x400` keep X/Y/Z and rotation bits
    /// `0x800/0x1000/0x2000` keep the corresponding fixed-Euler axes.
    pub fn extract_root_motion_transform(
        &self,
        frame: &RobotsV248SkeletalFrame,
    ) -> Result<RobotsV248SkeletalBonePose, RobotsV248MotionDecodeError> {
        let root = frame
            .bones
            .first()
            .ok_or_else(|| RobotsV248MotionDecodeError::new("decoded frame has no root bone"))?;
        let mut position = root.translation_delta.unwrap_or([0.0; 3]);
        for (value, base) in position.iter_mut().zip(self.base_translation) {
            *value += base;
        }
        let mut rotation = root.rotation;
        let extraction_key = self.raw_10 & 0x7f00;

        let translation_mask = extraction_key & 0x0700;
        if translation_mask != 0x0700 {
            if translation_mask == 0 {
                position = [0.0; 3];
            } else {
                if translation_mask & 0x0100 == 0 {
                    position[0] = 0.0;
                }
                if translation_mask & 0x0200 == 0 {
                    position[1] = 0.0;
                }
                if translation_mask & 0x0400 == 0 {
                    position[2] = 0.0;
                }
            }
        }

        let rotation_mask = extraction_key & 0x3800;
        if rotation_mask != 0x3800 {
            if rotation_mask == 0 {
                rotation = [0.0, 0.0, 0.0, 1.0];
            } else {
                const NATIVE_EULER_ORDERS: [u8; 8] = [4, 4, 4, 4, 7, 3, 5, 4];
                let order = NATIVE_EULER_ORDERS[((extraction_key >> 11) & 7) as usize];
                let mut euler = robots_v248_quat_to_fixed_euler(rotation, order)?;
                if rotation_mask & 0x0800 == 0 {
                    euler[0] = 0.0;
                }
                if rotation_mask & 0x1000 == 0 {
                    euler[1] = 0.0;
                }
                if rotation_mask & 0x2000 == 0 {
                    euler[2] = 0.0;
                }
                rotation = robots_v248_fixed_euler_to_quat(euler, order)?;
            }
        }

        // Native extraction can additionally compose packed-directory ID1 when
        // bit 0x4000 is present. No promoted gameplay consumer currently needs
        // that branch; fail closed instead of silently inventing mode-0
        // `0x0050273F` semantics.
        if extraction_key & 0x4000 != 0 {
            return Err(RobotsV248MotionDecodeError::new(
                "root-motion extraction with packed correction bit 0x4000 is not proven",
            ));
        }

        Ok(RobotsV248SkeletalBonePose { position, rotation })
    }

    fn apply_root_cache_correction(
        &self,
        position: &mut EXVector3,
        rotation: &mut [f32; 4],
        root_correction: Option<&RobotsV248RootCorrectionTransform>,
    ) -> Result<(), RobotsV248MotionDecodeError> {
        let correction_key = self.raw_10;
        let translation_mask = correction_key & 0x0700;
        if translation_mask != 0 {
            if translation_mask == 0x0700 {
                *position = [0.0; 3];
            } else {
                if translation_mask & 0x0100 != 0 {
                    position[0] = 0.0;
                }
                if translation_mask & 0x0200 != 0 {
                    position[1] = 0.0;
                }
                if translation_mask & 0x0400 != 0 {
                    position[2] = 0.0;
                }
            }
        }

        let rotation_mask = correction_key & 0x3800;
        if rotation_mask != 0 {
            if rotation_mask == 0x3800 {
                *rotation = [0.0, 0.0, 0.0, 1.0];
            } else {
                const NATIVE_EULER_ORDERS: [u8; 8] = [4, 4, 4, 4, 7, 3, 5, 4];
                let order = NATIVE_EULER_ORDERS[((correction_key >> 11) & 7) as usize];
                let mut euler = robots_v248_quat_to_fixed_euler(*rotation, order)?;
                if rotation_mask & 0x0800 != 0 {
                    euler[0] = 0.0;
                }
                if rotation_mask & 0x1000 != 0 {
                    euler[1] = 0.0;
                }
                if rotation_mask & 0x2000 != 0 {
                    euler[2] = 0.0;
                }
                *rotation = robots_v248_fixed_euler_to_quat(euler, order)?;
            }
        }

        if correction_key & 0x4000 != 0 {
            if let Some(root_correction) = root_correction {
                let mut correction_translation = root_correction.translation;
                if correction_key & 0x0100 == 0 {
                    correction_translation[0] = 0.0;
                }
                if correction_key & 0x0200 == 0 {
                    correction_translation[1] = 0.0;
                }
                if correction_key & 0x0400 == 0 {
                    correction_translation[2] = 0.0;
                }
                let rotated = robots_v248_quat_rotate_vector(root_correction.rotation, *position);
                *position = [
                    rotated[0] + correction_translation[0],
                    rotated[1] + correction_translation[1],
                    rotated[2] + correction_translation[2],
                ];
                *rotation = robots_v248_quat_mul(root_correction.rotation, *rotation);
            }
        }
        Ok(())
    }

    /// Decode one integer skeletal frame from the serialized Robots PC v248
    /// compressed motion stream. This mirrors `Robots.exe` `0x00567B30`:
    /// self-delimiting channel descriptors select sparse polynomial
    /// coefficients `c0..c5`, evaluated at `t = frame / frame_count`; every bone
    /// has three quaternion channels and translation-mask bones have three
    /// additional translation channels.
    pub fn decode_skeletal_frame(
        &self,
        motion: &[u8],
        frame_index: u16,
    ) -> Result<RobotsV248SkeletalFrame, RobotsV248MotionDecodeError> {
        self.decode_skeletal_frame_from_selected_block(
            motion,
            frame_index,
            0,
            self.frame_count,
            frame_index,
        )
    }

    pub fn decode_skeletal_frame_with_block_table(
        &self,
        motion: &[u8],
        frame_index: u16,
        block_table: &RobotsV248MotionBlockTable,
    ) -> Result<RobotsV248SkeletalFrame, RobotsV248MotionDecodeError> {
        if self.frame_count == 0 {
            return Err(RobotsV248MotionDecodeError::new(
                "EXGeoAnim has zero frame_count",
            ));
        }
        if frame_index >= self.frame_count {
            return Err(RobotsV248MotionDecodeError::new(format!(
                "frame {frame_index} outside EXGeoAnim frame_count {}",
                self.frame_count
            )));
        }
        let selection = block_table.select(frame_index, self.frame_count, motion.len())?;
        self.decode_skeletal_frame_from_selected_block(
            &motion[selection.stream_offset..],
            frame_index,
            selection.start_frame,
            selection.frame_count,
            selection.relative_frame,
        )
    }

    fn decode_skeletal_frame_from_selected_block(
        &self,
        motion: &[u8],
        frame_index: u16,
        block_start_frame: u16,
        block_frame_count: u16,
        block_relative_frame: u16,
    ) -> Result<RobotsV248SkeletalFrame, RobotsV248MotionDecodeError> {
        if self.frame_count == 0 {
            return Err(RobotsV248MotionDecodeError::new(
                "EXGeoAnim has zero frame_count",
            ));
        }
        if frame_index >= self.frame_count {
            return Err(RobotsV248MotionDecodeError::new(format!(
                "frame {frame_index} outside EXGeoAnim frame_count {}",
                self.frame_count
            )));
        }
        if motion.len() < 2 {
            return Err(RobotsV248MotionDecodeError::new(
                "compressed motion stream is shorter than its coefficient-table pointer",
            ));
        }

        let mut descriptor_offset = 2usize;
        let mut coefficient_offset = usize::from(read_motion_u16(motion, 0)?)
            .checked_mul(4)
            .ok_or_else(|| RobotsV248MotionDecodeError::new("coefficient offset overflow"))?;
        if coefficient_offset > motion.len() {
            return Err(RobotsV248MotionDecodeError::new(format!(
                "coefficient table starts at 0x{coefficient_offset:X} outside {}-byte motion stream",
                motion.len()
            )));
        }

        let frame_t = f32::from(frame_index) / f32::from(self.frame_count);
        let frame_t2 = frame_t * frame_t;
        let frame_t3 = frame_t2 * frame_t;
        let frame_t4 = frame_t3 * frame_t;
        let frame_t5 = frame_t4 * frame_t;
        let powers = [1.0f32, frame_t, frame_t2, frame_t3, frame_t4, frame_t5];
        let mut bones = Vec::with_capacity(usize::from(self.bone_count));

        for bone_index in 0..usize::from(self.bone_count) {
            let has_translation = self.has_translation_channel(bone_index);
            let first_channel_type = if has_translation { 5usize } else { 2usize };
            let mut values = [0.0f32; 6];

            for channel_type in (0..=first_channel_type).rev() {
                let (value, next_descriptor, next_coefficient) = decode_motion_channel(
                    motion,
                    descriptor_offset,
                    coefficient_offset,
                    block_frame_count,
                    block_start_frame,
                    block_relative_frame,
                    frame_index,
                    &powers,
                )?;
                values[channel_type] = value;
                descriptor_offset = next_descriptor;
                coefficient_offset = next_coefficient;
            }

            let translation_delta = has_translation.then_some([values[5], values[4], values[3]]);
            let qx = values[2];
            let qy = values[1];
            let encoded_z = values[0];
            let (qz, w_sign) = if encoded_z < 2.0 {
                (encoded_z, 1.0f32)
            } else {
                (encoded_z - 4.0, -1.0f32)
            };
            let remaining = 1.0f32 - qx * qx - qy * qy - qz * qz;
            // Native 0x00567B30 only takes sqrt for a positive remainder. If
            // quantization pushes it below zero, the raw remainder is retained
            // as W instead of turning the pose into NaN or rejecting the frame.
            let qw = if remaining > 0.0 {
                w_sign * remaining.sqrt()
            } else {
                remaining
            };
            bones.push(RobotsV248SkeletalBoneFrame {
                translation_delta,
                rotation: [qx, qy, qz, qw],
            });
        }

        Ok(RobotsV248SkeletalFrame { frame_index, bones })
    }

    /// Decode the integer-frame scalar/morph channels emitted by native
    /// `0x00503F80 -> 0x0056A045`. The scalar stream at serialized
    /// `EXGeoAnim+0x34` uses the same sparse polynomial segment/checkpoint
    /// encoding as skeletal channels, but has one independent channel per
    /// `EXGeoAnim+0x16` scalar and no quaternion reconstruction.
    pub fn decode_scalar_frame(
        &self,
        scalar_stream: &[u8],
        frame_index: u16,
    ) -> Result<Vec<f32>, RobotsV248MotionDecodeError> {
        if self.scalar_channel_count == 0 {
            return Ok(Vec::new());
        }
        if self.frame_count == 0 {
            return Err(RobotsV248MotionDecodeError::new(
                "EXGeoAnim has zero frame_count",
            ));
        }
        if frame_index >= self.frame_count {
            return Err(RobotsV248MotionDecodeError::new(format!(
                "scalar frame {frame_index} outside EXGeoAnim frame_count {}",
                self.frame_count
            )));
        }
        if scalar_stream.len() < 2 {
            return Err(RobotsV248MotionDecodeError::new(
                "compressed scalar stream is shorter than its coefficient-table pointer",
            ));
        }

        let mut descriptor_offset = 2usize;
        let mut coefficient_offset = usize::from(read_motion_u16(scalar_stream, 0)?)
            .checked_mul(4)
            .ok_or_else(|| {
                RobotsV248MotionDecodeError::new("scalar coefficient offset overflow")
            })?;
        if coefficient_offset > scalar_stream.len() {
            return Err(RobotsV248MotionDecodeError::new(format!(
                "scalar coefficient table starts at 0x{coefficient_offset:X} outside {}-byte stream",
                scalar_stream.len()
            )));
        }

        let frame_t = f32::from(frame_index) / f32::from(self.frame_count);
        let frame_t2 = frame_t * frame_t;
        let frame_t3 = frame_t2 * frame_t;
        let frame_t4 = frame_t3 * frame_t;
        let frame_t5 = frame_t4 * frame_t;
        let powers = [1.0f32, frame_t, frame_t2, frame_t3, frame_t4, frame_t5];
        let mut scalars = Vec::with_capacity(usize::from(self.scalar_channel_count));

        for _ in 0..usize::from(self.scalar_channel_count) {
            let (value, next_descriptor, next_coefficient) = decode_motion_channel(
                scalar_stream,
                descriptor_offset,
                coefficient_offset,
                self.frame_count,
                0,
                frame_index,
                frame_index,
                &powers,
            )?;
            scalars.push(value);
            descriptor_offset = next_descriptor;
            coefficient_offset = next_coefficient;
        }
        Ok(scalars)
    }
}

fn read_motion_u16(motion: &[u8], offset: usize) -> Result<u16, RobotsV248MotionDecodeError> {
    let end = offset
        .checked_add(2)
        .ok_or_else(|| RobotsV248MotionDecodeError::new("u16 read offset overflow"))?;
    let bytes = motion.get(offset..end).ok_or_else(|| {
        RobotsV248MotionDecodeError::new(format!(
            "u16 read at 0x{offset:X} exceeds {}-byte motion stream",
            motion.len()
        ))
    })?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_motion_f32(motion: &[u8], offset: usize) -> Result<f32, RobotsV248MotionDecodeError> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| RobotsV248MotionDecodeError::new("f32 read offset overflow"))?;
    let bytes = motion.get(offset..end).ok_or_else(|| {
        RobotsV248MotionDecodeError::new(format!(
            "f32 read at 0x{offset:X} exceeds {}-byte motion stream",
            motion.len()
        ))
    })?;
    Ok(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn decode_motion_channel(
    motion: &[u8],
    descriptor_offset: usize,
    coefficient_offset: usize,
    context_frame_count: u16,
    context_start_frame: u16,
    context_relative_frame: u16,
    frame_index: u16,
    powers: &[f32; 6],
) -> Result<(f32, usize, usize), RobotsV248MotionDecodeError> {
    let coefficient_words = usize::from(read_motion_u16(motion, descriptor_offset)?);
    let descriptor_words = usize::from(read_motion_u16(
        motion,
        descriptor_offset
            .checked_add(2)
            .ok_or_else(|| RobotsV248MotionDecodeError::new("descriptor header overflow"))?,
    )?);
    let coefficient_span = coefficient_words
        .checked_mul(4)
        .ok_or_else(|| RobotsV248MotionDecodeError::new("channel coefficient span overflow"))?;
    let descriptor_span = descriptor_words
        .checked_mul(2)
        .ok_or_else(|| RobotsV248MotionDecodeError::new("channel descriptor span overflow"))?;
    let next_coefficient = coefficient_offset
        .checked_add(coefficient_span)
        .ok_or_else(|| RobotsV248MotionDecodeError::new("channel coefficient end overflow"))?;
    let next_descriptor = descriptor_offset
        .checked_add(descriptor_span)
        .ok_or_else(|| RobotsV248MotionDecodeError::new("channel descriptor end overflow"))?;
    let checkpoint_count = usize::from(context_frame_count.saturating_sub(1) >> 7);
    let minimum_descriptor_words = 3usize
        .checked_add(checkpoint_count.saturating_mul(3))
        .ok_or_else(|| RobotsV248MotionDecodeError::new("checkpoint header size overflow"))?;
    if descriptor_words < minimum_descriptor_words
        || next_descriptor > motion.len()
        || next_coefficient > motion.len()
    {
        return Err(RobotsV248MotionDecodeError::new(format!(
            "invalid compressed channel span desc=0x{descriptor_offset:X}..0x{next_descriptor:X} coeff=0x{coefficient_offset:X}..0x{next_coefficient:X}"
        )));
    }

    let block_index = usize::from(context_relative_frame >> 7);
    if block_index > checkpoint_count {
        return Err(RobotsV248MotionDecodeError::new(format!(
            "frame {frame_index} selects checkpoint block {block_index} beyond {checkpoint_count}"
        )));
    }
    let (mut segment_descriptor, mut segment_coefficient, mut segment_end_frame) = if block_index
        == 0
    {
        let descriptor_header_words = 2usize
            .checked_add(checkpoint_count.saturating_mul(3))
            .ok_or_else(|| {
                RobotsV248MotionDecodeError::new("checkpoint descriptor offset overflow")
            })?;
        (
            descriptor_offset + descriptor_header_words * 2,
            coefficient_offset,
            u32::from(context_start_frame),
        )
    } else {
        // Native 0x00567B30 stores one three-u16 checkpoint per 128-frame
        // boundary starting at descriptor word 2:
        //   coefficient f32 skip, accumulated frame, descriptor word skip.
        let checkpoint_word = block_index * 3 - 1;
        let coefficient_skip = usize::from(read_motion_u16(
            motion,
            descriptor_offset + checkpoint_word * 2,
        )?);
        let frame_base = read_motion_u16(motion, descriptor_offset + (checkpoint_word + 1) * 2)?;
        let descriptor_skip = usize::from(read_motion_u16(
            motion,
            descriptor_offset + (checkpoint_word + 2) * 2,
        )?);
        (
            descriptor_offset + descriptor_skip * 2,
            coefficient_offset + coefficient_skip * 4,
            u32::from(frame_base),
        )
    };
    if segment_descriptor >= next_descriptor || segment_coefficient >= motion.len() {
        return Err(RobotsV248MotionDecodeError::new(format!(
            "checkpoint points outside compressed motion desc=0x{segment_descriptor:X} coeff=0x{segment_coefficient:X}"
        )));
    }

    while segment_descriptor < next_descriptor {
        let segment_word = read_motion_u16(motion, segment_descriptor)?;
        let coefficient_mask = segment_word & 0x003f;
        segment_end_frame = segment_end_frame.saturating_add(u32::from(segment_word >> 6));

        if u32::from(frame_index) <= segment_end_frame {
            let mut coefficient_cursor = segment_coefficient;
            let mut value = 0.0f32;
            for (coefficient_index, power) in powers.iter().enumerate() {
                if coefficient_mask & (1u16 << coefficient_index) != 0 {
                    let coefficient = read_motion_f32(motion, coefficient_cursor)?;
                    coefficient_cursor = coefficient_cursor.checked_add(4).ok_or_else(|| {
                        RobotsV248MotionDecodeError::new("coefficient cursor overflow")
                    })?;
                    value += coefficient * power;
                }
            }
            // Native 0x00567B30 does not compare the selected coefficient cursor
            // with `next_coefficient`; that value is only the base pointer saved
            // for the following channel. Total motion-stream bounds are still
            // enforced by read_motion_f32().
            return Ok((value, next_descriptor, next_coefficient));
        }

        let present_count = (coefficient_mask as u32).count_ones() as usize;
        let coefficient_skip = present_count
            .checked_mul(4)
            .ok_or_else(|| RobotsV248MotionDecodeError::new("segment coefficient skip overflow"))?;
        segment_coefficient = segment_coefficient
            .checked_add(coefficient_skip)
            .ok_or_else(|| {
                RobotsV248MotionDecodeError::new("segment coefficient cursor overflow")
            })?;
        if segment_coefficient >= motion.len() {
            return Err(RobotsV248MotionDecodeError::new(
                "segment coefficient cursor exceeds compressed motion stream",
            ));
        }
        segment_descriptor = segment_descriptor.checked_add(2).ok_or_else(|| {
            RobotsV248MotionDecodeError::new("segment descriptor cursor overflow")
        })?;
    }

    Err(RobotsV248MotionDecodeError::new(format!(
        "frame {frame_index} has no segment in compressed channel at 0x{descriptor_offset:X}"
    )))
}

fn robots_v248_packed_rel_target(field_offset: u64, packed: u32) -> BinResult<u64> {
    let relative = (packed as i32) >> 8;
    let target = i128::from(field_offset) + i128::from(relative);
    if target < 0 || target > i128::from(u64::MAX) {
        return Err(binrw::Error::AssertFail {
            pos: field_offset,
            message: format!(
                "Robots packed relative pointer {relative} from 0x{field_offset:X} is out of range"
            ),
        });
    }
    Ok(target as u64)
}

impl EXGeoAnimHeader {
    /// Read the serialized Robots PC v248 `EXGeoAnim` body referenced by this
    /// resource-table header while preserving the caller's stream position.
    pub fn read_robots_v248_exgeoanim<R: Read + Seek>(
        &self,
        reader: &mut R,
        endian: binrw::Endian,
    ) -> BinResult<RobotsV248EXGeoAnim> {
        let saved_position = reader.stream_position()?;
        reader.seek(std::io::SeekFrom::Start(u64::from(self.common.address)))?;
        let result = reader.read_type::<RobotsV248EXGeoAnim>(endian);
        reader.seek(std::io::SeekFrom::Start(saved_position))?;
        result
    }

    /// Read packed-directory ID1, the root correction transform consumed by
    /// native cache refresh `0x00503AD8 -> 0x0050273F` when correction-key bit
    /// `0x4000` is set. The record begins with XYZ translation at +0x00 and the
    /// quaternion at +0x10; +0x0C is not consumed by that native path.
    pub fn read_robots_v248_root_correction_transform<R: Read + Seek>(
        &self,
        reader: &mut R,
        endian: binrw::Endian,
    ) -> BinResult<Option<RobotsV248RootCorrectionTransform>> {
        let saved_position = reader.stream_position()?;
        let result = (|| {
            let object_offset = u64::from(self.common.address);
            let directory_field = object_offset + 0x38;
            reader.seek(std::io::SeekFrom::Start(directory_field))?;
            let packed_directory = reader.read_type::<u32>(endian)?;
            let flags = (packed_directory & 0xff) as u8;
            const ROOT_CORRECTION_ID: u8 = 1;
            if flags & (1 << ROOT_CORRECTION_ID) == 0 {
                return Ok(None);
            }

            let directory_base = robots_v248_packed_rel_target(directory_field, packed_directory)?;
            let lower_mask = (1u8 << ROOT_CORRECTION_ID) - 1;
            let rank = (flags & lower_mask).count_ones() as u64;
            let entry_offset =
                directory_base
                    .checked_add(rank * 4)
                    .ok_or_else(|| binrw::Error::AssertFail {
                        pos: directory_field,
                        message: "Robots EXGeoAnim packed ID1 entry overflow".to_string(),
                    })?;
            reader.seek(std::io::SeekFrom::Start(entry_offset))?;
            let packed_entry = reader.read_type::<u32>(endian)?;
            let correction_offset = robots_v248_packed_rel_target(entry_offset, packed_entry)?;
            reader.seek(std::io::SeekFrom::Start(correction_offset))?;
            let translation = [
                reader.read_type::<f32>(endian)?,
                reader.read_type::<f32>(endian)?,
                reader.read_type::<f32>(endian)?,
            ];
            let _raw_0c = reader.read_type::<u32>(endian)?;
            let rotation = [
                reader.read_type::<f32>(endian)?,
                reader.read_type::<f32>(endian)?,
                reader.read_type::<f32>(endian)?,
                reader.read_type::<f32>(endian)?,
            ];
            Ok(Some(RobotsV248RootCorrectionTransform {
                translation,
                rotation,
            }))
        })();
        reader.seek(std::io::SeekFrom::Start(saved_position))?;
        result
    }

    /// Read the native ID5 motion-block directory used by `0x005563F4` before
    /// `0x00567B30`. The packed directory at serialized EXGeoAnim+0x38 uses the
    /// exact `0x00504D0B` low-byte bitmap + signed-24-bit relative-pointer
    /// encoding. The pointed table starts with the unaligned block byte size as
    /// two u16 halves, followed by inclusive global end-frame values.
    pub fn read_robots_v248_motion_block_table<R: Read + Seek>(
        &self,
        reader: &mut R,
        endian: binrw::Endian,
    ) -> BinResult<Option<RobotsV248MotionBlockTable>> {
        let saved_position = reader.stream_position()?;
        let result = (|| {
            let object_offset = u64::from(self.common.address);
            let frame_count_offset = object_offset + 0x0e;
            reader.seek(std::io::SeekFrom::Start(frame_count_offset))?;
            let frame_count = reader.read_type::<u16>(endian)?;
            if frame_count == 0 {
                return Ok(None);
            }

            let directory_field = object_offset + 0x38;
            reader.seek(std::io::SeekFrom::Start(directory_field))?;
            let packed_directory = reader.read_type::<u32>(endian)?;
            let flags = (packed_directory & 0xff) as u8;
            const MOTION_BLOCK_TABLE_ID: u8 = 5;
            if flags & (1 << MOTION_BLOCK_TABLE_ID) == 0 {
                return Ok(None);
            }

            let directory_base = robots_v248_packed_rel_target(directory_field, packed_directory)?;
            let lower_mask = (1u8 << MOTION_BLOCK_TABLE_ID) - 1;
            let rank = (flags & lower_mask).count_ones() as u64;
            let entry_offset =
                directory_base
                    .checked_add(rank * 4)
                    .ok_or_else(|| binrw::Error::AssertFail {
                        pos: directory_field,
                        message: "Robots EXGeoAnim packed directory entry overflow".to_string(),
                    })?;
            reader.seek(std::io::SeekFrom::Start(entry_offset))?;
            let packed_entry = reader.read_type::<u32>(endian)?;
            let table_offset = robots_v248_packed_rel_target(entry_offset, packed_entry)?;
            reader.seek(std::io::SeekFrom::Start(table_offset))?;
            let size_low = u32::from(reader.read_type::<u16>(endian)?);
            let size_high = u32::from(reader.read_type::<u16>(endian)?);
            let raw_stride = (size_high << 16) | size_low;
            let aligned_stride = raw_stride
                .checked_add(0x1f)
                .map(|value| value & !0x1f)
                .ok_or_else(|| binrw::Error::AssertFail {
                    pos: table_offset,
                    message: "Robots motion block stride overflow".to_string(),
                })?;
            if aligned_stride == 0 {
                return Err(binrw::Error::AssertFail {
                    pos: table_offset,
                    message: "Robots motion block table has zero stride".to_string(),
                });
            }

            let final_frame = frame_count - 1;
            let mut end_frames = Vec::new();
            for _ in 0..4096 {
                let end_frame = reader.read_type::<u16>(endian)?;
                if let Some(previous) = end_frames.last() {
                    if end_frame <= *previous {
                        return Err(binrw::Error::AssertFail {
                            pos: reader.stream_position()?.saturating_sub(2),
                            message: format!(
                                "Robots motion block end frame {end_frame} is not after {previous}"
                            ),
                        });
                    }
                }
                end_frames.push(end_frame);
                if end_frame >= final_frame {
                    return Ok(Some(RobotsV248MotionBlockTable {
                        block_stride: aligned_stride as usize,
                        end_frames,
                    }));
                }
            }
            Err(binrw::Error::AssertFail {
                pos: table_offset,
                message: "Robots motion block table exceeded 4096 entries".to_string(),
            })
        })();
        reader.seek(std::io::SeekFrom::Start(saved_position))?;
        result
    }

    /// Read the self-delimiting compressed scalar stream referenced by
    /// serialized `EXGeoAnim+0x34`. Native `0x00503F80` passes this stream to
    /// `0x0056A045`; the first u16 locates the coefficient table in 4-byte units,
    /// and every scalar channel header advances both the descriptor and
    /// coefficient cursors, so no external byte-size field is required.
    pub fn read_robots_v248_scalar_stream<R: Read + Seek>(
        &self,
        reader: &mut R,
        endian: binrw::Endian,
    ) -> BinResult<Vec<u8>> {
        const MAX_SCALAR_STREAM_BYTES: usize = 64 * 1024 * 1024;
        let saved_position = reader.stream_position()?;
        let result = (|| {
            let object_offset = u64::from(self.common.address);
            reader.seek(std::io::SeekFrom::Start(object_offset + 0x16))?;
            let scalar_count = reader.read_type::<u16>(endian)?;
            if scalar_count == 0 {
                return Ok(Vec::new());
            }

            let scalar_field = object_offset + 0x34;
            reader.seek(std::io::SeekFrom::Start(scalar_field))?;
            let relative = reader.read_type::<i32>(endian)?;
            if relative == 0 {
                return Err(binrw::Error::AssertFail {
                    pos: scalar_field,
                    message: format!(
                        "Robots EXGeoAnim has {scalar_count} scalar channels but null +0x34 stream"
                    ),
                });
            }
            let stream_target = i128::from(scalar_field) + i128::from(relative);
            if stream_target < 0 || stream_target > i128::from(u64::MAX) {
                return Err(binrw::Error::AssertFail {
                    pos: scalar_field,
                    message: format!(
                        "Robots scalar stream relative pointer {relative} from 0x{scalar_field:X} is out of range"
                    ),
                });
            }
            let stream_target = stream_target as u64;
            reader.seek(std::io::SeekFrom::Start(stream_target))?;
            let coefficient_base_words = reader.read_type::<u16>(endian)?;
            let mut descriptor_offset = 2usize;
            let mut coefficient_offset = usize::from(coefficient_base_words)
                .checked_mul(4)
                .ok_or_else(|| binrw::Error::AssertFail {
                    pos: stream_target,
                    message: "Robots scalar coefficient base overflow".to_string(),
                })?;

            for _ in 0..usize::from(scalar_count) {
                let descriptor_address = stream_target
                    .checked_add(descriptor_offset as u64)
                    .ok_or_else(|| binrw::Error::AssertFail {
                        pos: stream_target,
                        message: "Robots scalar descriptor address overflow".to_string(),
                    })?;
                reader.seek(std::io::SeekFrom::Start(descriptor_address))?;
                let coefficient_words = usize::from(reader.read_type::<u16>(endian)?);
                let descriptor_words = usize::from(reader.read_type::<u16>(endian)?);
                if descriptor_words == 0 {
                    return Err(binrw::Error::AssertFail {
                        pos: descriptor_address + 2,
                        message: "Robots scalar channel has zero descriptor stride".to_string(),
                    });
                }
                descriptor_offset = descriptor_offset
                    .checked_add(descriptor_words.checked_mul(2).ok_or_else(|| {
                        binrw::Error::AssertFail {
                            pos: descriptor_address + 2,
                            message: "Robots scalar descriptor stride overflow".to_string(),
                        }
                    })?)
                    .ok_or_else(|| binrw::Error::AssertFail {
                        pos: descriptor_address,
                        message: "Robots scalar descriptor end overflow".to_string(),
                    })?;
                coefficient_offset = coefficient_offset
                    .checked_add(coefficient_words.checked_mul(4).ok_or_else(|| {
                        binrw::Error::AssertFail {
                            pos: descriptor_address,
                            message: "Robots scalar coefficient stride overflow".to_string(),
                        }
                    })?)
                    .ok_or_else(|| binrw::Error::AssertFail {
                        pos: descriptor_address,
                        message: "Robots scalar coefficient end overflow".to_string(),
                    })?;
            }

            let stream_len = descriptor_offset.max(coefficient_offset);
            if stream_len > MAX_SCALAR_STREAM_BYTES {
                return Err(binrw::Error::AssertFail {
                    pos: stream_target,
                    message: format!(
                        "Robots scalar stream size {stream_len} exceeds {MAX_SCALAR_STREAM_BYTES}-byte safety limit"
                    ),
                });
            }
            reader.seek(std::io::SeekFrom::Start(stream_target))?;
            let mut stream = vec![0u8; stream_len];
            reader.read_exact(&mut stream)?;
            Ok(stream)
        })();
        reader.seek(std::io::SeekFrom::Start(saved_position))?;
        result
    }

    /// Read the compressed motion blob referenced by `motiondata_info_addr`
    /// while preserving the caller's stream position.
    pub fn read_robots_v248_motion_stream<R: Read + Seek>(
        &self,
        reader: &mut R,
    ) -> std::io::Result<Vec<u8>> {
        let saved_position = reader.stream_position()?;
        reader.seek(std::io::SeekFrom::Start(u64::from(
            self.motiondata_info_addr,
        )))?;
        let mut motion = vec![0u8; self.datasize as usize];
        let read_result = reader.read_exact(&mut motion);
        let restore_result = reader.seek(std::io::SeekFrom::Start(saved_position));
        read_result?;
        restore_result?;
        Ok(motion)
    }
}

#[binrw]
#[derive(Debug, Serialize, Clone)]
pub struct EXGeoAnimSkinAnimDatumPayloadHeader {
    /// Exact HT_AnimDatum hash repeated from the owning 8-byte index entry.
    pub hashcode: u32,
    /// Serialized datum +0x04. Native map-collision consumers preserve it but
    /// its designer-facing role is not proven.
    pub raw_word_04: u16,
    /// Serialized datum +0x06. `0x004D64C0` proves mode 3 is capsule and all
    /// other values use the compact sphere representation.
    pub shape_mode: u8,
    pub raw_byte_07: u8,
}

/// Robots PC v248 variable-size AnimDatum payload referenced from AnimSkin
/// +0x60/+0x64. The first 0x30 bytes are the common spatial datum consumed by
/// `0x005391E8`; the tail starts with the transform selector read directly by
/// `0x00500569` before it indexes the AnimSkin transform matrix array.
#[derive(Debug, Serialize, Clone)]
pub struct EXGeoAnimSkinAnimDatumPayload {
    pub header: EXGeoAnimSkinAnimDatumPayloadHeader,
    /// Datum +0x08/+0x0C/+0x10. For HT_AnimDatum_MapCollisionCapsule, mode 3
    /// uses [0]=half-segment and [1]=radius; non-3 modes use [0]=sphere radius.
    pub shape_scalars: EXVector3,
    /// Local-space datum center at +0x14/+0x18/+0x1C.
    pub local_center: EXVector3,
    /// Local-space orientation quaternion at +0x20..+0x2C.
    pub local_orientation: [f32; 4],
    /// Datum +0x30. Native `0x00500569` multiplies this selector by 0x10 to
    /// select the transform matrix used by `0x005391E8`.
    pub transform_selector: u8,
    /// Serialized root-to-leaf hierarchy chain following +0x30. Corpus
    /// correlation against EXGeoAnimSkinHierData remains exact for all records.
    pub hierarchy_chain: Vec<u8>,
    #[serde(skip)]
    tail_padding: Vec<u8>,
}

impl EXGeoAnimSkinAnimDatumPayload {
    pub fn serialized_size(&self) -> usize {
        48 + ((2 + self.hierarchy_chain.len() + 3) & !3)
    }

    pub fn tail_padding_is_zero(&self) -> bool {
        self.tail_padding.iter().all(|byte| *byte == 0)
    }

    pub fn quaternion_norm(&self) -> f32 {
        self.local_orientation
            .iter()
            .map(|value| value * value)
            .sum::<f32>()
            .sqrt()
    }

    pub fn map_collision_radius(&self) -> Option<f32> {
        (self.header.hashcode == 0x1000_0004).then(|| {
            if self.header.shape_mode == 3 {
                self.shape_scalars[1]
            } else {
                self.shape_scalars[0]
            }
        })
    }

    pub fn map_collision_half_segment(&self) -> Option<f32> {
        (self.header.hashcode == 0x1000_0004 && self.header.shape_mode == 3)
            .then_some(self.shape_scalars[0])
    }
}

impl BinRead for EXGeoAnimSkinAnimDatumPayload {
    type Args<'a> = ();

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<Self> {
        let header = EXGeoAnimSkinAnimDatumPayloadHeader::read_options(reader, endian, ())?;
        let shape_scalars = <EXVector3>::read_options(reader, endian, ())?;
        let local_center = <EXVector3>::read_options(reader, endian, ())?;
        let local_orientation = <[f32; 4]>::read_options(reader, endian, ())?;
        let transform_selector = u8::read_options(reader, endian, ())?;
        let hierarchy_count = u8::read_options(reader, endian, ())? as usize;
        let mut hierarchy_chain = Vec::with_capacity(hierarchy_count);
        for _ in 0..hierarchy_count {
            hierarchy_chain.push(u8::read_options(reader, endian, ())?);
        }
        let unpadded_tail_size = 2 + hierarchy_count;
        let padded_tail_size = (unpadded_tail_size + 3) & !3;
        let padding_size = padded_tail_size - unpadded_tail_size;
        let mut tail_padding = Vec::with_capacity(padding_size);
        for _ in 0..padding_size {
            tail_padding.push(u8::read_options(reader, endian, ())?);
        }
        Ok(Self {
            header,
            shape_scalars,
            local_center,
            local_orientation,
            transform_selector,
            hierarchy_chain,
            tail_padding,
        })
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct EXGeoAnimSkinAnimDatumEntry {
    /// Exact HT_AnimDatum hash consumed by `0x00500569`.
    pub hashcode: u32,
    /// On a miss, native lookup advances by `1 + skip_count` index records.
    pub skip_count: u16,
    /// True when this record is visited by a fresh native hash lookup. Raw
    /// continuation records remain exposed for provenance even when skipped.
    pub searchable_head: bool,
    #[serde(skip)]
    datum_ptr: EXRelPtr16<EXGeoAnimSkinAnimDatumPayload>,
}

impl EXGeoAnimSkinAnimDatumEntry {
    pub fn datum_offset_absolute(&self) -> u64 {
        self.datum_ptr.offset_absolute()
    }

    pub fn datum(&self) -> &EXGeoAnimSkinAnimDatumPayload {
        self.datum_ptr.data_ref()
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct EXGeoAnimSkinAnimDatumHeader {
    /// Repeats the outer serialized count in all shipped Robots v248 sections.
    pub repeated_count: u16,
    pub raw_02: u8,
    pub raw_03: u8,
    pub raw_04: u8,
    /// Exact count consumed by native `0x00500569`.
    pub index_count: u8,
    #[serde(skip)]
    index_ptr: EXRelPtr16<()>,
    #[serde(skip)]
    first_datum_ptr: EXRelPtr,
}

impl EXGeoAnimSkinAnimDatumHeader {
    pub fn index_offset_absolute(&self) -> u64 {
        self.index_ptr.offset_absolute()
    }

    pub fn first_datum_offset_absolute(&self) -> u64 {
        self.first_datum_ptr.offset_absolute()
    }
}

/// Robots PC v248 AnimSkin AnimDatum channel serialized at object +0x60/+0x64.
///
/// `0x00500569` resolves this exact structure: the outer pair is raw index
/// count + self-relative pointer, the pointed 12-byte header contains byte +5
/// count and i16 +6 index pointer, and each 8-byte index record is
/// `{u32 HT_AnimDatum hash, u16 skip_count, i16 datum_rel}`. Datum pointers lead
/// to the common spatial records transformed by `0x005391E8`.
#[derive(Debug, Serialize, Clone)]
pub struct EXGeoAnimSkinAnimDatumSection {
    serialized_count: i32,
    #[serde(skip)]
    data_ptr: EXRelPtr,
    pub header: Option<EXGeoAnimSkinAnimDatumHeader>,
    /// All raw index records. `searchable_head` distinguishes the 0x00500569
    /// fresh-lookup traversal from continuation provenance records.
    pub entries: Vec<EXGeoAnimSkinAnimDatumEntry>,
}

impl EXGeoAnimSkinAnimDatumSection {
    pub fn serialized_len(&self) -> usize {
        self.serialized_count.max(0) as usize
    }

    pub fn data_offset_absolute(&self) -> u64 {
        self.data_ptr.offset_absolute()
    }

    pub fn searchable_entries(&self) -> impl Iterator<Item = &EXGeoAnimSkinAnimDatumEntry> {
        self.entries.iter().filter(|entry| entry.searchable_head)
    }

    pub fn find(&self, hashcode: u32) -> Option<&EXGeoAnimSkinAnimDatumPayload> {
        self.searchable_entries()
            .find(|entry| entry.hashcode == hashcode)
            .map(EXGeoAnimSkinAnimDatumEntry::datum)
    }
}

impl BinRead for EXGeoAnimSkinAnimDatumSection {
    type Args<'a> = ();

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<Self> {
        let serialized_count: i32 = BinRead::read_options(reader, endian, ())?;
        let data_ptr: EXRelPtr = BinRead::read_options(reader, endian, ())?;
        let mut header = None;
        let mut entries = Vec::new();

        if serialized_count > 0 {
            let saved_position = reader.stream_position()?;
            reader.seek(std::io::SeekFrom::Start(data_ptr.offset_absolute()))?;

            let parsed_header = EXGeoAnimSkinAnimDatumHeader {
                repeated_count: BinRead::read_options(reader, endian, ())?,
                raw_02: BinRead::read_options(reader, endian, ())?,
                raw_03: BinRead::read_options(reader, endian, ())?,
                raw_04: BinRead::read_options(reader, endian, ())?,
                index_count: BinRead::read_options(reader, endian, ())?,
                index_ptr: BinRead::read_options(reader, endian, ())?,
                first_datum_ptr: BinRead::read_options(reader, endian, ())?,
            };
            let index_offset = parsed_header.index_offset_absolute();
            header = Some(parsed_header);
            reader.seek(std::io::SeekFrom::Start(index_offset))?;

            entries.reserve(serialized_count as usize);
            for _ in 0..serialized_count {
                entries.push(EXGeoAnimSkinAnimDatumEntry {
                    hashcode: BinRead::read_options(reader, endian, ())?,
                    skip_count: BinRead::read_options(reader, endian, ())?,
                    searchable_head: false,
                    datum_ptr: BinRead::read_options(reader, endian, ())?,
                });
            }
            let mut index = 0usize;
            while index < entries.len() {
                entries[index].searchable_head = true;
                index = index.saturating_add(1 + usize::from(entries[index].skip_count));
            }
            reader.seek(std::io::SeekFrom::Start(saved_position))?;
        }

        Ok(Self {
            serialized_count,
            data_ptr,
            header,
            entries,
        })
    }
}

impl BinWrite for EXGeoAnimSkinAnimDatumSection {
    type Args<'a> = ();

    fn write_options<W: std::io::Write + Seek>(
        &self,
        _writer: &mut W,
        _endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<()> {
        todo!("AnimSkin AnimDatum section writing is not implemented")
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct EXGeoAnimSkinPostPairBlock {
    /// Optional sparse u16 pair table terminated by `0xFFFF,0xFFFF`.
    /// Its high-level role is not native-proven yet.
    pub sparse_pairs: Vec<[u16; 2]>,
    /// Exactly one structurally stable 32-byte / eight-float record per bone.
    /// Field semantics remain deliberately unnamed.
    pub bone_records: Vec<[f32; 8]>,
}

impl EXGeoAnimSkinPostPairBlock {
    pub fn serialized_size(&self) -> usize {
        (self.sparse_pairs.len() + 1) * 4 + self.bone_records.len() * 32
    }
}

impl BinRead for EXGeoAnimSkinPostPairBlock {
    type Args<'a> = (u32,);

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        (bone_count,): Self::Args<'_>,
    ) -> BinResult<Self> {
        let mut sparse_pairs = Vec::new();
        loop {
            let left = u16::read_options(reader, endian, ())?;
            let right = u16::read_options(reader, endian, ())?;
            if left == u16::MAX && right == u16::MAX {
                break;
            }
            sparse_pairs.push([left, right]);
        }

        let mut bone_records = Vec::with_capacity(bone_count as usize);
        for _ in 0..bone_count {
            bone_records.push(<[f32; 8]>::read_options(reader, endian, ())?);
        }

        Ok(Self {
            sparse_pairs,
            bone_records,
        })
    }
}

#[binrw]
#[derive(Debug, Serialize, Clone)]
#[brw(import(version: u32))]
pub struct EXGeoBaseAnimSkin {
    pub object_type: u32, // 0x0
    pub bone_count: u32,  // 0x4

    /// Two serialized DWORDs at +0x08/+0x0C. Robots.exe native AnimSkin consumers
    /// preserve these slots before the +0x10 bounds block; their semantics are not
    /// named because no traced v248 consumer assigns them a stable role yet.
    #[brw(if(version.ne(&213) && version.ne(&221) && version.ne(&163) && version.ne(&174)))]
    pub raw_08_0c: [u32; 2],
    #[brw(if(version.ne(&213) && version.ne(&163) && version.ne(&174)))]
    pub bounds_box: [EXVector; 2], // 0x10
    pub _unk30: [u32; 4], // 0x30

    #[br(count = bone_count)]
    pub absolute_bind_positions: EXRelPtr<Vec<EXVector>>, // 0x40
    #[br(count = bone_count)]
    pub relative_bind_positions: EXRelPtr<Vec<EXVector>>, // 0x44

    #[br(count = bone_count)]
    pub hier_data: EXRelPtr<Vec<EXGeoAnimSkinHierData>>, // 0x48
    /// Robots v248: start of a composite metadata block. Its target begins
    /// with DWORDs `[8, 0]`; the AnimBone pair table at +0x58 starts exactly
    /// eight bytes later. Semantics outside Robots v248 remain unclaimed.
    pub _unk4c: EXRelPtr<()>, // 0x4c
    #[brw(if(version.ne(&213) && version.ne(&163) && version.ne(&174)))]
    /// Robots v248 corpus: both DWORDs are zero for all 234 AnimSkins.
    pub _unk50: [u32; 2], // 0x50
    /// Robots v248 terminated `(bone_selector, HT_AnimBone low16)` u16 pairs.
    pub _unk58: EXRelPtr<u16>, // 0x58
    /// Robots v248: terminated sparse u16 pair table followed immediately by
    /// exactly `bone_count` structurally stable 32-byte / eight-float records.
    #[brw(if(version.eq(&248)))]
    #[br(args(bone_count))]
    pub robots_post_pair_block: Option<EXRelPtr<EXGeoAnimSkinPostPairBlock>>, // 0x5c
    #[brw(if(version.ne(&248)))]
    pub _unk5c: Option<EXRelPtr<()>>, // 0x5c, older EngineX layout remains opaque
    #[brw(if(version.eq(&248)))]
    pub robots_animdatum_section: Option<EXGeoAnimSkinAnimDatumSection>, // 0x60
    #[brw(if(version.ne(&248) && version.ne(&163)))]
    pub _unk60: Option<EXRelArray<()>>, // 0x60, older EngineX layout remains opaque
    pub entities: EXRelArray<EXGeoAnimSkinEntity>, // 0x68
    pub more_entities: EXRelArray<EXGeoAnimSkinEntity>, // 0x70, face-related entities?
    /// Robots PC v248 rigid Entity attachments driven by one bone matrix.
    /// Native 0x00500814 walks 0x14-byte records, uses +0x08 as a bone
    /// selector and +0x0C low24 as the current EDB Entity-list index.
    #[brw(if(version.eq(&248)))]
    pub bone_attachments: Option<EXRelArray<EXGeoAnimSkinBoneAttachment>>, // 0x78
    /// Layout intentionally remains opaque outside the Robots v248 target.
    #[brw(if(version.ne(&248)))]
    pub _unk78: Option<EXRelArray<()>>, // 0x78

    /// Robots PC v248 scalar-value count. Native initializer 0x004FC82C uses it
    /// to size a float buffer, and 0x004FE876 iterates exactly this many floats.
    #[brw(if(version.eq(&248)))]
    pub robots_scalar_value_count: u32, // 0x80

    /// Robots PC v248 groups over the scalar-value buffer. Native consumers
    /// 0x004FDF2E/0x004FE68D treat this as a count/relative-pointer array at
    /// +0x84/+0x88 with 0x10-byte records.
    #[brw(if(version.eq(&248)))]
    pub robots_scalar_groups: Option<EXRelArray<EXGeoAnimSkinScalarGroupRaw>>, // 0x84
}

/// Robots PC v248 0x10-byte AnimSkin scalar-group record.
///
/// Robots.exe 0x005186BD proves the scalar-count/base/mode fields and the
/// mode-specific relative data pointer. The first six bytes remain raw because
/// no traced v248 consumer gives them a stable semantic role yet.
#[binrw]
#[derive(Debug, Serialize, Clone)]
pub struct EXGeoAnimSkinScalarGroupRaw {
    pub raw_00_05: [u8; 6],
    /// Number of consecutive scalar values consumed by this morph group.
    pub scalar_count: u16, // +0x06
    /// First scalar index in the AnimSkin scalar-value buffer.
    pub scalar_base: u16, // +0x08
    /// Native morph mode. Robots.exe 0x005186BD has proven branches for 0 and 1.
    pub mode: i16, // +0x0A
    /// Mode-specific relative data. Native mode 1 treats this as a u16 sample-index table.
    pub mode_data: EXRelPtr, // +0x0C
}

#[binrw]
#[derive(Debug, Serialize, Clone)]
pub struct EXGeoAnimSkinBoneAttachment {
    /// Same 0x14-byte storage shape as a normal AnimSkin component, but the
    /// shipped rigid-attachment records keep the skin-data pointer null.
    pub skin_data_ptr: EXRelPtr, // +0x00
    /// Shipped Robots v248 rigid attachments serialize zero skinned parts.
    pub parts_count: u32, // +0x04
    /// Native Robots bone selector used to choose the current bone matrix.
    pub bone_selector: u32, // +0x08
    /// Current EDB Entity-list index in the low 24 bits.
    pub entity_index: u32, // +0x0C
    /// Shipped rigid attachments serialize the normal no-morph sentinel -1.
    pub morph_index: i32, // +0x10
}

impl EXGeoAnimSkinBoneAttachment {
    pub fn entity_list_index(&self) -> usize {
        (self.entity_index & 0x00ff_ffff) as usize
    }
}

#[binrw]
#[derive(Debug, Serialize, Clone)]
pub struct EXGeoAnimSkinEntity {
    skin_data_ptr: EXRelPtr,
    pub parts_count: u32,

    #[bw(assert(false), ignore)]
    #[br(parse_with(parse_late_skindata), args(&skin_data_ptr, parts_count))]
    pub skin_data: EXRelPtr<Vec<EXRelPtr<EXGeoAnimSkinPartWeights>>>,

    pub section_index: u32,
    /// Robots.exe 0x00500814 consumes the low 24 bits as an index into the
    /// current EDB Entity resource table (native record stride 0x14).
    pub entity_index: u32,
    pub morph_index: i32,
}

impl EXGeoAnimSkinEntity {
    /// Native Robots Entity-list index encoded in this AnimSkin component.
    pub fn entity_list_index(&self) -> usize {
        (self.entity_index & 0x00ff_ffff) as usize
    }
}

impl EXGeoBaseAnimSkin {
    /// Reads the Robots PC v248 AnimBone lookup pairs stored behind +0x58.
    /// Native consumer 0x005024D3 walks `(selector, animbone_low16)` u16 pairs
    /// until `animbone_low16 == 0xFFFF`.
    pub fn read_robots_v248_animbone_pairs<R: Read + Seek>(
        &self,
        reader: &mut R,
        endian: binrw::Endian,
    ) -> BinResult<Vec<(u16, u16)>> {
        if self._unk58.offset_relative() == 0 {
            return Ok(Vec::new());
        }

        let saved_position = reader.stream_position()?;
        reader.seek(std::io::SeekFrom::Start(self._unk58.offset_absolute()))?;
        let result = (|| {
            let mut pairs = Vec::new();
            for _ in 0..4096 {
                let selector = reader.read_type::<u16>(endian)?;
                let animbone_low16 = reader.read_type::<u16>(endian)?;
                if animbone_low16 == u16::MAX {
                    return Ok(pairs);
                }
                pairs.push((selector, animbone_low16));
            }
            Err(binrw::Error::AssertFail {
                pos: reader.stream_position()?,
                message: "Robots AnimBone pair table exceeded 4096 entries without terminator"
                    .to_string(),
            })
        })();
        reader.seek(std::io::SeekFrom::Start(saved_position))?;
        result
    }

    /// Resolves the optional Robots PC v248 AnimBone table into serialized
    /// bone slots. Unnamed slots remain `None`; exact global AnimBone hashes
    /// are preserved even when the generated HashDB has no human symbol.
    pub fn read_robots_v248_bone_hashcodes<R: Read + Seek>(
        &self,
        reader: &mut R,
        endian: binrw::Endian,
    ) -> BinResult<Vec<Option<u32>>> {
        let mut hashes = vec![None; self.bone_count as usize];
        for (selector, animbone_low16) in self.read_robots_v248_animbone_pairs(reader, endian)? {
            let index = usize::from(selector);
            let Some(slot) = hashes.get_mut(index) else {
                return Err(binrw::Error::AssertFail {
                    pos: self._unk58.offset_absolute(),
                    message: format!(
                        "Robots AnimBone selector {selector} exceeds bone_count {}",
                        self.bone_count
                    ),
                });
            };
            if slot.is_some() {
                return Err(binrw::Error::AssertFail {
                    pos: self._unk58.offset_absolute(),
                    message: format!("duplicate Robots AnimBone selector {selector}"),
                });
            }
            *slot = Some(0x0E00_0000 | u32::from(animbone_low16));
        }
        Ok(hashes)
    }
}

#[binrw]
#[derive(Debug, Serialize, Clone)]
pub struct EXGeoAnimSkinPartWeights {
    /// Number of bones retained by this mesh part's local skinning palette.
    pub palette_count: u32,
    #[br(count = palette_count)]
    #[bw(assert(false), ignore)]
    pub bone_palette: EXRelPtr<Vec<u8>>,
    /// Relative pointer to `mesh_vertex_count` consecutive 20-byte influence records.
    pub vertex_influence_data: EXRelPtr,
}

/// One PC Robots skinning record per mesh vertex.
///
/// Each selector is stored as `palette_slot * 3`, matching the three vec4 rows
/// used by the native affine skin matrix. The four weights are finite and sum
/// to one throughout the shipped PC corpus.
#[binrw]
#[derive(Debug, Serialize, Clone, Copy, PartialEq)]
pub struct EXGeoAnimSkinVertexInfluence {
    pub palette_slot_offsets_x3: [u8; 4],
    pub weights: [f32; 4],
}

impl EXGeoAnimSkinVertexInfluence {
    pub fn palette_slots(&self) -> Option<[u8; 4]> {
        self.palette_slot_offsets_x3
            .iter()
            .all(|value| value % 3 == 0)
            .then(|| self.palette_slot_offsets_x3.map(|value| value / 3))
    }

    pub fn bone_indices(&self, palette: &[u8]) -> Option<[u8; 4]> {
        let slots = self.palette_slots()?;
        let mut bones = [0u8; 4];
        for (lane, slot) in slots.into_iter().enumerate() {
            bones[lane] = *palette.get(slot as usize)?;
        }
        Some(bones)
    }
}

impl EXGeoAnimSkinPartWeights {
    pub fn read_vertex_influences<R: Read + Seek>(
        &self,
        reader: &mut R,
        endian: binrw::Endian,
        vertex_count: usize,
    ) -> BinResult<Vec<EXGeoAnimSkinVertexInfluence>> {
        let saved_position = reader.stream_position()?;
        reader.seek(std::io::SeekFrom::Start(
            self.vertex_influence_data.offset_absolute(),
        ))?;
        let result = reader.read_type_args::<Vec<EXGeoAnimSkinVertexInfluence>>(
            endian,
            VecArgs {
                count: vertex_count,
                inner: (),
            },
        );
        reader.seek(std::io::SeekFrom::Start(saved_position))?;
        result
    }
}

#[binrw]
#[derive(Debug, Serialize, Clone)]
pub struct EXGeoAnimSkinHierData {
    pub link_index: u16,
    pub max_index: u16,
    #[brw(pad_after(2))]
    pub flags: u16,
}

#[binrw::parser(reader, endian)]
fn parse_late_skindata(
    ptr: &EXRelPtr,
    length: u32,
) -> BinResult<EXRelPtr<Vec<EXRelPtr<EXGeoAnimSkinPartWeights>>>> {
    let pos_saved = reader.stream_position()?;
    reader.seek(std::io::SeekFrom::Start(ptr.offset_absolute()))?;

    let inner = <_>::read_options(
        reader,
        endian,
        VecArgs {
            count: length as usize,
            inner: (),
        },
    )?;
    reader.seek(std::io::SeekFrom::Start(pos_saved))?;

    Ok(EXRelPtr::new_with_offset(
        ptr.offset_relative(),
        ptr.offset_absolute(),
        inner,
    ))
}

#[cfg(test)]
mod tests {
    use std::{
        fs::File,
        io::{BufReader, Cursor, Seek, SeekFrom},
        path::Path,
    };

    use binrw::BinReaderExt;

    use super::*;
    use crate::{edb::EdbFile, versions::Platform};

    #[test]
    fn robots_v248_exgeoanim_serialized_offsets_match_native_layout() {
        let mut bytes = vec![0u8; 0x9c];
        bytes[0x0d] = 0x1f;
        bytes[0x0e..0x10].copy_from_slice(&29u16.to_le_bytes());
        bytes[0x14] = 29;
        bytes[0x15] = 4;
        bytes[0x16..0x18].copy_from_slice(&23u16.to_le_bytes());
        bytes[0x60..0x64].copy_from_slice(&0x0002_2401u32.to_le_bytes());
        bytes[0x8c..0x90].copy_from_slice(&0.25f32.to_le_bytes());
        bytes[0x90..0x94].copy_from_slice(&0.5f32.to_le_bytes());
        bytes[0x94..0x98].copy_from_slice(&0.75f32.to_le_bytes());

        let mut reader = Cursor::new(bytes);
        let parsed = reader
            .read_le::<RobotsV248EXGeoAnim>()
            .expect("serialized EXGeoAnim");
        assert_eq!(reader.stream_position().unwrap(), 0x9c);
        assert_eq!(parsed.raw_byte_0d, 0x1f);
        assert_eq!(parsed.frame_count, 29);
        assert_eq!(parsed.bone_count, 29);
        assert_eq!(parsed.raw_byte_15, 4);
        assert_eq!(parsed.scalar_channel_count, 23);
        assert_eq!(parsed.translation_channel_masks, [0x0002_2401, 0, 0, 0]);
        assert_eq!(parsed.base_translation, [0.25, 0.5, 0.75]);
    }

    #[test]
    fn robots_v248_root_motion_extraction_keeps_complementary_1500_channels() {
        let anim = RobotsV248EXGeoAnim {
            raw_00: 0,
            raw_04: 0,
            raw_08: 0,
            raw_byte_0c: 60,
            raw_byte_0d: 0,
            frame_count: 1,
            raw_10: 0x1500,
            bone_count: 1,
            raw_byte_15: 0,
            scalar_channel_count: 0,
            raw_18_5f: [0; 0x48],
            translation_channel_masks: [1, 0, 0, 0],
            raw_70_8b: [0; 0x1c],
            base_translation: [10.0, 20.0, 30.0],
            raw_98_9b: [0; 4],
        };
        let rotation = robots_v248_fixed_euler_to_quat([0.2, 0.3, 0.4], 4).unwrap();
        let frame = RobotsV248SkeletalFrame {
            frame_index: 0,
            bones: vec![RobotsV248SkeletalBoneFrame {
                translation_delta: Some([1.0, 2.0, 3.0]),
                rotation,
            }],
        };

        let extracted = anim
            .extract_root_motion_transform(&frame)
            .expect("extract synthetic root motion");
        assert_eq!(extracted.position, [11.0, 0.0, 33.0]);
        let expected_rotation = robots_v248_fixed_euler_to_quat([0.0, 0.3, 0.0], 4).unwrap();
        for (actual, expected) in extracted.rotation.iter().zip(expected_rotation) {
            assert!((actual - expected).abs() < 1.0e-6, "{actual} != {expected}");
        }
    }

    #[test]
    fn robots_v248_root_motion_extraction_rejects_unproven_packed_correction() {
        let anim = RobotsV248EXGeoAnim {
            raw_00: 0,
            raw_04: 0,
            raw_08: 0,
            raw_byte_0c: 60,
            raw_byte_0d: 0,
            frame_count: 1,
            raw_10: 0x4000,
            bone_count: 1,
            raw_byte_15: 0,
            scalar_channel_count: 0,
            raw_18_5f: [0; 0x48],
            translation_channel_masks: [0; 4],
            raw_70_8b: [0; 0x1c],
            base_translation: [0.0; 3],
            raw_98_9b: [0; 4],
        };
        let frame = RobotsV248SkeletalFrame {
            frame_index: 0,
            bones: vec![RobotsV248SkeletalBoneFrame {
                translation_delta: None,
                rotation: [0.0, 0.0, 0.0, 1.0],
            }],
        };
        assert!(anim.extract_root_motion_transform(&frame).is_err());
    }

    #[test]
    fn robots_v248_compressed_motion_decodes_sparse_polynomial_quaternion_channel() {
        let anim = RobotsV248EXGeoAnim {
            raw_00: 0,
            raw_04: 0,
            raw_08: 0,
            raw_byte_0c: 0,
            raw_byte_0d: 0,
            frame_count: 4,
            raw_10: 0,
            bone_count: 1,
            raw_byte_15: 0,
            scalar_channel_count: 0,
            raw_18_5f: [0; 0x48],
            translation_channel_masks: [0; 4],
            raw_70_8b: [0; 0x1c],
            base_translation: [0.0; 3],
            raw_98_9b: [0; 4],
        };
        let mut motion = vec![0u8; 32];
        // Three six-byte channel descriptors occupy bytes 2..20, so the first
        // coefficient begins at 5 * 4 = 20. Each channel has one constant c0
        // segment whose frame span covers integer frames 0..3.
        motion[0..2].copy_from_slice(&5u16.to_le_bytes());
        for descriptor_offset in [2usize, 8, 14] {
            motion[descriptor_offset..descriptor_offset + 2].copy_from_slice(&1u16.to_le_bytes());
            motion[descriptor_offset + 2..descriptor_offset + 4]
                .copy_from_slice(&3u16.to_le_bytes());
            motion[descriptor_offset + 4..descriptor_offset + 6]
                .copy_from_slice(&((3u16 << 6) | 1).to_le_bytes());
        }
        motion[20..24].copy_from_slice(&0.1f32.to_le_bytes());
        motion[24..28].copy_from_slice(&(-0.2f32).to_le_bytes());
        motion[28..32].copy_from_slice(&0.3f32.to_le_bytes());

        let frame = anim
            .decode_skeletal_frame(&motion, 3)
            .expect("decode synthetic compressed skeletal frame");
        assert_eq!(frame.frame_index, 3);
        assert_eq!(frame.bones.len(), 1);
        assert_eq!(frame.bones[0].translation_delta, None);
        let expected_w = (1.0f32 - 0.1 * 0.1 - 0.2 * 0.2 - 0.3 * 0.3).sqrt();
        let expected = [0.1, -0.2, 0.3, expected_w];
        for (actual, expected) in frame.bones[0].rotation.iter().zip(expected) {
            assert!((actual - expected).abs() < 1.0e-6, "{actual} != {expected}");
        }
        assert!(anim.decode_skeletal_frame(&motion, 4).is_err());
    }

    #[test]
    fn robots_v248_motion_block_selection_keeps_native_overlap_frame() {
        let table = RobotsV248MotionBlockTable {
            block_stride: 0x4000,
            end_frames: vec![127, 135],
        };
        let first = table
            .select(127, 136, 0x8000)
            .expect("select first native block");
        assert_eq!(first.stream_offset, 0);
        assert_eq!(first.start_frame, 0);
        assert_eq!(first.frame_count, 129);
        assert_eq!(first.relative_frame, 127);

        // Frame 128 is physically present as the first block's overlap sample,
        // but native selection already switches ownership to block 1.
        let second = table
            .select(128, 136, 0x8000)
            .expect("select second native block");
        assert_eq!(second.stream_offset, 0x4000);
        assert_eq!(second.start_frame, 128);
        assert_eq!(second.frame_count, 8);
        assert_eq!(second.relative_frame, 0);
    }

    #[test]
    fn robots_v248_same_skin_pose_uses_native_translation_and_bind_position_policy() {
        let anim = RobotsV248EXGeoAnim {
            raw_00: 0,
            raw_04: 0,
            raw_08: 0,
            raw_byte_0c: 0,
            raw_byte_0d: 0,
            frame_count: 1,
            raw_10: 0,
            bone_count: 3,
            raw_byte_15: 0,
            scalar_channel_count: 0,
            raw_18_5f: [0; 0x48],
            translation_channel_masks: [0b11, 0, 0, 0],
            raw_70_8b: [0; 0x1c],
            base_translation: [0.5, 1.0, 1.5],
            raw_98_9b: [0; 4],
        };
        let frame = RobotsV248SkeletalFrame {
            frame_index: 0,
            bones: vec![
                RobotsV248SkeletalBoneFrame {
                    translation_delta: Some([1.0, 2.0, 3.0]),
                    rotation: [0.0, 0.0, 0.0, 1.0],
                },
                RobotsV248SkeletalBoneFrame {
                    translation_delta: Some([4.0, 5.0, 6.0]),
                    rotation: [0.0, 0.0, 0.0, 1.0],
                },
                RobotsV248SkeletalBoneFrame {
                    translation_delta: None,
                    rotation: [0.0, 0.0, 0.0, 1.0],
                },
            ],
        };
        let bind_positions = [
            [10.0, 20.0, 30.0, 1.0],
            [11.0, 21.0, 31.0, 1.0],
            [12.0, 22.0, 32.0, 1.0],
        ];
        let pose = anim
            .assemble_same_skin_pose(&frame, &bind_positions)
            .expect("assemble native same-skin pose");
        assert_eq!(pose.bones[0].position, [1.5, 3.0, 4.5]);
        assert_eq!(pose.bones[1].position, [4.0, 5.0, 6.0]);
        assert_eq!(pose.bones[2].position, [12.0, 22.0, 32.0]);
    }

    #[test]
    fn real_robots_v248_eq01_dog_animmode_headers_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out")
            .join("extracted_main")
            .join("robots")
            .join("binary")
            .join("_bin_pc")
            .join("eq01_dog.edb");
        let file = File::open(&path).expect("open eq01_dog.edb");
        let edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse eq01_dog.edb");
        let modes = edb.header.animmode_list.data();
        let sets = edb.header.animset_list.data();
        assert_eq!(modes.len(), 23);
        assert_eq!(sets.len(), 22);
        assert_eq!(sets[0].common.hashcode, 0x8a00_0000);
        assert_eq!(sets[21].common.hashcode, 0x8a00_0015);
        assert!(sets
            .iter()
            .enumerate()
            .all(|(index, set)| set.common.debug as usize == 23 + index && set.num_anim_sets == 1));
        assert_eq!(modes[0].common.hashcode, 0x0900_0002);
        assert_eq!(modes[0].num_anim_modes, 23);
        assert_eq!(modes[0].common.address, 0x1010);
        assert!(modes
            .iter()
            .enumerate()
            .all(|(index, mode)| mode.common.address == 0x1010
                && mode.common.debug as usize == index
                && (index == 0 || mode.num_anim_modes == 0)));

        let move_index = modes
            .iter()
            .position(|mode| mode.common.hashcode == 0x0900_0003)
            .expect("HT_AnimMode_Move");
        assert_eq!(move_index, 1);

        let mut reader = BufReader::new(File::open(&path).expect("reopen eq01_dog.edb"));
        let transitions = modes[0]
            .read_robots_v248_transitions(&mut reader, binrw::Endian::Little)
            .expect("parse Default AnimMode transitions");
        assert_eq!(transitions.len(), 23);
        let move_transition = transitions
            .iter()
            .find(|transition| transition.new_mode_index as usize == move_index)
            .expect("Default -> Move transition");
        assert_eq!(move_transition.controls.len(), 1);
        let control = &move_transition.controls[0];
        assert_eq!(control.opcode, 0x0c00_0002);
        assert_eq!(control.resource_key, 20);
        assert_eq!(
            sets[control.resource_key as usize].common.hashcode,
            0x8a00_0014
        );
        assert_eq!(control.raw_word_08, 0);
        assert_eq!(control.sparse_mask, 0);
        assert!(control.sparse_values.is_empty());

        let move_groups = sets[control.resource_key as usize]
            .read_robots_v248_groups(&mut reader, binrw::Endian::Little)
            .expect("parse Default -> Move AnimSet");
        assert_eq!(move_groups.len(), 1);
        assert_eq!(move_groups[0].layer, 0);
        assert_eq!(move_groups[0].contribution_count, 1);
        assert_eq!(move_groups[0].weight, 0.0);
        assert_eq!(move_groups[0].contributions.len(), 1);
        assert_eq!(
            move_groups[0].contributions[0].resource_hashcode,
            0x8400_0016
        );
        assert_eq!(move_groups[0].contributions[0].raw_u16_04, 0x000b);
        assert_eq!(move_groups[0].contributions[0].raw_u16_06, 0x0005);
        assert_eq!(move_groups[0].contributions[0].raw_u32_08, 0);
    }

    #[test]
    fn real_robots_v248_rodney_exgeoanim_matches_native_dimensions_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out")
            .join("extracted_main")
            .join("robots")
            .join("binary")
            .join("_bin_pc")
            .join("p01_rod.edb");
        let file = File::open(&path).expect("open p01_rod.edb");
        let edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse p01_rod.edb");
        assert_eq!(edb.header.version, 248);
        let animation = edb
            .header
            .anim_list
            .data()
            .first()
            .expect("Rodney animation 0");
        assert_eq!(animation.common.hashcode, 0x0300_000e);
        assert_eq!(animation.common.address, 0x0003_4640);
        assert_eq!(animation.motiondata_info_addr, 0x001c_c0c0);

        let mut reader = BufReader::new(File::open(&path).expect("reopen p01_rod.edb"));
        let parsed = animation
            .read_robots_v248_exgeoanim(&mut reader, binrw::Endian::Little)
            .expect("read serialized Rodney EXGeoAnim");
        assert_eq!(parsed.frame_count, 29);
        assert_eq!(parsed.bone_count, 29);
        assert_eq!(parsed.scalar_channel_count, 23);
        assert_eq!(parsed.raw_byte_0d, 31);
        assert_eq!(parsed.raw_byte_15, 4);
        assert_eq!(parsed.translation_channel_masks[0], 0x0002_2401);
        assert!((parsed.base_translation[0] - 0.000_483_889_98).abs() < 1.0e-8);
        assert!((parsed.base_translation[1] - 0.870_571_97).abs() < 1.0e-7);
        assert_eq!(parsed.base_translation[2], 0.0);

        let motion = animation
            .read_robots_v248_motion_stream(&mut reader)
            .expect("read Rodney compressed motion stream");
        assert_eq!(motion.len(), animation.datasize as usize);
        let frame0 = parsed
            .decode_skeletal_frame(&motion, 0)
            .expect("decode Rodney native frame 0");
        assert_eq!(frame0.bones.len(), 29);
        assert!(frame0.bones[0].translation_delta.is_some());
        assert!(frame0.bones[10].translation_delta.is_some());
        assert!(frame0.bones[13].translation_delta.is_some());
        assert!(frame0.bones[17].translation_delta.is_some());
        assert!(frame0.bones[1].translation_delta.is_none());
        let expected_root_rotation = [0.053_74, -0.014_43, -0.005_470_1, 0.998_435_7];
        for (actual, expected) in frame0.bones[0].rotation.iter().zip(expected_root_rotation) {
            assert!((actual - expected).abs() < 2.0e-5, "{actual} != {expected}");
        }
        let root_delta0 = frame0.bones[0]
            .translation_delta
            .expect("root translation delta");
        assert!(root_delta0.iter().all(|value| value.abs() < 1.0e-6));

        let frame14 = parsed
            .decode_skeletal_frame(&motion, 14)
            .expect("decode Rodney native frame 14");
        let expected_root_rotation = [0.040_715_2, -0.014_359_9, -0.005_660_7, 0.999_051_6];
        for (actual, expected) in frame14.bones[0].rotation.iter().zip(expected_root_rotation) {
            assert!((actual - expected).abs() < 2.0e-5, "{actual} != {expected}");
        }
        let root_delta14 = frame14.bones[0]
            .translation_delta
            .expect("root frame 14 translation delta");
        let expected_delta14 = [-0.000_013_194_328, -0.043_592_427, -0.000_023_199_555];
        for (actual, expected) in root_delta14.iter().zip(expected_delta14) {
            assert!((actual - expected).abs() < 2.0e-5, "{actual} != {expected}");
        }

        let scalar_stream = animation
            .read_robots_v248_scalar_stream(&mut reader, binrw::Endian::Little)
            .expect("read Rodney compressed scalar stream");
        assert_eq!(scalar_stream.len(), 0x2f8);
        let scalar0 = parsed
            .decode_scalar_frame(&scalar_stream, 0)
            .expect("decode Rodney scalar frame 0");
        assert_eq!(scalar0.len(), 23);
        assert_eq!(scalar0[0], 0.0);
        assert!((scalar0[2] - (-2.135_087_3e-8)).abs() < 1.0e-10);
        assert!((scalar0[21] - 0.999_999_94).abs() < 1.0e-7);
        let scalar14 = parsed
            .decode_scalar_frame(&scalar_stream, 14)
            .expect("decode Rodney scalar frame 14");
        assert!((scalar14[2] - 0.450_079_44).abs() < 2.0e-6);
        assert!((scalar14[7] - 0.227_120_98).abs() < 2.0e-6);
        assert!((scalar14[20] - 1.000_000_95).abs() < 2.0e-6);

        // Clip 78 is the shipped regression that proves non-final block overlap:
        // ID5 serializes end=127, while native 0x005563F4 hands 0x00567B30 a
        // 129-frame decode context (0..128). Without that extra frame, the first
        // checkpoint triple is mistaken for polynomial segment words.
        let animation78 = edb
            .header
            .anim_list
            .data()
            .get(78)
            .expect("Rodney animation 78");
        let parsed78 = animation78
            .read_robots_v248_exgeoanim(&mut reader, binrw::Endian::Little)
            .expect("read Rodney animation 78 EXGeoAnim");
        assert_eq!(parsed78.frame_count, 136);
        let table78 = animation78
            .read_robots_v248_motion_block_table(&mut reader, binrw::Endian::Little)
            .expect("read Rodney animation 78 block table")
            .expect("Rodney animation 78 uses native block table");
        assert_eq!(table78.block_stride, 0x4000);
        assert_eq!(table78.end_frames, vec![127, 135]);
        let selection78 = table78
            .select(1, parsed78.frame_count, animation78.datasize as usize)
            .expect("select Rodney animation 78 block 0");
        assert_eq!(selection78.frame_count, 129);
        let motion78 = animation78
            .read_robots_v248_motion_stream(&mut reader)
            .expect("read Rodney animation 78 motion");
        let frame1_78 = parsed78
            .decode_skeletal_frame_with_block_table(&motion78, 1, &table78)
            .expect("decode Rodney animation 78 frame 1");
        let expected_root_rotation78 = [0.050_269_72, -0.014_080_064, -0.005_14, 0.998_623_2];
        for (actual, expected) in frame1_78.bones[0]
            .rotation
            .iter()
            .zip(expected_root_rotation78)
        {
            assert!((actual - expected).abs() < 2.0e-5, "{actual} != {expected}");
        }
    }

    #[test]
    fn real_robots_v248_animskin_tail_parses_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let source_root = Path::new(&game_root)
            .join("_eurotools_out")
            .join("extracted_main")
            .join("robots")
            .join("binary")
            .join("_bin_pc");
        let mut files = Vec::new();
        collect_edb_files(&source_root, &mut files);
        assert!(
            !files.is_empty(),
            "no Robots EDB files under {}",
            source_root.display()
        );

        let mut skins = 0usize;
        let mut total_bones = 0usize;
        let mut scalar_groups = 0usize;
        let mut morph_mode_0 = 0usize;
        let mut morph_mode_1 = 0usize;
        let mut morph_other_modes = 0usize;
        let mut animbone_pairs = 0usize;
        let mut animbone_selectors_in_bone_range = 0usize;
        let mut animbone_named = 0usize;
        let mut unknown_animbone_hashes = std::collections::BTreeSet::new();
        let mut animbone_selector_conflicts = 0usize;
        let mut animbone_id_conflicts = 0usize;
        let mut bone_attachments = 0usize;
        let mut attachment_bone_oob = 0usize;
        let mut attachment_entity_oob = 0usize;
        let mut post_pair_blocks = 0usize;
        let mut post_pair_sparse_pairs = 0usize;
        let mut post_pair_bone_records = 0usize;
        let mut post_pair_finite_bone_records = 0usize;

        let mut animdatum_sections_nonempty = 0usize;
        let mut animdatum_entries = 0usize;
        let mut animdatum_searchable_entries = 0usize;
        let mut animdatum_count_distribution = std::collections::BTreeMap::<usize, usize>::new();
        let mut animdatum_raw_skip_counts = std::collections::BTreeMap::<u16, usize>::new();
        let mut animdatum_head_skip_counts = std::collections::BTreeMap::<u16, usize>::new();
        let mut animdatum_hash_identity_matches = 0usize;
        let mut animdatum_family_records = 0usize;
        let mut animdatum_sentinels = 0usize;
        let mut animdatum_finite_records = 0usize;
        let mut animdatum_unit_quaternions = 0usize;
        let mut animdatum_tail_layout_matches = 0usize;
        let mut animdatum_tail_selector_in_range = 0usize;
        let mut animdatum_tail_hierarchy_matches = 0usize;
        let mut map_collision_raw_modes = std::collections::BTreeMap::<u8, usize>::new();
        let mut map_collision_head_modes = std::collections::BTreeMap::<u8, usize>::new();
        let mut rodney_capsule = None;

        for path in files {
            let file = File::open(&path)
                .unwrap_or_else(|error| panic!("open {}: {error}", path.display()));
            let mut edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
                .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()));
            if edb.header.version != 248 {
                continue;
            }
            let headers = edb.header.animskin_list.data().clone();
            let entity_count = edb.header.entity_list.len();
            for header in headers {
                edb.seek(SeekFrom::Start(header.common.address as u64))
                    .unwrap();
                let skin = edb
                    .read_type_args::<EXGeoBaseAnimSkin>(edb.endian, (248,))
                    .unwrap_or_else(|error| {
                        panic!(
                            "parse AnimSkin 0x{:08X} in {}: {error}",
                            header.common.hashcode,
                            path.display()
                        )
                    });
                skins += 1;
                total_bones += skin.bone_count as usize;
                assert_eq!(skin._unk50, [0, 0]);

                let section = skin
                    .robots_animdatum_section
                    .as_ref()
                    .expect("v248 must contain +0x60/+0x64 AnimDatum section");
                let section_len = section.serialized_len();
                *animdatum_count_distribution.entry(section_len).or_default() += 1;
                assert_eq!(section.entries.len(), section_len);
                if section_len == 0 {
                    assert!(section.header.is_none());
                } else {
                    animdatum_sections_nonempty += 1;
                    let section_header = section.header.as_ref().unwrap();
                    assert_eq!(usize::from(section_header.repeated_count), section_len);
                    assert_eq!(usize::from(section_header.index_count), section_len);
                    assert_eq!(section_header.raw_02, 1);
                    assert_eq!(section_header.raw_03, 1);
                    assert_eq!(section_header.raw_04, 0);
                    assert_eq!(
                        section_header.index_offset_absolute(),
                        section.data_offset_absolute() + 12
                    );
                    assert_eq!(
                        section.entries.first().unwrap().datum_offset_absolute(),
                        section_header.first_datum_offset_absolute()
                    );
                }

                for entry in &section.entries {
                    animdatum_entries += 1;
                    *animdatum_raw_skip_counts
                        .entry(entry.skip_count)
                        .or_default() += 1;
                    if entry.searchable_head {
                        animdatum_searchable_entries += 1;
                        *animdatum_head_skip_counts
                            .entry(entry.skip_count)
                            .or_default() += 1;
                    }
                    let datum = entry.datum();
                    if datum.header.hashcode == entry.hashcode {
                        animdatum_hash_identity_matches += 1;
                    }
                    if entry.hashcode & 0xFFFF_0000 == 0x1000_0000 {
                        animdatum_family_records += 1;
                    } else if entry.hashcode == u32::MAX {
                        animdatum_sentinels += 1;
                    }
                    if datum
                        .shape_scalars
                        .iter()
                        .chain(datum.local_center.iter())
                        .chain(datum.local_orientation.iter())
                        .all(|value| value.is_finite())
                    {
                        animdatum_finite_records += 1;
                    }
                    if (datum.quaternion_norm() - 1.0).abs() <= 1.0e-4 {
                        animdatum_unit_quaternions += 1;
                    }
                    if datum.tail_padding_is_zero() {
                        animdatum_tail_layout_matches += 1;
                    }
                    if usize::from(datum.transform_selector) < skin.bone_count as usize {
                        animdatum_tail_selector_in_range += 1;
                        let mut hierarchy_chain = Vec::new();
                        let mut current = usize::from(datum.transform_selector);
                        let mut valid_chain = true;
                        loop {
                            if hierarchy_chain.contains(&(current as u8)) {
                                valid_chain = false;
                                break;
                            }
                            hierarchy_chain.push(current as u8);
                            let parent = skin.hier_data[current].link_index;
                            if parent == u16::MAX {
                                break;
                            }
                            current = usize::from(parent);
                            if current >= skin.bone_count as usize || current > u8::MAX as usize {
                                valid_chain = false;
                                break;
                            }
                        }
                        hierarchy_chain.reverse();
                        if valid_chain && hierarchy_chain == datum.hierarchy_chain {
                            animdatum_tail_hierarchy_matches += 1;
                        }
                    }
                    if entry.hashcode == 0x1000_0004 {
                        *map_collision_raw_modes
                            .entry(datum.header.shape_mode)
                            .or_default() += 1;
                        if entry.searchable_head {
                            *map_collision_head_modes
                                .entry(datum.header.shape_mode)
                                .or_default() += 1;
                        }
                        if path.file_name().and_then(|name| name.to_str()) == Some("p01_rod.edb")
                            && header.common.hashcode == 0x0D00_0001
                            && entry.searchable_head
                        {
                            rodney_capsule = Some((
                                datum.header.shape_mode,
                                datum.map_collision_half_segment().unwrap(),
                                datum.map_collision_radius().unwrap(),
                                datum.local_center,
                                datum.transform_selector,
                                datum.hierarchy_chain.clone(),
                            ));
                        }
                    }
                }

                let post_pair_block = skin
                    .robots_post_pair_block
                    .as_ref()
                    .expect("v248 must contain typed +0x5C post-pair block");
                let post_pair = post_pair_block.data_ref();
                post_pair_blocks += 1;
                post_pair_sparse_pairs += post_pair.sparse_pairs.len();
                assert_eq!(post_pair.bone_records.len(), skin.bone_count as usize);
                post_pair_bone_records += post_pair.bone_records.len();
                post_pair_finite_bone_records += post_pair
                    .bone_records
                    .iter()
                    .filter(|record| record.iter().all(|value| value.is_finite()))
                    .count();

                let endian = edb.endian;
                let pairs = skin
                    .read_robots_v248_animbone_pairs(&mut edb, endian)
                    .unwrap_or_else(|error| {
                        panic!(
                            "read AnimBone table for AnimSkin 0x{:08X} in {}: {error}",
                            header.common.hashcode,
                            path.display()
                        )
                    });
                let mut selectors = std::collections::BTreeSet::new();
                let mut animbone_ids = std::collections::BTreeSet::new();
                for (selector, animbone_low16) in pairs {
                    animbone_pairs += 1;
                    if usize::from(selector) < skin.bone_count as usize {
                        animbone_selectors_in_bone_range += 1;
                    }
                    let animbone_hash = 0x0E00_0000 | u32::from(animbone_low16);
                    if crate::robots_hashdb::resolve(animbone_hash).is_some() {
                        animbone_named += 1;
                    } else {
                        unknown_animbone_hashes.insert(animbone_hash);
                    }
                    if !selectors.insert(selector) {
                        animbone_selector_conflicts += 1;
                    }
                    if !animbone_ids.insert(animbone_low16) {
                        animbone_id_conflicts += 1;
                    }
                }

                let groups = skin
                    .robots_scalar_groups
                    .as_ref()
                    .expect("v248 must contain +0x84/+0x88 scalar-group array");
                scalar_groups += groups.serialized_len();
                for group in groups.iter() {
                    match group.mode {
                        0 => morph_mode_0 += 1,
                        1 => morph_mode_1 += 1,
                        _ => morph_other_modes += 1,
                    }
                }

                let attachments = skin
                    .bone_attachments
                    .as_ref()
                    .expect("v248 must contain +0x78/+0x7C bone-attachment array");
                bone_attachments += attachments.len();
                for attachment in attachments.iter() {
                    assert_eq!(attachment.skin_data_ptr.offset_relative(), 0);
                    assert_eq!(attachment.parts_count, 0);
                    assert_eq!(attachment.morph_index, -1);
                    if attachment.bone_selector as usize >= skin.bone_count as usize {
                        attachment_bone_oob += 1;
                    }
                    if attachment.entity_list_index() >= entity_count {
                        attachment_entity_oob += 1;
                    }
                }
                for component in skin.entities.iter().chain(skin.more_entities.iter()) {
                    assert!(component.entity_list_index() < entity_count);
                }
            }
        }

        assert_eq!(skins, 234);
        assert_eq!(total_bones, 5282);
        assert_eq!(scalar_groups, 200);
        assert_eq!(morph_mode_0, 200);
        assert_eq!(morph_mode_1, 0);
        assert_eq!(morph_other_modes, 0);
        assert_eq!(animbone_pairs, 4197);
        assert_eq!(animbone_selectors_in_bone_range, animbone_pairs);
        assert_eq!(animbone_named, 4179);
        assert_eq!(
            unknown_animbone_hashes,
            std::collections::BTreeSet::from([0x0E00_001F])
        );
        assert_eq!(animbone_selector_conflicts, 0);
        assert_eq!(animbone_id_conflicts, 0);
        assert_eq!(bone_attachments, 41);
        assert_eq!(attachment_bone_oob, 0);
        assert_eq!(attachment_entity_oob, 0);
        assert_eq!(post_pair_blocks, skins);
        assert_eq!(post_pair_sparse_pairs, 116);
        assert_eq!(post_pair_bone_records, total_bones);
        assert_eq!(post_pair_finite_bone_records, post_pair_bone_records);

        assert_eq!(animdatum_sections_nonempty, 93);
        assert_eq!(animdatum_entries, 669);
        assert_eq!(animdatum_searchable_entries, 644);
        assert_eq!(animdatum_hash_identity_matches, animdatum_entries);
        assert_eq!(animdatum_family_records, 668);
        assert_eq!(animdatum_sentinels, 1);
        assert_eq!(animdatum_finite_records, animdatum_entries);
        assert_eq!(animdatum_unit_quaternions, animdatum_entries);
        assert_eq!(animdatum_tail_layout_matches, animdatum_entries);
        assert_eq!(animdatum_tail_selector_in_range, animdatum_entries);
        assert_eq!(animdatum_tail_hierarchy_matches, animdatum_entries);
        assert_eq!(
            animdatum_raw_skip_counts,
            std::collections::BTreeMap::from([(0, 644), (1, 23), (2, 2)])
        );
        assert_eq!(
            animdatum_head_skip_counts,
            std::collections::BTreeMap::from([(0, 621), (1, 21), (2, 2)])
        );
        assert_eq!(
            map_collision_raw_modes,
            std::collections::BTreeMap::from([(1, 3), (3, 84)])
        );
        assert_eq!(
            map_collision_head_modes,
            std::collections::BTreeMap::from([(1, 3), (3, 83)])
        );
        assert_eq!(
            animdatum_count_distribution,
            std::collections::BTreeMap::from([
                (0, 141),
                (1, 1),
                (2, 4),
                (3, 5),
                (4, 20),
                (5, 5),
                (6, 12),
                (7, 7),
                (8, 11),
                (9, 7),
                (10, 4),
                (11, 2),
                (12, 5),
                (13, 5),
                (14, 1),
                (16, 1),
                (17, 3),
            ])
        );
        let (mode, half_segment, radius, center, selector, hierarchy_chain) =
            rodney_capsule.expect("p01_rod Rodney MapCollision capsule");
        assert_eq!(mode, 3);
        assert!((half_segment - 0.45).abs() <= 1.0e-6);
        assert!((radius - 0.40).abs() <= 1.0e-6);
        assert!((center[0] - 0.0).abs() <= 1.0e-6);
        assert!((center[1] - 0.85).abs() <= 1.0e-6);
        assert!((center[2] - 0.0).abs() <= 1.0e-6);
        assert_eq!(selector, 0);
        assert_eq!(hierarchy_chain, [0]);
        eprintln!(
            "Robots v248 AnimSkin corpus: skins={skins} bones={total_bones} animdatum_sections={animdatum_sections_nonempty} raw_animdatums={animdatum_entries} searchable_animdatums={animdatum_searchable_entries} map_collision_raw={} map_collision_searchable={} rodney_capsule=half:{half_segment:.3}/radius:{radius:.3}/selector:{selector}/hierarchy:{hierarchy_chain:?} animbone_pairs={animbone_pairs} post_pair_blocks={post_pair_blocks} scalar_groups={scalar_groups} rigid_attachments={bone_attachments}",
            map_collision_raw_modes.values().sum::<usize>(),
            map_collision_head_modes.values().sum::<usize>(),
        );
    }

    fn collect_edb_files(root: &Path, output: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(root) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_edb_files(&path, output);
            } else if path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("edb"))
            {
                output.push(path);
            }
        }
    }
}
