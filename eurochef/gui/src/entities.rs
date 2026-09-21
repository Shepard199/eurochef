use std::{collections::BTreeMap, io::Seek, ops::Range, sync::Arc};

use anyhow::anyhow;
use egui::{mutex::RwLock, Color32, RichText, Widget};
use eurochef_edb::{
    anim::EXGeoBaseAnimSkin,
    binrw::BinReaderExt,
    edb::EdbFile,
    entity::{robots_v248_isolated_surface_category, EXGeoEntity},
    versions::Platform,
    Hashcode, HashcodeUtils,
};
use eurochef_shared::{
    entities::{read_entity, TriStrip, UXVertex},
    maps::format_typed_hashcode_with_id,
    robots_runtime::monster_navigation::RobotsMonsterNavGroup,
    textures::UXGeoTexture,
    IdentifiableResult,
};
use fnv::FnvHashMap;
use font_awesome as fa;
use glam::{Quat, Vec2, Vec3};
use glow::HasContext;
use nohash_hasher::IntMap;

use crate::{
    entity_frame::{EntityFrame, RenderableTexture},
    render::{
        self, camera::ArcBallCamera, entity::EntityRenderer, gl_helper, shaders::Shaders,
        viewer::RenderContext, RenderStore, RenderUniforms,
    },
    strip_ansi_codes,
    textures::cutoff_string,
};

pub struct EntityListPanel {
    file: Hashcode,
    render_store: Arc<RwLock<RenderStore>>,
    gl: Arc<glow::Context>,
    entity_renderer: Option<EntityFrame>,
    entity_label: String,
    hashcodes: Arc<IntMap<Hashcode, String>>,

    entity_previews: FnvHashMap<u32, Option<egui::TextureHandle>>,
    // TODO(cohae): Hack to get shaders for entity previews
    shaders: Shaders,

    entities: Vec<IdentifiableResult<(EXGeoEntity, ProcessedEntityMesh)>>,
    skins: Vec<IdentifiableResult<EXGeoBaseAnimSkin>>,
    ref_entities: Vec<IdentifiableResult<(EXGeoEntity, ProcessedEntityMesh)>>,
    framebuffer: (glow::Framebuffer, glow::Texture),
    framebuffer_msaa: (glow::Framebuffer, glow::Texture),

    /// Preview thumbnail width, in pixels
    preview_size: i32,

    platform: Platform,
}

#[derive(Debug, Clone, Copy)]
pub struct RobotsSurfaceTriangle {
    pub positions: [Vec3; 3],
    pub surface_mask: u16,
}

/// Exact Robots PC-v248 `EXGeoEntity::DoRayCast` face input. Unlike
/// `RobotsSurfaceTriangle`, this preserves every serialized 10-byte face,
/// including faces whose metadata word is zero.
#[derive(Debug, Clone, Copy)]
pub struct RobotsRaycastTriangle {
    pub positions: [Vec3; 3],
    pub face_mask: u16,
    pub trailing_raw: u16,
}

/// Immutable gameplay topology decoded from Robots v248 `EXGeoNavMeshEntity`.
/// Rendering keeps using the existing flattened vertex/strip buffers; AI navigation
/// consumes this view so renderer and gameplay do not own duplicate geometry.
#[derive(Debug, Clone, Default)]
pub struct ProcessedNavMesh {
    pub vertices: Vec<[f32; 3]>,
    pub faces: Vec<[u32; 3]>,
    pub adjacency: Vec<[Option<u32>; 3]>,
    pub groups: Vec<RobotsMonsterNavGroup>,
}

