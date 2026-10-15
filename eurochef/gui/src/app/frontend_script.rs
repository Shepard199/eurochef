//! Production-neutral frontend Script/Entity/Event ownership. Only proven
//! command timing and referenced resource IDs are represented here. We do NOT
//! pretend to interpret undocumented UI event payloads or animator matrices.
//! Gameplay NPC simulation remains in MapFrame, never in this module.
use std::collections::{HashMap, HashSet};
use anyhow::{Context, Result};
use eurochef_edb::edb::EdbFile;
use eurochef_shared::script::{UXGeoScript, UXGeoScriptCommandData};

pub(super) const TITLE_ROOT: u32 = 0x0400_02ae;
pub(super) const MAIN_MENU_PC: u32 = 0x0400_027d;
pub(super) const NEW_GAME_MENU: u32 = 0x0400_0010;
pub(super) const LOAD_GAME_MENU: u32 = 0x0400_0011;
pub(super) const OPTIONS_PC: u32 = 0x0400_02ec;
pub(super) const QUIT_PROMPT: u32 = 0x0400_02f8;

/// Event opcode 11 / type 0x16000008 has a 0x46 marker at
/// serialized command +0x1C and descriptor metadata scalar at +0x20.
/// Robots.exe 0x0043BE90 -> 0x0043D730 copies these into runtime
/// record +0x0C and +0x30; JumpToMarker receives the record ordinal, not metadata.
pub(super) const FRONTEND_MARKER_EVENT: u32 = 0x1600_0008;

#[derive(Debug, Clone, Copy, PartialEq)]
struct NativeFrontendMarker {
    uid: u32,
    payload_scalar: f32,
}

fn native_frontend_marker(event_type: u32, data: &[u8]) -> Option<NativeFrontendMarker> {
    if event_type != FRONTEND_MARKER_EVENT || data.len() != 20 { return None; }
    let read = |index: usize| -> Option<u32> {
        let start = index.checked_mul(4)?;
        Some(u32::from_le_bytes(data.get(start..start.checked_add(4)?)?.try_into().ok()?))
    };
    // Preserve only the independently recovered native marker layout.
    if (read(0)?, read(1)?, read(2)?) != (4, 4, 3) { return None; }
    let uid = read(3)?;
    let payload_scalar = f32::from_bits(read(4)?);
    if uid & 0xFF00_0000 != 0x4600_0000 ||
        super::frontend_hud_dispatch::native_marker_frame_lane(payload_scalar).is_none() {
        return None;
    }
    Some(NativeFrontendMarker { uid, payload_scalar })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FrontendScriptKind {
    Entity,
    SubScript,
    Sound,
    Event,
    Animation,
    Other,
}

#[derive(Clone, Debug)]
pub(super) struct FrontendScriptCommand {
    pub(super) kind: FrontendScriptKind,
    pub(super) reference_uid: Option<u32>,
    pub(super) start_frame: f32,
    pub(super) end_frame: f32,
    pub(super) controller_index: u8,
    pub(super) event_type: Option<u32>,
    pub(super) presentation_marker: Option<u32>,
    pub(super) presentation_payload_scalar: Option<f32>,
}

#[derive(Clone, Debug)]
pub(super) struct FrontendScriptNode {
    pub(super) uid: u32,
    pub(super) duration_frames: f32,
    pub(super) frames_per_second: f32,
    pub(super) commands: Vec<FrontendScriptCommand>,
    /// The MainMenu_PC paired Entity 0x82000031 native Y-position keyframes.
    chrome_y_keys: Option<Vec<(f32, f32)>>,
    /// Preserve the shipped H04 command→controller mapping, including
    /// repeated Entity UIDs on independent animation tracks.
    native_source: Option<Box<UXGeoScript>>,
}

/// An individual serialized H04 Entity command evaluated with the shared
/// Script renderer's controller interpolator. These are original local-space
/// tracks, NOT pixel coordinates or final inherited HUD matrices.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct FrontendEntityPose {
    pub(super) command_index: usize,
    pub(super) entity_uid: u32,
    pub(super) start_frame: f32,
    pub(super) end_frame: f32,
    pub(super) position: [f32; 3],
    pub(super) rotation: [f32; 4],
    pub(super) scale: [f32; 3],
}

