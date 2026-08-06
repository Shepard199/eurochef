use super::script_sound::{map_script_audio_instances, map_script_voice_key};
use super::*;
use crate::sound_preview::{
    is_playable_audio_reference, map_distance_gain, serialized_fade_seconds,
    serialized_sound_volume, SoundPreview, SoundVoiceGroup, SoundVoiceKey, SoundVoiceSpec,
};

#[derive(Debug, Clone, Copy)]
struct AmbientCandidate {
    sound_index: usize,
    gain: f32,
    distance: f32,
    pan: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ObjectAudioEventPlan {
    stop_loop_channel: u8,
    one_shot: Option<(u8, u32)>,
}

fn object_audio_event_plan(
    trigger_type: u32,
    event_mask: u32,
    profile: ObjectAudioProfile,
) -> Option<ObjectAudioEventPlan> {
    // Native handlers test 0x100 first, so a combined 0x300 mask activates.
    if event_mask & ROBOTS_EVENT_ACTIVATE != 0 {
        return Some(ObjectAudioEventPlan {
            stop_loop_channel: 3,
            one_shot: (trigger_type != 55)
                .then_some(profile.activate)
                .flatten()
                .map(|hashcode| (0, hashcode)),
        });
    }
    if event_mask & ROBOTS_EVENT_DEACTIVATE != 0 {
        return Some(ObjectAudioEventPlan {
            stop_loop_channel: 2,
            one_shot: (trigger_type != 55)
                .then_some(profile.deactivate)
                .flatten()
                .map(|hashcode| (1, hashcode)),
        });
    }
    None
}

fn object_audio_loop(
    trigger_type: u32,
    active: bool,
    last_event: Option<u32>,
    profile: ObjectAudioProfile,
) -> Option<(u8, u32)> {
    if active {
        return profile.active_loop.map(|hashcode| (2, hashcode));
    }
    if trigger_type == 55 {
        return None;
    }
    last_event
        .is_some_and(|event| event & ROBOTS_EVENT_DEACTIVATE != 0)
        .then_some(profile.inactive_loop)
        .flatten()
        .map(|hashcode| (3, hashcode))
}

fn object_audio_voice_key(map_hashcode: u32, trigger_index: usize, channel: u8) -> SoundVoiceKey {
    SoundVoiceKey::ObjectAudio {
        map_hashcode,
        trigger_index,
        channel,
    }
}

fn containing_sound_zones(map: &ProcessedMap, listener: Vec3) -> Vec<usize> {
    robots_map_zone_index_by_bounds(map.zones.len(), listener, |index| {
        let zone = &map.zones[index];
        (
            Vec3::from(zone.bounds_box[0]),
            Vec3::from(zone.bounds_box[1]),
        )
    })
    .into_iter()
    .collect()
}

fn listener_pan(listener_rotation: Quat, direction: Vec3) -> f32 {
    let local = listener_rotation.conjugate().mul_vec3(direction);
    let horizontal = Vec3::new(local.x, 0.0, local.z).normalize_or_zero();
    horizontal.x.clamp(-1.0, 1.0)
}

#[derive(Debug, Clone, Copy)]
struct NativeSpatialMix {
    gain: f32,
    pan: f32,
    looping: bool,
}

fn native_profile_spatial_mix(
    profile: Option<crate::sound_native::NativeSoundProfile>,
    listener_rotation: Quat,
    offset: Vec3,
) -> NativeSpatialMix {
    let fallback_pan = listener_pan(listener_rotation, offset);
    let Some(profile) = profile else {
        return NativeSpatialMix {
            gain: 1.0,
            pan: fallback_pan,
            looping: false,
        };
    };

    if !profile.is_3d && profile.tracking_type & 0x01 == 0 {
        return NativeSpatialMix {
            gain: profile.master_volume,
            pan: 0.0,
            looping: profile.looping,
        };
    }

    NativeSpatialMix {
        gain: profile.master_volume
            * map_distance_gain(offset.length(), profile.inner_radius, profile.outer_radius),
        pan: fallback_pan,
        looping: profile.looping,
    }
}

fn native_sound_spatial_mix(
    preview: &mut SoundPreview,
    hashcode: u32,
    listener_position: Vec3,
    listener_rotation: Quat,
    emitter_position: Vec3,
) -> NativeSpatialMix {
    native_profile_spatial_mix(
        preview.native_sound_profile(hashcode),
        listener_rotation,
        emitter_position - listener_position,
    )
}

fn ambient_candidates(
    map: &ProcessedMap,
    preview: &mut SoundPreview,
    listener_position: Vec3,
    listener_rotation: Quat,
    maximum_voices: usize,
) -> Vec<AmbientCandidate> {
    let mut seen = std::collections::BTreeSet::new();
    let mut candidates = containing_sound_zones(map, listener_position)
        .into_iter()
        .flat_map(|zone_index| map.zones[zone_index].sound_array.iter().copied())
        .filter(|sound_index| seen.insert(*sound_index))
        .filter_map(|sound_index| {
            let sound_index = sound_index as usize;
            let sound = map.sounds.get(sound_index)?;
            if !is_playable_audio_reference(sound.sound_ref) {
                return None;
            }
            let offset = sound.position - listener_position;
            let is_spatial = sound.outer_radius > 0.0;
            let distance = if is_spatial { offset.length() } else { 0.0 };
            let distance_gain = if is_spatial {
                map_distance_gain(distance, sound.inner_radius, sound.outer_radius)
            } else {
                1.0
            };
            let sfx_master_volume = preview
                .native_sound_profile(sound.sound_ref)
                .map(|profile| profile.master_volume)
                .unwrap_or(1.0);
            let gain = serialized_sound_volume(sound.volume) * sfx_master_volume * distance_gain;
            (gain > 0.0001).then_some(AmbientCandidate {
                sound_index,
                gain,
                distance,
                pan: if is_spatial {
                    listener_pan(listener_rotation, offset)
                } else {
                    0.0
                },
            })
        })
        .collect::<Vec<_>>();

    candidates.sort_by(|left, right| {
        right
            .gain
            .total_cmp(&left.gain)
            .then_with(|| left.distance.total_cmp(&right.distance))
            .then_with(|| left.sound_index.cmp(&right.sound_index))
    });
    candidates.truncate(maximum_voices.max(1));
    candidates
}

impl MapFrame {
    pub(super) fn dispatch_object_audio_event(
        &mut self,
        map: &ProcessedMap,
        trigger_index: usize,
        event_mask: u32,
    ) {
        let Some(trigger) = map.triggers.get(trigger_index) else {
            return;
        };
        if !robots_object_audio_is_enabled(trigger) {
            return;
        }
        let Some(profile) = robots_object_audio_profile_for_source(map, trigger_index) else {
            return;
        };
        let Some(plan) = object_audio_event_plan(trigger.ttype, event_mask, profile) else {
            return;
        };
        let (listener_position, listener_rotation) = {
            let mut viewer = self.viewer.lock();
            let camera = viewer.camera_mut();
            (camera.position(), camera.rotation())
        };
        let state_key = ((map.hashcode as u64) << 32) | trigger_index as u64;
        let event_snapshot = self
            .runtime_event_states
            .get(&state_key)
            .map(|state| state.snapshot(trigger, self.runtime_path_playback_speed));
        let emitter_position = runtime_path_preview_position_with_event(
            map,
            trigger,
            0.0,
            false,
            self.runtime_path_playback_speed,
            event_snapshot,
        );
        let mut preview = self.sound_preview.lock();
        if !preview.object_audio_enabled {
            return;
        }
        preview.preload_hashes(profile.playable_hashes());
        preview.stop_voice(
            object_audio_voice_key(map.hashcode, trigger_index, plan.stop_loop_channel),
            0.03,
        );
        if let Some((channel, hashcode)) = plan.one_shot {
            let mix = native_sound_spatial_mix(
                &mut preview,
                hashcode,
                listener_position,
                listener_rotation,
                emitter_position,
            );
            if mix.gain > 0.0001 {
                preview.restart_voice(
                    object_audio_voice_key(map.hashcode, trigger_index, channel),
                    SoundVoiceSpec {
                        hashcode,
                        looping: false,
                        volume: mix.gain,
                        speed: 1.0,
                        pan: mix.pan,
                        fade_in_seconds: 0.0,
                        fade_out_seconds: 0.03,
                        seek_seconds: 0.0,
                    },
                );
            }
        }
    }

