use std::{
    fs::File,
    io::{BufReader, Seek, SeekFrom},
};

use anyhow::{Context, Result};
use eurochef_edb::{
    anim::EXGeoBaseAnimSkin, binrw::BinReaderExt, edb::EdbFile, versions::Platform,
};

fn quat_mul(left: [f32; 4], right: [f32; 4]) -> [f32; 4] {
    let [lx, ly, lz, lw] = left;
    let [rx, ry, rz, rw] = right;
    [
        lw * rx + lx * rw + ly * rz - lz * ry,
        lw * ry - lx * rz + ly * rw + lz * rx,
        lw * rz + lx * ry - ly * rx + lz * rw,
        lw * rw - lx * rx - ly * ry - lz * rz,
    ]
}

fn quat_normalize(mut q: [f32; 4]) -> [f32; 4] {
    let length = q.iter().map(|v| v * v).sum::<f32>().sqrt();
    if length > f32::EPSILON {
        for value in &mut q {
            *value /= length;
        }
    }
    q
}

fn quat_slerp(left: [f32; 4], mut right: [f32; 4], t: f32) -> [f32; 4] {
    let mut dot = left.iter().zip(right).map(|(a, b)| a * b).sum::<f32>();
    if dot < 0.0 {
        for value in &mut right {
            *value = -*value;
        }
        dot = -dot;
    }
    if dot > 0.9995 {
        return quat_normalize([
            left[0] + (right[0] - left[0]) * t,
            left[1] + (right[1] - left[1]) * t,
            left[2] + (right[2] - left[2]) * t,
            left[3] + (right[3] - left[3]) * t,
        ]);
    }
    let theta = dot.clamp(-1.0, 1.0).acos();
    let sin_theta = theta.sin();
    let a = ((1.0 - t) * theta).sin() / sin_theta;
    let b = (t * theta).sin() / sin_theta;
    quat_normalize([
        left[0] * a + right[0] * b,
        left[1] * a + right[1] * b,
        left[2] * a + right[2] * b,
        left[3] * a + right[3] * b,
    ])
}

fn quat_rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let [x, y, z, w] = q;
    let [vx, vy, vz] = v;
    let tx = 2.0 * (y * vz - z * vy);
    let ty = 2.0 * (z * vx - x * vz);
    let tz = 2.0 * (x * vy - y * vx);
    [
        vx + w * tx + (y * tz - z * ty),
        vy + w * ty + (z * tx - x * tz),
        vz + w * tz + (x * ty - y * tx),
    ]
}

fn resolve_global_bone(
    index: usize,
    parents: &[u16],
    locals: &[([f32; 3], [f32; 4])],
    cache: &mut [Option<([f32; 3], [f32; 4])>],
    visiting: &mut [bool],
) -> Option<([f32; 3], [f32; 4])> {
    if let Some(value) = cache.get(index).copied().flatten() {
        return Some(value);
    }
    if *visiting.get(index)? {
        return None;
    }
    visiting[index] = true;
    let (local_position, local_rotation) = *locals.get(index)?;
    let parent = *parents.get(index)?;
    let global = if parent == u16::MAX {
        (local_position, local_rotation)
    } else {
        let (parent_position, parent_rotation) =
            resolve_global_bone(parent as usize, parents, locals, cache, visiting)?;
        let rotated = quat_rotate(parent_rotation, local_position);
        (
            [
                parent_position[0] + rotated[0],
                parent_position[1] + rotated[1],
                parent_position[2] + rotated[2],
            ],
            quat_mul(parent_rotation, local_rotation),
        )
    };
    visiting[index] = false;
    cache[index] = Some(global);
    Some(global)
}

