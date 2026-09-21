#![allow(non_camel_case_types)]
use binrw::{binrw, BinRead, BinReaderExt};
use serde::Serialize;

/// Robots.exe 0x005152ED/0x0051602F/0x00518E6B route serialized
/// `EXGeoBaseEntity.flags & 0x10` to D3D render-state 0x1C (FOGENABLE) with
/// value zero. It is a per-entity no-fog flag, not a transform/camera flag.
pub const ROBOTS_ENTITY_FLAG_NO_FOG: u32 = 0x10;

use crate::{
    common::{EXRelPtr, EXVector, EXVector3},
    entity_mesh::EXGeoMeshEntity,
    versions::Platform,
};

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
pub struct RobotsMeshDirectoryEntry {
    pub id: u8,
    pub relative_offset: u32,
    pub entry_offset_absolute: u64,
}

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
pub struct RobotsEntityDirectoryEntry {
    pub id: u8,
    /// Low byte of the packed per-directory entry. Native `0x00504D0B` does
    /// not consume it semantically, so keep it raw rather than invent a name.
    pub packed_low_byte: u8,
    pub relative_offset: u32,
    pub entry_offset_absolute: u64,
}

impl RobotsEntityDirectoryEntry {
    pub fn data_offset_absolute(&self) -> u64 {
        self.entry_offset_absolute + u64::from(self.relative_offset)
    }
}

#[derive(Debug, Serialize, Clone, Copy, PartialEq)]
pub struct RobotsEntityAnimDatumRecord {
    pub hashcode: u32,
    /// Serialized record +0x04. Runtime transformers preserve this at datum
    /// +0x2C, but its designer-facing meaning is not proven yet.
    pub raw_word_04: u16,
    /// Serialized record +0x06. Native map-collision conversion uses mode 3
    /// for capsule and the compact sphere path for other values.
    pub shape_mode: u8,
    pub raw_byte_07: u8,
    /// Serialized +0x08/+0x0C/+0x10. For HT_AnimDatum_MapCollisionCapsule,
    /// runtime mode != 3 uses scalar[0] as sphere radius; mode 3 uses scalar[0]
    /// as capsule half-segment length and scalar[1] as radius.
    pub shape_scalars: [f32; 3],
    /// Serialized local center at +0x14/+0x18/+0x1C.
    pub local_center: [f32; 3],
    /// Serialized local orientation quaternion at +0x20..+0x2C.
    pub local_orientation: [f32; 4],
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct RobotsEntityAnimDatumDirectory {
    /// Native consumers only prove byte +5 as count and i16 +6 as record
    /// self-relative pointer. Preserve the preceding bytes verbatim.
    pub header_raw_00_04: [u8; 5],
    pub record_count: u8,
    pub records_relative_offset: i16,
    pub records: Vec<RobotsEntityAnimDatumRecord>,
}

impl RobotsMeshDirectoryEntry {
    pub fn data_offset_absolute(&self) -> u64 {
        self.entry_offset_absolute + u64::from(self.relative_offset)
    }
}

/// Resolves one native Robots v248 Entity optional-directory entry from the
/// compact directory root at serialized object +0x40.
///
/// `Robots.exe::0x00504D0B` treats the low byte of the root DWORD as an
/// eight-bit presence mask and the upper 24 bits as a self-relative offset to
/// the packed entries. Entries are ordered by set-bit index; their upper 24
/// bits are self-relative offsets to the actual directory payload.
pub fn read_robots_entity_directory_entry<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    endian: binrw::Endian,
    entity_object_address: u64,
    wanted_id: u8,
) -> binrw::BinResult<Option<RobotsEntityDirectoryEntry>> {
    let saved_position = reader.stream_position()?;
    let result = (|| {
        if wanted_id >= 8 {
            return Ok(None);
        }
        let root_address = entity_object_address + 0x40;
        reader.seek(std::io::SeekFrom::Start(root_address))?;
        let root = reader.read_type::<u32>(endian)?;
        let mask = (root & 0xff) as u8;
        if mask & (1u8 << wanted_id) == 0 {
            return Ok(None);
        }
        let entries_relative = root >> 8;
        if entries_relative == 0 {
            return Ok(None);
        }
        let entries_address = root_address + u64::from(entries_relative);
        let lower_mask = if wanted_id == 0 {
            0
        } else {
            mask & ((1u8 << wanted_id) - 1)
        };
        let entry_index = lower_mask.count_ones() as u64;
        let entry_offset_absolute = entries_address + entry_index * 4;
        reader.seek(std::io::SeekFrom::Start(entry_offset_absolute))?;
        let packed = reader.read_type::<u32>(endian)?;
        let relative_offset = packed >> 8;
        if relative_offset == 0 {
            return Ok(None);
        }
        Ok(Some(RobotsEntityDirectoryEntry {
            id: wanted_id,
            packed_low_byte: (packed & 0xff) as u8,
            relative_offset,
            entry_offset_absolute,
        }))
    })();
    reader.seek(std::io::SeekFrom::Start(saved_position))?;
    result
}

/// Reads Robots v248 Entity optional-directory ID 5, the exact static
/// 0x30-byte AnimDatum array consumed by `EXItemAnimator_Entity::slot12`
/// (`0x00507219`).
pub fn read_robots_v248_entity_anim_datums<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    endian: binrw::Endian,
    entity_object_address: u64,
) -> binrw::BinResult<Option<RobotsEntityAnimDatumDirectory>> {
    let saved_position = reader.stream_position()?;
    let result = (|| {
        let Some(directory) =
            read_robots_entity_directory_entry(reader, endian, entity_object_address, 5)?
        else {
            return Ok(None);
        };
        let directory_address = directory.data_offset_absolute();
        reader.seek(std::io::SeekFrom::Start(directory_address))?;
        let mut header_raw_00_04 = [0u8; 5];
        reader.read_exact(&mut header_raw_00_04)?;
        let record_count = reader.read_type::<u8>(endian)?;
        let records_relative_offset = reader.read_type::<i16>(endian)?;
        if record_count == 0 {
            return Ok(Some(RobotsEntityAnimDatumDirectory {
                header_raw_00_04,
                record_count,
                records_relative_offset,
                records: Vec::new(),
            }));
        }
        if records_relative_offset == 0 {
            return Err(binrw::Error::AssertFail {
                pos: directory_address + 6,
                message: format!(
                    "Robots Entity AnimDatum directory has {record_count} records but a null record pointer"
                ),
            });
        }
        let records_base = (directory_address as i64 + 6)
            .checked_add(i64::from(records_relative_offset))
            .filter(|address| *address >= 0)
            .ok_or_else(|| binrw::Error::AssertFail {
                pos: directory_address + 6,
                message: "Robots Entity AnimDatum record pointer underflow".to_string(),
            })? as u64;
        reader.seek(std::io::SeekFrom::Start(records_base))?;
        let mut records = Vec::with_capacity(usize::from(record_count));
        for _ in 0..record_count {
            records.push(RobotsEntityAnimDatumRecord {
                hashcode: reader.read_type::<u32>(endian)?,
                raw_word_04: reader.read_type::<u16>(endian)?,
                shape_mode: reader.read_type::<u8>(endian)?,
                raw_byte_07: reader.read_type::<u8>(endian)?,
                shape_scalars: [
                    reader.read_type::<f32>(endian)?,
                    reader.read_type::<f32>(endian)?,
                    reader.read_type::<f32>(endian)?,
                ],
                local_center: [
                    reader.read_type::<f32>(endian)?,
                    reader.read_type::<f32>(endian)?,
                    reader.read_type::<f32>(endian)?,
                ],
                local_orientation: [
                    reader.read_type::<f32>(endian)?,
                    reader.read_type::<f32>(endian)?,
                    reader.read_type::<f32>(endian)?,
                    reader.read_type::<f32>(endian)?,
                ],
            });
        }
        Ok(Some(RobotsEntityAnimDatumDirectory {
            header_raw_00_04,
            record_count,
            records_relative_offset,
            records,
        }))
    })();
    reader.seek(std::io::SeekFrom::Start(saved_position))?;
    result
}

