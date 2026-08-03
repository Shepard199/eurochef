use std::io::{Seek, SeekFrom};

use binrw::BinReaderExt;
use serde::Serialize;

use crate::{edb::EdbFile, error::Result, Hashcode};

pub const EXGEO_PARTICLE_SIZE: usize = 0x118;
pub const EXGEO_PARTICLE_WORDS: usize = EXGEO_PARTICLE_SIZE / 4;
pub const EXGEO_PARTICLE_CURVE_OFFSET: usize = 0x108;
pub const EXGEO_PARTICLE_CURVE_RECORD_SIZE: usize = 0x10;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct EXGeoParticleCurveRecord {
    /// Native channel consumed by EXParticleSys. Channels 0..2 are Euler rotation,
    /// 3..5 are XYZ scale and 6..9 are RGBA colour.
    pub channel: u32,
    /// Normalized particle age in percent, matching the runtime 0..100 counter.
    pub age_percent: f32,
    pub value: f32,
    /// Native linear slope per one normalized-age unit.
    pub slope: f32,
}

impl EXGeoParticleCurveRecord {
    pub fn is_visual_channel(self) -> bool {
        self.channel <= 9
    }
}

/// Structurally recovered Robots EXGeoParticle object.
///
/// The 0x118-byte fixed record is followed by 0x10-byte native curve records ending
/// in channel 0xFFFFFFFF. Raw words are kept alongside promoted fields so a later
/// correction never requires reparsing or silently discards original data.
#[derive(Debug, Clone, Serialize)]
pub struct EXGeoParticle {
    pub hashcode: Hashcode,
    pub index: usize,
    pub address: u32,
    pub common: u32,
    pub raw_words: Vec<u32>,
    /// EXGeoParticle+0x08. A non-zero value is the particle render entity used by
    /// the native material/geometry path.
    pub render_entity: Option<Hashcode>,
    /// EXGeoParticle+0xF4. 0xFFFFFFFF selects EXParticleSys; 0x17000001 selects
    /// the corpus-proven HT_ParticleType_Inhibitted implementation.
    pub particle_type_selector: u32,
    /// EXGeoParticle+0xF8/+0xFC packed render-resource selectors.
    pub render_resource_selectors: Vec<u32>,
    /// Compatibility alias retained for existing corpus reports.
    pub tail_array_a: Vec<u32>,
    /// EXGeoParticle+0x100/+0x104 local entity hierarchy references.
    pub entity_references: Vec<u32>,
    /// Native appended channel records beginning at EXGeoParticle+0x108.
    pub curves: Vec<EXGeoParticleCurveRecord>,
}

impl EXGeoParticle {
    pub fn raw_u32(&self, object_offset: usize) -> Option<u32> {
        (object_offset % 4 == 0)
            .then(|| self.raw_words.get(object_offset / 4).copied())
            .flatten()
    }

    pub fn raw_f32(&self, object_offset: usize) -> Option<f32> {
        self.raw_u32(object_offset).map(f32::from_bits)
    }

    pub fn finite_f32(&self, object_offset: usize, fallback: f32) -> f32 {
        self.raw_f32(object_offset)
            .filter(|value| value.is_finite())
            .unwrap_or(fallback)
    }

    pub fn vec3(&self, object_offset: usize, fallback: [f32; 3]) -> [f32; 3] {
        [
            self.finite_f32(object_offset, fallback[0]),
            self.finite_f32(object_offset + 4, fallback[1]),
            self.finite_f32(object_offset + 8, fallback[2]),
        ]
    }

    /// Native EXParticleSys optimized storage selector at +0x0C.
    pub fn optimized_mode(&self) -> u32 {
        self.raw_u32(0x0C).unwrap_or(0)
    }

    pub fn behavior_flags(&self) -> (u16, u16) {
        (
            self.raw_u32(0x30).unwrap_or(0) as u16,
            (self.raw_u32(0x30).unwrap_or(0) >> 16) as u16,
        )
    }

    pub fn fixed_step(&self) -> f32 {
        let step = self.finite_f32(0x3C, 1.0 / 60.0);
        if step > 0.0 {
            step
        } else {
            1.0 / 60.0
        }
    }

    /// Spawn-position centre and symmetric extent consumed by 0x0051F72B.
    pub fn spawn_position_center(&self) -> [f32; 3] {
        self.vec3(0x48, [0.0; 3])
    }

    pub fn spawn_position_extent(&self) -> [f32; 3] {
        self.vec3(0x54, [0.0; 3])
    }

