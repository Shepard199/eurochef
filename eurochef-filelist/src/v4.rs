use std::io::{Read, Seek};

use crate::path::{read_filename, FilenameEncoding};
use crate::structures::EXFileListHeader4;
use crate::unified::{UXFileInfo, UXFileList, UXFileLoc};

use anyhow::Result;
use binrw::{BinReaderExt, Endian};

#[derive(Debug)]
pub struct EXFileList4 {
    pub endian: Endian,
    pub header: EXFileListHeader4,
    pub filenames: Vec<String>,
}

impl EXFileList4 {
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

        // The header pointer at +0x0c and each table element are self-relative.
        reader.seek(std::io::SeekFrom::Start(
            0xc + res.header.filename_list_offset as u64,
        ))?;

        let base_offset = reader.stream_position()?;
        let mut filename_offsets = Vec::with_capacity(res.header.fileinfo.len());
        for i in 0..res.header.fileinfo.len() as u64 {
            filename_offsets.push(reader.read_type::<u32>(endian)? as u64 + base_offset + i * 4);
        }

        for (file_index, start) in filename_offsets.into_iter().enumerate() {
            res.filenames.push(read_filename(
                reader,
                start,
                res.header.filesize as u64,
                file_index as u32,
                FilenameEncoding::Plain,
            )?);
        }

        Ok(res)
    }
}

impl From<EXFileList4> for UXFileList {
    fn from(val: EXFileList4) -> Self {
        UXFileList {
            version: val.header.version,
            num_filelists: None,
            build_type: None,
            endian: val.endian,
            files: val
                .filenames
                .into_iter()
                .zip(val.header.fileinfo)
                .map(|(filename, info)| {
                    (
                        filename,
                        UXFileInfo {
                            addr: info.addr,
                            filelist_num: None,
                            filelocs: vec![UXFileLoc {
                                addr: info.addr,
                                filelist_num: None,
                            }],
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
