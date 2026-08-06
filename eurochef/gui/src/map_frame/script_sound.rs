use super::*;
use crate::sound_preview::SoundVoiceKey;

#[derive(Debug, Clone, Copy)]
pub(super) struct MapScriptAudioInstance {
    pub(super) file: Hashcode,
    pub(super) script: Hashcode,
    pub(super) current_time: f32,
    pub(super) instance_seed: u64,
    pub(super) position: Vec3,
}

pub(super) fn map_script_voice_key(map_hashcode: u32, key: SoundVoiceKey) -> Option<SoundVoiceKey> {
    let SoundVoiceKey::Script {
        file,
        script,
        command_path,
    } = key
    else {
        return None;
    };
    Some(SoundVoiceKey::MapScript {
        map_hashcode,
        file,
        script,
        command_path,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn map_script_audio_instances(
    map: &ProcessedMap,
    current_file: Hashcode,
    render_store: &RenderStore,
    script_global_time: f32,
    runtime_time: f32,
    animate_scripts: bool,
    script_playback_speed: f32,
    animate_runtime_paths: bool,
    runtime_path_playback_speed: f32,
    render_filter: RenderFilter,
    runtime_event_states: &FxHashMap<u64, RuntimeEventPreviewState>,
) -> Vec<MapScriptAudioInstance> {
    let mut instances = Vec::new();

    if render_filter.contains(RenderFilter::Placements) {
        for (index, placement) in map.placements.iter().enumerate() {
            if placement.object_ref.base() != 0x0400_0000 {
                continue;
            }
            if render_store
                .get_script(current_file, placement.object_ref)
                .is_none()
            {
                continue;
            }
            instances.push(MapScriptAudioInstance {
                file: current_file,
                script: placement.object_ref,
                current_time: resolved_map_script_time(
                    render_store,
                    current_file,
                    placement.object_ref,
                    script_global_time,
                    animate_scripts,
                    script_playback_speed,
                ),
                instance_seed: 0x1000_0000_0000_0000 | index as u64 + 1,
                position: placement.position.into(),
            });
        }
    }

    if render_filter.contains(RenderFilter::Triggers) {
        for (index, trigger) in map.triggers.iter().enumerate() {
            let Some(visual_object) = trigger.engine_options.visual_object else {
                continue;
            };
            let visual_file = trigger_visual_file(
                current_file,
                visual_object,
                trigger.engine_options.visual_object_file,
            );
            let script_hashcode = match visual_object.base() {
                0x0200_0000 => render_store.find_assembly_script(visual_file, visual_object),
                0x0400_0000 => render_store
                    .get_script(visual_file, visual_object)
                    .map(|_| visual_object),
                _ => None,
            };
            let Some(script_hashcode) = script_hashcode else {
                continue;
            };
            if render_store
                .get_script(visual_file, script_hashcode)
                .is_none()
            {
                continue;
            }
            let state_key = ((map.hashcode as u64) << 32) | index as u64;
            let event_snapshot = runtime_event_states
                .get(&state_key)
                .map(|state| state.snapshot(trigger, runtime_path_playback_speed));
            instances.push(MapScriptAudioInstance {
                file: visual_file,
                script: script_hashcode,
                current_time: resolved_map_script_time(
                    render_store,
                    visual_file,
                    script_hashcode,
                    script_global_time,
                    animate_scripts,
                    script_playback_speed,
                ),
                instance_seed: 0x2000_0000_0000_0000 | index as u64 + 1,
                position: runtime_path_preview_position_with_event(
                    map,
                    trigger,
                    runtime_time,
                    animate_runtime_paths,
                    runtime_path_playback_speed,
                    event_snapshot,
                ),
            });
        }
    }

    instances
}

#[cfg(test)]
mod tests {
    use super::*;
    use eurochef_edb::{edb::EdbFile, versions::Platform};
    use std::{fs::File, io::BufReader};

    #[test]
    fn real_map_placements_with_sound_are_collected_when_requested() {
        let Ok(path) = std::env::var("EUROCHEF_REAL_AUDIO_EDB") else {
            return;
        };
        let file_uid = std::env::var("EUROCHEF_REAL_AUDIO_FILE_UID")
            .ok()
            .and_then(|value| u32::from_str_radix(value.trim_start_matches("0x"), 16).ok())
            .expect("EUROCHEF_REAL_AUDIO_FILE_UID must be a hex EDB UID");
        let open_edb = || {
            EdbFile::new(
                Box::new(BufReader::new(
                    File::open(&path).expect("open real map EDB"),
                )),
                Platform::Pc,
            )
            .expect("parse real map EDB")
        };

        let mut map_edb = open_edb();
        let maps = crate::maps::read_from_file(&mut map_edb);
        let mut script_edb = open_edb();
        let scripts = UXGeoScript::read_all(&mut script_edb).expect("read real map scripts");
        let mut render_store = RenderStore::new();
        for script in scripts {
            render_store.insert_script(file_uid, script);
        }

        let runtime_states = FxHashMap::default();
        let instances = maps
            .iter()
            .flat_map(|map| {
                map_script_audio_instances(
                    map,
                    file_uid,
                    &render_store,
                    0.0,
                    0.0,
                    true,
                    1.0,
                    false,
                    1.0,
                    RenderFilter::Placements | RenderFilter::Triggers,
                    &runtime_states,
                )
            })
            .collect::<Vec<_>>();
        let audible = instances
            .iter()
            .filter(|instance| {
                !crate::scripts::sound::script_sound_hashes(
                    &render_store,
                    instance.file,
                    instance.script,
                )
                .is_empty()
            })
            .count();

        assert!(
            !instances.is_empty(),
            "real map has no rendered Script instances"
        );
        assert!(
            audible > 0,
            "real rendered Script instances have no Sound commands"
        );
    }

    #[test]
    fn map_script_key_isolated_from_script_panel_group() {
        let key = map_script_voice_key(
            0x0100_0071,
            SoundVoiceKey::Script {
                file: 0x0100_0071,
                script: 0x0400_0123,
                command_path: 77,
            },
        )
        .unwrap();
        assert_eq!(
            key,
            SoundVoiceKey::MapScript {
                map_hashcode: 0x0100_0071,
                file: 0x0100_0071,
                script: 0x0400_0123,
                command_path: 77,
            }
        );
    }
}
