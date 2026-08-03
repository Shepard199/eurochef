pub const TYPE: u32 = 60;

pub fn path_hash(data: &[Option<u32>]) -> Option<u32> {
    data.get(1).copied().flatten()
}

pub fn mode(data: &[Option<u32>]) -> Option<u32> {
    data.first().copied().flatten()
}

pub fn flags(data: &[Option<u32>]) -> Option<u32> {
    data.get(2).copied().flatten()
}

pub fn enter_distance(data: &[Option<u32>]) -> Option<f32> {
    data.get(3)
        .copied()
        .flatten()
        .map(|value| value as i32 as f32 * 0.1)
}

pub fn leave_distance(data: &[Option<u32>]) -> Option<f32> {
    data.get(4)
        .copied()
        .flatten()
        .map(|value| value as i32 as f32 * 0.1)
}
