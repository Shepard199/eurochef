use std::{
    fs::{self, File},
    io::{BufReader, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use eurochef_edb::{binrw::BinReaderExt, common::EXRelPtr, edb::EdbFile, versions::Platform};

const FILE_UID: u32 = 0x0100_0023;
const SHEET_UID: u32 = 0x1400_0005;
const SELECTORS: [u32; 6] = [0, 7, 9, 13, 14, 20];

fn find_edb(root: &Path) -> Result<PathBuf> {
    for entry in fs::read_dir(root).with_context(|| format!("read {}", root.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("edb") {
            continue;
        }
        let Ok(file) = File::open(&path) else {
            continue;
        };
        let Ok(edb) = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc) else {
            continue;
        };
        if edb.header.hashcode == FILE_UID {
            return Ok(path);
        }
    }
    anyhow::bail!("file 0x{FILE_UID:08X} not found under {}", root.display())
}

fn main() -> Result<()> {
    let root = Path::new(r"D:\Games\Robots\_eurotools_out\extracted_main\robots\binary\_bin_pc");
    let path = find_edb(root)?;
    let file = File::open(&path)?;
    let mut edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)?;
    let endian = edb.endian;
    let spreadsheet = edb
        .header
        .spreadsheet_list
        .iter()
        .find(|sheet| sheet.common.hashcode == SHEET_UID)
        .cloned()
        .context("HT_SpreadSheet_MonsterDatabase missing")?;
    println!(
        "file={} uid=0x{:08X} sheet=0x{:08X} stype={} address=0x{:X}",
        path.display(),
        edb.header.hashcode,
        spreadsheet.common.hashcode,
        spreadsheet.stype,
        spreadsheet.common.address
    );
    edb.seek(SeekFrom::Start(spreadsheet.common.address as u64))?;
    let sheet_count: u32 = edb.read_type(endian)?;
    println!("sheet_count={sheet_count}");
    let mut sheets = Vec::new();
    for sheet_index in 0..sheet_count {
        let ptr: EXRelPtr = edb.read_type(endian)?;
        let return_pos = edb.stream_position()?;
        edb.seek(SeekFrom::Start(ptr.offset_absolute()))?;
        let row_count: u32 = edb.read_type(endian)?;
        let rows = edb.stream_position()?;
        println!("sheet={sheet_index} rows={row_count} address=0x{rows:X}");
        sheets.push((row_count, rows));
        edb.seek(SeekFrom::Start(return_pos))?;
    }
    let (row_count, rows) = sheets[0];
    println!("runtime_type=5 monster_sheet row_count={row_count}");
    for selector in SELECTORS {
        if selector >= row_count {
            println!("selector={selector} OUT_OF_RANGE");
            continue;
        }
        edb.seek(SeekFrom::Start(rows + u64::from(selector) * 24))?;
        let file_uid: u32 = edb.read_type(endian)?;
        let mut rest = [0u32; 5];
        for value in &mut rest {
            *value = edb.read_type(endian)?;
        }
        println!("selector={selector:02} file=0x{file_uid:08X} rest={rest:08X?}");
    }
    Ok(())
}
