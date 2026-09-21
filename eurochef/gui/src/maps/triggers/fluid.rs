pub const TYPE: u32 = 50;

/// Number of draws from the process-global gameplay RNG (`0x007BE1E4`) consumed by
/// `XItemHandler_Fluid::Setup` after the grid/entity validation has succeeded.
///
/// Native `0x0040E000` performs one centre disturbance without shared RNG, then loops
/// disturbance counters `1..data[5]`. X uses `FUN_00509C48` unless `width - 2 == 0`;
/// Y uses the separate `0x007BE1E8` LCG and is intentionally not counted here.
pub(crate) fn initial_shared_rng_draw_count(data: &[Option<u32>]) -> Option<u32> {
    let width = data.first().copied().flatten()?;
    let height = data.get(1).copied().flatten()?;
    if width == 0 || height == 0 {
        return None;
    }

    let disturbances = data.get(5).copied().flatten().unwrap_or_default();
    if width == 2 {
        Some(0)
    } else {
        Some(disturbances.saturating_sub(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fluid_initial_shared_rng_draw_count_matches_native_loop() {
        let mut data = [None; 16];
        data[0] = Some(13);
        data[1] = Some(19);
        data[5] = Some(5);
        assert_eq!(initial_shared_rng_draw_count(&data), Some(4));

        data[0] = Some(2);
        assert_eq!(initial_shared_rng_draw_count(&data), Some(0));

        data[0] = Some(13);
        data[5] = Some(1);
        assert_eq!(initial_shared_rng_draw_count(&data), Some(0));

        data[0] = Some(0);
        assert_eq!(initial_shared_rng_draw_count(&data), None);
    }
}
