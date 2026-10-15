//! Robots.exe 0x00440600/0x00440640 marker dispatch model.
//!
//! The *selection* logic is recovered from the original Windows vtables;
//! native UI controller/renderer effects must not be inferred from the
//! selection alone. This state is deliberately isolated from gameplay.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct HudMarkerDescriptor {
    pub(super) uid: u32,
    /// Source start frame of the selected 0xA4 runtime record.
    /// JumpToMarker loads this record's start after seeking by runtime ordinal.
    pub(super) event_frame: f32,
    /// The ordinal in the original runtime list of opcode-11 Event 0x16000008
    /// records. 0x004406CD passes this descriptor's first dword directly to
    /// 0x0043D2C0; it is NOT an absolute timeline frame.
    pub(super) runtime_record_index: usize,
    /// Raw final float in the serialized event payload. 0x0043FD40 stores
    /// its truncated form in marker descriptor +8, but 0x00440640 does NOT
    /// use it for immediate JumpToMarker positioning.
    pub(super) payload_scalar: f32,
    pub(super) fps: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct HudMarkerSeek {
    pub(super) uid: u32,
    pub(super) event_frame: f32,
    pub(super) event_seconds: f64,
    /// Actual native JumpToMarker argument (ordinal of runtime 0xA4 record).
    pub(super) runtime_record_index: usize,
    /// Metadata f32 from event payload +0x20, *not* the playback frame.
    pub(super) payload_scalar: f32,
}

#[derive(Debug, Default)]
pub(super) struct NativeHudMarkerSelector {
    descriptors: Vec<HudMarkerDescriptor>,
    current: Option<usize>,
    pending: Option<usize>,
    /// Native XStatus marker selection +0x10C=2; we do not emulate subsequent
    /// +0x9C/+0xA0 gameplay side effects until their owner is verified.
    selected_state: Option<u32>,
    last_seek: Option<HudMarkerSeek>,
}

/// Robots.exe 0x0043FD71 loads the runtime command's float at +0x30;
/// 0x0043FD77 converts it via MSVC __ftol and stores the signed i32
/// at marker descriptor +8. This is metadata; it does NOT set the
/// JumpToMarker script time. Selection passes descriptor word +0.
/// The descriptor's converted value is separate from the *record ordinal*
/// used for playback. It must not be substituted for Event.start or
/// passed as the 0x0043D2C0 argument. Validate raw payload safely.
pub(super) fn native_marker_frame_lane(runtime_frame: f32) -> Option<i32> {
    if !runtime_frame.is_finite() || runtime_frame < 0.0
        || f64::from(runtime_frame) >= f64::from(i32::MAX) + 1.0 {
        return None;
    }
    Some(runtime_frame.trunc() as i32)
}

impl NativeHudMarkerSelector {
    pub(super) fn reset(&mut self, descriptors: Vec<HudMarkerDescriptor>) {
        self.descriptors = descriptors.into_iter()
            .filter(|entry| entry.uid & 0xFF00_0000 == 0x4600_0000
                && entry.event_frame.is_finite() && entry.event_frame >= 0.0
                && native_marker_frame_lane(entry.payload_scalar).is_some()
                && entry.fps.is_finite() && entry.fps > 0.0)
            .take(128)
            .collect();
        self.current = None;
        self.pending = None;
        self.selected_state = None;
        self.last_seek = None;
    }

    /// Mirrors the +0xB4 index validation and deferred-vs-immediate selection.
    /// Unknown markers never trigger host menu actions.
    pub(super) fn select_uid(&mut self, uid: u32, deferred: bool) -> Option<HudMarkerSeek> {
        let index = self.descriptors.iter().position(|entry| entry.uid == uid)?;
        if deferred {
            self.pending = Some(index);
            return None;
        }
        self.pending = None;
        self.current = Some(index);
        self.selected_state = Some(2);
        let descriptor = self.descriptors[index];
        let seek = HudMarkerSeek {
            uid: descriptor.uid,
            event_frame: descriptor.event_frame,
            event_seconds: f64::from(descriptor.event_frame) / f64::from(descriptor.fps),
            runtime_record_index: descriptor.runtime_record_index,
            payload_scalar: descriptor.payload_scalar,
        };
        self.last_seek = Some(seek);
        Some(seek)
    }

    /// Corresponds to consumption of queued +0x144/+0x148 at 0x00440020.
    pub(super) fn tick(&mut self) -> Option<HudMarkerSeek> {
        let index = self.pending.take()?;
        let uid = self.descriptors[index].uid;
        self.select_uid(uid, false)
    }

    pub(super) fn selected_uid(&self) -> Option<u32> {
        self.current.map(|index| self.descriptors[index].uid)
    }
    pub(super) fn last_seek(&self) -> Option<HudMarkerSeek> { self.last_seek }
    pub(super) fn pending_uid(&self) -> Option<u32> {
        self.pending.map(|index| self.descriptors[index].uid)
    }
    pub(super) fn selected_state(&self) -> Option<u32> { self.selected_state }
}

/// Proven table at Robots.exe 0x005EDCA0 (HUD option UIDs) paired with
/// 0x005EDE7C (saved option UID). Called by the native 0x0049E910 +0x9C
/// handler, NOT by the generic no-op 0x00440750.
/// Entries after index 13 are intentionally excluded: the handler's
/// +/- branch only accepts indexes < 14.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct NativeOptionBinding {
    pub(super) option_uid: u32,
    pub(super) saved_setting_uid: u32,
    /// The handler handles 0x2A/0x2B left/right for volume controls only.
    pub(super) volume_adjustable: bool,
}