    pub(super) fn stop_object_audio_for_trigger(
        &mut self,
        map_hashcode: u32,
        trigger_index: usize,
    ) {
        let mut preview = self.sound_preview.lock();
        for channel in 0..=3 {
            preview.stop_voice(
                object_audio_voice_key(map_hashcode, trigger_index, channel),
                0.03,
            );
        }
    }

    fn sync_object_audio_loops(
        &mut self,
        map: &ProcessedMap,
        listener_position: Vec3,
        listener_rotation: Quat,
    ) {
        let mut preview = self.sound_preview.lock();
        if !self.native_runtime_event_gate || !preview.object_audio_enabled {
            preview.stop_group(SoundVoiceGroup::ObjectAudio, 0.05);
            return;
        }

        let mut preload = Vec::new();
        let mut desired = Vec::new();
        for (trigger_index, trigger) in map.triggers.iter().enumerate() {
            if !robots_object_audio_is_enabled(trigger) {
                continue;
            }
            let Some(profile) = robots_object_audio_profile_for_source(map, trigger_index) else {
                continue;
            };
            preload.extend(profile.playable_hashes());
            let key = ((map.hashcode as u64) << 32) | trigger_index as u64;
            let state = self.runtime_event_states.get(&key);
            let active = state.is_some_and(|state| state.active);
            let last_event = state.and_then(|state| state.last_event);
            let Some((channel, hashcode)) =
                object_audio_loop(trigger.ttype, active, last_event, profile)
            else {
                continue;
            };
            let event_snapshot =
                state.map(|state| state.snapshot(trigger, self.runtime_path_playback_speed));
            let loop_seek_seconds = if active {
                event_snapshot
                    .map(|snapshot| snapshot.elapsed_seconds)
                    .unwrap_or_default()
            } else {
                0.0
            };
            let emitter_position = runtime_path_preview_position_with_event(
                map,
                trigger,
                0.0,
                false,
                self.runtime_path_playback_speed,
                event_snapshot,
            );
            let mix = native_sound_spatial_mix(
                &mut preview,
                hashcode,
                listener_position,
                listener_rotation,
                emitter_position,
            );
            if mix.gain > 0.0001 {
                desired.push((
                    object_audio_voice_key(map.hashcode, trigger_index, channel),
                    SoundVoiceSpec {
                        hashcode,
                        looping: true,
                        volume: mix.gain,
                        speed: 1.0,
                        pan: mix.pan,
                        fade_in_seconds: 0.03,
                        fade_out_seconds: 0.05,
                        seek_seconds: loop_seek_seconds,
                    },
                ));
            }
        }
        preview.preload_hashes(preload);
        preview.sync_object_audio_loops(desired, 0.05);
    }