#[derive(Clone)]
pub struct ProcessedEntityMesh {
    pub vertex_data: Vec<UXVertex>,
    pub indices: Vec<u32>,
    pub strips: Vec<TriStrip>,
    pub flags: u32,
    pub is_navmesh: bool,
    pub part_vertex_ranges: Vec<Range<usize>>,
    /// Nonzero Robots v248 `face_info` metadata masks, aggregated across mesh parts.
    pub robots_surface_mask_counts: BTreeMap<u16, usize>,
    /// Exact nonzero native surface faces expressed in the same source vertex coordinates as the renderer.
    pub robots_surface_triangles: Vec<RobotsSurfaceTriangle>,
    /// Complete native face stream consumed by `EXGeoEntity::DoRayCast` at
    /// Robots.exe `0x005182B4`, including zero-metadata faces.
    pub robots_raycast_triangles: Vec<RobotsRaycastTriangle>,
    /// Native 0x607 topology retained for monster/NPC navigation. Split entities
    /// can contain more than one NavMesh child, hence the outer vector.
    pub robots_navmeshes: Vec<ProcessedNavMesh>,
}

mod panel;

fn collect_mesh_vertex_ranges(
    entity: &EXGeoEntity,
    vertex_offset: &mut usize,
    ranges: &mut Vec<Range<usize>>,
) {
    match entity {
        EXGeoEntity::Mesh(mesh) => {
            let start = *vertex_offset;
            *vertex_offset += mesh.vertices.len();
            ranges.push(start..*vertex_offset);
        }
        EXGeoEntity::Split(split) => {
            for child in &split.entities {
                collect_mesh_vertex_ranges(child, vertex_offset, ranges);
            }
        }
        _ => {}
    }
}

fn collect_robots_surface_mask_counts(entity: &EXGeoEntity, counts: &mut BTreeMap<u16, usize>) {
    match entity {
        EXGeoEntity::Mesh(mesh) => {
            if let Some(face_info) = &mesh.robots_face_info {
                for group in &face_info.groups {
                    for face in &group.faces {
                        let mask = face.surface_metadata & 0x78;
                        if mask != 0 {
                            *counts.entry(mask).or_default() += 1;
                        }
                    }
                }
            }
        }
        EXGeoEntity::Split(split) => {
            for child in &split.entities {
                collect_robots_surface_mask_counts(child, counts);
            }
        }
        _ => {}
    }
}

fn collect_robots_surface_triangles(
    entity: &EXGeoEntity,
    triangles: &mut Vec<RobotsSurfaceTriangle>,
) {
    match entity {
        EXGeoEntity::Mesh(mesh) => {
            let Some(face_info) = &mesh.robots_face_info else {
                return;
            };
            for group in &face_info.groups {
                for face in &group.faces {
                    let surface_mask = face.surface_metadata & 0x78;
                    if surface_mask == 0 {
                        continue;
                    }
                    let [a, b, c] = face.vertex_indices.map(usize::from);
                    let (Some(a), Some(b), Some(c)) = (
                        mesh.vertices.get(a),
                        mesh.vertices.get(b),
                        mesh.vertices.get(c),
                    ) else {
                        continue;
                    };
                    triangles.push(RobotsSurfaceTriangle {
                        positions: [Vec3::from(a.pos), Vec3::from(b.pos), Vec3::from(c.pos)],
                        surface_mask,
                    });
                }
            }
        }
        EXGeoEntity::Split(split) => {
            for child in &split.entities {
                collect_robots_surface_triangles(child, triangles);
            }
        }
        _ => {}
    }
}

fn collect_robots_raycast_triangles(
    entity: &EXGeoEntity,
    triangles: &mut Vec<RobotsRaycastTriangle>,
) {
    match entity {
        EXGeoEntity::Mesh(mesh) => {
            let Some(face_info) = &mesh.robots_face_info else {
                return;
            };
            for group in &face_info.groups {
                for face in &group.faces {
                    let [a, b, c] = face.vertex_indices.map(usize::from);
                    let (Some(a), Some(b), Some(c)) = (
                        mesh.vertices.get(a),
                        mesh.vertices.get(b),
                        mesh.vertices.get(c),
                    ) else {
                        continue;
                    };
                    triangles.push(RobotsRaycastTriangle {
                        positions: [Vec3::from(a.pos), Vec3::from(b.pos), Vec3::from(c.pos)],
                        face_mask: face.surface_metadata,
                        trailing_raw: face.trailing_raw,
                    });
                }
            }
        }
        EXGeoEntity::Split(split) => {
            for child in &split.entities {
                collect_robots_raycast_triangles(child, triangles);
            }
        }
        _ => {}
    }
}

