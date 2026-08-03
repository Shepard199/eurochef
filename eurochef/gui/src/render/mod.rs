use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use eurochef_edb::{
    particle::EXGeoParticle, Hashcode, HashcodeUtils, HC_BASE_ENTITY, HC_BASE_PARTICLE,
    HC_BASE_SCRIPT, HC_BASE_TEXTURE,
};

// ROBOTS_PATCH_0024_ANIMSKIN_VISUAL_RENDERING
const HC_BASE_ANIMATION: Hashcode = 0x03000000;
const HC_BASE_ANIMSKIN: Hashcode = 0x0D000000;
use eurochef_shared::script::{UXGeoScript, UXGeoScriptCommandData};
use glam::{Mat4, Quat};
use glow::HasContext;
use nohash_hasher::IntMap;

use crate::{animations::AnimationRuntime, entity_frame::RenderableTexture};

use self::{camera::Camera3D, entity::EntityRenderer};

pub mod billboard;
pub mod blend;
pub mod camera;
pub mod entity;
pub mod gl_helper;
pub mod global_lightmap;
pub mod grid;
pub mod particle;
pub mod pickbuffer;
pub mod script;
pub mod shaders;
pub mod trigger;
pub mod tweeny;
pub mod viewer;

#[derive(Debug, Clone, Copy)]
pub struct NativeLight {
    pub position: glam::Vec3,
    pub direction: glam::Vec3,
    pub colour: glam::Vec3,
    pub flags: u32,
    pub radius: f32,
    pub effect_fraction: f32,
    pub light_type: u16,
    pub beam_angle_degrees: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct NativeLightingTriangle {
    pub positions: [glam::Vec3; 3],
    pub colours: [glam::Vec4; 3],
    pub zone_index: usize,
}

#[derive(Debug, Clone)]
pub struct NativeLightZone {
    pub bounds_min: glam::Vec3,
    pub bounds_max: glam::Vec3,
    pub light_indices: Vec<usize>,
    pub ambience: f32,
}

impl NativeLightZone {
    pub fn contains(&self, point: glam::Vec3) -> bool {
        point.cmpge(self.bounds_min).all() && point.cmple(self.bounds_max).all()
    }

