pub const TYPE: u32 = 72;

pub fn path_hash(data: &[Option<u32>]) -> Option<u32> {
    data.first().copied().flatten()
}
