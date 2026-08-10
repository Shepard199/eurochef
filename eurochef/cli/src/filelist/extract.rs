use std::{
    fs::File,
    io::{BufReader, Read, Seek, Write},
    path::{Path, PathBuf},
};

use anyhow::Context;
use eurochef_edb::{
    binrw::{BinReaderExt, Endian},
    versions::{transform_windows_path, Platform},
};
use eurochef_filelist::{unified::UXFileLoc, UXFileList};
use indicatif::{ProgressBar, ProgressIterator, ProgressStyle};

use crate::filelist::TICK_STRINGS;

fn read_payload_at(
    data_file: &mut File,
    location: UXFileLoc,
    serialized_length: u32,
    endian: Endian,
) -> anyhow::Result<Vec<u8>> {
    data_file.seek(std::io::SeekFrom::Start(location.addr as u64))?;

    let magic: u32 = data_file.read_type(endian)?;
    let mut filesize = serialized_length;
    if magic == 0x47454F4D {
        data_file.seek(std::io::SeekFrom::Current(0x10))?;
        filesize = data_file.read_type(endian)?;
    }

    data_file.seek(std::io::SeekFrom::Start(location.addr as u64))?;
    let mut data = vec![0u8; filesize as usize];
    data_file.read_exact(&mut data)?;
    Ok(data)
}

fn read_payload_from_locations(
    data_files: &mut [Option<File>],
    data_file_paths: &[PathBuf],
    locations: &[UXFileLoc],
    serialized_length: u32,
    endian: Endian,
    filename: &str,
    hashcode: u32,
) -> anyhow::Result<Vec<u8>> {
    let mut failures = Vec::new();
    let mut attempted = Vec::<UXFileLoc>::new();

    for location in locations.iter().copied() {
        // Robots v7 repeats slot zero when fewer than four runtime slots are
        // serialized. A synchronous extractor gains nothing by retrying the
        // exact same archive/offset, so keep native order but skip duplicates.
        if attempted.contains(&location) {
            continue;
        }
        attempted.push(location);

        let data_file_index = location.filelist_num.unwrap_or(0) as usize;
        let path = data_file_paths
            .get(data_file_index)
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| format!("archive #{data_file_index}"));
        let Some(Some(data_file)) = data_files.get_mut(data_file_index) else {
            failures.push(format!(
                "location archive={} addr=0x{:X}: data file {} is unavailable",
                data_file_index, location.addr, path
            ));
            continue;
        };

        match read_payload_at(data_file, location, serialized_length, endian) {
            Ok(data) => return Ok(data),
            Err(error) => failures.push(format!(
                "location archive={} addr=0x{:X} ({}): {error:#}",
                data_file_index, location.addr, path
            )),
        }
    }

    anyhow::bail!(
        "FileList entry {filename} (hash {hashcode:08x}) failed at all {} serialized location(s): {}",
        attempted.len(),
        failures.join("; ")
    )
}

