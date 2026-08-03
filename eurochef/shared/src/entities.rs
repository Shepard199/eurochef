use std::io::{Read, Seek};

use bytemuck::{Pod, Zeroable};
use eurochef_edb::edb::EdbFile;
use eurochef_edb::{
    binrw::BinReaderExt,
    common::{EXVector, EXVector2, EXVector3},
    edb::DatabaseReader,
    entity::EXGeoEntity,
    entity_mesh::EXGeoEntityTriStrip,
    versions::Platform,
};
use tracing::error;

#[derive(Debug, Clone, Copy)]
pub struct TriStrip {
    pub start_index: u32,
    pub index_count: u32,
    pub texture_index: u32,
    pub transparency: u16,
    pub flags: u16,
    pub tri_count: u32,
    pub is_navmesh: bool,
}

// ROBOTS_PATCH_0015_MODERN_RUST_WARNINGS
// ROBOTS_PATCH_0016_BYTEMUCK_DERIVE_REFRESH
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
#[repr(C)]
pub struct UXVertex {
    pub pos: EXVector3,
    pub norm: EXVector3,
    pub uv: EXVector2,
    pub color: EXVector,
}

fn read_robots_instance_strip(
    instance: &eurochef_edb::entity::EXGeoInstanceEntity,
    vertex_data: &mut Vec<UXVertex>,
    indices: &mut Vec<u32>,
    strips: &mut Vec<TriStrip>,
    edb: &mut EdbFile,
    convert_strips: bool,
) -> anyhow::Result<bool> {
    // ROBOTS_PATCH_0025_READ_INSTANCE_STRIP
    const MAX_PRIMITIVES: u32 = 1_000_000;

    let primitive_count = instance.robots_v248_primitive_count;
    if primitive_count == 0 || primitive_count > MAX_PRIMITIVES {
        return Ok(false);
    }

    let vertex_count = primitive_count.saturating_add(2);
    if instance.robots_v248_vertices.len() != vertex_count as usize {
        return Ok(false);
    }

    let texture_index = instance.robots_texture_index();
    if texture_index >= edb.header.texture_list.len() {
        tracing::warn!(
            "Robots 0x606 texture selector {} outside texture list 0..{}",
            texture_index,
            edb.header.texture_list.len()
        );
        return Ok(false);
    }

    // The selector is an index into the current EDB texture list. Load that texture
    // through the same RenderStore path used by ordinary mesh strips.
    let texture_hashcode = edb.header.texture_list[texture_index].common.hashcode;
    edb.add_reference_internal(texture_hashcode);

    let vertex_base = vertex_data.len() as u32;
    for vertex in &instance.robots_v248_vertices {
        vertex_data.push(UXVertex {
            pos: vertex.position,
            norm: vertex.normal,
            uv: vertex.uv,
            color: [
                vertex.color[0] as f32 / 255.0,
                vertex.color[1] as f32 / 255.0,
                vertex.color[2] as f32 / 255.0,
                vertex.color[3] as f32 / 255.0,
            ],
        });
    }

    let start_index = indices.len() as u32;

    if convert_strips {
        for i in 0..primitive_count {
            let tri = if i % 2 == 0 {
                [vertex_base + i + 2, vertex_base + i + 1, vertex_base + i]
            } else {
                [vertex_base + i, vertex_base + i + 1, vertex_base + i + 2]
            };
            if tri[0] != tri[1] && tri[1] != tri[2] && tri[0] != tri[2] {
                indices.extend(tri);
            }
        }
    } else {
        indices.extend((0..vertex_count).map(|i| vertex_base + i));
    }

    // Slot-3 render state proof:
    //   mode 1 -> ADD, SRCALPHA, ONE
    //   mode 2 -> REVERSE_SUBTRACT, SRCALPHA, ONE
    //   other  -> ADD, SRCALPHA, INVSRCALPHA
    // TriStrip flags/transparency map to the same three EuroChef blend paths.
    let (transparency, flags) = match instance.robots_blend_mode() {
        1 => (1, 0),
        2 => (2, 0),
        _ => (0, 0x8),
    };

    strips.push(TriStrip {
        start_index,
        index_count: if convert_strips {
            indices.len() as u32 - start_index
        } else {
            vertex_count
        },
        texture_index: texture_index as u32,
        transparency,
        // 0x40 disables culling in EuroChef. The original 0x606 path is an
        // immediate-mode effect strip; render it two-sided to avoid losing one face
        // when old D3D state setup differs from the generic mesh path.
        flags: flags | 0x40,
        tri_count: if convert_strips {
            (indices.len() as u32 - start_index) / 3
        } else {
            primitive_count
        },
        is_navmesh: false,
    });

    Ok(true)
}
fn read_robots_instance_bounds(
    instance: &eurochef_edb::entity::EXGeoBaseEntity,
    vertex_data: &mut Vec<UXVertex>,
    indices: &mut Vec<u32>,
    strips: &mut Vec<TriStrip>,
    convert_strips: bool,
) -> anyhow::Result<()> {
    // Diagnostic only: these are the serialized EXGeoBaseEntity bounds. The remaining
    // Robots v248 0x606 payload/transform fields are not semantically proven yet, so do
    // not claim this box is a reconstructed placed instance mesh.
    let a = instance.bounds_box[0];
    let b = instance.bounds_box[1];
    let min = [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])];
    let max = [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])];

    if min.iter().chain(max.iter()).any(|v| !v.is_finite()) {
        return Ok(());
    }

    let corners = [
        [min[0], min[1], min[2]],
        [max[0], min[1], min[2]],
        [max[0], max[1], min[2]],
        [min[0], max[1], min[2]],
        [min[0], min[1], max[2]],
        [max[0], min[1], max[2]],
        [max[0], max[1], max[2]],
        [min[0], max[1], max[2]],
    ];
    let triangles: [[u32; 3]; 12] = [
        [0, 1, 2],
        [0, 2, 3],
        [4, 6, 5],
        [4, 7, 6],
        [0, 4, 5],
        [0, 5, 1],
        [1, 5, 6],
        [1, 6, 2],
        [2, 6, 7],
        [2, 7, 3],
        [3, 7, 4],
        [3, 4, 0],
    ];

    let vertex_base = vertex_data.len() as u32;
    for pos in corners {
        vertex_data.push(UXVertex {
            pos,
            norm: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
            color: [1.0, 0.55, 0.10, 1.0],
        });
    }

    let start = indices.len() as u32;
    for tri in triangles {
        indices.extend([
            vertex_base + tri[0],
            vertex_base + tri[1],
            vertex_base + tri[2],
        ]);
    }

    if convert_strips {
        strips.push(TriStrip {
            start_index: start,
            index_count: 36,
            texture_index: u32::MAX,
            transparency: 0,
            flags: 0,
            tri_count: 12,
            is_navmesh: false,
        });
    } else {
        for i in 0..12u32 {
            strips.push(TriStrip {
                start_index: start + i * 3,
                index_count: 3,
                texture_index: u32::MAX,
                transparency: 0,
                flags: 0,
                tri_count: 1,
                is_navmesh: false,
            });
        }
    }

    Ok(())
}
fn read_robots_navmesh(
    navmesh: &eurochef_edb::entity::EXGeoNavMeshEntity,
    vertex_data: &mut Vec<UXVertex>,
    indices: &mut Vec<u32>,
    strips: &mut Vec<TriStrip>,
    edb: &mut EdbFile,
    convert_strips: bool,
) -> anyhow::Result<()> {
    const INDEX_MASK: u32 = 0x000f_ffff;
    const MAX_VERTICES: u32 = 2_000_000;
    const MAX_FACES: u32 = 2_000_000;

    if navmesh.vertex_count > MAX_VERTICES || navmesh.face_count > MAX_FACES {
        anyhow::bail!(
            "Robots NavMesh counts are implausible: vertices={}, faces={}",
            navmesh.vertex_count,
            navmesh.face_count
        );
    }

    let vertex_base = vertex_data.len() as u32;

    edb.seek(std::io::SeekFrom::Start(navmesh.vertices.offset_absolute()))?;
    for _ in 0..navmesh.vertex_count {
        let pos: [f32; 3] = edb.read_type(edb.endian)?;
        vertex_data.push(UXVertex {
            pos,
            norm: [0.0, 1.0, 0.0],
            // Preserve world X/Z so the GUI can change texture scale without reparsing.
            uv: [pos[0], pos[2]],
            // Debug cyan. NavMesh has no proved render material in the v248 layout.
            color: [0.20, 0.80, 1.00, 1.00],
        });
    }

    edb.seek(std::io::SeekFrom::Start(navmesh.faces.offset_absolute()))?;
    let triangle_list_start = indices.len() as u32;

    for face_index in 0..navmesh.face_count {
        let raw: [u32; 4] = edb.read_type(edb.endian)?;
        let face = [
            raw[0] & INDEX_MASK,
            raw[1] & INDEX_MASK,
            raw[2] & INDEX_MASK,
        ];

        if face.iter().any(|index| *index >= navmesh.vertex_count) {
            anyhow::bail!(
                "Robots NavMesh face {} references vertex outside 0..{}: {:?}",
                face_index,
                navmesh.vertex_count,
                face
            );
        }

        let face_start = indices.len() as u32;
        indices.extend([
            vertex_base + face[0],
            vertex_base + face[1],
            vertex_base + face[2],
        ]);

        // The GUI path consumes triangle strips. A 3-index strip is exactly one
        // triangle, so keep one tiny strip per face when strip conversion is off.
        if !convert_strips {
            strips.push(TriStrip {
                start_index: face_start,
                index_count: 3,
                texture_index: u32::MAX,
                transparency: 0,
                flags: 0,
                tri_count: 1,
                is_navmesh: true,
            });
        }
    }

    // CLI/glTF path asks for converted triangles. One logical batch is enough.
    if convert_strips && navmesh.face_count != 0 {
        strips.push(TriStrip {
            start_index: triangle_list_start,
            index_count: navmesh.face_count * 3,
            texture_index: u32::MAX,
            transparency: 0,
            flags: 0,
            tri_count: navmesh.face_count,
            is_navmesh: true,
        });
    }

    Ok(())
}
pub fn read_entity(
    ent: &EXGeoEntity,
    vertex_data: &mut Vec<UXVertex>,
    indices: &mut Vec<u32>,
    strips: &mut Vec<TriStrip>,
    edb: &mut EdbFile,
    depth_limit: u32,
    remove_transparent: bool,
    convert_strips: bool,
) -> anyhow::Result<()> {
    if depth_limit == 0 {
        anyhow::bail!("Entity recursion limit reached!");
    }

    // ROBOTS_PATCH_0009_INSTANCE_0X606_EARLY_GUARD
    // PATCH_0025: 0x606 is now decoded as its real inline textured triangle strip.
    // Keep the old bounds box only as an opt-in fallback when the serialized stream
    // cannot be validated.
    if ent.type_code() == 0x606 {
        if let EXGeoEntity::Instance(instance) = ent {
            let rendered = read_robots_instance_strip(
                instance,
                vertex_data,
                indices,
                strips,
                edb,
                convert_strips,
            )?;

            if !rendered && eurochef_edb::entity::robots_instance_bounds_visible() {
                read_robots_instance_bounds(
                    &instance.base,
                    vertex_data,
                    indices,
                    strips,
                    convert_strips,
                )?;
            }
        }
        return Ok(());
    }
    match ent {
        EXGeoEntity::Split(split) => {
            for e in split.entities.iter() {
                read_entity(
                    e,
                    vertex_data,
                    indices,
                    strips,
                    edb,
                    depth_limit - 1,
                    remove_transparent,
                    convert_strips,
                )?;
            }
        }
        EXGeoEntity::Mesh(mesh) => {
            if let Some(edb) = edb.downcast_to_edbfile() {
                for v in &mesh.texture_list {
                    edb.add_reference_internal(
                        edb.header.texture_list[*v as usize].common.hashcode,
                    );
                }
            }

            if edb.platform == Platform::Ps2 {
                panic!("PS2 support is disabled");
            }

            let vertex_colors = if edb.platform.is_gx() {
                vec![[0.5, 0.5, 0.5, 1.0]; mesh.vertices.len()]
            } else {
                mesh.vertex_colors
                    .iter()
                    .map(|c| {
                        [
                            c[0] as f32 / 255.0,
                            c[1] as f32 / 255.0,
                            c[2] as f32 / 255.0,
                            c[3] as f32 / 255.0,
                        ]
                    })
                    .collect()
            };

            let vertex_offset = vertex_data.len() as u32;
            let mut new_indices: Vec<u32> = vec![];
            let mut tristrips = vec![];
            vertex_data.extend(
                mesh.vertices
                    .iter()
                    .zip(vertex_colors)
                    .map(|(v, c)| UXVertex {
                        pos: v.pos,
                        norm: v.normal,
                        uv: v.uv,
                        color: c,
                    }),
            );

            if edb.platform.is_gx() {
                // Move the vertices out of the main array, as we have to rebuild them
                let original_verts = vertex_data[vertex_offset as usize..].to_vec();
                vertex_data.drain(vertex_offset as usize..);
                for s in &mesh.tristrips_gx {
                    struct GxIndex {
                        pos: u16,
                        _unk0: u16,
                        color: u16,
                        uv: u16,
                    }

                    let mut converted_indices = vec![];
                    let mut offset = 0;
                    while offset < s.indices.len() {
                        let h = s.indices[offset];
                        let face_count = s.indices[offset + 1];

                        if h != 0x98 {
                            break;
                        }
                        offset += 2;
                        let mut temp = vec![];
                        let chunk: &[[u16; 4]] = bytemuck::cast_slice(
                            s.indices[offset..offset + face_count as usize * 4].as_ref(),
                        );
                        for c in chunk {
                            temp.push(GxIndex {
                                pos: c[0],
                                _unk0: c[1],
                                color: c[2],
                                uv: c[3],
                            });
                        }
                        offset += face_count as usize * 4;

                        converted_indices.push(temp);
                    }

                    let mut index_count = 0;
                    let start_index = new_indices.len();
                    for cv in converted_indices.into_iter() {
                        if index_count != 0 {
                            new_indices.push(vertex_data.len() as u32 - 1 - vertex_offset);
                            new_indices.push(vertex_data.len() as u32 - vertex_offset);
                            index_count += 2;
                        }

                        for c in cv {
                            let original_vert = original_verts[c.pos as usize];

                            // TODO(cohae): The only way we can know the amount of vertex colors is by iterating through all indices. This is something for the entity handling rewrite.
                            let mut color = [0u8; 4];
                            edb.seek(std::io::SeekFrom::Start(
                                mesh.data
                                    .vertex_color_offset
                                    .as_ref()
                                    .unwrap()
                                    .offset_absolute()
                                    + 4 * c.color as u64,
                            ))?;
                            edb.read_exact(&mut color)?;

                            edb.seek(std::io::SeekFrom::Start(
                                mesh.data
                                    .texture_coordinates
                                    .as_ref()
                                    .unwrap()
                                    .offset_absolute()
                                    + 4 * c.uv as u64,
                            ))?;
                            let uv: (i16, i16) = edb.read_type(edb.endian)?;

                            new_indices.push(vertex_data.len() as u32 - vertex_offset);
                            index_count += 1;

                            // FIXME(cohae): not actually index count, fix the structure. (there's probably more to this, check dbg file)
                            let uv_dividend = match (mesh.data.index_count >> 28) & 0b0111 {
                                0 => 65536.0,
                                1 => 32768.0,
                                2 => 16384.0, // Confirmed
                                3 => 8192.0,  // Confirmed
                                4 => 4096.0,  // Confirmed
                                5 => 2048.0,  // Confirmed
                                6 => 1024.0,
                                7 => 512.0, // Confirmed
                                _ => unreachable!(),
                            };

                            vertex_data.push(UXVertex {
                                pos: original_vert.pos,
                                norm: [0f32, 0f32, 0f32],
                                uv: [uv.0 as f32 / uv_dividend, uv.1 as f32 / uv_dividend],
                                color: [
                                    color[0] as f32 / 255.0,
                                    color[1] as f32 / 255.0,
                                    color[2] as f32 / 255.0,
                                    color[3] as f32 / 255.0,
                                ],
                            });
                        }
                    }

                    tristrips.push(EXGeoEntityTriStrip {
                        tricount: index_count as u32 - 2,
                        texture_index: s.texture_index as i32,
                        min_index: start_index as u16,
                        num_indices: index_count as u16,
                        flags: s.flags,
                        trans_type: s.transparency,
                        _unk10: 0,
                    });
                }
            } else {
                tristrips = mesh.tristrips.clone();
                new_indices = mesh.indices.iter().map(|v| *v as u32).collect();
            }

            let mut index_offset_local = 0;
            for t in tristrips {
                if t.tricount < 1 {
                    break;
                }

                if t.trans_type != 0 && remove_transparent {
                    index_offset_local += t.tricount + 2;
                    continue;
                }

                let texture_index = if mesh.data.base.flags & 0x1 != 0 {
                    // Index from texture list instead of the "global" array
                    if t.texture_index < mesh.texture_list.len() as i32 {
                        mesh.texture_list[t.texture_index as usize] as i32
                    } else {
                        error!(
                            "Tried to get texture #{} from texture list, but list only has {} elements!",
                            t.texture_index,
                            mesh.texture_list.len()
                        );
                        -1
                    }
                } else {
                    t.texture_index
                };

                if convert_strips {
                    let start_index = indices.len() as u32;
                    for i in
                        (index_offset_local as usize)..(index_offset_local + t.tricount) as usize
                    {
                        let tri = if (i - index_offset_local as usize) % 2 == 0 {
                            [
                                vertex_offset + new_indices[i + 2],
                                vertex_offset + new_indices[i + 1],
                                vertex_offset + new_indices[i],
                            ]
                        } else {
                            [
                                vertex_offset + new_indices[i],
                                vertex_offset + new_indices[i + 1],
                                vertex_offset + new_indices[i + 2],
                            ]
                        };
                        if tri[0] != tri[1] && tri[1] != tri[2] && tri[0] != tri[2] {
                            indices.extend(tri);
                        }
                    }
                    strips.push(TriStrip {
                        start_index,
                        index_count: indices.len() as u32 - start_index,
                        texture_index: texture_index as u32,
                        transparency: t.trans_type,
                        flags: t.flags,
                        tri_count: (indices.len() as u32 - start_index) / 3,
                        is_navmesh: false,
                    });
                    index_offset_local += t.tricount + 2;
                } else {
                    strips.push(TriStrip {
                        start_index: indices.len() as u32,
                        index_count: t.tricount + 2,
                        texture_index: texture_index as u32,
                        transparency: t.trans_type,
                        flags: t.flags,
                        tri_count: t.tricount,
                        is_navmesh: false,
                    });

                    indices.extend_from_slice(
                        &new_indices[(index_offset_local as usize)
                            ..(index_offset_local + t.tricount + 2) as usize]
                            .iter()
                            .map(|v| vertex_offset + v)
                            .collect::<Vec<u32>>(),
                    );

                    index_offset_local += t.tricount + 2;
                }
            }
        }
        EXGeoEntity::Instance(instance) => {
            if eurochef_edb::entity::robots_instance_bounds_visible() {
                read_robots_instance_bounds(
                    &instance.base,
                    vertex_data,
                    indices,
                    strips,
                    convert_strips,
                )?;
            }
        }
        EXGeoEntity::NavMesh(navmesh) => {
            eurochef_edb::entity::record_robots_navmesh_stats(navmesh);
            read_robots_navmesh(navmesh, vertex_data, indices, strips, edb, convert_strips)?;
        }
        EXGeoEntity::UnknownType(u) => {
            anyhow::bail!("Unsupported entity type 0x{u:x}")
        }
        _ => {
            anyhow::bail!("Unsupported entity type 0x{:x}", ent.type_code())
        }
    }

    Ok(())
}
