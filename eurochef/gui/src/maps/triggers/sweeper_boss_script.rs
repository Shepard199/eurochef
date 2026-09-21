use std::collections::BTreeMap;

use eurochef_edb::edb::EdbFile;
use eurochef_shared::script::{UXGeoScript, UXGeoScriptCommandData};

use super::sweeper_boss::{
    EYE_SCRIPT_CLOSED, EYE_SCRIPT_CLOSING, EYE_SCRIPT_DESTROYED, EYE_SCRIPT_HIT, EYE_SCRIPT_OPEN,
    EYE_SCRIPT_OPENING, RAT_SCRIPT_VALUE_EVENT,
};

const EYE_RUNTIME_SCRIPTS: [u32; 6] = [
    EYE_SCRIPT_CLOSED,
    EYE_SCRIPT_OPENING,
    EYE_SCRIPT_OPEN,
    EYE_SCRIPT_CLOSING,
    EYE_SCRIPT_HIT,
    EYE_SCRIPT_DESTROYED,
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeSweeperStandaloneScriptValueEvent {
    pub start_frame: i16,
    pub length: u16,
    pub value: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeSweeperStandaloneScriptProfile {
    pub script: u32,
    pub frame_rate: f32,
    pub length: u32,
    pub events: Vec<NativeSweeperStandaloneScriptValueEvent>,
}

/// Data-only catalog for the Eye-owned standalone `EXItemAnimator_Script` resources in
/// FinalBoss.edb (0x010000BC). Runtime state lives elsewhere; this object is immutable map data.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct RobotsSweeperEyeScripts {
    profiles: BTreeMap<u32, NativeSweeperStandaloneScriptProfile>,
}

impl RobotsSweeperEyeScripts {
    pub(crate) fn read(edb: &mut EdbFile) -> anyhow::Result<Self> {
        let endian = edb.endian;
        let scripts = UXGeoScript::read_all(edb)?;
        let mut profiles = BTreeMap::new();

        for script in scripts {
            if !EYE_RUNTIME_SCRIPTS.contains(&script.hashcode) {
                continue;
            }
            let mut events = Vec::new();
            for command in &script.commands {
                let UXGeoScriptCommandData::Event { event_type, data } = &command.data else {
                    continue;
                };
                if *event_type != RAT_SCRIPT_VALUE_EVENT {
                    continue;
                }
                let Some(bytes) = data.get(4..8) else {
                    continue;
                };
                let bytes: [u8; 4] = bytes.try_into().expect("four-byte ScriptValue payload");
                let value = match endian {
                    eurochef_edb::binrw::Endian::Little => f32::from_le_bytes(bytes),
                    eurochef_edb::binrw::Endian::Big => f32::from_be_bytes(bytes),
                };
                events.push(NativeSweeperStandaloneScriptValueEvent {
                    start_frame: command.start,
                    length: command.length,
                    value,
                });
            }

            profiles.insert(
                script.hashcode,
                NativeSweeperStandaloneScriptProfile {
                    script: script.hashcode,
                    frame_rate: script.timeline_framerate(),
                    length: script.length,
                    events,
                },
            );
        }

        Ok(Self { profiles })
    }

    pub(crate) fn profile(&self, script: u32) -> Option<&NativeSweeperStandaloneScriptProfile> {
        self.profiles.get(&script)
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.profiles.len()
    }

    #[cfg(test)]
    pub(crate) fn from_profiles_for_test(
        profiles: impl IntoIterator<Item = NativeSweeperStandaloneScriptProfile>,
    ) -> Self {
        Self {
            profiles: profiles
                .into_iter()
                .map(|profile| (profile.script, profile))
                .collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct NativeSweeperStandaloneScriptStep {
    pub frame_before: f32,
    pub frame_after: f32,
    pub last_script_value: Option<f32>,
    pub emitted_script_value_count: u8,
}

/// Runtime subset of standalone `EXItemAnimator_Script` needed by the Sweeper Eye scripts.
///
/// This stays separate from the Ratchet family-4 runtime because native Robots uses a
/// different animator class and clock. `0x004E8C0D` initializes base animator `+0x100`
/// to 1.0; `0x004FA5A8` passes `DAT_00620050 * +0x100` to `0x004FA3AD`; the normal
/// non-blocking Eye path dispatches commands at the current integer Script frame and only
/// then advances `+0x104` by 1.0. Eye callback `0x004CE940` returns zero, so standalone
/// positive-return hold semantics do not apply here.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct NativeSweeperStandaloneScriptRuntime {
    pub active_script: Option<u32>,
    pub current_frame: f32,
    pub next_event_index: usize,
}

impl NativeSweeperStandaloneScriptRuntime {
    pub(crate) fn restart(&mut self, script: u32) {
        self.active_script = Some(script);
        self.current_frame = 0.0;
        self.next_event_index = 0;
    }

    pub(crate) fn step(
        &mut self,
        catalog: &RobotsSweeperEyeScripts,
    ) -> NativeSweeperStandaloneScriptStep {
        let before = self.current_frame;
        let Some(script) = self.active_script else {
            return NativeSweeperStandaloneScriptStep {
                frame_before: before,
                frame_after: before,
                ..Default::default()
            };
        };
        let Some(profile) = catalog.profile(script) else {
            return NativeSweeperStandaloneScriptStep {
                frame_before: before,
                frame_after: before,
                ..Default::default()
            };
        };

        let current_integer_frame = before.trunc() as i32;
        let mut result = NativeSweeperStandaloneScriptStep {
            frame_before: before,
            frame_after: before,
            ..Default::default()
        };
        while let Some(event) = profile.events.get(self.next_event_index).copied() {
            if i32::from(event.start_frame) > current_integer_frame {
                break;
            }
            self.next_event_index += 1;
            result.last_script_value = Some(event.value);
            result.emitted_script_value_count = result.emitted_script_value_count.saturating_add(1);
        }

        self.current_frame = (before + 1.0).min(profile.length as f32);
        result.frame_after = self.current_frame;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eye_catalog_with_event(start_frame: i16) -> RobotsSweeperEyeScripts {
        let mut profiles = BTreeMap::new();
        profiles.insert(
            EYE_SCRIPT_OPENING,
            NativeSweeperStandaloneScriptProfile {
                script: EYE_SCRIPT_OPENING,
                frame_rate: 30.0,
                length: 11,
                events: vec![NativeSweeperStandaloneScriptValueEvent {
                    start_frame,
                    length: 1,
                    value: 1.0,
                }],
            },
        );
        RobotsSweeperEyeScripts { profiles }
    }

    #[test]
    fn standalone_eye_script_dispatches_before_one_frame_advance() {
        let catalog = eye_catalog_with_event(10);
        let mut runtime = NativeSweeperStandaloneScriptRuntime::default();
        runtime.restart(EYE_SCRIPT_OPENING);

        for expected_before in 0..10 {
            let step = runtime.step(&catalog);
            assert_eq!(step.frame_before, expected_before as f32);
            assert_eq!(step.frame_after, expected_before as f32 + 1.0);
            assert_eq!(step.last_script_value, None);
        }

        let signal = runtime.step(&catalog);
        assert_eq!(signal.frame_before, 10.0);
        assert_eq!(signal.frame_after, 11.0);
        assert_eq!(signal.last_script_value, Some(1.0));
        assert_eq!(signal.emitted_script_value_count, 1);

        let one_shot = runtime.step(&catalog);
        assert_eq!(one_shot.last_script_value, None);
        assert_eq!(one_shot.emitted_script_value_count, 0);
    }
}