fn original_main_menu_chrome_y_keys(script: &UXGeoScript) -> Option<Vec<(f32, f32)>> {
    if script.hashcode != MAIN_MENU_PC { return None; }
    // The shipped PC H04 MainMenu_PC command 1 and 2 both target
    // local Entity 0x82000031, with different X but identical native
    // Y-position keys at frames 0, 16, 36, 39, 54.
    let mut matching = script.commands.iter()
        .filter(|cmd| matches!(&cmd.data,
            UXGeoScriptCommandData::Entity { hashcode: 0x8200_0031, .. }))
        .filter_map(|cmd| script.controllers.get(usize::from(cmd.controller_header_index)))
        .map(|controller| &controller.channels.vector_0);
    let left = matching.next()?;
    let right = matching.next()?;
    if left.len() < 3 || left.len() != right.len() { return None; }
    let mut keys = Vec::new();
    for ((lf, lv), (rf, rv)) in left.iter().zip(right) {
        if !lf.is_finite() || !rf.is_finite() || !lv[1].is_finite() || !rv[1].is_finite()
            || (lf - rf).abs() > 0.001 || (lv[1] - rv[1]).abs() > 0.05
            || keys.last().is_some_and(|(frame, _): &(f32, f32)| *frame >= *lf)
        { return None; }
        keys.push((*lf, lv[1]));
    }
    let start = keys.first()?.1;
    let rest = keys.last()?.1;
    ((rest - start).abs() > 0.01).then_some(keys)
}

/// Native vector_0 uses the same sine easing as the shared Script renderer.
fn sample_chrome_key_y(keys: &[(f32, f32)], frame: f32) -> Option<f32> {
    let first = keys.first().copied()?;
    if frame <= first.0 { return Some(first.1); }
    for pair in keys.windows(2) {
        let (f0, y0) = pair[0];
        let (f1, y1) = pair[1];
        if frame <= f1 {
            let t = ((frame - f0) / (f1 - f0)).clamp(0.0, 1.0);
            let eased = crate::render::tweeny::ease_in_out_sine(t);
            return Some(y0 + (y1 - y0) * eased);
        }
    }
    keys.last().map(|(_, y)| *y)
}

impl FrontendScriptNode {
    fn from_native(script: UXGeoScript) -> Self {
        let fps = script.timeline_framerate();
        let chrome_y_keys = original_main_menu_chrome_y_keys(&script);
        let native_source = Some(Box::new(script.clone()));
        Self {
            uid: script.hashcode,
            duration_frames: script.length as f32,
            frames_per_second: fps,
            chrome_y_keys,
            native_source,
            commands: script.commands.into_iter().map(|command| {
                let (kind, reference_uid, event_type, presentation_marker, presentation_payload_scalar) = match command.data {
                    UXGeoScriptCommandData::Entity { hashcode, .. } =>
                        (FrontendScriptKind::Entity, Some(hashcode), None, None, None),
                    UXGeoScriptCommandData::SubScript { hashcode, .. } =>
                        (FrontendScriptKind::SubScript, Some(hashcode), None, None, None),
                    UXGeoScriptCommandData::Animation { anim_hashcode, .. } =>
                        (FrontendScriptKind::Animation, Some(anim_hashcode), None, None, None),
                    UXGeoScriptCommandData::Sound { hashcode } =>
                        (FrontendScriptKind::Sound, Some(hashcode), None, None, None),
                    UXGeoScriptCommandData::Event { event_type, data } => {
                        let marker = native_frontend_marker(event_type, &data);
                        (FrontendScriptKind::Event, None, Some(event_type),
                            marker.map(|m| m.uid), marker.map(|m| m.payload_scalar))
                    }
                    _ => (FrontendScriptKind::Other, None, None, None, None),
                };
                FrontendScriptCommand {
                    kind, reference_uid,
                    start_frame: f32::from(command.start),
                    end_frame: f32::from(command.start) + f32::from(command.length),
                    controller_index: command.controller_index,
                    event_type,
                    presentation_marker,
                    presentation_payload_scalar,
                }
            }).collect(),
        }
    }
}

#[derive(Default, Debug)]
pub(super) struct FrontendScriptCatalog {
    nodes: HashMap<u32, FrontendScriptNode>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct FrontendActiveCommand {
    pub(super) source_script_uid: u32,
    pub(super) command_index: usize,
    pub(super) kind: FrontendScriptKind,
    pub(super) resource_uid: Option<u32>,
    pub(super) controller_index: u8,
    pub(super) event_type: Option<u32>,
    pub(super) presentation_marker: Option<u32>,
    pub(super) presentation_payload_scalar: Option<f32>,
}

/// Start edge of a serialized H04 HudScript Event. The event UID is
/// preserved; gameplay/menu side effects must be mapped separately.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct FrontendScheduledEvent {
    pub(super) source_script_uid: u32,
    pub(super) command_index: usize,
    pub(super) event_type: u32,
    pub(super) presentation_marker: Option<u32>,
    pub(super) presentation_payload_scalar: Option<f32>,
    pub(super) at_seconds: f64,
}

