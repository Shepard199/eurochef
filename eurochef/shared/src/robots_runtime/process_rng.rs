/// Process-global native LCG at `DAT_007BE1E8`.
///
/// This stream is distinct from the gameplay RNG at `DAT_007BE1E4` and is shared
/// by TextGroup selection, monster-action probability gates, Fluid Y sampling and
/// other engine systems. Hosts therefore pass one session-owned seed across all
/// represented consumers instead of giving each subsystem a private generator.
pub const ROBOTS_PROCESS_LCG_STARTUP_SEED: u32 = 1;
pub const ROBOTS_PROCESS_LCG_MULTIPLIER: u32 = 0x0019_660D;
pub const ROBOTS_PROCESS_LCG_INCREMENT: u32 = 0x3C6E_F35F;
pub const ROBOTS_PROCESS_LCG_UNIT_FLOAT_SCALE: f32 = 1.0 / 2_147_483_648.0;

#[inline]
pub fn robots_process_lcg_step(seed: u32) -> u32 {
    seed.wrapping_mul(ROBOTS_PROCESS_LCG_MULTIPLIER)
        .wrapping_add(ROBOTS_PROCESS_LCG_INCREMENT)
}

/// Consume one `DAT_007BE1E8` draw and return the non-negative 31-bit lane used
/// directly by explosion-fragment randomization (`seed >> 1`). Unknown process
/// state fails closed and consumes nothing.
pub fn robots_process_lcg_next_u31(process_lcg_seed: &mut Option<u32>) -> Option<u32> {
    let seed = robots_process_lcg_step((*process_lcg_seed)?);
    *process_lcg_seed = Some(seed);
    Some(seed >> 1)
}

pub fn robots_process_lcg_next_unit_f32(process_lcg_seed: &mut Option<u32>) -> Option<f32> {
    Some(
        robots_process_lcg_next_u31(process_lcg_seed)? as f32 * ROBOTS_PROCESS_LCG_UNIT_FLOAT_SCALE,
    )
}

/// Native probability prefix used by monster action dispatcher `0x004550A0`.
///
/// `modulus` 0/1 succeeds deterministically and consumes no RNG. Values greater
/// than one consume exactly one `DAT_007BE1E8` draw and continue only when the
/// new seed is divisible by the modulus. `None` means the process-global boundary
/// is unknown; the host must fail closed without inventing or advancing a seed.
pub fn robots_process_lcg_modulo_zero_gate(
    process_lcg_seed: &mut Option<u32>,
    modulus: u32,
) -> Option<bool> {
    if modulus <= 1 {
        return Some(true);
    }
    let seed = robots_process_lcg_step((*process_lcg_seed)?);
    *process_lcg_seed = Some(seed);
    Some(seed % modulus == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monster_action_gate_matches_native_process_lcg_contract() {
        let mut unknown = None;
        assert_eq!(
            robots_process_lcg_modulo_zero_gate(&mut unknown, 0),
            Some(true)
        );
        assert_eq!(
            robots_process_lcg_modulo_zero_gate(&mut unknown, 1),
            Some(true)
        );
        assert_eq!(robots_process_lcg_modulo_zero_gate(&mut unknown, 2), None);

        let mut seed = Some(ROBOTS_PROCESS_LCG_STARTUP_SEED);
        let expected = robots_process_lcg_step(ROBOTS_PROCESS_LCG_STARTUP_SEED);
        assert_eq!(
            robots_process_lcg_modulo_zero_gate(&mut seed, 2),
            Some(expected % 2 == 0)
        );
        assert_eq!(seed, Some(expected));
    }

    #[test]
    fn next_u31_and_unit_float_share_one_native_draw() {
        let mut raw_seed = Some(ROBOTS_PROCESS_LCG_STARTUP_SEED);
        let raw = robots_process_lcg_next_u31(&mut raw_seed).unwrap();
        assert_eq!(raw_seed, Some(0x3C88_596C));
        assert_eq!(raw, 0x1E44_2CB6);

        let mut float_seed = Some(ROBOTS_PROCESS_LCG_STARTUP_SEED);
        let unit = robots_process_lcg_next_unit_f32(&mut float_seed).unwrap();
        assert_eq!(float_seed, raw_seed);
        assert_eq!(unit, raw as f32 * ROBOTS_PROCESS_LCG_UNIT_FLOAT_SCALE);
    }
}
