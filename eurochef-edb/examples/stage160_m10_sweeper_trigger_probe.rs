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
    let header = edb.header.clone();
    println!(
        "file=0x{:08X} maps={}",
        header.hashcode,
        header.map_list.len()
    );
    for map_header in &header.map_list {
        edb.seek(SeekFrom::Start(map_header.address as u64))?;
        let map = edb.read_type_args::<EXGeoMap>(edb.endian, (header.version,))?;
        println!(
            "map=0x{:08X} triggers={}",
            map_header.hashcode,
            map.trigger_header.triggers.len()
        );
        for (index, wrapped) in map.trigger_header.triggers.iter().enumerate().take(8) {
            let trigger = &wrapped.trigger;
            let ty = &map.trigger_header.trigger_types[trigger.type_index as usize];
            println!(
                "#{index} type={} subtype={} pos=[{:.6},{:.6},{:.6}] rot=[{:.6},{:.6},{:.6}] trig_flags=0x{:08X} game_flags=0x{:08X} data={:?} links={:?}",
                ty.trig_type,
                ty.trig_subtype,
                trigger.position[0],
                trigger.position[1],
                trigger.position[2],
                trigger.rotation[0],
                trigger.rotation[1],
                trigger.rotation[2],
                trigger.trig_flags,
                trigger.game_flags,
                trigger.data,
                trigger.links,
            );
        }
    }
    Ok(())
}
