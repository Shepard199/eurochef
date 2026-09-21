use std::{
    fs::{self, File},
    io::{BufReader, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use eurochef_edb::{binrw::BinReaderExt, common::EXRelPtr, edb::EdbFile, versions::Platform};

const FILE_UID: u32 = 0x0100_00BC;
const SHEET_UID: u32 = 0x1400_0010;

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
        .context("HT_SpreadSheet_SweeperBossPatterns missing")?;
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
    for sheet_index in 0..sheet_count {
        let ptr: EXRelPtr = edb.read_type(endian)?;
        let return_pos = edb.stream_position()?;
        edb.seek(SeekFrom::Start(ptr.offset_absolute()))?;
        let row_count: u32 = edb.read_type(endian)?;
        let row_address = edb.stream_position()?;
        println!(
            "sheet={sheet_index} ptr=0x{:X} row_count={} rows=0x{:X}",
            ptr.offset_absolute(),
            row_count,
            row_address
        );
        if row_count > 0 {
            let row_size = 10usize;
            let mut row = vec![0u8; row_size];
            let interesting = [0u32, 24, 25, 29, 50, 54, 65, 69];
            for row_index in interesting.into_iter().filter(|index| *index < row_count) {
                edb.seek(SeekFrom::Start(
                    row_address + u64::from(row_index) * row_size as u64,
                ))?;
                std::io::Read::read_exact(&mut edb, &mut row)?;
                let cells = row
                    .chunks_exact(2)
                    .map(|cell| (cell[0] as i8, cell[1] as i8))
                    .collect::<Vec<_>>();
                println!("  row={row_index:02} raw={row:02X?} cells={cells:?}");
            }
        }
        edb.seek(SeekFrom::Start(return_pos))?;
    }
    Ok(())
}