#[derive(Default, Debug)]
pub(super) struct FrontendSceneFrame {
    pub(super) commands: Vec<FrontendActiveCommand>,
    pub(super) unresolved_subscripts: Vec<u32>,
    pub(super) cyclic_subscripts: Vec<u32>,
}

impl FrontendSceneFrame {
    pub(super) fn has_geometry(&self) -> bool {
        self.commands.iter().any(|cmd| matches!(
            cmd.kind, FrontendScriptKind::Entity | FrontendScriptKind::Animation
        ))
    }
}

impl FrontendScriptCatalog {
    pub(super) fn from_original_edb(edb: &mut EdbFile) -> Result<Self> {
        let relevant = [
            TITLE_ROOT, MAIN_MENU_PC, NEW_GAME_MENU, LOAD_GAME_MENU,
            OPTIONS_PC, QUIT_PROMPT,
        ];
        let mut pending = relevant.to_vec();
        let mut seen = HashSet::new();
        let mut nodes = HashMap::new();
        // Bounded closure through H04-local sub-scripts. No other EDB is
        // guessed, and a corrupt cyclic graph cannot expand indefinitely.
        while !pending.is_empty() {
            anyhow::ensure!(seen.len() <= 1024, "Frontend Script graph exceeds budget");
            let batch: Vec<u32> = std::mem::take(&mut pending)
                .into_iter().filter(|uid| seen.insert(*uid)).collect();
            if batch.is_empty() { continue; }
            for script in UXGeoScript::read_hashcodes(edb, &batch)
                .context("Reading original HudScript commands")?
            {
                let node = FrontendScriptNode::from_native(script);
                for command in &node.commands {
                    if command.kind == FrontendScriptKind::SubScript {
                        if let Some(uid) = command.reference_uid {
                            if !seen.contains(&uid) { pending.push(uid); }
                        }
                    }
                }
                nodes.insert(node.uid, node);
            }
        }
        anyhow::ensure!(nodes.contains_key(&TITLE_ROOT), "TitleScreenRoot missing from h04_fron");
        anyhow::ensure!(nodes.contains_key(&MAIN_MENU_PC), "MainMenu_PC missing from h04_fron");
        Ok(Self { nodes })
    }

    /// Native H04 root Script Entity poses, one per serialized command
    /// instance. A duplicate resource UID never collapses distinct tracks.
    /// Uses render::script::controller_transform; original keyframes are read
    /// from H04, but exact native easing is not yet fully resolved.
    /// Only local TRS is decoded: parent/world and HUD projection remain open.
    pub(super) fn entity_poses(&self, uid: u32, seconds: f64) -> Vec<FrontendEntityPose> {
        let Some(node) = self.nodes.get(&uid) else { return Vec::new(); };
        let Some(script) = node.native_source.as_deref() else { return Vec::new(); };
        if !seconds.is_finite() || seconds < 0.0 || seconds > f32::MAX as f64 {
            return Vec::new();
        }
        let frame = (seconds * f64::from(node.frames_per_second)) as f32;
        if frame >= node.duration_frames { return Vec::new(); }
        let mut poses = Vec::new();
        for (command_index, command) in script.commands.iter().enumerate() {
            let UXGeoScriptCommandData::Entity {hashcode: entity_uid, ..} = &command.data
                else { continue; };
            let start = f32::from(command.start);
            let end = start + f32::from(command.length);
            if frame < start || frame >= end { continue; }
            let controller = script.controllers.get(command.controller_header_index as usize);
            let (position, rotation, scale) =
                crate::render::script::controller_transform(script, command, controller, seconds as f32);
            if !(position.is_finite() && rotation.is_finite() && scale.is_finite()) {
                continue;
            }
            poses.push(FrontendEntityPose {
                command_index, entity_uid: *entity_uid,
                start_frame: start, end_frame: end,
                position: position.to_array(),
                rotation: rotation.to_array(),
                scale: scale.to_array(),
            });
        }
        poses
    }

