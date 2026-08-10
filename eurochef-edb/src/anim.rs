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
    pub _unk4c: EXRelPtr<()>, // 0x4c
    #[brw(if(version.ne(&213) && version.ne(&163) && version.ne(&174)))]
    pub _unk50: [u32; 2], // 0x50
    pub _unk58: EXRelPtr<u16>, // 0x58
    pub _unk5c: EXRelPtr<()>, // 0x5c
    #[brw(if(version.ne(&163)))]
    pub _unk60: Option<EXRelArray<()>>, // 0x60
    pub entities: EXRelArray<EXGeoAnimSkinEntity>, // 0x68
    pub more_entities: EXRelArray<EXGeoAnimSkinEntity>, // 0x70, face-related entities?
    pub _unk78: EXRelArray<()>, // 0x78

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
    use std::{fs::File, io::BufReader, path::Path};

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
        eprintln!("Robots v248 AnimSkin corpus: skins={skins} scalar_groups={scalar_groups} mode0={morph_mode_0} mode1={morph_mode_1} mode1_scalar_counts={mode_1_scalar_counts:?}");
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
