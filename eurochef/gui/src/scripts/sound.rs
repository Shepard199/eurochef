use super::*;
use crate::sound_preview::{
    is_playable_audio_reference, SoundVoiceGroup, SoundVoiceKey, SoundVoiceSpec,
};

const MAX_SOUND_RECURSION_DEPTH: usize = 32;
const AUDIO_TIME_EPSILON: f32 = 0.002;

#[derive(Debug, Clone, Copy, PartialEq)]
struct ScriptSoundEvent {
    key: SoundVoiceKey,
    hashcode: u32,
    start_seconds: f32,
    end_seconds: f32,
}

fn next_command_path(parent: u64, command_index: usize) -> u64 {
    parent
        .wrapping_mul(0x9E37_79B1_85EB_CA87)
        .wrapping_add(command_index as u64 + 1)
}

fn event_active(event: &ScriptSoundEvent, current_time: f32) -> bool {
    current_time + AUDIO_TIME_EPSILON >= event.start_seconds
        && current_time < event.end_seconds - AUDIO_TIME_EPSILON
}

#[allow(clippy::too_many_arguments)]
fn collect_script_sound_events(
    render_store: &RenderStore,
    file: Hashcode,
    script_hashcode: Hashcode,
    base_time: f32,
    parent_end: f32,
    command_path: u64,
    ancestry: &mut Vec<(Hashcode, Hashcode)>,
    events: &mut Vec<ScriptSoundEvent>,
) {
    if ancestry.len() >= MAX_SOUND_RECURSION_DEPTH || ancestry.contains(&(file, script_hashcode)) {
        return;
    }
    let Some(script) = render_store.get_script(file, script_hashcode) else {
        return;
    };

    ancestry.push((file, script_hashcode));
    let script_end = (base_time + script.duration_seconds()).min(parent_end);
    for (command_index, command) in script.commands.iter().enumerate() {
        if command.length == 0 {
            continue;
        }
        let start_seconds = base_time + script.time_at_frame(command.start.max(0) as f32);
        let end_seconds =
            (base_time
                + script.time_at_frame(
                    command.start.max(0).saturating_add_unsigned(command.length) as f32,
                ))
            .min(script_end);
        if end_seconds <= start_seconds {
            continue;
        }
        let child_path = next_command_path(command_path, command_index);

        match command.data {
            UXGeoScriptCommandData::Sound { hashcode } if is_playable_audio_reference(hashcode) => {
                events.push(ScriptSoundEvent {
                    key: SoundVoiceKey::Script {
                        file,
                        script: script_hashcode,
                        command_path: child_path,
                    },
                    hashcode,
                    start_seconds,
                    end_seconds,
                })
            }
            UXGeoScriptCommandData::SubScript {
                hashcode,
                file: declared_file,
            } => {
                let child_file = if declared_file == u32::MAX || hashcode.is_local() {
                    file
                } else {
                    declared_file
                };
                collect_script_sound_events(
                    render_store,
                    child_file,
                    hashcode,
                    start_seconds,
                    end_seconds,
                    child_path,
                    ancestry,
                    events,
                );
            }
            _ => {}
        }
    }
    ancestry.pop();
}

fn script_sound_events(
    render_store: &RenderStore,
    file: Hashcode,
    script_hashcode: Hashcode,
) -> Vec<ScriptSoundEvent> {
    let Some(script) = render_store.get_script(file, script_hashcode) else {
        return vec![];
    };
    let mut events = vec![];
    collect_script_sound_events(
        render_store,
        file,
        script_hashcode,
        0.0,
        script.duration_seconds(),
        0xCBF2_9CE4_8422_2325,
        &mut vec![],
        &mut events,
    );
    events.sort_by(|left, right| {
        left.start_seconds
            .total_cmp(&right.start_seconds)
            .then_with(|| left.end_seconds.total_cmp(&right.end_seconds))
    });
    events
}

pub(crate) fn active_script_sound_events(
    render_store: &RenderStore,
    file: Hashcode,
    script_hashcode: Hashcode,
    current_time: f32,
    instance_seed: u64,
) -> Vec<(SoundVoiceKey, u32, f32)> {
    let Some(script) = render_store.get_script(file, script_hashcode) else {
        return vec![];
    };
    let mut events = vec![];
    collect_script_sound_events(
        render_store,
        file,
        script_hashcode,
        0.0,
        script.duration_seconds(),
        instance_seed,
        &mut vec![],
        &mut events,
    );
    events
        .into_iter()
        .filter(|event| event_active(event, current_time))
        .map(|event| {
            (
                event.key,
                event.hashcode,
                current_time - event.start_seconds,
            )
        })
        .collect()
}

impl ScriptListPanel {
    pub(super) fn sync_script_timeline_audio(&mut self, context: &egui::Context) {
        let current_script = self.selected_script;
        let current_time = self.current_time.max(0.0);
        let audio_delta = (current_time - self.last_audio_time).abs();
        let discontinuity = self.last_audio_script != current_script
            || current_time + AUDIO_TIME_EPSILON < self.last_audio_time
            || (!self.is_playing && audio_delta > AUDIO_TIME_EPSILON)
            || audio_delta > 0.5;

        let events = {
            let render_store = self.render_store.read();
            script_sound_events(&render_store, self.file, current_script)
        };
        let active = events
            .iter()
            .copied()
            .filter(|event| event_active(event, current_time))
            .collect::<Vec<_>>();

        let mut preview = self.sound_preview.lock();
        preview.tick();
        if discontinuity {
            preview.reset_group(SoundVoiceGroup::Script);
        }
        preview.preload_hashes(events.iter().map(|event| event.hashcode));

        if preview.script_enabled {
            let playback_speed = self.playback_speed.max(0.05);
            let desired = active.into_iter().map(|event| {
                (
                    event.key,
                    SoundVoiceSpec {
                        hashcode: event.hashcode,
                        looping: false,
                        volume: 1.0,
                        speed: playback_speed,
                        pan: 0.0,
                        fade_in_seconds: 0.01,
                        fade_out_seconds: 0.03,
                        seek_seconds: (current_time - event.start_seconds).max(0.0),
                    },
                )
            });
            preview.sync_group(SoundVoiceGroup::Script, desired, 0.03);
            if self.is_playing {
                preview.resume_group(SoundVoiceGroup::Script);
            } else {
                preview.pause_group(SoundVoiceGroup::Script);
            }
        } else {
            preview.stop_group(SoundVoiceGroup::Script, 0.03);
        }

        if preview.has_pending_work()
            || preview.voice_count(SoundVoiceGroup::Script) > 0
            || (self.is_playing && !events.is_empty())
        {
            context.request_repaint();
        }
        self.last_audio_script = current_script;
        self.last_audio_time = current_time;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_range_uses_start_inclusive_and_end_exclusive() {
        let event = ScriptSoundEvent {
            key: SoundVoiceKey::Manual,
            hashcode: 1,
            start_seconds: 1.0,
            end_seconds: 2.0,
        };
        assert!(!event_active(&event, 0.9));
        assert!(event_active(&event, 1.0));
        assert!(event_active(&event, 1.5));
        assert!(!event_active(&event, 2.0));
    }

    #[test]
    fn nested_command_paths_are_stable_and_distinct() {
        let parent = next_command_path(7, 2);
        assert_eq!(parent, next_command_path(7, 2));
        assert_ne!(parent, next_command_path(7, 3));
        assert_ne!(next_command_path(parent, 0), next_command_path(parent, 1));
    }
}
