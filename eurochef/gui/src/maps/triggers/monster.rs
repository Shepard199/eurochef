pub const TEST_TYPE: u32 = 3;
pub const BASE_TYPES: [u32; 5] = [10, 11, 18, 33, 74];
pub const FISH_TYPE: u32 = 70;

pub fn is_base_type(trigger_type: u32) -> bool {
    BASE_TYPES.contains(&trigger_type)
}

pub fn is_family_type(trigger_type: u32) -> bool {
    trigger_type == TEST_TYPE || trigger_type == FISH_TYPE || is_base_type(trigger_type)
}

pub fn runtime_selector(trigger_type: u32, data: &[Option<u32>]) -> Option<u32> {
    is_family_type(trigger_type)
        .then(|| data.first().copied().flatten())
        .flatten()
}

pub fn proximity_radius(trigger_type: u32, data: &[Option<u32>]) -> Option<f32> {
    (is_base_type(trigger_type) || trigger_type == FISH_TYPE)
        .then(|| data.get(1).copied().flatten())
        .flatten()
        .map(|value| (value as i32) as f32 * 0.1)
}

pub fn test_runtime_value(trigger_type: u32, data: &[Option<u32>]) -> Option<u32> {
    (trigger_type == TEST_TYPE)
        .then(|| data.get(1).copied().flatten())
        .flatten()
}

pub fn path_hash(trigger_type: u32, data: &[Option<u32>]) -> Option<u32> {
    is_base_type(trigger_type)
        .then(|| data.get(2).copied().flatten())
        .flatten()
}

pub fn data4_value(trigger_type: u32, data: &[Option<u32>]) -> Option<u32> {
    (is_base_type(trigger_type) || trigger_type == TEST_TYPE)
        .then(|| data.get(4).copied().flatten())
        .flatten()
}

pub fn flags(trigger_type: u32, data: &[Option<u32>]) -> Option<u32> {
    (is_base_type(trigger_type) || trigger_type == TEST_TYPE)
        .then(|| data.get(7).copied().flatten())
        .flatten()
}

pub fn data15_value(trigger_type: u32, data: &[Option<u32>]) -> Option<u32> {
    (is_base_type(trigger_type) || trigger_type == TEST_TYPE)
        .then(|| data.get(15).copied().flatten())
        .flatten()
}
