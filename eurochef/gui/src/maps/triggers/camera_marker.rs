pub const TYPE: u32 = 20;

pub fn path_hash(data: &[Option<u32>]) -> Option<u32> {
    data.get(4).copied().flatten()
}

pub fn scaled_data0(data: &[Option<u32>]) -> Option<f32> {
    data.first()
        .copied()
        .flatten()
        .map(|value| value as i32 as f32 * 0.1)
}

pub fn flags(data: &[Option<u32>]) -> Option<u32> {
    data.get(2).copied().flatten()
}
