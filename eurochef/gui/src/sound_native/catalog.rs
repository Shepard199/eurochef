use std::{
    collections::HashMap,
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct NativeSoundProfile {
    pub(crate) inner_radius: f32,
    pub(crate) outer_radius: f32,
    pub(crate) duration_seconds: f32,
    pub(crate) looping: bool,
    pub(crate) tracking_3d: i8,
    pub(crate) sample_streamed: bool,
    pub(crate) is_3d: bool,
    pub(crate) ducker_length: i16,
    pub(crate) min_delay: i16,
    pub(crate) max_delay: i16,
    pub(crate) reverb_send: i8,
    pub(crate) tracking_type: u8,
    pub(crate) max_voices: u8,
    pub(crate) priority: u8,
    pub(crate) ducker: i8,
    pub(crate) master_volume: f32,
    pub(crate) group_hashcode: u16,
    pub(crate) group_max_channels: u8,
    pub(crate) flags: u16,
    pub(crate) user_flags: u16,
    pub(crate) doppler_value: i8,
    pub(crate) user_value: i8,
    pub(crate) sample_count: u16,
}

impl NativeSoundProfile {
    pub(crate) fn is_multi_sample(self) -> bool {
        self.flags & (1 << 3) != 0
    }

    pub(crate) fn is_shuffled(self) -> bool {
        self.flags & (1 << 5) != 0
    }

    pub(crate) fn is_polyphonic(self) -> bool {
        self.flags & (1 << 7) != 0
    }

    pub(crate) fn has_random_delay(self) -> bool {
        self.min_delay != 0 || self.max_delay != 0
    }
}

impl Default for NativeSoundProfile {
    fn default() -> Self {
        Self {
            inner_radius: 0.0,
            outer_radius: 0.0,
            duration_seconds: 0.0,
            looping: false,
            tracking_3d: 0,
            sample_streamed: false,
            is_3d: false,
            ducker_length: 0,
            min_delay: 0,
            max_delay: 0,
            reverb_send: 0,
            tracking_type: 0,
            max_voices: 0,
            priority: 0,
            ducker: 0,
            master_volume: 1.0,
            group_hashcode: 0,
            group_max_channels: 0,
            flags: 0,
            user_flags: 0,
            doppler_value: 0,
            user_value: 0,
            sample_count: 0,
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct NativeSoundProfileCatalog {
    profiles: HashMap<u32, NativeSoundProfile>,
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

impl NativeSoundProfileCatalog {
    pub(crate) fn load_pc_robots(root: &Path) -> Result<Self, String> {
        let search_root = preferred_sound_profile_root(root);
        let mut files = Vec::new();
        collect_sfx_files(&search_root, &mut files)?;
        files.sort();

        let mut catalog = Self::default();
        for file in files.iter().filter(|path| is_sound_details_file(path)) {
            catalog.load_details_file(file)?;
        }
        for file in files.iter().filter(|path| !is_sound_details_file(path)) {
            catalog.load_bank_profiles(file)?;
        }
        Ok(catalog)
    }

    pub(crate) fn profile(&self, hashcode: u32) -> Option<NativeSoundProfile> {
        self.profiles.get(&hashcode).copied()
    }

    fn load_details_file(&mut self, path: &Path) -> Result<(), String> {
        let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
        if bytes.len() < 0x28 || &bytes[..4] != b"MUSX" {
            return Ok(());
        }

        let mut reader = Reader::new(&bytes, path).at(0x20)?;
        let minimum = reader.u32()?;
        let maximum = reader.u32()?;
        if minimum & 0xFFFF_0000 != maximum & 0xFFFF_0000 || maximum < minimum {
            return Err(format!(
                "{}: invalid sound-details range 0x{minimum:08x}..0x{maximum:08x}",
                path.display()
            ));
        }

        let prefix = minimum & 0xFFFF_0000;
        let count = (maximum & 0x0000_FFFF) as usize + 1;
        for index in 0..count {
            let hashcode = prefix | index as u32;
            let mut item = Reader::new(&bytes, path).at(0x28 + index * 12)?;
            let profile = NativeSoundProfile {
                inner_radius: item.u16()? as f32,
                outer_radius: item.u16()? as f32,
                duration_seconds: item.f32()?,
                looping: item.u8()? != 0,
                tracking_3d: item.i8()?,
                sample_streamed: item.u8()? != 0,
                is_3d: item.u8()? != 0,
                ..NativeSoundProfile::default()
            };
            if hashcode >= minimum && hashcode <= maximum {
                self.profiles.entry(hashcode).or_insert(profile);
            }
        }
        Ok(())
    }

    fn load_bank_profiles(&mut self, path: &Path) -> Result<(), String> {
        let mut file =
            fs::File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
        let mut header = [0u8; 36];
        if file.read_exact(&mut header).is_err() || &header[..4] != b"MUSX" {
            return Ok(());
        }
        let file_hashcode = u32::from_le_bytes(header[4..8].try_into().unwrap());
        let version = i32::from_le_bytes(header[8..12].try_into().unwrap());
        if !matches!(version, 4 | 5 | 6) || ((file_hashcode >> 12) & 0x0f) != 0x0e {
            return Ok(());
        }

        file.seek(SeekFrom::Start(0))
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let mut reader = Reader::new(&bytes, path).at(32)?;
        let sfx_start = reader.u32()? as usize;
        let mut table_header = Reader::new(&bytes, path).at(sfx_start)?;
        let count = table_header.u32()? as usize;
        for index in 0..count {
            let mut table = Reader::new(&bytes, path).at(sfx_start + 4 + index * 8)?;
            let hashcode = 0x1AF0_0000 | table.u32()?;
            let offset = table.u32()? as usize;
            let mut sound = Reader::new(&bytes, path).at(sfx_start + offset)?;
            let ducker_length = sound.i16()?;
            let min_delay = sound.i16()?;
            let max_delay = sound.i16()?;
            let reverb_send = sound.i8()?;
            let tracking_type = sound.u8()?;
            let max_voices = sound.i8()?.max(0) as u8;
            let priority = sound.i8()?.max(0) as u8;
            let ducker = sound.i8()?;
            let master_volume = (sound.i8()?.max(0) as f32 / 100.0).clamp(0.0, 1.27);
            let group_hashcode = sound.u16()?;
            let group_max_channels = sound.i8()?.max(0) as u8;
            sound.skip(1)?; // serialized padding
            let mut flags = 0u16;
            for bit in 0..16 {
                if sound.u8()? == 1 {
                    flags |= 1 << bit;
                }
            }
            let mut user_flags = 0u16;
            let mut doppler_value = 0i8;
            let mut user_value = 0i8;
            if version > 4 {
                for bit in 0..16 {
                    if sound.u8()? == 1 {
                        user_flags |= 1 << bit;
                    }
                }
                doppler_value = sound.i8()?;
                user_value = sound.i8()?;
            }
            let sample_count = sound.u16()?;

            let profile = self.profiles.entry(hashcode).or_default();
            profile.ducker_length = ducker_length;
            profile.min_delay = min_delay;
            profile.max_delay = max_delay;
            profile.reverb_send = reverb_send;
            profile.tracking_type = tracking_type;
            profile.max_voices = max_voices;
            profile.priority = priority;
            profile.ducker = ducker;
            profile.master_volume = master_volume;
            profile.group_hashcode = group_hashcode;
            profile.group_max_channels = group_max_channels;
            profile.flags = flags;
            profile.user_flags = user_flags;
            profile.doppler_value = doppler_value;
            profile.user_value = user_value;
            profile.sample_count = sample_count;
            profile.looping = flags & (1 << 6) != 0;
            profile.is_3d |= tracking_type & 0x01 != 0;
        }
        Ok(())
    }
}

fn preferred_sound_profile_root(root: &Path) -> PathBuf {
    let candidates = [
        root.to_path_buf(),
        root.join("_eurotools_out/extracted_usa/robots/binary/_bin_pc/audio"),
        root.join("extracted_usa/robots/binary/_bin_pc/audio"),
        root.join("robots/binary/_bin_pc/audio"),
        root.join("audio"),
    ];
    candidates
        .into_iter()
        .find(|candidate| directory_has_sound_details(candidate))
        .unwrap_or_else(|| root.to_path_buf())
}

fn directory_has_sound_details(path: &Path) -> bool {
    fs::read_dir(path).ok().is_some_and(|entries| {
        entries
            .filter_map(Result::ok)
            .any(|entry| is_sound_details_file(&entry.path()))
    })
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

fn is_sound_details_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.to_ascii_lowercase().contains("sounddetails"))
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
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn i8(&mut self) -> Result<i8, String> {
        Ok(self.take(1)?[0] as i8)
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
    fn f32(&mut self) -> Result<f32, String> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_robots_sound_profile_policy_census_when_requested() {
        let Ok(root) = std::env::var("ROBOTS_SOUND_PROFILE_ROOT") else {
            return;
        };
        let root = Path::new(&root);
        let profiles = NativeSoundProfileCatalog::load_pc_robots(root)
            .expect("load Robots native sound profiles");
        let sounds =
            NativeSoundCatalog::load_pc_robots(root).expect("load Robots native sound sources");

        let mut flag_counts = [0usize; 16];
        let mut tracking_counts = std::collections::BTreeMap::<u8, usize>::new();
        let mut delayed = 0usize;
        let mut delay_ranges = std::collections::BTreeMap::<(i16, i16), usize>::new();
        let mut reverb_nonzero = 0usize;
        let mut doppler_nonzero = 0usize;
        let mut grouped = 0usize;
        let mut delayed_rows = Vec::new();
        for (hashcode, profile) in &profiles.profiles {
            for (bit, count) in flag_counts.iter_mut().enumerate() {
                *count += usize::from(profile.flags & (1 << bit) != 0);
            }
            *tracking_counts.entry(profile.tracking_type).or_default() += 1;
            if profile.min_delay != 0 || profile.max_delay != 0 {
                delayed += 1;
                *delay_ranges
                    .entry((profile.min_delay, profile.max_delay))
                    .or_default() += 1;
                delayed_rows.push((
                    *hashcode,
                    profile.min_delay,
                    profile.max_delay,
                    profile.flags,
                    profile.tracking_type,
                    sounds
                        .sounds
                        .get(hashcode)
                        .map(Vec::len)
                        .unwrap_or_default(),
                ));
            }
            reverb_nonzero += usize::from(profile.reverb_send != 0);
            doppler_nonzero += usize::from(profile.doppler_value != 0);
            grouped += usize::from(profile.group_hashcode != 0);
        }
        let multi_source = sounds
            .sounds
            .values()
            .filter(|sources| sources.len() > 1)
            .count();
        let maximum_sources = sounds
            .sounds
            .values()
            .map(Vec::len)
            .max()
            .unwrap_or_default();
        delayed_rows.sort();
        eprintln!("delayed profile rows: {delayed_rows:#?}");
        eprintln!(
            "sound policy census: profiles={} sounds={} delayed={} delay_ranges={:?} tracking={:?} flags={:?} reverb_nonzero={} doppler_nonzero={} grouped={} multi_source={} max_sources={}",
            profiles.profiles.len(),
            sounds.sounds.len(),
            delayed,
            delay_ranges,
            tracking_counts,
            flag_counts,
            reverb_nonzero,
            doppler_nonzero,
            grouped,
            multi_source,
            maximum_sources,
        );
        let sample_count_mismatches = profiles
            .profiles
            .iter()
            .filter_map(|(hashcode, profile)| {
                let sources = sounds.sounds.get(hashcode)?;
                (profile.sample_count as usize != sources.len()).then_some((
                    *hashcode,
                    profile.sample_count,
                    sources.len(),
                ))
            })
            .collect::<Vec<_>>();
        assert!(
            sample_count_mismatches.is_empty(),
            "serialized sample_count mismatches: {sample_count_mismatches:#?}"
        );
        assert_eq!(profiles.profiles.len(), 1335);
        assert_eq!(sounds.sounds.len(), 1240);
        assert_eq!(flag_counts[3], 93, "MultiSample profile count changed");
        assert_eq!(flag_counts[5], 49, "Shuffled profile count changed");
        assert_eq!(flag_counts[7], 86, "Polyphonic profile count changed");
        assert_eq!(delayed, 60);
        assert_eq!(doppler_nonzero, 0);
    }

    #[test]
    fn real_robots_object_audio_profiles_when_requested() {
        let Ok(root) = std::env::var("ROBOTS_SOUND_PROFILE_ROOT") else {
            return;
        };
        let catalog = NativeSoundProfileCatalog::load_pc_robots(Path::new(&root))
            .expect("load Robots native sound profiles");

        let fan = catalog.profile(0x1AF0_031D).expect("fan loop profile");
        assert_eq!((fan.inner_radius, fan.outer_radius), (10.0, 40.0));
        assert!(fan.is_3d);
        assert_eq!(fan.tracking_type, 1);
        assert!(fan.looping);
        assert!((fan.master_volume - 0.8).abs() < 0.0001);

        let lift = catalog.profile(0x1AF0_032E).expect("lift loop profile");
        assert_eq!((lift.inner_radius, lift.outer_radius), (1.0, 30.0));
        assert_eq!(lift.tracking_type, 1);
        assert!(lift.looping);
        assert!((lift.master_volume - 0.65).abs() < 0.0001);

        let vehicle = catalog.profile(0x1AF0_0377).expect("vehicle loop profile");
        assert_eq!((vehicle.inner_radius, vehicle.outer_radius), (3.0, 30.0));
        assert_eq!(vehicle.tracking_type, 1);
        assert!(vehicle.looping);
        assert!((vehicle.master_volume - 0.65).abs() < 0.0001);
    }
}
