use eurochef_edb::{script::EXGeoAnimScriptControllerHeader, Hashcode, HashcodeUtils};
use eurochef_shared::script::{UXGeoScript, UXGeoScriptCommand, UXGeoScriptCommandData};
use glam::{Quat, Vec3};

use crate::{map_frame::QueuedEntityRender, render::tweeny::ease_in_out_sine};

use super::RenderStore;

const MAX_SCRIPT_RECURSION_DEPTH: usize = 64;

pub fn first_resolved_visual_time(
    current_file: Hashcode,
    script_hashcode: Hashcode,
    render_store: &RenderStore,
) -> Option<f32> {
    fn visit(
        current_file: Hashcode,
        script_hashcode: Hashcode,
        render_store: &RenderStore,
        ancestry: &mut Vec<(Hashcode, Hashcode)>,
    ) -> Option<f32> {
        if ancestry.len() >= MAX_SCRIPT_RECURSION_DEPTH
            || ancestry.contains(&(current_file, script_hashcode))
        {
            return None;
        }
        let script = render_store.get_script(current_file, script_hashcode)?;
        ancestry.push((current_file, script_hashcode));

        let mut first = None::<f32>;
        for command in &script.commands {
            let command_start = script.time_at_frame(command.start.max(0) as f32);
            let candidate = match command.data {
                UXGeoScriptCommandData::Entity { hashcode, file } => {
                    let file = if file == u32::MAX || hashcode.is_local() {
                        current_file
                    } else {
                        file
                    };
                    render_store
                        .get_entity(file, hashcode)
                        .filter(|entity| entity.has_renderable_geometry())
                        .map(|_| command_start)
                }
                UXGeoScriptCommandData::Animation {
                    skin_file,
                    skin_hashcode,
                    anim_file,
                    anim_hashcode,
                } => {
                    let animation_file = if anim_file == u32::MAX || anim_hashcode.is_local() {
                        current_file
                    } else {
                        anim_file
                    };
                    let explicit_skin_file = if skin_file == u32::MAX || skin_hashcode.is_local() {
                        current_file
                    } else {
                        skin_file
                    };
                    let (resolved_skin_file, resolved_skin) = if skin_hashcode == u32::MAX {
                        (
                            animation_file,
                            render_store
                                .get_animation_runtime(animation_file)
                                .and_then(|runtime| runtime.bound_skin_hashcode(anim_hashcode)),
                        )
                    } else {
                        (explicit_skin_file, Some(skin_hashcode))
                    };
                    resolved_skin
                        .filter(|skin| {
                            render_store
                                .get_animskin_entities(resolved_skin_file, *skin)
                                .is_some_and(|entities| !entities.is_empty())
                        })
                        .map(|_| command_start)
                }
                UXGeoScriptCommandData::Particle { hashcode, file } => {
                    let file = if file == u32::MAX || hashcode.is_local() {
                        current_file
                    } else {
                        file
                    };
                    render_store
                        .get_particle(file, hashcode)
                        .map(|_| command_start)
                }
                UXGeoScriptCommandData::SubScript { hashcode, file } => {
                    let file = if file == u32::MAX || hashcode.is_local() {
                        current_file
                    } else {
                        file
                    };
                    visit(file, hashcode, render_store, ancestry)
                        .map(|child_time| command_start + child_time)
                }
                _ => None,
            };
            if let Some(candidate) = candidate {
                first = Some(first.map_or(candidate, |value| value.min(candidate)));
            }
        }

        ancestry.pop();
        first
    }

    visit(current_file, script_hashcode, render_store, &mut Vec::new())
}

fn command_controller(
    controllers: &[EXGeoAnimScriptControllerHeader],
    controller_header_index: u16,
) -> Option<&EXGeoAnimScriptControllerHeader> {
    if controller_header_index == u16::MAX {
        return None;
    }

    controllers.get(controller_header_index as usize)
}

fn interpolation_pair<const N: usize>(
    values: &[(f32, [f32; N])],
    current_frame: f32,
    command_start: f32,
    default: [f32; N],
) -> (f32, [f32; N], f32, [f32; N]) {
    let previous_index = values
        .iter()
        .rposition(|(frame, _)| *frame <= current_frame);

    if let Some(previous_index) = previous_index {
        let (start, start_value) = values[previous_index];
        let (end, end_value) = values
            .get(previous_index + 1)
            .copied()
            .unwrap_or((start, start_value));
        (start, start_value, end, end_value)
    } else if let Some((end, end_value)) = values.first().copied() {
        (command_start, default, end, end_value)
    } else {
        (command_start, default, command_start, default)
    }
}

