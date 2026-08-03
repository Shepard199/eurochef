use binrw::{binread, BinRead, BinReaderExt, BinResult, VecArgs};
use serde::Serialize;
use tracing::debug;

use crate::common::{EXRelPtr, EXVector, EXVector3};

#[binread]
#[derive(Debug, Serialize, Clone)]
pub struct EXGeoAnimScript {
    #[brw(assert(vtable.eq(&0x300)))]
    pub vtable: u32, // 0x0
    pub length: u32,                  // 0x4
    pub _unk8: u8,                    // 0x8
    pub timejump_count: u8,           // 0x9
    pub script_flags: u16,            // 0xa
    pub frame_rate: f32,              // 0xc
    pub bounds_box: [EXVector; 2],    // 0x10
    pub unk30: u32,                   // 0x30
    pub thread_controllers: EXRelPtr, // 0x34
    pub thread_info: EXRelPtr,        // 0x38
    pub thread_controller_count: u16, // 0x3c
    pub _unk3e: u16,                  // 0x3e
    pub used_controller_types: u32,   // 0x40

    #[br(parse_with = parse_commands)]
    pub commands: Vec<EXGeoAnimScriptCmd>,
}

#[derive(Debug, Serialize, Clone)]
pub struct EXGeoAnimScriptControllerHeader {
    pub controller_count: u16,
    pub channel_count: u16,
    pub ctrl_mask: u32,
    pub ctrl_channel_mask: u32,

    pub channels: EXGeoAnimScriptControllerChannels,
}

#[derive(Debug, Serialize, Clone)]
pub struct EXGeoAnimScriptUnknownChannel {
    pub bit: u32,
    pub keyframe_count: i16,
    pub flags: u16,
    pub data_ptr: EXRelPtr,
}

// ROBOTS_PATCH_0026_ANIMSCRIPT_CHANNELS
#[derive(Debug, Serialize, Clone, Default)]
pub struct EXGeoAnimScriptControllerChannels {
    pub time_0: Vec<(f32, f32)>,         // 0x00000001
    pub time_1: Vec<(f32, f32)>,         // 0x00000002
    pub vector_0: Vec<(f32, EXVector3)>, // 0x00000004, position
    pub quat_0: Vec<(f32, EXVector)>,    // 0x00000008, rotation
    pub vector_1: Vec<(f32, EXVector3)>, // 0x00000010, scale
    pub vector_2: Vec<(f32, EXVector3)>, // 0x00000040

    // 0x80: structurally keyed float4, 20 bytes/keyframe.
    // Semantic name stays neutral until a runtime consumer proves it.
    pub vector_3: Vec<(f32, EXVector)>,

    // ROBOTS_PATCH_0033_ANIMSCRIPT_4000_STRUCTURAL_PAIR
    // ROBOTS_PATCH_0042_ANIMSCRIPT_4000_SCRIPT_CONTEXT
    // 0x4000: structurally proven as two f32 values per keyframe (8 bytes).
    // Semantic roles remain deliberately unnamed until native payload consumption
    // is proven. pair_14_raw[0]/[1] are raw float lanes, not "time/value" claims.
    pub pair_14_raw: Vec<(f32, f32)>,

    // In-memory provenance only. This does not consume serialized bytes.
    // It preserves the exact EXRelPtr target so shared-layer diagnostics can tie
    // a promoted pair corpus back to its script/controller context.
    pub pair_14_data_abs: Option<u64>,

    // 0x8000/0x10000: structurally keyed scalar floats, 8 bytes/keyframe.
    pub scalar_15: Vec<(f32, f32)>,
    pub scalar_16: Vec<(f32, f32)>,

    // Genuine remaining unknowns are retained, not discarded.
    pub unknown: Vec<EXGeoAnimScriptUnknownChannel>,
}

impl BinRead for EXGeoAnimScriptControllerHeader {
    type Args<'a> = ();

    #[allow(unused_braces)]
    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        (): Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        // PATCH_0033 boundary anchor: exact start of the serialized controller header.
        let robots_controller_header_start = reader.stream_position()?;

