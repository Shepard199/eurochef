use std::io::{Seek, SeekFrom};

use binrw::BinReaderExt;

use crate::{edb::EdbFile, error::Result};

pub const ROBOTS_TRIGGER_DATABASE_FILE: u32 = 0x0100_0026;
pub const ROBOTS_TRIGGER_SPREADSHEET: u32 = 0x1400_0009;

#[derive(Clone, Debug, PartialEq)]
pub struct RobotsPatternGroup {
    pub hashcode: u32,
    pub default_interval: f32,
    pub masks: Vec<u8>,
}

fn row_offset(edb: &mut EdbFile, body_offset: u64, row_index: u32) -> Result<u64> {
    let endian = edb.endian;
    let pointer_offset = body_offset + 4 + u64::from(row_index) * 4;
    edb.seek(SeekFrom::Start(pointer_offset))?;
    let relative = edb.read_type::<i32>(endian)?;
    Ok((pointer_offset as i64 + i64::from(relative)) as u64)
}

pub fn read_robots_pattern_groups(edb: &mut EdbFile) -> Result<Vec<RobotsPatternGroup>> {
    let Some(header) = edb
        .header
        .spreadsheet_list
        .iter()
        .find(|header| header.common.hashcode == ROBOTS_TRIGGER_SPREADSHEET)
        .cloned()
    else {
        return Ok(Vec::new());
    };

    // Native Robots loader 0x00447E30 opens HT_File_D02_Triggers / HT_SpreadSheet_Triggers.
    // Its generic spreadsheet accessors 0x0054F333/0x0054F34C treat the body as:
    //   u32 row_count;
    //   i32 row_relative_offsets[row_count];
    // with each row storing u32 item_count followed by its packed records.
    if header.stype != 2 {
        return Ok(Vec::new());
    }

    let original_position = edb.stream_position()?;
    let endian = edb.endian;
    let body_offset = u64::from(header.common.address);
    edb.seek(SeekFrom::Start(body_offset))?;
    let row_count = edb.read_type::<u32>(endian)?;
    if row_count < 2 {
        edb.seek(SeekFrom::Start(original_position))?;
        return Ok(Vec::new());
    }

    let headers_offset = row_offset(edb, body_offset, 0)?;
    edb.seek(SeekFrom::Start(headers_offset))?;
    let header_count = edb.read_type::<u32>(endian)?;
    let mut groups = Vec::with_capacity(header_count as usize);
    for _ in 0..header_count {
        let hashcode = edb.read_type::<u32>(endian)?;
        let default_interval = edb.read_type::<f32>(endian)?;
        groups.push(RobotsPatternGroup {
            hashcode,
            default_interval,
            masks: Vec::new(),
        });
    }

    let masks_offset = row_offset(edb, body_offset, 1)?;
    edb.seek(SeekFrom::Start(masks_offset))?;
    let mask_count = edb.read_type::<u32>(endian)?;
    for _ in 0..mask_count {
        let hashcode = edb.read_type::<u32>(endian)?;
        let mask_word = edb.read_type::<u32>(endian)?;
        if let Some(group) = groups.iter_mut().find(|group| group.hashcode == hashcode) {
            // Native 0x00447F27 reads only the low byte from spreadsheet record +4.
            group.masks.push(mask_word as u8);
        }
    }

    edb.seek(SeekFrom::Start(original_position))?;
    Ok(groups)
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use crate::versions::Platform;

    use super::*;

    #[test]
    fn real_robots_pattern_groups_when_fixture_is_requested() {
        let Ok(path) = std::env::var("ROBOTS_PATTERN_DB_FIXTURE") else {
            return;
        };
        let file = File::open(path).expect("open Robots D02 trigger database fixture");
        let mut edb = EdbFile::new(Box::new(file), Platform::Pc).expect("parse Robots D02 EDB");
        assert_eq!(edb.header.hashcode, ROBOTS_TRIGGER_DATABASE_FILE);

        let groups = read_robots_pattern_groups(&mut edb).expect("parse pattern groups");
        let opposite = groups
            .iter()
            .find(|group| group.hashcode == 0x5000_0002)
            .expect("HT_PatternGroup_Opposite");
        assert_eq!(opposite.default_interval, 5.0);
        assert_eq!(opposite.masks, vec![0x55, 0xAA]);
    }
}
