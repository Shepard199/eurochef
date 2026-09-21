use std::{
    fs::File,
    io::{BufReader, Seek, SeekFrom},
};

use anyhow::{Context, Result};
use eurochef_edb::{
    binrw::BinReaderExt, edb::EdbFile, entity::read_robots_v248_entity_anim_datums, map::EXGeoMap,
    versions::Platform,
};
use eurochef_shared::script::{UXGeoScript, UXGeoScriptCommandData};

fn main() -> Result<()> {
    let path = r"D:\Games\Robots\_eurotools_out\extracted_main\robots\binary\_bin_pc\bo5_fin.edb";
    let file = File::open(path).with_context(|| format!("open {path}"))?;
    let mut edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)?;

    let scripts = UXGeoScript::read_all(&mut edb)?;
    let script = scripts
        .iter()
        .find(|script| script.hashcode == 0x0400_0250)
        .context("HT_Script_Sweeper_Boss missing")?;
    println!(
        "script 0x{:08X} fps={} length={}",
        script.hashcode, script.framerate, script.length
    );
    for (index, command) in script.commands.iter().enumerate() {
        if let UXGeoScriptCommandData::Entity { hashcode, file } = command.data {
            println!(
                "script entity cmd#{index}: uid=0x{hashcode:08X} file=0x{file:08X} start={} length={} header_index={} runtime_ctrl={} parent_ctrl={}",
                command.start,
                command.length,
                command.controller_header_index,
                command.controller_index,
                command.parent_controller_index,
            );
            if let Some(controller) = script
                .controllers
                .get(command.controller_header_index as usize)
            {
                println!("  controller={controller:#?}");
            }
        }
    }

    let headers = edb.header.entity_list.data().clone();
    for (index, header) in headers.iter().enumerate() {
        if index <= 5 || header.common.hashcode == 0x0200_01AF {
            println!(
                "entity_list[{index}] uid=0x{:08X} address=0x{:X}",
                header.common.hashcode, header.common.address
            );
        }
    }
    let mut ratchet_center = None;
    for header in headers {
        let endian = edb.endian;
        let Some(directory) =
            read_robots_v248_entity_anim_datums(&mut edb, endian, header.common.address as u64)?
        else {
            continue;
        };
        for record in directory.records {
            if record.hashcode == 0x1000_0036 {
                println!(
                    "ratchet datum entity=0x{:08X} raw04=0x{:04X} mode={} raw07=0x{:02X} scalars={:?} center={:?} quat={:?}",
                    header.common.hashcode,
                    record.raw_word_04,
                    record.shape_mode,
                    record.raw_byte_07,
                    record.shape_scalars,
                    record.local_center,
                    record.local_orientation,
                );
                ratchet_center = Some(record.local_center);
            }
        }
    }

    let center = ratchet_center.context("HT_AnimDatum_RatchetPosition missing")?;
    let map_path =
        r"D:\Games\Robots\_eurotools_out\extracted_main\robots\binary\_bin_pc\m10_boss.edb";
    let map_file = File::open(map_path).with_context(|| format!("open {map_path}"))?;
    let mut map_edb = EdbFile::new(Box::new(BufReader::new(map_file)), Platform::Pc)?;
    let map_header = map_edb
        .header
        .map_list
        .iter()
        .next()
        .cloned()
        .context("m10_boss map missing")?;
    map_edb.seek(SeekFrom::Start(map_header.address as u64))?;
    let map = map_edb.read_type_args::<EXGeoMap>(map_edb.endian, (map_edb.header.version,))?;
    let (_, controller) = map
        .trigger_header
        .triggers
        .iter()
        .enumerate()
        .find(|(_, wrapped)| {
            map.trigger_header.trigger_types[wrapped.trigger.type_index as usize].trig_type == 87
        })
        .context("type87 controller missing")?;
    println!(
        "controller eye link order={:?}",
        &controller.trigger.links[..5]
    );
    for (ordinal, link) in controller.trigger.links[..5].iter().copied().enumerate() {
        let eye = &map
            .trigger_header
            .triggers
            .iter()
            .nth(link as usize)
            .context("controller eye link out of range")?
            .trigger;
        let yaw = eye.rotation[1];
        let (sin_y, cos_y) = yaw.sin_cos();
        let world = [
            center[0] * cos_y + center[2] * sin_y + eye.position[0],
            center[1] + eye.position[1],
            -center[0] * sin_y + center[2] * cos_y + eye.position[2],
            0.0,
        ];
        println!(
            "ratchet anchor ordinal={ordinal} trigger=#{link} eye_pos={:?} eye_rot={:?} world={world:?}",
            eye.position, eye.rotation
        );
    }
    Ok(())
}
