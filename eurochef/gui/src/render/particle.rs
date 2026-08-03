use std::collections::BTreeMap;

use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use eurochef_edb::particle::EXGeoParticle;
#[cfg(test)]
use eurochef_edb::particle::EXGeoParticleCurveRecord;
use glam::{EulerRot, Mat4, Quat, Vec3, Vec4};
use glow::HasContext;

use super::{
    blend::{set_blending_mode, BlendMode},
    script::{sample_particle_emitter_transform, QueuedParticleRender},
    viewer::RenderContext,
    RenderStore,
};

#[derive(Debug, Clone, Copy)]
pub struct ParticlePreviewSettings {
    pub enabled: bool,
    /// Safety cap only. Native EXGeoParticle+0xC4 remains the actual per-emitter pool size.
    pub max_particles: usize,
}

impl Default for ParticlePreviewSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            max_particles: 4096,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NativeParticleInstance {
    pub birth_time: f32,
    pub position: Vec3,
    pub scale: Vec3,
    pub rotation: Vec3,
    pub colour: Vec4,
    pub age_percent: f32,
    pub resource_selector: Option<u32>,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuParticleInstance {
    position: [f32; 3],
    scale: [f32; 3],
    rotation: [f32; 3],
    colour: [f32; 4],
}

impl From<NativeParticleInstance> for GpuParticleInstance {
    fn from(value: NativeParticleInstance) -> Self {
        Self {
            position: value.position.to_array(),
            scale: value.scale.to_array(),
            rotation: value.rotation.to_array(),
            colour: value.colour.to_array(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct SpawnedParticle {
    birth_time: f32,
    lifetime: f32,
    initial_age_percent: f32,
    speed_curve_offset: f32,
    seed: u32,
    resource_selector: Option<u32>,
}

impl SpawnedParticle {
    fn expiry_time(self) -> f32 {
        let remaining = (100.0 - self.initial_age_percent).clamp(0.0, 100.0) * 0.01;
        self.birth_time + self.lifetime * remaining
    }
}

#[derive(Debug, Clone, Copy)]
struct NativeRng {
    state: u32,
}

impl NativeRng {
    fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    fn next_signed(&mut self) -> f32 {
        self.state = self
            .state
            .wrapping_mul(0x0019_660D)
            .wrapping_add(0x3C6E_F35F);
        // Robots converts state>>1 with 2^-30 and subtracts one.
        ((self.state >> 1) as f32).mul_add(2.0f32.powi(-30), -1.0)
    }

    fn next_index(&mut self, count: usize) -> usize {
        if count <= 1 {
            return 0;
        }
        let normalized = (self.next_signed() + 1.0) * 0.5;
        ((normalized * count as f32) as usize).min(count - 1)
    }
}

pub struct ParticleRenderer {
    quad: glow::VertexArray,
    instance_buffer: glow::Buffer,
}

impl ParticleRenderer {
    const VERTEX_DATA: &'static [[f32; 5]] = &[
        [-0.5, -0.5, 0.0, 0.0, 1.0],
        [-0.5, 0.5, 0.0, 0.0, 0.0],
        [0.5, -0.5, 0.0, 1.0, 1.0],
        [0.5, 0.5, 0.0, 1.0, 0.0],
    ];

    pub fn new(gl: &glow::Context) -> Result<Self, String> {
        unsafe {
            let quad = gl.create_vertex_array()?;
            gl.bind_vertex_array(Some(quad));

            let vertex_buffer = gl.create_buffer()?;
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vertex_buffer));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(Self::VERTEX_DATA),
                glow::STATIC_DRAW,
            );
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, 5 * 4, 0);
            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(
                1,
                2,
                glow::FLOAT,
                false,
                5 * 4,
                3 * std::mem::size_of::<f32>() as i32,
            );

            let instance_buffer = gl.create_buffer()?;
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(instance_buffer));
            let stride = std::mem::size_of::<GpuParticleInstance>() as i32;
            let mut offset = 0;
            for (location, components) in [(2, 3), (3, 3), (4, 3), (5, 4)] {
                gl.enable_vertex_attrib_array(location);
                gl.vertex_attrib_pointer_f32(
                    location,
                    components,
                    glow::FLOAT,
                    false,
                    stride,
                    offset,
                );
                gl.vertex_attrib_divisor(location, 1);
                offset += components * std::mem::size_of::<f32>() as i32;
            }
            gl.bind_vertex_array(None);

