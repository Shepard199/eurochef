use std::io::Cursor;

use eurochef_edb::{versions::Platform, Hashcode};
use eurochef_shared::entities::{TriStrip, UXVertex};
use glam::{Mat4, Quat, Vec2, Vec3, Vec4};
use glow::HasContext;

use crate::entities::ProcessedEntityMesh;

use super::{
    blend::{set_blending_mode, BlendMode},
    gl_helper, robots_advance_live_lighting, robots_transform_world_light_sample,
    robots_world_light_sample,
    viewer::RenderContext,
    NativeLight, NativeLightZone, RenderStore, RobotsDirectionalSlot,
};

const NAVMESH_TEXTURE_DATA: &[u8] =
    include_bytes!("../../../../assets/icons/triggers/navigation.png");

unsafe fn load_navmesh_texture(gl: &glow::Context) -> glow::Texture {
    let mut cursor = Cursor::new(NAVMESH_TEXTURE_DATA);
    let mut decoder = png::Decoder::new(std::io::BufReader::new(&mut cursor));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().unwrap();
    let mut data = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut data).unwrap();
    gl_helper::load_texture(
        gl,
        info.width as i32,
        info.height as i32,
        &data[..info.buffer_size()],
        glow::RGBA,
        0,
    )
}

const ROBOTS_NATIVE_LIGHT_RANGE: u16 = 0x1;
#[cfg(test)]
const ROBOTS_NATIVE_LIGHT_POSITION_NORMAL: u16 = 0x2;
const ROBOTS_NATIVE_LIGHT_BEAM_CONE: u16 = 0x4;
#[cfg(test)]
const ROBOTS_NATIVE_LIGHT_BEAM_NORMAL: u16 = 0x8;
const ROBOTS_NATIVE_LIGHT_FACTOR_EPSILON: f32 = 0.003_882_353;

fn native_light_has(light_type: u16, feature: u16) -> bool {
    light_type & feature != 0
}

fn native_light_range_factor(distance: f32, radius: f32, full_effect_fraction: f32) -> f32 {
    if !distance.is_finite()
        || !radius.is_finite()
        || !full_effect_fraction.is_finite()
        || radius <= f32::EPSILON
    {
        return 0.0;
    }

    let normalized_distance = distance / radius;
    if normalized_distance >= 1.0 {
        return 0.0;
    }
    if normalized_distance <= full_effect_fraction {
        return 1.0;
    }

    let fade_range = 1.0 - full_effect_fraction;
    if fade_range <= f32::EPSILON {
        return 1.0;
    }
    ((1.0 - normalized_distance) / fade_range).max(0.0)
}