/// Robots.exe `FUN_005134D0` tests the bitmask at object +0x88, then scans
/// packed DWORDs from +0x90. The low byte is the directory ID and the upper
/// 24 bits are an unsigned self-relative byte offset from that DWORD.
pub fn read_robots_mesh_directory_entry<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    endian: binrw::Endian,
    mesh_object_address: u64,
    wanted_id: u8,
) -> binrw::BinResult<Option<RobotsMeshDirectoryEntry>> {
    let saved_position = reader.stream_position()?;
    let result = (|| {
        reader.seek(std::io::SeekFrom::Start(mesh_object_address + 0x88))?;
        let directory_mask = reader.read_type::<u16>(endian)?;
        if wanted_id >= 16 || directory_mask & (1u16 << wanted_id) == 0 {
            return Ok(None);
        }
        reader.seek(std::io::SeekFrom::Start(mesh_object_address + 0x90))?;
        for _ in 0..directory_mask.count_ones() {
            let entry_offset_absolute = reader.stream_position()?;
            let packed = reader.read_type::<u32>(endian)?;
            let id = (packed & 0xff) as u8;
            let relative_offset = packed >> 8;
            if id == wanted_id {
                return Ok(Some(RobotsMeshDirectoryEntry {
                    id,
                    relative_offset,
                    entry_offset_absolute,
                }));
            }
        }
        Err(binrw::Error::AssertFail {
            pos: mesh_object_address + 0x90,
            message: format!(
                "Robots Mesh directory mask 0x{directory_mask:04X} advertises id {wanted_id} but no packed entry exists"
            ),
        })
    })();
    reader.seek(std::io::SeekFrom::Start(saved_position))?;
    result
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct RobotsFaceInfoRecord {
    /// Native `0x004F4EFC` reads these three u16 values as the triangle's vertex indices.
    pub vertex_indices: [u16; 3],
    /// Native `0x0041C2C0` consumes this exact word through the contact record's face pointer/index.
    pub surface_metadata: u16,
    /// Fifth u16 in the native 10-byte face record; no semantic name is proven yet.
    pub trailing_raw: u16,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct RobotsFaceInfoGroup {
    /// First u16 in the native per-group header. Collision consumers do not name it.
    pub header_raw: u16,
    /// Second u16 in the group header is the exact face count used by `0x004F4EFC`.
    pub faces: Vec<RobotsFaceInfoRecord>,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct RobotsFaceInfo {
    /// Native v248 face-info stream starts with four u16 values; only word 0 is proven as group count.
    pub header_raw: [u16; 4],
    pub groups: Vec<RobotsFaceInfoGroup>,
}

/// Exact Robots v248 surface-category mapping from the contact-face metadata OR accumulator
/// consumed by `Robots.exe::0x0041C2C0`.
pub fn robots_v248_surface_category_from_contact_or(metadata_or: u16) -> (u16, &'static str) {
    match metadata_or & 0x78 {
        0x08 => (0x0001, "slippery_floor_primary"),
        0x10 => (0x0002, "electric_floor"),
        0x38 => (0x0004, "playerball_death_surface"),
        0x28 => (0x0020, "slime_floor"),
        0x60 => (0x0080, "unresolved_surface_category"),
        0x68 => (0x0100, "slippery_floor_secondary"),
        _ if metadata_or & 0x40 != 0 => (0x0040, "playerball_chase_oily_surface"),
        _ => (0, "no_special_runtime_category"),
    }
}

/// Single-face diagnostic view of the native mapping. Runtime may OR several touching faces first.
pub fn robots_v248_isolated_surface_category(face_metadata: u16) -> (u16, &'static str) {
    let (category, role) = robots_v248_surface_category_from_contact_or(face_metadata);
    if category == 0 {
        (0, "no_special_runtime_category_in_isolation")
    } else {
        (category, role)
    }
}

/// Reads the Robots PC v248 `EXGeoMeshEntityData.face_info` stream at serialized +0x68.
///
/// `Robots.exe::0x004F4EFC` resolves this exact pointer, skips the four-u16 global header,
/// then walks `{u16 raw, u16 face_count}` group headers followed by `face_count` records of
/// five u16 values. `0x0041C2C0` proves record word 3 is the surface metadata used for
/// slippery/electric/slime/oily/etc. classification.
pub fn read_robots_v248_face_info<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    endian: binrw::Endian,
    face_info_address: u64,
) -> binrw::BinResult<RobotsFaceInfo> {
    let saved_position = reader.stream_position()?;
    let result = (|| {
        reader.seek(std::io::SeekFrom::Start(face_info_address))?;
        let header_raw = [
            reader.read_type::<u16>(endian)?,
            reader.read_type::<u16>(endian)?,
            reader.read_type::<u16>(endian)?,
            reader.read_type::<u16>(endian)?,
        ];
        let group_count = usize::from(header_raw[0]);
        if group_count > 4096 {
            return Err(binrw::Error::AssertFail {
                pos: face_info_address,
                message: format!("implausible Robots face-info group count {group_count}"),
            });
        }

        let mut groups = Vec::with_capacity(group_count);
        let mut total_faces = 0usize;
        for _ in 0..group_count {
            let header_raw_group = reader.read_type::<u16>(endian)?;
            let face_count = usize::from(reader.read_type::<u16>(endian)?);
            total_faces =
                total_faces
                    .checked_add(face_count)
                    .ok_or_else(|| binrw::Error::AssertFail {
                        pos: reader.stream_position().unwrap_or(face_info_address),
                        message: "Robots face-info face count overflow".to_string(),
                    })?;
            if total_faces > 1_000_000 {
                return Err(binrw::Error::AssertFail {
                    pos: reader.stream_position()?,
                    message: format!("implausible Robots face-info face total {total_faces}"),
                });
            }

            let mut faces = Vec::with_capacity(face_count);
            for _ in 0..face_count {
                faces.push(RobotsFaceInfoRecord {
                    vertex_indices: [
                        reader.read_type::<u16>(endian)?,
                        reader.read_type::<u16>(endian)?,
                        reader.read_type::<u16>(endian)?,
                    ],
                    surface_metadata: reader.read_type::<u16>(endian)?,
                    trailing_raw: reader.read_type::<u16>(endian)?,
                });
            }
            groups.push(RobotsFaceInfoGroup {
                header_raw: header_raw_group,
                faces,
            });
        }
        Ok(RobotsFaceInfo { header_raw, groups })
    })();
    reader.seek(std::io::SeekFrom::Start(saved_position))?;
    result
}

/// Reads the additive morph-position samples used by shipped Robots PC v248.
///
/// Native `FUN_005186BD` requests Mesh directory ID 2. Its payload is an array
/// of self-relative i32 shape pointers. Each shape stores one 0x10-byte record
/// per mesh vertex; only XYZ at +0/+4/+8 are added to the base vertex position.
pub fn read_robots_v248_morph_shape_deltas<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    endian: binrw::Endian,
    mesh_object_address: u64,
    vertex_count: usize,
    shape_count: usize,
) -> binrw::BinResult<Vec<Vec<EXVector3>>> {
    let saved_position = reader.stream_position()?;
    let result = (|| {
        if shape_count == 0 {
            return Ok(Vec::new());
        }
        if shape_count > 4096 {
            return Err(binrw::Error::AssertFail {
                pos: mesh_object_address,
                message: format!("implausible Robots morph shape count {shape_count}"),
            });
        }
        let directory = read_robots_mesh_directory_entry(reader, endian, mesh_object_address, 2)?
            .ok_or_else(|| binrw::Error::AssertFail {
            pos: mesh_object_address + 0x88,
            message: "Robots morph-bearing Mesh has no directory ID 2".to_string(),
        })?;
        let directory_address = directory.data_offset_absolute();
        let mut shapes = Vec::with_capacity(shape_count);
        for shape_index in 0..shape_count {
            let pointer_address = directory_address + shape_index as u64 * 4;
            reader.seek(std::io::SeekFrom::Start(pointer_address))?;
            let relative = reader.read_type::<i32>(endian)?;
            let shape_address = (pointer_address as i64)
                .checked_add(i64::from(relative))
                .filter(|address| *address >= 0)
                .ok_or_else(|| binrw::Error::AssertFail {
                    pos: pointer_address,
                    message: format!("invalid Robots morph shape pointer {relative}"),
                })? as u64;
            reader.seek(std::io::SeekFrom::Start(shape_address))?;
            let mut deltas = Vec::with_capacity(vertex_count);
            for _ in 0..vertex_count {
                let x = reader.read_type::<f32>(endian)?;
                let y = reader.read_type::<f32>(endian)?;
                let z = reader.read_type::<f32>(endian)?;
                let _unused = reader.read_type::<f32>(endian)?;
                if !x.is_finite() || !y.is_finite() || !z.is_finite() {
                    return Err(binrw::Error::AssertFail {
                        pos: reader.stream_position()?.saturating_sub(16),
                        message: "non-finite Robots morph shape delta".to_string(),
                    });
                }
                deltas.push([x, y, z]);
            }
            shapes.push(deltas);
        }
        Ok(shapes)
    })();
    reader.seek(std::io::SeekFrom::Start(saved_position))?;
    result
}

#[binrw]
#[derive(Debug, Serialize, Clone)]
#[brw(import(version: u32))]
// Robots PC v248 serialized base layout is native-proven at object offsets
// +0x04..+0x53. Earlier EngineX layouts remain version-specific and are outside
// the Robots-only contract; do not generalize the v248 layout backwards.
pub struct EXGeoBaseEntity {
    pub flags: u32,       // 0x4
    pub sort_value: u16,  // 0x8
    pub render_order: u8, // 0xa
    #[serde(skip)]
    _pad0: u8, // 0xb
    pub surface_area: f32, // 0xc
    pub bounds_box: [EXVector; 2], // 0x10
    _unk30: [u32; 4],     // 0x30
    #[brw(if(version > 221))]
    _unk40: [u32; 4],
    pub gdi_count: u16, // 0x50
    pub gdi_index: u16, // 0x52
}

#[binrw]
#[derive(Debug, Serialize, Clone)]
#[brw(import(version: u32, platform: Platform))]
pub struct EXGeoMeshEntityData {
    #[brw(args(version))]
    pub base: EXGeoBaseEntity, // 0x0

    pub texture_list: EXRelPtr<EXGeoEntity_TextureList>, // 0x54
    pub tristrip_data_offset: EXRelPtr,                  // 0x58 / Is a weird format on PS2
    pub vertex_data_offset: EXRelPtr,                    // 0x5c / 0x60

    #[brw(if(platform == Platform::GameCube || platform == Platform::Wii))]
    pub texture_coordinates: Option<EXRelPtr>, // 0x60

    #[brw(if(platform != Platform::Ps2))]
    pub vertex_color_offset: Option<EXRelPtr>, // 0x60 / on ps2 this is included in tristrip_data
    #[brw(if(platform != Platform::Ps2))]
    pub face_collision: Option<EXRelPtr>, // 0x64 / not on ps2
    #[brw(if(platform != Platform::Ps2))]
    pub face_info: Option<EXRelPtr>, // 0x68 / not on ps2

    pub index_data: EXRelPtr, // 0x6c / 0x64 on ps2

    pub _unk70: u32, // 0x70 / 0x64

    #[brw(if(platform == Platform::GameCube || platform == Platform::Wii))]
    _unk74: u32, // 0x74

    #[brw(if(platform == Platform::Wii))]
    _unk78: [f32; 10], // ???

    // TODO: Can we make this less fucky?
    #[brw(if(platform == Platform::Ps2))]
    tristrip_count_ps2: u16, // 0x68
    #[brw(if(platform == Platform::Ps2))]
    vertex_count_ps2: u16, // 0x6a
    #[brw(if(platform == Platform::Ps2))]
    index_count_ps2: u16, // 0x6d

    #[brw(if(platform != Platform::Ps2))]
    tristrip_count_all: u32, // 0x74
    #[brw(if(platform != Platform::Ps2))]
    vertex_count_all: u32, // 0x78
    #[brw(if(platform != Platform::Ps2))]
    _unk7c_all: u32, // 0x7c
    #[brw(if(platform != Platform::Ps2))]
    index_count_all: u32, // 0x80

    #[br(calc = if platform == Platform::Ps2 { tristrip_count_ps2 as u32 } else { tristrip_count_all })]
    pub tristrip_count: u32,
    #[br(calc = if platform == Platform::Ps2 { vertex_count_ps2 as u32 } else { vertex_count_all })]
    pub vertex_count: u32,
    #[br(calc = if platform == Platform::Ps2 { 0 } else { _unk7c_all })]
    pub _unk7c: u32,
    #[br(calc = if platform == Platform::Ps2 { index_count_ps2 as u32 } else { index_count_all })]
    pub index_count: u32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RobotsEntityTypeStats {
    pub mesh: usize,
    pub split: usize,
    pub instance: usize,
    pub navmesh: usize,
    pub mapzone: usize,
    pub unknown: usize,
}

static ROBOTS_ENTITY_MESH: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static ROBOTS_ENTITY_SPLIT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static ROBOTS_ENTITY_INSTANCE: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
static ROBOTS_ENTITY_NAVMESH: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
static ROBOTS_ENTITY_MAPZONE: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
static ROBOTS_ENTITY_UNKNOWN: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

pub fn robots_entity_type_stats() -> RobotsEntityTypeStats {
    use std::sync::atomic::Ordering;
    RobotsEntityTypeStats {
        mesh: ROBOTS_ENTITY_MESH.load(Ordering::Relaxed),
        split: ROBOTS_ENTITY_SPLIT.load(Ordering::Relaxed),
        instance: ROBOTS_ENTITY_INSTANCE.load(Ordering::Relaxed),
        navmesh: ROBOTS_ENTITY_NAVMESH.load(Ordering::Relaxed),
        mapzone: ROBOTS_ENTITY_MAPZONE.load(Ordering::Relaxed),
        unknown: ROBOTS_ENTITY_UNKNOWN.load(Ordering::Relaxed),
    }
}

pub fn reset_robots_entity_type_stats() {
    use std::sync::atomic::Ordering;
    ROBOTS_ENTITY_MESH.store(0, Ordering::Relaxed);
    ROBOTS_ENTITY_SPLIT.store(0, Ordering::Relaxed);
    ROBOTS_ENTITY_INSTANCE.store(0, Ordering::Relaxed);
    ROBOTS_ENTITY_NAVMESH.store(0, Ordering::Relaxed);
    ROBOTS_ENTITY_MAPZONE.store(0, Ordering::Relaxed);
    ROBOTS_ENTITY_UNKNOWN.store(0, Ordering::Relaxed);
    reset_robots_navmesh_stats();
}

fn record_robots_entity_type(obj_type: u32, version: u32) {
    if version != 248 {
        return;
    }
    use std::sync::atomic::Ordering;
    match obj_type {
        0x601 => {
            ROBOTS_ENTITY_MESH.fetch_add(1, Ordering::Relaxed);
        }
        0x603 => {
            ROBOTS_ENTITY_SPLIT.fetch_add(1, Ordering::Relaxed);
        }
        0x606 => {
            ROBOTS_ENTITY_INSTANCE.fetch_add(1, Ordering::Relaxed);
        }
        0x607 => {
            ROBOTS_ENTITY_NAVMESH.fetch_add(1, Ordering::Relaxed);
        }
        0x608 => {
            ROBOTS_ENTITY_MAPZONE.fetch_add(1, Ordering::Relaxed);
        }
        0x600..=0x6ff => {
            ROBOTS_ENTITY_UNKNOWN.fetch_add(1, Ordering::Relaxed);
        }
        _ => {}
    }
}
#[derive(Debug, Clone, Copy, Default)]
pub struct RobotsNavMeshStats {
    pub objects: usize,
    pub vertices: usize,
    pub faces: usize,
    pub groups: usize,
}

static ROBOTS_NAVMESH_OBJECTS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
static ROBOTS_NAVMESH_VERTICES: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
static ROBOTS_NAVMESH_FACES: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
static ROBOTS_NAVMESH_GROUPS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

static ROBOTS_INSTANCE_BOUNDS_VISIBLE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub fn robots_instance_bounds_visible() -> bool {
    ROBOTS_INSTANCE_BOUNDS_VISIBLE.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn set_robots_instance_bounds_visible(visible: bool) {
    ROBOTS_INSTANCE_BOUNDS_VISIBLE.store(visible, std::sync::atomic::Ordering::Relaxed);
}
pub fn robots_navmesh_stats() -> RobotsNavMeshStats {
    use std::sync::atomic::Ordering;
    RobotsNavMeshStats {
        objects: ROBOTS_NAVMESH_OBJECTS.load(Ordering::Relaxed),
        vertices: ROBOTS_NAVMESH_VERTICES.load(Ordering::Relaxed),
        faces: ROBOTS_NAVMESH_FACES.load(Ordering::Relaxed),
        groups: ROBOTS_NAVMESH_GROUPS.load(Ordering::Relaxed),
    }
}

pub fn reset_robots_navmesh_stats() {
    use std::sync::atomic::Ordering;
    ROBOTS_NAVMESH_OBJECTS.store(0, Ordering::Relaxed);
    ROBOTS_NAVMESH_VERTICES.store(0, Ordering::Relaxed);
    ROBOTS_NAVMESH_FACES.store(0, Ordering::Relaxed);
    ROBOTS_NAVMESH_GROUPS.store(0, Ordering::Relaxed);
}

pub fn record_robots_navmesh_stats(entity: &EXGeoNavMeshEntity) {
    use std::sync::atomic::Ordering;
    ROBOTS_NAVMESH_OBJECTS.fetch_add(1, Ordering::Relaxed);
    ROBOTS_NAVMESH_VERTICES.fetch_add(entity.vertex_count as usize, Ordering::Relaxed);
    ROBOTS_NAVMESH_FACES.fetch_add(entity.face_count as usize, Ordering::Relaxed);
    ROBOTS_NAVMESH_GROUPS.fetch_add(entity.group_count as usize, Ordering::Relaxed);
}
// ROBOTS_PATCH_0019_INSTANCE_SELECTOR_SCHEMA
// ROBOTS_PATCH_0025_INSTANCE_INLINE_STRIP_RENDERING
//
// Robots PC EDB v248 EXGeoInstanceEntity is an inline textured triangle strip.
// Static slot-3 code passes:
//   PrimitiveType = 5 (triangle strip)
//   PrimitiveCount = [instance + 0x54]
//   vertex data = instance + 0x60
//   vertex stride = 0x24
//
// The +0x58 paged selector resolves into the current EDB texture-list header
// at the same numeric index. The old page/slot helpers remain valid because the
// game wraps that texture list in a runtime paged table.
#[binrw]
#[derive(Debug, Serialize, Clone)]
pub struct EXGeoInstanceVertex {
    pub position: [f32; 3], // +0x00
    pub normal: [f32; 3],   // +0x0C
    pub color: [u8; 4],     // +0x18
    pub uv: [f32; 2],       // +0x1C
}

#[binrw]
#[derive(Debug, Serialize, Clone)]
#[brw(import(version: u32))]
pub struct EXGeoInstanceEntity {
    #[brw(args(version))]
    pub base: EXGeoBaseEntity, // serialized object offsets 0x04..0x53

    #[brw(if(version == 248))]
    pub robots_v248_primitive_count: u32, // +0x54, DrawPrimitiveUP PrimitiveCount

    #[brw(if(version == 248))]
    pub robots_v248_selector: u32, // +0x58, texture-list index / runtime paged texture selector

    #[brw(if(version == 248))]
    pub robots_v248_raw_5c: u32, // +0x5C; upper u16 controls blend path, low u16 still unnamed

    #[br(count = if version == 248 {
        robots_v248_primitive_count.saturating_add(2)
    } else {
        0
    })]
    #[bw(if(version == 248))]
    pub robots_v248_vertices: Vec<EXGeoInstanceVertex>, // +0x60, inline 0x24-byte vertex stream
}

