use std::io::{Read, Seek};

use binrw::{binrw, BinRead, BinReaderExt, BinResult, BinWrite, VecArgs};
use serde::Serialize;

use crate::{
    array::EXRelArray,
    common::{EXRelPtr, EXRelPtr16, EXVector, EXVector3},
};

#[binrw]
#[derive(Debug, Serialize, Clone)]
pub struct EXGeoAnimSkinAuxiliaryPayloadHeader {
    /// Repeats the owning descriptor's AnimBone low16 identity.
    pub animbone_low16: u16,
    /// Repeats the owning descriptor's flags.
    pub raw_flags: u16,
    /// Zero in 665/669 shipped payloads and 0x8000 in the remaining four;
    /// native meaning remains unresolved.
    pub raw_04: u16,
    /// Shipped values are 0, 1, 3, or 5; native meaning remains unresolved.
    pub raw_06: u8,
    /// Repeats the owning descriptor's `raw_variant` as one byte.
    pub repeated_variant: u8,
}

#[derive(Debug, Serialize, Clone)]
pub struct EXGeoAnimSkinAuxiliaryPayload {
    pub header: EXGeoAnimSkinAuxiliaryPayloadHeader,
    /// First structurally stable vector in the detailed record. Native role is unresolved.
    pub raw_vector_a: EXVector3,
    /// Second structurally stable vector in the detailed record. Native role is unresolved.
    pub raw_vector_b: EXVector3,
    /// Shipped Robots v248 corpus: normalized quaternion for all 669 payloads.
    pub unit_quaternion: [f32; 4],
    /// Leaf bone selector for `hierarchy_chain`. This is a hierarchy role and
    /// is not assumed to be the same bone as the descriptor's `HT_AnimBone` ID.
    pub hierarchy_leaf_selector: u8,
    /// Serialized root-to-leaf bone-selector chain. The shipped corpus matches
    /// `EXGeoAnimSkinHierData::link_index` ancestry for all 669 payloads.
    pub hierarchy_chain: Vec<u8>,
    #[serde(skip)]
    tail_padding: Vec<u8>,
}

impl EXGeoAnimSkinAuxiliaryPayload {
    pub fn serialized_size(&self) -> usize {
        48 + ((2 + self.hierarchy_chain.len() + 3) & !3)
    }

    pub fn tail_padding_is_zero(&self) -> bool {
        self.tail_padding.iter().all(|byte| *byte == 0)
    }

    pub fn quaternion_norm(&self) -> f32 {
        self.unit_quaternion
            .iter()
            .map(|value| value * value)
            .sum::<f32>()
            .sqrt()
    }
}

impl BinRead for EXGeoAnimSkinAuxiliaryPayload {
    type Args<'a> = ();

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<Self> {
        let header = EXGeoAnimSkinAuxiliaryPayloadHeader::read_options(reader, endian, ())?;
        let raw_vector_a = <EXVector3>::read_options(reader, endian, ())?;
        let raw_vector_b = <EXVector3>::read_options(reader, endian, ())?;
        let unit_quaternion = <[f32; 4]>::read_options(reader, endian, ())?;
        let hierarchy_leaf_selector = u8::read_options(reader, endian, ())?;
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
            raw_vector_a,
            raw_vector_b,
            unit_quaternion,
            hierarchy_leaf_selector,
            hierarchy_chain,
            tail_padding,
        })
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct EXGeoAnimSkinAuxiliaryEntry {
    /// Low 16 bits of an `HT_AnimBone` hashcode. The shipped corpus contains
    /// 668 named AnimBone IDs plus one 0xFFFF sentinel descriptor.
    pub animbone_low16: u16,
    /// Usually 0x1000 in the shipped corpus (668/669), with one 0xFFFF case.
    pub raw_flags: u16,
    /// Shipped values are 0, 1, or 2; the native meaning is still unresolved.
    pub raw_variant: u16,
    /// Self-relative pointer to this descriptor's detailed payload.
    #[serde(skip)]
    payload_ptr: EXRelPtr16<EXGeoAnimSkinAuxiliaryPayload>,
}

