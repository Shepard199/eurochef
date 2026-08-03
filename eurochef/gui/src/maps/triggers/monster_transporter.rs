pub const TYPE: u32 = 73;

pub fn primary_path_hash(data: &[Option<u32>]) -> Option<u32> {
    data.get(1).copied().flatten()
}

pub fn secondary_path_hash(data: &[Option<u32>]) -> Option<u32> {
    data.get(4).copied().flatten()
}