impl EXGeoInstanceEntity {
    pub fn robots_selector_page(&self) -> u32 {
        self.robots_v248_selector >> 6
    }

    pub fn robots_selector_slot(&self) -> u32 {
        self.robots_v248_selector & 0x3f
    }

    pub fn robots_selector_slot_offset(&self) -> u32 {
        self.robots_selector_slot() * 0x38
    }

    pub fn robots_texture_index(&self) -> usize {
        self.robots_v248_selector as usize
    }

    pub fn robots_blend_mode(&self) -> u16 {
        (self.robots_v248_raw_5c >> 16) as u16
    }

    pub fn robots_vertex_count(&self) -> u32 {
        self.robots_v248_primitive_count.saturating_add(2)
    }
}

impl std::ops::Deref for EXGeoInstanceEntity {
    type Target = EXGeoBaseEntity;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

pub const ROBOTS_INSTANCE_SELECTOR_OFFSET: u32 = 0x58;
pub const ROBOTS_INSTANCE_SELECTOR_PAGE_SHIFT: u32 = 6;
pub const ROBOTS_INSTANCE_SELECTOR_SLOT_MASK: u32 = 0x3f;
pub const ROBOTS_INSTANCE_SELECTOR_RECORD_STRIDE: u32 = 0x38;
#[binrw]
#[derive(Debug, Serialize, Clone)]
#[brw(import(version: u32))]
pub struct EXGeoNavMeshEntity {
    #[brw(args(version))]
    pub base: EXGeoBaseEntity, // 0x04..0x53