    /// The first two distinct activation frames of native HUD Entities.
    /// Used only to time provisional entrance art while original transform
    /// controllers / 3D backdrop are not yet connected to Game Mode.
    pub(super) fn entrance_progress(&self, uid: u32, elapsed_seconds: f64) -> Option<f32> {
        let node = self.nodes.get(&uid)?;
        if !elapsed_seconds.is_finite() || elapsed_seconds < 0.0 || node.frames_per_second <= 0.0 {
            return None;
        }
        let first = node.commands.iter()
            .filter(|c| c.kind == FrontendScriptKind::Entity)
            .map(|c| c.start_frame)
            .reduce(f32::min)?;
        let second = node.commands.iter()
            .filter(|c| c.kind == FrontendScriptKind::Entity && c.start_frame > first)
            .map(|c| c.start_frame)
            .reduce(f32::min)?;
        let position = ((elapsed_seconds * f64::from(node.frames_per_second)
            - f64::from(first)) / f64::from(second - first)).clamp(0.0, 1.0) as f32;
        Some(position * position * (3.0 - 2.0 * position))
    }

    /// Normalized vertical offset from paired native H04 MainMenu_PC Entity
    /// position keys. 1.0 is the initial below-rest pose, 0.0 is the
    /// *shipped* final pose, and negative values reproduce the overshoot.
    /// Only converting those world units to screen pixels remains provisional.
    pub(super) fn main_menu_chrome_motion(&self, uid: u32, seconds: f64) -> Option<f32> {
        let node = self.nodes.get(&uid)?;
        let keys = node.chrome_y_keys.as_deref()?;
        if !seconds.is_finite() || seconds < 0.0 || node.frames_per_second <= 0.0 {
            return None;
        }
        let frame = (seconds * f64::from(node.frames_per_second)) as f32;
        let first = keys.first()?.1;
        let rest = keys.last()?.1;
        let current = sample_chrome_key_y(keys, frame)?;
        Some(((rest - current) / (rest - first)).clamp(-1.0, 1.5))
    }

    /// Emit Event start edges crossed during one frontend clock interval.
    /// Distinct SubScript instances retain their source identity and absolute
    /// times. The collector never performs undocumented native side effects.
    pub(super) fn events_started_between(
        &self, root: u32, previous: f64, current: f64,
    ) -> Vec<FrontendScheduledEvent> {
        let mut events = Vec::new();
        if !previous.is_finite() || !current.is_finite()
            || current < 0.0 || current < previous { return events; }
        self.collect_event_edges(root, 0.0, f64::INFINITY, previous, current,
            &mut Vec::new(), &mut events);
        events.sort_by(|a, b| a.at_seconds.total_cmp(&b.at_seconds));
        events
    }

    fn collect_event_edges(
        &self, uid: u32, origin: f64, clip_end: f64,
        previous: f64, current: f64,
        ancestry: &mut Vec<u32>, events: &mut Vec<FrontendScheduledEvent>,
    ) {
        if ancestry.len() >= 24 || ancestry.contains(&uid) || events.len() >= 128 { return; }
        let Some(node) = self.nodes.get(&uid) else { return; };
        if node.frames_per_second <= 0.0 { return; }
        let fps = f64::from(node.frames_per_second);
        let valid_end = clip_end.min(origin + f64::from(node.duration_frames) / fps);
        ancestry.push(uid);
        for (index, command) in node.commands.iter().enumerate() {
            if events.len() >= 128 { break; }
            let start = origin + f64::from(command.start_frame) / fps;
            let end = (origin + f64::from(command.end_frame) / fps).min(valid_end);
            if start >= valid_end || start >= end { continue; }
            if command.kind == FrontendScriptKind::Event {
                if let Some(event_type) = command.event_type {
                    if start > previous && start <= current {
                        events.push(FrontendScheduledEvent {
                            source_script_uid: uid, command_index: index,
                            event_type, presentation_marker: command.presentation_marker,
                            presentation_payload_scalar: command.presentation_payload_scalar,
                            at_seconds: start,
                        });
                    }
                }
            } else if command.kind == FrontendScriptKind::SubScript
                && start <= current && end > previous
            {
                if let Some(child) = command.reference_uid {
                    self.collect_event_edges(child, start, end, previous, current, ancestry, events);
                }
            }
        }
        ancestry.pop();
    }

    /// Marker descriptors recovered from shipped H04 opcode-11 events. This
    /// includes all native 0x16000008 records, even non-clickable sentinels.
    /// marker descriptor contains a runtime record index, NOT a script frame.
    pub(super) fn marker_evidence(&self, script_uid: u32) -> Vec<super::frontend_hud_dispatch::HudMarkerDescriptor> {
        let Some(node) = self.nodes.get(&script_uid) else { return Vec::new(); };
        node.commands.iter().filter(|command| command.event_type == Some(FRONTEND_MARKER_EVENT))
            .enumerate().filter_map(|(runtime_record_index, command)| {
            Some(super::frontend_hud_dispatch::HudMarkerDescriptor {
                uid: command.presentation_marker?,
                event_frame: command.start_frame,
                fps: node.frames_per_second,
                runtime_record_index,
                payload_scalar: command.presentation_payload_scalar?,
            })
        }).collect()
    }