impl EXGeoAnimSkinAuxiliaryEntry {
    pub fn animbone_hashcode(&self) -> Option<u32> {
        (self.animbone_low16 != u16::MAX).then(|| 0x0E00_0000 | u32::from(self.animbone_low16))
    }

    pub fn payload_offset_absolute(&self) -> u64 {
        self.payload_ptr.offset_absolute()
    }

    pub fn payload(&self) -> &EXGeoAnimSkinAuxiliaryPayload {
        self.payload_ptr.data_ref()
    }

    pub fn payload_header(&self) -> &EXGeoAnimSkinAuxiliaryPayloadHeader {
        &self.payload_ptr.data_ref().header
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct EXGeoAnimSkinAuxiliaryHeader {
    pub repeated_count: u16,
    pub raw_02: u8,
    pub raw_03: u8,
    pub raw_04: u8,
    pub repeated_count_u8: u8,
    pub raw_06: u16,
    #[serde(skip)]
    first_payload_ptr: EXRelPtr,
}

impl EXGeoAnimSkinAuxiliaryHeader {
    pub fn first_payload_offset_absolute(&self) -> u64 {
        self.first_payload_ptr.offset_absolute()
    }
}

/// Robots PC v248 AnimSkin section serialized at +0x60/+0x64.
///
/// The outer pair is count + relative pointer. The pointed data starts with a
/// 12-byte self-describing header followed by `count` 8-byte descriptors. The
/// header's final relptr lands exactly at the first byte after that descriptor
/// table. Descriptor high-level semantics remain deliberately unnamed until a
/// native consumer is traced.
#[derive(Debug, Serialize, Clone)]
pub struct EXGeoAnimSkinAuxiliarySection {
    serialized_count: i32,
    #[serde(skip)]
    data_ptr: EXRelPtr,
    pub header: Option<EXGeoAnimSkinAuxiliaryHeader>,
    pub entries: Vec<EXGeoAnimSkinAuxiliaryEntry>,
}

impl EXGeoAnimSkinAuxiliarySection {
    pub fn serialized_len(&self) -> usize {
        self.serialized_count.max(0) as usize
    }

    pub fn data_offset_absolute(&self) -> u64 {
        self.data_ptr.offset_absolute()
    }
}

impl BinRead for EXGeoAnimSkinAuxiliarySection {
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

            let repeated_count: u16 = BinRead::read_options(reader, endian, ())?;
            let raw_02: u8 = BinRead::read_options(reader, endian, ())?;
            let raw_03: u8 = BinRead::read_options(reader, endian, ())?;
            let raw_04: u8 = BinRead::read_options(reader, endian, ())?;
            let repeated_count_u8: u8 = BinRead::read_options(reader, endian, ())?;
            let raw_06: u16 = BinRead::read_options(reader, endian, ())?;
            let first_payload_ptr: EXRelPtr = BinRead::read_options(reader, endian, ())?;
            header = Some(EXGeoAnimSkinAuxiliaryHeader {
                repeated_count,
                raw_02,
                raw_03,
                raw_04,
                repeated_count_u8,
                raw_06,
                first_payload_ptr,
            });

            entries.reserve(serialized_count as usize);
            for _ in 0..serialized_count {
                entries.push(EXGeoAnimSkinAuxiliaryEntry {
                    animbone_low16: BinRead::read_options(reader, endian, ())?,
                    raw_flags: BinRead::read_options(reader, endian, ())?,
                    raw_variant: BinRead::read_options(reader, endian, ())?,
                    payload_ptr: BinRead::read_options(reader, endian, ())?,
                });
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

impl BinWrite for EXGeoAnimSkinAuxiliarySection {
    type Args<'a> = ();

    fn write_options<W: std::io::Write + Seek>(
        &self,
        _writer: &mut W,
        _endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<()> {
        todo!("AnimSkin auxiliary section writing is not implemented")
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
    pub robots_auxiliary_section: Option<EXGeoAnimSkinAuxiliarySection>, // 0x60
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
        io::{BufReader, Read, Seek, SeekFrom},
        path::Path,
    };

    use binrw::BinReaderExt;

    use super::*;
    use crate::{edb::EdbFile, versions::Platform};

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
        let mut scalar_groups = 0usize;
        let mut morph_mode_0 = 0usize;
        let mut morph_mode_1 = 0usize;
        let mut morph_other_modes = 0usize;
        let mut mode_1_scalar_counts = std::collections::BTreeSet::new();
        let mut total_bones = 0usize;
        let mut animbone_pairs = 0usize;
        let mut animbone_selectors_in_bone_range = 0usize;
        let mut animbone_named = 0usize;
        let mut unknown_animbone_hashes = std::collections::BTreeSet::new();
        let mut animbone_selector_conflicts = 0usize;
        let mut animbone_id_conflicts = 0usize;
        let mut bone_attachments = 0usize;
        let mut attachment_bone_oob = 0usize;
        let mut attachment_entity_oob = 0usize;
        let mut animbone_block_layout_matches = 0usize;
        let mut auxiliary_sections_nonempty = 0usize;
        let mut auxiliary_entries = 0usize;
        let mut auxiliary_count_distribution = std::collections::BTreeMap::<usize, usize>::new();
        let mut auxiliary_flag_values = std::collections::BTreeMap::<u16, usize>::new();
        let mut auxiliary_variant_values = std::collections::BTreeMap::<u16, usize>::new();
        let mut auxiliary_key_named_animbones = 0usize;
        let mut auxiliary_unknown_animbone_keys = std::collections::BTreeSet::new();
        let mut auxiliary_sentinel_entries = 0usize;
        let mut auxiliary_first_payload_matches = 0usize;
        let mut auxiliary_payloads_in_file = 0usize;
        let mut auxiliary_payload_ordered_pairs = 0usize;
        let mut auxiliary_payload_pairs = 0usize;
        let mut auxiliary_payload_header_identity_matches = 0usize;
        let mut auxiliary_payload_header_zero_word_matches = 0usize;
        let mut auxiliary_payload_raw04_values = std::collections::BTreeMap::<u16, usize>::new();
        let mut auxiliary_payload_raw06_by_variant =
            std::collections::BTreeMap::<u16, std::collections::BTreeMap<u8, usize>>::new();
        let mut auxiliary_payload_span_by_raw06 =
            std::collections::BTreeMap::<u8, std::collections::BTreeMap<u64, usize>>::new();
        let mut auxiliary_tail_layout_matches = 0usize;
        let mut auxiliary_tail_sorted = 0usize;
        let mut auxiliary_tail_first_is_last = 0usize;
        let mut auxiliary_tail_count_distribution = std::collections::BTreeMap::<u8, usize>::new();
        let mut auxiliary_tail_all_entries = 0usize;
        let mut auxiliary_tail_selector_in_range = 0usize;
        let mut auxiliary_tail_sparse_selector_matches = 0usize;
        let mut auxiliary_tail_sparse_selector_comparable = 0usize;
        let mut auxiliary_tail_hierarchy_matches = 0usize;
        let mut auxiliary_tail_payload_size_matches = 0usize;
        let mut auxiliary_extra_named_bone_slots = 0usize;
        let mut auxiliary_name_conflicts = 0usize;
        let mut auxiliary_fixed_body_finite = 0usize;
        let mut auxiliary_unit_quaternions = 0usize;
        let mut auxiliary_quaternion_norm_min = f32::INFINITY;
        let mut auxiliary_quaternion_norm_max = f32::NEG_INFINITY;
        let mut post_pair_blocks = 0usize;
        let mut post_pair_sparse_pairs = 0usize;
        let mut post_pair_sparse_pair_skins = 0usize;
        let mut post_pair_bone_records = 0usize;
        let mut post_pair_finite_bone_records = 0usize;
        let mut post_pair_gap_sizes = std::collections::BTreeMap::<u64, usize>::new();
        for path in files {
            let file = File::open(&path).unwrap_or_else(|error| {
                panic!("open {}: {error}", path.display());
            });
            let mut edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
                .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()));
            if edb.header.version != 248 {
                continue;
            }
            let headers = edb.header.animskin_list.data().clone();
            let entity_count = edb.header.entity_list.len();
            for header in headers {
                edb.seek(std::io::SeekFrom::Start(header.common.address as u64))
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
                let aux_endian = edb.endian;
                let aux_pairs = skin
                    .read_robots_v248_animbone_pairs(&mut edb, aux_endian)
                    .expect("read AnimBone pairs for auxiliary correlation");
                let aux_id_to_selector = aux_pairs
                    .iter()
                    .map(|(selector, animbone_low16)| (*animbone_low16, *selector))
                    .collect::<std::collections::BTreeMap<_, _>>();
                let aux_selector_to_id = aux_pairs
                    .iter()
                    .map(|(selector, animbone_low16)| (*selector, *animbone_low16))
                    .collect::<std::collections::BTreeMap<_, _>>();
                assert_ne!(skin._unk4c.offset_relative(), 0);
                let post_pair_block = skin
                    .robots_post_pair_block
                    .as_ref()
                    .expect("v248 must contain typed +0x5C post-pair block");
                assert_ne!(post_pair_block.offset_relative(), 0);
                assert_eq!(skin._unk50, [0, 0]);

                let auxiliary = skin
                    .robots_auxiliary_section
                    .as_ref()
                    .expect("v248 must contain +0x60/+0x64 auxiliary section");
                let auxiliary_len = auxiliary.serialized_len();
                *auxiliary_count_distribution
                    .entry(auxiliary_len)
                    .or_default() += 1;
                assert_eq!(auxiliary.entries.len(), auxiliary_len);
                if auxiliary_len != 0 {
                    auxiliary_sections_nonempty += 1;
                    auxiliary_entries += auxiliary_len;
                    let auxiliary_header = auxiliary
                        .header
                        .as_ref()
                        .expect("non-empty v248 auxiliary section must have nested header");
                    assert_eq!(usize::from(auxiliary_header.repeated_count), auxiliary_len);
                    assert_eq!(auxiliary_header.raw_02, 1);
                    assert_eq!(auxiliary_header.raw_03, 1);
                    assert_eq!(auxiliary_header.raw_04, 0);
                    assert_eq!(
                        usize::from(auxiliary_header.repeated_count_u8),
                        auxiliary_len
                    );
                    assert_eq!(auxiliary_header.raw_06, 6);
                    let first_payload = auxiliary_header.first_payload_offset_absolute();
                    assert_eq!(
                        first_payload,
                        auxiliary.data_offset_absolute() + 12 + auxiliary_len as u64 * 8
                    );
                    if auxiliary
                        .entries
                        .first()
                        .is_some_and(|entry| entry.payload_offset_absolute() == first_payload)
                    {
                        auxiliary_first_payload_matches += 1;
                    }
                    let mut payload_headers = Vec::with_capacity(auxiliary_len);
                    for entry in &auxiliary.entries {
                        *auxiliary_flag_values.entry(entry.raw_flags).or_default() += 1;
                        *auxiliary_variant_values
                            .entry(entry.raw_variant)
                            .or_default() += 1;
                        let payload_offset = entry.payload_offset_absolute();
                        if payload_offset < u64::from(edb.header.file_size) {
                            auxiliary_payloads_in_file += 1;
                        }
                        let payload_header = entry.payload_header();
                        if payload_header.animbone_low16 == entry.animbone_low16
                            && payload_header.raw_flags == entry.raw_flags
                            && u16::from(payload_header.repeated_variant) == entry.raw_variant
                        {
                            auxiliary_payload_header_identity_matches += 1;
                        }
                        *auxiliary_payload_raw04_values
                            .entry(payload_header.raw_04)
                            .or_default() += 1;
                        if payload_header.raw_04 == 0 {
                            auxiliary_payload_header_zero_word_matches += 1;
                        }
                        *auxiliary_payload_raw06_by_variant
                            .entry(entry.raw_variant)
                            .or_default()
                            .entry(payload_header.raw_06)
                            .or_default() += 1;

                        let payload = entry.payload();
                        if payload
                            .raw_vector_a
                            .iter()
                            .chain(payload.raw_vector_b.iter())
                            .chain(payload.unit_quaternion.iter())
                            .all(|value| value.is_finite())
                        {
                            auxiliary_fixed_body_finite += 1;
                        }
                        let quaternion_norm = payload.quaternion_norm();
                        auxiliary_quaternion_norm_min =
                            auxiliary_quaternion_norm_min.min(quaternion_norm);
                        auxiliary_quaternion_norm_max =
                            auxiliary_quaternion_norm_max.max(quaternion_norm);
                        if (quaternion_norm - 1.0).abs() <= 1.0e-4 {
                            auxiliary_unit_quaternions += 1;
                        }

                        let tail_selector = payload.hierarchy_leaf_selector;
                        let tail_count = payload.hierarchy_chain.len() as u8;
                        let payload_size = payload.serialized_size();
                        let indices = payload.hierarchy_chain.as_slice();
                        auxiliary_tail_all_entries += 1;
                        *auxiliary_tail_count_distribution
                            .entry(tail_count)
                            .or_default() += 1;
                        if payload.tail_padding_is_zero() {
                            auxiliary_tail_layout_matches += 1;
                        }
                        if indices.windows(2).all(|pair| pair[0] < pair[1]) {
                            auxiliary_tail_sorted += 1;
                        }
                        if indices.last().copied() == Some(tail_selector) {
                            auxiliary_tail_first_is_last += 1;
                        }
                        if usize::from(tail_selector) < skin.bone_count as usize {
                            auxiliary_tail_selector_in_range += 1;
                            let leaf_index = usize::from(tail_selector);
                            let mut hierarchy_chain = Vec::new();
                            let mut current = leaf_index;
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
                                if current >= skin.bone_count as usize || current > u8::MAX as usize
                                {
                                    valid_chain = false;
                                    break;
                                }
                            }
                            hierarchy_chain.reverse();
                            if valid_chain && hierarchy_chain.as_slice() == indices {
                                auxiliary_tail_hierarchy_matches += 1;
                            }
                        }
                        if let Some(expected_selector) =
                            aux_id_to_selector.get(&entry.animbone_low16)
                        {
                            auxiliary_tail_sparse_selector_comparable += 1;
                            if usize::from(*expected_selector) == usize::from(tail_selector) {
                                auxiliary_tail_sparse_selector_matches += 1;
                            }
                        }
                        if entry.animbone_low16 != u16::MAX {
                            match aux_selector_to_id.get(&u16::from(tail_selector)) {
                                Some(existing) if *existing != entry.animbone_low16 => {
                                    auxiliary_name_conflicts += 1;
                                }
                                None => auxiliary_extra_named_bone_slots += 1,
                                _ => {}
                            }
                        }
                        payload_headers.push((payload_offset, payload_header.raw_06, payload_size));
                        if let Some(candidate_animbone) = entry.animbone_hashcode() {
                            if crate::robots_hashdb::resolve(candidate_animbone).is_some() {
                                auxiliary_key_named_animbones += 1;
                            } else {
                                auxiliary_unknown_animbone_keys.insert(candidate_animbone);
                            }
                        } else {
                            auxiliary_sentinel_entries += 1;
                            assert_eq!(entry.raw_flags, u16::MAX);
                        }
                    }
                    for pair in payload_headers.windows(2) {
                        auxiliary_payload_pairs += 1;
                        if pair[1].0 > pair[0].0 {
                            auxiliary_payload_ordered_pairs += 1;
                            let span = pair[1].0 - pair[0].0;
                            *auxiliary_payload_span_by_raw06
                                .entry(pair[0].1)
                                .or_default()
                                .entry(span)
                                .or_default() += 1;
                            if span as usize == pair[0].2 {
                                auxiliary_tail_payload_size_matches += 1;
                            }
                        }
                    }
                } else {
                    assert!(auxiliary.header.is_none());
                }

                let post_pair_start = post_pair_block.offset_absolute();
                let post_pair = post_pair_block.data_ref();
                post_pair_blocks += 1;
                post_pair_sparse_pairs += post_pair.sparse_pairs.len();
                if !post_pair.sparse_pairs.is_empty() {
                    post_pair_sparse_pair_skins += 1;
                }
                assert_eq!(post_pair.bone_records.len(), skin.bone_count as usize);
                post_pair_bone_records += post_pair.bone_records.len();
                post_pair_finite_bone_records += post_pair
                    .bone_records
                    .iter()
                    .filter(|record| record.iter().all(|value| value.is_finite()))
                    .count();

                let post_pair_end = post_pair_start + post_pair.serialized_size() as u64;
                let mut following_offsets = Vec::new();
                if auxiliary.serialized_len() != 0 {
                    following_offsets.push(auxiliary.data_offset_absolute());
                }
                if skin.entities.len() != 0 {
                    following_offsets.push(skin.entities.data_offset_absolute());
                }
                if skin.more_entities.len() != 0 {
                    following_offsets.push(skin.more_entities.data_offset_absolute());
                }
                if let Some(attachments) = skin.bone_attachments.as_ref() {
                    if attachments.len() != 0 {
                        following_offsets.push(attachments.data_offset_absolute());
                    }
                }
                if let Some(groups) = skin.robots_scalar_groups.as_ref() {
                    if groups.serialized_len() != 0 {
                        following_offsets.push(groups.data_offset_absolute());
                    }
                }
                let next_offset = following_offsets
                    .into_iter()
                    .filter(|offset| *offset >= post_pair_end)
                    .min()
                    .expect("v248 post-pair block must precede another known payload");
                let gap = next_offset - post_pair_end;
                assert!(matches!(gap, 0 | 4 | 8 | 12));
                *post_pair_gap_sizes.entry(gap).or_default() += 1;

                let animbone_block_start = skin._unk4c.offset_absolute();
                let block_header = read_test_bytes(&mut edb, animbone_block_start, 8);
                assert_eq!(
                    u32::from_le_bytes(block_header[0..4].try_into().unwrap()),
                    8
                );
                assert_eq!(
                    u32::from_le_bytes(block_header[4..8].try_into().unwrap()),
                    0
                );

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
                assert_eq!(skin._unk58.offset_absolute(), animbone_block_start + 8);
                assert_eq!(
                    post_pair_block.offset_absolute(),
                    animbone_block_start + 12 + pairs.len() as u64 * 4
                );
                animbone_block_layout_matches += 1;
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
                        1 => {
                            morph_mode_1 += 1;
                            mode_1_scalar_counts.insert(group.scalar_count);
                        }
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
                    assert!(
                        component.entity_list_index() < entity_count,
                        "AnimSkin 0x{:08X} in {} references Entity index {} outside {} entries",
                        header.common.hashcode,
                        path.display(),
                        component.entity_list_index(),
                        entity_count
                    );
                }
            }
        }

        assert!(skins > 0, "Robots corpus contained no v248 AnimSkins");
        assert_eq!(morph_other_modes, 0, "unexpected Robots morph modes");
        assert_eq!(skins, 234);
        assert_eq!(total_bones, 5282);
        assert_eq!(scalar_groups, 200);
        assert_eq!(morph_mode_0, 200);
        assert_eq!(morph_mode_1, 0);
        assert!(mode_1_scalar_counts.is_empty());
        assert_eq!(animbone_pairs, 4197);
        assert_eq!(animbone_selectors_in_bone_range, animbone_pairs);
        assert_eq!(animbone_named, 4179);
        assert_eq!(
            unknown_animbone_hashes,
            std::collections::BTreeSet::from([0x0E00_001F])
        );
        assert_eq!(animbone_selector_conflicts, 0);
        assert_eq!(animbone_id_conflicts, 0);
        assert_eq!(animbone_block_layout_matches, skins);
        assert_eq!(bone_attachments, 41);
        assert_eq!(attachment_bone_oob, 0);
        assert_eq!(attachment_entity_oob, 0);
        assert_eq!(auxiliary_sections_nonempty, 93);
        assert_eq!(auxiliary_entries, 669);
        assert_eq!(
            auxiliary_count_distribution,
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
        assert_eq!(
            auxiliary_flag_values,
            std::collections::BTreeMap::from([(0x1000, 668), (0xFFFF, 1)])
        );
        assert_eq!(
            auxiliary_variant_values,
            std::collections::BTreeMap::from([(0, 644), (1, 23), (2, 2)])
        );
        assert_eq!(auxiliary_key_named_animbones, 668);
        assert!(auxiliary_unknown_animbone_keys.is_empty());
        assert_eq!(auxiliary_sentinel_entries, 1);
        assert_eq!(auxiliary_first_payload_matches, auxiliary_sections_nonempty);
        assert_eq!(auxiliary_payloads_in_file, auxiliary_entries);
        assert_eq!(
            auxiliary_payload_pairs,
            auxiliary_entries - auxiliary_sections_nonempty
        );
        assert_eq!(auxiliary_payload_ordered_pairs, auxiliary_payload_pairs);
        assert_eq!(auxiliary_payload_header_identity_matches, auxiliary_entries);
        assert_eq!(auxiliary_payload_header_zero_word_matches, 665);
        assert_eq!(
            auxiliary_payload_raw04_values,
            std::collections::BTreeMap::from([(0, 665), (0x8000, 4)])
        );
        assert_eq!(
            auxiliary_payload_raw06_by_variant,
            std::collections::BTreeMap::from([
                (
                    0,
                    std::collections::BTreeMap::from([(0, 196), (1, 156), (3, 254), (5, 38)])
                ),
                (
                    1,
                    std::collections::BTreeMap::from([(0, 5), (1, 10), (3, 8)])
                ),
                (2, std::collections::BTreeMap::from([(3, 2)])),
            ])
        );
        assert!(auxiliary_payload_span_by_raw06
            .values()
            .all(|spans| { spans.keys().all(|span| matches!(*span, 52 | 56 | 60)) }));
        assert_eq!(auxiliary_fixed_body_finite, auxiliary_entries);
        assert_eq!(auxiliary_unit_quaternions, auxiliary_entries);
        assert!((auxiliary_quaternion_norm_min - 1.0).abs() <= 1.0e-4);
        assert!((auxiliary_quaternion_norm_max - 1.0).abs() <= 1.0e-4);
        assert_eq!(auxiliary_tail_all_entries, auxiliary_entries);
        assert_eq!(auxiliary_tail_layout_matches, auxiliary_entries);
        assert_eq!(auxiliary_tail_sorted, auxiliary_entries);
        assert_eq!(auxiliary_tail_first_is_last, auxiliary_entries);
        assert_eq!(auxiliary_tail_selector_in_range, auxiliary_entries);
        assert_eq!(auxiliary_tail_hierarchy_matches, auxiliary_entries);
        assert_eq!(auxiliary_tail_payload_size_matches, auxiliary_payload_pairs);
        assert_eq!(auxiliary_tail_sparse_selector_comparable, 342);
        assert_eq!(auxiliary_tail_sparse_selector_matches, 45);
        assert_eq!(auxiliary_extra_named_bone_slots, 91);
        assert_eq!(auxiliary_name_conflicts, 532);
        assert_eq!(
            auxiliary_tail_count_distribution,
            std::collections::BTreeMap::from([
                (1, 138),
                (2, 150),
                (3, 128),
                (4, 61),
                (5, 54),
                (6, 41),
                (7, 66),
                (8, 29),
                (9, 1),
                (10, 1),
            ])
        );
        assert_eq!(post_pair_blocks, skins);
        assert_eq!(post_pair_sparse_pairs, 116);
        assert_eq!(post_pair_sparse_pair_skins, 37);
        assert_eq!(post_pair_bone_records, total_bones);
        assert_eq!(post_pair_finite_bone_records, post_pair_bone_records);
        assert_eq!(
            post_pair_gap_sizes,
            std::collections::BTreeMap::from([(0, 60), (4, 62), (8, 50), (12, 62)])
        );
        eprintln!(
            "Robots v248 AnimSkin corpus: skins={skins} bones={total_bones} animbone_pairs={animbone_pairs} auxiliary_entries={auxiliary_entries} typed_payloads={auxiliary_tail_all_entries} hierarchy_chains={auxiliary_tail_hierarchy_matches} post_pair_blocks={post_pair_blocks} post_pair_sparse_pairs={post_pair_sparse_pairs} post_pair_bone_records={post_pair_bone_records} scalar_groups={scalar_groups} rigid_attachments={bone_attachments}"
        );
    }

    fn read_test_bytes(edb: &mut EdbFile, address: u64, length: usize) -> Vec<u8> {
        edb.seek(SeekFrom::Start(address))
            .expect("seek AnimSkin table");
        let mut bytes = vec![0u8; length];
        edb.read_exact(&mut bytes).expect("read AnimSkin table");
        bytes
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
