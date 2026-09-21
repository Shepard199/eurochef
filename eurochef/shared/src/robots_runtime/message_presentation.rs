use std::collections::VecDeque;

use serde::Serialize;

use super::npc_text::RobotsTextMessageDefinition;

pub const ROBOTS_MESSAGE_VOICE_PADDING_SECONDS: f32 = 0.5;
pub const ROBOTS_MESSAGE_VOICE_VOLUME: f32 = 0.5;
pub const ROBOTS_MESSAGE_MINIMUM_SECONDS: f32 = 2.4;
pub const ROBOTS_MESSAGE_FIXED_FRAMES_PER_SECOND: f32 = 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum RobotsMessagePresentationEffect {
    StartVoice { sound_uid: u32, volume: f32 },
    StopVoice,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum RobotsMessageFinishTiming {
    /// Native record +0x04 is zero. The message has no automatic finish frame
    /// from `0x0049BEA0` and is completed by presentation/user state instead.
    Untimed,
    /// The host has a valid `0x1A......` SoundDetails UID but has not yet loaded
    /// its duration. Native resolves this synchronously before queue insertion;
    /// engine adapters may resolve it at their resource boundary.
    NeedsSoundDuration { sound_uid: u32, enqueue_frame: f32 },
    /// Native absolute finish-frame value stored in message record +0x04.
    AbsoluteFrame(f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsMessageQueueRecord {
    /// +0x00
    pub message_uid: u32,
    /// +0x04
    pub finish_timing: RobotsMessageFinishTiming,
    /// +0x08, NPC simple-text callsite passes 0.
    pub parameter_08: u32,
    /// +0x0C, NPC simple-text callsite passes -1.
    pub parameter_0c: i32,
    /// +0x10, NPC simple-text callsite passes -1.
    pub parameter_10: i32,
    /// +0x14, NPC simple-text callsite passes -1.
    pub parameter_14: i32,
    /// +0x18, first native EXGeoTextItem userdata dword.
    pub ui_flags: u32,
    /// +0x1C, second native EXGeoTextItem userdata dword.
    pub sound_uid: u32,
}

#[inline]
pub fn robots_message_finish_frame_from_sound_duration(
    current_frame: f32,
    sound_duration_seconds: f32,
) -> f32 {
    // 0x0049BF7C..0x0049BFC1:
    // SoundDetails.duration + 0.5, clamp to minimum 2.4 seconds, fabs,
    // multiply by 60, CRT __ftol, then add current native frame.
    let seconds = (sound_duration_seconds + ROBOTS_MESSAGE_VOICE_PADDING_SECONDS)
        .max(ROBOTS_MESSAGE_MINIMUM_SECONDS)
        .abs();
    let frames = (seconds * ROBOTS_MESSAGE_FIXED_FRAMES_PER_SECOND).round_ties_even();
    current_frame + frames
}

/// Build the exact NPC Simple Text queue payload selected at
/// `0x0046BBA0 -> 0x0049BEA0`.
///
/// The NPC callsite passes `(duration=0, p08=0, p0c=-1, p10=-1, p14=-1,
/// flag=0)`. A `0x1A......` userdata sound UID therefore needs its native
/// SoundDetails duration. `-1` is the no-sound sentinel and remains untimed.
/// Other invalid/non-SoundDetails UIDs follow native lookup fallback duration
/// 0.0, which produces the 2.4-second / 144-frame minimum.
pub fn build_npc_simple_text_record(
    message: &RobotsTextMessageDefinition,
    current_frame: f32,
    sound_duration_seconds: Option<f32>,
) -> RobotsMessageQueueRecord {
    let finish_timing =
        if message.sound_uid == u32::MAX {
            RobotsMessageFinishTiming::Untimed
        } else if let Some(duration) = sound_duration_seconds {
            RobotsMessageFinishTiming::AbsoluteFrame(
                robots_message_finish_frame_from_sound_duration(current_frame, duration),
            )
        } else if message.sound_uid & 0x7F00_0000 == 0x1A00_0000 {
            RobotsMessageFinishTiming::NeedsSoundDuration {
                sound_uid: message.sound_uid,
                enqueue_frame: current_frame,
            }
        } else {
            RobotsMessageFinishTiming::AbsoluteFrame(
                robots_message_finish_frame_from_sound_duration(current_frame, 0.0),
            )
        };

    RobotsMessageQueueRecord {
        message_uid: message.message_uid,
        finish_timing,
        parameter_08: 0,
        parameter_0c: -1,
        parameter_10: -1,
        parameter_14: -1,
        ui_flags: message.ui_flags,
        sound_uid: message.sound_uid,
    }
}

/// Build the exact shipped Cutscene `ShowMessage` queue record produced by
/// `0x00407E40 -> 0x0049BEA0`. When raw native arg1 at command+0x18 has bit0
/// clear, native overrides the duration with `(length + 1) / script_rate` and
/// converts that interval to the global 60 Hz message clock. The alternate arg1
/// bit0-set path keeps the incoming x87 duration and remains fail-closed until a
/// shipped caller requires it.
pub fn build_cutscene_show_message_record(
    message: &RobotsTextMessageDefinition,
    current_frame: f32,
    script_frames_per_second: f32,
    command_length: u16,
    use_script_length: bool,
    parameter_08: i32,
    parameter_10: i32,
    parameter_14: i32,
) -> Option<RobotsMessageQueueRecord> {
    if !use_script_length
        || !script_frames_per_second.is_finite()
        || script_frames_per_second <= 0.0
    {
        return None;
    }
    let duration_native_frames = (command_length as f32 + 1.0)
        * (ROBOTS_MESSAGE_FIXED_FRAMES_PER_SECOND / script_frames_per_second);
    Some(RobotsMessageQueueRecord {
        message_uid: message.message_uid,
        finish_timing: RobotsMessageFinishTiming::AbsoluteFrame(
            current_frame + duration_native_frames,
        ),
        parameter_08: parameter_08 as u32,
        parameter_0c: -1,
        parameter_10,
        parameter_14,
        ui_flags: message.ui_flags,
        sound_uid: message.sound_uid,
    })
}

/// Resolve the host-side resource dependency that native `0x0049BEA0` satisfies
/// synchronously through `SoundDetails`. Returns false when the record does not
/// currently need a sound duration or the UID does not match.
pub fn resolve_message_sound_duration(
    record: &mut RobotsMessageQueueRecord,
    sound_uid: u32,
    sound_duration_seconds: f32,
) -> bool {
    let RobotsMessageFinishTiming::NeedsSoundDuration {
        sound_uid: pending_uid,
        enqueue_frame,
    } = record.finish_timing
    else {
        return false;
    };
    if pending_uid != sound_uid {
        return false;
    }
    record.finish_timing = RobotsMessageFinishTiming::AbsoluteFrame(
        robots_message_finish_frame_from_sound_duration(enqueue_frame, sound_duration_seconds),
    );
    true
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct NativeMessagePresentationState {
    /// Global native 32-byte queue (`DAT_007B2C44`, count `DAT_007B2C40`).
    pub queue: VecDeque<RobotsMessageQueueRecord>,
    /// Message record copied by `0x004A67E0` into presentation +0x114..+0x130.
    pub active: Option<RobotsMessageQueueRecord>,
    /// HUD manager +0x1C / native message-box object existence.
    pub message_box_exists: bool,
    /// `0x0049B170(8)` marks the message-box XItem for deferred destruction.
    pub pending_destroy: bool,
    /// Engine-neutral side effects emitted at the same native boundaries as the
    /// message-box voice controller (`0x007B2564`). Hosts drain these without
    /// owning message timing or queue semantics.
    pub pending_effects: VecDeque<RobotsMessagePresentationEffect>,
    pub enqueue_requests: u32,
    pub wake_requests: u32,
    pub dequeue_requests: u32,
    pub clear_requests: u32,
}

impl NativeMessagePresentationState {
    /// `0x0049BEA0`: ensure HUD bit8/message-box exists, append one record, then
    /// invoke presentation vslot +0x90 (`0x004A7120`) to wake idle presentation.
    pub fn enqueue(&mut self, record: RobotsMessageQueueRecord) {
        self.message_box_exists = true;
        self.pending_destroy = false;
        self.queue.push_back(record);
        self.enqueue_requests = self.enqueue_requests.wrapping_add(1);
        self.wake_requests = self.wake_requests.wrapping_add(1);
    }

    /// `0x004A6540(state0) -> 0x004A67E0`: copy the queue front into the
    /// presentation active slot and shift the global queue by one record.
    pub fn dequeue_front_into_active(&mut self) -> Option<RobotsMessageQueueRecord> {
        if self.active.is_some() {
            return self.active;
        }
        let record = self.queue.pop_front()?;
        self.active = Some(record);
        if record.sound_uid != u32::MAX {
            self.pending_effects
                .push_back(RobotsMessagePresentationEffect::StartVoice {
                    sound_uid: record.sound_uid,
                    volume: ROBOTS_MESSAGE_VOICE_VOLUME,
                });
        }
        self.dequeue_requests = self.dequeue_requests.wrapping_add(1);
        Some(record)
    }

    pub fn active_is_timed_out(&self, current_frame: f32) -> bool {
        matches!(
            self.active.map(|record| record.finish_timing),
            Some(RobotsMessageFinishTiming::AbsoluteFrame(finish)) if current_frame >= finish
        )
    }

    /// Presentation completion boundary. Visual state0..4 fades remain a host/UI
    /// concern; the engine-neutral runtime only releases the active native record.
    pub fn complete_active(&mut self) -> Option<RobotsMessageQueueRecord> {
        self.active.take()
    }

    /// Native focus-loss cleanup `0x0049C240`: stop/inactivate the current
    /// message-box presentation and clear the entire shared queue. The object is
    /// not synchronously destroyed here.
    pub fn clear_all(&mut self) {
        self.queue.clear();
        if self
            .active
            .is_some_and(|record| record.sound_uid != u32::MAX)
        {
            self.pending_effects
                .push_back(RobotsMessagePresentationEffect::StopVoice);
        }
        self.active = None;
        // 0x0049C240 recomputes the HUD destruction mask and sets bit8 for an
        // existing message-box object that is not already marked for destroy.
        if self.message_box_exists {
            self.pending_destroy = true;
        }
        self.clear_requests = self.clear_requests.wrapping_add(1);
    }

    /// Native state 999 with an empty queue requests `0x0049B170(8)`.
    pub fn request_destroy_if_idle(&mut self) -> bool {
        if self.message_box_exists && self.active.is_none() && self.queue.is_empty() {
            self.pending_destroy = true;
            true
        } else {
            false
        }
    }

    /// Deferred XItem destruction eventually clears HUD manager +0x1C.
    pub fn complete_deferred_destroy(&mut self) {
        if self.pending_destroy {
            self.message_box_exists = false;
            self.pending_destroy = false;
        }
    }

    pub fn take_effect(&mut self) -> Option<RobotsMessagePresentationEffect> {
        self.pending_effects.pop_front()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(sound_uid: u32) -> RobotsTextMessageDefinition {
        RobotsTextMessageDefinition {
            message_uid: 0x4400_0001,
            text: "test".into(),
            ui_flags: 0xB,
            sound_uid,
            spreadsheet_sound_uid: u32::MAX,
        }
    }

    #[test]
    fn npc_simple_text_voiced_record_requires_sound_duration_until_host_resolves_it() {
        let definition = message(0x1A00_0012);
        let mut unresolved = build_npc_simple_text_record(&definition, 100.0, None);
        assert_eq!(
            unresolved.finish_timing,
            RobotsMessageFinishTiming::NeedsSoundDuration {
                sound_uid: 0x1A00_0012,
                enqueue_frame: 100.0,
            }
        );
        assert!(resolve_message_sound_duration(
            &mut unresolved,
            0x1A00_0012,
            1.0
        ));
        assert_eq!(
            unresolved.finish_timing,
            RobotsMessageFinishTiming::AbsoluteFrame(244.0)
        );

        let resolved = build_npc_simple_text_record(&definition, 100.0, Some(1.0));
        assert_eq!(
            resolved.finish_timing,
            RobotsMessageFinishTiming::AbsoluteFrame(244.0)
        );
        assert_eq!(resolved.parameter_08, 0);
        assert_eq!(resolved.parameter_0c, -1);
        assert_eq!(resolved.parameter_10, -1);
        assert_eq!(resolved.parameter_14, -1);
        assert_eq!(resolved.ui_flags, 0xB);
    }

    #[test]
    fn native_voice_timing_clamps_to_144_frames_minimum() {
        assert_eq!(
            robots_message_finish_frame_from_sound_duration(10.0, 0.0),
            154.0
        );
        assert_eq!(
            robots_message_finish_frame_from_sound_duration(10.0, 1.0),
            154.0
        );
        assert_eq!(
            robots_message_finish_frame_from_sound_duration(10.0, 3.0),
            220.0
        );
    }

    #[test]
    fn dequeue_and_cleanup_emit_exact_message_voice_requests() {
        let mut state = NativeMessagePresentationState::default();
        let voiced = build_npc_simple_text_record(&message(0x1A00_0012), 0.0, Some(1.0));
        state.enqueue(voiced);
        assert_eq!(state.take_effect(), None);
        assert_eq!(state.dequeue_front_into_active(), Some(voiced));
        assert_eq!(
            state.take_effect(),
            Some(RobotsMessagePresentationEffect::StartVoice {
                sound_uid: 0x1A00_0012,
                volume: 0.5,
            })
        );
        state.clear_all();
        assert_eq!(
            state.take_effect(),
            Some(RobotsMessagePresentationEffect::StopVoice)
        );
        assert_eq!(state.take_effect(), None);
    }

    #[test]
    fn no_sound_sentinel_is_untimed_but_invalid_sound_uses_native_fallback_duration_zero() {
        assert_eq!(
            build_npc_simple_text_record(&message(u32::MAX), 50.0, None).finish_timing,
            RobotsMessageFinishTiming::Untimed
        );
        assert_eq!(
            build_npc_simple_text_record(&message(0), 50.0, None).finish_timing,
            RobotsMessageFinishTiming::AbsoluteFrame(194.0)
        );
    }

    #[test]
    fn cutscene_show_message_uses_script_length_on_raw_arg1_bit0_clear() {
        let definition = message(u32::MAX);
        let first =
            build_cutscene_show_message_record(&definition, 1000.0, 30.0, 152, true, 0, -1, -1)
                .unwrap();
        assert_eq!(
            first.finish_timing,
            RobotsMessageFinishTiming::AbsoluteFrame(1306.0)
        );
        assert_eq!(first.parameter_08, 0);
        assert_eq!(first.parameter_0c, -1);
        assert_eq!(first.parameter_10, -1);
        assert_eq!(first.parameter_14, -1);

        let second =
            build_cutscene_show_message_record(&definition, 2000.0, 30.0, 84, true, 0, -1, -1)
                .unwrap();
        assert_eq!(
            second.finish_timing,
            RobotsMessageFinishTiming::AbsoluteFrame(2170.0)
        );
        assert!(
            build_cutscene_show_message_record(&definition, 0.0, 30.0, 84, false, 0, -1, -1,)
                .is_none()
        );
    }

    #[test]
    fn queue_and_message_box_lifecycle_are_separate() {
        let mut state = NativeMessagePresentationState::default();
        let first = build_npc_simple_text_record(&message(u32::MAX), 0.0, None);
        let mut second_definition = message(u32::MAX);
        second_definition.message_uid = 0x4400_0002;
        let second = build_npc_simple_text_record(&second_definition, 0.0, None);

        state.enqueue(first);
        state.enqueue(second);
        assert!(state.message_box_exists);
        assert_eq!(state.queue.len(), 2);
        assert_eq!(state.wake_requests, 2);

        assert_eq!(state.dequeue_front_into_active(), Some(first));
        assert_eq!(state.queue.len(), 1);
        assert_eq!(state.dequeue_front_into_active(), Some(first));
        assert_eq!(state.complete_active(), Some(first));
        assert_eq!(state.dequeue_front_into_active(), Some(second));

        state.clear_all();
        assert!(state.message_box_exists);
        assert!(state.queue.is_empty());
        assert!(state.active.is_none());
        assert!(state.request_destroy_if_idle());
        assert!(state.pending_destroy);
        state.complete_deferred_destroy();
        assert!(!state.message_box_exists);
    }
}