    pub(super) fn contains(&self, uid: u32) -> bool { self.nodes.contains_key(&uid) }

    pub(super) fn node(&self, uid: u32) -> Option<&FrontendScriptNode> {
        self.nodes.get(&uid)
    }

    pub(super) fn sample(&self, root: u32, elapsed_seconds: f64) -> FrontendSceneFrame {
        let mut out = FrontendSceneFrame::default();
        if !elapsed_seconds.is_finite() || elapsed_seconds < 0.0 { return out; }
        let mut ancestry = Vec::new();
        self.sample_inner(root, elapsed_seconds, &mut ancestry, &mut out);
        out
    }

    fn sample_inner(&self, uid: u32, time_seconds: f64,
                    ancestry: &mut Vec<u32>, output: &mut FrontendSceneFrame) {
        if ancestry.contains(&uid) {
            output.cyclic_subscripts.push(uid);
            return;
        }
        if ancestry.len() >= 24 { output.cyclic_subscripts.push(uid); return; }
        let Some(node) = self.nodes.get(&uid) else {
            output.unresolved_subscripts.push(uid);
            return;
        };
        let frame = time_seconds * f64::from(node.frames_per_second);
        if frame >= f64::from(node.duration_frames) { return; }
        ancestry.push(uid);
        for (index, cmd) in node.commands.iter().enumerate() {
            if frame < f64::from(cmd.start_frame) || frame >= f64::from(cmd.end_frame) { continue; }
            output.commands.push(FrontendActiveCommand {
                source_script_uid: uid, command_index: index, kind: cmd.kind,
                resource_uid: cmd.reference_uid,
                controller_index: cmd.controller_index, event_type: cmd.event_type,
                presentation_marker: cmd.presentation_marker,
                presentation_payload_scalar: cmd.presentation_payload_scalar,
            });
            if cmd.kind == FrontendScriptKind::SubScript {
                if let Some(child) = cmd.reference_uid {
                    let child_seconds = time_seconds -
                        f64::from(cmd.start_frame) / f64::from(node.frames_per_second);
                    self.sample_inner(child, child_seconds, ancestry, output);
                }
            }
        }
        ancestry.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs::File, io::BufReader};
    use eurochef_edb::versions::Platform;

