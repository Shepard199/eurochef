use std::io::{Read, Seek};

use crate::path::{read_filename, FilenameEncoding};
use crate::structures::EXFileListHeader9;
use crate::unified::{UXFileInfo, UXFileList, UXFileLoc};

use anyhow::Result;
use binrw::{BinReaderExt, Endian};

#[derive(Debug)]
pub struct EXFileList9 {
    pub endian: Endian,
    pub header: EXFileListHeader9,
    pub filenames: Vec<String>,
}

impl EXFileList9 {
    pub fn read<R>(reader: &mut R) -> Result<Self>
    where
        R: Read + Seek,
    {
        let endian = if reader.read_ne::<u8>()? != 0 {
            Endian::Little
        } else {
            Endian::Big
        };
        reader.seek(std::io::SeekFrom::Start(0))?;

        let mut res = Self {
            endian,
            header: reader.read_type(endian)?,
            filenames: vec![],
        };

        reader.seek(std::io::SeekFrom::Start(
            0x10 + res.header.filename_list_offset as u64,
        ))?;

        let base_offset = reader.stream_position()?;
        let mut filename_offsets = Vec::with_capacity(res.header.fileinfo.len());
        for i in 0..res.header.fileinfo.len() as u64 {
            filename_offsets.push(reader.read_type::<u32>(endian)? as u64 + base_offset + i * 4);
        }

        let encoding = if res.header.version >= 10 {
            FilenameEncoding::V10
        } else {
            FilenameEncoding::V7
        };
        for (file_index, start) in filename_offsets.into_iter().enumerate() {
            res.filenames.push(read_filename(
                reader,
                start,
                res.header.filesize as u64,
                file_index as u32,
                encoding,
            )?);
        }

        Ok(res)
    }
}

impl From<EXFileList9> for UXFileList {
    fn from(val: EXFileList9) -> Self {
        UXFileList {
            version: val.header.version,
            num_filelists: Some(val.header.num_filelists),
            build_type: Some(val.header.build_type),
            endian: val.endian,
            files: val
                .filenames
                .into_iter()
                .zip(val.header.fileinfo)
                .map(|(filename, info)| {
                    let filelocs = info
                        .fileloc
                        .iter()
                        .map(|location| UXFileLoc {
                            addr: location.addr,
                            filelist_num: Some(location.filelist_num),
                        })
                        .collect::<Vec<_>>();
                    let primary = filelocs.first().copied();
                    (
                        filename,
                        UXFileInfo {
                            addr: primary.map_or(0, |location| location.addr),
                            filelist_num: primary.and_then(|location| location.filelist_num),
                            filelocs,
                            flags: info.flags,
                            hashcode: info.hashcode,
                            length: info.length,
                            version: info.version,
                        },
                    )
                })
                .collect(),
        }
    }
}