fn main() -> Result<()> {
    let path = r"D:\Games\Robots\_eurotools_out\extracted_main\robots\binary\_bin_pc\nb11_rat.edb";
    let file = File::open(path).with_context(|| format!("open {path}"))?;
    let mut edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)?;
    let endian = edb.endian;
    let version = edb.header.version;
    let modes = edb.header.animmode_list.data().clone();
    let sets = edb.header.animset_list.data().clone();

    println!(
        "file=0x{:08X} modes={} sets={}",
        edb.header.hashcode,
        modes.len(),
        sets.len()
    );
    let sources = [0x0900_0106u32, 0x0900_0002u32];
    let targets = [
        0x0900_0025u32,
        0x0900_0027u32,
        0x0900_002Du32,
        0x0900_007Eu32,
        0x0900_0109u32,
        0x0900_0108u32,
        0x0900_0104u32,
        0x0900_0105u32,
        0x0900_0107u32,
    ];

    for source in sources {
        let Some(source_mode) = modes
            .iter()
            .find(|mode| mode.common.hashcode == source)
            .cloned()
        else {
            println!("source=0x{source:08X} missing");
            continue;
        };
        let transitions = source_mode.read_robots_v248_transitions(&mut edb, endian)?;
        println!("source=0x{source:08X} transitions={}", transitions.len());
        for target in targets {
            let Some(target_index) = modes.iter().position(|mode| mode.common.hashcode == target)
            else {
                println!("  target=0x{target:08X} missing");
                continue;
            };
            let Some(transition) = transitions
                .iter()
                .find(|t| t.new_mode_index as usize == target_index)
            else {
                println!("  target=0x{target:08X} index={target_index} no-transition");
                continue;
            };
            println!(
                "  target=0x{target:08X} index={target_index} controls={} list=0x{:X}",
                transition.controls.len(),
                transition.control_list_offset
            );
            for control in &transition.controls {
                println!(
                    "    control off=0x{:X} opcode=0x{:08X} key={} mask=0x{:04X} sparse={:?}",
                    control.offset,
                    control.opcode,
                    control.resource_key,
                    control.sparse_mask,
                    control.sparse_values
                );
                if matches!(control.opcode, 0x0C00_0002 | 0x0C00_0003) {
                    if let Some(set) = sets.get(control.resource_key as usize).cloned() {
                        let groups = set.read_robots_v248_groups(&mut edb, endian)?;
                        println!(
                            "      animset=0x{:08X} groups={}",
                            set.common.hashcode,
                            groups.len()
                        );
                        for (group_index, group) in groups.iter().enumerate() {
                            println!(
                                "        group={group_index} layer={} weight={} contributions={}",
                                group.layer,
                                group.weight,
                                group.contributions.len()
                            );
                            for contribution in &group.contributions {
                                println!(
                                    "          resource=0x{:08X} raw04=0x{:04X} raw06=0x{:04X} raw08=0x{:08X}",
                                    contribution.resource_hashcode,
                                    contribution.raw_u16_04,
                                    contribution.raw_u16_06,
                                    contribution.raw_u32_08
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    let animation = edb
        .header
        .anim_list
        .iter()
        .find(|animation| animation.common.hashcode == 0x8300_0009)
        .cloned()
        .context("missing firing Animation 0x83000009")?;
    let skin_header = edb
        .header
        .animskin_list
        .iter()
        .find(|skin| skin.base_skin_num == animation.skin_num)
        .cloned()
        .context("missing firing Animation bound AnimSkin")?;
    edb.seek(SeekFrom::Start(skin_header.common.address as u64))?;
    let skin = edb.read_type_args::<EXGeoBaseAnimSkin>(endian, (version,))?;
    let bone_hashcodes = skin.read_robots_v248_bone_hashcodes(&mut edb, endian)?;
    let right_hand_selector = bone_hashcodes
        .iter()
        .position(|hash| *hash == Some(0x0E00_0015))
        .context("AnimSkin has no HT_AnimBone_R_Hand")?;

    let serialized = animation.read_robots_v248_exgeoanim(&mut edb, endian)?;
    let block_table = animation.read_robots_v248_motion_block_table(&mut edb, endian)?;
    let root_correction = animation.read_robots_v248_root_correction_transform(&mut edb, endian)?;
    let motion = animation.read_robots_v248_motion_stream(&mut edb)?;
    let event_phase = 17.0f32 / 36.0f32;
    let asset_frame = event_phase * f32::from(serialized.frame_count.saturating_sub(1));
    let frame0 = asset_frame.floor() as u16;
    let frame1 = asset_frame.ceil() as u16;
    let fraction = asset_frame.fract();

    let decode_pose = |frame_index: u16| -> Result<_> {
        let raw = match block_table.as_ref() {
            Some(table) => serialized
                .decode_skeletal_frame_with_block_table(&motion, frame_index, table)
                .map_err(anyhow::Error::msg)?,
            None => serialized
                .decode_skeletal_frame(&motion, frame_index)
                .map_err(anyhow::Error::msg)?,
        };
        serialized
            .assemble_same_skin_pose_with_root_correction(
                &raw,
                &skin.relative_bind_positions,
                root_correction.as_ref(),
            )
            .map_err(anyhow::Error::msg)
    };
    let pose0 = decode_pose(frame0)?;
    let pose1 = decode_pose(frame1)?;
    let locals0 = pose0
        .bones
        .iter()
        .map(|bone| (bone.position, bone.rotation))
        .collect::<Vec<_>>();
    let locals1 = pose1
        .bones
        .iter()
        .map(|bone| (bone.position, bone.rotation))
        .collect::<Vec<_>>();
    let build_interpolated_locals = |t: f32| {
        pose0
            .bones
            .iter()
            .zip(&pose1.bones)
            .map(|(left, right)| {
                (
                    [
                        left.position[0] + (right.position[0] - left.position[0]) * t,
                        left.position[1] + (right.position[1] - left.position[1]) * t,
                        left.position[2] + (right.position[2] - left.position[2]) * t,
                    ],
                    quat_slerp(left.rotation, right.rotation, t),
                )
            })
            .collect::<Vec<_>>()
    };
    let locals = build_interpolated_locals(fraction);
    let native_frame_step = f32::from(serialized.raw_byte_0c) / 60.0;
    let native_launch_asset_frame = 17.0 - native_frame_step;
    let native_launch_fraction = native_launch_asset_frame.fract();
    let native_locals = build_interpolated_locals(native_launch_fraction);
    let parents = skin
        .hier_data
        .iter()
        .map(|hierarchy| hierarchy.link_index)
        .collect::<Vec<_>>();
    let mut cache0 = vec![None; locals0.len()];
    let mut visiting0 = vec![false; locals0.len()];
    let (right_hand_frame0_position, right_hand_frame0_rotation) = resolve_global_bone(
        right_hand_selector,
        &parents,
        &locals0,
        &mut cache0,
        &mut visiting0,
    )
    .context("could not resolve frame0 R_Hand global transform")?;
    let mut cache1 = vec![None; locals1.len()];
    let mut visiting1 = vec![false; locals1.len()];
    let (right_hand_frame1_position, right_hand_frame1_rotation) = resolve_global_bone(
        right_hand_selector,
        &parents,
        &locals1,
        &mut cache1,
        &mut visiting1,
    )
    .context("could not resolve frame1 R_Hand global transform")?;
    let mut native_cache = vec![None; native_locals.len()];
    let mut native_visiting = vec![false; native_locals.len()];
    let (right_hand_native_position, right_hand_native_rotation) = resolve_global_bone(
        right_hand_selector,
        &parents,
        &native_locals,
        &mut native_cache,
        &mut native_visiting,
    )
    .context("could not resolve native launch R_Hand global transform")?;
    let mut cache = vec![None; locals.len()];
    let mut visiting = vec![false; locals.len()];
    let (right_hand_position, right_hand_rotation) = resolve_global_bone(
        right_hand_selector,
        &parents,
        &locals,
        &mut cache,
        &mut visiting,
    )
    .context("could not resolve interpolated R_Hand global transform")?;

    println!(
        "firing animation=0x{:08X} skin=0x{:08X} asset_frames={} anim_rate_byte={} bones={} r_hand_selector={} parent={} script_event_frame=17 command_length=36 gui_phase={:.9} gui_asset_frame={:.9} gui_sample={}..{} t={:.9}",
        animation.common.hashcode,
        skin_header.common.hashcode,
        serialized.frame_count,
        serialized.raw_byte_0c,
        serialized.bone_count,
        right_hand_selector,
        parents[right_hand_selector],
        event_phase,
        asset_frame,
        frame0,
        frame1,
        fraction
    );
    println!(
        "r_hand_frame_16 position=[{:.9},{:.9},{:.9}] rotation=[{:.9},{:.9},{:.9},{:.9}]",
        right_hand_frame0_position[0],
        right_hand_frame0_position[1],
        right_hand_frame0_position[2],
        right_hand_frame0_rotation[0],
        right_hand_frame0_rotation[1],
        right_hand_frame0_rotation[2],
        right_hand_frame0_rotation[3]
    );
    println!(
        "r_hand_frame_17 position=[{:.9},{:.9},{:.9}] rotation=[{:.9},{:.9},{:.9},{:.9}]",
        right_hand_frame1_position[0],
        right_hand_frame1_position[1],
        right_hand_frame1_position[2],
        right_hand_frame1_rotation[0],
        right_hand_frame1_rotation[1],
        right_hand_frame1_rotation[2],
        right_hand_frame1_rotation[3]
    );
    println!(
        "native_launch previous_asset_frame={:.9} step={:.9} r_hand_position=[{:.9},{:.9},{:.9}] rotation=[{:.9},{:.9},{:.9},{:.9}]",
        native_launch_asset_frame,
        native_frame_step,
        right_hand_native_position[0],
        right_hand_native_position[1],
        right_hand_native_position[2],
        right_hand_native_rotation[0],
        right_hand_native_rotation[1],
        right_hand_native_rotation[2],
        right_hand_native_rotation[3]
    );
    println!(
        "r_hand_gui_interpolated position=[{:.9},{:.9},{:.9}] rotation=[{:.9},{:.9},{:.9},{:.9}]",
        right_hand_position[0],
        right_hand_position[1],
        right_hand_position[2],
        right_hand_rotation[0],
        right_hand_rotation[1],
        right_hand_rotation[2],
        right_hand_rotation[3]
    );
    Ok(())
}