fn finite_vec3(values: [f32; 3], default: Vec3) -> Vec3 {
    if values.iter().all(|value| value.is_finite()) {
        Vec3::from(values)
    } else {
        default
    }
}

fn finite_quat(values: [f32; 4]) -> Quat {
    if !values.iter().all(|value| value.is_finite()) {
        return Quat::IDENTITY;
    }

    let value = Quat::from_array(values);
    if value.length_squared().is_finite() && value.length_squared() > f32::EPSILON {
        value.normalize()
    } else {
        Quat::IDENTITY
    }
}

fn controller_transform(
    script: &UXGeoScript,
    command: &UXGeoScriptCommand,
    controller: Option<&EXGeoAnimScriptControllerHeader>,
    current_time: f32,
) -> (Vec3, Quat, Vec3) {
    let Some(controller) = controller else {
        return (Vec3::ZERO, Quat::IDENTITY, Vec3::ONE);
    };

    let current_frame = script.frame_at_time(current_time);
    let mut position = Vec3::ZERO;
    let mut rotation = Quat::IDENTITY;
    let mut scale = Vec3::ONE;

    if !controller.channels.quat_0.is_empty() {
        let (start, start_value, end, end_value) = interpolation_pair(
            &controller.channels.quat_0,
            current_frame,
            command.start as f32,
            Quat::IDENTITY.to_array(),
        );
        let start_value = finite_quat(start_value);
        let end_value = finite_quat(end_value);
        rotation = if start == end {
            start_value
        } else {
            let offset = ((current_frame - start) / (end - start)).clamp(0.0, 1.0);
            start_value.slerp(end_value, offset).normalize()
        };
    }

    if !controller.channels.vector_0.is_empty() {
        let (start, start_value, end, end_value) = interpolation_pair(
            &controller.channels.vector_0,
            current_frame,
            command.start as f32,
            Vec3::ZERO.to_array(),
        );
        let start_value = finite_vec3(start_value, Vec3::ZERO);
        let end_value = finite_vec3(end_value, start_value);
        position = if start == end {
            start_value
        } else {
            let offset = ((current_frame - start) / (end - start)).clamp(0.0, 1.0);
            start_value.lerp(end_value, ease_in_out_sine(offset))
        };
    }

    if !controller.channels.vector_1.is_empty() {
        let (start, start_value, end, end_value) = interpolation_pair(
            &controller.channels.vector_1,
            current_frame,
            command.start as f32,
            Vec3::ONE.to_array(),
        );
        let start_value = finite_vec3(start_value, Vec3::ONE);
        let end_value = finite_vec3(end_value, start_value);
        scale = if start == end {
            start_value
        } else {
            let offset = ((current_frame - start) / (end - start)).clamp(0.0, 1.0);
            start_value.lerp(end_value, offset)
        };
    }

    (position, rotation, scale)
}

pub fn render_script<F>(
    position: Vec3,
    rotation: Quat,
    scale: Vec3,
    current_file: Hashcode,
    script_hashcode: Hashcode,
    current_time: f32,
    render_store: &RenderStore,
    render: &mut F,
    hashcode_stack: Vec<u32>,
) where
    F: FnMut(QueuedEntityRender),
{
    render_script_with_mode(
        position,
        rotation,
        scale,
        current_file,
        script_hashcode,
        current_time,
        render_store,
        render,
        hashcode_stack,
        false,
        true,
    );
}

pub fn render_script_without_static_animations<F>(
    position: Vec3,
    rotation: Quat,
    scale: Vec3,
    current_file: Hashcode,
    script_hashcode: Hashcode,
    current_time: f32,
    render_store: &RenderStore,
    render: &mut F,
    hashcode_stack: Vec<u32>,
) where
    F: FnMut(QueuedEntityRender),
{
    render_script_with_mode(
        position,
        rotation,
        scale,
        current_file,
        script_hashcode,
        current_time,
        render_store,
        render,
        hashcode_stack,
        false,
        false,
    );
}

/// Queues every command in a sky script. Map skies are static compositions,
/// not timeline previews, so commands with a non-zero start frame must remain visible.
pub fn render_static_script<F>(
    position: Vec3,
    rotation: Quat,
    scale: Vec3,
    current_file: Hashcode,
    script_hashcode: Hashcode,
    current_time: f32,
    render_store: &RenderStore,
    render: &mut F,
    hashcode_stack: Vec<u32>,
) where
    F: FnMut(QueuedEntityRender),
{
    render_script_with_mode(
        position,
        rotation,
        scale,
        current_file,
        script_hashcode,
        current_time,
        render_store,
        render,
        hashcode_stack,
        true,
        true,
    );
}