    // Robots v248: six unknown dwords before the array counts.
    pub raw_54_68: [u32; 6], // 0x54..0x6b

    pub vertex_count: u32, // 0x6c
    pub face_count: u32,   // 0x70
    pub group_count: u32,  // 0x74

    pub vertices: EXRelPtr,  // 0x78, vertex_count * [f32; 3]
    pub faces: EXRelPtr,     // 0x7c, face_count * [u32; 4]
    pub adjacency: EXRelPtr, // 0x80, face_count * [u32; 3]
    pub groups: EXRelPtr,    // 0x84, group_count * [u32; 2]

    // Keep all currently-unknown tail fields raw. The final words contain
    // 0x0BADF00D..0x0BADF010 sentinels in Robots v248 samples.
    pub raw_88_b4: [u32; 12], // 0x88..0xb7
}
// ROBOTS_PATCH_0030_MAPZONE_NATIVE_LAYOUT
// ROBOTS_PATCH_0031_MAPZONE_SERIALIZED_LAYOUT_REPAIR
//
// PATCH_0030 recovered a valid *runtime object* layout from the exact original
// Robots.exe, but applying that 0x2D8 native layout directly to serialized EDB
// records was not proven and caused a visual regression (missing map geometry).
//
// Keep the native evidence as metadata only. Do NOT use it to consume bytes from
// the EDB stream until a native loader/fixup routine proves the serialized format.
pub const ROBOTS_MAPZONE_NATIVE_MAX_ENTRIES: usize = 32;
pub const ROBOTS_MAPZONE_NATIVE_ENTRY_STRIDE: usize = 0x14;
pub const ROBOTS_MAPZONE_NATIVE_OBJECT_SIZE: usize = 0x2D8;

#[binrw]
#[derive(Debug, Serialize, Clone)]
pub struct EXGeoMapZoneEntryRaw {
    pub raw_00: u32,
    pub raw_04: u32,
    pub raw_08: u32,
    pub raw_0c: u32,
    pub raw_10: u32,
}

// Native-layout proof container only. This is intentionally NOT BinRead/BinWrite
// and therefore cannot advance the serialized EDB stream.
#[derive(Debug, Serialize, Clone)]
pub struct EXGeoMapZoneNativeLayoutProof {
    pub robots_v248_active_count: u8,
    pub robots_v248_raw_55_57: [u8; 3],
    pub robots_v248_entries: Vec<EXGeoMapZoneEntryRaw>,
}

impl EXGeoMapZoneNativeLayoutProof {
    pub fn robots_v248_active_entries(&self) -> &[EXGeoMapZoneEntryRaw] {
        let count = usize::from(self.robots_v248_active_count).min(self.robots_v248_entries.len());
        &self.robots_v248_entries[..count]
    }
}

#[binrw]
#[derive(Debug, Serialize, Clone)]
#[brw(import(version: u32, _platform: Platform))]
pub struct EXGeoMapZoneEntity {
    #[brw(args(version))]
    pub base: EXGeoBaseEntity, // serialized base payload

