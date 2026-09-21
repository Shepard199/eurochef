use std::{
    fs::File,
    io::{BufReader, Seek, SeekFrom},
};

use anyhow::{Context, Result};
use eurochef_edb::{
    anim::EXGeoBaseAnimSkin, binrw::BinReaderExt, edb::EdbFile, versions::Platform,
};

fn qmul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let [ax, ay, az, aw] = a;
    let [bx, by, bz, bw] = b;
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

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

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn len(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn scan(file_name: &str) -> Result<()> {
    let path =
        format!(r"D:\Games\Robots\_eurotools_out\extracted_main\robots\binary\_bin_pc\{file_name}");
    let file = File::open(&path).with_context(|| format!("open {path}"))?;
    let mut edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)?;
    let endian = edb.endian;
    let skin_header = edb
        .header
        .animskin_list
        .data()
        .iter()
        .find(|skin| skin.common.hashcode == 0x0D00_0001)
        .cloned()
        .context("AnimSkin 0x0D000001 missing")?;
    edb.seek(SeekFrom::Start(skin_header.common.address as u64))?;
    let skin = edb.read_type_args::<EXGeoBaseAnimSkin>(endian, (248,))?;
    let hit = skin
        .robots_animdatum_section
        .as_ref()
        .context("AnimDatum section missing")?
        .find(0x1000_0010)
        .context("HitArea missing")?;
    let selector = usize::from(hit.transform_selector);
    let mut chain = Vec::new();
    let mut bone = selector;
    loop {
        chain.push(bone);
        let parent = usize::from(skin.hier_data[bone].link_index);
        if parent >= bone || parent >= skin.bone_count as usize {
            break;
        }
        bone = parent;
    }
    chain.reverse();
    let shape_extent = if hit.header.shape_mode == 3 {
        hit.shape_scalars[0] + hit.shape_scalars[1]
    } else {
        hit.shape_scalars[0]
    };
    let mut clips = 0usize;
    let mut frames = 0usize;
    let mut max_center = 0.0f32;
    let mut max_enclosing = 0.0f32;
    let mut max_clip = 0u32;
    let mut max_frame = 0usize;
    for animation in edb.header.anim_list.data().clone() {
        if animation.skin_num != skin_header.base_skin_num {
            continue;
        }
        let serialized = animation.read_robots_v248_exgeoanim(&mut edb, endian)?;
        if serialized.bone_count as usize != skin.bone_count as usize || serialized.frame_count == 0
        {
            continue;
        }
        let motion = animation.read_robots_v248_motion_stream(&mut edb)?;
        let blocks = animation.read_robots_v248_motion_block_table(&mut edb, endian)?;
        let correction = animation.read_robots_v248_root_correction_transform(&mut edb, endian)?;
        clips += 1;
        for frame in 0..usize::from(serialized.frame_count) {
            let raw = match blocks.as_ref() {
                Some(table) => serialized.decode_skeletal_frame_with_block_table(
                    &motion,
                    frame as u16,
                    table,
                )?,
                None => serialized.decode_skeletal_frame(&motion, frame as u16)?,
            };
            let pose = serialized.assemble_same_skin_pose_with_root_correction(
                &raw,
                &skin.relative_bind_positions,
                correction.as_ref(),
            )?;
            let mut gp = [0.0f32; 3];
            let mut gq = [0.0, 0.0, 0.0, 1.0];
            for &index in &chain {
                let local = &pose.bones[index];
                gp = add(gp, rotate(gq, local.position));
                gq = qmul(gq, local.rotation);
            }
            let center = add(gp, rotate(gq, hit.local_center));
            let center_len = len(center);
            let enclosing = center_len + shape_extent;
            max_center = max_center.max(center_len);
            if enclosing > max_enclosing {
                max_enclosing = enclosing;
                max_clip = animation.common.hashcode;
                max_frame = frame;
            }
            frames += 1;
        }
    }
    println!(
        "{file_name}: mode={} scalars={:?} center={:?} selector={} chain={chain:?} clips={clips} frames={frames} max_center={max_center:.9} max_owner_enclosing_radius={max_enclosing:.9} clip=0x{max_clip:08X} frame={max_frame}",
        hit.header.shape_mode,
        hit.shape_scalars,
        hit.local_center,
        hit.transform_selector,
    );
    Ok(())
}

fn main() -> Result<()> {
    scan("eb09_mal.edb")?;
    scan("eb10_rol.edb")?;
    scan("nb11_rat.edb")?;
    Ok(())
}
