use std::io::{Read, Seek};

use binrw::{binrw, BinRead, BinReaderExt, BinResult, VecArgs};
use serde::Serialize;

use crate::{
    array::EXRelArray,
    common::{EXRelPtr, EXVector},
};

#[binrw]
#[derive(Debug, Serialize, Clone)]
#[brw(import(version: u32))]
pub struct EXGeoBaseAnimSkin {
    pub object_type: u32, // 0x0
    pub bone_count: u32,  // 0x4

    // TODO(cohae): Probably wrong, just needed to get rid of 8 bytes
    #[brw(if(version.ne(&213) && version.ne(&221) && version.ne(&163) && version.ne(&174)))]
    pub _unkc: [u32; 2], // 0xc
    #[brw(if(version.ne(&213) && version.ne(&163) && version.ne(&174)))]
    pub bounds_box: [EXVector; 2], // 0x10
    pub _unk30: [u32; 4], // 0x30

    #[br(count = bone_count)]
    pub absolute_bind_positions: EXRelPtr<Vec<EXVector>>, // 0x40
    #[br(count = bone_count)]
    pub relative_bind_positions: EXRelPtr<Vec<EXVector>>, // 0x44

    #[br(count = bone_count)]
    pub hier_data: EXRelPtr<Vec<EXGeoAnimSkinHierData>>, // 0x48
    pub _unk4c: EXRelPtr<()>, // 0x4c
    #[brw(if(version.ne(&213) && version.ne(&163) && version.ne(&174)))]
    pub _unk50: [u32; 2], // 0x50
    pub _unk58: EXRelPtr<u16>, // 0x58
    pub _unk5c: EXRelPtr<()>, // 0x5c
    #[brw(if(version.ne(&163)))]
    pub _unk60: Option<EXRelArray<()>>, // 0x60
    pub entities: EXRelArray<EXGeoAnimSkinEntity>, // 0x68
    pub more_entities: EXRelArray<EXGeoAnimSkinEntity>, // 0x70, face-related entities?
    pub _unk78: EXRelArray<()>,
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
    pub entity_index: u32, // TODO(cohae): Add to reference list
    pub morph_index: i32,
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