pub fn render_static_script_without_animations<F>(
    position: Vec3,
    rotation: Quat,
    scale: Vec3,
    current_file: Hashcode,
    script_hashcode: Hashcode,
    current_time: f32,
    render_store: &RenderStore,
    render: &mut F,
    hashcode_stack: Vec<u32>,
) where
    F: FnMut(QueuedEntityRender),
{
    render_script_with_mode(
        position,
        rotation,
        scale,
        current_file,
        script_hashcode,
        current_time,
        render_store,
        render,
        hashcode_stack,
        true,
        false,
    );
}

#[allow(clippy::too_many_arguments)]
fn render_script_with_mode<F>(
    position: Vec3,
    rotation: Quat,
    scale: Vec3,
    current_file: Hashcode,
    script_hashcode: Hashcode,
    current_time: f32,
    render_store: &RenderStore,
    render: &mut F,
    hashcode_stack: Vec<u32>,
    static_scene: bool,
    include_animation_geometry: bool,
) where
    F: FnMut(QueuedEntityRender),
{
    puffin::profile_function!();

    if hashcode_stack.len() >= MAX_SCRIPT_RECURSION_DEPTH {
        return;
    }

    let script = render_store.get_script(current_file, script_hashcode);
    if script.is_none() {
        return;
    }
    let script = script.unwrap();

    // Exact frame times such as 2.0 / 30.0 can multiply back to
    // 1.9999999 in f32. Add a tiny tolerance in frame units before floor so
    // commands start on their serialized frame instead of one update late.
    let current_frame = (script.frame_at_time(current_time) + 1.0e-4).floor() as isize;
    let current_frame_commands: Vec<&UXGeoScriptCommand> = script
        .commands
        .iter()
        .filter(|c| static_scene || c.range().contains(&current_frame))
        .collect();

    let transforms = current_frame_commands
        .iter()
        .map(|command| {
            let controller =
                command_controller(&script.controllers, command.controller_header_index);
            controller_transform(script, command, controller, current_time)
        })
        .collect::<Vec<_>>();

    let mut ancestry = hashcode_stack;
    if !ancestry.contains(&script_hashcode) {
        ancestry.push(script_hashcode);
    }

    for (c, transform) in current_frame_commands.iter().zip(&transforms) {
        // ROBOTS_PATCH_0020_SCRIPT_LOCAL_REFERENCE_RESOLUTION
        // EdbFile::add_reference treats local hashes as current-file references
        // regardless of the serialized file field. Rendering must use the same rule.
        match c.data {
            UXGeoScriptCommandData::Entity {
                hashcode,
                file: entity_file,
            } => render(QueuedEntityRender {
                entity: (
                    if entity_file == u32::MAX || hashcode.is_local() {
                        current_file
                    } else {
                        entity_file
                    },
                    hashcode,
                ),
                entity_alt: None,
                position: position + rotation.mul_vec3(scale * transform.0),
                rotation: rotation * transform.1,
                scale: scale * transform.2,
            }),
            // ROBOTS_PATCH_0024_RENDER_ANIMATION_SKIN
            UXGeoScriptCommandData::Animation {
                skin_file,
                skin_hashcode,
                anim_file,
                anim_hashcode,
            } => {
                if include_animation_geometry {
                    let resolved_anim_file = if anim_file == u32::MAX || anim_hashcode.is_local() {
                        current_file
                    } else {
                        anim_file
                    };
                    let (resolved_skin_file, resolved_skin_hashcode) = if skin_hashcode == u32::MAX
                    {
                        let bound_skin = render_store
                            .get_animation_runtime(resolved_anim_file)
                            .and_then(|runtime| runtime.bound_skin_hashcode(anim_hashcode));
                        (resolved_anim_file, bound_skin)
                    } else {
                        let file = if skin_file == u32::MAX || skin_hashcode.is_local() {
                            current_file
                        } else {
                            skin_file
                        };
                        (file, Some(skin_hashcode))
                    };

                    if let Some(resolved_skin_hashcode) = resolved_skin_hashcode {
                        if let Some(entity_hashcodes) = render_store
                            .get_animskin_entities(resolved_skin_file, resolved_skin_hashcode)
                        {
                            for entity_hashcode in entity_hashcodes {
                                render(QueuedEntityRender {
                                    entity: (resolved_skin_file, *entity_hashcode),
                                    entity_alt: None,
                                    position: position + rotation.mul_vec3(scale * transform.0),
                                    rotation: rotation * transform.1,
                                    scale: scale * transform.2,
                                });
                            }
                        }
                    }
                }
            }
            UXGeoScriptCommandData::SubScript { hashcode, file } => {
                if ancestry.contains(&hashcode) {
                    continue;
                }

                let child_time = (current_time - script.time_at_frame(c.start as f32)).max(0.0);
                let mut child_ancestry = ancestry.clone();
                child_ancestry.push(hashcode);

                render_script_with_mode(
                    position + rotation.mul_vec3(scale * transform.0),
                    rotation * transform.1,
                    scale * transform.2,
                    if file == u32::MAX || hashcode.is_local() {
                        current_file
                    } else {
                        file
                    },
                    hashcode,
                    child_time,
                    render_store,
                    render,
                    child_ancestry,
                    static_scene,
                    include_animation_geometry,
                );
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct QueuedAnimationRender {
    pub animation: (Hashcode, Hashcode),
    pub skin: (Hashcode, Hashcode),
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    pub phase: f32,
}

#[allow(clippy::too_many_arguments)]
pub fn collect_script_animations(
    position: Vec3,
    rotation: Quat,
    scale: Vec3,
    current_file: Hashcode,
    script_hashcode: Hashcode,
    current_time: f32,
    render_store: &RenderStore,
    queue: &mut Vec<QueuedAnimationRender>,
    hashcode_stack: Vec<u32>,
    static_scene: bool,
) {
    if hashcode_stack.len() >= MAX_SCRIPT_RECURSION_DEPTH {
        return;
    }
    let Some(script) = render_store.get_script(current_file, script_hashcode) else {
        return;
    };
    let current_frame = (script.frame_at_time(current_time) + 1.0e-4).floor() as isize;
    let commands = script
        .commands
        .iter()
        .filter(|command| static_scene || command.range().contains(&current_frame))
        .collect::<Vec<_>>();
    let transforms = commands
        .iter()
        .map(|command| {
            let controller =
                command_controller(&script.controllers, command.controller_header_index);
            controller_transform(script, command, controller, current_time)
        })
        .collect::<Vec<_>>();

    let mut ancestry = hashcode_stack;
    if !ancestry.contains(&script_hashcode) {
        ancestry.push(script_hashcode);
    }

    for (command, transform) in commands.into_iter().zip(transforms) {
        match command.data {
            UXGeoScriptCommandData::Animation {
                skin_file,
                skin_hashcode,
                anim_file,
                anim_hashcode,
            } => {
                let resolved_animation_file = if anim_file == u32::MAX || anim_hashcode.is_local() {
                    current_file
                } else {
                    anim_file
                };
                let resolved_skin_file = if skin_file == u32::MAX || skin_hashcode.is_local() {
                    current_file
                } else {
                    skin_file
                };
                let fps = script.timeline_framerate().max(f32::EPSILON);
                let duration = f32::from(command.length.max(1)) / fps;
                let start_time = script.time_at_frame(command.start as f32);
                let local_time = (current_time - start_time).clamp(0.0, duration);
                let phase = (local_time / duration).clamp(0.0, 1.0);
                queue.push(QueuedAnimationRender {
                    animation: (resolved_animation_file, anim_hashcode),
                    skin: (resolved_skin_file, skin_hashcode),
                    position: position + rotation.mul_vec3(scale * transform.0),
                    rotation: rotation * transform.1,
                    scale: scale * transform.2,
                    phase,
                });
            }
            UXGeoScriptCommandData::SubScript { hashcode, file } => {
                if ancestry.contains(&hashcode) {
                    continue;
                }
                let child_time =
                    (current_time - script.time_at_frame(command.start as f32)).max(0.0);
                let mut child_ancestry = ancestry.clone();
                child_ancestry.push(hashcode);
                collect_script_animations(
                    position + rotation.mul_vec3(scale * transform.0),
                    rotation * transform.1,
                    scale * transform.2,
                    if file == u32::MAX || hashcode.is_local() {
                        current_file
                    } else {
                        file
                    },
                    hashcode,
                    child_time,
                    render_store,
                    queue,
                    child_ancestry,
                    static_scene,
                );
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ParticleTimelineNode {
    pub file: Hashcode,
    pub script: Hashcode,
    pub command_index: usize,
}

#[derive(Debug, Clone)]
pub struct QueuedParticleRender {
    pub particle: (Hashcode, Hashcode),
    pub base_position: Vec3,
    pub base_rotation: Quat,
    pub base_scale: Vec3,
    pub timeline: Vec<ParticleTimelineNode>,
    pub emission_start_root_time: f32,
    pub local_time: f32,
    pub duration: f32,
}

pub fn sample_particle_emitter_transform(
    emitter: &QueuedParticleRender,
    render_store: &RenderStore,
    root_time: f32,
) -> (Vec3, Quat, Vec3) {
    let mut position = emitter.base_position;
    let mut rotation = emitter.base_rotation;
    let mut scale = emitter.base_scale;
    let mut script_origin = 0.0f32;

    for node in &emitter.timeline {
        let Some(script) = render_store.get_script(node.file, node.script) else {
            break;
        };
        let Some(command) = script.commands.get(node.command_index) else {
            break;
        };
        let script_time = (root_time - script_origin).max(0.0);
        let controller = command_controller(&script.controllers, command.controller_header_index);
        let transform = controller_transform(script, command, controller, script_time);
        position += rotation.mul_vec3(scale * transform.0);
        rotation *= transform.1;
        scale *= transform.2;

        if matches!(command.data, UXGeoScriptCommandData::SubScript { .. }) {
            script_origin += script.time_at_frame(command.start as f32);
        }
    }

    (position, rotation, scale)
}

#[allow(clippy::too_many_arguments)]
pub fn collect_script_particles(
    position: Vec3,
    rotation: Quat,
    scale: Vec3,
    current_file: Hashcode,
    script_hashcode: Hashcode,
    current_time: f32,
    render_store: &RenderStore,
    output: &mut Vec<QueuedParticleRender>,
    ancestry: Vec<Hashcode>,
) {
    collect_script_particles_inner(
        position,
        rotation,
        scale,
        current_file,
        script_hashcode,
        current_time,
        0.0,
        render_store,
        output,
        ancestry,
        Vec::new(),
    );
}

#[allow(clippy::too_many_arguments)]
fn collect_script_particles_inner(
    base_position: Vec3,
    base_rotation: Quat,
    base_scale: Vec3,
    current_file: Hashcode,
    script_hashcode: Hashcode,
    current_root_time: f32,
    script_origin_root_time: f32,
    render_store: &RenderStore,
    output: &mut Vec<QueuedParticleRender>,
    ancestry: Vec<Hashcode>,
    timeline: Vec<ParticleTimelineNode>,
) {
    if ancestry.len() >= MAX_SCRIPT_RECURSION_DEPTH || ancestry.contains(&script_hashcode) {
        return;
    }
    let Some(script) = render_store.get_script(current_file, script_hashcode) else {
        return;
    };

    let current_script_time = (current_root_time - script_origin_root_time).max(0.0);
    let mut ancestry = ancestry;
    ancestry.push(script_hashcode);

    for (command_index, command) in script.commands.iter().enumerate() {
        let command_start = script.time_at_frame(command.start as f32);
        if current_script_time + f32::EPSILON < command_start {
            continue;
        }
        let command_duration = script
            .time_at_frame(command.length.max(1) as f32)
            .max(1.0 / 60.0);
        let local_time = (current_script_time - command_start).max(0.0);
        let mut command_timeline = timeline.clone();
        command_timeline.push(ParticleTimelineNode {
            file: current_file,
            script: script_hashcode,
            command_index,
        });

        match command.data {
            UXGeoScriptCommandData::Particle { hashcode, file } => {
                let particle_file = if file == u32::MAX || hashcode.is_local() {
                    current_file
                } else {
                    file
                };
                let max_lifetime = render_store
                    .get_particle(particle_file, hashcode)
                    .map(|particle| particle.lifetime_center() + particle.lifetime_extent())
                    .unwrap_or_default()
                    .max(1.0 / 60.0);
                if local_time <= command_duration + max_lifetime {
                    output.push(QueuedParticleRender {
                        particle: (particle_file, hashcode),
                        base_position,
                        base_rotation,
                        base_scale,
                        timeline: command_timeline,
                        emission_start_root_time: script_origin_root_time + command_start,
                        local_time,
                        duration: command_duration,
                    });
                }
            }
            UXGeoScriptCommandData::SubScript { hashcode, file } => {
                collect_script_particles_inner(
                    base_position,
                    base_rotation,
                    base_scale,
                    if file == u32::MAX || hashcode.is_local() {
                        current_file
                    } else {
                        file
                    },
                    hashcode,
                    current_root_time,
                    script_origin_root_time + command_start,
                    render_store,
                    output,
                    ancestry.clone(),
                    command_timeline,
                );
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::entity::EntityRenderer;
    use eurochef_edb::{script::EXGeoAnimScriptControllerChannels, versions::Platform};
    use eurochef_shared::script::{UXGeoScript, UXGeoScriptCommand, UXGeoScriptCommandData};

    fn controller(x: f32) -> EXGeoAnimScriptControllerHeader {
        EXGeoAnimScriptControllerHeader {
            controller_count: 1,
            channel_count: 1,
            ctrl_mask: 0x4,
            ctrl_channel_mask: 0x4,
            channels: EXGeoAnimScriptControllerChannels {
                vector_0: vec![(0.0, [x, 0.0, 0.0])],
                ..Default::default()
            },
        }
    }

    fn identity_controller() -> EXGeoAnimScriptControllerHeader {
        EXGeoAnimScriptControllerHeader {
            controller_count: 0,
            channel_count: 0,
            ctrl_mask: 0,
            ctrl_channel_mask: 0,
            channels: EXGeoAnimScriptControllerChannels::default(),
        }
    }

    fn test_script(
        hashcode: Hashcode,
        length: u32,
        commands: Vec<UXGeoScriptCommand>,
    ) -> UXGeoScript {
        UXGeoScript {
            hashcode,
            framerate: 30.0,
            length,
            num_threads: 1,
            commands,
            serialized_controller_count: 0,
            controller_record_metadata: vec![],
            controllers: vec![],
            controller_group_indices: vec![],
            controller_groups: vec![],
        }
    }

    #[test]
    fn first_resolved_visual_time_descends_into_subscripts() {
        let file = 0x0100_001D;
        let parent = 0x0400_1000;
        let child = 0x0400_1001;
        let entity = 0x0200_1234;
        let zero_anchor = 0x0200_1235;
        let mut store = RenderStore::new();
        let mut visible_renderer = EntityRenderer::new(file, Platform::Pc);
        visible_renderer.set_serialized_vertex_count_for_test(3);
        store.insert_entity(file, entity, 0, visible_renderer);
        let mut zero_renderer = EntityRenderer::new(file, Platform::Pc);
        zero_renderer.set_serialized_vertex_count_for_test(0);
        store.insert_entity(file, zero_anchor, 1, zero_renderer);
        store.insert_script(
            file,
            test_script(
                child,
                90,
                vec![UXGeoScriptCommand {
                    opcode: 3,
                    start: 30,
                    length: 30,
                    controller_header_index: u16::MAX,
                    controller_index: u8::MAX,
                    parent_controller_index: u8::MAX,
                    data: UXGeoScriptCommandData::Entity {
                        hashcode: entity,
                        file: u32::MAX,
                    },
                }],
            ),
        );
        store.insert_script(
            file,
            test_script(
                parent,
                120,
                vec![
                    UXGeoScriptCommand {
                        opcode: 3,
                        start: 0,
                        length: 120,
                        controller_header_index: u16::MAX,
                        controller_index: u8::MAX,
                        parent_controller_index: u8::MAX,
                        data: UXGeoScriptCommandData::Entity {
                            hashcode: zero_anchor,
                            file: u32::MAX,
                        },
                    },
                    UXGeoScriptCommand {
                        opcode: 4,
                        start: 15,
                        length: 90,
                        controller_header_index: u16::MAX,
                        controller_index: u8::MAX,
                        parent_controller_index: u8::MAX,
                        data: UXGeoScriptCommandData::SubScript {
                            hashcode: child,
                            file: u32::MAX,
                        },
                    },
                ],
            ),
        );

        let resolved = first_resolved_visual_time(file, parent, &store).unwrap();
        assert!((resolved - 1.5).abs() < f32::EPSILON);
    }

    #[test]
    fn vehicle_commands_use_direct_controller_indices() {
        let current_file = 0x0100_00C1;
        let script_hashcode = 0x0400_026F;
        let entities = [
            0x8200_0002,
            0x8200_0000,
            0x8200_0001,
            0x8200_0001,
            0x8200_0001,
        ];
        let commands = entities
            .iter()
            .enumerate()
            .map(|(index, hashcode)| UXGeoScriptCommand {
                opcode: 3,
                start: 0,
                length: 61,
                controller_header_index: index as u16,
                controller_index: index as u8,
                parent_controller_index: u8::MAX,
                data: UXGeoScriptCommandData::Entity {
                    hashcode: *hashcode,
                    file: u32::MAX,
                },
            })
            .collect();

        let script = UXGeoScript {
            hashcode: script_hashcode,
            framerate: 30.0,
            length: 61,
            num_threads: 5,
            commands,
            serialized_controller_count: 4,
            controller_record_metadata: vec![[0, 0], [3, 1], [3, 1], [3, 1], [3, 1]],
            controllers: vec![
                identity_controller(),
                controller(1.0),
                controller(2.0),
                controller(3.0),
                controller(4.0),
            ],
            controller_group_indices: vec![vec![0], vec![1], vec![2], vec![3], vec![4]],
            controller_groups: vec![
                vec![identity_controller()],
                vec![controller(1.0)],
                vec![controller(2.0)],
                vec![controller(3.0)],
                vec![controller(4.0)],
            ],
        };
        let mut store = RenderStore::new();
        store.insert_script(current_file, script);

        let mut positions = Vec::new();
        render_script(
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::ONE,
            current_file,
            script_hashcode,
            0.0,
            &store,
            &mut |queued| positions.push(queued.position.x),
            vec![],
        );

        assert_eq!(positions, vec![0.0, 1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn static_script_keeps_commands_outside_the_preview_frame() {
        let file = 0x0100_0001;
        let script_hashcode = 0x0400_0001;
        let mut store = RenderStore::new();
        store.insert_script(
            file,
            UXGeoScript {
                hashcode: script_hashcode,
                framerate: 30.0,
                length: 120,
                num_threads: 1,
                commands: vec![UXGeoScriptCommand {
                    opcode: 3,
                    start: 90,
                    length: 30,
                    controller_header_index: u16::MAX,
                    controller_index: u8::MAX,
                    parent_controller_index: u8::MAX,
                    data: UXGeoScriptCommandData::Entity {
                        hashcode: 0x0200_017D,
                        file: u32::MAX,
                    },
                }],
                serialized_controller_count: 0,
                controller_record_metadata: vec![],
                controllers: vec![],
                controller_group_indices: vec![],
                controller_groups: vec![],
            },
        );

        let mut queued = Vec::new();
        render_static_script(
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::ONE,
            file,
            script_hashcode,
            0.0,
            &store,
            &mut |render| queued.push(render.entity),
            vec![],
        );

        assert_eq!(queued, vec![(file, 0x0200_017D)]);
    }

    #[test]
    fn script_8400000a_entity_is_visible_only_on_frames_2_through_6() {
        let file = 0x0100_0012;
        let script_hashcode = 0x8400_000A;
        let mut store = RenderStore::new();
        for index in 0..10u32 {
            store.insert_script(
                file,
                UXGeoScript {
                    hashcode: 0x8400_0000 | index,
                    framerate: 30.0,
                    length: 1,
                    num_threads: 0,
                    commands: vec![],
                    serialized_controller_count: 0,
                    controller_record_metadata: vec![],
                    controllers: vec![],
                    controller_group_indices: vec![],
                    controller_groups: vec![],
                },
            );
        }
        store.insert_script(
            file,
            UXGeoScript {
                hashcode: script_hashcode,
                framerate: 30.0,
                length: 7,
                num_threads: 1,
                commands: vec![UXGeoScriptCommand {
                    opcode: 3,
                    start: 2,
                    length: 5,
                    controller_header_index: 0,
                    controller_index: 2,
                    parent_controller_index: u8::MAX,
                    data: UXGeoScriptCommandData::Entity {
                        hashcode: 0x8200_0001,
                        file: u32::MAX,
                    },
                }],
                serialized_controller_count: 1,
                controller_record_metadata: vec![[0, 0]],
                controllers: vec![identity_controller()],
                controller_group_indices: vec![vec![0]],
                controller_groups: vec![vec![identity_controller()]],
            },
        );

        for frame in 0..=7 {
            let mut queued = Vec::new();
            render_script(
                Vec3::ZERO,
                Quat::IDENTITY,
                Vec3::ONE,
                file,
                script_hashcode,
                frame as f32 / 30.0,
                &store,
                &mut |render| queued.push(render.entity),
                vec![],
            );

            let expected = if (2..=6).contains(&frame) {
                vec![(file, 0x8200_0001)]
            } else {
                vec![]
            };
            assert_eq!(queued, expected, "frame {frame}");
        }
    }

    #[test]
    fn particle_commands_use_serialized_fps_and_command_local_time() {
        let file = 0x0100_0097;
        let script_hashcode = 0x0400_021D;
        let mut store = RenderStore::new();
        store.insert_script(
            file,
            UXGeoScript {
                hashcode: script_hashcode,
                framerate: 60.0,
                length: 120,
                num_threads: 1,
                commands: vec![UXGeoScriptCommand {
                    opcode: 6,
                    start: 30,
                    length: 60,
                    controller_header_index: u16::MAX,
                    controller_index: u8::MAX,
                    parent_controller_index: u8::MAX,
                    data: UXGeoScriptCommandData::Particle {
                        hashcode: 0x9100_0002,
                        file: u32::MAX,
                    },
                }],
                serialized_controller_count: 0,
                controller_record_metadata: vec![],
                controllers: vec![],
                controller_group_indices: vec![],
                controller_groups: vec![],
            },
        );

        let mut before = Vec::new();
        collect_script_particles(
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::ONE,
            file,
            script_hashcode,
            29.0 / 60.0,
            &store,
            &mut before,
            vec![],
        );
        assert!(before.is_empty());

        let mut active = Vec::new();
        collect_script_particles(
            Vec3::new(1.0, 2.0, 3.0),
            Quat::IDENTITY,
            Vec3::ONE,
            file,
            script_hashcode,
            60.0 / 60.0,
            &store,
            &mut active,
            vec![],
        );
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].particle, (file, 0x9100_0002));
        let sampled = sample_particle_emitter_transform(
            &active[0],
            &store,
            active[0].emission_start_root_time + 0.25,
        );
        assert_eq!(sampled.0, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(sampled.1, Quat::IDENTITY);
        assert_eq!(sampled.2, Vec3::ONE);
        assert!((active[0].local_time - 0.5).abs() < f32::EPSILON);
        assert!((active[0].duration - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn animation_commands_use_serialized_duration_and_complete_asset_phase() {
        let file = 0x0100_0086;
        let script_hashcode = 0x0400_002A;
        let mut store = RenderStore::new();
        store.insert_script(
            file,
            UXGeoScript {
                hashcode: script_hashcode,
                framerate: 60.0,
                length: 120,
                num_threads: 1,
                commands: vec![UXGeoScriptCommand {
                    opcode: 4,
                    start: 30,
                    length: 60,
                    controller_header_index: u16::MAX,
                    controller_index: u8::MAX,
                    parent_controller_index: u8::MAX,
                    data: UXGeoScriptCommandData::Animation {
                        skin_file: u32::MAX,
                        skin_hashcode: 0x8D00_0000,
                        anim_file: u32::MAX,
                        anim_hashcode: 0x8300_0002,
                    },
                }],
                serialized_controller_count: 0,
                controller_record_metadata: vec![],
                controllers: vec![],
                controller_group_indices: vec![],
                controller_groups: vec![],
            },
        );

        let mut before = Vec::new();
        collect_script_animations(
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::ONE,
            file,
            script_hashcode,
            29.0 / 60.0,
            &store,
            &mut before,
            vec![],
            false,
        );
        assert!(before.is_empty());

        let mut middle = Vec::new();
        collect_script_animations(
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::ONE,
            file,
            script_hashcode,
            60.0 / 60.0,
            &store,
            &mut middle,
            vec![],
            false,
        );
        assert_eq!(middle.len(), 1);
        assert_eq!(middle[0].animation, (file, 0x8300_0002));
        assert_eq!(middle[0].skin, (file, 0x8D00_0000));
        assert!((middle[0].phase - 0.5).abs() < 1.0e-6);

        let mut end = Vec::new();
        collect_script_animations(
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::ONE,
            file,
            script_hashcode,
            89.0 / 60.0,
            &store,
            &mut end,
            vec![],
            false,
        );
        assert_eq!(end.len(), 1);
        assert!((end[0].phase - 59.0 / 60.0).abs() < 1.0e-6);
    }

    #[test]
    fn script_84000015_rotates_entity_82000050_on_the_serialized_keys() {
        let command = UXGeoScriptCommand {
            opcode: 3,
            start: 0,
            length: 285,
            controller_header_index: 0,
            controller_index: 1,
            parent_controller_index: u8::MAX,
            data: UXGeoScriptCommandData::Entity {
                hashcode: 0x8200_0050,
                file: u32::MAX,
            },
        };
        let keys = vec![
            (0.0, [0.0, 0.5519369, 0.0, 0.83388585]),
            (100.0, [0.0, -0.3826834, 0.0, 0.9238795]),
            (140.0, [0.0, -0.3826834, 0.0, 0.9238795]),
            (241.0, [0.0, 0.55193686, 0.0, 0.83388585]),
        ];
        let controller = EXGeoAnimScriptControllerHeader {
            controller_count: 1,
            channel_count: 4,
            ctrl_mask: 0x8,
            ctrl_channel_mask: 0x8,
            channels: EXGeoAnimScriptControllerChannels {
                quat_0: keys.clone(),
                ..Default::default()
            },
        };
        let script = UXGeoScript {
            hashcode: 0x8400_0015,
            framerate: 30.0,
            length: 285,
            num_threads: 4,
            commands: vec![command.clone()],
            serialized_controller_count: 1,
            controller_record_metadata: vec![[4, 1]],
            controllers: vec![controller.clone()],
            controller_group_indices: vec![vec![0]],
            controller_groups: vec![vec![controller.clone()]],
        };

        for (frame, expected) in keys {
            let (position, rotation, scale) = controller_transform(
                &script,
                &command,
                Some(&controller),
                script.time_at_frame(frame),
            );
            assert_eq!(position, Vec3::ZERO);
            assert_eq!(scale, Vec3::ONE);
            let expected = Quat::from_array(expected).normalize();
            assert!(rotation.dot(expected).abs() > 0.99999, "frame {frame}");
        }
    }
}