            Ok(Self {
                quad,
                instance_buffer,
            })
        }
    }

    pub unsafe fn render(
        &self,
        gl: &glow::Context,
        context: &RenderContext,
        emitter: &QueuedParticleRender,
        particle: &EXGeoParticle,
        settings: ParticlePreviewSettings,
        render_store: &RenderStore,
    ) {
        if !settings.enabled || particle.particle_type_selector == 0x1700_0001 {
            return;
        }

        let instances = simulate_native_particles(
            particle,
            emitter.local_time,
            emitter.duration,
            settings.max_particles,
        );
        if instances.is_empty() {
            return;
        }

        let file = emitter.particle.0;
        if let Some(render_entity) = particle.render_entity {
            if let Some(entity) = render_store.get_entity(file, render_entity) {
                for instance in &instances {
                    let spawn_root_time = emitter.emission_start_root_time + instance.birth_time;
                    let (emitter_position, emitter_rotation, emitter_scale) =
                        sample_particle_emitter_transform(emitter, render_store, spawn_root_time);
                    let position = emitter_position
                        + emitter_rotation.mul_vec3(emitter_scale * instance.position);
                    let rotation = emitter_rotation
                        * Quat::from_euler(
                            EulerRot::XYZ,
                            instance.rotation.x,
                            instance.rotation.y,
                            instance.rotation.z,
                        );
                    entity.draw_particle(
                        gl,
                        context,
                        position,
                        rotation,
                        emitter_scale * instance.scale,
                        instance.colour,
                        instance.resource_selector,
                        emitter.local_time as f64,
                        render_store,
                    );
                }
                return;
            }
        }

        let mut grouped: BTreeMap<Option<u32>, Vec<GpuParticleInstance>> = BTreeMap::new();
        for instance in instances {
            let spawn_root_time = emitter.emission_start_root_time + instance.birth_time;
            let (emitter_position, emitter_rotation, emitter_scale) =
                sample_particle_emitter_transform(emitter, render_store, spawn_root_time);
            let world_instance = NativeParticleInstance {
                birth_time: instance.birth_time,
                position: emitter_position
                    + emitter_rotation.mul_vec3(emitter_scale * instance.position),
                scale: emitter_scale * instance.scale,
                rotation: instance.rotation,
                colour: instance.colour,
                age_percent: instance.age_percent,
                resource_selector: instance.resource_selector,
            };
            grouped
                .entry(world_instance.resource_selector)
                .or_default()
                .push(world_instance.into());
        }

        let shader = context.shaders.particle;
        gl.use_program(Some(shader));
        gl.depth_mask(false);
        set_blending_mode(gl, particle_blend_mode(particle));
        gl.uniform_matrix_4_f32_slice(
            gl.get_uniform_location(shader, "u_view").as_ref(),
            false,
            &context.uniforms.view.to_cols_array(),
        );
        gl.uniform_matrix_4_f32_slice(
            gl.get_uniform_location(shader, "u_emitterModel").as_ref(),
            false,
            &Mat4::IDENTITY.to_cols_array(),
        );
        gl.uniform_matrix_4_f32_slice(
            gl.get_uniform_location(shader, "u_billboardRotation")
                .as_ref(),
            false,
            &Mat4::from_quat(-context.uniforms.camera_rotation).to_cols_array(),
        );
        gl.uniform_1_i32(gl.get_uniform_location(shader, "u_texture").as_ref(), 0);

        gl.bind_vertex_array(Some(self.quad));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.instance_buffer));
        for (selector, group) in grouped {
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&group),
                glow::DYNAMIC_DRAW,
            );
            let texture = selector.and_then(|selector| {
                resolved_texture_frame(render_store, file, selector, emitter.local_time)
            });
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, texture);
            gl.uniform_1_i32(
                gl.get_uniform_location(shader, "u_hasTexture").as_ref(),
                i32::from(texture.is_some()),
            );
            gl.draw_arrays_instanced(glow::TRIANGLE_STRIP, 0, 4, group.len() as i32);
        }
    }
}

