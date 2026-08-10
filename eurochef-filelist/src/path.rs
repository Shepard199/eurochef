use std::io::{Read, Seek, SeekFrom};

use anyhow::{bail, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FilenameEncoding {
    Plain,
    V7,
    V10,
}

fn decode_filename_byte_v7(file_index: u32, byte_index: u32, byte: u8) -> u8 {
    (byte as u32)
        .wrapping_add(0x16)
        .wrapping_sub(file_index)
        .wrapping_sub(byte_index) as u8
}

fn decode_filename_byte_v10(file_index: u32, byte_index: u32, byte: u8) -> u8 {
    (byte as u32)
        .wrapping_sub(0x6a)
        .wrapping_sub(file_index * 4)
        .wrapping_sub(byte_index * 4) as u8
}

pub fn unscramble_filename_v7(file_index: u32, bytes: &mut [u8]) {
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = decode_filename_byte_v7(file_index, i as u32, *b);

        if *b == 0 {
            break;
        }
    }
}

// TODO: This should take a string and output a Cow<[u8]> to ensure null-termination
pub fn scramble_filename_v7(file_index: u32, bytes: &mut [u8]) {
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = (*b as u32)
            .wrapping_sub(0x16)
            .wrapping_add(file_index)
            .wrapping_add(i as u32) as u8;
    }
}

pub fn unscramble_filename_v10(file_index: u32, bytes: &mut [u8]) {
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = decode_filename_byte_v10(file_index, i as u32, *b);

        if *b == 0 {
            break;
        }
    }
}

/// Reads one filename through its self-relative offset without assuming that
/// the filename strings are laid out in record order.
///
/// Robots.exe `0x0052DA6E` walks the v7 filename offset table in original file
/// record order and applies the cipher with that original record index. A
/// sorted-offset pass therefore changes the cipher key and can associate a
/// filename with the wrong file record when strings are not physically
/// monotonic.
pub(crate) fn read_filename<R: Read + Seek>(
    reader: &mut R,
    start: u64,
    file_end: u64,
    file_index: u32,
    encoding: FilenameEncoding,
) -> Result<String> {
    if start >= file_end {
        bail!(
            "filename {} starts outside filelist: 0x{:X} >= 0x{:X}",
            file_index,
            start,
            file_end
        );
    }

    reader.seek(SeekFrom::Start(start))?;
    let mut decoded = Vec::new();
    for byte_index in 0..(file_end - start) {
        let mut raw = [0u8; 1];
        reader.read_exact(&mut raw)?;
        let byte = match encoding {
            FilenameEncoding::Plain => raw[0],
            FilenameEncoding::V7 => decode_filename_byte_v7(file_index, byte_index as u32, raw[0]),
            FilenameEncoding::V10 => {
                decode_filename_byte_v10(file_index, byte_index as u32, raw[0])
            }
        };
        if byte == 0 {
            return Ok(String::from_utf8_lossy(&decoded).into_owned());
        }
        decoded.push(byte);
    }

    bail!("filename {} is missing a null terminator", file_index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unscramble_path() {
        let mut input: [u8; 39] = [
            0xC3, 0x86, 0xA9, 0xB5, 0xB5, 0xBF, 0xC3, 0xB5, 0xB8, 0xB0, 0xB7, 0xBF, 0xC5, 0xB9,
            0xCB, 0xD3, 0xB7, 0xBB, 0xBF, 0xC7, 0xCD, 0xBF, 0xD1, 0xC5, 0xBF, 0xD1, 0xDC, 0xC5,
            0xD0, 0xD6, 0xDD, 0xD9, 0xCD, 0xD6, 0x9B, 0xD3, 0xD3, 0xD2, 0x71,
        ];

        unscramble_filename_v7(353, &mut input);

        assert_eq!(&input, b"x:\\gforce\\binary\\_bin_pc\\mw_intobj.edb\0")
    }

    #[test]
    fn scramble_path() {
        let mut input = b"x:\\gforce\\binary\\_bin_pc\\as_fplay.edb\0".to_vec();
        let output: [u8; 38] = [
            0x62, 0x25, 0x48, 0x54, 0x54, 0x5E, 0x62, 0x54, 0x57, 0x4F, 0x56, 0x5E, 0x64, 0x58,
            0x6A, 0x72, 0x56, 0x5A, 0x5E, 0x66, 0x6C, 0x5E, 0x70, 0x64, 0x5E, 0x64, 0x77, 0x64,
            0x6C, 0x77, 0x74, 0x6A, 0x83, 0x39, 0x71, 0x71, 0x70, 0x0F,
        ];

        scramble_filename_v7(0, &mut input);

        assert_eq!(&input, &output)
    }
}