    // Serialized layout restored after PATCH_0030 regression.
    // Keep these raw until the actual v248 disk loader/fixup path is proven.
    pub _unk54: u32,        // serialized field at +0x54
    pub entity_refptr: u32, // serialized field at +0x58
}

#[binrw]
#[derive(Debug, Serialize, Clone)]
#[brw(import(version: u32, platform: Platform))]
pub struct EXGeoSplitEntity {
    #[brw(args(version))]
    pub base: EXGeoBaseEntity, // 0x0

    // TODO(cohae): Older games have different limits, how do we handle that when writing files?
    #[brw(assert(entity_count.le(&1024)))]
    pub entity_count: u32, // 0x54

    #[brw(if(version.gt(&213)))]
    _unk58: u32,

    #[br(count = entity_count, args { inner: (version, platform) })]
    pub entities: Vec<EXRelPtr<EXGeoEntity>>, // 0x5c
}

#[binrw]
#[derive(Debug, Serialize, Clone)]
pub struct EXGeoEntity_TextureList {
    #[serde(skip)]
    pub texture_count: u16,

    #[br(count = texture_count)]
    pub textures: Vec<u16>,
}

#[derive(Debug, Serialize, Clone)]
pub enum EXGeoEntity {
    Mesh(EXGeoMeshEntity),
    Split(EXGeoSplitEntity),
    MapZone(EXGeoMapZoneEntity),
    Instance(EXGeoInstanceEntity), // Robots v248 payload decoded conditionally; non-v248 keeps base-only read
    NavMesh(EXGeoNavMeshEntity),
    UnknownType(u32),
}

impl EXGeoEntity {
    pub fn base(&self) -> Option<&EXGeoBaseEntity> {
        match self {
            EXGeoEntity::Mesh(e) => Some(&e.data.base),
            EXGeoEntity::Split(e) => Some(&e.base),
            EXGeoEntity::MapZone(e) => Some(&e.base),
            EXGeoEntity::Instance(e) => Some(&e.base),
            EXGeoEntity::NavMesh(e) => Some(&e.base),
            EXGeoEntity::UnknownType(_e) => None,
        }
    }

    pub fn type_code(&self) -> u32 {
        match self {
            EXGeoEntity::Mesh { .. } => 0x601,
            EXGeoEntity::Split { .. } => 0x603,
            EXGeoEntity::Instance { .. } => 0x606,
            EXGeoEntity::NavMesh { .. } => 0x607,
            EXGeoEntity::MapZone { .. } => 0x608,
            EXGeoEntity::UnknownType(ty) => *ty,
        }
    }
}

impl BinRead for EXGeoEntity {
    type Args<'a> = (u32, Platform);

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let obj_type: u32 = reader.read_type(endian)?;
        record_robots_entity_type(obj_type, args.0);