fn particle_blend_mode(particle: &EXGeoParticle) -> BlendMode {
    let flags = particle.behavior_flags().0;
    if flags & 0x0008 != 0 {
        BlendMode::Additive
    } else if flags & 0x0010 != 0 {
        BlendMode::ReverseSubtract
    } else {
        BlendMode::Blend
    }
}

fn resolved_texture_frame(
    render_store: &RenderStore,
    file: u32,
    selector: u32,
    time: f32,
) -> Option<glow::Texture> {
    // Robots stores this as a flat material/texture index. The runtime renderer
    // decomposes it into page=(selector>>6), slot=(selector&0x3F) and indexes
    // 0x38-byte material records, but the serialized flat index remains exact.
    let (_, mut texture) = render_store.get_texture_by_index(file, selector as usize)?;
    if let Some((external_file, external_texture)) = texture.external_reference {
        texture = render_store.get_texture(external_file, external_texture)?;
    }
    if texture.frames.is_empty() {
        return None;
    }
    if texture.framerate == 0 || texture.frames.len() == 1 {
        return texture.frames.first().copied();
    }
    let frame_scale = texture.frame_count.max(1) as f32 / texture.frames.len() as f32;
    let frame_time = (1.0 / texture.framerate as f32) * frame_scale;
    let frame = ((time.max(0.0) / frame_time) as usize) % texture.frames.len();
    texture.frames.get(frame).copied()
}

pub fn simulate_native_particles(
    particle: &EXGeoParticle,
    local_time: f32,
    emission_duration: f32,
    safety_limit: usize,
) -> Vec<NativeParticleInstance> {
    let rate = particle.emission_rate();
    let pool_limit = particle.pool_limit().min(safety_limit);
    if rate <= f32::EPSILON || pool_limit == 0 || local_time <= 0.0 {
        return Vec::new();
    }

    let emission_end = local_time.min(emission_duration.max(0.0));
    let total_events = (emission_end * rate).floor().max(0.0) as usize;
    let total_events = total_events.min(100_000);
    let mut active = Vec::<SpawnedParticle>::with_capacity(pool_limit);

    for spawn_index in 0..total_events {
        let birth_time = (spawn_index + 1) as f32 / rate;
        active.retain(|entry| birth_time < entry.expiry_time());
        if active.len() >= pool_limit {
            continue;
        }

        let seed = particle
            .hashcode
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add(spawn_index as u32);
        let mut rng = NativeRng::new(seed);
        let lifetime = (particle.lifetime_center()
            + rng.next_signed() * particle.lifetime_extent())
        .max(particle.fixed_step());
        let emitter_age = emitter_age_percent(particle, birth_time);
        let flags = particle.behavior_flags().0;
        let initial_age_percent = if flags & 0x0100 != 0 {
            evaluate_curve(particle, 11, emitter_age, 0.0).clamp(0.0, 100.0)
        } else {
            0.0
        };
        let speed_curve_offset = if flags & 0x0200 != 0 {
            evaluate_curve(particle, 10, emitter_age, 0.0)
        } else {
            0.0
        };
        let resource_selector = select_resource(particle, spawn_index, &mut rng);
        active.push(SpawnedParticle {
            birth_time,
            lifetime,
            initial_age_percent,
            speed_curve_offset,
            seed: rng.state,
            resource_selector,
        });
    }

    active
        .into_iter()
        .filter(|entry| local_time < entry.expiry_time())
        .map(|entry| build_instance(particle, entry, local_time))
        .collect()
}

