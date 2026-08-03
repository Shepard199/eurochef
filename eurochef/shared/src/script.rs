use std::{io::Seek, ops::Range};

use serde::Serialize;
use tracing::debug;

use eurochef_edb::{
    binrw::{BinReaderExt, Endian},
    edb::EdbFile,
    error::Result,
    header::EXGeoAnimScriptHeader,
    script::{EXGeoAnimScript, EXGeoAnimScriptControllerChannels, EXGeoAnimScriptControllerHeader},
    Hashcode,
};

#[derive(Debug, Clone, Serialize)]
pub enum UXGeoScriptCommandData {
    Entity {
        hashcode: Hashcode,
        file: Hashcode,
    },
    Animation {
        skin_file: Hashcode,
        skin_hashcode: Hashcode,
        anim_file: Hashcode,
        anim_hashcode: Hashcode,
    },
    Sound {
        hashcode: Hashcode,
    },
    Particle {
        hashcode: Hashcode,
        file: Hashcode,
    },
    Event {
        event_type: Hashcode,
        data: Vec<u8>,
    },
    SubScript {
        hashcode: Hashcode,
        file: Hashcode,
    },
    Unknown {
        cmd: u8,
        data: Vec<u8>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct UXGeoScriptCommand {
    /// Native AnimScript opcode.
    pub opcode: u8,
    pub start: i16,
    pub length: u16,
    /// Direct index into the flat serialized transform-controller table.
    pub controller_header_index: u16,
    /// Runtime controller-record index selected by the command.
    pub controller_index: u8,
    /// Parent/linked runtime controller-record index; 0xFF means none.
    pub parent_controller_index: u8,

    pub data: UXGeoScriptCommandData,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RobotsScriptPayloadDiagnostic {
    /// Opcode 7 reaches 0x004FA0DD, creates an HT_Light-backed runtime object,
    /// allocates a 0x44-byte dynamic-light record through 0x00522FFC and
    /// initializes that record through 0x00522E29/0x00522EB4.
    DynamicLight {
        /// Payload +0x00. Preserved for provenance; 0x004FA0DD does not read it.
        raw_word_0: u32,
        /// Payload +0x04 low word. Zero selects the no-orientation initializer;
        /// non-zero selects the identity-orientation initializer.
        orientation_selector: u16,
        /// Payload +0x06..+0x07. Preserved; the proven creator does not read it.
        raw_orientation_tail: u16,
        /// Payload +0x08 low byte. Bit 1 is copied to runtime light byte +0x42.
        mode_byte: u8,
        /// Payload +0x09..+0x0B. Preserved; the proven creator does not read it.
        raw_mode_tail: [u8; 3],
        /// Payload +0x0C. 0x00522E29/0x00522EB4 expand its four bytes to
        /// normalized float channels at dynamic-light +0x20..+0x2C.
        packed_color: u32,
        /// Payload +0x10, copied to dynamic-light +0x34.
        runtime_scalar_34: f32,
        /// Payload +0x14, copied to dynamic-light +0x38.
        runtime_scalar_38: f32,
    },
    /// Opcode 10 reaches 0x004FA262 and EXItemAnimator_Camera initializer
    /// 0x00567A14. Names intentionally use native object offsets because the
    /// projection semantics of the four values are not yet symbol-proven.
    Camera {
        /// Payload +0x00. Preserved; 0x00567A14 does not read it.
        raw_word_0: u32,
        /// Payload +0x04. Preserved; 0x00567A14 does not read it.
        raw_word_1: u32,
        /// Payload +0x08 -> EXItemAnimator_Camera +0x114.
        object_114: f32,
        /// Payload +0x0C -> EXItemAnimator_Camera +0x118.
        object_118: f32,
        /// Payload +0x10 -> EXItemAnimator_Camera +0x110.
        object_110: f32,
        /// Payload +0x14 -> EXItemAnimator_Camera +0x11C.
        object_11c: f32,
    },
    /// Opcode 13 reaches subtype 8, creator 0x004FA1B6 and
    /// EXItemAnimator_Collision initializer 0x005677EC.
    Collision {
        /// Payload +0x00 -> EXItemAnimator_Collision +0x140.
        object_140: u32,
        /// Payload +0x04 low byte -> EXItemAnimator_Collision +0x13F.
        object_13f_mode: u8,
        /// Payload +0x05..+0x07. Preserved; 0x005677EC does not read it.
        raw_mode_tail: [u8; 3],
        /// Payload +0x08 -> EXItemAnimator_Collision +0x130.
        object_130: f32,
        /// Payload +0x0C -> EXItemAnimator_Collision +0x134.
        object_134: f32,
        /// Payload +0x10 -> EXItemAnimator_Collision +0x138.
        object_138: f32,
    },
}

impl RobotsScriptPayloadDiagnostic {
    pub fn native_summary(&self) -> String {
        match self {
            Self::DynamicLight {
                raw_word_0,
                orientation_selector,
                raw_orientation_tail,
                mode_byte,
                raw_mode_tail,
                packed_color,
                runtime_scalar_34,
                runtime_scalar_38,
            } => format!(
                "native 0x004FA0DD -> dynamic light: raw+00=0x{raw_word_0:08X}, orientation_selector={orientation_selector}, raw+06=0x{raw_orientation_tail:04X}, mode=0x{mode_byte:02X} (bit1={}), raw+09={:02X}{:02X}{:02X}, packed_color=0x{packed_color:08X}, +0x34={runtime_scalar_34:.9}, +0x38={runtime_scalar_38:.9}",
                (mode_byte & 2) != 0,
                raw_mode_tail[0],
                raw_mode_tail[1],
                raw_mode_tail[2],
            ),
            Self::Camera {
                raw_word_0,
                raw_word_1,
                object_114,
                object_118,
                object_110,
                object_11c,
            } => format!(
                "native 0x004FA262 -> EXItemAnimator_Camera: raw+00=0x{raw_word_0:08X}, raw+04=0x{raw_word_1:08X}, object+0x114={object_114:.9}, +0x118={object_118:.9}, +0x110={object_110:.9}, +0x11C={object_11c:.9}"
            ),
            Self::Collision {
                object_140,
                object_13f_mode,
                raw_mode_tail,
                object_130,
                object_134,
                object_138,
            } => format!(
                "native 0x004FA1B6 -> EXItemAnimator_Collision: object+0x140=0x{object_140:08X}, +0x13F=0x{object_13f_mode:02X}, raw+05={:02X}{:02X}{:02X}, +0x130={object_130:.9}, +0x134={object_134:.9}, +0x138={object_138:.9}",
                raw_mode_tail[0],
                raw_mode_tail[1],
                raw_mode_tail[2],
            ),
        }
    }
}

impl UXGeoScriptCommand {
    pub fn range(&self) -> Range<isize> {
        let start = self.start as isize;
        start..start.saturating_add(self.length as isize)
    }

    pub fn uses_controller_header(&self) -> bool {
        opcode_uses_controller_header(self.opcode)
    }
}

/// Native dispatcher cases that reach 0x004F92FF and therefore interpret the
/// command u16 at +0x08 as a flat thread_controllers pointer-table index.
pub fn opcode_uses_controller_header(opcode: u8) -> bool {
    matches!(opcode, 1..=10 | 13 | 15)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsScriptCommandRole {
    pub name: &'static str,
    pub family: &'static str,
    pub runtime_subtype: Option<u8>,
    pub classified: bool,
}

pub fn robots_script_command_role(opcode: u8, payload_size: usize) -> RobotsScriptCommandRole {
    let (name, family, runtime_subtype, classified) = match opcode {
        1 => ("Controller Init", "control", None, true),
        2 => ("Animation", "geometry", Some(0), true),
        3 => ("Entity", "geometry", Some(2), true),
        4 => ("SubScript", "geometry", Some(5), true),
        5 if payload_size >= 24 => ("Sound", "sound", Some(6), true),
        5 => ("Malformed Sound", "malformed", Some(6), false),
        6 => ("Particle", "particle", Some(4), true),
        7 if payload_size >= 24 => ("Dynamic Light Animator", "geometry", Some(3), true),
        7 => (
            "Malformed Dynamic Light Animator",
            "malformed",
            Some(3),
            false,
        ),
        8 => ("Reserved Animator 8", "control", Some(7), true),
        9 => ("Reserved Animator 9", "control", Some(7), true),
        10 => ("Camera", "geometry", Some(1), true),
        11 => ("Event", "event", None, true),
        12 => ("External Callback", "control", None, true),
        13 => ("Collision", "geometry", Some(8), true),
        14 => ("Force Feedback", "effect", Some(10), true),
        15 => ("Resource-backed Animator 15", "geometry", Some(9), true),
        16 => ("Loop", "control", None, true),
        17 => ("Controller Fan-out", "control", None, true),
        18 => ("Terminator", "terminator", None, true),
        _ => ("Unknown", "unknown", None, false),
    };
    RobotsScriptCommandRole {
        name,
        family,
        runtime_subtype,
        classified,
    }
}

/// Decodes only payload lanes whose native Robots.exe consumers are proven.
///
/// The shipped Robots PC EDB corpus is little-endian. Raw command bytes remain
/// stored in `UXGeoScriptCommandData::Unknown`; this diagnostic is an overlay,
/// not a lossy replacement for serialized provenance.
pub fn robots_script_payload_diagnostic(
    opcode: u8,
    data: &[u8],
) -> Option<RobotsScriptPayloadDiagnostic> {
    let word = |offset: usize| {
        data.get(offset..offset + 4)
            .and_then(|bytes| bytes.try_into().ok())
            .map(u32::from_le_bytes)
    };
    let float = |offset: usize| word(offset).map(f32::from_bits);

    match opcode {
        7 if data.len() >= 24 => Some(RobotsScriptPayloadDiagnostic::DynamicLight {
            raw_word_0: word(0)?,
            orientation_selector: u16::from_le_bytes(data.get(4..6)?.try_into().ok()?),
            raw_orientation_tail: u16::from_le_bytes(data.get(6..8)?.try_into().ok()?),
            mode_byte: *data.get(8)?,
            raw_mode_tail: data.get(9..12)?.try_into().ok()?,
            packed_color: word(12)?,
            runtime_scalar_34: float(16)?,
            runtime_scalar_38: float(20)?,
        }),
        10 if data.len() >= 24 => Some(RobotsScriptPayloadDiagnostic::Camera {
            raw_word_0: word(0)?,
            raw_word_1: word(4)?,
            object_114: float(8)?,
            object_118: float(12)?,
            object_110: float(16)?,
            object_11c: float(20)?,
        }),
        13 if data.len() >= 20 => Some(RobotsScriptPayloadDiagnostic::Collision {
            object_140: word(0)?,
            object_13f_mode: *data.get(4)?,
            raw_mode_tail: data.get(5..8)?.try_into().ok()?,
            object_130: float(8)?,
            object_134: float(12)?,
            object_138: float(16)?,
        }),
        _ => None,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UXGeoScript {
    pub hashcode: Hashcode,
    pub framerate: f32,
    pub length: u32,
    pub num_threads: u32,

    pub commands: Vec<UXGeoScriptCommand>,
    /// Serialized header field at EXGeoAnimScript+0x3C. This is not the logical
    /// pointer-table length because special/identity slots are not counted.
    pub serialized_controller_count: u16,
    /// Raw 4-byte thread_info records as two u16 lanes.
    pub controller_record_metadata: Vec<[u16; 2]>,
    /// Flat logical pointer table addressed by command.controller_header_index.
    pub controllers: Vec<EXGeoAnimScriptControllerHeader>,
    /// Exact flat controller indices referenced by each runtime record.
    pub controller_group_indices: Vec<Vec<u16>>,
    /// Command-proven controller headers grouped by runtime record.
    pub controller_groups: Vec<Vec<EXGeoAnimScriptControllerHeader>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct ScriptCommandTypeCounts {
    pub entities: usize,
    pub animations: usize,
    pub subscripts: usize,
    pub particles: usize,
    pub sounds: usize,
    pub events: usize,
    pub unknown: usize,
}

impl UXGeoScript {
    /// Returns the serialized AnimScript frame rate when it is usable.
    ///
    /// Robots stores script length in frames and frame rate at EXGeoAnimScript+0x0C.
    /// The native controller advances that timeline from the fixed 60 Hz engine step,
    /// so one GUI second maps to exactly `serialized_fps` script frames at speed 1.0.
    pub fn timeline_framerate(&self) -> f32 {
        if self.framerate.is_finite() && self.framerate > f32::EPSILON {
            self.framerate
        } else {
            30.0
        }
    }

    pub fn frame_at_time(&self, seconds: f32) -> f32 {
        seconds.max(0.0) * self.timeline_framerate()
    }

    pub fn time_at_frame(&self, frame: f32) -> f32 {
        frame.max(0.0) / self.timeline_framerate()
    }

    pub fn duration_seconds(&self) -> f32 {
        self.time_at_frame(self.length as f32)
    }

    /// First frame that can produce renderable geometry in the current GUI.
    /// Particle, sound and event commands are intentionally excluded because
    /// they do not currently enqueue EntityRenderer geometry.
    pub fn first_geometry_frame(&self) -> Option<i16> {
        self.commands
            .iter()
            .filter(|command| {
                matches!(
                    &command.data,
                    UXGeoScriptCommandData::Entity { .. }
                        | UXGeoScriptCommandData::Animation { .. }
                        | UXGeoScriptCommandData::SubScript { .. }
                )
            })
            .map(|command| command.start)
            .min()
    }

    /// First frame containing any visual command, including particle-only scripts.
    pub fn first_visual_frame(&self) -> Option<i16> {
        self.commands
            .iter()
            .filter(|command| {
                matches!(
                    &command.data,
                    UXGeoScriptCommandData::Entity { .. }
                        | UXGeoScriptCommandData::Animation { .. }
                        | UXGeoScriptCommandData::SubScript { .. }
                        | UXGeoScriptCommandData::Particle { .. }
                )
            })
            .map(|command| command.start)
            .min()
    }

    pub fn command_type_counts(&self) -> ScriptCommandTypeCounts {
        let mut counts = ScriptCommandTypeCounts::default();
        for command in &self.commands {
            match &command.data {
                UXGeoScriptCommandData::Entity { .. } => counts.entities += 1,
                UXGeoScriptCommandData::Animation { .. } => counts.animations += 1,
                UXGeoScriptCommandData::SubScript { .. } => counts.subscripts += 1,
                UXGeoScriptCommandData::Particle { .. } => counts.particles += 1,
                UXGeoScriptCommandData::Sound { .. } => counts.sounds += 1,
                UXGeoScriptCommandData::Event { .. } => counts.events += 1,
                UXGeoScriptCommandData::Unknown { .. } => counts.unknown += 1,
            }
        }
        counts
    }

    pub fn read_all(edb: &mut EdbFile) -> Result<Vec<UXGeoScript>> {
        let header = edb.header.clone();
        let mut res = vec![];
        for c in &header.animscript_list {
            res.push(Self::read(c, edb)?);
        }

        Ok(res)
    }

    /// Read specific hashcodes
    pub fn read_hashcodes(edb: &mut EdbFile, hashcodes: &[Hashcode]) -> Result<Vec<UXGeoScript>> {
        let header = edb.header.clone();
        let mut res = vec![];
        for c in header
            .animscript_list
            .iter()
            .filter(|c| hashcodes.contains(&c.hashcode))
        {
            res.push(Self::read(c, edb)?);
        }

        Ok(res)
    }

    pub fn read(header: &EXGeoAnimScriptHeader, edb: &mut EdbFile) -> Result<UXGeoScript> {
        edb.seek(std::io::SeekFrom::Start(header.address as u64))?;
        let script = edb.read_type::<EXGeoAnimScript>(edb.endian)?;
        let logical_controller_slot_count = script
            .commands
            .iter()
            .filter(|command| {
                opcode_uses_controller_header(command.cmd)
                    && command.controller_header_index != u16::MAX
            })
            .map(|command| command.controller_header_index as usize + 1)
            .max()
            .unwrap_or(0);

        let mut commands = vec![];
        for c in script.commands {
            let data = match c.cmd {
                2 if c.data.len() >= 24 => UXGeoScriptCommandData::Animation {
                    skin_file: u32_from_index(&c.data, edb.endian, 8)?,
                    skin_hashcode: u32_from_index(&c.data, edb.endian, 12)?,
                    anim_file: u32_from_index(&c.data, edb.endian, 16)?,
                    anim_hashcode: u32_from_index(&c.data, edb.endian, 20)?,
                },
                3 if c.data.len() >= 12 => UXGeoScriptCommandData::Entity {
                    hashcode: u32_from_index(&c.data, edb.endian, 8)?,
                    file: u32_from_index(&c.data, edb.endian, 4)?,
                },
                4 if c.data.len() >= 12 => UXGeoScriptCommandData::SubScript {
                    hashcode: u32_from_index(&c.data, edb.endian, 8)?,
                    file: u32_from_index(&c.data, edb.endian, 4)?,
                },
                5 if c.data.len() >= 24 => UXGeoScriptCommandData::Sound {
                    hashcode: u32_from_index(&c.data, edb.endian, 20)?,
                },
                6 if c.data.len() >= 12 => UXGeoScriptCommandData::Particle {
                    hashcode: u32_from_index(&c.data, edb.endian, 8)?,
                    file: u32_from_index(&c.data, edb.endian, 4)?,
                },
                11 if c.data.len() >= 4 => UXGeoScriptCommandData::Event {
                    event_type: u32_from_index(&c.data, edb.endian, 0)?,
                    data: c.data[4..].to_vec(),
                },
                i => UXGeoScriptCommandData::Unknown {
                    cmd: i,
                    data: c.data,
                },
            };

            match &data {
                UXGeoScriptCommandData::Entity { hashcode, file }
                | UXGeoScriptCommandData::Particle { hashcode, file }
                | UXGeoScriptCommandData::SubScript { hashcode, file } => {
                    edb.add_reference(*file, *hashcode)
                }
                UXGeoScriptCommandData::Animation {
                    skin_file,
                    skin_hashcode,
                    anim_file,
                    anim_hashcode,
                } => {
                    edb.add_reference(*skin_file, *skin_hashcode);
                    edb.add_reference(*anim_file, *anim_hashcode);
                }
                _ => {}
            };

            commands.push(UXGeoScriptCommand {
                opcode: c.cmd,
                start: c.start,
                length: c.length,
                controller_header_index: c.controller_header_index,
                controller_index: c.controller_index,
                parent_controller_index: c.parent_controller_index,
                data,
            });
        }

        let pos_saved = edb.stream_position()?;

        // Preserve the two u16 lanes for each runtime record. They describe
        // runtime tail/controller metadata, but they do not bound the logical
        // thread_controllers pointer table because special and identity slots
        // are not counted uniformly. Native commands provide the authoritative
        // flat pointer indices.
        edb.seek(std::io::SeekFrom::Start(
            script.thread_info.offset_absolute(),
        ))?;
        let mut controller_record_metadata = Vec::with_capacity(script._unk8 as usize);
        for _ in 0..script._unk8 {
            controller_record_metadata.push(edb.read_type::<[u16; 2]>(edb.endian)?);
        }

        edb.seek(std::io::SeekFrom::Start(
            script.thread_controllers.offset_absolute(),
        ))?;

        let empty_controller = || EXGeoAnimScriptControllerHeader {
            controller_count: 0,
            channel_count: 0,
            ctrl_mask: 0,
            ctrl_channel_mask: 0,
            channels: EXGeoAnimScriptControllerChannels::default(),
        };

        let mut controllers = Vec::with_capacity(logical_controller_slot_count);
        for controller_slot_index in 0..logical_controller_slot_count {
            let pointer_position = edb.stream_position()?;
            let relative: i32 = edb.read_type(edb.endian)?;
            let controller = if relative == 0 {
                empty_controller()
            } else {
                let target = pointer_position as i64 + relative as i64;
                if target < 0 {
                    debug!(
                        "ANIMSCRIPT_CONTROLLER_NEGATIVE_POINTER script=0x{:08x} slot={} pointer=0x{:x} relative={}",
                        header.hashcode,
                        controller_slot_index,
                        pointer_position,
                        relative
                    );
                    empty_controller()
                } else {
                    let return_position = edb.stream_position()?;
                    edb.seek(std::io::SeekFrom::Start(target as u64))?;
                    let parsed = edb.read_type::<EXGeoAnimScriptControllerHeader>(edb.endian);
                    edb.seek(std::io::SeekFrom::Start(return_position))?;
                    match parsed {
                        Ok(controller) => controller,
                        Err(error) => {
                            debug!(
                                "ANIMSCRIPT_CONTROLLER_PARSE_FALLBACK script=0x{:08x} slot={} error={}",
                                header.hashcode,
                                controller_slot_index,
                                error
                            );
                            empty_controller()
                        }
                    }
                }
            };

            if !controller.channels.pair_14_raw.is_empty() {
                let first = controller.channels.pair_14_raw.first().copied();
                let last = controller.channels.pair_14_raw.last().copied();

                debug!(
                    "ROBOTS_ANIMSCRIPT_4000_SCRIPT_CONTEXT script=0x{:08x} script_addr=0x{:x} length={} frame_rate={:.9} controller_index={} controller_count={} channel_count={} ctrl_mask=0x{:08x} ctrl_channel_mask=0x{:08x} data_abs={} pair_count={} first={} last={}",
                    header.hashcode,
                    header.address,
                    script.length,
                    script.frame_rate,
                    controller_slot_index,
                    controller.controller_count,
                    controller.channel_count,
                    controller.ctrl_mask,
                    controller.ctrl_channel_mask,
                    controller
                        .channels
                        .pair_14_data_abs
                        .map(|value| format!("0x{value:x}"))
                        .unwrap_or_else(|| "none".to_string()),
                    controller.channels.pair_14_raw.len(),
                    first
                        .map(|(lane0, lane1)| format!(
                            "{:08x}:{:08x}:{:.9}:{:.9}",
                            lane0.to_bits(),
                            lane1.to_bits(),
                            lane0,
                            lane1
                        ))
                        .unwrap_or_else(|| "none".to_string()),
                    last
                        .map(|(lane0, lane1)| format!(
                            "{:08x}:{:08x}:{:.9}:{:.9}",
                            lane0.to_bits(),
                            lane1.to_bits(),
                            lane0,
                            lane1
                        ))
                        .unwrap_or_else(|| "none".to_string())
                );
            }

            controllers.push(controller);
        }

        let mut controller_group_indices = vec![Vec::<u16>::new(); script._unk8 as usize];
        for command in &commands {
            if !command.uses_controller_header()
                || command.controller_index == u8::MAX
                || command.controller_header_index == u16::MAX
            {
                continue;
            }

            let Some(record_indices) =
                controller_group_indices.get_mut(command.controller_index as usize)
            else {
                debug!(
                    "ANIMSCRIPT_COMMAND_RECORD_OUT_OF_RANGE script=0x{:08x} opcode={} record={} records={}",
                    header.hashcode,
                    command.opcode,
                    command.controller_index,
                    script._unk8
                );
                continue;
            };

            if command.controller_header_index as usize >= controllers.len() {
                debug!(
                    "ANIMSCRIPT_COMMAND_HEADER_OUT_OF_RANGE script=0x{:08x} opcode={} header={} slots={}",
                    header.hashcode,
                    command.opcode,
                    command.controller_header_index,
                    controllers.len()
                );
                continue;
            }

            if !record_indices.contains(&command.controller_header_index) {
                record_indices.push(command.controller_header_index);
            }
        }

        let controller_groups = controller_group_indices
            .iter()
            .map(|indices| {
                indices
                    .iter()
                    .filter_map(|index| controllers.get(*index as usize).cloned())
                    .collect()
            })
            .collect();

        edb.seek(std::io::SeekFrom::Start(pos_saved))?;

        Ok(UXGeoScript {
            hashcode: header.hashcode,
            framerate: script.frame_rate,
            length: script.length,
            num_threads: script._unk8 as u32,
            commands,
            serialized_controller_count: script.thread_controller_count,
            controller_record_metadata,
            controllers,
            controller_group_indices,
            controller_groups,
        })
    }
}

fn u32_from_index(data: &[u8], endian: Endian, index: usize) -> anyhow::Result<u32> {
    let Some(bytes) = data.get(index..index + 4) else {
        return Ok(0);
    };
    Ok(match endian {
        Endian::Big => u32::from_be_bytes(bytes.try_into()?),
        Endian::Little => u32::from_le_bytes(bytes.try_into()?),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(start: i16, data: UXGeoScriptCommandData) -> UXGeoScriptCommand {
        UXGeoScriptCommand {
            opcode: 0,
            start,
            length: 1,
            controller_header_index: u16::MAX,
            controller_index: u8::MAX,
            parent_controller_index: u8::MAX,
            data,
        }
    }

    fn script(commands: Vec<UXGeoScriptCommand>) -> UXGeoScript {
        UXGeoScript {
            hashcode: 0x8400_0000,
            framerate: 30.0,
            length: 10,
            num_threads: 0,
            commands,
            serialized_controller_count: 0,
            controller_record_metadata: vec![],
            controllers: vec![],
            controller_group_indices: vec![],
            controller_groups: vec![],
        }
    }

    #[test]
    fn first_geometry_frame_skips_particle_and_event_commands() {
        let script = script(vec![
            command(
                0,
                UXGeoScriptCommandData::Particle {
                    hashcode: 0x9100_0000,
                    file: u32::MAX,
                },
            ),
            command(
                1,
                UXGeoScriptCommandData::Event {
                    event_type: 0x1600_0000,
                    data: vec![],
                },
            ),
            command(
                2,
                UXGeoScriptCommandData::Entity {
                    hashcode: 0x8200_0001,
                    file: u32::MAX,
                },
            ),
        ]);

        assert_eq!(script.first_visual_frame(), Some(0));
        assert_eq!(script.first_geometry_frame(), Some(2));
    }

    #[test]
    fn command_type_counts_preserve_non_geometry_scripts() {
        let script = script(vec![
            command(
                0,
                UXGeoScriptCommandData::Particle {
                    hashcode: 0x9100_0000,
                    file: u32::MAX,
                },
            ),
            command(
                0,
                UXGeoScriptCommandData::Sound {
                    hashcode: 0x1af0_0000,
                },
            ),
        ]);

        assert_eq!(
            script.command_type_counts(),
            ScriptCommandTypeCounts {
                particles: 1,
                sounds: 1,
                ..Default::default()
            }
        );
        assert_eq!(script.first_geometry_frame(), None);
    }

    #[test]
    fn animscript_timeline_uses_serialized_frames_per_second() {
        let mut script = script(vec![]);
        script.framerate = 30.0;
        script.length = 90;

        assert_eq!(script.frame_at_time(2.0), 60.0);
        assert_eq!(script.time_at_frame(60.0), 2.0);
        assert_eq!(script.duration_seconds(), 3.0);
    }

    #[test]
    fn robots_native_command_roles_preserve_diagnostic_boundaries() {
        let dynamic_light = robots_script_command_role(7, 24);
        assert_eq!(dynamic_light.name, "Dynamic Light Animator");
        assert_eq!(dynamic_light.family, "geometry");
        assert_eq!(dynamic_light.runtime_subtype, Some(3));
        assert!(dynamic_light.classified);

        let resource_15 = robots_script_command_role(15, 0);
        assert_eq!(resource_15.name, "Resource-backed Animator 15");
        assert_eq!(resource_15.runtime_subtype, Some(9));

        let camera = robots_script_command_role(10, 24);
        assert_eq!(camera.runtime_subtype, Some(1));
        assert_eq!(robots_script_command_role(13, 20).runtime_subtype, Some(8));
        assert_eq!(robots_script_command_role(17, 0).family, "control");
        assert_eq!(robots_script_command_role(18, 0).family, "terminator");

        let malformed_sound = robots_script_command_role(5, 20);
        assert_eq!(malformed_sound.family, "malformed");
        assert!(!malformed_sound.classified);
    }

    #[test]
    fn robots_dynamic_light_payload_preserves_raw_lanes_and_native_fields() {
        let mut payload = Vec::new();
        for word in [
            0xFFFF_FFFF,
            0x00B4_0000,
            0x0800_0002,
            0xFF02_0FFF,
            1.0_f32.to_bits(),
            4.0_f32.to_bits(),
        ] {
            payload.extend_from_slice(&word.to_le_bytes());
        }

        assert_eq!(
            robots_script_payload_diagnostic(7, &payload),
            Some(RobotsScriptPayloadDiagnostic::DynamicLight {
                raw_word_0: 0xFFFF_FFFF,
                orientation_selector: 0,
                raw_orientation_tail: 0x00B4,
                mode_byte: 2,
                raw_mode_tail: [0, 0, 8],
                packed_color: 0xFF02_0FFF,
                runtime_scalar_34: 1.0,
                runtime_scalar_38: 4.0,
            })
        );
    }

    #[test]
    fn robots_camera_payload_maps_to_exact_native_object_offsets() {
        let mut payload = Vec::new();
        for word in [
            0xFFFF_FFFF,
            0,
            0.48_f32.to_bits(),
            0.64_f32.to_bits(),
            1.0_f32.to_bits(),
            1.0_f32.to_bits(),
        ] {
            payload.extend_from_slice(&word.to_le_bytes());
        }

        assert_eq!(
            robots_script_payload_diagnostic(10, &payload),
            Some(RobotsScriptPayloadDiagnostic::Camera {
                raw_word_0: 0xFFFF_FFFF,
                raw_word_1: 0,
                object_114: 0.48,
                object_118: 0.64,
                object_110: 1.0,
                object_11c: 1.0,
            })
        );
    }

    #[test]
    fn robots_collision_payload_maps_to_exact_native_object_offsets() {
        let mut payload = Vec::new();
        for word in [0xFFFF_FFFF, 1, 0.5_f32.to_bits(), 0, 0] {
            payload.extend_from_slice(&word.to_le_bytes());
        }

        assert_eq!(
            robots_script_payload_diagnostic(13, &payload),
            Some(RobotsScriptPayloadDiagnostic::Collision {
                object_140: 0xFFFF_FFFF,
                object_13f_mode: 1,
                raw_mode_tail: [0, 0, 0],
                object_130: 0.5,
                object_134: 0.0,
                object_138: 0.0,
            })
        );
    }

    #[test]
    fn invalid_animscript_rate_has_a_finite_diagnostic_fallback() {
        let mut script = script(vec![]);
        script.framerate = 0.0;
        script.length = 30;

        assert_eq!(script.timeline_framerate(), 30.0);
        assert_eq!(script.duration_seconds(), 1.0);
    }
}
