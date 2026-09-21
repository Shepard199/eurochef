use std::{fs::File, io::BufReader};

use anyhow::{Context, Result};
use eurochef_edb::{edb::EdbFile, versions::Platform, HashcodeUtils};
use eurochef_shared::{
    robots_runtime::explosion::read_robots_explosion_database, script::UXGeoScript,
};

fn main() -> Result<()> {
    let path = r"D:\Games\Robots\_eurotools_out\extracted_main\robots\binary\_bin_pc\fx03_exp.edb";
    let requested = std::env::args()
        .nth(1)
        .map(|value| {
            let value = value.trim_start_matches("0x");
            u32::from_str_radix(value, 16).with_context(|| format!("parse script uid {value}"))
        })
        .transpose()?
        .unwrap_or(0x0400_004C);
    let file = File::open(path).with_context(|| format!("open {path}"))?;
    let mut edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)?;
    let database = read_robots_explosion_database(&mut edb)?;
    for fragment in database
        .fragments
        .iter()
        .filter(|row| row.pickup_uid != u32::MAX)
    {
        println!(
            "pickup_fragment=0x{:08X} resource=0x{:08X} file=0x{:08X} pickup=0x{:08X} qty={} flags=0x{:08X}",
            fragment.selector, fragment.script_uid, fragment.file_uid, fragment.pickup_uid, fragment.pickup_quantity(), fragment.flags
        );
    }
    let scripts = UXGeoScript::read_all(&mut edb)?;
    let script = if requested.is_local() {
        scripts.get(requested.index() as usize)
    } else {
        scripts.iter().find(|script| script.hashcode == requested)
    }
    .with_context(|| format!("script 0x{requested:08X} missing"))?;
    println!(
        "script=0x{:08X} fps={} length={} duration={:.6}s commands={}",
        script.hashcode,
        script.framerate,
        script.length,
        script.duration_seconds(),
        script.commands.len()
    );
    for definition in database
        .definitions
        .iter()
        .filter(|definition| definition.main_script_uid == script.hashcode)
    {
        println!(
            "  explosion=0x{:08X} hit_selector=0x{:08X} hit_flags=0x{:08X}",
            definition.selector,
            definition.hit_query_selector(),
            definition.hit_query_initial_flags()
        );
    }
    for (index, command) in script.commands.iter().enumerate() {
        println!(
            "#{index} opcode={} start={} length={} ctrl={} parent={} header={} data={:?}",
            command.opcode,
            command.start,
            command.length,
            command.controller_index,
            command.parent_controller_index,
            command.controller_header_index,
            command.data
        );
        if let Some(controller) = script
            .controllers
            .get(command.controller_header_index as usize)
        {
            println!("  controller={controller:?}");
        }
    }
    Ok(())
}