        let controller_count = reader.read_type(endian)?;
        let channel_count = reader.read_type(endian)?;
        let ctrl_mask = reader.read_type(endian)?;
        let ctrl_channel_mask = reader.read_type(endian)?;

        let mut channels = EXGeoAnimScriptControllerChannels::default();

        if controller_count == 0 {
            return Ok(EXGeoAnimScriptControllerHeader {
                controller_count,
                channel_count,
                ctrl_mask,
                ctrl_channel_mask,
                channels,
            });
        }

        macro_rules! with_offset {
            ($offset:expr_2021, $inner:tt) => {{
                let pos_saved = reader.stream_position()?;
                reader.seek(std::io::SeekFrom::Start($offset))?;

                let res = $inner;

                reader.seek(std::io::SeekFrom::Start(pos_saved))?;

                res
            }};
        }

        macro_rules! read_channel_descriptor {
            () => {{
                let num_keyframes: i16 = reader.read_type(endian)?;
                let flags: u16 = reader.read_type(endian)?;
                let data_ptr: EXRelPtr = reader.read_type(endian)?;
                (num_keyframes, flags, data_ptr)
            }};
        }

        macro_rules! read_channel {
            () => {{
                let (num_keyframes, _flags, data_ptr) = read_channel_descriptor!();
                with_offset!(data_ptr.offset_absolute(), {
                    reader.read_type_args(
                        endian,
                        VecArgs {
                            count: num_keyframes.unsigned_abs() as usize,
                            inner: (),
                        },
                    )?
                })
            }};
        }

        // ROBOTS_PATCH_0032_ANIMSCRIPT_4000_FORENSIC_PROBE
        //
        // Do not infer payload type from the bit number or neighboring channels.
        // For unresolved 0x4000 we first capture:
        //   - exact descriptor byte boundary,
        //   - operand widths already encoded by the descriptor schema,
        //   - ctrl_mask / ctrl_channel_mask role context,
        //   - EXRelPtr absolute target provenance,
        //   - file boundary,
        //   - bounded raw payload bytes.
        //
        // The stream position is restored before normal parsing continues.
        macro_rules! preserve_unknown_channel {
            ($bit:expr_2021) => {{
                let descriptor_start = reader.stream_position()?;
                let (num_keyframes, flags, data_ptr) = read_channel_descriptor!();
                let descriptor_end = reader.stream_position()?;

                if $bit == 0x4000 {
                    let return_pos = descriptor_end;
                    let data_abs = data_ptr.offset_absolute();

                    let file_end = reader.seek(std::io::SeekFrom::End(0))?;
                    reader.seek(std::io::SeekFrom::Start(return_pos))?;

                    let available = file_end.saturating_sub(data_abs);
                    let probe_len_u64 = available.min(64);
                    let probe_len = usize::try_from(probe_len_u64).unwrap_or(0);
                    let mut raw_probe = vec![0u8; probe_len];

                    let data_in_file = data_abs <= file_end;
                    if data_in_file && probe_len != 0 {
                        reader.seek(std::io::SeekFrom::Start(data_abs))?;
                        std::io::Read::read_exact(reader, &mut raw_probe)?;
                    }

                    reader.seek(std::io::SeekFrom::Start(return_pos))?;

                    let raw_hex = raw_probe
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<Vec<_>>()
                        .join("");

                    debug!(
                        "ROBOTS_ANIMSCRIPT_4000_FORENSIC descriptor=[0x{:x},0x{:x}) descriptor_bytes={} controller_count={} channel_count={} ctrl_mask=0x{:08x} ctrl_channel_mask=0x{:08x} keyframes={} flags=0x{:04x} data_abs=0x{:x} file_end=0x{:x} data_in_file={} raw_len={} raw={}",
                        descriptor_start,
                        descriptor_end,
                        descriptor_end.saturating_sub(descriptor_start),
                        controller_count,
                        channel_count,
                        ctrl_mask,
                        ctrl_channel_mask,
                        num_keyframes,
                        flags,
                        data_abs,
                        file_end,
                        data_in_file,
                        raw_probe.len(),
                        raw_hex
                    );
                } else {
                    debug!(
                        "Unresolved anim script controller channel 0x{:x} (addr=0x{:x}, keyframes={}, flags=0x{:04x})",
                        $bit,
                        data_ptr.offset_absolute(),
                        num_keyframes,
                        flags
                    );
                }

                channels.unknown.push(EXGeoAnimScriptUnknownChannel {
                    bit: $bit,
                    keyframe_count: num_keyframes,
                    flags,
                    data_ptr,
                });
            }};
        }

