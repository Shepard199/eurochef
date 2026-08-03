use eurochef_edb::Hashcode;
use eurochef_shared::script::{UXGeoScript, UXGeoScriptCommandData};
use glam::Quat;

use crate::{map_frame::QueuedEntityRender, render::RenderStore};

const ROTATING_ENTITY: Hashcode = 0x0200_0012;
const NATIVE_FIXED_HZ: f32 = 60.0;
const NATIVE_ANGLE_SCALE: f32 = 0.002;

fn script_contains_native_fan_entity_inner(
    script: &UXGeoScript,
    current_file: Hashcode,
    render_store: &RenderStore,
    visited: &mut Vec<(Hashcode, Hashcode)>,
) -> bool {
    let key = (current_file, script.hashcode);
    if visited.contains(&key) {
        return false;
    }
    visited.push(key);

    let found = script.commands.iter().any(|command| match &command.data {
        UXGeoScriptCommandData::Entity { hashcode, file } => {
            let entity_file = if hashcode & 0x8000_0000 != 0 || *file == u32::MAX {
                current_file
            } else {
                *file
            };
            render_store.resolve_entity_hashcode(entity_file, *hashcode) == Some(ROTATING_ENTITY)
        }
        UXGeoScriptCommandData::SubScript { hashcode, file } => {
            let child_file = if hashcode & 0x8000_0000 != 0 || *file == u32::MAX {
                current_file
            } else {
                *file
            };
            render_store
                .get_script(child_file, *hashcode)
                .is_some_and(|child| {
                    script_contains_native_fan_entity_inner(
                        child,
                        child_file,
                        render_store,
                        visited,
                    )
                })
        }
        _ => false,
    });

    visited.pop();
    found
}

pub(super) fn script_contains_native_fan_entity(
    script: &UXGeoScript,
    current_file: Hashcode,
    render_store: &RenderStore,
) -> bool {
    script_contains_native_fan_entity_inner(script, current_file, render_store, &mut Vec::new())
}

pub(crate) fn advance_native_fan_angle(
    current_angle: f32,
    delta_seconds: f32,
    runtime_value: i32,
    playback_speed: f32,
) -> f32 {
    let delta = delta_seconds.max(0.0)
        * playback_speed.max(0.0)
        * NATIVE_FIXED_HZ
        * runtime_value as f32
        * NATIVE_ANGLE_SCALE;
    (current_angle + delta).rem_euclid(std::f32::consts::TAU)
}

pub(crate) fn apply_native_fan_rotation(
    queue: &mut [QueuedEntityRender],
    render_store: &RenderStore,
    angle: f32,
) -> usize {
    let rotation = Quat::from_rotation_z(angle);
    let mut affected = 0usize;
    for queued in queue {
        if queued.entity_alt.is_some() {
            continue;
        }
        if render_store.resolve_entity_hashcode(queued.entity.0, queued.entity.1)
            == Some(ROTATING_ENTITY)
        {
            queued.rotation *= rotation;
            affected += 1;
        }
    }
    affected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_fan_angle_uses_fixed_sixty_hz_runtime_delta() {
        let angle = advance_native_fan_angle(0.0, 1.0 / 60.0, 50, 1.0);
        assert!((angle - 0.1).abs() < 1.0e-6);
    }

    #[test]
    fn native_fan_angle_preserves_signed_runtime_direction() {
        let angle = advance_native_fan_angle(0.0, 1.0 / 60.0, -50, 1.0);
        assert!((angle - (std::f32::consts::TAU - 0.1)).abs() < 1.0e-6);
    }
}