    #[test]
    fn native_frontend_event_marker_reads_independent_event_and_payload_scalars() {
        let mut data = [0u8; 20];
        for (index, value) in [4u32, 4, 3, 0x4600_0001, 0].iter().enumerate() {
            data[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        assert_eq!(native_frontend_marker(FRONTEND_MARKER_EVENT, &data),
            Some(NativeFrontendMarker { uid: 0x4600_0001, payload_scalar: 0.0 }));
        // Source H04 Event.start can be frame 56; it is NOT the target frame.
        data[16..20].copy_from_slice(&12.75f32.to_le_bytes());
        assert_eq!(native_frontend_marker(FRONTEND_MARKER_EVENT, &data),
            Some(NativeFrontendMarker { uid: 0x4600_0001, payload_scalar: 12.75 }));
        assert_eq!(native_frontend_marker(0x1600_0007, &data), None);
        assert_eq!(native_frontend_marker(FRONTEND_MARKER_EVENT, &data[..16]), None);
        data[8] = 9;
        assert_eq!(native_frontend_marker(FRONTEND_MARKER_EVENT, &data), None);
        data[8] = 3;
        data[15] = 0x45;
        assert_eq!(native_frontend_marker(FRONTEND_MARKER_EVENT, &data), None);
        data[15] = 0x46;
        data[16..20].copy_from_slice(&f32::NAN.to_le_bytes());
        assert_eq!(native_frontend_marker(FRONTEND_MARKER_EVENT, &data), None);
    }

    #[test]
    fn native_chrome_y_sampler_preserves_overshoot_and_rest() {
        let keys = [(0.0, -32.31428), (16.0, -17.623365),
            (36.0, -3.6433), (39.0, -3.6433), (54.0, -12.729241)];
        assert_eq!(sample_chrome_key_y(&keys, 0.0), Some(-32.31428));
        assert_eq!(sample_chrome_key_y(&keys, 16.0), Some(-17.623365));
        assert_eq!(sample_chrome_key_y(&keys, 36.0), Some(-3.6433));
        assert_eq!(sample_chrome_key_y(&keys, 54.0), Some(-12.729241));
        assert_eq!(sample_chrome_key_y(&keys, 90.0), Some(-12.729241));
        let midpoint = sample_chrome_key_y(&keys, 8.0).unwrap();
        assert!(midpoint > -32.31428 && midpoint < -17.623365);
    }

    #[test]
    fn timing_and_local_subscripts_are_bounded_and_isolated() {
        let mut cat = FrontendScriptCatalog::default();
        cat.nodes.insert(TITLE_ROOT, FrontendScriptNode {
            uid: TITLE_ROOT, duration_frames: 120.0, frames_per_second: 60.0, chrome_y_keys: None, native_source: None,
            commands: vec![
                FrontendScriptCommand {
                    kind: FrontendScriptKind::Entity, reference_uid: Some(0x8200_0001),
                    start_frame: 10.0, end_frame: 60.0,
                    controller_index: 3, event_type: None, presentation_marker: None, presentation_payload_scalar: None,
                },
                FrontendScriptCommand {
                    kind: FrontendScriptKind::SubScript, reference_uid: Some(MAIN_MENU_PC),
                    start_frame: 30.0, end_frame: 90.0,
                    controller_index: 4, event_type: None, presentation_marker: None, presentation_payload_scalar: None,
                },
            ],
        });
        cat.nodes.insert(MAIN_MENU_PC, FrontendScriptNode {
            uid: MAIN_MENU_PC, duration_frames: 40.0, frames_per_second: 20.0, chrome_y_keys: None, native_source: None,
            commands: vec![FrontendScriptCommand {
                kind: FrontendScriptKind::Event, reference_uid: None,
                event_type: Some(0x1600_0008), start_frame: 5.0, end_frame: 15.0,
                controller_index: 1, presentation_marker: None, presentation_payload_scalar: None,
            }],
        });
        assert!(cat.sample(TITLE_ROOT, 0.0).commands.is_empty());
        let active = cat.sample(TITLE_ROOT, 0.75);
        assert!(active.has_geometry());
        assert!(active.commands.iter().any(|c| c.event_type == Some(0x1600_0008)));
        assert!(!cat.sample(TITLE_ROOT, 3.0).has_geometry());
        assert!(cat.sample(TITLE_ROOT, f64::NAN).commands.is_empty());
        let fired = cat.events_started_between(TITLE_ROOT, 0.70, 0.80);
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].source_script_uid, MAIN_MENU_PC);
        assert_eq!(fired[0].event_type, 0x1600_0008);
        assert!((fired[0].at_seconds - 0.75).abs() < 1e-6);
        assert!(cat.events_started_between(TITLE_ROOT, 0.75, 0.80).is_empty());
        assert!(cat.events_started_between(TITLE_ROOT, 0.9, 0.8).is_empty());
    }

    #[test]
    fn corrupt_self_reference_never_spins() {
        let mut cat = FrontendScriptCatalog::default();
        cat.nodes.insert(TITLE_ROOT, FrontendScriptNode {
            uid: TITLE_ROOT, duration_frames: 200.0, frames_per_second: 60.0, chrome_y_keys: None, native_source: None,
            commands: vec![FrontendScriptCommand {
                kind: FrontendScriptKind::SubScript,
                reference_uid: Some(TITLE_ROOT),
                start_frame: 0.0, end_frame: 200.0,
                controller_index: 0, event_type: None, presentation_marker: None, presentation_payload_scalar: None,
            }],
        });
        let frame = cat.sample(TITLE_ROOT, 0.2);
        assert_eq!(frame.cyclic_subscripts, vec![TITLE_ROOT]);
        assert_eq!(frame.commands.len(), 1);
    }

