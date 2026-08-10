use std::io::{Read, Seek};

use anyhow::Result;
use binrw::{BinReaderExt, Endian};

use crate::{EXFileList4, EXFileList5, EXFileList9};

pub struct UXFileList {
    pub version: u32,
    /// `None` when using a single '.dat' file
    pub num_filelists: Option<u16>,
    pub build_type: Option<u16>,
    pub endian: Endian,
    pub files: Vec<(String, UXFileInfo)>,
}

pub struct UXFileInfo {
    /// Compatibility view of the first serialized location.
    pub addr: u32,
    /// Compatibility view of the first serialized location.
    pub filelist_num: Option<u32>,

    /// All serialized locations, in native record order.
    pub filelocs: Vec<UXFileLoc>,

    pub length: u32,
    pub hashcode: u32,
    pub version: u32,
    pub flags: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UXFileLoc {
    pub addr: u32,
    pub filelist_num: Option<u32>,
}

impl UXFileInfo {
    pub fn primary_fileloc(&self) -> Option<UXFileLoc> {
        self.filelocs.first().copied()
    }

    /// Robots.exe v7 loader `0x0052DA6E` expands each serialized file-location
    /// list to four runtime slots. Missing slots repeat location zero. The read
    /// path at `0x0052D76E` submits all four resulting offsets to the native I/O
    /// backend together, so callers must not discard slots 1..3 as metadata.
    pub fn native_v7_fileloc_slots(&self) -> Option<[UXFileLoc; 4]> {
        let first = self.primary_fileloc()?;
        let mut slots = [first; 4];
        for (slot, location) in slots.iter_mut().zip(self.filelocs.iter().copied()) {
            *slot = location;
        }
        Some(slots)
    }
}

// TODO: We should probably have our own error types, considering that this is a library
impl UXFileList {
    pub fn read<R>(reader: &mut R) -> Result<Self>
    where
        R: Read + Seek,
    {
        let marker: u8 = reader.read_ne()?;
        let endian = if marker == 0 {
            Endian::Big
        } else {
            Endian::Little
        };
        reader.seek(std::io::SeekFrom::Start(0))?;

        Self::read_endian(reader, endian)
    }

    pub fn read_endian<R>(reader: &mut R, endian: Endian) -> Result<Self>
    where
        R: Read + Seek,
    {
        let version: u32 = reader.read_type(endian)?;
        reader.seek(std::io::SeekFrom::Start(0))?;

        Ok(match version {
            4 => EXFileList4::read(reader)?.into(),
            5..=7 => EXFileList5::read(reader)?.into(),
            9..=10 => EXFileList9::read(reader)?.into(),
            v => return Err(anyhow::anyhow!("Unsupported filelist version {}", v)),
        })
    }
}
