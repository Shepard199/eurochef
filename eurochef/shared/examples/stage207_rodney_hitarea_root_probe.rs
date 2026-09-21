use std::{
    fs::File,
    io::{BufReader, Seek, SeekFrom},
};

use anyhow::{Context, Result};
use eurochef_edb::{
    anim::EXGeoBaseAnimSkin, binrw::BinReaderExt, edb::EdbFile, versions::Platform,
};

fn rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let [x, y, z, w] = q;
    let tx = 2.0 * (y * v[2] - z * v[1]);
    let ty = 2.0 * (z * v[0] - x * v[2]);
    let tz = 2.0 * (x * v[1] - y * v[0]);
    [
        v[0] + w * tx + (y * tz - z * ty),
        v[1] + w * ty + (z * tx - x * tz),
        v[2] + w * tz + (x * ty - y * tx),
    ]
}

fn main() -> Result<()> {
    let path = r"D:\Games\Robots\_eurotools_out\extracted_main\robots\binary\_bin_pc\p01_rod.edb";
    let file = File::open(path).with_context(|| format!("open {path}"))?;
    let mut edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)?;
    let skin_header = edb
        .header
        .animskin_list
        .data()
        .iter()
        .find(|skin| skin.common.hashcode == 0x0D00_0001)
        .cloned()
        .context("Rodney AnimSkin 0x0D000001 missing")?;
    edb.seek(SeekFrom::Start(skin_header.common.address as u64))?;
    let skin = edb.read_type_args::<EXGeoBaseAnimSkin>(edb.endian, (248,))?;
    let hit = skin
        .robots_animdatum_section
        .as_ref()
        .context("Rodney AnimDatum section missing")?
        .find(0x1000_0010)
        .context("Rodney HitArea missing")?;
    println!(
        "hit mode={} scalars={:?} center={:?} selector={} hierarchy={:?}",
        hit.header.shape_mode,
        hit.shape_scalars,
        hit.local_center,
        hit.transform_selector,
        hit.hierarchy_chain
    );

    let animations = edb.header.anim_list.data().clone();
    let endian = edb.endian;
    let mut clips = 0usize;
    let mut frames = 0usize;
    let mut max_root_translation = 0.0f32;
    let mut max_hit_center_offset = 0.0f32;
    let mut max_root_clip = 0u32;
    let mut max_root_frame = 0usize;
    let mut max_hit_clip = 0u32;
    let mut max_hit_frame = 0usize;
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];

    for animation in animations {
        if animation.skin_num != skin_header.base_skin_num {
            continue;
        }
        let serialized = animation.read_robots_v248_exgeoanim(&mut edb, endian)?;
        if serialized.bone_count as usize != skin.bone_count as usize || serialized.frame_count == 0
        {
            continue;
        }
        let motion = animation.read_robots_v248_motion_stream(&mut edb)?;
        let block_table = animation.read_robots_v248_motion_block_table(&mut edb, endian)?;
        let correction = animation.read_robots_v248_root_correction_transform(&mut edb, endian)?;
        clips += 1;
        for frame in 0..usize::from(serialized.frame_count) {
            let raw = match block_table.as_ref() {
                Some(table) => serialized.decode_skeletal_frame_with_block_table(
                    &motion,
                    frame as u16,
                    table,
                )?,
                None => serialized.decode_skeletal_frame(&motion, frame as u16)?,
            };
            let assembled = serialized.assemble_same_skin_pose_with_root_correction(
                &raw,
                &skin.relative_bind_positions,
                correction.as_ref(),
            )?;
            let root = &assembled.bones[0];
            let p = root.position;
            let root_len = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
            if root_len > max_root_translation {
                max_root_translation = root_len;
                max_root_clip = animation.common.hashcode;
                max_root_frame = frame;
            }
            let local = rotate(root.rotation, hit.local_center);
            let center = [p[0] + local[0], p[1] + local[1], p[2] + local[2]];
            let center_len =
                (center[0] * center[0] + center[1] * center[1] + center[2] * center[2]).sqrt();
            if center_len > max_hit_center_offset {
                max_hit_center_offset = center_len;
                max_hit_clip = animation.common.hashcode;
                max_hit_frame = frame;
            }
            for axis in 0..3 {
                min[axis] = min[axis].min(center[axis]);
                max[axis] = max[axis].max(center[axis]);
            }
            frames += 1;
        }
    }
    println!(
        "clips={clips} frames={frames} max_root_translation={max_root_translation:.9} clip=0x{max_root_clip:08X} frame={max_root_frame}"
    );
    println!("hit_center_offset_aabb min={min:?} max={max:?}");
    println!(
        "max_hit_center_offset={max_hit_center_offset:.9} clip=0x{max_hit_clip:08X} frame={max_hit_frame}"
    );
    Ok(())
}
