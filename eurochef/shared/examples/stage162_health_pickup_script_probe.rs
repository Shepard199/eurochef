use std::{fs::File, io::BufReader};

use anyhow::{Context, Result};
use eurochef_edb::{edb::EdbFile, versions::Platform};
use eurochef_shared::script::UXGeoScript;

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap_or_else(|| {
        r"D:\Games\Robots\_eurotools_out\extracted_main\robots\binary\_bin_pc\bo5_fin.edb".into()
    });
    let script_uid = args
        .next()
        .map(|value| u32::from_str_radix(value.trim_start_matches("0x"), 16))
        .transpose()?
        .unwrap_or(0x0400_0022);
    let file = File::open(&path).with_context(|| format!("open {path}"))?;
    let mut edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)?;
    let scripts = UXGeoScript::read_all(&mut edb)?;
    let script = scripts
        .iter()
        .find(|script| script.hashcode == script_uid)
        .with_context(|| format!("Script 0x{script_uid:08X} missing"))?;
    println!(
        "script=0x{:08X} fps={} length={} duration={:.6}s commands={}",
        script.hashcode,
        script.framerate,
        script.length,
        script.duration_seconds(),
        script.commands.len()
    );
    for (index, command) in script.commands.iter().enumerate() {
        println!(
            "#{index} opcode={} start={} length={} ctrl={} parent={} data={:?}",
            command.opcode,
            command.start,
            command.length,
            command.controller_index,
            command.parent_controller_index,
            command.data
        );
    }
    Ok(())
}