        if (ctrl_mask & 0x1) != 0 {
            channels.time_0 = read_channel!();
        }

        if (ctrl_mask & 0x2) != 0 {
            channels.time_1 = read_channel!();
        }

        if (ctrl_mask & 0x4) != 0 {
            channels.vector_0 = read_channel!();
        }

        if (ctrl_mask & 0x8) != 0 {
            channels.quat_0 = read_channel!();
        }

        if (ctrl_mask & 0x10) != 0 {
            channels.vector_1 = read_channel!();
        }

        if (ctrl_mask & 0x20) != 0 {
            preserve_unknown_channel!(0x20);
        }

        if (ctrl_mask & 0x40) != 0 {
            channels.vector_2 = read_channel!();
        }

        if (ctrl_mask & 0x80) != 0 {
            channels.vector_3 = read_channel!();
        }

        for i in 8..14 {
            if (ctrl_mask & (1 << i)) != 0 {
                preserve_unknown_channel!(1 << i);
            }
        }

        if (ctrl_mask & 0x4000) != 0 {
            // Consume the descriptor exactly once.
            let descriptor_start = reader.stream_position()?;
            let (num_keyframes, flags, data_ptr) = read_channel_descriptor!();
            let descriptor_end = reader.stream_position()?;

            let data_abs = data_ptr.offset_absolute();
            let count = num_keyframes.unsigned_abs() as usize;
            let expected_bytes = (count as u64).saturating_mul(8);
            let expected_end = data_abs.saturating_add(expected_bytes);

            // Fail closed. We only promote to the neutral two-f32 structure when:
            //  - descriptor width is exactly 8 bytes;
            //  - keyframe count is non-negative;
            //  - flags are zero, matching all observed corpus samples;
            //  - no higher channel bits are set, making 0x4000 the final payload;
            //  - count * 8 lands exactly on the controller header boundary.
            let no_higher_channels = (ctrl_mask & 0xffff8000) == 0;
            let exact_boundary = descriptor_end.saturating_sub(descriptor_start) == 8
                && num_keyframes >= 0
                && flags == 0
                && no_higher_channels
                && data_abs <= robots_controller_header_start
                && expected_end == robots_controller_header_start;

            if exact_boundary {
                channels.pair_14_raw = with_offset!(data_abs, {
                    reader.read_type_args(endian, VecArgs { count, inner: () })?
                });
                channels.pair_14_data_abs = Some(data_abs);

                debug!(
                    "ROBOTS_ANIMSCRIPT_4000_STRUCTURAL_PROOF data_abs=0x{:x} count={} bytes={} end=0x{:x} header_start=0x{:x} descriptor=[0x{:x},0x{:x}) flags=0x{:04x} ctrl_mask=0x{:08x}",
                    data_abs,
                    count,
                    expected_bytes,
                    expected_end,
                    robots_controller_header_start,
                    descriptor_start,
                    descriptor_end,
                    flags,
                    ctrl_mask
                );

                // ROBOTS_PATCH_0040_ANIMSCRIPT_4000_PAIR_CORPUS
                //
                // Forensic logging only. Preserve both raw IEEE-754 bit patterns and
                // numeric f32 renderings without assigning semantic lane names.
                // No additional reads/seeks occur here; values come from pair_14_raw
                // already decoded by the PATCH_0033 exact-boundary gate.
                let pair_dump = channels
                    .pair_14_raw
                    .iter()
                    .take(64)
                    .enumerate()
                    .map(|(index, (lane0, lane1))| {
                        format!(
                            "{}:{:08x}:{:08x}:{:.9}:{:.9}",
                            index,
                            lane0.to_bits(),
                            lane1.to_bits(),
                            lane0,
                            lane1
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",");

                debug!(
                    "ROBOTS_ANIMSCRIPT_4000_PAIR_CORPUS data_abs=0x{:x} count={} logged={} truncated={} pairs={}",
                    data_abs,
                    channels.pair_14_raw.len(),
                    channels.pair_14_raw.len().min(64),
                    channels.pair_14_raw.len() > 64,
                    pair_dump
                );
            } else {
                debug!(
                    "ROBOTS_ANIMSCRIPT_4000_NOT_PROMOTED data_abs=0x{:x} count={} expected_end=0x{:x} header_start=0x{:x} descriptor_bytes={} flags=0x{:04x} no_higher_channels={} ctrl_mask=0x{:08x}",
                    data_abs,
                    count,
                    expected_end,
                    robots_controller_header_start,
                    descriptor_end.saturating_sub(descriptor_start),
                    flags,
                    no_higher_channels,
                    ctrl_mask
                );

                channels.unknown.push(EXGeoAnimScriptUnknownChannel {
                    bit: 0x4000,
                    keyframe_count: num_keyframes,
                    flags,
                    data_ptr,
                });
            }
        }

        if (ctrl_mask & 0x8000) != 0 {
            channels.scalar_15 = read_channel!();
        }

        if (ctrl_mask & 0x10000) != 0 {
            channels.scalar_16 = read_channel!();
        }

        for i in 17..32 {
            if (ctrl_mask & (1 << i)) != 0 {
                preserve_unknown_channel!(1 << i);
            }
        }

        Ok(Self {
            controller_count,
            channel_count,
            ctrl_mask,
            ctrl_channel_mask,

            channels,
        })
    }
}

#[binrw::parser(reader, endian)]
fn parse_commands() -> BinResult<Vec<EXGeoAnimScriptCmd>> {
    let mut res = Vec::new();
    let mut commands_left = 1024;
    loop {
        if commands_left == 0 {
            return Err(binrw::Error::AssertFail {
                pos: reader.stream_position()?,
                message: "Exceeded command limit".to_string(),
            });
        }

        let cmd = EXGeoAnimScriptCmd::read_options(reader, endian, ())?;
        res.push(cmd.clone());
        if cmd.cmd_size == 0 {
            break;
        }

        commands_left -= 1;
    }

    Ok(res)
}

#[derive(Debug, Serialize, Clone)]
pub struct EXGeoAnimScriptCmd {
    pub cmd: u8,
    pub cmd_size: u8,
    /// Start frame
    pub cmd_frame: i16,
    pub data: Vec<u8>,

