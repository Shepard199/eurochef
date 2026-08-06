use std::{io::Seek, ops::Range, sync::Arc};

use anyhow::anyhow;
use egui::{mutex::RwLock, Color32, RichText, Widget};
use eurochef_edb::{
    anim::EXGeoBaseAnimSkin, binrw::BinReaderExt, edb::EdbFile, entity::EXGeoEntity,
    versions::Platform, Hashcode, HashcodeUtils,
};
use eurochef_shared::{
    entities::{read_entity, TriStrip, UXVertex},
    maps::format_hashcode_with_id,
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

#[derive(Clone)]
pub struct ProcessedEntityMesh {
    pub vertex_data: Vec<UXVertex>,
    pub indices: Vec<u32>,
    pub strips: Vec<TriStrip>,
    pub flags: u32,
    pub is_navmesh: bool,
    pub part_vertex_ranges: Vec<Range<usize>>,
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
        textures: &[(usize, IdentifiableResult<UXGeoTexture>)],
    ) -> Vec<(usize, RenderableTexture)> {
        textures
            .iter()
            .map(|(i, it)| unsafe {
                if let Ok(t) = &it.data {
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

                    (
                        *i,
                        RenderableTexture {
                            external_reference: t.external_texture,
                            frames,
                            framerate: t.framerate as usize,
                            frame_count: t.frame_count as usize,
                            flags: t.flags,
                            // EngineX(T) calculates these as step per frame by dividing each axis by 30000. We're calculating this with seconds instead of frames
                            scroll: Vec2::new(
                                t.scroll[0] as f32 / 500.0,
                                t.scroll[1] as f32 / 500.0,
                            ),
                            hashcode: it.hashcode,
                        },
                    )
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

                    (
                        *i,
                        RenderableTexture {
                            external_reference: None,
                            frames: vec![handle],
                            framerate: 0,
                            frame_count: 0,
                            flags: 0,
                            scroll: Vec2::ZERO,
                            hashcode: it.hashcode,
                        },
                    )
                }
            })
            .collect()
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
                let index = (entry.entity_index & 0x00ff_ffff) as usize;
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
