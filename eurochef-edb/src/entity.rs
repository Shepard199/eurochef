#![allow(non_camel_case_types)]
use binrw::{binrw, BinRead, BinReaderExt};
use serde::Serialize;

use crate::{
    common::{EXRelPtr, EXVector},
    entity_mesh::EXGeoMeshEntity,
    versions::Platform,
};

#[binrw]
#[derive(Debug, Serialize, Clone)]
#[brw(import(version: u32))]
// TODO: Format is slightly different on versions 248 and below
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
