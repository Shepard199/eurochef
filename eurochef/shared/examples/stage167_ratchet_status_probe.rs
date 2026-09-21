use std::{
    fs::File,
    io::{BufReader, Seek, SeekFrom},
};

use anyhow::{Context, Result};
use eurochef_edb::{
    binrw::BinReaderExt, edb::EdbFile, script::EXGeoAnimScript, versions::Platform,
};
use eurochef_shared::script::{UXGeoScript, UXGeoScriptCommandData};

fn main() -> Result<()> {
    let path = r"D:\Games\Robots\_eurotools_out\extracted_main\robots\binary\_bin_pc\nb11_rat.edb";
    let file = File::open(path).with_context(|| format!("open {path}"))?;
    let mut edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)?;
    let scripts = UXGeoScript::read_all(&mut edb)?;
    for script in &scripts {
        for cmd in &script.commands {
            if let UXGeoScriptCommandData::Event { event_type, data } = &cmd.data {
                if *event_type == 0x1600_0007 {
                    println!(
                        "script=0x{:08X} fps={} len={} opcode={} event_start={} event_len={} payload={:02X?}",
                        script.hashcode,
                        script.framerate,
                        script.length,
                        cmd.opcode,
                        cmd.start,
                        cmd.length,
                        data
                    );
                }
            }
        }
    }

    for script_hashcode in [
        0x8400_0006u32,
        0x8400_0007,
        0x8400_000B,
        0x8400_000C,
        0x8400_000D,
        0x8400_0012,
    ] {
        let header = edb
            .header
            .animscript_list
            .iter()
            .find(|header| header.hashcode == script_hashcode)
            .cloned()
            .with_context(|| format!("missing raw Script 0x{script_hashcode:08X}"))?;
        edb.seek(SeekFrom::Start(header.address as u64))?;
        let raw = edb.read_type::<EXGeoAnimScript>(edb.endian)?;
        println!(
            "RAW script=0x{script_hashcode:08X} len={} fps={}",
            raw.length, raw.frame_rate
        );
        for (index, command) in raw.commands.iter().enumerate() {
            println!(
                "  #{index} opcode={} size={} cmd_frame={} start={} length={} data={:02X?}",
                command.cmd,
                command.cmd_size,
                command.cmd_frame,
                command.start,
                command.length,
                command.data
            );
        }
    }
    let endian = edb.endian;
    let runtime_scripts = [
        0x8400_0006u32,
        0x8400_0007,
        0x8400_0008,
        0x8400_0009,
        0x8400_000A,
        0x8400_000B,
        0x8400_000C,
        0x8400_000D,
        0x8400_000E,
        0x8400_000F,
        0x8400_0010,
        0x8400_0011,
        0x8400_0012,
    ];
    for script_hashcode in runtime_scripts {
        let script = scripts
            .iter()
            .find(|script| script.hashcode == script_hashcode)
            .with_context(|| format!("missing UX Script 0x{script_hashcode:08X}"))?;
        let anim_hashcode = script
            .commands
            .iter()
            .find_map(|command| match command.data {
                UXGeoScriptCommandData::Animation { anim_hashcode, .. } => Some(anim_hashcode),
                _ => None,
            });
        let Some(anim_hashcode) = anim_hashcode else {
            println!("CLOCK script=0x{script_hashcode:08X} no-animation");
            continue;
        };
        let animation = edb
            .header
            .anim_list
            .iter()
            .find(|animation| animation.common.hashcode == anim_hashcode)
            .cloned()
            .with_context(|| format!("missing Animation 0x{anim_hashcode:08X}"))?;
        let serialized = animation.read_robots_v248_exgeoanim(&mut edb, endian)?;
        println!(
            "CLOCK script=0x{script_hashcode:08X} script_len={} script_fps={} animation=0x{anim_hashcode:08X} anim_frames={} anim_fps={}",
            script.length,
            script.framerate,
            serialized.frame_count,
            serialized.raw_byte_0c
        );
    }

    Ok(())
}
