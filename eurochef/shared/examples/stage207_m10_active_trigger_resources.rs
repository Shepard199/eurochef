use std::{
    fs::File,
    io::{BufReader, Seek, SeekFrom},
};

use anyhow::{Context, Result};
use eurochef_edb::{binrw::BinReaderExt, edb::EdbFile, map::EXGeoMap, versions::Platform};

fn main() -> Result<()> {
    let path = r"D:\Games\Robots\_eurotools_out\extracted_main\robots\binary\_bin_pc\m10_boss.edb";
    let file = File::open(path).with_context(|| format!("open {path}"))?;
    let mut edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)?;
    let header = edb
        .header
        .map_list
        .iter()
        .next()
        .cloned()
        .context("m10 map missing")?;
    edb.seek(SeekFrom::Start(header.address as u64))?;
    let map = edb.read_type_args::<EXGeoMap>(edb.endian, (edb.header.version,))?;
    for (index, wrapped) in map.trigger_header.triggers.iter().enumerate() {
        let trigger = &wrapped.trigger;
        let ttype = map.trigger_header.trigger_types[trigger.type_index as usize].trig_type;
        println!(
            "#{index:02} type={ttype:3} pos={:?} visual={:?} file={:?} game=0x{:08X} trig=0x{:08X} links={:?}",
            trigger.position,
            trigger.engine_options.visual_object,
            trigger.engine_options.visual_object_file,
            trigger.game_flags,
            trigger.trig_flags,
            trigger.links,
        );
    }
    Ok(())
}