fn emitter_age_percent(particle: &EXGeoParticle, time: f32) -> f32 {
    let fixed_step = particle.fixed_step();
    let base = particle.lifetime_center().max(fixed_step);
    let increment = fixed_step * 100.0 / base;
    let steps = (time.max(0.0) / fixed_step).floor() as usize;

    if particle.behavior_flags().0 & 0x0001 != 0 {
        return base + steps as f32 * increment;
    }

    let first_reset = if base >= 100.0 {
        1
    } else {
        ((100.0 - base) / increment).ceil().max(1.0) as usize
    };
    if steps < first_reset {
        return base + steps as f32 * increment;
    }

    let cycle_steps = (100.0 / increment).ceil().max(1.0) as usize;
    ((steps - first_reset) % cycle_steps) as f32 * increment
}

fn select_resource(
    particle: &EXGeoParticle,
    spawn_index: usize,
    rng: &mut NativeRng,
) -> Option<u32> {
    let resources = &particle.render_resource_selectors;
    if resources.is_empty() {
        return None;
    }
    let index = match particle.resource_selection_mode() {
        1 if resources.len() > 1 => {
            let period = (resources.len() - 1) * 2;
            let phase = spawn_index % period;
            if phase < resources.len() {
                phase
            } else {
                period - phase
            }
        }
        2 => rng.next_index(resources.len()),
        _ => spawn_index % resources.len(),
    };
    resources.get(index).copied()
}

fn build_instance(
    particle: &EXGeoParticle,
    spawned: SpawnedParticle,
    local_time: f32,
) -> NativeParticleInstance {
    let mut rng = NativeRng::new(spawned.seed);
    let center = Vec3::from_array(particle.spawn_position_center());
    let extent = Vec3::from_array(particle.spawn_position_extent());
    let mut position =
        center + extent * Vec3::new(rng.next_signed(), rng.next_signed(), rng.next_signed());

    let azimuth = particle.azimuth_center() + rng.next_signed() * particle.azimuth_extent();
    let elevation = particle.elevation_center() + rng.next_signed() * particle.elevation_extent();
    let speed = particle.speed_center()
        + rng.next_signed() * particle.speed_extent()
        + spawned.speed_curve_offset;
    let direction = Vec3::new(
        elevation.cos() * azimuth.sin(),
        elevation.sin(),
        elevation.cos() * azimuth.cos(),
    );
    let mut velocity = direction * speed;

    let scale_center = Vec3::from_array(particle.initial_scale_center());
    let scale_extent = Vec3::from_array(particle.initial_scale_extent());
    let shared_scale_random = rng.next_signed();
    let mut scale = if particle.behavior_flags().0 & 0x40 != 0 {
        scale_center + scale_extent * shared_scale_random
    } else {
        scale_center
            + scale_extent * Vec3::new(shared_scale_random, rng.next_signed(), rng.next_signed())
    };

    let age_seconds = (local_time - spawned.birth_time).max(0.0);
    let fixed_step = particle.fixed_step();
    let completed_steps = (age_seconds / fixed_step).floor() as usize;
    let multiplier = Vec3::from_array(particle.velocity_multiplier());
    let acceleration = Vec3::from_array(particle.acceleration());
    for _ in 0..completed_steps.min(100_000) {
        position += velocity * fixed_step;
        velocity = velocity * multiplier + acceleration;
    }

    let age_percent =
        (spawned.initial_age_percent + age_seconds * 100.0 / spawned.lifetime).clamp(0.0, 100.0);
    let mut rotation = Vec3::ZERO;
    let mut colour = Vec4::ONE;
    for channel in 0..=9 {
        let default = match channel {
            0..=2 => rotation[channel as usize],
            3..=5 => scale[(channel - 3) as usize],
            6..=9 => colour[(channel - 6) as usize],
            _ => unreachable!(),
        };
        let value = evaluate_curve(particle, channel, age_percent, default);
        match channel {
            0..=2 => rotation[channel as usize] = value,
            3..=5 => scale[(channel - 3) as usize] = value,
            6..=9 => colour[(channel - 6) as usize] = value.clamp(0.0, 1.0),
            _ => {}
        }
    }

    NativeParticleInstance {
        birth_time: spawned.birth_time,
        position,
        scale,
        rotation,
        colour,
        age_percent,
        resource_selector: spawned.resource_selector,
    }
}

