use super::*;
use crate::sound_preview::{
    is_playable_audio_reference, map_distance_gain, serialized_fade_seconds,
    serialized_sound_volume, SoundVoiceGroup, SoundVoiceKey, SoundVoiceSpec,
};

#[derive(Debug, Clone, Copy)]
struct AmbientCandidate {
    sound_index: usize,
    gain: f32,
    distance: f32,
    pan: f32,
}

#[cfg(test)]
fn smallest_containing_bounds(bounds: &[(Vec3, Vec3)], listener: Vec3) -> Option<usize> {
    bounds
        .iter()
        .enumerate()
        .filter_map(|(index, (bounds_min, bounds_max))| {
            let contains = listener.cmpge(*bounds_min).all() && listener.cmple(*bounds_max).all();
            contains.then_some((index, (*bounds_max - *bounds_min).max(Vec3::ZERO)))
        })
        .min_by(|(_, left), (_, right)| {
            (left.x * left.y * left.z).total_cmp(&(right.x * right.y * right.z))
        })
        .map(|(index, _)| index)
}

fn containing_sound_zones(map: &ProcessedMap, listener: Vec3) -> Vec<usize> {
    map.zones
        .iter()
        .enumerate()
        .filter_map(|(index, zone)| {
            let a = Vec3::from(zone.bounds_box[0]);
            let b = Vec3::from(zone.bounds_box[1]);
            (listener.cmpge(a.min(b)).all() && listener.cmple(a.max(b)).all()).then_some(index)
        })
        .collect()
}

fn listener_pan(listener_rotation: Quat, direction: Vec3) -> f32 {
    let local = listener_rotation.conjugate().mul_vec3(direction);
    let horizontal = Vec3::new(local.x, 0.0, local.z).normalize_or_zero();
    horizontal.x.clamp(-1.0, 1.0)
}

fn ambient_candidates(
    map: &ProcessedMap,
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
            let gain = serialized_sound_volume(sound.volume) * distance_gain;
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
    pub(super) fn sync_map_ambient_audio(
        &mut self,
        map: &ProcessedMap,
        current_file: Hashcode,
        script_global_time: f32,
        listener_position: Vec3,
        listener_rotation: Quat,
        context: &egui::Context,
    ) {
        let mut preview = self.sound_preview.lock();
        preview.tick();
        let ambient_enabled = preview.ambient_enabled;
        if !ambient_enabled {
            preview.stop_group(SoundVoiceGroup::MapAmbient, 0.1);
        }

        let maximum_voices = preview.max_ambient_voices;
        let candidates = ambient_enabled
            .then(|| ambient_candidates(map, listener_position, listener_rotation, maximum_voices))
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
                    seek_seconds: 0.0,
                },
            ))
        });
        preview.sync_group(SoundVoiceGroup::MapAmbient, desired, 0.1);
        let script_events = if self.animate_scripts {
            let render_store = self.render_store.read();
            map.placements
                .iter()
                .enumerate()
                .filter(|(_, placement)| placement.object_ref.base() == 0x0400_0000)
                .flat_map(|(index, placement)| {
                    let script_time = render_store
                        .get_script(current_file, placement.object_ref)
                        .map(|script| {
                            map_script_time(
                                script,
                                script_global_time,
                                self.animate_scripts,
                                self.script_playback_speed,
                            )
                        })
                        .unwrap_or_default();
                    crate::scripts::sound::active_script_sound_events(
                        &render_store,
                        current_file,
                        placement.object_ref,
                        script_time,
                        index as u64 + 1,
                    )
                })
                .collect::<Vec<_>>()
        } else {
            vec![]
        };
        preview.preload_hashes(script_events.iter().map(|(_, hashcode, _)| *hashcode));
        if preview.script_enabled {
            preview.sync_group(
                SoundVoiceGroup::Script,
                script_events
                    .into_iter()
                    .map(|(key, hashcode, seek_seconds)| {
                        (
                            key,
                            SoundVoiceSpec {
                                hashcode,
                                looping: false,
                                volume: 1.0,
                                speed: self.script_playback_speed.max(0.05),
                                pan: 0.0,
                                fade_in_seconds: 0.01,
                                fade_out_seconds: 0.03,
                                seek_seconds: seek_seconds.max(0.0),
                            },
                        )
                    }),
                0.03,
            );
        } else {
            preview.stop_group(SoundVoiceGroup::Script, 0.03);
        }
        if preview.has_pending_work() || preview.voice_count(SoundVoiceGroup::MapAmbient) > 0 {
            context.request_repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn containing_sound_zone_prefers_smallest_nested_bounds() {
        let bounds = vec![
            (Vec3::splat(-10.0), Vec3::splat(10.0)),
            (Vec3::splat(-2.0), Vec3::splat(2.0)),
        ];
        assert_eq!(smallest_containing_bounds(&bounds, Vec3::ZERO), Some(1));
        assert_eq!(smallest_containing_bounds(&bounds, Vec3::splat(20.0)), None);
    }

    #[test]
    fn listener_pan_tracks_camera_local_left_and_right() {
        assert!(listener_pan(Quat::IDENTITY, Vec3::X) > 0.99);
        assert!(listener_pan(Quat::IDENTITY, -Vec3::X) < -0.99);
        assert!(listener_pan(Quat::from_rotation_y(std::f32::consts::PI), Vec3::X) < -0.99);
    }
}
