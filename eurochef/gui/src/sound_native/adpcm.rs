const INDEX_TABLE: [i32; 16] = [-1, -1, -1, -1, 2, 4, 6, 8, -1, -1, -1, -1, 2, 4, 6, 8];
const STEP_TABLE: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66,
    73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449,
    494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272,
    2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493,
    10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767,
];

pub(crate) fn decode_eurocom_ima_adpcm(encoded: &[u8]) -> Result<Vec<i16>, String> {
    if encoded.is_empty() || encoded.len() % 32 != 0 {
        return Err(format!(
            "Eurocom IMA ADPCM requires a non-empty multiple of 32 bytes, got {}",
            encoded.len()
        ));
    }

    let mut samples = Vec::with_capacity(encoded.len() * 56 / 32);
    for block in encoded.chunks_exact(32) {
        let mut predicted = i16::from_le_bytes([block[0], block[1]]) as i32;
        let mut index = (block[2] as usize).min(88);
        for nibble_index in 0..56 {
            let byte = block[4 + nibble_index / 2];
            let nibble = if nibble_index % 2 == 0 {
                byte >> 4
            } else {
                byte & 0x0f
            };
            index = (index as i32 + INDEX_TABLE[nibble as usize]).clamp(0, 88) as usize;
            let step = STEP_TABLE[index];
            let magnitude = nibble & 7;
            let mut difference = step >> 3;
            if magnitude & 4 != 0 {
                difference += step;
            }
            if magnitude & 2 != 0 {
                difference += step >> 1;
            }
            if magnitude & 1 != 0 {
                difference += step >> 2;
            }
            if nibble & 8 != 0 {
                predicted -= difference;
            } else {
                predicted += difference;
            }
            predicted = predicted.clamp(i16::MIN as i32, i16::MAX as i32);
            samples.push(predicted as i16);
        }
    }
    Ok(samples)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_block_decodes_to_56_silent_samples() {
        assert_eq!(decode_eurocom_ima_adpcm(&[0; 32]).unwrap(), vec![0; 56]);
    }

    #[test]
    fn rejects_incomplete_blocks() {
        assert!(decode_eurocom_ima_adpcm(&[0; 31]).is_err());
    }
}