pub(super) fn native_option_binding(uid: u32) -> Option<NativeOptionBinding> {
    const OPTIONS: &[(u32, u32)] = &[
        (0x4600_0022, 0xAABB_CCDD), // original sentinel, not a saved setting
        (0x4600_0023, 0x5800_0011),
        (0x4600_0024, 0x5800_0015),
        (0x4600_0025, 0x5800_0033),
        (0x4600_0026, 0x5800_0034),
        (0x4600_0027, 0x5800_0035),
        (0x4600_0028, 0x5800_0012),
        (0x4600_0029, 0x5800_000e), // music volume
        (0x4600_002a, 0x5800_000d), // SFX volume
        (0x4600_0039, 0x5800_000c), // sound mode
        (0x4600_003a, 0x5800_000f),
        (0x4600_003b, 0x5800_0010),
        (0x4600_0050, 0x5800_002a),
        (0x4600_0061, 0x5800_002b),
    ];
    OPTIONS.iter().copied().find(|(option, _)| *option == uid)
        .and_then(|(option_uid, saved_setting_uid)| {
            (saved_setting_uid != 0xAABB_CCDD).then_some(NativeOptionBinding {
                option_uid, saved_setting_uid,
                volume_adjustable: matches!(saved_setting_uid, 0x5800_000d | 0x5800_000e),
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn marker(uid: u32, frame: f32) -> HudMarkerDescriptor {
        HudMarkerDescriptor { uid, event_frame: frame, runtime_record_index: 1, payload_scalar: 0.0, fps: 30.0 }
    }

    #[test]
    fn native_runtime_float_to_integer_marker_frame_matches_ftol_path() {
        assert_eq!(native_marker_frame_lane(56.0), Some(56));
        assert_eq!(native_marker_frame_lane(25.999), Some(25));
        assert_eq!(native_marker_frame_lane(24.125), Some(24));
        assert_eq!(native_marker_frame_lane(75.0), Some(75));
        assert_eq!(native_marker_frame_lane(-0.01), None);
        assert_eq!(native_marker_frame_lane(f32::NAN), None);
        assert_eq!(native_marker_frame_lane(f32::INFINITY), None);
        assert_eq!(native_marker_frame_lane(2_147_483_648.0), None);
    }

    #[test]
    fn original_slot9c_settings_bindings_match_native_tables() {
        assert_eq!(native_option_binding(0x4600_0022), None); // sentinel
        assert_eq!(native_option_binding(0x4600_0029),
            Some(NativeOptionBinding {
                option_uid: 0x4600_0029,
                saved_setting_uid: 0x5800_000e,
                volume_adjustable: true,
            }));
        assert_eq!(native_option_binding(0x4600_002a).unwrap().saved_setting_uid, 0x5800_000d);
        assert_eq!(native_option_binding(0x4600_0023).unwrap().saved_setting_uid, 0x5800_0011);
        assert_eq!(native_option_binding(0x4600_0039).unwrap().volume_adjustable, false);
        assert_eq!(native_option_binding(0x4600_005e), None); // input config path
        assert_eq!(native_option_binding(0x4600_0001), None); // main menu marker
    }

    #[test]
    fn native_marker_select_bounds_and_deferred_tick() {
        let mut player = NativeHudMarkerSelector::default();
        player.reset(vec![marker(0x4600_0001, 56.0), marker(0x4600_000c, 25.0)]);
        assert_eq!(player.select_uid(0x4600_00ff, false), None);
        assert_eq!(player.selected_uid(), None);
        assert_eq!(player.select_uid(0x4600_0001, false),
            Some(HudMarkerSeek { uid: 0x4600_0001, event_frame: 56.0, event_seconds: 56.0 / 30.0,
                runtime_record_index: 1, payload_scalar: 0.0 }));
        assert_eq!(player.selected_state(), Some(2));
        assert_eq!(player.select_uid(0x4600_000c, true), None);
        assert_eq!(player.selected_uid(), Some(0x4600_0001));
        assert_eq!(player.pending_uid(), Some(0x4600_000c));
        assert_eq!(player.tick().unwrap().uid, 0x4600_000c);
        assert_eq!(player.selected_uid(), Some(0x4600_000c));
        assert_eq!(player.tick(), None);
    }

    #[test]
    fn native_marker_selects_record_index_and_uses_event_start_not_payload() {
        let mut player = NativeHudMarkerSelector::default();
        player.reset(vec![HudMarkerDescriptor {
            uid: 0x4600_0001, event_frame: 56.0,
            runtime_record_index: 1, payload_scalar: 12.75, fps: 30.0,
        }]);
        let seek = player.select_uid(0x4600_0001, false).unwrap();
        assert_eq!(seek.event_frame, 56.0);
        assert_eq!(seek.runtime_record_index, 1);
        assert!((seek.event_seconds - 56.0 / 30.0).abs() < 1e-9);
        assert_eq!(seek.payload_scalar, 12.75);
    }

    #[test]
    fn native_marker_reset_rejects_corrupt_descriptor() {
        let mut player = NativeHudMarkerSelector::default();
        player.reset(vec![
            marker(0x4600_0001, f32::NAN), marker(0x4500_0002, 0.0),
            HudMarkerDescriptor { uid: 0x4600_0003, event_frame: 5.0, runtime_record_index: 1, payload_scalar: 0.0, fps: 0.0 },
            marker(0x4600_0004, 4.0)
        ]);
        assert_eq!(player.select_uid(0x4600_0001, false), None);
        assert_eq!(player.select_uid(0x4600_0004, false).unwrap().event_seconds, 4.0 / 30.0);
        player.reset(vec![]);
        assert_eq!(player.selected_uid(), None);
        assert_eq!(player.pending_uid(), None);
    }
}
