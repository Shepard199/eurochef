//! Read-only camera track captured from the *original* Robots.exe frontend.
//!
//! One line is: seconds pos_x pos_y pos_z target_x target_y target_z fov_deg roll_deg
//! Positions and targets correspond to original runtime Camera globals at
//! 0x007B2BE0..E8 and 0x007B2BF0..F8 (as read by Robots.exe 0x00472B70).
//! FOV and roll MUST be measured/supplied, not guessed from M00 geometry.
use anyhow::{bail, Context, Result};
use glam::Vec3;
use crate::render::camera::NativeViewCamera;

#[derive(Clone, Copy, Debug)]
pub(super) struct NativeCameraKeyframe {
    seconds: f32,
    eye: Vec3,
    target: Vec3,
    fov_degrees: f32,
    roll_degrees: f32,
}

#[derive(Debug)]
pub(super) struct NativeCameraCapture {
    frames: Vec<NativeCameraKeyframe>,
}

impl NativeCameraCapture {
    /// Import one direct x32dbg memory dump of original Robots.exe address
    /// 0x007B2BE0, exactly 0x24 bytes. The native update 0x00472B70 reads:
    /// eye xyz at +0/+4/+8, target xyz at +0x10/+0x14/+0x18,
    /// roll (degrees) at +0x20. Default vertical FOV is the shipped binary's
    /// 0x00619AFC = 45.0, used by 0x00472B70 at mode-0 initialization.
    pub(super) fn from_runtime_dump(data: &[u8]) -> Result<Self> {
        if data.len() != 0x24 { bail!("original camera dump must be exactly 0x24 bytes"); }
        let value = |offset: usize| -> f32 {
            f32::from_le_bytes(data[offset..offset + 4].try_into().expect("bounded dump offset"))
        };
        let entry = NativeCameraKeyframe {
            seconds: 0.0,
            eye: Vec3::new(value(0), value(4), value(8)),
            target: Vec3::new(value(16), value(20), value(24)),
            fov_degrees: 45.0,
            roll_degrees: value(32),
        };
        let text = format!("0 {} {} {} {} {} {} {} {}",
            entry.eye.x, entry.eye.y, entry.eye.z,
            entry.target.x, entry.target.y, entry.target.z,
            entry.fov_degrees, entry.roll_degrees);
        Self::parse(&text)
    }

    pub(super) fn parse(text: &str) -> Result<Self> {
        let mut frames = Vec::<NativeCameraKeyframe>::new();
        for (line_index, line) in text.lines().enumerate() {
            let content = line.split('#').next().unwrap_or("").trim();
            if content.is_empty() { continue; }
            let parts = content.split_whitespace().collect::<Vec<_>>();
            if parts.len() != 9 {
                bail!("camera capture line {}: expected 9 floats, got {}", line_index + 1, parts.len());
            }
            let values = parts.iter().map(|part| part.parse::<f32>())
                .collect::<std::result::Result<Vec<_>, _>>()
                .with_context(|| format!("camera capture line {}: invalid float", line_index + 1))?;
            if !values.iter().all(|v| v.is_finite()) {
                bail!("camera capture line {}: nonfinite coordinate", line_index + 1);
            }
            let sample = NativeCameraKeyframe {
                seconds: values[0],
                eye: Vec3::new(values[1], values[2], values[3]),
                target: Vec3::new(values[4], values[5], values[6]),
                fov_degrees: values[7],
                roll_degrees: values[8],
            };
            if sample.seconds < 0.0 ||
                frames.last().is_some_and(|prev| sample.seconds <= prev.seconds) {
                bail!("camera capture line {}: sample times must be nonnegative and increasing", line_index + 1);
            }
            if !(10.0..=140.0).contains(&sample.fov_degrees) ||
                (sample.target - sample.eye).length_squared() < 1.0e-6 ||
                sample.roll_degrees.abs() > 360.0 {
                bail!("camera capture line {}: invalid native camera pose", line_index + 1);
            }
            if frames.len() >= 100_000 { bail!("camera capture exceeds 100000 samples"); }
            frames.push(sample);
        }
        if frames.is_empty() { bail!("camera capture has no samples"); }
        Ok(Self { frames })
    }