fn collect_robots_navmeshes(
    entity: &EXGeoEntity,
    edb: &mut EdbFile,
    out: &mut Vec<ProcessedNavMesh>,
) -> anyhow::Result<()> {
    const INDEX_MASK: u32 = 0x000f_ffff;
    match entity {
        EXGeoEntity::NavMesh(navmesh) => {
            let restore = edb.stream_position()?;

            edb.seek(std::io::SeekFrom::Start(navmesh.vertices.offset_absolute()))?;
            let mut vertices = Vec::with_capacity(navmesh.vertex_count as usize);
            for _ in 0..navmesh.vertex_count {
                vertices.push(edb.read_type::<[f32; 3]>(edb.endian)?);
            }

            edb.seek(std::io::SeekFrom::Start(navmesh.faces.offset_absolute()))?;
            let mut faces = Vec::with_capacity(navmesh.face_count as usize);
            for face_index in 0..navmesh.face_count {
                let raw = edb.read_type::<[u32; 4]>(edb.endian)?;
                let face = [
                    raw[0] & INDEX_MASK,
                    raw[1] & INDEX_MASK,
                    raw[2] & INDEX_MASK,
                ];
                if face.iter().any(|index| *index >= navmesh.vertex_count) {
                    anyhow::bail!(
                        "Robots NavMesh face {face_index} references vertex outside 0..{}: {face:?}",
                        navmesh.vertex_count
                    );
                }
                faces.push(face);
            }

            edb.seek(std::io::SeekFrom::Start(
                navmesh.adjacency.offset_absolute(),
            ))?;
            let mut adjacency = Vec::with_capacity(navmesh.face_count as usize);
            for face_index in 0..navmesh.face_count {
                let raw = edb.read_type::<[u32; 3]>(edb.endian)?;
                let mut decoded = [None; 3];
                for edge in 0..3 {
                    let neighbor = raw[edge] & INDEX_MASK;
                    if neighbor != INDEX_MASK {
                        if neighbor >= navmesh.face_count {
                            anyhow::bail!(
                                "Robots NavMesh adjacency {face_index} edge {edge} references face {neighbor} outside 0..{}",
                                navmesh.face_count
                            );
                        }
                        decoded[edge] = Some(neighbor);
                    }
                }
                adjacency.push(decoded);
            }

            edb.seek(std::io::SeekFrom::Start(navmesh.groups.offset_absolute()))?;
            let mut groups = Vec::with_capacity(navmesh.group_count as usize);
            for group_index in 0..navmesh.group_count {
                let raw = edb.read_type::<[u32; 2]>(edb.endian)?;
                let face_count = raw[0] & 0x00ff_ffff;
                let start_face = raw[1] & INDEX_MASK;
                if start_face > navmesh.face_count
                    || face_count > navmesh.face_count
                    || start_face.saturating_add(face_count) > navmesh.face_count
                {
                    anyhow::bail!(
                        "Robots NavMesh group {group_index} range {start_face}+{face_count} exceeds {} faces",
                        navmesh.face_count
                    );
                }
                groups.push(RobotsMonsterNavGroup {
                    start_face,
                    face_count,
                    flags0: (raw[0] >> 24) as u8,
                    flags1: (raw[1] >> 20) as u16,
                });
            }

            out.push(ProcessedNavMesh {
                vertices,
                faces,
                adjacency,
                groups,
            });
            edb.seek(std::io::SeekFrom::Start(restore))?;
        }
        EXGeoEntity::Split(split) => {
            for child in &split.entities {
                collect_robots_navmeshes(child, edb, out)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub(crate) fn robots_surface_mask_label(mask: u16) -> String {
    let (category, role) = robots_v248_isolated_surface_category(mask);
    if category == 0 {
        format!("0x{mask:02X} / no special category in isolation")
    } else {
        format!("0x{mask:02X} -> 0x{category:04X} / {role}")
    }
}

fn apply_navmesh_uv(vertices: &mut [UXVertex]) {
    for vertex in vertices {
        vertex.uv = [vertex.pos[0], vertex.pos[2]];
    }
}

impl ProcessedEntityMesh {
    pub fn bounding_box(&self) -> (Vec3, Vec3) {
        if self.vertex_data.is_empty() {
            return (Vec3::ZERO, Vec3::ZERO);
        }

        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        for v in &self.vertex_data {
            min = min.min(v.pos.into());
            max = max.max(v.pos.into());
        }

        (min, max)
    }
}

impl EntityListPanel {
    pub fn new(
        file: Hashcode,
        render_store: Arc<RwLock<RenderStore>>,
        ctx: &egui::Context,
        gl: Arc<glow::Context>,
        entities: Vec<IdentifiableResult<(EXGeoEntity, ProcessedEntityMesh)>>,
        skins: Vec<IdentifiableResult<EXGeoBaseAnimSkin>>,
        ref_entities: Vec<IdentifiableResult<(EXGeoEntity, ProcessedEntityMesh)>>,
        hashcodes: Arc<IntMap<Hashcode, String>>,
        platform: Platform,
    ) -> Self {
        let mut entity_previews = FnvHashMap::default();
        for ires in entities.iter().filter(|ir| ir.data.is_ok()) {
            entity_previews.insert(ires.hashcode, None);
        }
        for ires in skins.iter().filter(|ir| ir.data.is_ok()) {
            entity_previews.insert(ires.hashcode, None);
        }
        for ires in ref_entities.iter().filter(|ir| ir.data.is_ok()) {
            entity_previews.insert(ires.hashcode, None);
        }

        let preview_size = (256.0 * ctx.pixels_per_point()) as i32;

        #[cfg(not(target_family = "wasm"))]
        let framebuffer_msaa = unsafe { Self::create_preview_framebuffer(&gl, true, preview_size) };
        #[cfg(target_family = "wasm")]
        let framebuffer_msaa =
            unsafe { Self::create_preview_framebuffer(&gl, false, preview_size) };

        EntityListPanel {
            file,
            render_store,
            framebuffer_msaa,
            framebuffer: unsafe { Self::create_preview_framebuffer(&gl, false, preview_size) },
            shaders: Shaders::load_shaders(&gl),
            gl,
            entity_renderer: None,
            entity_label: String::new(),
            hashcodes,
            entities,
            skins,
            ref_entities,
            entity_previews,
            preview_size,
            platform,
        }
    }

    // TODO(cohae): Move
    pub fn load_textures(
        gl: &glow::Context,
        owner_edb_uid: Hashcode,
        textures: &[(usize, IdentifiableResult<UXGeoTexture>)],
    ) -> Vec<(usize, RenderableTexture)> {
        let mut exact_groups: FnvHashMap<String, RenderableTexture> = FnvHashMap::default();
        let mut output = Vec::with_capacity(textures.len());

        for (i, it) in textures {
            let renderable = unsafe {
                if let Ok(t) = &it.data {
                    let identity = eurochef_edb::robots_texture_identity::active_record(
                        owner_edb_uid,
                        it.hashcode,
                    );
                    if let Some(identity) = &identity {
                        if let Some(existing) = exact_groups.get(&identity.exact_group) {
                            let mut reused = existing.clone();
                            reused.hashcode = it.hashcode;
                            output.push((*i, reused));
                            continue;
                        }
                    }

                    let mut frames = vec![];
                    for d in &t.frames {
                        let handle = gl_helper::load_texture(
                            gl,
                            t.width as i32,
                            t.height as i32,
                            d,
                            glow::RGBA,
                            t.flags,
                        );
                        frames.push(handle);
                    }

                    let renderable = RenderableTexture {
                        external_reference: t.external_texture,
                        frames,
                        framerate: t.framerate as usize,
                        frame_count: t.frame_count as usize,
                        flags: t.flags,
                        // EngineX(T) calculates these as step per frame by dividing each axis by 30000. We're calculating this with seconds instead of frames
                        scroll: Vec2::new(t.scroll[0] as f32 / 500.0, t.scroll[1] as f32 / 500.0),
                        hashcode: it.hashcode,
                    };
                    if let Some(identity) = identity {
                        exact_groups.insert(identity.exact_group, renderable.clone());
                    }
                    renderable
                } else {
                    let handle = gl_helper::load_texture(
                        gl,
                        2,
                        2,
                        &[
                            255, 0, 255, 255, 255, 0, 255, 255, 255, 0, 255, 255, 255, 0, 255, 255,
                        ],
                        glow::RGBA,
                        0,
                    );

                    RenderableTexture {
                        external_reference: None,
                        frames: vec![handle],
                        framerate: 0,
                        frame_count: 0,
                        flags: 0,
                        scroll: Vec2::ZERO,
                        hashcode: it.hashcode,
                    }
                }
            };
            output.push((*i, renderable));
        }

        output
    }
}
fn entity_is_requested(
    entity_index: usize,
    entity_hashcode: Hashcode,
    requested: &[Hashcode],
) -> bool {
    requested.iter().any(|hashcode| {
        if hashcode.is_local() {
            hashcode.index() as usize == entity_index
        } else {
            *hashcode == entity_hashcode
        }
    })
}

/// Leave hashcodes empty to load all entities
pub fn read_from_file(
    edb: &mut EdbFile,
    hashcodes: Option<&[Hashcode]>,
) -> anyhow::Result<(
    Vec<(
        usize,
        IdentifiableResult<(EXGeoEntity, ProcessedEntityMesh)>,
    )>,
    Vec<IdentifiableResult<EXGeoBaseAnimSkin>>,
    Vec<IdentifiableResult<(EXGeoEntity, ProcessedEntityMesh)>>,
)> {
    let header = edb.header.clone();

    // ROBOTS_PATCH_0024_REQUESTED_ANIMSKIN_ENTITIES
    // AnimSkin records reference component entities by entity-list index. When a
    // script requests an AnimSkin, load both the skin descriptor and those meshes.
    let mut skins = vec![];
    let mut skin_entity_indices: Vec<usize> = vec![];

    for s in header.animskin_list.iter() {
        let selected = hashcodes
            .map(|requested| requested.contains(&s.common.hashcode))
            .unwrap_or(true);

        if !selected {
            continue;
        }

        edb.seek(std::io::SeekFrom::Start(s.common.address as u64))?;
        let skin = edb.read_type_args::<EXGeoBaseAnimSkin>(edb.endian, (edb.header.version,));

        if let Ok(parsed) = &skin {
            for entry in parsed.entities.iter().chain(parsed.more_entities.iter()) {
                let index = entry.entity_list_index();
                if index < header.entity_list.len() && !skin_entity_indices.contains(&index) {
                    skin_entity_indices.push(index);
                }
            }
        }

        skins.push(IdentifiableResult::new(
            s.common.hashcode,
            match skin {
                Ok(skin) => Ok(skin),
                Err(e) => Err(anyhow!("Failed to read animskin: {e:?}")),
            },
        ));
    }

    let mut entities = vec![];
    for (i, e) in header.entity_list.iter().enumerate().filter(|(i, c)| {
        if let Some(hashcodes) = hashcodes {
            entity_is_requested(*i, c.common.hashcode, hashcodes) || skin_entity_indices.contains(i)
        } else {
            true
        }
    }) {
        let ent = read_entity_identifiable(e.common.address, edb);
        entities.push((i, IdentifiableResult::new(e.common.hashcode, ent)));
    }

    let mut refents = vec![];
    if hashcodes.is_none() {
        for (i, r) in header.refpointer_list.iter().enumerate() {
            edb.seek(std::io::SeekFrom::Start(r.address as u64))?;

            let etype = edb.read_type::<u32>(edb.endian)?;
            if etype == 0x601 || etype == 0x602 || etype == 0x603 {
                let ent = read_entity_identifiable(r.address, edb);
                refents.push(IdentifiableResult::new(i as _, ent));
            }
        }
    }

    Ok((entities, skins, refents))
}

fn read_entity_identifiable(
    address: u32,
    edb: &mut EdbFile,
) -> anyhow::Result<(EXGeoEntity, ProcessedEntityMesh)> {
    edb.seek(std::io::SeekFrom::Start(address as u64))?;

    let ent = edb.read_type_args(edb.endian, (edb.header.version, edb.platform))?;

    let mut vertex_data = vec![];
    let mut indices = vec![];
    let mut strips = vec![];
    read_entity(
        &ent,
        &mut vertex_data,
        &mut indices,
        &mut strips,
        edb,
        4,
        false,
        false,
    )?;

    let mut part_vertex_ranges = Vec::new();
    let mut part_vertex_offset = 0usize;
    collect_mesh_vertex_ranges(&ent, &mut part_vertex_offset, &mut part_vertex_ranges);
    if part_vertex_offset != vertex_data.len() {
        // GX entities can duplicate vertices while decoding indexed attributes.
        // Robots PC meshes preserve one contiguous source range per mesh part.
        part_vertex_ranges.clear();
    }

    let flags = ent.base().map(|b| b.flags).unwrap_or_default();
    let mut robots_surface_mask_counts = BTreeMap::new();
    collect_robots_surface_mask_counts(&ent, &mut robots_surface_mask_counts);
    let mut robots_surface_triangles = Vec::new();
    collect_robots_surface_triangles(&ent, &mut robots_surface_triangles);
    let mut robots_raycast_triangles = Vec::new();
    collect_robots_raycast_triangles(&ent, &mut robots_raycast_triangles);
    let mut robots_navmeshes = Vec::new();
    collect_robots_navmeshes(&ent, edb, &mut robots_navmeshes)?;
    let is_navmesh = strips.iter().any(|strip| strip.is_navmesh);
    if is_navmesh {
        apply_navmesh_uv(&mut vertex_data);
    }

    Ok((
        ent,
        ProcessedEntityMesh {
            vertex_data,
            indices,
            strips,
            flags,
            is_navmesh,
            part_vertex_ranges,
            robots_surface_mask_counts,
            robots_surface_triangles,
            robots_raycast_triangles,
            robots_navmeshes,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_entity_references_match_entity_list_indices() {
        let requested = [0x8200_0000, 0x8200_0001, 0x8200_0002];
        assert!(entity_is_requested(0, 0x0200_017A, &requested));
        assert!(entity_is_requested(1, 0x0200_017B, &requested));
        assert!(entity_is_requested(2, 0x0200_01AE, &requested));
        assert!(!entity_is_requested(3, 0x0200_0000, &requested));
    }

    #[test]
    fn global_entity_references_still_match_hashcodes() {
        let requested = [0x0200_01AE];
        assert!(entity_is_requested(99, 0x0200_01AE, &requested));
        assert!(!entity_is_requested(2, 0x0200_017A, &requested));
    }

    #[test]
    fn zero_geometry_anchor_has_finite_zero_bounds() {
        let mesh = ProcessedEntityMesh {
            vertex_data: vec![],
            indices: vec![],
            strips: vec![],
            flags: 0,
            is_navmesh: false,
            part_vertex_ranges: vec![],
            robots_surface_mask_counts: BTreeMap::new(),
            robots_surface_triangles: Vec::new(),
            robots_raycast_triangles: Vec::new(),
            robots_navmeshes: Vec::new(),
        };

        assert_eq!(mesh.bounding_box(), (Vec3::ZERO, Vec3::ZERO));
    }

    #[test]
    fn navmesh_uvs_preserve_world_xz_for_dynamic_scaling() {
        let mut vertices = vec![UXVertex {
            pos: [32.0, 7.0, -16.0],
            norm: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
            color: [1.0; 4],
        }];

        apply_navmesh_uv(&mut vertices);

        assert_eq!(vertices[0].uv, [32.0, -16.0]);
    }
}
