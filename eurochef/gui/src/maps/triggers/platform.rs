use glam::Vec3;

use super::{float, scaled_speed};

pub(super) const TYPE: u32 = 8;

pub(super) fn path_hash(data: &[Option<u32>]) -> Option<u32> {
    data.get(2).copied().flatten()
}
pub(super) fn speed(data: &[Option<u32>]) -> Option<f32> {
    Some(scaled_speed(float(data, 5)))
}
pub(super) fn acceleration(data: &[Option<u32>]) -> Option<f32> {
    Some(float(data, 6).unwrap_or_default().abs() * 0.1)
}

pub(super) fn angular_velocity(data: &[Option<u32>]) -> Option<Vec3> {
    let component = |slot| float(data, slot).unwrap_or_default();
    let value = Vec3::new(component(3), component(4), component(1));
    (value.is_finite() && value.length_squared() > f32::EPSILON).then_some(value)
}
