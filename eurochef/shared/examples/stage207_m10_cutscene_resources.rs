use std::{fs::File, io::BufReader};

use anyhow::{Context, Result};
use eurochef_edb::{edb::EdbFile, versions::Platform};
use eurochef_shared::script::{UXGeoScript, UXGeoScriptCommandData};

fn main() -> Result<()> {
    let path = r"D:\Games\Robots\_eurotools_out\extracted_main\robots\binary\_bin_pc\m10_c01.edb";
    let file = File::open(path).with_context(|| format!("open {path}"))?;
    let mut edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)?;
    let scripts = UXGeoScript::read_all(&mut edb)?;
    for script in scripts
        .into_iter()
        .filter(|script| (0x0400_028E..=0x0400_0291).contains(&script.hashcode))
    {
        println!(
            "script=0x{:08X} commands={}",
            script.hashcode,
            script.commands.len()
        );
        for (index, command) in script.commands.iter().enumerate() {
            print!(
                "  cmd#{index} op={} start={} length={} ctrl={} parent={} ",
                command.opcode,
                command.start,
                command.length,
                command.controller_index,
                command.parent_controller_index,
            );
            match &command.data {
                UXGeoScriptCommandData::Entity { hashcode, file } => {
                    println!("Entity uid=0x{hashcode:08X} file=0x{file:08X}")
                }
                UXGeoScriptCommandData::Event { event_type, data } => {
                    println!("Event type=0x{event_type:08X} data={:02X?}", data)
                }
                UXGeoScriptCommandData::Unknown { cmd, data } => {
                    println!("Control/Unknown raw_cmd={cmd} data={:02X?}", data)
                }
                other => println!("{other:?}"),
            }
        }
    }
    Ok(())
}