    /// Sample measured poses. Intermediate positions use a provisional linear
    /// interpolation; exact original interpolation remains a separate frontier.
    pub(super) fn sample(&self, seconds: f32) -> NativeViewCamera {
        let time = if seconds.is_finite() { seconds.max(0.0) } else { 0.0 };
        let idx = self.frames.partition_point(|frame| frame.seconds <= time);
        let current = if idx == 0 { self.frames[0] } else { self.frames[idx - 1] };
        let next = self.frames.get(idx).copied().unwrap_or(current);
        let factor = if next.seconds > current.seconds {
            ((time - current.seconds) / (next.seconds - current.seconds)).clamp(0.0, 1.0)
        } else { 0.0 };
        NativeViewCamera::new(
            current.eye.lerp(next.eye, factor),
            current.target.lerp(next.target, factor),
            current.fov_degrees + (next.fov_degrees - current.fov_degrees) * factor,
        ).with_roll(current.roll_degrees + (next.roll_degrees - current.roll_degrees) * factor)
    }

    pub(super) fn frame_count(&self) -> usize { self.frames.len() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn captured_native_camera_samples_and_interpolates_without_m00_guesses() {
        let track = NativeCameraCapture::parse(
            "# sec eyeX eyeY eyeZ targetX targetY targetZ verticalFov roll\n\
             0 1 2 3 1 2 4 60 0\n\
             2 5 6 7 5 6 8 80 10\n").unwrap();
        assert_eq!(track.frame_count(), 2);
        let middle = track.sample(1.0);
        assert_eq!(middle.position, Vec3::new(3.0, 4.0, 5.0));
        assert_eq!(middle.target, Vec3::new(3.0, 4.0, 6.0));
        assert_eq!(middle.vertical_fov_degrees, 70.0);
        assert_eq!(middle.roll_degrees, 5.0);
        assert_eq!(track.sample(-12.0).position, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(track.sample(20.0).position, Vec3::new(5.0, 6.0, 7.0));
    }

    #[test]
    fn native_x32dbg_camera_dump_matches_absolute_globals() {
        // Bytes at 007B2BE0..007B2C03. Between eye and target, the
        // source layout contains one unrelated f32 slot.
        let floats = [1.0f32, 2.0, 3.0, -100.0, 4.0, 5.0, 6.0, 42.0, 12.5];
        let mut dump = Vec::new();
        for value in floats { dump.extend_from_slice(&value.to_le_bytes()); }
        let track = NativeCameraCapture::from_runtime_dump(&dump).unwrap();
        assert_eq!(track.frame_count(), 1);
        let camera = track.sample(0.0);
        assert_eq!(camera.position, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(camera.target, Vec3::new(4.0, 5.0, 6.0));
        assert_eq!(camera.vertical_fov_degrees, 45.0);
        assert_eq!(camera.roll_degrees, 12.5);
        assert!(NativeCameraCapture::from_runtime_dump(&dump[..32]).is_err());
        dump[0..4].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(NativeCameraCapture::from_runtime_dump(&dump).is_err());
    }

    #[test]
    fn native_capture_rejects_bad_time_and_poses() {
        for text in [
            "", "0 0 0 0 0 0 0 60 0",
            "0 0 0 0 0 0 1 9 0",
            "0 0 0 0 0 0 1 60 nan",
            "0 0 0 0 0 0 1 60 0\n0 1 2 3 1 2 4 60 0",
            "0 0 0 0 0 0 1 60 0\n-1 1 2 3 1 2 4 60 0",
            "0 0 0 0 0 0 1 60",
        ] {
            assert!(NativeCameraCapture::parse(text).is_err(), "bad camera data: {text}");
        }
    }
}