    pub(super) fn sync_map_ambient_audio(
        &mut self,
        map: &ProcessedMap,
        current_file: Hashcode,
        script_global_time: f32,
        runtime_time: f32,
        listener_position: Vec3,
        listener_rotation: Quat,
        context: &egui::Context,
    ) {
        self.sync_object_audio_loops(map, listener_position, listener_rotation);
        let mut preview = self.sound_preview.lock();
        preview.tick();
        let ambient_enabled = preview.ambient_enabled;
        if !ambient_enabled {
            preview.stop_group(SoundVoiceGroup::MapAmbient, 0.1);
        }

        let maximum_voices = preview.max_ambient_voices;
        let candidates = ambient_enabled
            .then(|| {
                ambient_candidates(
                    map,
                    &mut preview,
                    listener_position,
                    listener_rotation,
                    maximum_voices,
                )
            })
            .unwrap_or_default();
        preview.preload_hashes(
            candidates
                .iter()
                .filter_map(|candidate| map.sounds.get(candidate.sound_index))
                .map(|sound| sound.sound_ref),
        );

        let desired = candidates.into_iter().filter_map(|candidate| {
            let sound = map.sounds.get(candidate.sound_index)?;
            Some((
                SoundVoiceKey::MapAmbient {
                    map_hashcode: map.hashcode,
                    sound_index: candidate.sound_index,
                },
                SoundVoiceSpec {
                    hashcode: sound.sound_ref,
                    looping: true,
                    volume: candidate.gain,
                    speed: 1.0,
                    pan: candidate.pan,
                    fade_in_seconds: serialized_fade_seconds(sound.fade_in),
                    fade_out_seconds: serialized_fade_seconds(sound.fade_out),
                    // Map ambient loops continue in native time while virtualized
                    // outside the active zone/radius. Re-enter at the map phase
                    // instead of restarting every loop from sample zero.
                    seek_seconds: runtime_time.max(0.0),
                },
            ))
        });
        preview.sync_group(SoundVoiceGroup::MapAmbient, desired, 0.1);
        let (script_preload_hashes, script_events) = {
            let render_store = self.render_store.read();
            let instances = map_script_audio_instances(
                map,
                current_file,
                &render_store,
                script_global_time,
                runtime_time,
                self.animate_scripts,
                self.script_playback_speed,
                self.animate_runtime_paths,
                self.runtime_path_playback_speed,
                self.render_filter,
                &self.runtime_event_states,
            );
            let preload_hashes = instances
                .iter()
                .flat_map(|instance| {
                    crate::scripts::sound::script_sound_hashes(
                        &render_store,
                        instance.file,
                        instance.script,
                    )
                })
                .collect::<Vec<_>>();
            let active_events = if self.animate_scripts {
                instances
                    .iter()
                    .flat_map(|instance| {
                        crate::scripts::sound::active_script_sound_events(
                            &render_store,
                            instance.file,
                            instance.script,
                            instance.current_time,
                            instance.instance_seed,
                        )
                        .into_iter()
                        .filter_map(
                            move |(key, hashcode, seek_seconds)| {
                                map_script_voice_key(map.hashcode, key)
                                    .map(|key| (key, hashcode, seek_seconds, instance.position))
                            },
                        )
                    })
                    .collect::<Vec<_>>()
            } else {
                vec![]
            };
            (preload_hashes, active_events)
        };
        preview.preload_hashes(script_preload_hashes);
        if preview.script_enabled && self.animate_scripts {
            let desired = script_events
                .into_iter()
                .filter_map(|(key, hashcode, seek_seconds, emitter_position)| {
                    let mix = native_sound_spatial_mix(
                        &mut preview,
                        hashcode,
                        listener_position,
                        listener_rotation,
                        emitter_position,
                    );
                    (mix.gain > 0.0001).then_some((
                        key,
                        SoundVoiceSpec {
                            hashcode,
                            looping: mix.looping,
                            volume: mix.gain,
                            speed: self.script_playback_speed.max(0.05),
                            pan: mix.pan,
                            fade_in_seconds: 0.01,
                            fade_out_seconds: 0.03,
                            seek_seconds: seek_seconds.max(0.0),
                        },
                    ))
                })
                .collect::<Vec<_>>();
            preview.sync_group(SoundVoiceGroup::MapScript, desired, 0.03);
            preview.resume_group(SoundVoiceGroup::MapScript);
        } else {
            preview.stop_group(SoundVoiceGroup::MapScript, 0.03);
        }
        if preview.has_pending_work()
            || preview.voice_count(SoundVoiceGroup::MapAmbient) > 0
            || preview.voice_count(SoundVoiceGroup::ObjectAudio) > 0
            || preview.voice_count(SoundVoiceGroup::MapScript) > 0
        {
            context.request_repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listener_pan_tracks_camera_local_left_and_right() {
        assert!(listener_pan(Quat::IDENTITY, Vec3::X) > 0.99);
        assert!(listener_pan(Quat::IDENTITY, -Vec3::X) < -0.99);
        assert!(listener_pan(Quat::from_rotation_y(std::f32::consts::PI), Vec3::X) < -0.99);
    }

    #[test]
    fn native_3d_sound_uses_master_volume_and_radius_falloff() {
        let profile = crate::sound_native::NativeSoundProfile {
            inner_radius: 10.0,
            outer_radius: 40.0,
            looping: true,
            is_3d: true,
            tracking_type: 1,
            master_volume: 0.8,
            ..crate::sound_native::NativeSoundProfile::default()
        };
        let inner = native_profile_spatial_mix(Some(profile), Quat::IDENTITY, Vec3::X * 10.0);
        assert!((inner.gain - 0.8).abs() < 0.0001);
        assert!(inner.pan > 0.99);
        assert!(inner.looping);

        let middle = native_profile_spatial_mix(Some(profile), Quat::IDENTITY, Vec3::X * 25.0);
        assert!((middle.gain - 0.4).abs() < 0.0001);

        let outside = native_profile_spatial_mix(Some(profile), Quat::IDENTITY, Vec3::X * 40.0);
        assert_eq!(outside.gain, 0.0);
    }

    #[test]
    fn native_2d_sound_ignores_distance_and_pan() {
        let profile = crate::sound_native::NativeSoundProfile {
            inner_radius: 1.0,
            outer_radius: 2.0,
            looping: false,
            is_3d: false,
            tracking_type: 0,
            master_volume: 0.65,
            ..crate::sound_native::NativeSoundProfile::default()
        };
        let mix = native_profile_spatial_mix(Some(profile), Quat::IDENTITY, Vec3::X * 5000.0);
        assert!((mix.gain - 0.65).abs() < 0.0001);
        assert_eq!(mix.pan, 0.0);
        assert!(!mix.looping);
    }

    fn test_object_audio_profile() -> ObjectAudioProfile {
        ObjectAudioProfile {
            linked_trigger_index: Some(9),
            activate: Some(0x1AF0_1000),
            deactivate: Some(0x1AF0_1001),
            active_loop: Some(0x1AF0_1002),
            inactive_loop: Some(0x1AF0_1003),
        }
    }

    #[test]
    fn object_audio_activate_and_deactivate_use_exact_channels() {
        let profile = test_object_audio_profile();
        assert_eq!(
            object_audio_event_plan(80, ROBOTS_EVENT_ACTIVATE, profile),
            Some(ObjectAudioEventPlan {
                stop_loop_channel: 3,
                one_shot: Some((0, 0x1AF0_1000)),
            })
        );
        assert_eq!(
            object_audio_event_plan(80, ROBOTS_EVENT_DEACTIVATE, profile),
            Some(ObjectAudioEventPlan {
                stop_loop_channel: 2,
                one_shot: Some((1, 0x1AF0_1001)),
            })
        );
        assert_eq!(
            object_audio_loop(80, true, Some(ROBOTS_EVENT_ACTIVATE), profile),
            Some((2, 0x1AF0_1002))
        );
        assert_eq!(
            object_audio_loop(80, false, Some(ROBOTS_EVENT_DEACTIVATE), profile),
            Some((3, 0x1AF0_1003))
        );
        assert_eq!(object_audio_loop(80, false, None, profile), None);
    }

    #[test]
    fn object_audio_combined_event_uses_native_activate_precedence() {
        let plan = object_audio_event_plan(
            37,
            ROBOTS_EVENT_ACTIVATE | ROBOTS_EVENT_DEACTIVATE,
            test_object_audio_profile(),
        )
        .unwrap();
        assert_eq!(plan.stop_loop_channel, 3);
        assert_eq!(plan.one_shot, Some((0, 0x1AF0_1000)));
    }

    #[test]
    fn clock_object_audio_uses_only_active_loop() {
        let profile = test_object_audio_profile();
        assert_eq!(
            object_audio_event_plan(55, ROBOTS_EVENT_ACTIVATE, profile)
                .unwrap()
                .one_shot,
            None
        );
        assert_eq!(
            object_audio_loop(55, true, Some(ROBOTS_EVENT_ACTIVATE), profile),
            Some((2, 0x1AF0_1002))
        );
        assert_eq!(
            object_audio_loop(55, false, Some(ROBOTS_EVENT_DEACTIVATE), profile),
            None
        );
    }
}
