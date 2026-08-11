use std::ops::Range;

use eurochef_edb::{anim::EXGeoBaseAnimSkin, Hashcode};
use eurochef_shared::entities::UXVertex;
use glam::{Mat4, Quat, Vec3};

use super::AnimationPartSkin;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AnimationBonePose {
    pub position: Vec3,
    pub rotation: Quat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeBoneRemap {
    /// Source bone selector -> target bone selector. `0xFF` is the native
    /// unresolved sentinel and is preserved if parent fallback cannot resolve.
    pub selectors: Vec<u8>,
    /// Native 128-bit target-bone ancestor mask built from directly matched
    /// target selectors and their parent chains.
    pub target_ancestor_mask: [u32; 4],
}

/// Reproduces Robots.exe `FUN_004FCE8C` skin-to-skin selector remapping.
///
/// Direct mappings are established only by equal serialized `HT_AnimBone`
/// identities. Unnamed/unmatched source bones then inherit their source
/// parent's already-resolved selector; an unmatched source root maps to target
/// selector zero. Directly matched target selectors and all of their target
/// ancestors are recorded in the same four-DWORD mask used by the game.
pub(crate) fn build_native_bone_remap(
    source_names: &[Option<Hashcode>],
    source_parents: &[u16],
    target_names: &[Option<Hashcode>],
    target_parents: &[u16],
) -> Option<NativeBoneRemap> {
    if source_names.len() != source_parents.len()
        || target_names.len() != target_parents.len()
        || source_names.len() > u8::MAX as usize
        || target_names.len() > 128
    {
        return None;
    }

    let mut target_by_name = std::collections::HashMap::new();
    for (target_selector, name) in target_names.iter().copied().enumerate() {
        let Some(name) = name else {
            continue;
        };
        if target_by_name.insert(name, target_selector as u8).is_some() {
            return None;
        }
    }

    let mut selectors = vec![u8::MAX; source_names.len()];
    let mut target_ancestor_mask = [0u32; 4];

    for (source_selector, name) in source_names.iter().copied().enumerate() {
        let Some(name) = name else {
            continue;
        };
        let Some(&target_selector) = target_by_name.get(&name) else {
            continue;
        };
        selectors[source_selector] = target_selector;

        let mut target_chain_selector = target_selector as usize;
        let mut visited = [false; 128];
        loop {
            if target_chain_selector >= target_parents.len() || visited[target_chain_selector] {
                return None;
            }
            visited[target_chain_selector] = true;
            target_ancestor_mask[target_chain_selector / 32] |=
                1u32 << (target_chain_selector & 31);
            let parent = target_parents[target_chain_selector];
            if parent == u16::MAX {
                break;
            }
            target_chain_selector = parent as usize;
        }
    }

    for source_selector in 0..selectors.len() {
        if selectors[source_selector] != u8::MAX {
            continue;
        }
        let parent = source_parents[source_selector];
        selectors[source_selector] = if parent == u16::MAX {
            0
        } else {
            let parent = parent as usize;
            if parent >= source_selector || parent >= selectors.len() {
                return None;
            }
            selectors[parent]
        };
    }

    Some(NativeBoneRemap {
        selectors,
        target_ancestor_mask,
    })
}

fn bind_pose_bone_poses(skin: &EXGeoBaseAnimSkin) -> Vec<AnimationBonePose> {
    skin.relative_bind_positions
        .iter()
        .map(|position| AnimationBonePose {
            position: Vec3::new(position[0], position[1], position[2]),
            rotation: Quat::IDENTITY,
        })
        .collect()
}

pub(crate) fn bind_pose_global_bone_matrices(skin: &EXGeoBaseAnimSkin) -> Option<Vec<Mat4>> {
    let poses = bind_pose_bone_poses(skin);
    build_global_bone_matrices(skin, &poses)
}

pub(crate) fn bind_pose_skin_matrices(skin: &EXGeoBaseAnimSkin) -> Option<Vec<Mat4>> {
    let poses = bind_pose_bone_poses(skin);
    build_skin_matrices(skin, &poses)
}

pub(crate) fn build_global_bone_matrices(
    skin: &EXGeoBaseAnimSkin,
    poses: &[AnimationBonePose],
) -> Option<Vec<Mat4>> {
    let parents = skin
        .hier_data
        .iter()
        .map(|hierarchy| hierarchy.link_index)
        .collect::<Vec<_>>();
    build_global_bone_matrices_from_data(&parents, poses)
}

pub(crate) fn build_skin_matrices(
    skin: &EXGeoBaseAnimSkin,
    poses: &[AnimationBonePose],
) -> Option<Vec<Mat4>> {
    let absolute_bind_positions = skin
        .absolute_bind_positions
        .iter()
        .map(|position| Vec3::new(position[0], position[1], position[2]))
        .collect::<Vec<_>>();
    let globals = build_global_bone_matrices(skin, poses)?;
    if absolute_bind_positions.len() != globals.len() {
        return None;
    }
    Some(
        globals
            .into_iter()
            .zip(absolute_bind_positions)
            .map(|(global, absolute_bind)| global * Mat4::from_translation(-absolute_bind))
            .collect(),
    )
}

#[cfg(test)]
fn build_skin_matrices_from_data(
    absolute_bind_positions: &[Vec3],
    parents: &[u16],
    poses: &[AnimationBonePose],
) -> Option<Vec<Mat4>> {
    let globals = build_global_bone_matrices_from_data(parents, poses)?;
    if absolute_bind_positions.len() != globals.len() {
        return None;
    }
    Some(
        globals
            .into_iter()
            .zip(absolute_bind_positions)
            .map(|(global, absolute_bind)| global * Mat4::from_translation(-*absolute_bind))
            .collect(),
    )
}

fn build_global_bone_matrices_from_data(
    parents: &[u16],
    poses: &[AnimationBonePose],
) -> Option<Vec<Mat4>> {
    let bone_count = poses.len();
    if parents.len() != bone_count {
        return None;
    }
    let mut globals = vec![Mat4::IDENTITY; bone_count];
    let mut states = vec![0u8; bone_count];
    for bone_index in 0..bone_count {
        resolve_global_bone(bone_index, parents, poses, &mut globals, &mut states)?;
    }
    Some(globals)
}

fn resolve_global_bone(
    bone_index: usize,
    parents: &[u16],
    poses: &[AnimationBonePose],
    globals: &mut [Mat4],
    states: &mut [u8],
) -> Option<Mat4> {
    match states.get(bone_index).copied()? {
        2 => return globals.get(bone_index).copied(),
        1 => return None,
        _ => {}
    }
    states[bone_index] = 1;

    let pose = *poses.get(bone_index)?;
    let local = Mat4::from_translation(pose.position) * Mat4::from_quat(pose.rotation);
    let parent_index = *parents.get(bone_index)?;
    let global = if parent_index == u16::MAX {
        local
    } else {
        let parent_index = parent_index as usize;
        if parent_index >= globals.len() || parent_index == bone_index {
            return None;
        }
        resolve_global_bone(parent_index, parents, poses, globals, states)? * local
    };

    globals[bone_index] = global;
    states[bone_index] = 2;
    Some(global)
}

pub(crate) fn skin_vertices(
    original: &[UXVertex],
    output: &mut [UXVertex],
    part_vertex_ranges: &[Range<usize>],
    part_skins: &[AnimationPartSkin],
    skin_matrices: &[Mat4],
) -> Option<()> {
    skin_vertices_with_morph(
        original,
        output,
        part_vertex_ranges,
        part_skins,
        skin_matrices,
        &[],
        None,
    )
}

/// Applies the shipped Robots v248 additive position morph before skeletal skinning.
/// Native `FUN_005186BD` copies each 0x20-byte base vertex, adds only XYZ from
/// 0x10-byte morph-shape records, then the normal skeletal/render path runs.
pub(crate) fn skin_vertices_with_morph(
    original: &[UXVertex],
    output: &mut [UXVertex],
    part_vertex_ranges: &[Range<usize>],
    part_skins: &[AnimationPartSkin],
    skin_matrices: &[Mat4],
    morph_scalars: &[f32],
    morph_scalar_base: Option<usize>,
) -> Option<()> {
    if original.len() != output.len() || part_vertex_ranges.len() != part_skins.len() {
        return None;
    }
    output.clone_from_slice(original);

    for (part_index, (range, part_skin)) in part_vertex_ranges.iter().zip(part_skins).enumerate() {
        if part_skin.part_index != part_index
            || range.end > original.len()
            || range.len() != part_skin.vertex_count
            || range.len() != part_skin.influences.len()
            || part_skin
                .morph_shapes
                .iter()
                .any(|shape| shape.len() != range.len())
        {
            return None;
        }
        if !part_skin.morph_shapes.is_empty() && morph_scalar_base.is_none() {
            return None;
        }

        for (vertex_offset, influence) in part_skin.influences.iter().enumerate() {
            let vertex_index = range.start + vertex_offset;
            let source = original[vertex_index];
            let mut source_position = Vec3::from_array(source.pos);
            if let Some(scalar_base) = morph_scalar_base {
                for (shape_index, shape) in part_skin.morph_shapes.iter().enumerate() {
                    let scalar = *morph_scalars.get(scalar_base.checked_add(shape_index)?)?;
                    if !scalar.is_finite() {
                        return None;
                    }
                    source_position += shape[vertex_offset] * scalar;
                }
            }
            let source_normal = Vec3::from_array(source.norm);
            let mut position = Vec3::ZERO;
            let mut normal = Vec3::ZERO;

            for lane in 0..4 {
                let weight = influence.weights[lane];
                if weight.abs() <= f32::EPSILON {
                    continue;
                }
                let matrix = *skin_matrices.get(influence.bone_indices[lane] as usize)?;
                position += matrix.transform_point3(source_position) * weight;
                normal += matrix.transform_vector3(source_normal) * weight;
            }

            output[vertex_index].pos = position.to_array();
            output[vertex_index].norm = normal.normalize_or_zero().to_array();
        }
    }
    Some(())
}

/// Applies one current global bone matrix to rigid bone-attached Entity geometry.
/// Robots.exe 0x00500814 uses the selected bone matrix directly for AnimSkin +0x78
/// records, without the inverse-bind correction used by skinned component vertices.
pub(crate) fn transform_vertices_rigid(
    original: &[UXVertex],
    output: &mut [UXVertex],
    bone_global: Mat4,
) -> Option<()> {
    if original.len() != output.len() || !bone_global.is_finite() {
        return None;
    }
    output.clone_from_slice(original);
    for (source, target) in original.iter().zip(output.iter_mut()) {
        target.pos = bone_global
            .transform_point3(Vec3::from_array(source.pos))
            .to_array();
        target.norm = bone_global
            .transform_vector3(Vec3::from_array(source.norm))
            .normalize_or_zero()
            .to_array();
    }
    Some(())
}

pub(crate) fn matrix_max_abs_difference(left: Mat4, right: Mat4) -> f32 {
    left.to_cols_array()
        .into_iter()
        .zip(right.to_cols_array())
        .map(|(left, right)| (left - right).abs())
        .fold(0.0, f32::max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animations::AnimationVertexInfluence;

    const ABSOLUTE_BIND_POSITIONS: [Vec3; 2] = [Vec3::new(1.0, 2.0, 3.0), Vec3::new(1.0, 4.0, 3.0)];
    const PARENTS: [u16; 2] = [u16::MAX, 0];
    const BIND_POSES: [AnimationBonePose; 2] = [
        AnimationBonePose {
            position: Vec3::new(1.0, 2.0, 3.0),
            rotation: Quat::IDENTITY,
        },
        AnimationBonePose {
            position: Vec3::new(0.0, 2.0, 0.0),
            rotation: Quat::IDENTITY,
        },
    ];

    #[test]
    fn bind_pose_produces_identity_skin_matrices() {
        let matrices =
            build_skin_matrices_from_data(&ABSOLUTE_BIND_POSITIONS, &PARENTS, &BIND_POSES)
                .expect("valid hierarchy");
        assert_eq!(matrices.len(), 2);
        for matrix in matrices {
            assert!(matrix_max_abs_difference(matrix, Mat4::IDENTITY) <= 1.0e-6);
        }
    }

    #[test]
    fn rotated_child_uses_parent_global_transform() {
        let poses = [
            AnimationBonePose {
                position: Vec3::new(1.0, 2.0, 3.0),
                rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
            },
            BIND_POSES[1],
        ];
        let matrices = build_skin_matrices_from_data(&ABSOLUTE_BIND_POSITIONS, &PARENTS, &poses)
            .expect("valid hierarchy");
        let transformed = matrices[1].transform_point3(ABSOLUTE_BIND_POSITIONS[1]);
        assert!(transformed.distance(Vec3::new(-1.0, 2.0, 3.0)) <= 1.0e-5);
    }

    #[test]
    fn hierarchy_cycles_are_rejected() {
        let parents = [1, 0];
        assert!(
            build_skin_matrices_from_data(&ABSOLUTE_BIND_POSITIONS, &parents, &BIND_POSES,)
                .is_none()
        );
    }

    #[test]
    fn native_bone_remap_matches_direct_ids_parent_fallback_and_target_mask() {
        let source_names = [
            Some(0x0E00_0001),
            Some(0x0E00_0002),
            None,
            Some(0x0E00_0003),
        ];
        let source_parents = [u16::MAX, 0, 1, 1];
        let target_names = [
            Some(0x0E00_0001),
            Some(0x0E00_0002),
            Some(0x0E00_0003),
            None,
        ];
        let target_parents = [u16::MAX, 0, 1, 2];

        let remap = build_native_bone_remap(
            &source_names,
            &source_parents,
            &target_names,
            &target_parents,
        )
        .expect("valid native remap");

        assert_eq!(remap.selectors, vec![0, 1, 1, 2]);
        assert_eq!(remap.target_ancestor_mask, [0b111, 0, 0, 0]);
    }

    #[test]
    fn native_bone_remap_rejects_duplicate_target_animbone_ids() {
        let source_names = [Some(0x0E00_0001)];
        let source_parents = [u16::MAX];
        let target_names = [Some(0x0E00_0001), Some(0x0E00_0001)];
        let target_parents = [u16::MAX, 0];
        assert!(build_native_bone_remap(
            &source_names,
            &source_parents,
            &target_names,
            &target_parents,
        )
        .is_none());
    }

    fn test_vertex() -> UXVertex {
        UXVertex {
            pos: [0.0, 0.0, 0.0],
            norm: [0.0, 0.0, 1.0],
            uv: [0.25, 0.75],
            color: [1.0, 0.5, 0.25, 1.0],
        }
    }

    #[test]
    fn rigid_bone_attachment_uses_global_matrix_without_inverse_bind() {
        let mut vertex = test_vertex();
        vertex.pos = [1.0, 0.0, 0.0];
        let original = [vertex];
        let mut output = [vertex];
        let global = Mat4::from_translation(Vec3::new(10.0, 2.0, 0.0))
            * Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2);
        transform_vertices_rigid(&original, &mut output, global).expect("valid rigid transform");
        assert!(Vec3::from_array(output[0].pos).distance(Vec3::new(10.0, 3.0, 0.0)) <= 1.0e-6);
        assert!(Vec3::from_array(output[0].norm).distance(Vec3::new(0.0, 0.0, 1.0)) <= 1.0e-6);
        assert_eq!(output[0].uv, original[0].uv);
        assert_eq!(output[0].color, original[0].color);
    }

    #[test]
    fn identity_skinning_preserves_vertex_data() {
        let original = [test_vertex()];
        let mut output = [test_vertex()];
        let parts = [AnimationPartSkin {
            part_index: 0,
            vertex_count: 1,
            influences: vec![AnimationVertexInfluence {
                bone_indices: [0, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0],
            }],
            morph_shapes: Vec::new(),
        }];
        skin_vertices(&original, &mut output, &[0..1], &parts, &[Mat4::IDENTITY])
            .expect("valid identity skinning");
        assert_eq!(output[0].pos, original[0].pos);
        assert_eq!(output[0].norm, original[0].norm);
        assert_eq!(output[0].uv, original[0].uv);
        assert_eq!(output[0].color, original[0].color);
    }

    #[test]
    fn four_weight_skinning_blends_transforms() {
        let original = [test_vertex()];
        let mut output = [test_vertex()];
        let parts = [AnimationPartSkin {
            part_index: 0,
            vertex_count: 1,
            influences: vec![AnimationVertexInfluence {
                bone_indices: [0, 1, 0, 0],
                weights: [0.25, 0.75, 0.0, 0.0],
            }],
            morph_shapes: Vec::new(),
        }];
        let matrices = [
            Mat4::from_translation(Vec3::X),
            Mat4::from_translation(Vec3::Y * 2.0),
        ];
        skin_vertices(&original, &mut output, &[0..1], &parts, &matrices)
            .expect("valid blended skinning");
        assert!(Vec3::from_array(output[0].pos).distance(Vec3::new(0.25, 1.5, 0.0)) <= 1.0e-6);
        assert!(Vec3::from_array(output[0].norm).distance(Vec3::Z) <= 1.0e-6);
        assert_eq!(output[0].uv, original[0].uv);
        assert_eq!(output[0].color, original[0].color);
    }

    #[test]
    fn robots_morph_is_applied_before_skeletal_skinning() {
        let mut source = test_vertex();
        source.pos = [1.0, 0.0, 0.0];
        let original = [source];
        let mut output = [source];
        let parts = [AnimationPartSkin {
            part_index: 0,
            vertex_count: 1,
            influences: vec![AnimationVertexInfluence {
                bone_indices: [0, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0],
            }],
            morph_shapes: vec![vec![Vec3::new(2.0, 0.0, 0.0)]],
        }];
        skin_vertices_with_morph(
            &original,
            &mut output,
            &[0..1],
            &parts,
            &[Mat4::from_translation(Vec3::Y)],
            &[0.5],
            Some(0),
        )
        .expect("valid Robots morph + skinning");
        assert!(Vec3::from_array(output[0].pos).distance(Vec3::new(2.0, 1.0, 0.0)) <= 1.0e-6);
        assert_eq!(output[0].norm, original[0].norm);
        assert_eq!(output[0].uv, original[0].uv);
        assert_eq!(output[0].color, original[0].color);
    }
}
