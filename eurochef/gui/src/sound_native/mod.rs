mod adpcm;
mod catalog;

pub(crate) use adpcm::decode_eurocom_ima_adpcm;
pub(crate) use catalog::{
    NativeSoundCatalog, NativeSoundProfile, NativeSoundProfileCatalog, NativeWave,
};

pub(crate) struct DecodedWave {
    pub(crate) samples: Vec<i16>,
    pub(crate) frequency: u32,
    pub(crate) channels: u16,
}

pub(crate) fn decode_wave(wave: &NativeWave) -> Result<DecodedWave, String> {
    let mut samples = if wave.uses_adpcm {
        decode_eurocom_ima_adpcm(&wave.encoded)?
    } else {
        wave.encoded
            .chunks_exact(2)
            .map(|sample| i16::from_le_bytes([sample[0], sample[1]]))
            .collect()
    };
    samples.truncate(wave.total_samples as usize);
    if let Some(right) = &wave.right_encoded {
        let mut right = decode_eurocom_ima_adpcm(right)?;
        right.truncate(wave.total_samples as usize);
        let frames = samples.len().min(right.len());
        let mut interleaved = Vec::with_capacity(frames * 2);
        for (left, right) in samples.into_iter().zip(right).take(frames) {
            interleaved.extend([left, right]);
        }
        samples = interleaved;
    }
    Ok(DecodedWave {
        samples,
        frequency: wave.frequency,
        channels: wave.channels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stereo_music_interleaves_decoded_channels() {
        let wave = NativeWave {
            encoded: vec![0; 32],
            right_encoded: Some({
                let mut data = vec![0; 32];
                data[0] = 1;
                data
            }),
            frequency: 32_000,
            total_samples: 56,
            uses_adpcm: true,
            channels: 2,
        };
        let decoded = decode_wave(&wave).unwrap();
        assert_eq!(decoded.channels, 2);
        assert_eq!(decoded.samples.len(), 112);
        assert_eq!(decoded.samples[0], 0);
        assert_ne!(decoded.samples[1], 0);
    }
}
