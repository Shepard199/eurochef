use super::{float, scaled_speed};

pub(super) const TYPE: u32 = 80;

pub(super) fn path_hash(data: &[Option<u32>]) -> Option<u32> {
    data.get(1).copied().flatten()
}
pub(super) fn speed(data: &[Option<u32>]) -> Option<f32> {
    Some(scaled_speed(float(data, 2)))
}
pub(super) fn acceleration(data: &[Option<u32>]) -> Option<f32> {
    Some(float(data, 3).unwrap_or_default().abs() * 0.1)
}