pub fn execute_command(
    filename: String,
    output_folder: String,
    create_scr: bool,
) -> anyhow::Result<()> {
    println!("Extracting {filename} to {output_folder}");
    let mut file = File::open(&filename).context("Failed to open filelist header")?;
    let mut reader = BufReader::new(&mut file);
    let filelist = UXFileList::read(&mut reader)?;

    std::fs::create_dir_all(&output_folder)?;

    let platform = {
        if let Some((path_with_platform, _)) = filelist
            .files
            .iter()
            .find(|(k, _)| k.to_lowercase().contains("_bin_"))
        {
            Platform::from_path(transform_windows_path(path_with_platform))
        } else {
            None
        }
    };

    if let Some(p) = platform {
        println!("Detected platform: {:?}", p);
    }

    let scr_path = Path::new(&(output_folder.to_owned() + "/../"))
        .canonicalize()?
        .join(format!(
            "FileList{}.scr",
            platform
                .map(|p| p.shorthand().to_uppercase())
                .unwrap_or_default()
        ));

    let mut scr_file = if create_scr {
        if scr_path.is_file() {
            // swy: let's avoid writing on top of potentially edited, or vastly-improved .scr descriptor files
            println!(
                "The file at '{}' already exists, move or rename it first; .scr file will NOT be written",
                scr_path.display()
            );

            None
        } else {
            // swy: nothing seemingly there, let's go ahead and write as requested
            println!(
                "Creating an accompanying '{}' file",
                scr_path.file_name().and_then(|s| s.to_str()).unwrap()
            );

            File::create(scr_path.clone())
                .context("Failed to create the .scr file")
                .ok()
        }
    } else {
        None // swy: the user explicitly asked not to write the .scr file at all
    };

    if let Some(f) = scr_file.as_mut() {
        writeln!(
            f,
            "[FileInfomation]

[FileList]\n"
        )
        .expect("Failed to write scr file header");
    }

    let file_base = &filename[..filename.len() - 3];
    let mut data_file_paths = vec![];
    if let Some(num_filelists) = filelist.num_filelists {
        for i in 0..(num_filelists + 1) {
            data_file_paths.push(PathBuf::from(format!("{}{:03}", file_base, i)));
        }
    } else {
        data_file_paths.push(PathBuf::from(format!("{}DAT", file_base)));
    }
    let mut data_files = data_file_paths
        .iter()
        .map(|path| File::open(path).ok())
        .collect::<Vec<_>>();

    let pb = ProgressBar::new(filelist.files.len() as u64);
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {msg} ({pos}/{len})",
        )
        .unwrap()
        .progress_chars("##-")
        .tick_chars(TICK_STRINGS),
    );
    pb.set_message("Extracting files");

    for (i, (filename, info)) in filelist.files.iter().enumerate().progress_with(pb) {
        let filename_fixed = filename.replace('\\', "/");
        let fpath = Path::new(&filename_fixed);

        if let Some(ref mut f) = scr_file {
            writeln!(f, "{}", filename).expect("Failed to write file name to .scr");
        };

        if fpath.to_string_lossy().is_empty() {
            println!(
                "Skipping file {} with empty path (hashcode {:08x})",
                i, info.hashcode
            );
            continue;
        }

        let data = read_payload_from_locations(
            &mut data_files,
            &data_file_paths,
            &info.filelocs,
            info.length,
            filelist.endian,
            filename,
            info.hashcode,
        )
        .with_context(|| format!("failed to extract FileList entry {i}"))?;

        let fpath_noprefix = Path::new(&output_folder).join(&fpath.to_str().unwrap()[3..]);
        std::fs::create_dir_all(fpath_noprefix.parent().unwrap())?;
        File::create(&fpath_noprefix)
            .context(format!("Failed to create output file {fpath_noprefix:?}"))?
            .write_all(&data)?;
    }

    println!("Successfully extracted {} files", filelist.files.len());

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    #[test]
    fn extractor_falls_back_to_later_serialized_location() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "eurochef-filelist-fallback-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        let bad = root.join("Filelist.000");
        let good = root.join("Filelist.001");
        fs::write(&bad, [0u8; 4]).unwrap();
        fs::write(&good, b"ABCDEFGH").unwrap();

        let paths = vec![bad.clone(), good.clone()];
        let mut files = vec![
            Some(File::open(&bad).unwrap()),
            Some(File::open(&good).unwrap()),
        ];
        let locations = vec![
            UXFileLoc {
                addr: 0x100,
                filelist_num: Some(0),
            },
            UXFileLoc {
                addr: 0,
                filelist_num: Some(1),
            },
        ];

        let data = read_payload_from_locations(
            &mut files,
            &paths,
            &locations,
            8,
            Endian::Little,
            "fallback.bin",
            0x1234_5678,
        )
        .unwrap();
        assert_eq!(data, b"ABCDEFGH");

        fs::remove_dir_all(root).unwrap();
    }
}
