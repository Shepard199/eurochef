use std::{collections::HashMap, fs, path::Path};

#[derive(Debug, Clone)]
pub(crate) struct NativeWave {
    pub(crate) encoded: Vec<u8>,
    pub(crate) right_encoded: Option<Vec<u8>>,
    pub(crate) frequency: u32,
    pub(crate) total_samples: u32,
    pub(crate) uses_adpcm: bool,
    pub(crate) channels: u16,
}

#[derive(Debug, Default)]
pub(crate) struct NativeSoundCatalog {
    sounds: HashMap<u32, Vec<NativeSource>>,
    streams: Vec<NativeWave>,
    music: HashMap<u32, NativeWave>,
}

#[derive(Debug)]
enum NativeSource {
    Wave(NativeWave),
    Stream(usize),
    SubSound(u32),
}

impl NativeSoundCatalog {
    pub(crate) fn load_pc_robots(root: &Path) -> Result<Self, String> {
        let mut catalog = Self::default();
        let mut files = Vec::new();
        collect_sfx_files(root, &mut files)?;
        files.sort();
        for file in files {
            let _ = catalog.load_bank(&file);
        }
        Ok(catalog)
    }

    pub(crate) fn wave(&self, hashcode: u32, pool_index: usize) -> Option<&NativeWave> {
        self.wave_inner(hashcode, pool_index, 0)
    }

    fn wave_inner(&self, hashcode: u32, pool_index: usize, depth: u8) -> Option<&NativeWave> {
        if depth >= 32 {
            return None;
        }
        if let Some(music) = self.music.get(&hashcode) {
            return Some(music);
        }
        match self.sounds.get(&hashcode)?.get(pool_index)? {
            NativeSource::Wave(wave) => Some(wave),
            NativeSource::Stream(index) => self.streams.get(*index),
            NativeSource::SubSound(hashcode) => self.wave_inner(*hashcode, 0, depth + 1),
        }
    }

    fn load_bank(&mut self, path: &Path) -> Result<(), String> {
        let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
        let mut reader = Reader::new(&bytes, path);
        if reader.take(4)? != b"MUSX" {
            return Ok(());
        }
        let file_hashcode = reader.u32()?;
        let version = reader.i32()?;
        if !matches!(version, 4 | 5 | 6) {
            return Ok(());
        }
        let _file_size = reader.u32()?;
        let _platform = reader.take(4)?;
        let _timespan = reader.u32()?;
        let uses_adpcm = reader.u32()? != 0;
        let _padding = reader.u32()?;
        let section = (file_hashcode >> 12) & 0x0f;
        if section == 0x0d {
            let index_start = reader.u32()? as usize;
            let index_length = reader.u32()? as usize;
            let data_start = reader.u32()? as usize;
            let data_length = reader.u32()? as usize;
            return self.load_streams(&reader, index_start, index_length, data_start, data_length);
        }
        if section == 0x0f {
            let _marker_start = reader.u32()? as usize;
            let _marker_length = reader.u32()? as usize;
            let data_start = reader.u32()? as usize;
            let data_length = reader.u32()? as usize;
            return self.load_music(&reader, file_hashcode, data_start, data_length);
        }
        if section != 0x0e {
            return Ok(());
        }
        let sfx_start = reader.u32()? as usize;
        let _sfx_length = reader.u32()?;
        let sample_info_start = reader.u32()? as usize;
        let _sample_info_length = reader.u32()?;
        let _special_start = reader.u32()?;
        let _special_length = reader.u32()?;
        let sample_data_start = reader.u32()? as usize;
        let _sample_data_length = reader.u32()?;

        let wave_count = reader.at(sample_info_start)?.u32()? as usize;
        let mut waves = Vec::with_capacity(wave_count);
        for index in 0..wave_count {
            let mut wave_reader = reader.at(sample_info_start + 4 + index * 32)?;
            let _flags = wave_reader.u32()?;
            let address = wave_reader.u32()? as usize;
            let _memory_size = wave_reader.u32()?;
            let frequency = wave_reader.u32()?;
            let sample_size = wave_reader.u32()? as usize;
            let _psi_header = wave_reader.u32()?;
            let _loop_start = wave_reader.u32()?;
            let _duration = wave_reader.u32()?;
            let encoded = reader
                .range(sample_data_start + address, sample_size)?
                .to_vec();
            waves.push(NativeWave {
                encoded,
                right_encoded: None,
                frequency,
                total_samples: (sample_size as u32 * 56) / 32,
                uses_adpcm,
                channels: 1,
            });
        }

        let sfx_count = reader.at(sfx_start)?.u32()? as usize;
        for index in 0..sfx_count {
            let mut table = reader.at(sfx_start + 4 + index * 8)?;
            let hashcode = 0x1af0_0000 | table.u32()?;
            let offset = table.u32()? as usize;
            let mut sound = reader.at(sfx_start + offset)?;
            sound.skip(12)?;
            sound.skip(4)?; // PC group hash, channel count, padding
            let mut flags = 0u16;
            for bit in 0..16 {
                if sound.take(1)?[0] == 1 {
                    flags |= 1 << bit;
                }
            }
            if version > 4 {
                sound.skip(18)?; // user flags plus Doppler/UserValue
            }
            if version > 5 {
                sound.skip(2)?;
            }
            let pool_count = sound.u16()? as usize;
            let mut pool = Vec::with_capacity(pool_count);
            for _ in 0..pool_count {
                let wave_index = sound.i16()?;
                sound.skip(6)?;
                if flags & (1 << 10) != 0 {
                    pool.push(NativeSource::SubSound(
                        0x1af0_0000 | wave_index as u16 as u32,
                    ));
                } else if wave_index >= 0 {
                    let wave = waves.get(wave_index as usize).ok_or_else(|| {
                        format!(
                            "{}: sound 0x{hashcode:08x} references wave {wave_index}",
                            path.display()
                        )
                    })?;
                    pool.push(NativeSource::Wave(wave.clone()));
                } else {
                    let stream_index = if version == 6 {
                        (wave_index as u16 & 0x3fff) as usize
                    } else {
                        wave_index.unsigned_abs() as usize - 1
                    };
                    pool.push(NativeSource::Stream(stream_index));
                }
            }
            if !pool.is_empty() {
                self.sounds.entry(hashcode).or_insert(pool);
            }
        }
        Ok(())
    }