        Ok(match obj_type {
            0x601 => EXGeoEntity::Mesh(reader.read_type_args(endian, args)?),
            0x603 => EXGeoEntity::Split(reader.read_type_args(endian, args)?),
            0x606 => EXGeoEntity::Instance(reader.read_type_args(endian, (args.0,))?),
            0x607 if args.0 == 248 => {
                EXGeoEntity::NavMesh(reader.read_type_args(endian, (args.0,))?)
            }
            0x608 => EXGeoEntity::MapZone(reader.read_type_args(endian, args)?),
            t @ 0x600..=0x6ff => EXGeoEntity::UnknownType(t),
            _ => {
                return Err(binrw::Error::NoVariantMatch {
                    pos: reader.stream_position()?,
                })
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        fs::File,
        io::{BufReader, Read, Seek, SeekFrom},
        path::Path,
    };

    use super::*;
    use crate::{edb::EdbFile, map::EXGeoMap};

    fn collect_robots_v248_spatial_leaf_flags(
        edb: &mut EdbFile,
        address: u64,
        depth: usize,
        out: &mut Vec<(u16, u8)>,
    ) {
        assert!(
            depth < 64,
            "Robots placement spatial tree recursion overflow"
        );
        edb.seek(SeekFrom::Start(address))
            .expect("seek Robots placement spatial node");
        let mut header = [0u8; 4];
        edb.read_exact(&mut header)
            .expect("read Robots placement spatial node header");
        let child_count = header[0] as usize;
        let leaf_count = u16::from_le_bytes([header[2], header[3]]) as usize;
        if child_count == 0 {
            for item_index in 0..leaf_count {
                edb.seek(SeekFrom::Start(address + 4 + item_index as u64 * 4))
                    .expect("seek Robots placement spatial leaf item");
                let mut item = [0u8; 4];
                edb.read_exact(&mut item)
                    .expect("read Robots placement spatial leaf item");
                out.push((u16::from_le_bytes([item[0], item[1]]), item[2]));
            }
            return;
        }

        for child_index in 0..child_count {
            let relptr_address = address + 4 + child_index as u64 * 0x1c;
            edb.seek(SeekFrom::Start(relptr_address))
                .expect("seek Robots placement spatial child relptr");
            let relative = edb
                .read_type::<i32>(binrw::Endian::Little)
                .expect("read Robots placement spatial child relptr");
            if relative == 0 {
                continue;
            }
            collect_robots_v248_spatial_leaf_flags(
                edb,
                (relptr_address as i64 + i64::from(relative)) as u64,
                depth + 1,
                out,
            );
        }
    }

    #[test]
    fn real_robots_v248_m02_city_placement_spatial_leaf_flags_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/m02_city.edb");
        let file = File::open(&path).expect("open m02_city.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse m02_city.edb");
        assert_eq!(edb.header.version, 248);
        let map_header = edb.header.map_list.data()[0].clone();
        edb.seek(SeekFrom::Start(map_header.address as u64))
            .expect("seek m02 EXGeoMap");
        let endian = edb.endian;
        let map = edb
            .read_type_args::<EXGeoMap>(endian, (248,))
            .expect("parse m02 EXGeoMap");

        let mut histogram = BTreeMap::<u8, usize>::new();
        let mut total = 0usize;
        let mut raycast = 0usize;
        let mut typed_raycast_entries = Vec::<(usize, u16)>::new();
        let mut raycast_entries = Vec::<(usize, u16, u8, u32, u16, u16)>::new();
        for (zone_index, zone) in map.zones.iter().enumerate() {
            let Some(outer) = zone.unk20.as_ref() else {
                continue;
            };
            if let Some(tree) = outer.data_ref().spatial_tree.as_ref() {
                let mut typed_indices = Vec::new();
                tree.collect_flagged_placement_indices(0x08, &mut typed_indices);
                typed_raycast_entries.extend(
                    typed_indices
                        .into_iter()
                        .map(|placement_index| (zone_index, placement_index)),
                );
            }
            if outer.offset_relative() == 0 {
                continue;
            }
            let outer_address = outer.offset_absolute();
            edb.seek(SeekFrom::Start(outer_address + 8))
                .expect("seek zone placement spatial tree relptr");
            let tree_relative = edb
                .read_type::<i32>(endian)
                .expect("read zone placement spatial tree relptr");
            if tree_relative == 0 {
                continue;
            }
            let tree_address = (outer_address as i64 + 8 + i64::from(tree_relative)) as u64;
            let mut leaves = Vec::new();
            collect_robots_v248_spatial_leaf_flags(&mut edb, tree_address, 0, &mut leaves);
            for (placement_index, flags) in leaves {
                total += 1;
                *histogram.entry(flags).or_default() += 1;
                let placement = map
                    .placements
                    .data()
                    .get(placement_index as usize)
                    .expect("spatial leaf placement index in range");
                assert_eq!(
                    flags,
                    placement.engine_flags as u8,
                    "m02_city zone {zone_index} placement {placement_index} spatial flags differ from engine_flags low byte"
                );
                if flags & 0x08 != 0 {
                    raycast += 1;
                    raycast_entries.push((
                        zone_index,
                        placement_index,
                        flags,
                        placement.object_ref,
                        placement.engine_flags,
                        placement.map_on,
                    ));
                }
            }
        }
        assert_eq!(total, 312);
        assert_eq!(raycast, 5);
        assert_eq!(
            histogram,
            BTreeMap::from([(0, 2), (1, 6), (8, 4), (11, 1), (16, 299)])
        );
        assert_eq!(
            typed_raycast_entries,
            vec![(6, 22), (6, 21), (7, 22), (15, 1), (19, 6)]
        );
        assert_eq!(
            typed_raycast_entries,
            raycast_entries
                .iter()
                .map(|(zone, placement, ..)| (*zone, *placement))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn real_robots_v248_morph_mesh_directory_tail_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/bo2_sewe.edb");
        let file = File::open(&path).expect("open bo2_sewe.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse bo2_sewe.edb");
        assert_eq!(edb.header.version, 248);

        for entity_index in [1usize, 2usize] {
            let header = edb.header.entity_list.data()[entity_index].clone();
            let mut tail = [0u8; 0x20];
            edb.seek(SeekFrom::Start(header.common.address as u64 + 0x84))
                .expect("seek morph mesh tail");
            edb.read_exact(&mut tail).expect("read morph mesh tail");
            let raw_84 = u32::from_le_bytes(tail[0..4].try_into().unwrap());
            let directory_mask = u16::from_le_bytes(tail[4..6].try_into().unwrap());
            let packed_90 = u32::from_le_bytes(tail[12..16].try_into().unwrap());
            eprintln!(
                "Robots morph Mesh entity={} uid=0x{:08X} +84=0x{raw_84:08X} +88_mask=0x{directory_mask:04X} +90=0x{packed_90:08X} tail={:02X?}",
                entity_index,
                header.common.hashcode,
                tail
            );
            assert_ne!(directory_mask & (1 << 2), 0, "morph directory ID 2 absent");
            assert_eq!(packed_90 & 0xff, 2, "first packed directory is not ID 2");
            assert_ne!(
                packed_90 >> 8,
                0,
                "morph directory self-relative offset is zero"
            );
            let endian = edb.endian;
            let shapes = read_robots_v248_morph_shape_deltas(
                &mut edb,
                endian,
                header.common.address as u64,
                20,
                1,
            )
            .expect("read eyelid morph shape");
            assert_eq!(shapes.len(), 1);
            assert_eq!(shapes[0].len(), 20);
            assert!(
                shapes[0]
                    .iter()
                    .flatten()
                    .any(|value| value.abs() > f32::EPSILON),
                "eyelid morph shape is unexpectedly all zero"
            );
        }
    }

    #[test]
    fn real_robots_v248_entity_animdatum_directory_corpus_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let source_root =
            Path::new(&game_root).join("_eurotools_out/extracted_main/robots/binary/_bin_pc");
        let mut paths = Vec::new();
        collect_edb_files(&source_root, &mut paths);
        assert!(
            !paths.is_empty(),
            "no Robots EDB files under {}",
            source_root.display()
        );

        let mut entities = 0usize;
        let mut directories = 0usize;
        let mut records = 0usize;
        let mut map_collision_records = 0usize;
        let mut hash_counts = BTreeMap::<u32, usize>::new();
        let mut mode_counts = BTreeMap::<u8, usize>::new();
        let mut samples = Vec::new();
        for path in paths {
            let file = File::open(&path)
                .unwrap_or_else(|error| panic!("open {}: {error}", path.display()));
            let mut edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
                .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()));
            if edb.header.version != 248 {
                continue;
            }
            let headers = edb.header.entity_list.data().clone();
            for header in headers {
                entities += 1;
                let endian = edb.endian;
                let Some(directory) = read_robots_v248_entity_anim_datums(
                    &mut edb,
                    endian,
                    header.common.address as u64,
                )
                .unwrap_or_else(|error| {
                    panic!(
                        "read Entity AnimDatum {} uid=0x{:08X}: {error}",
                        path.display(),
                        header.common.hashcode
                    )
                }) else {
                    continue;
                };
                directories += 1;
                assert_eq!(usize::from(directory.record_count), directory.records.len());
                for record in directory.records {
                    records += 1;
                    *hash_counts.entry(record.hashcode).or_default() += 1;
                    if record.hashcode == 0x1000_0004 {
                        map_collision_records += 1;
                        *mode_counts.entry(record.shape_mode).or_default() += 1;
                        if samples.len() < 64 {
                            samples.push((
                                path.display().to_string(),
                                header.common.hashcode,
                                record,
                            ));
                        }
                    }
                }
            }
        }
        eprintln!(
            "Robots Entity AnimDatum corpus entities={entities} directories={directories} records={records} map_collision={map_collision_records} modes={mode_counts:?} hashes={hash_counts:?}"
        );
        for (path, uid, record) in &samples {
            eprintln!(
                "MapCollisionCapsule {} entity=0x{uid:08X} mode={} raw04=0x{:04X} raw07=0x{:02X} scalars={:?} center={:?} quat={:?}",
                path,
                record.shape_mode,
                record.raw_word_04,
                record.raw_byte_07,
                record.shape_scalars,
                record.local_center,
                record.local_orientation,
            );
        }
        assert!(
            directories > 0,
            "no Robots Entity optional directory ID 5 found"
        );
        assert!(records > 0, "no Robots Entity AnimDatum records found");
        assert!(
            map_collision_records > 0,
            "no HT_AnimDatum_MapCollisionCapsule records found"
        );
    }