    pub start: i16,
    pub length: u16,
    /// Direct index into EXGeoAnimScript::thread_controllers.
    pub controller_header_index: u16,
    /// Runtime controller-record index. This addresses EXItemAnimator_Script+0x110
    /// with a 0x20-byte stride.
    pub controller_index: u8,
    /// Parent/linked runtime controller-record index; 0xFF means none.
    pub parent_controller_index: u8,
}

impl BinRead for EXGeoAnimScriptCmd {
    type Args<'a> = ();
    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let cmd = u8::read_options(reader, endian, ())?;
        let size = u8::read_options(reader, endian, ())?;
        let frame = i16::read_options(reader, endian, ())?;

        let (start, length, controller_header_index, controller_index, parent_controller_index) =
            if cmd != 0x12 {
                <_>::read_options(reader, endian, ())?
            } else {
                (0, 0, 0, 0, 0)
            };

        let data = if size == 0 {
            vec![]
        } else {
            <Vec<u8>>::read_options(
                reader,
                endian,
                VecArgs {
                    count: if cmd != 0x12 {
                        (size - 4 - 8) as usize
                    } else {
                        (size - 4) as usize
                    },
                    inner: (),
                },
            )?
        };

        Ok(Self {
            cmd,
            cmd_size: size,
            cmd_frame: frame,
            data,
            start,
            length,
            controller_header_index,
            controller_index,
            parent_controller_index,
        })
    }
}