    /// Initial XYZ scale centre and symmetric extent consumed by 0x0051FDAB.
    pub fn initial_scale_center(&self) -> [f32; 3] {
        self.vec3(0x60, [1.0; 3])
    }

    pub fn initial_scale_extent(&self) -> [f32; 3] {
        self.vec3(0x6C, [0.0; 3])
    }

    /// Per-fixed-step additive velocity term consumed by 0x0051EC1C.
    pub fn acceleration(&self) -> [f32; 3] {
        self.vec3(0x78, [0.0; 3])
    }

    /// Serialized vector copied to EXParticleSys+0x120. The default PC spawn/update
    /// path does not consume it directly, so its gameplay role remains intentionally
    /// unnamed until a concrete alternate-path consumer is proven.
    pub fn runtime_vector_120(&self) -> [f32; 3] {
        self.vec3(0x84, [0.0; 3])
    }

    /// Per-fixed-step component multiplier consumed before acceleration.
    pub fn velocity_multiplier(&self) -> [f32; 3] {
        self.vec3(0x90, [1.0; 3])
    }

    pub fn azimuth_center(&self) -> f32 {
        self.finite_f32(0x9C, 0.0)
    }

    pub fn azimuth_extent(&self) -> f32 {
        self.finite_f32(0xA0, 0.0).abs()
    }

    pub fn elevation_center(&self) -> f32 {
        self.finite_f32(0xA4, 0.0)
    }

    pub fn elevation_extent(&self) -> f32 {
        self.finite_f32(0xA8, 0.0).abs()
    }

    pub fn speed_center(&self) -> f32 {
        self.finite_f32(0xAC, 0.0)
    }

    pub fn speed_extent(&self) -> f32 {
        self.finite_f32(0xB0, 0.0).abs()
    }

    /// Native lifetime sample is base + random[-1,1] * extent.
    pub fn lifetime_center(&self) -> f32 {
        self.finite_f32(0xB4, self.fixed_step())
            .max(self.fixed_step())
    }

    pub fn lifetime_extent(&self) -> f32 {
        self.finite_f32(0xB8, 0.0).abs()
    }

    /// Native particles-per-second value copied to EXParticleSys+0x1D0.
    pub fn emission_rate(&self) -> f32 {
        self.finite_f32(0xC0, 0.0).max(0.0)
    }

    /// Native particle-record pool capacity.
    pub fn pool_limit(&self) -> usize {
        self.raw_u32(0xC4).unwrap_or(0).clamp(0, 4096) as usize
    }

    pub fn resource_selection_mode(&self) -> u32 {
        self.raw_u32(0xD8).unwrap_or(0)
    }

    pub fn curve_records_for_channel(
        &self,
        channel: u32,
    ) -> impl Iterator<Item = &EXGeoParticleCurveRecord> {
        self.curves
            .iter()
            .filter(move |record| record.channel == channel)
    }

    pub fn read_all(edb: &mut EdbFile) -> Result<Vec<Self>> {
        let headers = edb.header.particle_list.data().clone();
        let mut particles = Vec::with_capacity(headers.len());
        for (index, header) in headers.iter().enumerate() {
            particles.push(Self::read(header.hashcode, index, header.address, edb)?);
        }
        Ok(particles)
    }

    pub fn read_hashcodes(edb: &mut EdbFile, hashcodes: &[Hashcode]) -> Result<Vec<Self>> {
        let headers = edb.header.particle_list.data().clone();
        let mut particles = Vec::new();
        for (index, header) in headers.iter().enumerate() {
            if hashcodes.contains(&header.hashcode)
                || (header.hashcode & 0x8000_0000 != 0
                    && hashcodes
                        .iter()
                        .any(|hash| hash & 0x8000_0000 != 0 && (hash & 0xffff) as usize == index))
            {
                particles.push(Self::read(header.hashcode, index, header.address, edb)?);
            }
        }
        Ok(particles)
    }

