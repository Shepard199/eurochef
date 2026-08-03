use base64::Engine;
use eurochef_shared::entities::{TriStrip, UXVertex};
use gltf::json::{self as gjson, validation::Checked};
use std::collections::HashMap;

use super::entities::Transparency;

// pub fn write_glb<W: Write>(gltf: &gjson::Root, out: &mut W) -> anyhow::Result<()> {
//     let json_string = gjson::serialize::to_string(gltf).context("glTF serialization error")?;
//     let mut json_offset = json_string.len() as u32;
//     align_to_multiple_of_four(&mut json_offset);
//     let glb = gltf::binary::Glb {
//         header: gltf::binary::Header {
//             magic: *b"glTF",
//             version: 2,
//             length: json_offset,
//         },
//         bin: None,
//         json: Cow::Owned(json_string.into_bytes()),
//     };
//     glb.to_writer(out).context("glTF binary output error")?;

//     Ok(())
// }

// fn align_to_multiple_of_four(n: &mut u32) {
//     *n = (*n + 3) & !3;
// }

/// Creates a scene with a single mesh in it
pub fn create_mesh_scene(name: &str) -> gjson::Root {
    let node = gjson::Node {
        camera: None,
        children: None,
        extensions: Default::default(),
        extras: Default::default(),
        matrix: None,
        mesh: Some(gjson::Index::new(0)),
        name: Some(name.to_string()),
        rotation: None,
        scale: None,
        translation: None,
        skin: None,
        weights: None,
    };
    let mesh = gjson::Mesh {
        extensions: Default::default(),
        extras: Default::default(),
        name: Some(name.to_string()),
        primitives: vec![],
        weights: None,
    };
    let sampler = gjson::texture::Sampler::default();
    gjson::Root {
        accessors: vec![],
        buffers: vec![],
        buffer_views: vec![],
        meshes: vec![mesh],
        nodes: vec![node],
        samplers: vec![sampler],
        scenes: vec![gjson::Scene {
            extensions: Default::default(),
            extras: Default::default(),
            name: None,
            nodes: vec![gjson::Index::new(0)],
        }],
        asset: gjson::Asset {
            generator: Some(format!("Eurochef {name}")),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Constructs a primitive and adds it to the first mesh in the scene
pub fn add_mesh_to_scene(
    root: &mut gjson::Root,
    vertices: &[UXVertex],
    indices: &[u32],
    strips: &[TriStrip],
    use_normals: bool,
    texture_map: &HashMap<u32, (String, Transparency)>,
    _file_hash: u32,
) {
    let mut material_map: HashMap<u32, u32> = HashMap::new();
    // Restore material map
    for (i, m) in root.materials.iter().enumerate() {
        let msplit = m.name.as_ref().unwrap().split('_').next().unwrap();
        let mhashcode = u32::from_str_radix(msplit, 16).unwrap();
        material_map.insert(mhashcode, i as u32);
    }

    for t in strips {
        if let std::collections::hash_map::Entry::Vacant(e) = material_map.entry(t.texture_index) {
            let (img_uri, transparency) = texture_map
                .get(&t.texture_index)
                .cloned()
                .unwrap_or((format!("{:08x}.png", t.texture_index), Transparency::Opaque));

            let texture_name = format!("{:08x}", t.texture_index);

            root.images.push(gjson::Image {
                uri: Some(img_uri),
                buffer_view: None,
                extensions: None,
                extras: Default::default(),
                mime_type: None,
                name: Some(texture_name.clone()),
            });

            root.textures.push(gjson::Texture {
                sampler: Some(gjson::Index::new(0)),
                extensions: None,
                extras: Default::default(),
                source: gjson::Index::new(root.images.len() as u32 - 1),
                name: None,
            });

            root.materials.push(gjson::Material {
                alpha_mode: Checked::Valid(if transparency != Transparency::Opaque {
                    match transparency {
                        Transparency::Opaque => gjson::material::AlphaMode::Opaque,
                        Transparency::Blend => gjson::material::AlphaMode::Blend,
                        Transparency::_Additive => gjson::material::AlphaMode::Blend,
                        Transparency::Cutout => gltf::material::AlphaMode::Mask,
                    }
                } else {
                    match t.transparency {
                        0 => gjson::material::AlphaMode::Opaque,
                        // 1 => Additive blending
                        // 2 => Reverse_subtract blending
                        _ => gjson::material::AlphaMode::Blend,
                    }
                }),
                pbr_metallic_roughness: gjson::material::PbrMetallicRoughness {
                    metallic_factor: gjson::material::StrengthFactor(0.),
                    roughness_factor: gjson::material::StrengthFactor(1.),
                    base_color_texture: Some(gjson::texture::Info {
                        index: gjson::Index::new(root.textures.len() as u32 - 1),
                        tex_coord: 0,
                        extensions: None,
                        extras: Default::default(),
                    }),
                    ..Default::default()
                },
                name: Some(texture_name),
                double_sided: (t.flags & 0x40) != 0,
                ..Default::default()
            });

            let material_index = root.materials.len() as u32 - 1;
            e.insert(material_index);
        }

        let start = t.start_index as usize;
        let end = start.saturating_add(t.index_count as usize);
        let Some(source_indices) = indices.get(start..end) else {
            continue;
        };
        let mut local_vertices = Vec::with_capacity(source_indices.len());
        let mut local_indices = Vec::with_capacity(source_indices.len());
        for tri in source_indices.chunks_exact(3) {
            let base = local_vertices.len() as u32;
            for source_index in [tri[0], tri[1], tri[2]] {
                let Some(vertex) = vertices.get(source_index as usize) else {
                    continue;
                };
                local_vertices.push(*vertex);
            }
            if local_vertices.len() == base as usize + 3 {
                local_indices.extend([base, base + 1, base + 2]);
            }
        }
        if local_indices.len() < 3 {
            continue;
        }
        let material_id = material_map.get(&t.texture_index).unwrap();

        let local_vdata: &[u8] = bytemuck::cast_slice(&local_vertices);
        if local_vertices.len() > u16::MAX as usize {
            continue;
        }
        let local_indices_u16: Vec<u16> = local_indices.iter().map(|i| *i as u16).collect();
        let local_idata: &[u8] = bytemuck::cast_slice(&local_indices_u16);
        let (local_min, local_max) = bounding_coords(&local_vertices);

        let local_vertex_buffer = gjson::Buffer {
            byte_length: local_vdata.len().into(),
            extensions: Default::default(),
            extras: Default::default(),
            name: None,
            uri: Some(create_data_uri(local_vdata)),
        };
        let local_vertex_buffer_index = root.buffers.len() as u32;
        root.buffers.push(local_vertex_buffer.clone());

        let local_vertex_buffer_view = gjson::buffer::View {
            buffer: gjson::Index::new(local_vertex_buffer_index),
            byte_length: local_vertex_buffer.byte_length,
            byte_offset: None,
            byte_stride: Some(gjson::buffer::Stride(std::mem::size_of::<UXVertex>())),
            extensions: Default::default(),
            extras: Default::default(),
            name: None,
            target: Some(Checked::Valid(gjson::buffer::Target::ArrayBuffer)),
        };
        let local_vertex_buffer_view_index = root.buffer_views.len() as u32;
        root.buffer_views.push(local_vertex_buffer_view);

        let local_index_buffer = gjson::Buffer {
            byte_length: local_idata.len().into(),
            extensions: Default::default(),
            extras: Default::default(),
            name: None,
            uri: Some(create_data_uri(local_idata)),
        };
        let local_index_buffer_index = root.buffers.len() as u32;
        root.buffers.push(local_index_buffer.clone());

        let index_buffer_view = gjson::buffer::View {
            buffer: gjson::Index::new(local_index_buffer_index),
            byte_length: local_index_buffer.byte_length,
            byte_offset: None,
            byte_stride: None,
            extensions: Default::default(),
            extras: Default::default(),
            name: None,
            target: Some(Checked::Valid(gjson::buffer::Target::ElementArrayBuffer)),
        };
        root.buffer_views.push(index_buffer_view);

        let local_position_index = root.accessors.len() as u32;
        root.accessors.push(gjson::Accessor {
            buffer_view: Some(gjson::Index::new(local_vertex_buffer_view_index)),
            byte_offset: None,
            count: local_vertices.len().into(),
            component_type: Checked::Valid(gjson::accessor::GenericComponentType(
                gjson::accessor::ComponentType::F32,
            )),
            extensions: Default::default(),
            extras: Default::default(),
            type_: Checked::Valid(gjson::accessor::Type::Vec3),
            min: Some(gjson::Value::from(Vec::from(local_min))),
            max: Some(gjson::Value::from(Vec::from(local_max))),
            name: None,
            normalized: false,
            sparse: None,
        });
        let local_normals_index = root.accessors.len() as u32;
        root.accessors.push(gjson::Accessor {
            buffer_view: Some(gjson::Index::new(local_vertex_buffer_view_index)),
            byte_offset: Some((3 * std::mem::size_of::<f32>()).into()),
            count: local_vertices.len().into(),
            component_type: Checked::Valid(gjson::accessor::GenericComponentType(
                gjson::accessor::ComponentType::F32,
            )),
            extensions: Default::default(),
            extras: Default::default(),
            type_: Checked::Valid(gjson::accessor::Type::Vec3),
            min: None,
            max: None,
            name: None,
            normalized: false,
            sparse: None,
        });
        let local_uvs_index = root.accessors.len() as u32;
        root.accessors.push(gjson::Accessor {
            buffer_view: Some(gjson::Index::new(local_vertex_buffer_view_index)),
            byte_offset: Some((6 * std::mem::size_of::<f32>()).into()),
            count: local_vertices.len().into(),
            component_type: Checked::Valid(gjson::accessor::GenericComponentType(
                gjson::accessor::ComponentType::F32,
            )),
            extensions: Default::default(),
            extras: Default::default(),
            type_: Checked::Valid(gjson::accessor::Type::Vec2),
            min: None,
            max: None,
            name: None,
            normalized: false,
            sparse: None,
        });
        let local_colors_index = root.accessors.len() as u32;
        root.accessors.push(gjson::Accessor {
            buffer_view: Some(gjson::Index::new(local_vertex_buffer_view_index)),
            byte_offset: Some((8 * std::mem::size_of::<f32>()).into()),
            count: local_vertices.len().into(),
            component_type: Checked::Valid(gjson::accessor::GenericComponentType(
                gjson::accessor::ComponentType::F32,
            )),
            extensions: Default::default(),
            extras: Default::default(),
            type_: Checked::Valid(gjson::accessor::Type::Vec4),
            min: None,
            max: None,
            name: None,
            normalized: false,
            sparse: None,
        });

        let index_accessor = gjson::Accessor {
            buffer_view: Some(gjson::Index::new(root.buffer_views.len() as u32 - 1)),
            byte_offset: None,
            count: (local_indices_u16.len() as u64).into(),
            component_type: Checked::Valid(gjson::accessor::GenericComponentType(
                gjson::accessor::ComponentType::U16,
            )),
            extensions: Default::default(),
            extras: Default::default(),
            type_: Checked::Valid(gjson::accessor::Type::Scalar),
            min: None,
            max: None,
            name: None,
            normalized: false,
            sparse: None,
        };
        root.accessors.push(index_accessor);

        let primitive = gjson::mesh::Primitive {
            attributes: {
                let mut map = std::collections::BTreeMap::new();
                map.insert(
                    Checked::Valid(gjson::mesh::Semantic::Positions),
                    gjson::Index::new(local_position_index),
                );
                if use_normals {
                    map.insert(
                        Checked::Valid(gjson::mesh::Semantic::Normals),
                        gjson::Index::new(local_normals_index),
                    );
                }
                map.insert(
                    Checked::Valid(gjson::mesh::Semantic::TexCoords(0)),
                    gjson::Index::new(local_uvs_index),
                );
                map.insert(
                    Checked::Valid(gjson::mesh::Semantic::Colors(0)),
                    gjson::Index::new(local_colors_index),
                );
                map
            },
            extensions: Default::default(),
            extras: Default::default(),
            indices: Some(gjson::Index::new(root.accessors.len() as u32 - 1)),
            material: Some(gjson::Index::new(*material_id)),
            mode: Checked::Valid(gjson::mesh::Mode::Triangles),
            targets: None,
        };

        root.meshes[0].primitives.push(primitive);
    }
}

fn bounding_coords(vertices: &[UXVertex]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::MAX, f32::MAX, f32::MAX];
    let mut max = [f32::MIN, f32::MIN, f32::MIN];

    for v in vertices {
        let p = v.pos;
        for i in 0..3 {
            min[i] = f32::min(min[i], p[i]);
            max[i] = f32::max(max[i], p[i]);
        }
    }
    (min, max)
}

fn create_data_uri(data: &[u8]) -> String {
    let mut uri = "data:application/octet-stream;base64,".to_string();
    base64::engine::general_purpose::STANDARD.encode_string(data, &mut uri);
    uri
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texture_asset_name_does_not_include_the_source_edb_hash() {
        let mut root = create_mesh_scene("mesh");
        let mut textures = HashMap::new();
        textures.insert(
            0x8600_013a,
            ("8600013a.png".to_string(), Transparency::Opaque),
        );
        let vertices = [
            UXVertex {
                pos: [0.0, 0.0, 0.0],
                norm: [0.0, 0.0, 1.0],
                uv: [0.0, 0.0],
                color: [1.0; 4],
            },
            UXVertex {
                pos: [1.0, 0.0, 0.0],
                norm: [0.0, 0.0, 1.0],
                uv: [1.0, 0.0],
                color: [1.0; 4],
            },
            UXVertex {
                pos: [0.0, 1.0, 0.0],
                norm: [0.0, 0.0, 1.0],
                uv: [0.0, 1.0],
                color: [1.0; 4],
            },
        ];
        add_mesh_to_scene(
            &mut root,
            &vertices,
            &[0, 1, 2],
            &[TriStrip {
                start_index: 0,
                index_count: 3,
                texture_index: 0x8600_013a,
                transparency: 0,
                flags: 0,
                tri_count: 1,
                is_navmesh: false,
            }],
            true,
            &textures,
            0x0100_0012,
        );

        assert_eq!(root.images[0].name.as_deref(), Some("8600013a"));
        assert_eq!(root.materials[0].name.as_deref(), Some("8600013a"));
        assert!(root.extensions_used.is_empty());
        let primitive = &root.meshes[0].primitives[0];
        assert_eq!(primitive.mode, Checked::Valid(gjson::mesh::Mode::Triangles));
        assert!(primitive
            .attributes
            .contains_key(&Checked::Valid(gjson::mesh::Semantic::Colors(0))));

        let index_accessor = &root.accessors[primitive.indices.unwrap().value()];
        let index_view = &root.buffer_views[index_accessor.buffer_view.unwrap().value()];
        let index_uri = root.buffers[index_view.buffer.value()].uri.as_ref().unwrap();
        let raw_indices = base64::engine::general_purpose::STANDARD
            .decode(index_uri.split_once(',').unwrap().1)
            .unwrap();
        let written_indices: &[u16] = bytemuck::cast_slice(&raw_indices);
        assert_eq!(written_indices, &[0, 1, 2]);

        let position_accessor = &root.accessors[primitive
            .attributes
            .get(&Checked::Valid(gjson::mesh::Semantic::Positions))
            .unwrap()
            .value()];
        assert_eq!(position_accessor.count, 3usize.into());
    }
}
