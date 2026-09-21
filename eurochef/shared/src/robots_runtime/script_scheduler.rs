use serde::Serialize;

use crate::script::{UXGeoScript, UXGeoScriptCommand, UXGeoScriptCommandData};

use super::events::RobotsScriptEventView;

pub const ROBOTS_SCRIPT_DEFAULT_OPCODE_MASK: u32 = 0x0007_FFFE;
const ROBOTS_SCRIPT_SCHEDULER_MAX_COMMANDS_PER_TICK: usize = 65_536;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsScriptSchedulerMode {
    #[default]
    Normal,
    Paused,
    /// Native `EXItemAnimator_Script +0x11C == 3`: commands are consumed in
    /// stream order without waiting for their serialized start frame; current
    /// frame is snapped to each command start before dispatch.
    Seek,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsScriptNativeEventResult {
    /// Native Handler result 0 (and ordinary non-special return values when the
    /// Handler did not replace the Script cursor).
    Continue,
    /// Native Handler result 1: keep the cursor on this Event and return from
    /// the current Script update.
    Hold,
    /// Native Handler result 2: reset/restart Script and resume dispatch in the
    /// same native update.
    Restart,
    /// Native Handler result 3: enter the `+0x11C == 3` seek/scan mode.
    EnterSeek,
    /// Native Handler result 4: leave seek mode and pin current frame to this
    /// Event's serialized start.
    LeaveSeek,
    /// Handler changed the same `EXItemAnimator_Script` timeline through native
    /// `+0x20 -> 0x004FB462 -> 0x004FA74C`. Bits preserve the exact f32 target
    /// while keeping this control result Eq/portable.
    SeekToFrameBits { frame_bits: u32 },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RobotsScriptSchedulerState {
    pub frame: f32,
    pub cursor: usize,
    pub mode: RobotsScriptSchedulerMode,
    pub finished: bool,
    pub enabled_opcode_mask: u32,
    /// Native loop counters addressed by opcode16 payload byte `command+0x0D`.
    pub loop_counters: Vec<i32>,
}

impl Default for RobotsScriptSchedulerState {
    fn default() -> Self {
        Self {
            frame: 0.0,
            cursor: 0,
            mode: RobotsScriptSchedulerMode::Normal,
            finished: false,
            enabled_opcode_mask: ROBOTS_SCRIPT_DEFAULT_OPCODE_MASK,
            loop_counters: Vec::new(),
        }
    }
}

impl RobotsScriptSchedulerState {
    pub fn reset(&mut self) {
        let opcode_mask = self.enabled_opcode_mask;
        *self = Self::default();
        self.enabled_opcode_mask = opcode_mask;
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RobotsScriptSchedulerStep {
    pub processed_commands: u32,
    pub dispatched_events: u32,
    pub controller_fanouts: u32,
    pub loop_jumps: u32,
    pub held_on_event: bool,
    pub unsupported_mode3_loop: bool,
    pub invalid_control_payload: bool,
    pub command_guard_exhausted: bool,
    pub terminated: bool,
}

fn opcode_enabled(mask: u32, opcode: u8) -> bool {
    opcode < 32 && mask & (1u32 << opcode) != 0
}

fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    let bytes: [u8; 4] = data.get(offset..offset + 4)?.try_into().ok()?;
    Some(u32::from_le_bytes(bytes))
}

fn seek_after_frame(script: &UXGeoScript, target_frame: i32) -> usize {
    script
        .commands
        .iter()
        .position(|command| command.opcode == 18 || i32::from(command.start) > target_frame)
        .unwrap_or(script.commands.len())
}

fn apply_external_timeline_seek(
    script: &UXGeoScript,
    state: &mut RobotsScriptSchedulerState,
    target_frame: f32,
) -> bool {
    // Native 0x004FA74C returns immediately when +0x104 already equals target.
    if state.frame == target_frame {
        return false;
    }
    state.frame = target_frame;
    state.cursor = seek_after_frame(script, target_frame.trunc() as i32);
    state.finished = false;
    true
}

fn restart_script(state: &mut RobotsScriptSchedulerState) {
    state.frame = 0.0;
    state.cursor = 0;
    state.finished = false;
}

fn apply_loop_command(
    script: &UXGeoScript,
    state: &mut RobotsScriptSchedulerState,
    command: &UXGeoScriptCommand,
    step: &mut RobotsScriptSchedulerStep,
) -> bool {
    let UXGeoScriptCommandData::Unknown { data, .. } = &command.data else {
        step.invalid_control_payload = true;
        return false;
    };
    if data.len() < 12 {
        step.invalid_control_payload = true;
        return false;
    }

    // 0x004FAB6E reads mode byte at command+0x0C and counter index at +0x0D.
    // Mode 3 invokes Handler vslot +0x1C before the counter/jump logic. That
    // host callback is intentionally not guessed here; shipped M10 Cutscenes
    // contain no opcode16 commands, so fail closed if such a loop is reached.
    if data[0] == 3 {
        step.unsupported_mode3_loop = true;
        return false;
    }

    let counter_index = data[1] as usize;
    let Some(repeat_count) = read_u32(data, 4).map(|value| value as i32) else {
        step.invalid_control_payload = true;
        return false;
    };
    let Some(target_frame) = read_u32(data, 8) else {
        step.invalid_control_payload = true;
        return false;
    };

    if state.loop_counters.len() <= counter_index {
        state.loop_counters.resize(counter_index + 1, 0);
    }
    let counter = &mut state.loop_counters[counter_index];
    if *counter < 1 {
        *counter = repeat_count;
    } else {
        *counter -= 1;
        if *counter == 0 {
            state.cursor += 1;
            return true;
        }
    }

    // Native 0x004FA74C seeks the visual/controller state to target_frame and
    // leaves the command cursor at the first stream command after that frame.
    // Events at/before the seek target are therefore not re-fired by a loop.
    state.frame = target_frame as f32;
    state.cursor = seek_after_frame(script, target_frame as i32);
    step.loop_jumps = step.loop_jumps.saturating_add(1);
    true
}

/// Engine-neutral event/control scheduler for native `EXItemAnimator_Script`.
///
/// `0x004FA5A8 -> 0x004FA3AD` proves the ordering used here:
/// commands are consumed in serialized stream order while `command.start <=
/// trunc(current_frame)`; the first future command ends this update after adding
/// exactly `delta_frames`. Opcode11 Handler result 1 holds the cursor, result 2
/// restarts, result 3 enters seek mode, result 4 leaves seek mode at the Event
/// frame. Opcode17 is controller fan-out only. Opcode18 terminates immediately.
///
/// Geometry/audio/controller creation is deliberately outside this reducer. The
/// UE/GUI host receives only due Event commands through `event_handler`.
pub fn advance_script_scheduler<F>(
    script: &UXGeoScript,
    state: &mut RobotsScriptSchedulerState,
    delta_frames: f32,
    mut event_handler: F,
) -> RobotsScriptSchedulerStep
where
    F: FnMut(RobotsScriptEventView<'_>) -> RobotsScriptNativeEventResult,
{
    let mut step = RobotsScriptSchedulerStep::default();
    if state.finished || state.mode == RobotsScriptSchedulerMode::Paused {
        return step;
    }

    for _ in 0..ROBOTS_SCRIPT_SCHEDULER_MAX_COMMANDS_PER_TICK {
        let Some(command) = script.commands.get(state.cursor) else {
            state.finished = true;
            step.terminated = true;
            return step;
        };

        if !opcode_enabled(state.enabled_opcode_mask, command.opcode) {
            state.cursor += 1;
            continue;
        }

        if state.mode == RobotsScriptSchedulerMode::Seek {
            state.frame = command.start as f32;
        } else if i32::from(command.start) > state.frame.trunc() as i32 {
            state.frame += delta_frames;
            return step;
        }

        step.processed_commands = step.processed_commands.saturating_add(1);
        match command.opcode {
            11 => {
                let Some(event) = RobotsScriptEventView::from_command(command) else {
                    state.cursor += 1;
                    continue;
                };
                step.dispatched_events = step.dispatched_events.saturating_add(1);
                match event_handler(event) {
                    RobotsScriptNativeEventResult::Continue => state.cursor += 1,
                    RobotsScriptNativeEventResult::Hold => {
                        step.held_on_event = true;
                        return step;
                    }
                    RobotsScriptNativeEventResult::Restart => {
                        restart_script(state);
                    }
                    RobotsScriptNativeEventResult::EnterSeek => {
                        state.mode = RobotsScriptSchedulerMode::Seek;
                        state.cursor += 1;
                    }
                    RobotsScriptNativeEventResult::LeaveSeek => {
                        state.mode = RobotsScriptSchedulerMode::Normal;
                        state.frame = command.start as f32;
                        state.cursor += 1;
                    }
                    RobotsScriptNativeEventResult::SeekToFrameBits { frame_bits } => {
                        let target_frame = f32::from_bits(frame_bits);
                        if !apply_external_timeline_seek(script, state, target_frame) {
                            // Native 0x004FA74C no-op: Handler still returned 0, so
                            // ordinary opcode11 stream advancement consumes this Event.
                            state.cursor += 1;
                        }
                    }
                }
            }
            16 => {
                if !apply_loop_command(script, state, command, &mut step) {
                    return step;
                }
            }
            17 => {
                step.controller_fanouts = step.controller_fanouts.saturating_add(1);
                state.cursor += 1;
            }
            18 => {
                state.finished = true;
                step.terminated = true;
                return step;
            }
            _ => state.cursor += 1,
        }
    }

    step.command_guard_exhausted = true;
    step
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::UXGeoScriptCommandData;

    fn command(opcode: u8, start: i16, data: UXGeoScriptCommandData) -> UXGeoScriptCommand {
        UXGeoScriptCommand {
            opcode,
            start,
            length: 1,
            controller_header_index: 0,
            controller_index: 0,
            parent_controller_index: 0xff,
            data,
        }
    }

    fn event(start: i16, event_type: u32) -> UXGeoScriptCommand {
        command(
            11,
            start,
            UXGeoScriptCommandData::Event {
                event_type,
                data: vec![0; 8],
            },
        )
    }

    fn script(commands: Vec<UXGeoScriptCommand>) -> UXGeoScript {
        UXGeoScript {
            hashcode: 0x0400_0001,
            framerate: 30.0,
            length: 300,
            num_threads: 1,
            commands,
            serialized_controller_count: 0,
            controller_record_metadata: vec![],
            controllers: vec![],
            controller_group_indices: vec![],
            controller_groups: vec![],
        }
    }

    #[test]
    fn native_gate_dispatches_frame_zero_then_advances_only_at_first_future_command() {
        let script = script(vec![
            event(0, 1),
            event(1, 2),
            command(
                18,
                0,
                UXGeoScriptCommandData::Unknown {
                    cmd: 18,
                    data: vec![],
                },
            ),
        ]);
        let mut state = RobotsScriptSchedulerState::default();
        let mut seen = vec![];
        let first = advance_script_scheduler(&script, &mut state, 1.0, |event| {
            seen.push(event.event_type);
            RobotsScriptNativeEventResult::Continue
        });
        assert_eq!(seen, vec![1]);
        assert_eq!(state.frame, 1.0);
        assert_eq!(state.cursor, 1);
        assert!(!first.terminated);

        let second = advance_script_scheduler(&script, &mut state, 1.0, |event| {
            seen.push(event.event_type);
            RobotsScriptNativeEventResult::Continue
        });
        assert_eq!(seen, vec![1, 2]);
        assert!(second.terminated);
        assert!(state.finished);
    }

    #[test]
    fn positive_native_event_result_holds_exact_cursor_and_frame() {
        let script = script(vec![event(0, 7), event(1, 8)]);
        let mut state = RobotsScriptSchedulerState::default();
        let step = advance_script_scheduler(&script, &mut state, 1.0, |_| {
            RobotsScriptNativeEventResult::Hold
        });
        assert!(step.held_on_event);
        assert_eq!(state.cursor, 0);
        assert_eq!(state.frame, 0.0);
    }

    #[test]
    fn seek_mode_consumes_later_commands_without_waiting_for_frame_clock() {
        let script = script(vec![event(0, 1), event(40, 2), event(90, 3)]);
        let mut state = RobotsScriptSchedulerState::default();
        let mut seen = vec![];
        let _ = advance_script_scheduler(&script, &mut state, 1.0, |event| {
            seen.push(event.event_type);
            match event.event_type {
                1 => RobotsScriptNativeEventResult::EnterSeek,
                3 => RobotsScriptNativeEventResult::LeaveSeek,
                _ => RobotsScriptNativeEventResult::Continue,
            }
        });
        assert_eq!(seen, vec![1, 2, 3]);
        assert_eq!(state.mode, RobotsScriptSchedulerMode::Normal);
        assert_eq!(state.frame, 90.0);
    }

    #[test]
    fn loop_seek_skips_events_at_or_before_target_like_native_004fa74c() {
        let loop_data = vec![1, 0, 0, 0, 0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0];
        let script = script(vec![
            event(0, 1),
            event(4, 2),
            command(
                16,
                5,
                UXGeoScriptCommandData::Unknown {
                    cmd: 16,
                    data: loop_data,
                },
            ),
            event(6, 3),
        ]);
        let mut state = RobotsScriptSchedulerState {
            frame: 5.0,
            cursor: 2,
            ..Default::default()
        };
        let step = advance_script_scheduler(&script, &mut state, 1.0, |_| {
            RobotsScriptNativeEventResult::Continue
        });
        assert_eq!(step.loop_jumps, 1);
        assert_eq!(state.frame, 1.0);
        assert_eq!(state.cursor, 1);
    }

    #[test]
    fn external_seek_noop_at_current_frame_consumes_only_the_current_event() {
        let script = script(vec![event(0, 1), event(0, 2), event(4, 3)]);
        let mut state = RobotsScriptSchedulerState::default();
        let mut seen = vec![];
        let _ = advance_script_scheduler(&script, &mut state, 1.0, |event| {
            seen.push(event.event_type);
            if event.event_type == 1 {
                RobotsScriptNativeEventResult::SeekToFrameBits {
                    frame_bits: 0.0f32.to_bits(),
                }
            } else {
                RobotsScriptNativeEventResult::Continue
            }
        });
        assert_eq!(seen, vec![1, 2]);
        assert_eq!(state.cursor, 2);
        assert_eq!(state.frame, 1.0);
    }

    #[test]
    fn external_seek_rebuild_skips_all_events_at_or_before_target_frame() {
        let script = script(vec![event(0, 1), event(0, 2), event(3, 3), event(5, 4)]);
        let mut state = RobotsScriptSchedulerState::default();
        let mut seen = vec![];
        let _ = advance_script_scheduler(&script, &mut state, 1.0, |event| {
            seen.push(event.event_type);
            RobotsScriptNativeEventResult::SeekToFrameBits {
                frame_bits: 3.5f32.to_bits(),
            }
        });
        assert_eq!(seen, vec![1]);
        assert_eq!(state.cursor, 3);
        assert_eq!(state.frame, 4.5);
    }

    #[test]
    fn controller_fanout_never_emits_a_script_event() {
        let script = script(vec![
            command(
                17,
                0,
                UXGeoScriptCommandData::Unknown {
                    cmd: 17,
                    data: vec![],
                },
            ),
            command(
                18,
                0,
                UXGeoScriptCommandData::Unknown {
                    cmd: 18,
                    data: vec![],
                },
            ),
        ]);
        let mut state = RobotsScriptSchedulerState::default();
        let step = advance_script_scheduler(&script, &mut state, 1.0, |_| {
            panic!("fanout is not an Event")
        });
        assert_eq!(step.controller_fanouts, 1);
        assert!(step.terminated);
    }
}
