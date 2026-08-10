use std::io::{Read, Seek};

use crate::path::{read_filename, FilenameEncoding};
use crate::structures::EXFileListHeader5;
use crate::unified::{UXFileInfo, UXFileList, UXFileLoc};

use anyhow::Result;
use binrw::{BinReaderExt, Endian};

#[derive(Debug)]
pub struct EXFileList5 {
    pub endian: Endian,
    pub header: EXFileListHeader5,
    pub filenames: Vec<String>,
}

impl EXFileList5 {
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

        // Robots.exe v7 loader 0x0052DA6E resolves this field relative to the
        // field itself (+0x10), then resolves every filename table element
        // relative to that element's own address.
        reader.seek(std::io::SeekFrom::Start(
            0x10 + res.header.filename_list_offset as u64,
        ))?;

        let base_offset = reader.stream_position()?;
        let mut filename_offsets = Vec::with_capacity(res.header.fileinfo.len());
        for i in 0..res.header.fileinfo.len() as u64 {
            filename_offsets.push(reader.read_type::<u32>(endian)? as u64 + base_offset + i * 4);
        }

        let encoding = if res.header.version >= 7 {
            FilenameEncoding::V7
        } else {
            FilenameEncoding::Plain
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

impl From<EXFileList5> for UXFileList {
    fn from(val: EXFileList5) -> Self {
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

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        ffi::OsStr,
        fs::{self, File},
        io::{BufReader, Cursor},
        path::{Path, PathBuf},
    };

    use super::*;
    use crate::path::scramble_filename_v7;

    fn push_u16(data: &mut Vec<u8>, value: u16) {
        data.extend_from_slice(&value.to_le_bytes());
    }

    fn push_u32(data: &mut Vec<u8>, value: u32) {
        data.extend_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn v7_filename_cipher_uses_record_index_not_sorted_string_offset() {
        let mut data = Vec::new();
        push_u32(&mut data, 7);
        push_u32(&mut data, 0); // filesize, patched below
        push_u32(&mut data, 2);
        push_u16(&mut data, 0);
        push_u16(&mut data, 0);
        push_u32(&mut data, 0x3c); // filename table: 0x10 + 0x3c = 0x4c

        for (hash, addr) in [(0x1111_1111, 0x100), (0x2222_2222, 0x200)] {
            push_u32(&mut data, 4);
            push_u32(&mut data, hash);
            push_u32(&mut data, 1);
            push_u32(&mut data, 0);
            push_u32(&mut data, 1);
            push_u32(&mut data, addr);
            push_u32(&mut data, 0);
        }
        assert_eq!(data.len(), 0x4c);

        let entry0_pos = data.len();
        push_u32(&mut data, 0);
        let entry1_pos = data.len();
        push_u32(&mut data, 0);

        // Physical string order is deliberately reversed. The native loader
        // still decrypts by original file record index.
        let mut name1 = b"second.edb\0".to_vec();
        scramble_filename_v7(1, &mut name1);
        let name1_pos = data.len();
        data.extend_from_slice(&name1);

        let mut name0 = b"first.edb\0".to_vec();
        scramble_filename_v7(0, &mut name0);
        let name0_pos = data.len();
        data.extend_from_slice(&name0);

        data[entry0_pos..entry0_pos + 4]
            .copy_from_slice(&((name0_pos - entry0_pos) as u32).to_le_bytes());
        data[entry1_pos..entry1_pos + 4]
            .copy_from_slice(&((name1_pos - entry1_pos) as u32).to_le_bytes());
        let filesize = data.len() as u32;
        data[4..8].copy_from_slice(&filesize.to_le_bytes());

        let parsed = EXFileList5::read(&mut Cursor::new(data)).unwrap();
        assert_eq!(parsed.filenames, ["first.edb", "second.edb"]);
        assert_eq!(parsed.header.fileinfo[0].hashcode, 0x1111_1111);
        assert_eq!(parsed.header.fileinfo[1].hashcode, 0x2222_2222);
    }

    #[test]
    fn unified_v7_native_four_slot_expansion_repeats_location_zero() {
        let info = UXFileInfo {
            addr: 0x1000,
            filelist_num: Some(1),
            filelocs: vec![
                UXFileLoc {
                    addr: 0x1000,
                    filelist_num: Some(1),
                },
                UXFileLoc {
                    addr: 0x2000,
                    filelist_num: Some(3),
                },
            ],
            length: 123,
            hashcode: 0x0100_0042,
            version: 9,
            flags: 0x20,
        };

        assert_eq!(
            info.native_v7_fileloc_slots().unwrap(),
            [
                UXFileLoc {
                    addr: 0x1000,
                    filelist_num: Some(1),
                },
                UXFileLoc {
                    addr: 0x2000,
                    filelist_num: Some(3),
                },
                UXFileLoc {
                    addr: 0x1000,
                    filelist_num: Some(1),
                },
                UXFileLoc {
                    addr: 0x1000,
                    filelist_num: Some(1),
                },
            ]
        );
    }

    fn find_fixture(root: &Path, wanted: &str) -> Option<PathBuf> {
        let generated = root.join("_eurotools_out");
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.starts_with(&generated) {
                    continue;
                }
                if path.is_dir() {
                    stack.push(path);
                } else if path
                    .file_name()
                    .and_then(OsStr::to_str)
                    .is_some_and(|name| name.eq_ignore_ascii_case(wanted))
                {
                    return Some(path);
                }
            }
        }
        None
    }

    fn validate_real_filelist(path: &Path) -> (usize, BTreeMap<usize, usize>, u32) {
        let file = File::open(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let mut reader = BufReader::new(file);
        let filelist = crate::UXFileList::read(&mut reader)
            .unwrap_or_else(|error| panic!("failed to parse {}: {error:#}", path.display()));

        assert_eq!(filelist.version, 7, "Robots.exe 0x0052DA6E accepts v7 only");
        assert!(!filelist.files.is_empty());

        let archive_count = filelist.num_filelists.unwrap_or(0) as usize + 1;
        let archive_sizes = (0..archive_count)
            .map(|index| {
                let archive = path.with_extension(format!("{index:03}"));
                let size = fs::metadata(&archive)
                    .unwrap_or_else(|error| panic!("{}: {error}", archive.display()))
                    .len();
                (archive, size)
            })
            .collect::<Vec<_>>();

        let mut location_counts = BTreeMap::<usize, usize>::new();
        let mut max_filelist_num = 0u32;
        for (filename, info) in &filelist.files {
            assert!(
                !info.filelocs.is_empty(),
                "{filename} hash={:08X} has no serialized location",
                info.hashcode
            );
            assert!(
                info.filelocs.len() <= 4,
                "{filename} hash={:08X} has {} locations; native runtime stores four",
                info.hashcode,
                info.filelocs.len()
            );
            *location_counts.entry(info.filelocs.len()).or_default() += 1;
            assert!(info.native_v7_fileloc_slots().is_some());

            for location in &info.filelocs {
                let archive_index = location.filelist_num.unwrap_or(0) as usize;
                max_filelist_num = max_filelist_num.max(archive_index as u32);
                let (archive, archive_size) =
                    archive_sizes.get(archive_index).unwrap_or_else(|| {
                        panic!(
                            "{filename} hash={:08X} references archive {} outside 0..{}",
                            info.hashcode,
                            archive_index,
                            archive_sizes.len()
                        )
                    });
                let end = u64::from(location.addr) + u64::from(info.length);
                assert!(
                    end <= *archive_size,
                    "{filename} hash={:08X} range 0x{:X}..0x{:X} exceeds {} size 0x{:X}",
                    info.hashcode,
                    location.addr,
                    end,
                    archive.display(),
                    archive_size
                );
            }
        }

        assert!(max_filelist_num < archive_count as u32);
        (filelist.files.len(), location_counts, max_filelist_num)
    }

    #[test]
    fn real_robots_v7_filelists_and_archives_when_requested() {
        let Ok(root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            eprintln!("SKIP real Robots FileList corpus: EUROCHEF_ROBOTS_GAME_ROOT is unset");
            return;
        };
        let root = Path::new(&root);
        let main = find_fixture(root, "Filelist.bin")
            .unwrap_or_else(|| panic!("Filelist.bin not found under {}", root.display()));
        let usa = find_fixture(root, "File_USA.bin")
            .unwrap_or_else(|| panic!("File_USA.bin not found under {}", root.display()));
        assert_ne!(main, usa);

        for (label, path) in [("main", main), ("usa", usa)] {
            let (files, location_counts, max_filelist_num) = validate_real_filelist(&path);
            eprintln!(
                "ROBOTS_FILELIST {label} path={} files={} location_counts={:?} max_filelist_num={}",
                path.display(),
                files,
                location_counts,
                max_filelist_num
            );
        }
    }
}
