use std::io::{Read, Seek, SeekFrom};

use binrw::BinReaderExt;

use crate::{common::EXRelPtr, edb::EdbFile, error::Result};

pub const ROBOTS_HUB_TRACK_FILE: u32 = 0x0100_0043;
pub const ROBOTS_BALL_TRACK_SPREADSHEET: u32 = 0x1400_000B;
pub const ROBOTS_BALL_TRACK_SCHEDULE_SPREADSHEET: u32 = 0x1400_000E;

#[derive(Clone, Debug, PartialEq)]
pub struct RobotsBallPoolEntry {
    pub resource_hashcode: u32,
    pub count: u16,
    pub variant: u16,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RobotsBallPathEntry {
    pub path_hashcode: u32,
    /// Serialized row +0x04. The native shipped spawn path examined so far
    /// does not consume this field, so keep it raw instead of naming it.
    pub raw_value4: f32,
    /// Serialized row +0x08. Native BallTrack writes this to the spawned ball
    /// handler +0x31C and uses it in the fixed-time path advance.
    pub runtime_speed: f32,
    /// Serialized row +0x0C/+0x10. Native 0x004E3F00 chooses a random start
    /// distance in this inclusive floating-point interval before converting it
    /// to the path parameter used by the spawned ball.
    pub start_distance_min: f32,
    pub start_distance_max: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RobotsBallTrackPair {
    pub sheet_index: u32,
    pub pool: Vec<RobotsBallPoolEntry>,
    pub paths: Vec<RobotsBallPathEntry>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RobotsBallTrackSchedule {
    pub sheet_index: u32,
    /// Native alternate scheduler `0x004E41A0` indexes this payload exactly as
    /// `rows[row_cursor][path_lane]` with a fixed eight-byte row stride.
    pub rows: Vec<[u8; 8]>,
}

fn data_sheet(
    edb: &mut EdbFile,
    spreadsheet_hashcode: u32,
    sheet_index: u32,
) -> Result<Option<(u32, u64)>> {
    let Some(header) = edb
        .header
        .spreadsheet_list
        .iter()
        .find(|header| header.common.hashcode == spreadsheet_hashcode)
        .cloned()
    else {
        return Ok(None);
    };
    if header.stype != 2 {
        return Ok(None);
    }

    let endian = edb.endian;
    let body_offset = u64::from(header.common.address);
    edb.seek(SeekFrom::Start(body_offset))?;
    let sheet_count = edb.read_type::<u32>(endian)?;
    if sheet_index >= sheet_count {
        return Ok(None);
    }

    edb.seek(SeekFrom::Start(
        body_offset + 4 + u64::from(sheet_index) * 4,
    ))?;
    let ptr: EXRelPtr = edb.read_type(endian)?;
    edb.seek(SeekFrom::Start(ptr.offset_absolute()))?;
    let row_count = edb.read_type::<u32>(endian)?;
    let row_address = edb.stream_position()?;
    Ok(Some((row_count, row_address)))
}

pub fn read_robots_ball_track_pair(
    edb: &mut EdbFile,
    sheet_index: u32,
) -> Result<Option<RobotsBallTrackPair>> {
    let original_position = edb.stream_position()?;
    let endian = edb.endian;

    let Some((pool_count, pool_address)) =
        data_sheet(edb, ROBOTS_BALL_TRACK_SPREADSHEET, sheet_index)?
    else {
        edb.seek(SeekFrom::Start(original_position))?;
        return Ok(None);
    };
    edb.seek(SeekFrom::Start(pool_address))?;
    let mut pool = Vec::with_capacity(pool_count as usize);
    for _ in 0..pool_count {
        pool.push(RobotsBallPoolEntry {
            resource_hashcode: edb.read_type(endian)?,
            count: edb.read_type(endian)?,
            variant: edb.read_type(endian)?,
        });
    }

    let Some((path_count, path_address)) =
        data_sheet(edb, ROBOTS_BALL_TRACK_SPREADSHEET, sheet_index + 1)?
    else {
        edb.seek(SeekFrom::Start(original_position))?;
        return Ok(None);
    };
    edb.seek(SeekFrom::Start(path_address))?;
    let mut paths = Vec::with_capacity(path_count as usize);
    for _ in 0..path_count {
        paths.push(RobotsBallPathEntry {
            path_hashcode: edb.read_type(endian)?,
            raw_value4: edb.read_type(endian)?,
            runtime_speed: edb.read_type(endian)?,
            start_distance_min: edb.read_type(endian)?,
            start_distance_max: edb.read_type(endian)?,
        });
    }

    edb.seek(SeekFrom::Start(original_position))?;
    Ok(Some(RobotsBallTrackPair {
        sheet_index,
        pool,
        paths,
    }))
}

pub fn read_robots_ball_track_schedule(
    edb: &mut EdbFile,
    sheet_index: u32,
) -> Result<Option<RobotsBallTrackSchedule>> {
    let original_position = edb.stream_position()?;
    let Some((row_count, row_address)) =
        data_sheet(edb, ROBOTS_BALL_TRACK_SCHEDULE_SPREADSHEET, sheet_index)?
    else {
        edb.seek(SeekFrom::Start(original_position))?;
        return Ok(None);
    };

    edb.seek(SeekFrom::Start(row_address))?;
    let mut rows = Vec::with_capacity(row_count as usize);
    for _ in 0..row_count {
        let mut row = [0u8; 8];
        edb.read_exact(&mut row)?;
        rows.push(row);
    }
    edb.seek(SeekFrom::Start(original_position))?;
    Ok(Some(RobotsBallTrackSchedule { sheet_index, rows }))
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use crate::versions::Platform;

    use super::*;

    #[test]
    fn real_robots_hub_track_pair_when_fixture_is_requested() {
        let Ok(path) = std::env::var("ROBOTS_BALL_TRACK_DB_FIXTURE") else {
            return;
        };
        let file = File::open(path).expect("open Robots T00 HubTrack fixture");
        let mut edb =
            EdbFile::new(Box::new(file), Platform::Pc).expect("parse Robots HubTrack EDB");
        assert_eq!(edb.header.hashcode, ROBOTS_HUB_TRACK_FILE);

        let pair = read_robots_ball_track_pair(&mut edb, 14)
            .expect("parse BallTrack pair")
            .expect("BallTrack pair 14/15");
        assert_eq!(
            pair.pool,
            vec![
                RobotsBallPoolEntry {
                    resource_hashcode: 0x0200_001C,
                    count: 40,
                    variant: 0,
                },
                RobotsBallPoolEntry {
                    resource_hashcode: 0x0200_001B,
                    count: 40,
                    variant: 0,
                },
                RobotsBallPoolEntry {
                    resource_hashcode: 0x0200_001D,
                    count: 40,
                    variant: 0,
                },
            ]
        );
        assert_eq!(pair.paths.len(), 5);
        assert_eq!(pair.paths[0].path_hashcode, 0x0B00_000E);
        assert_eq!(pair.paths[0].raw_value4.to_bits(), 0x3DCC_CCCC);
        assert_eq!(pair.paths[0].runtime_speed, 18.0);
        assert_eq!(pair.paths[0].start_distance_min, 100.0);
        assert_eq!(pair.paths[0].start_distance_max, 120.0);
    }

    #[test]
    fn real_robots_alternate_schedule_when_fixture_is_requested() {
        let Ok(path) = std::env::var("ROBOTS_BALL_TRACK_DB_FIXTURE") else {
            return;
        };
        let file = File::open(path).expect("open Robots T00 HubTrack fixture");
        let mut edb =
            EdbFile::new(Box::new(file), Platform::Pc).expect("parse Robots HubTrack EDB");
        let schedule = read_robots_ball_track_schedule(&mut edb, 1)
            .expect("parse BallTrack alternate schedule")
            .expect("BallTrack alternate schedule sheet 1");
        assert_eq!(schedule.sheet_index, 1);
        assert!(!schedule.rows.is_empty());
        assert!(schedule.rows.iter().flatten().any(|&value| value != 0));
    }

    #[test]
    fn real_robots_shipped_ball_track_pool_variants_when_fixture_is_requested() {
        let Ok(path) = std::env::var("ROBOTS_BALL_TRACK_DB_FIXTURE") else {
            return;
        };
        let file = File::open(path).expect("open Robots T00 HubTrack fixture");
        let mut edb =
            EdbFile::new(Box::new(file), Platform::Pc).expect("parse Robots HubTrack EDB");
        let mut variants = Vec::new();
        for sheet_index in [2u32, 6, 8, 14, 16] {
            let pair = read_robots_ball_track_pair(&mut edb, sheet_index)
                .expect("parse shipped BallTrack pair")
                .unwrap_or_else(|| panic!("missing shipped BallTrack pair {sheet_index}"));
            variants.extend(
                pair.pool
                    .into_iter()
                    .map(|entry| (sheet_index, entry.variant)),
            );
        }
        eprintln!("shipped BallTrack pool variants: {variants:?}");
        assert!(!variants.is_empty());
    }
}