    fn load_streams(
        &mut self,
        reader: &Reader<'_>,
        index_start: usize,
        index_length: usize,
        data_start: usize,
        _data_length: usize,
    ) -> Result<(), String> {
        let mut offsets = reader.at(index_start)?;
        for _ in 0..index_length / 4 {
            let offset = offsets.u32()? as usize;
            let mut stream = reader.at(data_start + offset)?;
            let _marker_size = stream.u32()?;
            let audio_offset = stream.u32()? as usize;
            let audio_size = stream.u32()? as usize;
            let start_marker_count = stream.u32()? as usize;
            let marker_count = stream.u32()? as usize;
            stream.skip(16)?;
            stream.skip(start_marker_count * 24 + marker_count * 20)?;
            let encoded = reader
                .range(data_start + audio_offset, audio_size)?
                .to_vec();
            self.streams.push(NativeWave {
                total_samples: encoded.len() as u32 / 32 * 56,
                encoded,
                right_encoded: None,
                frequency: 22_050,
                uses_adpcm: true,
                channels: 1,
            });
        }
        Ok(())
    }

    fn load_music(
        &mut self,
        reader: &Reader<'_>,
        file_hashcode: u32,
        data_start: usize,
        data_length: usize,
    ) -> Result<(), String> {
        let data = reader.range(data_start, data_length)?;
        if data.len() % 64 != 0 {
            return Err(format!(
                "{}: PC music data is not stereo 32-byte interleaves",
                reader.path.display()
            ));
        }
        let mut left = Vec::with_capacity(data.len() / 2);
        let mut right = Vec::with_capacity(data.len() / 2);
        for pair in data.chunks_exact(64) {
            left.extend_from_slice(&pair[..32]);
            right.extend_from_slice(&pair[32..]);
        }
        self.music.insert(
            0x1b00_0000 | (file_hashcode & 0x0fff),
            NativeWave {
                total_samples: left.len() as u32 / 32 * 56,
                encoded: left,
                right_encoded: Some(right),
                frequency: 32_000,
                uses_adpcm: true,
                channels: 2,
            },
        );
        Ok(())
    }
}

fn collect_sfx_files(root: &Path, files: &mut Vec<std::path::PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(root).map_err(|error| format!("{}: {error}", root.display()))? {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.is_dir() {
            collect_sfx_files(&path, files)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("sfx"))
        {
            files.push(path);
        }
    }
    Ok(())
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
    path: &'a Path,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8], path: &'a Path) -> Self {
        Self {
            bytes,
            position: 0,
            path,
        }
    }
    fn at(&self, position: usize) -> Result<Self, String> {
        if position > self.bytes.len() {
            return Err(format!(
                "{}: offset 0x{position:x} outside file",
                self.path.display()
            ));
        }
        Ok(Self {
            bytes: self.bytes,
            position,
            path: self.path,
        })
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], String> {
        let value = self.range(self.position, count)?;
        self.position += count;
        Ok(value)
    }
    fn range(&self, start: usize, count: usize) -> Result<&'a [u8], String> {
        self.bytes
            .get(start..start.saturating_add(count))
            .ok_or_else(|| format!("{}: truncated MUSX", self.path.display()))
    }
    fn skip(&mut self, count: usize) -> Result<(), String> {
        self.take(count).map(|_| ())
    }
    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn i16(&mut self) -> Result<i16, String> {
        Ok(i16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> Result<i32, String> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
}
