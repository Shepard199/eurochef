use std::{
    fs::File,
    io::{BufReader, Seek, SeekFrom},
};

use anyhow::{Context, Result};
use eurochef_edb::{
    anim::EXGeoBaseAnimSkin, binrw::BinReaderExt, edb::EdbFile,
    entity::read_robots_v248_entity_anim_datums, versions::Platform,
};

fn scan(path: &str) -> Result<()> {
    let file = File::open(path).with_context(|| format!("open {path}"))?;
    let mut edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)?;
    let headers = edb.header.entity_list.data().clone();
    println!("FILE {path} entities={}", headers.len());
    for header in headers {
        let endian = edb.endian;
        let Some(directory) =
            read_robots_v248_entity_anim_datums(&mut edb, endian, header.common.address as u64)?
        else {
            continue;
        };
        for record in directory.records {
            if matches!(record.hashcode, 0x1000_0009 | 0x1000_0010) {
                println!(
                    "entity=0x{:08X} datum=0x{:08X} raw04=0x{:04X} mode={} raw07=0x{:02X} scalars={:?} center={:?} quat={:?}",
                    header.common.hashcode,
                    record.hashcode,
                    record.raw_word_04,
                    record.shape_mode,
                    record.raw_byte_07,
                    record.shape_scalars,
                    record.local_center,
                    record.local_orientation,
                );
            }
        }
    }

    let skin_headers = edb.header.animskin_list.data().clone();
    for header in skin_headers {
        edb.seek(SeekFrom::Start(header.common.address as u64))?;
        let skin = edb.read_type_args::<EXGeoBaseAnimSkin>(edb.endian, (edb.header.version,))?;
        let Some(section) = skin.robots_animdatum_section.as_ref() else {
            continue;
        };
        for entry in section.searchable_entries() {
            if matches!(entry.hashcode, 0x1000_0009 | 0x1000_0010) {
                let datum = entry.datum();
                println!(
                    "animskin=0x{:08X} datum=0x{:08X} mode={} scalars={:?} center={:?} quat={:?} selector={} hierarchy={:?}",
                    header.common.hashcode,
                    entry.hashcode,
                    datum.header.shape_mode,
                    datum.shape_scalars,
                    datum.local_center,
                    datum.local_orientation,
                    datum.transform_selector,
                    datum.hierarchy_chain,
                );
            }
        }
    }
    Ok(())
}

fn visit_dir(path: &std::path::Path) -> Result<()> {
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            visit_dir(&path)?;
        } else if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("edb"))
        {
            scan(path.to_string_lossy().as_ref())?;
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let root = std::path::Path::new(
        r"D:\Games\Robots\_eurotools_out\extracted_main\robots\binary\_bin_pc",
    );
    visit_dir(root)
}