    #[test]
    fn real_robots_v248_mesh_directory_header_corpus_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let source_root =
            Path::new(&game_root).join("_eurotools_out/extracted_main/robots/binary/_bin_pc");
        let mut paths = Vec::new();
        collect_edb_files(&source_root, &mut paths);
        assert!(
            !paths.is_empty(),
            "no Robots EDB files under {}",
            source_root.display()
        );

        let mut meshes = 0usize;
        let mut directory_meshes = 0usize;
        let mut morph_directory_meshes = 0usize;
        let mut max_raw_84 = 0u32;
        let mut max_directory_entries = 0u32;
        for path in paths {
            let file = File::open(&path)
                .unwrap_or_else(|error| panic!("open {}: {error}", path.display()));
            let mut edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
                .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()));
            if edb.header.version != 248 {
                continue;
            }
            let headers = edb.header.entity_list.data().clone();
            for header in headers {
                edb.seek(SeekFrom::Start(header.common.address as u64))
                    .expect("seek Entity type");
                let object_type = edb.read_type::<u32>(edb.endian).expect("read Entity type");
                if object_type != 0x601 {
                    continue;
                }
                meshes += 1;
                edb.seek(SeekFrom::Start(header.common.address as u64 + 0x84))
                    .expect("seek Mesh directory header");
                let raw_84 = edb.read_type::<u32>(edb.endian).expect("read Mesh +0x84");
                let directory_mask = edb
                    .read_type::<u16>(edb.endian)
                    .expect("read directory mask");
                let reserved_8a = edb.read_type::<u16>(edb.endian).expect("read +0x8A");
                let reserved_8c = edb.read_type::<u32>(edb.endian).expect("read +0x8C");
                let directory_entries = directory_mask.count_ones();
                max_raw_84 = max_raw_84.max(raw_84);
                max_directory_entries = max_directory_entries.max(directory_entries);
                assert_eq!(
                    reserved_8a,
                    0,
                    "nonzero Mesh +0x8A in {} uid=0x{:08X}",
                    path.display(),
                    header.common.hashcode
                );
                assert_eq!(
                    reserved_8c,
                    0,
                    "nonzero Mesh +0x8C in {} uid=0x{:08X}",
                    path.display(),
                    header.common.hashcode
                );
                if directory_entries != 0 {
                    directory_meshes += 1;
                }
                let mut seen_ids = 0u16;
                for _ in 0..directory_entries {
                    let packed = edb
                        .read_type::<u32>(edb.endian)
                        .expect("read packed Mesh directory");
                    let id = (packed & 0xff) as u8;
                    assert!(id < 16, "Mesh directory id {id} is outside mask width");
                    assert_ne!(
                        directory_mask & (1u16 << id),
                        0,
                        "Mesh directory id {id} absent from mask"
                    );
                    assert_eq!(
                        seen_ids & (1u16 << id),
                        0,
                        "duplicate Mesh directory id {id}"
                    );
                    assert_ne!(packed >> 8, 0, "zero Mesh directory relative offset");
                    seen_ids |= 1u16 << id;
                }
                assert_eq!(
                    seen_ids, directory_mask,
                    "Mesh directory entries do not cover mask"
                );
                if directory_mask & (1 << 2) != 0 {
                    morph_directory_meshes += 1;
                }
            }
        }
        assert!(meshes > 0);
        assert!(morph_directory_meshes > 0);
        eprintln!("Robots v248 Mesh directory corpus: meshes={meshes} directory_meshes={directory_meshes} morph_id2_meshes={morph_directory_meshes} max_raw_84={max_raw_84} max_directory_entries={max_directory_entries}");
    }

    fn accumulate_robots_face_info(
        entity: &EXGeoEntity,
        mesh_count: &mut usize,
        face_info_meshes: &mut usize,
        group_count: &mut usize,
        face_count: &mut usize,
        masked_surface_histogram: &mut BTreeMap<u16, usize>,
    ) {
        match entity {
            EXGeoEntity::Mesh(mesh) => {
                *mesh_count += 1;
                let Some(face_info) = &mesh.robots_face_info else {
                    return;
                };
                *face_info_meshes += 1;
                assert_eq!(usize::from(face_info.header_raw[0]), face_info.groups.len());
                *group_count += face_info.groups.len();
                for group in &face_info.groups {
                    *face_count += group.faces.len();
                    for face in &group.faces {
                        for &vertex_index in &face.vertex_indices {
                            assert!(
                                usize::from(vertex_index) < mesh.vertices.len(),
                                "Robots face-info vertex index {} exceeds Mesh vertex count {}",
                                vertex_index,
                                mesh.vertices.len()
                            );
                        }
                        *masked_surface_histogram
                            .entry(face.surface_metadata & 0x78)
                            .or_default() += 1;
                    }
                }
            }
            EXGeoEntity::Split(split) => {
                for child in &split.entities {
                    accumulate_robots_face_info(
                        child,
                        mesh_count,
                        face_info_meshes,
                        group_count,
                        face_count,
                        masked_surface_histogram,
                    );
                }
            }
            _ => {}
        }
    }

    #[test]
    fn real_robots_v248_face_info_corpus_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let source_root =
            Path::new(&game_root).join("_eurotools_out/extracted_main/robots/binary/_bin_pc");
        let mut paths = Vec::new();
        collect_edb_files(&source_root, &mut paths);
        assert!(
            !paths.is_empty(),
            "no Robots EDB files under {}",
            source_root.display()
        );

        let mut mesh_count = 0usize;
        let mut face_info_meshes = 0usize;
        let mut group_count = 0usize;
        let mut face_count = 0usize;
        let mut masked_surface_histogram = BTreeMap::<u16, usize>::new();
        for path in paths {
            let file = File::open(&path)
                .unwrap_or_else(|error| panic!("open {}: {error}", path.display()));
            let mut edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
                .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()));
            if edb.header.version != 248 {
                continue;
            }
            let headers = edb.header.entity_list.data().clone();
            for header in headers {
                edb.seek(SeekFrom::Start(header.common.address as u64))
                    .expect("seek Entity");
                let endian = edb.endian;
                let entity = edb
                    .read_type_args::<EXGeoEntity>(endian, (248, Platform::Pc))
                    .unwrap_or_else(|error| {
                        panic!(
                            "parse Entity uid=0x{:08X} in {}: {error}",
                            header.common.hashcode,
                            path.display()
                        )
                    });
                accumulate_robots_face_info(
                    &entity,
                    &mut mesh_count,
                    &mut face_info_meshes,
                    &mut group_count,
                    &mut face_count,
                    &mut masked_surface_histogram,
                );
            }

            let refpointers = edb.header.refpointer_list.data().clone();
            for refpointer in refpointers {
                edb.seek(SeekFrom::Start(refpointer.address as u64))
                    .expect("seek refpointer");
                let endian = edb.endian;
                let object_type = edb.read_type::<u32>(endian).expect("read refpointer type");
                if !matches!(object_type, 0x601 | 0x602 | 0x603) {
                    continue;
                }
                edb.seek(SeekFrom::Start(refpointer.address as u64))
                    .expect("rewind refpointer Entity");
                let entity = edb
                    .read_type_args::<EXGeoEntity>(endian, (248, Platform::Pc))
                    .unwrap_or_else(|error| {
                        panic!(
                            "parse ref Entity at 0x{:08X} in {}: {error}",
                            refpointer.address,
                            path.display()
                        )
                    });
                accumulate_robots_face_info(
                    &entity,
                    &mut mesh_count,
                    &mut face_info_meshes,
                    &mut group_count,
                    &mut face_count,
                    &mut masked_surface_histogram,
                );
            }
        }

        assert!(mesh_count > 0, "no Robots v248 Mesh entities parsed");
        assert!(
            face_info_meshes > 0,
            "no nonzero Robots v248 face_info streams parsed"
        );
        assert!(
            face_count > 0,
            "Robots v248 face_info streams contain no faces"
        );
        let reachable_aggregate_masks =
            reachable_surface_metadata_or_masks(&masked_surface_histogram);
        assert!(
            reachable_aggregate_masks.contains(&0x68),
            "full shipped face-info corpus should be able to OR into 0x68"
        );
        assert!(
            !reachable_aggregate_masks.contains(&0x60),
            "full shipped face-info corpus unexpectedly ORs into exact 0x60"
        );
        eprintln!(
            "Robots v248 full face-info reachable contact-OR masks: {reachable_aggregate_masks:?}"
        );
        eprintln!(
            "Robots v248 face-info corpus: meshes={mesh_count} face_info_meshes={face_info_meshes} groups={group_count} faces={face_count} masked_surface_histogram={masked_surface_histogram:?}"
        );
    }

    fn reachable_surface_metadata_or_masks(histogram: &BTreeMap<u16, usize>) -> BTreeSet<u16> {
        let mut reachable = histogram
            .iter()
            .filter_map(|(&mask, &count)| (count != 0).then_some(mask & 0x78))
            .collect::<BTreeSet<_>>();
        loop {
            let current = reachable.iter().copied().collect::<Vec<_>>();
            let mut changed = false;
            for &a in &current {
                for &b in &current {
                    changed |= reachable.insert((a | b) & 0x78);
                }
            }
            if !changed {
                return reachable;
            }
        }
    }

    #[test]
    fn real_robots_v248_mapzone_surface_corpus_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let source_root =
            Path::new(&game_root).join("_eurotools_out/extracted_main/robots/binary/_bin_pc");
        let mut paths = Vec::new();
        collect_edb_files(&source_root, &mut paths);

        let mut referenced_meshes = 0usize;
        let mut face_info_meshes = 0usize;
        let mut groups = 0usize;
        let mut faces = 0usize;
        let mut histogram = BTreeMap::<u16, usize>::new();
        let mut unique_targets = BTreeSet::<(String, usize)>::new();

        for path in paths {
            let file = File::open(&path)
                .unwrap_or_else(|error| panic!("open {}: {error}", path.display()));
            let mut edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
                .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()));
            if edb.header.version != 248 || edb.header.map_list.data().is_empty() {
                continue;
            }
            let map_headers = edb.header.map_list.data().clone();
            let refpointers = edb.header.refpointer_list.data().clone();
            for map_header in map_headers {
                edb.seek(SeekFrom::Start(map_header.address as u64))
                    .expect("seek EXGeoMap");
                let endian = edb.endian;
                let map = edb
                    .read_type_args::<EXGeoMap>(endian, (248,))
                    .unwrap_or_else(|error| panic!("parse map in {}: {error}", path.display()));
                for zone in &map.zones {
                    let Some(mapzone_ref) = refpointers.get(zone.entity_refptr as usize) else {
                        continue;
                    };
                    edb.seek(SeekFrom::Start(mapzone_ref.address as u64))
                        .expect("seek MapZone Entity");
                    let mapzone_entity = edb
                        .read_type_args::<EXGeoEntity>(endian, (248, Platform::Pc))
                        .unwrap_or_else(|error| {
                            panic!("parse MapZone Entity in {}: {error}", path.display())
                        });
                    let EXGeoEntity::MapZone(mapzone) = mapzone_entity else {
                        continue;
                    };
                    let target_index = mapzone.entity_refptr as usize;
                    let Some(target_ref) = refpointers.get(target_index) else {
                        continue;
                    };
                    let unique_key = (path.to_string_lossy().into_owned(), target_index);
                    if !unique_targets.insert(unique_key) {
                        continue;
                    }
                    edb.seek(SeekFrom::Start(target_ref.address as u64))
                        .expect("seek MapZone referenced Entity");
                    let object_type = edb.read_type::<u32>(endian).expect("read Entity type");
                    if !matches!(object_type, 0x601 | 0x602 | 0x603) {
                        continue;
                    }
                    edb.seek(SeekFrom::Start(target_ref.address as u64))
                        .expect("rewind MapZone referenced Entity");
                    let entity = edb
                        .read_type_args::<EXGeoEntity>(endian, (248, Platform::Pc))
                        .unwrap_or_else(|error| {
                            panic!(
                                "parse MapZone referenced Entity in {}: {error}",
                                path.display()
                            )
                        });
                    accumulate_robots_face_info(
                        &entity,
                        &mut referenced_meshes,
                        &mut face_info_meshes,
                        &mut groups,
                        &mut faces,
                        &mut histogram,
                    );
                }
            }
        }

        assert!(referenced_meshes > 0, "no MapZone-referenced meshes parsed");
        assert!(
            face_info_meshes > 0,
            "no MapZone-referenced face_info parsed"
        );
        let reachable_aggregate_masks = reachable_surface_metadata_or_masks(&histogram);
        assert!(
            reachable_aggregate_masks.contains(&0x68),
            "shipped MapZone face metadata should be able to OR into 0x68"
        );
        assert!(
            !reachable_aggregate_masks.contains(&0x60),
            "shipped MapZone face metadata unexpectedly ORs into exact 0x60"
        );
        eprintln!("Robots v248 MapZone reachable contact-OR masks: {reachable_aggregate_masks:?}");
        eprintln!(
            "Robots v248 MapZone surface corpus: targets={} meshes={referenced_meshes} face_info_meshes={face_info_meshes} groups={groups} faces={faces} masked_surface_histogram={histogram:?}",
            unique_targets.len()
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
