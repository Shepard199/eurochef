pub const TYPE: u32 = 48;

pub fn runtime_selector(data: &[Option<u32>]) -> Option<u32> {
    data.first().copied().flatten()
}

pub fn runtime_uid(data: &[Option<u32>]) -> Option<u32> {
    data.get(1).copied().flatten()
}

pub fn flags(data: &[Option<u32>]) -> Option<u32> {
    data.get(2).copied().flatten()
}

pub fn text_group(data: &[Option<u32>]) -> Option<u32> {
    data.get(3).copied().flatten()
}

pub fn alternate_cutscenes(data: &[Option<u32>]) -> [Option<u32>; 4] {
    std::array::from_fn(|index| data.get(index + 4).copied().flatten())
}

pub fn is_null_cutscene(hash: u32) -> bool {
    matches!(hash, 0 | u32::MAX | 0x0400_0000)
}