    #[test]
    fn real_windows_m00_frontend_scene_has_native_resources() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/attachment_game_root/_eurotools_out/extracted_main/robots/binary/_bin_pc/m00_fron.edb");
        if !path.is_file() { return; }
        let file = File::open(&path).unwrap();
        let edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).unwrap();
        let h = &edb.header;
        eprintln!("M00 scene edb={:#010x} maps={} entities={} scripts={} textures={} skins={} animations={}",
            h.hashcode, h.map_list.len(), h.entity_list.len(), h.animscript_list.len(),
            h.texture_list.len(), h.animskin_list.len(), h.anim_list.len());
        assert_eq!(h.hashcode, 0x0100_000D);
        assert_eq!((h.map_list.len(), h.entity_list.len(), h.animscript_list.len()), (1, 5, 4));
        let scripts = UXGeoScript::read_all(&mut EdbFile::new(
            Box::new(BufReader::new(File::open(path).unwrap())), Platform::Pc).unwrap()).unwrap();
        for script in &scripts {
            eprintln!("M00 script={:#010x} length={} fps={} types={:?}",
                script.hashcode, script.length, script.timeline_framerate(), script.command_type_counts());
            for cmd in &script.commands { eprintln!("  M00 command {:?}", cmd.data); }
        }
    }

    #[test]
    fn shipped_h04_marker_frames_are_start_not_cmd_frame() {
        use std::io::{Seek, SeekFrom};
        use eurochef_edb::{binrw::BinReaderExt, script::EXGeoAnimScript};
        use eurochef_edb::versions::Platform;
        use std::{fs::File, io::BufReader};
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/attachment_game_root/_eurotools_out/extracted_main/robots/binary/_bin_pc/h04_fron.edb");
        if !path.is_file() { return; }
        let mut edb = EdbFile::new(Box::new(BufReader::new(File::open(&path).unwrap())), Platform::Pc).unwrap();
        let entries = edb.header.animscript_list.clone();
        let expected = [
            (MAIN_MENU_PC, 0x4600_0001, 56i16),
            (NEW_GAME_MENU, 0x4600_000c, 25i16),
            (LOAD_GAME_MENU, 0x4600_000d, 24i16),
            (OPTIONS_PC, 0x4600_0022, 75i16),
        ];
        for (script_uid, marker_uid, start_frame) in expected {
            let entry = entries.iter().find(|e| e.hashcode == script_uid).unwrap();
            edb.seek(SeekFrom::Start(entry.address as u64)).unwrap();
            let parsed = edb.read_type::<EXGeoAnimScript>(edb.endian).unwrap();
            assert_eq!(parsed.frame_rate, 30.0);
            let cmd = parsed.commands.iter().find(|cmd| cmd.cmd == 11 &&
                cmd.data.len() >= 24 &&
                u32::from_le_bytes(cmd.data[0..4].try_into().unwrap()) == FRONTEND_MARKER_EVENT &&
                u32::from_le_bytes(cmd.data[16..20].try_into().unwrap()) == marker_uid
            ).unwrap();
            assert_eq!(cmd.start, start_frame);
            // Native +0x30 comes from raw command+0x20 = last event float.
            assert_eq!(f32::from_le_bytes(cmd.data[20..24].try_into().unwrap()), 0.0);
            assert!(cmd.cmd_frame < 0);
            assert_ne!(cmd.cmd_frame, cmd.start, "cmd_frame is not the timeline start");
        }
    }

    #[test]
    fn real_windows_h04_scripts_contain_expected_pc_frontend() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/attachment_game_root/_eurotools_out/extracted_main/robots/binary/_bin_pc/h04_fron.edb");
        if !path.is_file() { return; }
        let file = File::open(path).unwrap();
        let mut edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).unwrap();
        let cat = FrontendScriptCatalog::from_original_edb(&mut edb).unwrap();
        let main = cat.node(MAIN_MENU_PC).unwrap();
        let root = cat.node(TITLE_ROOT).unwrap();
        assert!(main.commands.len() >= 30);
        assert!(root.commands.len() >= 10);
        assert!(cat.sample(MAIN_MENU_PC, 0.0).has_geometry());
        // Original H04 controller keys: world Y ascends, overshoots and
        // settles by frame 54. The host's pixels/unit remains provisional.
        let native = |frame: f64| cat.main_menu_chrome_motion(MAIN_MENU_PC, frame / 30.0).unwrap();
        assert!((native(0.0) - 1.0).abs() < 1e-4);
        assert!(native(16.0) > 0.0 && native(16.0) < 1.0);
        assert!(native(36.0) < -0.40 && native(36.0) > -0.55);
        assert!((native(54.0)).abs() < 1e-4);
        assert!((native(90.0)).abs() < 1e-4);
        assert!(cat.main_menu_chrome_motion(OPTIONS_PC, 0.0).is_none());
        // In the shipped H04, identical Entity UID 0x82000031 appears
        // as *independent command instances*. Their per-instance transforms
        // cannot be collapsed into one global chrome position.
        let initial = cat.entity_poses(MAIN_MENU_PC, 0.0);
        let pair: Vec<_> = initial.iter().filter(|p| p.entity_uid == 0x8200_0031
            && matches!(p.command_index, 1 | 2)).collect();
        assert_eq!(pair.len(), 2);
        assert_ne!(pair[0].command_index, pair[1].command_index);
        assert!((pair[0].position[0] - pair[1].position[0]).abs() > 0.01);
        assert!((pair[0].position[1] - pair[1].position[1]).abs() < 0.05);
        let moving = cat.entity_poses(MAIN_MENU_PC, 30.0 / 30.0);
        let left = moving.iter().find(|p| p.command_index == 1).unwrap();
        assert!((left.position[1] - pair[0].position[1]).abs() > 1.0);
        assert!(cat.entity_poses(MAIN_MENU_PC, -0.1).is_empty());
        assert!(cat.entity_poses(MAIN_MENU_PC, f64::NAN).is_empty());

        assert!(cat.contains(NEW_GAME_MENU));
        assert!(cat.contains(OPTIONS_PC));
        for (script, marker) in [
            (MAIN_MENU_PC, 0x4600_0001),
            (NEW_GAME_MENU, 0x4600_000c),
            (LOAD_GAME_MENU, 0x4600_000d),
            (OPTIONS_PC, 0x4600_0022),
        ] {
            assert!(cat.node(script).unwrap().commands.iter()
                .any(|command| command.presentation_marker == Some(marker)),
                "Shipped H04 script {script:#010x} must contain its original 0x46 marker");
        }
        for (uid, marker, frame) in [
            (MAIN_MENU_PC, 0x4600_0001, 56.0),
            (NEW_GAME_MENU, 0x4600_000c, 25.0),
            (LOAD_GAME_MENU, 0x4600_000d, 24.0),
            (OPTIONS_PC, 0x4600_0022, 75.0),
        ] {
            assert!(cat.marker_evidence(uid).iter().any(|entry|
                entry.uid == marker && entry.event_frame == frame &&
                    entry.payload_scalar == 0.0 && entry.runtime_record_index == 1 && entry.fps == 30.0),
                "Original H04 marker ordinal/start and metadata missing for {uid:#010x}");
        }
        // x32dbg 2026-10-14 (windowed Robots.exe) confirms 0x00440640
        // SELECT 0 -> JumpToMarker(record 1), SELECT 1 -> record 2,
        // SELECT 2 -> record 3. An unselectable runtime record at
        // MainMenu frame 39 occupies ordinal 0.
        let shipped_main_markers = cat.marker_evidence(MAIN_MENU_PC);
        for (uid, record_index, start_frame) in [
            (0x4600_0001, 1, 56.0), (0x4600_0002, 2, 66.0),
            (0x4600_0003, 3, 76.0), (0x4600_0004, 4, 86.0),
            (0x4600_003c, 5, 96.0),
        ] {
            let marker = shipped_main_markers.iter().find(|entry| entry.uid == uid).unwrap();
            assert_eq!((marker.runtime_record_index, marker.event_frame), (record_index, start_frame));
        }
        let main_marker_events = cat.events_started_between(MAIN_MENU_PC, 0.0, 3.0);
        assert_eq!(cat.marker_evidence(MAIN_MENU_PC)
            .iter().find(|entry| entry.uid == 0x4600_0001).unwrap().runtime_record_index, 1);
        assert!(main_marker_events.iter().any(|event|
            event.presentation_marker == Some(0x4600_0001)
                && event.presentation_payload_scalar == Some(0.0)
                && (event.at_seconds - 56.0 / 30.0).abs() < 1e-6
        ), "H04 MainMenu 0x46 marker must fire at original frame 56");
        // Exact shipped Windows H04 Entity entry frames (30 FPS).
        assert_eq!(cat.entrance_progress(TITLE_ROOT, 0.0), Some(0.0));
        assert!((cat.entrance_progress(TITLE_ROOT, 95.0 / 30.0).unwrap() - 0.5).abs() < 0.001);
        assert_eq!(cat.entrance_progress(TITLE_ROOT, 190.0 / 30.0), Some(1.0));
        assert_eq!(cat.entrance_progress(MAIN_MENU_PC, 0.0), Some(0.0));
        assert!((cat.entrance_progress(MAIN_MENU_PC, 20.0 / 30.0).unwrap() - 0.5).abs() < 0.001);
        assert_eq!(cat.entrance_progress(MAIN_MENU_PC, 40.0 / 30.0), Some(1.0));
        let events = cat.events_started_between(MAIN_MENU_PC, -1.0 / 60.0, 0.4);
        assert!(events.iter().any(|event| (event.at_seconds - 10.0 / 30.0).abs() < 1e-6),
            "Native Main Menu H04 event edges must fire at shipped frame 10");
    }
}