    pub fn volume(&self) -> f32 {
        let size = (self.bounds_max - self.bounds_min).max(glam::Vec3::ZERO);
        size.x * size.y * size.z
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RobotsDirectionalSlot {
    pub direction: glam::Vec3,
    pub colour: glam::Vec3,
}

#[derive(Debug, Clone, Copy)]
pub struct RobotsGlobalLighting {
    pub direction: glam::Vec3,
    pub colour: glam::Vec3,
    pub ambient: glam::Vec3,
    pub level_coefficients: [f32; 6],
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RobotsLiveLightingState {
    pub slots: [RobotsDirectionalSlot; 3],
    pub ambient: glam::Vec3,
    pub target_slots: [RobotsDirectionalSlot; 3],
    pub target_ambient: glam::Vec3,
    pub sample_position: glam::Vec3,
    pub last_tick: u64,
    pub initialized: bool,
}

pub fn robots_advance_live_lighting(state: &mut RobotsLiveLightingState, tick: u64) {
    if !state.initialized || tick <= state.last_tick {
        return;
    }
    let steps = (tick - state.last_tick).min(600) as i32;
    let light_alpha = 1.0 - 0.95_f32.powi(steps);
    let ambient_alpha = 1.0 - 0.9_f32.powi(steps);
    for (slot, target) in state.slots.iter_mut().zip(state.target_slots) {
        slot.direction = slot.direction.lerp(target.direction, light_alpha);
        if slot.direction.length_squared() > f32::EPSILON {
            slot.direction = slot.direction.normalize();
        }
        slot.colour = slot.colour.lerp(target.colour, light_alpha);
    }
    state.ambient = state.ambient.lerp(state.target_ambient, ambient_alpha);
    state.last_tick = tick;
}

pub fn robots_world_light_sample(
    triangles: &[NativeLightingTriangle],
    zones: &[NativeLightZone],
    zone_override: Option<usize>,
    point: glam::Vec3,
) -> glam::Vec3 {
    let preferred_zone = zone_override.or_else(|| {
        zones
            .iter()
            .enumerate()
            .filter(|(_, zone)| zone.contains(point))
            .min_by(|(_, a), (_, b)| a.volume().total_cmp(&b.volume()))
            .map(|(index, _)| index)
    });

    fn barycentric_xz(point: glam::Vec3, tri: &NativeLightingTriangle) -> Option<[f32; 3]> {
        let a = tri.positions[0];
        let b = tri.positions[1];
        let c = tri.positions[2];
        let v0 = glam::Vec2::new(b.x - a.x, b.z - a.z);
        let v1 = glam::Vec2::new(c.x - a.x, c.z - a.z);
        let v2 = glam::Vec2::new(point.x - a.x, point.z - a.z);
        let denominator = v0.x * v1.y - v1.x * v0.y;
        if denominator.abs() <= 1.0e-7 {
            return None;
        }
        let w1 = (v2.x * v1.y - v1.x * v2.y) / denominator;
        let w2 = (v0.x * v2.y - v2.x * v0.y) / denominator;
        let w0 = 1.0 - w1 - w2;
        const EPSILON: f32 = -1.0e-4;
        (w0 >= EPSILON && w1 >= EPSILON && w2 >= EPSILON).then_some([w0, w1, w2])
    }

    let sample = |zone_filter: Option<usize>| {
        triangles
            .iter()
            .filter(|triangle| zone_filter.is_none_or(|zone| triangle.zone_index == zone))
            .filter_map(|triangle| {
                let weights = barycentric_xz(point, triangle)?;
                let y = triangle.positions[0].y * weights[0]
                    + triangle.positions[1].y * weights[1]
                    + triangle.positions[2].y * weights[2];
                let vertical_distance = (y - point.y).abs();
                if vertical_distance > 50.0 {
                    return None;
                }
                let colour = triangle.colours[0].truncate() * weights[0]
                    + triangle.colours[1].truncate() * weights[1]
                    + triangle.colours[2].truncate() * weights[2];
                Some((vertical_distance, colour))
            })
            .min_by(|(a, _), (b, _)| a.total_cmp(b))
            .map(|(_, colour)| colour)
    };

    preferred_zone
        .and_then(|zone| sample(Some(zone)))
        .or_else(|| sample(None))
        .unwrap_or(glam::Vec3::splat(0.5))
}

pub fn robots_transform_world_light_sample(
    sample: glam::Vec3,
    coefficients: [f32; 6],
    zone_ambience: Option<f32>,
) -> glam::Vec3 {
    let mut transformed = sample * coefficients[4];
    let magnitude = transformed.length();
    if magnitude > coefficients[1] && magnitude > f32::EPSILON {
        transformed *= coefficients[1] / magnitude;
    }
    let energy = transformed.length();
    let achromatic = glam::Vec3::splat(energy * 0.577_350_26);
    transformed = transformed * coefficients[5] + achromatic * (1.0 - coefficients[5]);

    // EXGeoIdentifier.ambience is carried with the exact containing MapZone, but no
    // instruction-proven arithmetic consumer has been recovered yet. Preserve the value
    // for diagnostics instead of silently inventing a multiplier.
    let _ = zone_ambience;
    transformed.max(glam::Vec3::splat(coefficients[2]))
}

pub fn robots_level_lighting_coefficients(file: Hashcode) -> Option<[f32; 6]> {
    Some(match file {
        0x01000012 => [0.30, 1.75, 0.40, 1.50, 1.80, 0.60], // Village
        0x0100001d => [0.27, 1.60, 0.51, 1.00, 1.30, 0.50], // Robot City
        0x01000071 => [0.25, 1.30, 0.46, 1.00, 1.00, 0.60], // Hub 1
        0x01000015 => [0.20, 1.60, 0.30, 1.00, 1.00, 0.50], // Courtyard
        0x01000050 => [0.35, 1.20, 0.30, 1.50, 1.15, 0.40], // Outmodes
        0x01000072 => [0.25, 1.30, 0.46, 1.00, 1.00, 0.60], // Hub 2
        0x01000053 => [0.27, 1.60, 0.40, 1.00, 1.35, 0.50], // Sewer
        0x01000073 => [0.25, 1.30, 0.46, 1.00, 1.00, 0.60], // Hub 3
        0x0100006e => [0.27, 1.40, 0.40, 1.29, 1.30, 0.40], // Mansion
        0x0100006f => [0.25, 1.80, 0.55, 1.20, 1.30, 0.40], // Chase
        0x0100001a => [0.20, 1.10, 0.25, 1.00, 1.00, 0.50], // Chop Shop
        0x010000bb => [0.25, 1.00, 0.25, 1.00, 1.30, 0.50], // Final Boss
        0x01000001 => [0.20, 1.30, 0.40, 1.00, 1.00, 0.50], // Testmap
        0x0100000f => [0.20, 1.30, 0.40, 1.00, 1.00, 0.50], // Enemies Testmap
        0x01000074 => [0.20, 1.30, 0.40, 1.00, 1.00, 0.50], // Ball Test
        _ => return None,
    })
}

pub fn robots_global_lighting(file: Hashcode) -> Option<RobotsGlobalLighting> {
    let level_coefficients = robots_level_lighting_coefficients(file)?;
    Some(RobotsGlobalLighting {
        // Exact third-slot fallback direction written by Robots.exe at 0x00402570.
        direction: glam::Vec3::new(
            f32::from_bits(0x3f64_f92b),
            f32::from_bits(0xbee4_f93c),
            0.0,
        ),
        // 0x00402570 broadcasts coefficient[0] into the fallback directional RGB.
        colour: glam::Vec3::splat(level_coefficients[0]),
        // 0x00402708 floors each ambient channel by coefficient[2].
        ambient: glam::Vec3::splat(level_coefficients[2]),
        level_coefficients,
    })
}

#[derive(Default, Clone)]
pub struct RenderUniforms {
    pub view: Mat4,
    pub camera_rotation: Quat,
    pub time: f32,
    pub global_lighting_enabled: bool,
    pub global_lighting: Option<RobotsGlobalLighting>,
    pub native_lights_enabled: bool,
    pub native_light_strength: f32,
    pub native_lights: Vec<NativeLight>,
    pub native_light_zones: Vec<NativeLightZone>,
    pub native_lighting_triangles: Vec<NativeLightingTriangle>,
    pub global_lightmap: Option<std::sync::Arc<global_lightmap::GpuGlobalLightmap>>,
    pub live_lighting_states: Arc<Mutex<HashMap<u64, RobotsLiveLightingState>>>,
}

impl RenderUniforms {
    pub fn update<C: Camera3D + ?Sized>(
        &mut self,
        orthographic: bool,
        camera: &mut C,
        aspect_ratio: f32,
        time: f32,
    ) {
        let aspect_ratio_vert = (1.0 / aspect_ratio).max(1.0);

        let mut projection = if orthographic {
            glam::camera::rh::proj::opengl::orthographic(
                (-(aspect_ratio * aspect_ratio_vert) * -camera.zoom()) * 2.0,
                ((aspect_ratio * aspect_ratio_vert) * -camera.zoom()) * 2.0,
                (aspect_ratio_vert * -camera.zoom()) * 2.0,
                (-aspect_ratio_vert * -camera.zoom()) * 2.0,
                -2500.0,
                2500.0,
            )
        } else {
            glam::camera::rh::proj::directx::perspective(
                2.0 * aspect_ratio_vert.atan(),
                aspect_ratio,
                0.02,
                2000.0,
            )
        };

        if !orthographic {
            projection.x_axis = -projection.x_axis;
        }

        self.view = projection * camera.calculate_matrix();
        self.camera_rotation = camera.rotation();
        self.time = time;
    }
}

pub unsafe fn start_render(gl: &glow::Context) {
    gl.depth_mask(true);
    gl.clear_depth_f32(1.0);
    gl.clear(glow::DEPTH_BUFFER_BIT);
    gl.cull_face(glow::FRONT);
    gl.enable(glow::DEPTH_TEST);
    gl.depth_func(glow::LEQUAL);
}

pub struct RenderStore {
    files: IntMap<
        Hashcode,
        (
            IntMap<Hashcode, (usize, EntityRenderer)>,
            IntMap<Hashcode, (usize, RenderableTexture)>,
            Vec<UXGeoScript>,
            Vec<Hashcode>, // All loaded hashcodes, used for analysis
            IntMap<Hashcode, (usize, Vec<Hashcode>)>, // AnimSkin -> component entity hashcodes
            IntMap<Hashcode, (usize, EXGeoParticle)>, // Particle resource objects
        ),
    >,
    animation_runtimes: IntMap<Hashcode, Arc<AnimationRuntime>>,
}

impl RenderStore {
    pub fn new() -> Self {
        Self {
            files: Default::default(),
            animation_runtimes: Default::default(),
        }
    }

    pub fn purge(&mut self, purge_memory: bool) {
        self.files.clear();
        self.animation_runtimes.clear();
        if purge_memory {
            self.files.shrink_to_fit();
            self.animation_runtimes.shrink_to_fit();
        }
    }

    pub fn insert_animation_runtime(
        &mut self,
        file: Hashcode,
        runtime: Arc<AnimationRuntime>,
    ) {
        self.animation_runtimes.insert(file, runtime);
    }

    pub fn get_animation_runtime(&self, file: Hashcode) -> Option<Arc<AnimationRuntime>> {
        self.animation_runtimes.get(&file).cloned()
    }

    #[allow(dead_code)]
    pub fn purge_file(&mut self, file: Hashcode) {
        self.files.remove(&file);
    }

    pub fn set_vertex_lighting(&mut self, enabled: bool) {
        for file in self.files.values_mut() {
            for (_, renderer) in file.0.values_mut() {
                renderer.vertex_lighting = enabled;
            }
        }
    }

    pub fn set_navmesh_options(&mut self, visible: bool, texture_scale: f32) {
        for file in self.files.values_mut() {
            for (_, renderer) in file.0.values_mut() {
                renderer.navmesh_visible = visible;
                renderer.navmesh_texture_scale = texture_scale;
            }
        }
    }

    pub fn get_entity(&self, file: Hashcode, entity_hashcode: Hashcode) -> Option<&EntityRenderer> {
        self.files.get(&file).and_then(|v| {
            if entity_hashcode.is_local() {
                v.0.iter()
                    .find(|(_, (v, _))| *v == entity_hashcode.index() as usize)
                    .map(|(_, (_, v))| v)
            } else {
                v.0.get(&entity_hashcode).map(|(_, v)| v)
            }
        })
    }

    pub fn resolve_entity_hashcode(
        &self,
        file: Hashcode,
        entity_hashcode: Hashcode,
    ) -> Option<Hashcode> {
        self.files.get(&file).and_then(|v| {
            if entity_hashcode.is_local() {
                v.0.iter()
                    .find(|(_, (index, _))| *index == entity_hashcode.index() as usize)
                    .map(|(hashcode, _)| *hashcode)
            } else {
                v.0.contains_key(&entity_hashcode)
                    .then_some(entity_hashcode)
            }
        })
    }

    pub fn resolve_script_hashcode(
        &self,
        file: Hashcode,
        script_hashcode: Hashcode,
    ) -> Option<Hashcode> {
        self.files.get(&file).and_then(|v| {
            if script_hashcode.is_local() {
                v.2.get(script_hashcode.index() as usize)
                    .map(|script| script.hashcode)
            } else {
                v.2.iter()
                    .any(|script| script.hashcode == script_hashcode)
                    .then_some(script_hashcode)
            }
        })
    }

    pub fn get_script(&self, file: Hashcode, script_hashcode: Hashcode) -> Option<&UXGeoScript> {
        let resolved = self.resolve_script_hashcode(file, script_hashcode)?;
        self.files
            .get(&file)
            .and_then(|v| v.2.iter().find(|script| script.hashcode == resolved))
    }

    pub fn resolve_particle_hashcode(
        &self,
        file: Hashcode,
        particle_hashcode: Hashcode,
    ) -> Option<Hashcode> {
        self.files.get(&file).and_then(|v| {
            if particle_hashcode.is_local() {
                v.5.iter()
                    .find(|(_, (index, _))| *index == particle_hashcode.index() as usize)
                    .map(|(hashcode, _)| *hashcode)
            } else {
                v.5.contains_key(&particle_hashcode)
                    .then_some(particle_hashcode)
            }
        })
    }

    pub fn resolve_animskin_hashcode(
        &self,
        file: Hashcode,
        skin_hashcode: Hashcode,
    ) -> Option<Hashcode> {
        self.files.get(&file).and_then(|v| {
            if skin_hashcode.is_local() {
                v.4.iter()
                    .find(|(_, (index, _))| *index == skin_hashcode.index() as usize)
                    .map(|(hashcode, _)| *hashcode)
            } else {
                v.4.contains_key(&skin_hashcode).then_some(skin_hashcode)
            }
        })
    }

    pub fn get_particle(
        &self,
        file: Hashcode,
        particle_hashcode: Hashcode,
    ) -> Option<&EXGeoParticle> {
        self.files.get(&file).and_then(|v| {
            if particle_hashcode.is_local() {
                v.5.iter()
                    .find(|(_, (index, _))| *index == particle_hashcode.index() as usize)
                    .map(|(_, (_, particle))| particle)
            } else {
                v.5.get(&particle_hashcode).map(|(_, particle)| particle)
            }
        })
    }

    pub fn find_assembly_script(&self, file: Hashcode, body: Hashcode) -> Option<Hashcode> {
        let file_data = self.files.get(&file)?;
        let body_index = if body.is_local() {
            Some(body.index() as usize)
        } else {
            file_data.0.get(&body).map(|(index, _)| *index)
        };

        file_data.2.iter().find_map(|script| {
            let entities = script
                .commands
                .iter()
                .filter_map(|command| match command.data {
                    UXGeoScriptCommandData::Entity { hashcode, .. } => Some(hashcode),
                    _ => None,
                });
            let mut has_body = false;
            let mut count = 0;
            for entity in entities {
                has_body |= if entity.is_local() {
                    body_index == Some(entity.index() as usize)
                } else {
                    entity == body
                };
                count += 1;
            }
            (has_body && count > 1).then_some(script.hashcode)
        })
    }

    // pub fn iter_entities(&self, file: Hashcode) -> Option<Iter<u32, EntityRenderer>> {
    //     self.files.get(&file).map(|v| v.0.iter())
    // }

    pub fn get_animskin_entities(
        &self,
        file: Hashcode,
        skin_hashcode: Hashcode,
    ) -> Option<&Vec<Hashcode>> {
        self.files.get(&file).and_then(|v| {
            if skin_hashcode.is_local() {
                v.4.iter()
                    .find(|(_, (index, _))| *index == skin_hashcode.index() as usize)
                    .map(|(_, (_, entities))| entities)
            } else {
                v.4.get(&skin_hashcode).map(|(_, entities)| entities)
            }
        })
    }
    pub fn get_texture(
        &self,
        file: Hashcode,
        texture_hashcode: Hashcode,
    ) -> Option<&RenderableTexture> {
        self.files
            .get(&file)
            .and_then(|v| v.1.get(&texture_hashcode).map(|(_, v)| v))
    }

    pub fn get_texture_by_index(
        &self,
        file: Hashcode,
        index: usize,
    ) -> Option<(u32, &RenderableTexture)> {
        self.files.get(&file).and_then(|v| {
            v.1.iter()
                .find(|(_, (v, _))| *v == index)
                .map(|(hc, (_, v))| (*hc, v))
        })
    }

    fn insert_hashcode(&mut self, file: Hashcode, hashcode: Hashcode) {
        if let Some(v) = self.files.get_mut(&file) {
            v.3.push(hashcode);
        }
    }

    // pub fn is_file_loaded(&self, file: Hashcode) -> bool {
    //     self.files.contains_key(&file)
    // }

    pub fn is_object_loaded(&self, file: Hashcode, hashcode: Hashcode) -> bool {
        match hashcode.base() {
            HC_BASE_ENTITY | HC_BASE_SCRIPT | HC_BASE_TEXTURE | HC_BASE_ANIMSKIN
            | HC_BASE_PARTICLE => {}
            // Animation motion is a known dependency, but skeletal sampling is not yet
            // implemented as a standalone RenderStore object.
            HC_BASE_ANIMATION => return true,
            v => {
                debug!("Checked load for unknown object type 0x{v:x} (hc {hashcode:08x})");
                return true;
            }
        }

        self.files
            .get(&file)
            .map(|f| f.3.contains(&hashcode))
            .unwrap_or(false)
    }

    pub fn insert_entity(
        &mut self,
        file: Hashcode,
        entity_hashcode: Hashcode,
        index: usize,
        entity: EntityRenderer,
    ) {
        let file_entry = match self.files.entry(file) {
            std::collections::hash_map::Entry::Occupied(o) => &mut o.into_mut().0,
            std::collections::hash_map::Entry::Vacant(v) => &mut v.insert(Default::default()).0,
        };

        file_entry.insert(entity_hashcode, (index, entity));
        self.insert_hashcode(file, entity_hashcode);
    }

    pub fn insert_texture(
        &mut self,
        file: Hashcode,
        texture_hashcode: Hashcode,
        index: usize,
        texture: RenderableTexture,
    ) {
        let file_entry = match self.files.entry(file) {
            std::collections::hash_map::Entry::Occupied(o) => &mut o.into_mut().1,
            std::collections::hash_map::Entry::Vacant(v) => &mut v.insert(Default::default()).1,
        };

        file_entry.insert(texture_hashcode, (index, texture));
        self.insert_hashcode(file, texture_hashcode);
    }

    pub fn insert_animskin(
        &mut self,
        file: Hashcode,
        skin_hashcode: Hashcode,
        index: usize,
        entities: Vec<Hashcode>,
    ) {
        let file_entry = match self.files.entry(file) {
            std::collections::hash_map::Entry::Occupied(o) => &mut o.into_mut().4,
            std::collections::hash_map::Entry::Vacant(v) => &mut v.insert(Default::default()).4,
        };

        file_entry.insert(skin_hashcode, (index, entities));
        self.insert_hashcode(file, skin_hashcode);
    }

    pub fn insert_particle(&mut self, file: Hashcode, particle: EXGeoParticle) {
        let file_entry = match self.files.entry(file) {
            std::collections::hash_map::Entry::Occupied(o) => &mut o.into_mut().5,
            std::collections::hash_map::Entry::Vacant(v) => &mut v.insert(Default::default()).5,
        };

        let hashcode = particle.hashcode;
        file_entry.insert(hashcode, (particle.index, particle));
        self.insert_hashcode(file, hashcode);
    }

    pub fn insert_script(&mut self, file: Hashcode, script: UXGeoScript) {
        let file_entry = match self.files.entry(file) {
            std::collections::hash_map::Entry::Occupied(o) => &mut o.into_mut().2,
            std::collections::hash_map::Entry::Vacant(v) => &mut v.insert(Default::default()).2,
        };

        let script_hashcode = script.hashcode;
        file_entry.push(script);
        self.insert_hashcode(file, script_hashcode);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eurochef_edb::versions::Platform;
    use eurochef_shared::script::UXGeoScriptCommand;

    fn entity_command(hashcode: Hashcode) -> UXGeoScriptCommand {
        UXGeoScriptCommand {
            opcode: 3,
            start: 0,
            length: 1,
            controller_header_index: 0,
            controller_index: 0,
            parent_controller_index: 0xff,
            data: UXGeoScriptCommandData::Entity {
                hashcode,
                file: 0x010000c1,
            },
        }
    }

    #[test]
    fn assembly_script_matches_global_body_to_local_entity_index() {
        let file = 0x010000c1;
        let global_body = 0x020001ae;
        let assembly_script = 0x0400026f;
        let mut store = RenderStore::new();

        store.insert_entity(
            file,
            global_body,
            2,
            EntityRenderer::new(file, Platform::Pc),
        );
        store.insert_script(
            file,
            UXGeoScript {
                hashcode: assembly_script,
                framerate: 30.0,
                length: 1,
                num_threads: 1,
                commands: vec![
                    entity_command(0x82000002),
                    entity_command(0x82000000),
                    entity_command(0x82000001),
                ],
                serialized_controller_count: 0,
                controller_record_metadata: vec![],
                controllers: vec![],
                controller_group_indices: vec![],
                controller_groups: vec![],
            },
        );

        assert_eq!(
            store.find_assembly_script(file, global_body),
            Some(assembly_script)
        );
    }

    #[test]
    fn spatial_world_light_sample_interpolates_vertex_colours_and_respects_vertical_span() {
        let triangle = NativeLightingTriangle {
            positions: [
                glam::Vec3::new(0.0, 0.0, 0.0),
                glam::Vec3::new(1.0, 0.0, 0.0),
                glam::Vec3::new(0.0, 0.0, 1.0),
            ],
            colours: [
                glam::Vec4::new(1.0, 0.0, 0.0, 1.0),
                glam::Vec4::new(0.0, 1.0, 0.0, 1.0),
                glam::Vec4::new(0.0, 0.0, 1.0, 1.0),
            ],
            zone_index: 0,
        };
        let zone = NativeLightZone {
            bounds_min: glam::Vec3::splat(-1.0),
            bounds_max: glam::Vec3::splat(2.0),
            light_indices: vec![],
            ambience: 0.75,
        };

        let sample = robots_world_light_sample(
            &[triangle],
            &[zone.clone()],
            Some(0),
            glam::Vec3::new(0.25, 10.0, 0.25),
        );
        assert!((sample - glam::Vec3::new(0.5, 0.25, 0.25)).length() < 0.000_001);

        let fallback = robots_world_light_sample(
            &[triangle],
            &[zone],
            Some(0),
            glam::Vec3::new(0.25, 51.0, 0.25),
        );
        assert_eq!(fallback, glam::Vec3::splat(0.5));
    }

    #[test]
    fn world_light_transform_uses_proven_coefficients_but_not_unproven_zone_ambience() {
        let coefficients = [0.3, 10.0, 0.2, 1.5, 2.0, 1.0];
        let without_ambience =
            robots_transform_world_light_sample(glam::Vec3::new(0.1, 0.2, 0.3), coefficients, None);
        let with_ambience = robots_transform_world_light_sample(
            glam::Vec3::new(0.1, 0.2, 0.3),
            coefficients,
            Some(0.01),
        );
        assert!((without_ambience - glam::Vec3::new(0.2, 0.4, 0.6)).length() < 0.000_001);
        assert_eq!(with_ambience, without_ambience);
    }

    #[test]
    fn live_lighting_uses_exact_repeated_smoothing_factors() {
        let mut state = RobotsLiveLightingState {
            slots: [RobotsDirectionalSlot {
                direction: glam::Vec3::X,
                colour: glam::Vec3::ZERO,
            }; 3],
            ambient: glam::Vec3::ZERO,
            target_slots: [RobotsDirectionalSlot {
                direction: glam::Vec3::Y,
                colour: glam::Vec3::ONE,
            }; 3],
            target_ambient: glam::Vec3::ONE,
            sample_position: glam::Vec3::ZERO,
            last_tick: 0,
            initialized: true,
        };
        robots_advance_live_lighting(&mut state, 1);
        assert!((state.slots[0].colour.x - 0.05).abs() < 0.000_001);
        assert!((state.ambient.x - 0.1).abs() < 0.000_001);
        assert!((state.slots[0].direction.length() - 1.0).abs() < 0.000_001);

        robots_advance_live_lighting(&mut state, 2);
        assert!((state.slots[0].colour.x - 0.0975).abs() < 0.000_001);
        assert!((state.ambient.x - 0.19).abs() < 0.000_001);
    }

    #[test]
    fn local_script_and_animskin_references_resolve_to_global_hashcodes() {
        let file = 0x010000c1;
        let mut store = RenderStore::new();
        store.insert_script(
            file,
            UXGeoScript {
                hashcode: 0x04000123,
                framerate: 30.0,
                length: 1,
                num_threads: 1,
                commands: vec![],
                serialized_controller_count: 0,
                controller_record_metadata: vec![],
                controllers: vec![],
                controller_group_indices: vec![],
                controller_groups: vec![],
            },
        );
        store.insert_animskin(file, 0x0d000055, 7, vec![]);

        assert_eq!(
            store.resolve_script_hashcode(file, 0x84000000),
            Some(0x04000123)
        );
        assert_eq!(
            store.resolve_animskin_hashcode(file, 0x8d000007),
            Some(0x0d000055)
        );
        assert_eq!(
            store.get_script(file, 0x84000000).map(|v| v.hashcode),
            Some(0x04000123)
        );
    }

    #[test]
    fn robots_global_lighting_uses_exact_level_fallback_values() {
        let village = robots_global_lighting(0x01000012).unwrap();
        assert_eq!(village.direction.x.to_bits(), 0x3f64_f92b);
        assert_eq!(village.direction.y.to_bits(), 0xbee4_f93c);
        assert_eq!(village.direction.z.to_bits(), 0);
        assert!((village.direction.length() - 1.0).abs() < 0.000_001);
        assert_eq!(village.colour, glam::Vec3::splat(0.30));
        assert_eq!(village.ambient, glam::Vec3::splat(0.40));
        assert_eq!(
            village.level_coefficients,
            [0.30, 1.75, 0.40, 1.50, 1.80, 0.60]
        );

        let hub1 = robots_global_lighting(0x01000071).unwrap();
        assert_eq!(hub1.colour, glam::Vec3::splat(0.25));
        assert_eq!(hub1.ambient, glam::Vec3::splat(0.46));
        assert!(robots_global_lighting(0x0100ffff).is_none());
    }
}
