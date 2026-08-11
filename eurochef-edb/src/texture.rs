use binrw::binrw;

use crate::{
    array::EXGeoCommonArrayElement,
    common::{EXRelPtr, EXRelPtr16},
    versions::Platform,
};

#[binrw]
#[derive(Debug, Clone)]
pub struct EXGeoTextureHeader {
    pub common: EXGeoCommonArrayElement,
    pub width: u16,
    pub height: u16,
    pub game_flags: u32,
    pub flags: u32,
}

#[binrw]
#[derive(Debug)]
#[brw(import(version: u32, platform: Platform))]
pub struct EXGeoTexture {
    #[brw(if(version <= 205))]
    unk0: u32,

    pub width: u16,      // 0x0
    pub height: u16,     // 0x2
    pub depth: u16,      // 0x4
    pub game_flags: u16, // 0x6
    /// Robots PC v248: native world-face path 0x0041AD50 reads this exact i16,
    /// multiplies it by 0.002, derives the triangle U direction from position/UV
    /// data and adds the resulting tangential surface velocity to XItemPhysics.
    pub scroll_u: i16, // 0x8
    /// Same native surface-motion path as scroll_u, along the triangle V direction.
    /// These fields are texture-scroll-driven conveyor/surface motion, not friction
    /// or restitution coefficients.
    pub scroll_v: i16, // 0xa
    pub frame_count: u8, // 0xc
    pub image_count: u8, // 0xd
    pub frame_rate: u8,  // 0xe
    _pad0: u8,           // 0xf
    pub values_used: u8, // 0x10
    pub regions_count: u8, // 0x11
    pub mip_count: u8,   // 0x12
    pub format: u8,      // 0x13
    pub unk_14: u32,     // 0x14
    pub color: [u8; 4],  // 0x18

    // TODO(cohae): Might apply to predator as well
    /// If set, contains the hashcode of another file
    /// The first frame offset will be replaced with a texture hashcode from that file
    #[br(map = |x: i32| if x == -1 { None } else { Some(x as u32) } )]
    #[br(if(version >= 250))]
    pub external_file: Option<u32>, // 0x1c

    animseq_data: EXRelPtr16, // 0x1c, OFFSET.W ANIMSEQDATA
    value_data: EXRelPtr16,   // 0x1e, OFFSET.W VALUEDATA
    #[br(if(version > 163))]
    fur_data: Option<EXRelPtr16>, // 0x20, OFFSET.W FURDATA
    #[br(if(version > 163))]
    region_data: Option<EXRelPtr16>, // 0x22, OFFSET.W REGIONDATA

    #[brw(if(platform == Platform::Ps2 && version != 248 && version != 177 && version != 168))]
    // #[brw(if(platform == Platform::Ps2 && (version <= 163 || version == 213)))]
    _unk2: u32, // 0x24

    /// Robots PC v248 byte size of one base image payload. Robots.exe
    /// 0x0057240D reads frame relative pointers from +0x28, while the shipped
    /// corpus proves this +0x24 DWORD equals width*height*depth*bpp/8 for all
    /// 3963 decoded Textures.
    #[brw(if(version == 248))]
    pub robots_base_image_size: u32,

    #[brw(if(platform == Platform::Ps2))]
    pub clut_offset: Option<EXRelPtr>,

    /// Certain platforms such as PC and PS2 calculate data size from other parameters.
    /// Some games (e.g. Chaos Bleeds on Xbox) also seem to do this.
    /// For general usage it is not recommended to rely on this field exlusively for data size.
    #[brw(if(platform != Platform::Pc && platform != Platform::Ps2 && !(version == 170 && platform == Platform::Xbox)))]
    pub data_size: Option<u32>,

    #[br(count = image_count)]
    pub frame_offsets: Vec<EXRelPtr>,
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader, io::Seek, path::Path};

    use binrw::BinReaderExt;

    use super::*;
    use crate::edb::EdbFile;

    #[test]
    fn real_robots_v248_texture_base_image_size_and_frame_table_parse_when_game_root_is_configured()
    {
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

        let mut textures = 0usize;
        for path in files {
            let file = File::open(&path)
                .unwrap_or_else(|error| panic!("open {}: {error}", path.display()));
            let mut edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
                .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()));
            if edb.header.version != 248 {
                continue;
            }
            let headers = edb.header.texture_list.data().clone();
            for header in headers {
                edb.seek(std::io::SeekFrom::Start(header.common.address as u64))
                    .unwrap();
                let texture = edb
                    .read_type_args::<EXGeoTexture>(edb.endian, (248, Platform::Pc))
                    .unwrap_or_else(|error| {
                        panic!(
                            "parse Texture 0x{:08X} in {}: {error}",
                            header.common.hashcode,
                            path.display()
                        )
                    });
                assert_eq!(texture.frame_offsets.len(), texture.image_count as usize);
                let bpp = match texture.format {
                    0 | 1 | 5 => 16u64,
                    2 | 3 => 4u64,
                    4 | 7 | 8 | 9 => 8u64,
                    6 => 32u64,
                    format => panic!("unexpected Robots PC Texture format 0x{format:02X}"),
                };
                let expected_base_image_size = (u64::from(texture.width)
                    * u64::from(texture.height)
                    * u64::from(texture.depth)
                    * bpp
                    + 7)
                    / 8;
                assert_eq!(
                    u64::from(texture.robots_base_image_size),
                    expected_base_image_size,
                    "Texture 0x{:08X} in {} has unexpected +0x24 base image size",
                    header.common.hashcode,
                    path.display()
                );
                textures += 1;
            }
        }

        assert!(textures > 0, "Robots corpus contained no v248 Textures");
        eprintln!(
            "Robots v248 Texture corpus: textures={textures} base_image_size={textures}/{textures} exact"
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
