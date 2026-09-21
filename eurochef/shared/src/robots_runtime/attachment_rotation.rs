/// Native attached-XItem quaternion update used by `0x00454CA0` and the
/// WatchBot component attachment host. The original code post-multiplies the
/// current quaternion by a local-Y delta and does not renormalize afterwards.
pub fn postmultiply_local_y_rotation(
    current_quaternion_xyzw: [f32; 4],
    angle_radians: f32,
) -> [f32; 4] {
    let half_angle = angle_radians * 0.5;
    let sin_half = (half_angle as f64).sin() as f32;
    let cos_half = (half_angle as f64).cos() as f32;
    let [x, y, z, w] = current_quaternion_xyzw;
    [
        x * cos_half - z * sin_half,
        y * cos_half + w * sin_half,
        x * sin_half + z * cos_half,
        w * cos_half - y * sin_half,
    ]
}

/// Same native attached-XItem postmultiply path for the local-Z lane used by
/// EW05 ThiefBot `0x0045F7C0 -> 0x00454CA0(child, 0, 0, angle)`.
pub fn postmultiply_local_z_rotation(
    current_quaternion_xyzw: [f32; 4],
    angle_radians: f32,
) -> [f32; 4] {
    let half_angle = angle_radians * 0.5;
    let sin_half = (half_angle as f64).sin() as f32;
    let cos_half = (half_angle as f64).cos() as f32;
    let [x, y, z, w] = current_quaternion_xyzw;
    [
        x * cos_half + y * sin_half,
        y * cos_half - x * sin_half,
        z * cos_half + w * sin_half,
        w * cos_half - z * sin_half,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_y_postmultiply_matches_native_identity_case() {
        let angle = std::f32::consts::FRAC_PI_2;
        let result = postmultiply_local_y_rotation([0.0, 0.0, 0.0, 1.0], angle);
        let half = std::f32::consts::FRAC_PI_4;
        assert!(result[0].abs() < 1.0e-6);
        assert!((result[1] - half.sin()).abs() < 1.0e-6);
        assert!(result[2].abs() < 1.0e-6);
        assert!((result[3] - half.cos()).abs() < 1.0e-6);
    }
}