    fn read(hashcode: Hashcode, index: usize, address: u32, edb: &mut EdbFile) -> Result<Self> {
        let return_position = edb.stream_position()?;
        edb.seek(SeekFrom::Start(address as u64))?;
        let raw_words: [u32; EXGEO_PARTICLE_WORDS] = edb.read_type(edb.endian)?;
        if raw_words[0] != 0x700 {
            edb.seek(SeekFrom::Start(return_position))?;
            return Err(binrw::Error::AssertFail {
                pos: address as u64,
                message: format!(
                    "EXGeoParticle 0x{hashcode:08X} has common 0x{:08X}, expected 0x00000700",
                    raw_words[0]
                ),
            }
            .into());
        }

        let render_entity = (raw_words[0x08 / 4] != 0).then_some(raw_words[0x08 / 4]);
        if let Some(entity) = render_entity {
            edb.add_reference_internal(entity);
        }

        let particle_type_selector = raw_words[0xF4 / 4];
        let render_resource_selectors = read_relative_u32_array(
            edb,
            address as u64 + 0xFC,
            raw_words[0xF8 / 4] as usize,
            raw_words[0xFC / 4] as i32,
        )?;
        let entity_references = read_relative_u32_array(
            edb,
            address as u64 + 0x104,
            raw_words[0x100 / 4] as usize,
            raw_words[0x104 / 4] as i32,
        )?;
        for entity in &entity_references {
            edb.add_reference_internal(*entity);
        }

        let curves = read_curve_records(edb, address as u64, edb.header.file_size as u64)?;
        edb.seek(SeekFrom::Start(return_position))?;

        Ok(Self {
            hashcode,
            index,
            address,
            common: raw_words[0],
            raw_words: raw_words.to_vec(),
            render_entity,
            particle_type_selector,
            tail_array_a: render_resource_selectors.clone(),
            render_resource_selectors,
            entity_references,
            curves,
        })
    }
}

fn read_relative_u32_array(
    edb: &mut EdbFile,
    pointer_position: u64,
    count: usize,
    relative: i32,
) -> Result<Vec<u32>> {
    if count == 0 {
        return Ok(Vec::new());
    }
    let target = pointer_position as i64 + relative as i64;
    if target < 0 {
        return Err(binrw::Error::AssertFail {
            pos: pointer_position,
            message: format!("negative EXGeoParticle relative target: {target}"),
        }
        .into());
    }

    let return_position = edb.stream_position()?;
    edb.seek(SeekFrom::Start(target as u64))?;
    let mut values = Vec::with_capacity(count);
    for _ in 0..count.min(4096) {
        values.push(edb.read_type(edb.endian)?);
    }
    edb.seek(SeekFrom::Start(return_position))?;
    Ok(values)
}

fn read_curve_records(
    edb: &mut EdbFile,
    object_address: u64,
    file_size: u64,
) -> Result<Vec<EXGeoParticleCurveRecord>> {
    let return_position = edb.stream_position()?;
    let mut cursor = object_address + EXGEO_PARTICLE_CURVE_OFFSET as u64;
    let mut curves = Vec::new();

    while cursor + EXGEO_PARTICLE_CURVE_RECORD_SIZE as u64 <= file_size && curves.len() < 4096 {
        edb.seek(SeekFrom::Start(cursor))?;
        let channel: u32 = edb.read_type(edb.endian)?;
        if channel == u32::MAX {
            break;
        }
        // Native consumers only dispatch channels 0..11. A larger value means this
        // object has no appended curve stream at the expected location; stop instead
        // of walking into an unrelated relptr payload.
        if channel > 11 {
            break;
        }
        let age_percent: f32 = edb.read_type(edb.endian)?;
        let value: f32 = edb.read_type(edb.endian)?;
        let slope: f32 = edb.read_type(edb.endian)?;
        if !age_percent.is_finite() || !value.is_finite() || !slope.is_finite() {
            break;
        }
        curves.push(EXGeoParticleCurveRecord {
            channel,
            age_percent,
            value,
            slope,
        });
        cursor += EXGEO_PARTICLE_CURVE_RECORD_SIZE as u64;
    }

    edb.seek(SeekFrom::Start(return_position))?;
    Ok(curves)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proven_particle_offsets_are_word_aligned() {
        assert_eq!(EXGEO_PARTICLE_SIZE, 0x118);
        assert_eq!(EXGEO_PARTICLE_WORDS, 70);
        assert_eq!(0xF4 / 4, 61);
        assert_eq!(0x104 / 4, 65);
        assert_eq!(EXGEO_PARTICLE_CURVE_OFFSET, 0x108);
    }

    #[test]
    fn visual_curve_channels_match_native_dispatch() {
        for channel in 0..=9 {
            assert!(EXGeoParticleCurveRecord {
                channel,
                age_percent: 0.0,
                value: 0.0,
                slope: 0.0,
            }
            .is_visual_channel());
        }
        assert!(!EXGeoParticleCurveRecord {
            channel: 10,
            age_percent: 0.0,
            value: 0.0,
            slope: 0.0,
        }
        .is_visual_channel());
    }
}
