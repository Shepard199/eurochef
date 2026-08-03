use std::ops::Range;

use eurochef_edb::anim::EXGeoBaseAnimSkin;
use eurochef_shared::entities::UXVertex;
use glam::{Mat4, Quat, Vec3};

use super::AnimationPartSkin;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AnimationBonePose {
    pub position: Vec3,
    pub rotation: Quat,
}

pub(crate) fn bind_pose_skin_matrices(skin: &EXGeoBaseAnimSkin) -> Option<Vec<Mat4>> {
    let poses = skin
        .relative_bind_positions
        .iter()
        .map(|position| AnimationBonePose {
            position: Vec3::new(position[0], position[1], position[2]),
            rotation: Quat::IDENTITY,
        })
        .collect::<Vec<_>>();
    build_skin_matrices(skin, &poses)
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
    let parents = skin
        .hier_data
        .iter()
        .map(|hierarchy| hierarchy.link_index)
        .collect::<Vec<_>>();
    build_skin_matrices_from_data(&absolute_bind_positions, &parents, poses)
}

fn build_skin_matrices_from_data(
    absolute_bind_positions: &[Vec3],
    parents: &[u16],
    poses: &[AnimationBonePose],
) -> Option<Vec<Mat4>> {
    let bone_count = poses.len();
    if absolute_bind_positions.len() != bone_count || parents.len() != bone_count {
        return None;
    }

    let mut globals = vec![Mat4::IDENTITY; bone_count];
    let mut states = vec![0u8; bone_count];
    for bone_index in 0..bone_count {
        resolve_global_bone(bone_index, parents, poses, &mut globals, &mut states)?;
    }

    Some(
        globals
            .into_iter()
            .zip(absolute_bind_positions)
            .map(|(global, absolute_bind)| global * Mat4::from_translation(-*absolute_bind))
            .collect(),
    )
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
    if original.len() != output.len() || part_vertex_ranges.len() != part_skins.len() {
        return None;
    }
    output.clone_from_slice(original);

    for (part_index, (range, part_skin)) in part_vertex_ranges.iter().zip(part_skins).enumerate() {
        if part_skin.part_index != part_index
            || range.end > original.len()
            || range.len() != part_skin.vertex_count
            || range.len() != part_skin.influences.len()
        {
            return None;
        }

        for (vertex_offset, influence) in part_skin.influences.iter().enumerate() {
            let vertex_index = range.start + vertex_offset;
            let source = original[vertex_index];
            let source_position = Vec3::from_array(source.pos);
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

    fn test_vertex() -> UXVertex {
        UXVertex {
            pos: [0.0, 0.0, 0.0],
            norm: [0.0, 0.0, 1.0],
            uv: [0.25, 0.75],
            color: [1.0, 0.5, 0.25, 1.0],
        }
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
}
