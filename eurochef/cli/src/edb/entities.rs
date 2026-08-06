use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, Cursor, Seek, SeekFrom, Write},
    path::Path,
};

use anyhow::Context;
use base64::Engine;
use eurochef_edb::{
    binrw::BinReaderExt,
    edb::EdbFile,
    entity::{EXGeoEntity, EXGeoNavMeshEntity},
    versions::Platform,
};
use eurochef_shared::{entities::read_entity, textures::UXGeoTexture};
use image::ImageFormat;
use indicatif::{ProgressBar, ProgressIterator, ProgressStyle};

use crate::{
    edb::{gltf_export, TICK_STRINGS},
    PlatformArg,
};

fn export_static_mesh_obj<W: Write>(
    mut out: W,
    name: &str,
    vertices: &[eurochef_shared::entities::UXVertex],
    indices: &[u32],
    strips: &[eurochef_shared::entities::TriStrip],
) -> anyhow::Result<()> {
    writeln!(out, "o {name}")?;
    for v in vertices {
        writeln!(out, "v {} {} {}", v.pos[0], v.pos[1], v.pos[2])?;
    }
    for v in vertices {
        writeln!(out, "vt {} {}", v.uv[0], 1.0 - v.uv[1])?;
    }
    for v in vertices {
        writeln!(out, "vn {} {} {}", v.norm[0], v.norm[1], v.norm[2])?;
    }
    for (strip_index, strip) in strips.iter().enumerate() {
        writeln!(out, "g prim_{strip_index:03}")?;
        let start = strip.start_index as usize;
        let end = start.saturating_add(strip.index_count as usize);
        let Some(strip_indices) = indices.get(start..end) else {
            continue;
        };
        for tri in strip_indices.chunks_exact(3) {
            let a = tri[0] + 1;
            let b = tri[2] + 1;
            let c = tri[1] + 1;
            writeln!(out, "f {a}/{a}/{a} {b}/{b}/{b} {c}/{c}/{c}")?;
        }
    }
    Ok(())
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Transparency {
    Opaque,
    Blend,
    _Additive,
    Cutout,
}

fn export_navmesh(
    edb: &mut EdbFile,
    navmesh: &EXGeoNavMeshEntity,
    output_folder: &Path,
    source_edb: &str,
    ent_id: &str,
) -> anyhow::Result<()> {
    let mut vertices: Vec<[f32; 3]> = Vec::with_capacity(navmesh.vertex_count as usize);
    edb.seek(SeekFrom::Start(navmesh.vertices.offset_absolute()))?;
    for _ in 0..navmesh.vertex_count {
        vertices.push(edb.read_type::<[f32; 3]>(edb.endian)?);
    }

    let mut raw_faces: Vec<[u32; 4]> = Vec::with_capacity(navmesh.face_count as usize);
    let mut faces: Vec<[u32; 3]> = Vec::with_capacity(navmesh.face_count as usize);
    edb.seek(SeekFrom::Start(navmesh.faces.offset_absolute()))?;
    for face_index in 0..navmesh.face_count {
        let raw = edb.read_type::<[u32; 4]>(edb.endian)?;
        let face = [
            raw[0] & 0x000f_ffff,
            raw[1] & 0x000f_ffff,
            raw[2] & 0x000f_ffff,
        ];
        if face.iter().any(|index| *index >= navmesh.vertex_count) {
            anyhow::bail!(
                "NavMesh face {} contains vertex outside 0..{}: {:?}",
                face_index,
                navmesh.vertex_count,
                face
            );
        }
        raw_faces.push(raw);
        faces.push(face);
    }

    let mut raw_adjacency: Vec<[u32; 3]> = Vec::with_capacity(navmesh.face_count as usize);
    let mut adjacency: Vec<[i64; 3]> = Vec::with_capacity(navmesh.face_count as usize);
    edb.seek(SeekFrom::Start(navmesh.adjacency.offset_absolute()))?;
    for face_index in 0..navmesh.face_count {
        let raw = edb.read_type::<[u32; 3]>(edb.endian)?;
        let mut decoded = [-1i64; 3];
        for edge in 0..3 {
            let neighbor = raw[edge] & 0x000f_ffff;
            if neighbor != 0x000f_ffff {
                if neighbor >= navmesh.face_count {
                    anyhow::bail!(
                        "NavMesh adjacency {} edge {} references face {} outside 0..{}",
                        face_index,
                        edge,
                        neighbor,
                        navmesh.face_count
                    );
                }
                decoded[edge] = neighbor as i64;
            }
        }
        raw_adjacency.push(raw);
        adjacency.push(decoded);
    }

    let mut groups = Vec::with_capacity(navmesh.group_count as usize);
    edb.seek(SeekFrom::Start(navmesh.groups.offset_absolute()))?;
    for group_index in 0..navmesh.group_count {
        let raw = edb.read_type::<[u32; 2]>(edb.endian)?;
        let face_count = raw[0] & 0x00ff_ffff;
        let start_face = raw[1] & 0x000f_ffff;
        if start_face > navmesh.face_count
            || face_count > navmesh.face_count
            || start_face.saturating_add(face_count) > navmesh.face_count
        {
            anyhow::bail!(
                "NavMesh group {} range {}+{} exceeds {} faces",
                group_index,
                start_face,
                face_count,
                navmesh.face_count
            );
        }
        groups.push(serde_json::json!({
            "index": group_index,
            "start_face": start_face,
            "face_count": face_count,
            "flags0": raw[0] >> 24,
            "flags1": raw[1] >> 20,
            "raw": raw,
        }));
    }

    let metadata = serde_json::json!({
        "source_edb": source_edb,
        "entity_id": ent_id,
        "entity_type": "0x607",
        "class_name": "EXGeoNavMeshEntity",
        "vertex_count": navmesh.vertex_count,
        "face_count": navmesh.face_count,
        "group_count": navmesh.group_count,
        "offsets": {
            "vertices": format!("0x{:08X}", navmesh.vertices.offset_absolute()),
            "faces": format!("0x{:08X}", navmesh.faces.offset_absolute()),
            "adjacency": format!("0x{:08X}", navmesh.adjacency.offset_absolute()),
            "groups": format!("0x{:08X}", navmesh.groups.offset_absolute()),
        },
        "raw_54_68": navmesh.raw_54_68,
        "raw_88_b4": navmesh.raw_88_b4,
        "vertices": vertices,
        "faces": faces,
        "raw_faces": raw_faces,
        "adjacency": adjacency,
        "raw_adjacency": raw_adjacency,
        "groups": groups,
    });

    let json_path = output_folder.join(format!("{}_navmesh.json", ent_id));
    let json_file = File::create(json_path)?;
    serde_json::to_writer_pretty(json_file, &metadata)?;

    let mut obj = String::new();
    obj.push_str("# Robots EXGeoNavMeshEntity exported natively by patched EuroChef\n");
    obj.push_str(&format!("o navmesh_{}\n", ent_id));
    for vertex in &vertices {
        obj.push_str(&format!(
            "v {:.9} {:.9} {:.9}\n",
            vertex[0], vertex[1], vertex[2]
        ));
    }
    for _ in &vertices {
        obj.push_str("vt 0.0 0.0\n");
    }

    let mut covered = vec![false; faces.len()];
    for (group_index, group) in groups.iter().enumerate() {
        let start = group["start_face"].as_u64().unwrap_or(0) as usize;
        let count = group["face_count"].as_u64().unwrap_or(0) as usize;
        obj.push_str(&format!("g group_{:03}\n", group_index));
        for face_index in start..start.saturating_add(count).min(faces.len()) {
            let face = faces[face_index];
            obj.push_str(&format!(
                "f {}/{} {}/{} {}/{}\n",
                face[0] + 1,
                face[0] + 1,
                face[1] + 1,
                face[1] + 1,
                face[2] + 1,
                face[2] + 1
            ));
            covered[face_index] = true;
        }
    }
    if covered.iter().any(|value| !*value) {
        obj.push_str("g ungrouped\n");
        for (face_index, face) in faces.iter().enumerate() {
            if !covered[face_index] {
                obj.push_str(&format!(
                    "f {}/{} {}/{} {}/{}\n",
                    face[0] + 1,
                    face[0] + 1,
                    face[1] + 1,
                    face[1] + 1,
                    face[2] + 1,
                    face[2] + 1
                ));
            }
        }
    }
    std::fs::write(output_folder.join(format!("{}_navmesh.obj", ent_id)), obj)?;
    Ok(())
}

fn export_nonrenderable_entity<T: serde::Serialize>(
    output_folder: &Path,
    source_edb: &str,
    ent_id: &str,
    entity_type: &str,
    class_name: &str,
    data: &T,
) -> anyhow::Result<()> {
    let path = output_folder.join(format!(
        "{}_{}.json",
        ent_id,
        class_name.to_ascii_lowercase()
    ));
    let file = File::create(path)?;
    let value = serde_json::json!({
        "source_edb": source_edb,
        "entity_id": ent_id,
        "entity_type": entity_type,
        "class_name": class_name,
        "data": data,
    });
    serde_json::to_writer_pretty(file, &value)?;
    Ok(())
}
pub fn execute_command(
    filename: String,
    platform: Option<PlatformArg>,
    output_folder: Option<String>,
    dont_embed_textures: bool,
    remove_transparent: bool,
) -> anyhow::Result<()> {
    let output_folder = output_folder.unwrap_or(format!(
        "./entities/{}/",
        Path::new(&filename).file_name().unwrap().to_string_lossy()
    ));
    let output_folder = Path::new(&output_folder);

    let platform = platform
        .map(|p| p.into())
        .or(Platform::from_path(&filename))
        .expect("Failed to detect platform");

    let file = File::open(&filename)?;
    let reader = BufReader::new(file);
    let mut edb = EdbFile::new(Box::new(reader), platform)?;
    let header = edb.header.clone();
    let source_edb = Path::new(&filename)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    match platform {
        Platform::Pc
        | Platform::Xbox
        | Platform::Xbox360
        | Platform::Ps2
        | Platform::GameCube
        | Platform::Wii => {}
        _ => {
            anyhow::bail!("Entity extraction is only supported for PC, Xbox, Xbox 360, PS2, GameCube and Wii platforms")
        }
    }

    #[cfg(not(debug_assertions))]
    if platform == Platform::Ps2 {
        error!("PS2 entities are only supported through the GUI for now.");
        return Ok(());
    }

    info!("Selected platform {platform:?}");

    let mut texture_uri_map: HashMap<u32, (String, Transparency)> = HashMap::new();
    if dont_embed_textures {
        for t in &header.texture_list {
            texture_uri_map.insert(
                t.common.hashcode,
                (
                    format!("{:08x}_frame0.png", t.common.hashcode),
                    Transparency::Opaque,
                ),
            );
        }
    } else {
        let pb = ProgressBar::new(header.texture_list.len() as u64)
            .with_finish(indicatif::ProgressFinish::AndLeave);
        pb.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {msg} ({pos}/{len})",
            )
            .unwrap()
            .progress_chars("##-")
            .tick_chars(TICK_STRINGS),
        );
        pb.set_message("Extracting textures");

        let textures = UXGeoTexture::read_all(&mut edb);
        for (_, it) in textures.into_iter() {
            let hash_str = format!("0x{:x}", it.hashcode);
            let _span = error_span!("texture", hash = %hash_str);
            let _span_enter = _span.enter();

            if let Ok(t) = it.data {
                if t.frames.is_empty() {
                    error!("Skipping texture with no frames");
                    continue;
                }

                // TODO(cohae): This is very wrong, textures only specify whether they're cutout. see GUI entity renderer for more info
                // ~~cohae: This is wrong on a few levels, but it's just for transparency~~
                let flags_shift = if header.version == 248 { 0x19 } else { 0x18 };

                let is_transparent_blend = (((t.flags >> flags_shift) >> 6) & 1) != 0;
                let is_transparent_cutout = (((t.flags >> flags_shift) >> 5) & 1) != 0;
                let transparency = match (is_transparent_blend, is_transparent_cutout) {
                    (false, false) => Transparency::Opaque,
                    (true, false) => Transparency::Blend,
                    (false, true) => Transparency::Cutout,
                    _ => Transparency::Blend,
                };

                let mut cur = Cursor::new(Vec::new());
                image::write_buffer_with_format(
                    &mut cur,
                    &t.frames[0],
                    t.width as u32,
                    t.height as u32,
                    image::ColorType::Rgba8,
                    ImageFormat::Png,
                )?;

                let mut uri = "data:image/png;base64,".to_string();
                base64::engine::general_purpose::STANDARD
                    .encode_string(&cur.into_inner(), &mut uri);
                texture_uri_map.insert(it.hashcode, (uri, transparency));
            }
        }
    }

    std::fs::create_dir_all(output_folder)?;
    let mut entity_offsets: Vec<(u64, String)> = header
        .entity_list
        .iter()
        .map(|e| (e.common.address as u64, format!("{:x}", e.common.hashcode)))
        .collect();

    // Find entities in refpointers
    for (i, r) in header.refpointer_list.iter().enumerate() {
        edb.seek(std::io::SeekFrom::Start(r.address as u64))?;
        let etype = edb.read_type::<u32>(edb.endian)?;

        if etype == 0x601 || etype == 0x603 || etype == 0x607 {
            entity_offsets.push((r.address as u64, format!("ref_{i}")))
        }
    }

    let pb = ProgressBar::new(entity_offsets.len() as u64)
        .with_finish(indicatif::ProgressFinish::AndLeave);
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {msg} ({pos}/{len})",
        )
        .unwrap()
        .progress_chars("##-")
        .tick_chars(TICK_STRINGS),
    );
    pb.set_message("Extracting entities");

    for (ent_offset, ent_id) in entity_offsets.iter().progress_with(pb) {
        let _span = error_span!("entity", id = %ent_id);
        let _span_enter = _span.enter();

        edb.seek(std::io::SeekFrom::Start(*ent_offset))?;

        let ent = edb.read_type_args::<EXGeoEntity>(edb.endian, (header.version, platform));
        if let Err(err) = ent {
            error!("Failed to read entity: {err}");
            continue;
        }

        let ent = ent.unwrap();

        match &ent {
            EXGeoEntity::NavMesh(navmesh) => {
                if let Err(err) =
                    export_navmesh(&mut edb, navmesh, output_folder, &source_edb, ent_id)
                {
                    error!("Failed to export NavMesh entity: {err}");
                }
                continue;
            }
            EXGeoEntity::Instance(instance) => {
                if let Err(err) = export_nonrenderable_entity(
                    output_folder,
                    &source_edb,
                    ent_id,
                    "0x606",
                    "EXGeoInstanceEntity",
                    instance,
                ) {
                    error!("Failed to export Instance entity metadata: {err}");
                }
                continue;
            }
            EXGeoEntity::MapZone(zone) => {
                if let Err(err) = export_nonrenderable_entity(
                    output_folder,
                    &source_edb,
                    ent_id,
                    "0x608",
                    "EXGeoMapZoneEntity",
                    zone,
                ) {
                    error!("Failed to export MapZone entity metadata: {err}");
                }
                continue;
            }
            _ => {}
        }

        if let EXGeoEntity::Mesh(ref mesh) = ent {
            if mesh.data.vertex_count == 0 {
                warn!(
                    "Skipping entity without vertex data! (v={}/i={}/t={})",
                    mesh.data.vertex_count, mesh.data.index_count, mesh.data.tristrip_count
                );
                continue;
            }
        }

        let mut vertex_data = vec![];
        let mut indices = vec![];
        let mut strips = vec![];

        if let Err(err) = read_entity(
            &ent,
            &mut vertex_data,
            &mut indices,
            &mut strips,
            &mut edb,
            4,
            remove_transparent,
            true,
        ) {
            error!("Failed to extract entity: {err}");
            continue;
        }

        if strips.is_empty() {
            warn!(
                "Processed entity doesnt have tristrips! (v={}/i={}/t={})",
                vertex_data.len(),
                indices.len(),
                strips.len()
            );
            continue;
        }

        // Process vertex data (flipping vertex data and UVs)
        for v in &mut vertex_data {
            v.pos[0] = -v.pos[0];
        }

        // Look up texture hashcodes
        for t in &mut strips {
            if t.texture_index != u32::MAX {
                t.texture_index = header.texture_list[t.texture_index as usize]
                    .common
                    .hashcode;
            }
        }

        if vertex_data.is_empty() {
            warn!(
                "Processed entity doesnt have vertex data! (v={}/i={}/t={})",
                vertex_data.len(),
                indices.len(),
                strips.len()
            );
        }

        let mut gltf = gltf_export::create_mesh_scene(ent_id);
        gltf_export::add_mesh_to_scene(
            &mut gltf,
            &vertex_data,
            &indices,
            &strips,
            ![252, 250, 240, 221].contains(&header.version),
            &texture_uri_map,
            header.hashcode,
        );

        let mut outfile = File::create(output_folder.join(format!("{}.gltf", ent_id)))?;
        gltf::json::serialize::to_writer(&mut outfile, &gltf)
            .context("glTF serialization error")?;

        let mut obj_file = File::create(output_folder.join(format!("{}_ue.obj", ent_id)))?;
        export_static_mesh_obj(&mut obj_file, ent_id, &vertex_data, &indices, &strips)?;
    }

    info!("Successfully extracted entities!");

    Ok(())
}