fn evaluate_curve(particle: &EXGeoParticle, channel: u32, age_percent: f32, default: f32) -> f32 {
    let key = particle
        .curve_records_for_channel(channel)
        .filter(|record| record.age_percent <= age_percent)
        .max_by(|a, b| a.age_percent.total_cmp(&b.age_percent));
    key.map(|record| record.value + record.slope * (age_percent - record.age_percent))
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn particle_with_words(words: &[(usize, u32)]) -> EXGeoParticle {
        let mut raw_words = vec![0; eurochef_edb::particle::EXGEO_PARTICLE_WORDS];
        raw_words[0] = 0x700;
        for (offset, value) in words {
            raw_words[*offset / 4] = *value;
        }
        EXGeoParticle {
            hashcode: 0x9100_0001,
            index: 0,
            address: 0,
            common: 0x700,
            raw_words,
            render_entity: None,
            particle_type_selector: u32::MAX,
            render_resource_selectors: vec![4, 7, 9],
            tail_array_a: vec![4, 7, 9],
            entity_references: vec![],
            curves: vec![],
        }
    }

    #[test]
    fn native_rng_matches_lcg_step() {
        let mut rng = NativeRng::new(0);
        let value = rng.next_signed();
        assert_eq!(rng.state, 0x3C6E_F35F);
        assert!((-1.0..=1.0).contains(&value));
    }

    #[test]
    fn resource_mode_zero_wraps_and_mode_one_ping_pongs() {
        let mut particle = particle_with_words(&[(0xD8, 0)]);
        let mut rng = NativeRng::new(1);
        assert_eq!(select_resource(&particle, 4, &mut rng), Some(7));
        particle.raw_words[0xD8 / 4] = 1;
        assert_eq!(select_resource(&particle, 3, &mut rng), Some(7));
        assert_eq!(select_resource(&particle, 4, &mut rng), Some(4));
    }

    #[test]
    fn curve_evaluation_uses_native_age_value_slope_record() {
        let mut particle = particle_with_words(&[]);
        particle.curves = vec![EXGeoParticleCurveRecord {
            channel: 3,
            age_percent: 25.0,
            value: 2.0,
            slope: 0.1,
        }];
        assert_eq!(evaluate_curve(&particle, 3, 20.0, 1.0), 1.0);
        assert_eq!(evaluate_curve(&particle, 3, 35.0, 1.0), 3.0);
    }

    #[test]
    fn native_pool_and_lifetime_limit_active_instances() {
        let particle = particle_with_words(&[
            (0x3C, (1.0f32 / 60.0).to_bits()),
            (0x60, 1.0f32.to_bits()),
            (0x64, 1.0f32.to_bits()),
            (0x68, 1.0f32.to_bits()),
            (0x90, 1.0f32.to_bits()),
            (0x94, 1.0f32.to_bits()),
            (0x98, 1.0f32.to_bits()),
            (0xB4, 1.0f32.to_bits()),
            (0xC0, 20.0f32.to_bits()),
            (0xC4, 4),
        ]);
        let instances = simulate_native_particles(&particle, 0.5, 1.0, 4096);
        assert!(instances.len() <= 4);
        assert!(!instances.is_empty());
    }
}