fn native_light_cone_factor(direction: Vec3, light_to_point: Vec3, beam_angle_degrees: f32) -> f32 {
    let distance = light_to_point.length();
    if distance <= f32::EPSILON || !beam_angle_degrees.is_finite() {
        return 0.0;
    }
    let angle_fraction = beam_angle_degrees / 180.0;
    if angle_fraction <= f32::EPSILON {
        return 0.0;
    }

    let alignment = direction.dot(light_to_point / distance);
    let factor = 1.0 + (alignment - 1.0) / angle_fraction;
    if factor >= ROBOTS_NATIVE_LIGHT_FACTOR_EPSILON {
        factor.min(1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
fn native_light_position_normal_factor(to_light: Vec3, normal: Vec3) -> f32 {
    let distance = to_light.length();
    if distance <= f32::EPSILON {
        return 0.0;
    }
    let factor = normal.dot(to_light / distance);
    if factor >= ROBOTS_NATIVE_LIGHT_FACTOR_EPSILON {
        factor
    } else {
        0.0
    }
}

#[cfg(test)]
fn native_light_beam_normal_factor(direction: Vec3, normal: Vec3) -> f32 {
    let factor = normal.dot(-direction);
    if factor >= ROBOTS_NATIVE_LIGHT_FACTOR_EPSILON {
        factor
    } else {
        0.0
    }
}

#[cfg(test)]
fn native_light_feature_factor(
    light: &NativeLight,
    point: Vec3,
    normal: Vec3,
    use_position_normal: bool,
) -> f32 {
    let mut factor = 1.0;
    let point_from_light = point - light.position;

    if native_light_has(light.light_type, ROBOTS_NATIVE_LIGHT_RANGE) {
        factor *= native_light_range_factor(
            point_from_light.length(),
            light.radius,
            light.effect_fraction,
        );
    }
    if native_light_has(light.light_type, ROBOTS_NATIVE_LIGHT_BEAM_CONE) {
        factor *=
            native_light_cone_factor(light.direction, point_from_light, light.beam_angle_degrees);
    }
    if native_light_has(light.light_type, ROBOTS_NATIVE_LIGHT_BEAM_NORMAL) {
        factor *= native_light_beam_normal_factor(light.direction, normal);
    }
    if use_position_normal
        && native_light_has(light.light_type, ROBOTS_NATIVE_LIGHT_POSITION_NORMAL)
    {
        factor *= native_light_position_normal_factor(-point_from_light, normal);
    }
    factor
}

fn native_light_position_influence(light: &NativeLight, point: Vec3) -> f32 {
    let mut factor = 1.0;
    let point_from_light = point - light.position;
    if native_light_has(light.light_type, ROBOTS_NATIVE_LIGHT_RANGE) {
        factor *= native_light_range_factor(
            point_from_light.length(),
            light.radius,
            light.effect_fraction,
        );
    }
    if native_light_has(light.light_type, ROBOTS_NATIVE_LIGHT_BEAM_CONE) {
        factor *=
            native_light_cone_factor(light.direction, point_from_light, light.beam_angle_degrees);
    }
    factor
}

fn containing_native_light_zone(zones: &[NativeLightZone], object_position: Vec3) -> Option<usize> {
    zones
        .iter()
        .enumerate()
        .filter(|(_, zone)| zone.contains(object_position))
        .min_by(|(_, a), (_, b)| a.volume().total_cmp(&b.volume()))
        .map(|(index, _)| index)
}

fn select_native_lights<'a>(
    lights: &'a [NativeLight],
    zones: &[NativeLightZone],
    zone_override: Option<usize>,
    object_position: Vec3,
) -> Vec<&'a NativeLight> {
    // The map-light query caller at 0x00402070 passes capacity 3 to
    // 0x00554B2D. Qualifying lights are retained in serialized zone order.
    const MAX_MAP_LIGHTS: usize = 3;

    let Some(zone_index) = zone_override
        .filter(|index| *index < zones.len())
        .or_else(|| containing_native_light_zone(zones, object_position))
    else {
        return Vec::new();
    };
    let Some(zone) = zones.get(zone_index) else {
        return Vec::new();
    };

    let mut selected = Vec::with_capacity(MAX_MAP_LIGHTS);
    for index in zone.light_indices.iter().copied() {
        let Some(light) = lights.get(index) else {
            continue;
        };
        // 0x00554B2D tests byte [EXGeoLight+0x10] bit 0 before evaluating
        // the type mask. Other serialized flag bits remain preserved/unknown.
        if light.flags & 0x1 == 0 || native_light_position_influence(light, object_position) <= 0.0
        {
            continue;
        }
        selected.push(light);
        if selected.len() == MAX_MAP_LIGHTS {
            break;
        }
    }
    selected
}

#[derive(Clone)]
struct EntityMeshGpu {
    vertex_count: usize,
    vertex_array: glow::VertexArray,
    vertex_buffer: glow::Buffer,
    index_buffer: glow::Buffer,
    strips: Vec<TriStrip>,
}

#[derive(Clone)]
pub struct EntityRenderer {
    mesh: Option<EntityMeshGpu>,
    platform: Platform,
    flags: u32,
    navmesh_texture: Option<glow::Texture>,
    pub file_hashcode: Hashcode,
    pub vertex_lighting: bool,
    pub opaque_effect_preview: bool,
    pub navmesh_visible: bool,
    pub navmesh_texture_scale: f32,
    /// Exact EXGeoMapZone index for map-zone geometry. Dynamic objects leave this unset
    /// and resolve the containing zone from their world position.
    pub native_light_zone: Option<usize>,
    /// MapZone geometry is rendered at world origin, so its light query must sample the
    /// serialized zone bounds rather than the render transform position.
    pub native_light_sample_position: Option<Vec3>,
}

impl EntityRenderer {
    pub fn new(file_hashcode: Hashcode, platform: Platform) -> Self {
        Self {
            mesh: None,
            platform,
            flags: 0,
            navmesh_texture: None,
            vertex_lighting: true,
            opaque_effect_preview: false,
            navmesh_visible: true,
            navmesh_texture_scale: 1.0 / 16.0,
            native_light_zone: None,
            native_light_sample_position: None,
            file_hashcode,
        }
    }

    /// Returns the center of the model (average of all points)
    pub unsafe fn load_mesh(&mut self, gl: &glow::Context, mesh: &ProcessedEntityMesh) -> Vec3 {
        let ProcessedEntityMesh {
            vertex_data,
            indices,
            strips,
            flags,
            is_navmesh,
            ..
        } = mesh;

        let bounding_box = mesh.bounding_box();
        let center = (bounding_box.0 + bounding_box.1) / 2.0;

        let vertex_array = gl.create_vertex_array().unwrap();
        gl.bind_vertex_array(Some(vertex_array));
        let vertex_buffer = gl.create_buffer().unwrap();
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vertex_buffer));
        gl.buffer_data_u8_slice(
            glow::ARRAY_BUFFER,
            bytemuck::cast_slice(vertex_data),
            glow::DYNAMIC_DRAW,
        );
        let index_buffer = gl.create_buffer().unwrap();
        gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(index_buffer));
        gl.buffer_data_u8_slice(
            glow::ELEMENT_ARRAY_BUFFER,
            bytemuck::cast_slice(indices),
            glow::STATIC_DRAW,
        );

        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(
            0,
            3,
            glow::FLOAT,
            false,
            std::mem::size_of::<UXVertex>() as i32,
            0,
        );

        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(
            1,
            3,
            glow::FLOAT,
            false,
            std::mem::size_of::<UXVertex>() as i32,
            3 * std::mem::size_of::<f32>() as i32,
        );

        gl.enable_vertex_attrib_array(2);
        gl.vertex_attrib_pointer_f32(
            2,
            2,
            glow::FLOAT,
            false,
            std::mem::size_of::<UXVertex>() as i32,
            6 * std::mem::size_of::<f32>() as i32,
        );

        gl.enable_vertex_attrib_array(3);
        gl.vertex_attrib_pointer_f32(
            3,
            4,
            glow::FLOAT,
            false,
            std::mem::size_of::<UXVertex>() as i32,
            8 * std::mem::size_of::<f32>() as i32,
        );

        gl.bind_vertex_array(None);

        let mut strips_sorted = strips.to_vec();
        strips_sorted.sort_by(|a, b| a.transparency.cmp(&b.transparency));

        self.mesh = Some(EntityMeshGpu {
            vertex_count: vertex_data.len(),
            vertex_array,
            vertex_buffer,
            index_buffer,
            strips: strips_sorted,
        });
        self.flags = *flags;
        self.navmesh_texture = (*is_navmesh).then(|| load_navmesh_texture(gl));

        center
    }

    pub unsafe fn update_vertices(&self, gl: &glow::Context, vertices: &[UXVertex]) -> bool {
        let Some(mesh) = self.mesh.as_ref() else {
            return false;
        };
        if vertices.len() != mesh.vertex_count {
            return false;
        }
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(mesh.vertex_buffer));
        gl.buffer_sub_data_u8_slice(glow::ARRAY_BUFFER, 0, bytemuck::cast_slice(vertices));
        true
    }

    unsafe fn init_draw(
        &self,
        gl: &glow::Context,
        shader: glow::Program,
        position: Vec3,
        rotation: Quat,
        scale: Vec3,
        tint: Vec4,
        context: &RenderContext,
    ) {
        gl.use_program(Some(shader));
        gl.uniform_matrix_4_f32_slice(
            gl.get_uniform_location(shader, "u_view").as_ref(),
            false,
            &context.uniforms.view.to_cols_array(),
        );

        let mut rotation = rotation;

        if (self.flags & 0x4) != 0 {
            rotation = context.uniforms.camera_rotation;
        }

        let model =
            Mat4::from_translation(position) * Mat4::from_quat(rotation) * Mat4::from_scale(scale);
        gl.uniform_matrix_4_f32_slice(
            gl.get_uniform_location(shader, "u_model").as_ref(),
            false,
            &model.to_cols_array(),
        );

        gl.uniform_matrix_4_f32_slice(
            gl.get_uniform_location(shader, "u_normal").as_ref(),
            false,
            &(context.uniforms.view * model)
                .inverse()
                .transpose()
                .to_cols_array(),
        );
        gl.uniform_matrix_4_f32_slice(
            gl.get_uniform_location(shader, "u_world_normal").as_ref(),
            false,
            &model.inverse().transpose().to_cols_array(),
        );

        self.upload_global_lighting(gl, shader, position, context);
        self.upload_native_lights(gl, shader, position, context);
        gl.uniform_1_i32(gl.get_uniform_location(shader, "u_texture").as_ref(), 0);
        gl.uniform_4_f32(
            gl.get_uniform_location(shader, "u_tint").as_ref(),
            tint.x,
            tint.y,
            tint.z,
            tint.w,
        );
    }

    unsafe fn upload_global_lighting(
        &self,
        gl: &glow::Context,
        shader: glow::Program,
        object_position: Vec3,
        context: &RenderContext,
    ) {
        let global = context
            .uniforms
            .global_lighting_enabled
            .then_some(context.uniforms.global_lighting)
            .flatten();
        gl.uniform_1_i32(
            gl.get_uniform_location(shader, "u_globalLightingEnabled")
                .as_ref(),
            i32::from(global.is_some()),
        );
        let lightmap = global.and(context.uniforms.global_lightmap.as_deref());
        gl.uniform_1_i32(
            gl.get_uniform_location(shader, "u_globalLightmapEnabled")
                .as_ref(),
            i32::from(lightmap.is_some()),
        );
        if let Some(lightmap) = lightmap {
            gl.active_texture(glow::TEXTURE1);
            gl.bind_texture(glow::TEXTURE_2D, Some(lightmap.texture));
            gl.uniform_1_i32(
                gl.get_uniform_location(shader, "u_globalLightmap").as_ref(),
                1,
            );
            gl.uniform_2_f32(
                gl.get_uniform_location(shader, "u_globalLightmapMin")
                    .as_ref(),
                lightmap.min.x,
                lightmap.min.y,
            );
            gl.uniform_2_f32(
                gl.get_uniform_location(shader, "u_globalLightmapSpan")
                    .as_ref(),
                lightmap.span.x,
                lightmap.span.y,
            );
            let coefficients = global
                .expect("lightmap requires global lighting")
                .level_coefficients;
            gl.uniform_4_f32(
                gl.get_uniform_location(shader, "u_globalLightmapCoefficients")
                    .as_ref(),
                coefficients[2],
                coefficients[1],
                coefficients[4],
                coefficients[5],
            );
            gl.active_texture(glow::TEXTURE0);
        }

        let mut directions = [0.0f32; 9];
        let mut colours = [0.0f32; 9];
        let mut ambient = Vec3::ZERO;

        if let Some(global) = global {
            let sample_position =
                self.native_light_sample_position.unwrap_or(object_position) + Vec3::Y;
            let tick = (context.uniforms.time.max(0.0) * 60.0).floor() as u64;
            let state_key = if context.lighting_key != 0 {
                context.lighting_key
            } else {
                let mut key = self.file_hashcode as u64;
                for bits in sample_position.to_array().map(f32::to_bits) {
                    key = key.rotate_left(13) ^ bits as u64;
                }
                key
            };

            let cached = {
                let mut states = context.uniforms.live_lighting_states.lock().unwrap();
                states.get_mut(&state_key).and_then(|state| {
                    let same_position = state.initialized
                        && state.sample_position.distance_squared(sample_position) <= 1.0e-8;
                    if same_position {
                        robots_advance_live_lighting(state, tick);
                        Some(*state)
                    } else {
                        None
                    }
                })
            };

            let state = if let Some(cached) = cached {
                cached
            } else {
                let zone_index = self
                    .native_light_zone
                    .filter(|index| *index < context.uniforms.native_light_zones.len())
                    .or_else(|| {
                        containing_native_light_zone(
                            &context.uniforms.native_light_zones,
                            sample_position,
                        )
                    });
                let zone_ambience = zone_index
                    .and_then(|index| context.uniforms.native_light_zones.get(index))
                    .map(|zone| zone.ambience);
                let raw_world_sample = robots_world_light_sample(
                    &context.uniforms.native_lighting_triangles,
                    &context.uniforms.native_light_zones,
                    zone_index,
                    sample_position,
                );
                let target_ambient = robots_transform_world_light_sample(
                    raw_world_sample,
                    global.level_coefficients,
                    zone_ambience,
                );

                let mut target_slots = [RobotsDirectionalSlot::default(); 3];
                if context.uniforms.native_lights_enabled {
                    for (slot, light) in select_native_lights(
                        &context.uniforms.native_lights,
                        &context.uniforms.native_light_zones,
                        zone_index,
                        sample_position,
                    )
                    .into_iter()
                    .enumerate()
                    {
                        let influence = native_light_position_influence(light, sample_position);
                        target_slots[slot] = RobotsDirectionalSlot {
                            direction: (sample_position - light.position).normalize_or_zero(),
                            colour: light.colour
                                * influence
                                * global.level_coefficients[3]
                                * context.uniforms.native_light_strength.max(0.0),
                        };
                    }
                }
                if target_slots[2].colour.length_squared() <= f32::EPSILON {
                    target_slots[2] = RobotsDirectionalSlot {
                        direction: global.direction,
                        colour: global.colour,
                    };
                }

                let mut states = context.uniforms.live_lighting_states.lock().unwrap();
                let state = states.entry(state_key).or_default();
                if !state.initialized {
                    state.slots = target_slots;
                    state.ambient = target_ambient;
                    state.last_tick = tick;
                    state.initialized = true;
                }
                state.target_slots = target_slots;
                state.target_ambient = target_ambient;
                state.sample_position = sample_position;
                robots_advance_live_lighting(state, tick);
                *state
            };

            for (index, slot) in state.slots.iter().enumerate() {
                directions[index * 3..index * 3 + 3].copy_from_slice(&slot.direction.to_array());
                colours[index * 3..index * 3 + 3].copy_from_slice(&slot.colour.to_array());
            }
            ambient = state.ambient;
        }

        gl.uniform_3_f32_slice(
            gl.get_uniform_location(shader, "u_globalLightDirection[0]")
                .as_ref(),
            &directions,
        );
        gl.uniform_3_f32_slice(
            gl.get_uniform_location(shader, "u_globalLightColour[0]")
                .as_ref(),
            &colours,
        );
        gl.uniform_3_f32(
            gl.get_uniform_location(shader, "u_globalAmbient").as_ref(),
            ambient.x,
            ambient.y,
            ambient.z,
        );
    }

    unsafe fn upload_native_lights(
        &self,
        gl: &glow::Context,
        shader: glow::Program,
        object_position: Vec3,
        context: &RenderContext,
    ) {
        const MAX_NATIVE_LIGHTS: usize = 16;

        let light_sample_position = self.native_light_sample_position.unwrap_or(object_position);
        let lights = if context.uniforms.native_lights_enabled
            && !context.uniforms.global_lighting_enabled
        {
            select_native_lights(
                &context.uniforms.native_lights,
                &context.uniforms.native_light_zones,
                self.native_light_zone,
                light_sample_position,
            )
        } else {
            Vec::new()
        };

        let mut positions = [0.0f32; MAX_NATIVE_LIGHTS * 4];
        let mut directions = [0.0f32; MAX_NATIVE_LIGHTS * 4];
        let mut colours = [0.0f32; MAX_NATIVE_LIGHTS * 4];
        let mut parameters = [0.0f32; MAX_NATIVE_LIGHTS * 2];

        for (index, light) in lights.iter().enumerate() {
            let p = index * 4;
            positions[p..p + 3].copy_from_slice(&light.position.to_array());
            positions[p + 3] = light.radius.max(0.0);

            directions[p..p + 3].copy_from_slice(&light.direction.to_array());
            directions[p + 3] = light.light_type as f32;

            colours[p..p + 3].copy_from_slice(&light.colour.to_array());
            colours[p + 3] = light.effect_fraction;

            let parameter = index * 2;
            parameters[parameter] = light.beam_angle_degrees;
        }

        gl.uniform_1_i32(
            gl.get_uniform_location(shader, "u_nativeLightCount")
                .as_ref(),
            lights.len() as i32,
        );
        gl.uniform_1_f32(
            gl.get_uniform_location(shader, "u_nativeLightStrength")
                .as_ref(),
            context.uniforms.native_light_strength.max(0.0),
        );
        gl.uniform_4_f32_slice(
            gl.get_uniform_location(shader, "u_nativeLightPositionRadius[0]")
                .as_ref(),
            &positions,
        );
        gl.uniform_4_f32_slice(
            gl.get_uniform_location(shader, "u_nativeLightDirectionType[0]")
                .as_ref(),
            &directions,
        );
        gl.uniform_4_f32_slice(
            gl.get_uniform_location(shader, "u_nativeLightColorEffect[0]")
                .as_ref(),
            &colours,
        );
        gl.uniform_2_f32_slice(
            gl.get_uniform_location(shader, "u_nativeLightParameters[0]")
                .as_ref(),
            &parameters,
        );
    }

    pub fn get_shader(&self, context: &RenderContext) -> glow::Program {
        if self.vertex_lighting {
            context.shaders.entity_simple
        } else {
            context.shaders.entity_simple_unlit
        }
    }

    pub unsafe fn draw_both(
        &self,
        gl: &glow::Context,
        context: &RenderContext,
        position: Vec3,
        rotation: Quat,
        scale: Vec3,
        time: f64,
        render_store: &RenderStore,
    ) {
        self.draw_opaque(gl, context, position, rotation, scale, time, render_store);
        gl.depth_mask(false);
        self.draw_transparent(gl, context, position, rotation, scale, time, render_store);
    }

    pub unsafe fn draw_particle(
        &self,
        gl: &glow::Context,
        context: &RenderContext,
        position: Vec3,
        rotation: Quat,
        scale: Vec3,
        tint: Vec4,
        resource_selector: Option<u32>,
        time: f64,
        render_store: &RenderStore,
    ) {
        let Some(mesh) = self.mesh.as_ref() else {
            return;
        };
        gl.bind_vertex_array(Some(mesh.vertex_array));
        gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(mesh.index_buffer));
        gl.depth_mask(false);

        let selected_strip = resource_selector
            .map(|selector| (selector & 0x3f) as usize)
            .filter(|index| *index < mesh.strips.len());
        let shader = self.get_shader(context);
        for (index, strip) in mesh.strips.iter().enumerate() {
            if selected_strip.is_some_and(|selected| selected != index) {
                continue;
            }
            self.draw_strip(
                gl,
                shader,
                strip,
                time,
                render_store,
                position,
                rotation,
                scale,
                tint,
                context,
            );
        }
    }

    pub unsafe fn draw_opaque(
        &self,
        gl: &glow::Context,
        context: &RenderContext,
        position: Vec3,
        rotation: Quat,
        scale: Vec3,
        time: f64,
        render_store: &RenderStore,
    ) {
        puffin::profile_function!();
        if let Some(mesh) = self.mesh.as_ref() {
            gl.bind_vertex_array(Some(mesh.vertex_array));
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(mesh.index_buffer));

            for t in mesh
                .strips
                .iter()
                .filter(|t| t.transparency == 0 && (t.flags & 0x8) == 0)
            {
                self.draw_strip(
                    gl,
                    self.get_shader(context),
                    t,
                    time,
                    render_store,
                    position,
                    rotation,
                    scale,
                    Vec4::ONE,
                    context,
                );
            }
        }
    }

    pub unsafe fn draw_transparent(
        &self,
        gl: &glow::Context,
        context: &RenderContext,
        position: Vec3,
        rotation: Quat,
        scale: Vec3,
        time: f64,
        render_store: &RenderStore,
    ) {
        puffin::profile_function!();
        if let Some(mesh) = self.mesh.as_ref() {
            gl.bind_vertex_array(Some(mesh.vertex_array));
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(mesh.index_buffer));

            let shader = if self.vertex_lighting {
                context.shaders.entity_simple
            } else {
                context.shaders.entity_simple_unlit
            };
            for t in mesh
                .strips
                .iter()
                .filter(|t| t.transparency != 0 || (t.flags & 0x8) != 0)
            {
                self.draw_strip(
                    gl,
                    shader,
                    t,
                    time,
                    render_store,
                    position,
                    rotation,
                    scale,
                    Vec4::ONE,
                    context,
                );
            }
        }
    }

    unsafe fn draw_strip(
        &self,
        gl: &glow::Context,
        shader: glow::Program,
        t: &TriStrip,
        time: f64,
        render_store: &RenderStore,
        position: Vec3,
        rotation: Quat,
        scale: Vec3,
        tint: Vec4,
        context: &RenderContext,
    ) {
        puffin::profile_function!();

        // For stripflags (EX):
        // 0x1 - transparent / vertex blended?
        // 0x2 - ?
        // 0x4 - additive
        // 0x8 - ? kinda like 0x1 but not really
        // 0x10 - invisible
        // 0x20 - ?
        // 0x40 - double sided (disable culling)
        // 0x80 - seems to be used for anything that's not transparent OR using vertex color transparency stuck to the world
        // 0x100 - ? (used by godrays in gforce)
        // 0x200 - mostly additive surfaces, but not all
        // 0x400 - used by everything that isn't a floor
        // 0x800 - unused?
        // 0x1000 - unused?
        // 0x2000 - unused?
        // 0x4000 - unused?
        // 0x8000 - unused?

        if t.is_navmesh && !self.navmesh_visible {
            return;
        }

        // Hide what is hidden
        if (t.flags & 0x10) != 0 {
            return;
        }

        let mut shader = shader;

        let mut transparency = match t.transparency & 0xff {
            2 => BlendMode::ReverseSubtract,
            1 => BlendMode::Additive,
            0 | _ => BlendMode::None,
        };

        if self.opaque_effect_preview
            && matches!(
                transparency,
                BlendMode::Additive | BlendMode::ReverseSubtract
            )
        {
            transparency = BlendMode::None;
        }

        if ((t.flags & 0x8) != 0 || (t.flags & 0x1) != 0) && transparency == BlendMode::None {
            transparency = BlendMode::Blend;
        }

        if (t.flags & 0x40) != 0 {
            gl.disable(glow::CULL_FACE);
        } else {
            // TODO(cohae): PS2/GX Strips aren't built with the correct winding order
            match self.platform {
                Platform::GameCube | Platform::Wii | Platform::Ps2 => {
                    gl.disable(glow::CULL_FACE);
                }
                _ => {
                    gl.enable(glow::CULL_FACE);
                }
            }
        }

        let mut scroll = Vec2::ZERO;

        gl.active_texture(glow::TEXTURE0);
        if t.is_navmesh {
            gl.bind_texture(glow::TEXTURE_2D, self.navmesh_texture);
        } else if t.texture_index == u32::MAX {
            gl.bind_texture(glow::TEXTURE_2D, None);
        } else if let Some((_, tex)) =
            render_store.get_texture_by_index(self.file_hashcode, t.texture_index as usize)
        {
            let mut tex = tex;
            if let Some((external_file, external_texture)) = tex.external_reference {
                if let Some(resolved) = render_store.get_texture(external_file, external_texture) {
                    tex = resolved;
                }
            }

            let frametime_scale = tex.frame_count as f32 / tex.frames.len() as f32;
            let frame_time = (1. / tex.framerate as f32) * frametime_scale;

            scroll = tex.scroll * time as f32;

            if !tex.frames.is_empty() {
                gl.bind_texture(
                    glow::TEXTURE_2D,
                    Some(tex.frames[(time as f32 / frame_time) as usize % tex.frames.len()]),
                );
            } else {
                gl.bind_texture(glow::TEXTURE_2D, None);
            }
            if (((tex.flags >> 0x18) >> 5) & 0b11) != 0 && (t.flags & 0x8) == 0 {
                transparency = BlendMode::Cutout;
            }

            // Environment texture
            if (tex.flags & 0x30000) != 0 {
                shader = context.shaders.entity_simple_matcap;
            }
        } else {
            gl.bind_texture(glow::TEXTURE_2D, None);
        }

        self.init_draw(gl, shader, position, rotation, scale, tint, context);

        gl.uniform_1_f32(
            gl.get_uniform_location(shader, "u_uv_scale").as_ref(),
            if t.is_navmesh {
                self.navmesh_texture_scale
            } else {
                1.0
            },
        );
        gl.uniform_2_f32(
            gl.get_uniform_location(shader, "u_scroll").as_ref(),
            scroll.x,
            scroll.y,
        );

        set_blending_mode(gl, transparency);

        gl.uniform_1_f32(
            gl.get_uniform_location(shader, "u_cutoutThreshold")
                .as_ref(),
            if transparency == BlendMode::Cutout {
                0.5
            } else {
                0.0
            },
        );

        gl.draw_elements(
            glow::TRIANGLE_STRIP,
            (t.tri_count + 2) as i32,
            glow::UNSIGNED_INT,
            t.start_index as i32 * std::mem::size_of::<u32>() as i32,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        containing_native_light_zone, native_light_feature_factor, native_light_range_factor,
        select_native_lights,
    };
    use crate::render::{NativeLight, NativeLightZone};
    use glam::Vec3;

    fn light(light_type: u16, x: f32) -> NativeLight {
        NativeLight {
            position: Vec3::new(x, 0.0, 0.0),
            direction: -Vec3::X,
            colour: Vec3::ONE,
            flags: 1,
            radius: 100.0,
            effect_fraction: 0.1,
            light_type,
            beam_angle_degrees: 60.0,
        }
    }

    #[test]
    fn smallest_containing_map_zone_selects_its_exact_light_indices() {
        let lights = vec![light(3, 1.0), light(7, 2.0), light(11, 3.0)];
        let zones = vec![
            NativeLightZone {
                bounds_min: Vec3::splat(-10.0),
                bounds_max: Vec3::splat(10.0),
                light_indices: vec![0, 2],
                ambience: 0.0,
            },
            NativeLightZone {
                bounds_min: Vec3::splat(-1.0),
                bounds_max: Vec3::splat(1.0),
                light_indices: vec![1],
                ambience: 0.0,
            },
        ];
        assert_eq!(containing_native_light_zone(&zones, Vec3::ZERO), Some(1));
        let selected = select_native_lights(&lights, &zones, None, Vec3::ZERO);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].light_type, 7);
    }

    #[test]
    fn native_map_light_query_keeps_three_active_lights_in_zone_order() {
        let mut lights = (0..6)
            .map(|index| light(3, index as f32))
            .collect::<Vec<_>>();
        lights[1].flags = 0;
        lights[2].position = Vec3::new(200.0, 0.0, 0.0);
        let zones = [NativeLightZone {
            bounds_min: Vec3::splat(-10.0),
            bounds_max: Vec3::splat(10.0),
            light_indices: vec![5, 1, 2, 4, 3, 0],
            ambience: 0.0,
        }];
        let selected = select_native_lights(&lights, &zones, None, Vec3::ZERO);
        assert_eq!(selected.len(), 3);
        assert_eq!(selected[0].position.x, 5.0);
        assert_eq!(selected[1].position.x, 4.0);
        assert_eq!(selected[2].position.x, 3.0);
        assert!(select_native_lights(&lights, &[], None, Vec3::ZERO).is_empty());
    }

    #[test]
    fn native_light_range_uses_inner_full_effect_then_linear_fade() {
        assert_eq!(native_light_range_factor(0.2, 1.0, 0.25), 1.0);
        assert!((native_light_range_factor(0.5, 1.0, 0.25) - 2.0 / 3.0).abs() < 0.0001);
        assert_eq!(native_light_range_factor(1.0, 1.0, 0.25), 0.0);
    }

    #[test]
    fn shipped_native_light_types_multiply_the_exact_feature_bits() {
        let point = Vec3::new(0.5, 0.0, 0.0);
        let normal_perpendicular = Vec3::Y;
        let normal_facing = -Vec3::X;

        let mut range = light(1, 0.0);
        range.position = Vec3::ZERO;
        range.direction = Vec3::X;
        range.radius = 1.0;
        range.effect_fraction = 0.25;
        range.beam_angle_degrees = 90.0;
        let radial = native_light_feature_factor(&range, point, normal_perpendicular, true);
        assert!((radial - 2.0 / 3.0).abs() < 0.0001);

        range.light_type = 3;
        assert_eq!(
            native_light_feature_factor(&range, point, normal_perpendicular, true),
            0.0
        );
        assert!(
            (native_light_feature_factor(&range, point, normal_facing, true) - radial).abs()
                < 0.0001
        );

        range.light_type = 5;
        assert!(
            (native_light_feature_factor(&range, point, normal_perpendicular, true) - radial).abs()
                < 0.0001
        );

        range.light_type = 7;
        assert_eq!(
            native_light_feature_factor(&range, point, normal_perpendicular, true),
            0.0
        );

        range.light_type = 11;
        assert_eq!(
            native_light_feature_factor(&range, point, normal_perpendicular, true),
            0.0
        );
        assert!(
            (native_light_feature_factor(&range, point, normal_facing, true) - radial).abs()
                < 0.0001
        );
    }
}
