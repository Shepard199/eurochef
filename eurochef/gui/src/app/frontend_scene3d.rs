//! Self-contained H00 frontend scene playback. Reuses existing native
//! Script controller interpolation and EntityRenderer GL path; never mutates
//! the gameplay MapFrame or its RenderStore.
use std::{fs::File, io::BufReader, path::Path, sync::Arc};
use anyhow::{Context, Result};
use egui::mutex::{Mutex, RwLock};
use eurochef_edb::{edb::EdbFile, particle::EXGeoParticle, versions::Platform, HashcodeUtils};
use eurochef_shared::{script::{UXGeoScript, UXGeoScriptCommandData}, textures::UXGeoTexture};
use glam::{EulerRot, Quat, Vec2, Vec3};
use glow::HasContext;

use crate::{
    entities,
    app::frontend_native_camera::NativeCameraCapture,
    map_frame::QueuedEntityRender,
    render::{
        entity::EntityRenderer,
        script::{render_script, collect_script_particles},
        particle::{ParticleRenderer, ParticlePreviewSettings},
        viewer::BaseViewer,
        RenderStore,
    },
};

pub(super) const FRONTEND_SCENE_UID: u32 = 0x0100_000D;

/// Dedicated renderer: resources owned by the m00 EDB, and no gameplay state.
pub(super) struct FrontendScene3D {
    store: Arc<RwLock<RenderStore>>,
    viewer: Arc<Mutex<BaseViewer>>,
    camera_capture: Option<Arc<NativeCameraCapture>>,
    particles: Arc<ParticleRenderer>,
    /// Static MapZone refpointer meshes, separate from script-controlled Entity instances.
    zone_geometry: Arc<Vec<EntityRenderer>>,
    /// Original M00 Map placements, resolved by EntityRenderer and native transforms.
    placements: Arc<Vec<QueuedEntityRender>>,
    file_uid: u32,
    /// Local timeline roots containing genuine Entity or Particle commands.
    roots: Vec<(u32, f32, bool)>,
}

impl FrontendScene3D {
    pub(super) fn from_original_edb(gl: &glow::Context, path: &Path) -> Result<Self> {
        let input = File::open(path).with_context(|| format!("opening frontend 3D {}", path.display()))?;
        let mut edb = EdbFile::new(Box::new(BufReader::new(input)), Platform::Pc)?;
        let file_uid = edb.header.hashcode;
        anyhow::ensure!(file_uid == FRONTEND_SCENE_UID, "Expected original m00_fron EDB, got {file_uid:#010x}");

        let maps = crate::maps::read_from_file(&mut edb);
        let (entities, _, ref_entities) = entities::read_from_file(&mut edb, None)?;
        let scripts = UXGeoScript::read_all(&mut edb)?;
        let particles = EXGeoParticle::read_all(&mut edb)?;
        let texture_data = UXGeoTexture::read_all(&mut edb);
        let roots: Vec<_> = scripts.iter()
            .filter(|script| script.commands.iter().any(|command|
                matches!(command.data, UXGeoScriptCommandData::Entity { .. } | UXGeoScriptCommandData::Particle { .. })))
            .map(|script| {
                let loops = script.commands.iter().any(|command|
                    matches!(command.data, UXGeoScriptCommandData::Unknown { cmd: 0x10, .. }));
                (script.hashcode, script.duration_seconds(), loops)
            })
            .collect();
        anyhow::ensure!(!roots.is_empty(), "m00 frontend has no renderable script roots");

        let mut store = RenderStore::new();
        for script in scripts {
            store.insert_script(file_uid, script);
        }
        for particle in particles {
            store.insert_particle(file_uid, particle);
        }
        let mut center_sum = Vec3::ZERO;
        let mut valid_centers = 0u32;
        for (index, entity) in entities {
            let mut renderer = EntityRenderer::new(file_uid, Platform::Pc);
            if let Ok((_, mesh)) = &entity.data {
                let center = unsafe { renderer.load_mesh(gl, mesh) };
                if center.is_finite() {
                    center_sum += center;
                    valid_centers += 1;
                }
            }
            store.insert_entity(file_uid, entity.hashcode, index, renderer);
        }
        // MapZone geometry lives behind refpointer indices, not the normal
        // Entity list. Keep its static draw passes separate from script actors.
        let mut zone_geometry = Vec::new();
        let mut zone_center = Vec3::ZERO;
        let mut zone_count = 0usize;
        let mut zone_radius = 0.0f32;
        for map in &maps {
            for zone in &map.mapzone_entities {
                let Some((_, mesh)) = ref_entities.iter()
                    .find(|resource| resource.hashcode == zone.entity_refptr)
                    .and_then(|resource| resource.data.as_ref().ok()) else {
                    tracing::warn!("M00 zone mesh refpointer {} missing", zone.entity_refptr);
                    continue;
                };
                let (bounds_min, bounds_max) = mesh.bounding_box();
                let center = (bounds_min + bounds_max) * 0.5;
                if center.is_finite() {
                    zone_center += center;
                    zone_count += 1;
                    zone_radius = zone_radius.max((bounds_max - bounds_min).length() * 0.5);
                }
                let mut renderer = EntityRenderer::new(file_uid, Platform::Pc);
                unsafe { renderer.load_mesh(gl, mesh); }
                zone_geometry.push(renderer);
            }
        }
        tracing::info!("M00 frontend loaded {} static MapZone meshes", zone_geometry.len());
        let placements: Vec<QueuedEntityRender> = maps.iter()
            .flat_map(|map| map.placements.iter())
            .filter(|placement| placement.object_ref.base() == 0x0200_0000)
            .map(|placement| QueuedEntityRender {
                entity: (file_uid, placement.object_ref),
                entity_alt: None,
                position: placement.position.into(),
                rotation: Quat::from_euler(EulerRot::ZXY, placement.rotation[2],
                    placement.rotation[0], placement.rotation[1]),
                scale: placement.scale.into(),
            })
            .collect();
        tracing::info!("M00 frontend loaded {} Entity placements", placements.len());
        for (index, texture) in entities::EntityListPanel::load_textures(gl, file_uid, &texture_data) {
            store.insert_texture(file_uid, texture.hashcode, index, texture);
        }
        // Two formats: a direct 36-byte x32dbg dump (single measured
        // pose) or a timestamped text track for multiple captured frames.
        // Do not silently substitute bounds-based orbit as a native pose.
        let raw_path = path.with_file_name("frontend_camera_dump.bin");
        let capture_path = path.with_file_name("frontend_camera_capture.txt");
        let capture_result = if raw_path.is_file() {
            Some(std::fs::read(&raw_path)
                .with_context(|| format!("reading native camera dump {}", raw_path.display()))
                .and_then(|bytes| NativeCameraCapture::from_runtime_dump(&bytes)))
        } else if capture_path.is_file() {
            Some(std::fs::read_to_string(&capture_path)
                .with_context(|| format!("reading camera track {}", capture_path.display()))
                .and_then(|contents| NativeCameraCapture::parse(&contents)))
        } else { None };
        let camera_capture = match capture_result {
            Some(Ok(capture)) => {
                tracing::info!("Original camera capture loaded ({} samples)", capture.frame_count());
                Some(Arc::new(capture))
            }
            Some(Err(error)) => {
                tracing::warn!("Ignoring invalid frontend camera capture: {error:#}");
                None
            }
            None => None,
        };
        let mut viewer = BaseViewer::new(gl);
        viewer.show_grid = false;
        // The shipped ViewCamera route has not been resolved yet. Until it is,
        // focus the existing orbit viewer on real mesh bounds, not world zero.
        if zone_count > 0 {
            let focus = zone_center / zone_count as f32;
            let zoom = (zone_radius * 2.2).clamp(2.0, 10000.0);
            viewer.camera_orbit = crate::render::camera::ArcBallCamera::new(
                Vec3::ZERO, Vec2::new(12.0, 145.0), zoom, false);
            viewer.focus_on_point(focus, 1.0);
        } else if valid_centers > 0 {
            viewer.focus_on_point(center_sum / valid_centers as f32, 1.6);
        }
        Ok(Self {
            store: Arc::new(RwLock::new(store)),
            viewer: Arc::new(Mutex::new(viewer)),
            camera_capture,
            particles: Arc::new(ParticleRenderer::new(gl).map_err(anyhow::Error::msg)?),
            zone_geometry: Arc::new(zone_geometry),
            placements: Arc::new(placements),
            file_uid,
            roots,
        })
    }

    /// Enqueue an actual GPU layer underneath the egui HUD artwork.
    /// Camera is currently the editor viewer's default, not yet the shipped
    /// frontend ViewCamera; H04 event-based root ownership is also pending.
    pub(super) fn paint(&self, ui: &egui::Ui, rect: egui::Rect, elapsed: f32) {
        let store = self.store.clone();
        let viewer = self.viewer.clone();
        let camera_capture = self.camera_capture.clone();
        let particle_renderer = self.particles.clone();
        let zone_geometry = self.zone_geometry.clone();
        let placements = self.placements.clone();
        let particle_settings = ParticlePreviewSettings::default();
        let roots = self.roots.clone();
        let file_uid = self.file_uid;
        let time = elapsed.max(0.0);
        let cb = egui_glow::CallbackFn::new(move |info, painter| unsafe {
            let mut viewer = viewer.lock();
            if let Some(track) = camera_capture.as_ref() {
                viewer.set_native_camera(track.sample(time));
            }
            viewer.start_render(painter.gl(), info.viewport.aspect_ratio(), time);
            let context = viewer.render_context();
            let store = store.read();
            let mut queue = placements.iter().cloned().collect::<Vec<_>>();
            let mut particle_queue = Vec::new();
            for (script, duration, loops) in &roots {
                let local_time = if *loops && *duration > 0.0 {
                    time.rem_euclid(*duration)
                } else { time };
                render_script(
                    Vec3::ZERO, Quat::IDENTITY, Vec3::ONE,
                    file_uid, *script, local_time,
                    &store, &mut |render| queue.push(render), vec![],
                );
                collect_script_particles(Vec3::ZERO, Quat::IDENTITY, Vec3::ONE,
                    file_uid, *script, local_time, &store, &mut particle_queue, vec![]);
            }
            for zone in zone_geometry.iter() {
                zone.draw_opaque(painter.gl(), &context,
                    Vec3::ZERO, Quat::IDENTITY, Vec3::ONE, f64::from(time), &store);
            }
            for entry in &queue {
                if let Some(entity) = store.get_entity(entry.entity.0, entry.entity.1) {
                    entity.draw_opaque(painter.gl(), &context, entry.position,
                        entry.rotation, entry.scale, f64::from(time), &store);
                }
            }
            painter.gl().depth_mask(false);
            for zone in zone_geometry.iter() {
                zone.draw_transparent(painter.gl(), &context,
                    Vec3::ZERO, Quat::IDENTITY, Vec3::ONE, f64::from(time), &store);
            }
            for entry in &queue {
                if let Some(entity) = store.get_entity(entry.entity.0, entry.entity.1) {
                    entity.draw_transparent(painter.gl(), &context, entry.position,
                        entry.rotation, entry.scale, f64::from(time), &store);
                }
            }
            for emitter in &particle_queue {
                if let Some(particle) = store.get_particle(emitter.particle.0, emitter.particle.1) {
                    particle_renderer.render(painter.gl(), &context, emitter, particle,
                        particle_settings, &store);
                }
            }
        });
        ui.painter().add(egui::PaintCallback { rect, callback: Arc::new(cb) });
    }

    pub(super) fn script_count(&self) -> usize { self.roots.len() }
    pub(super) fn has_captured_camera(&self) -> bool { self.camera_capture.is_some() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    #[test]
    fn shipped_m00_entity_scripts_produce_real_native_render_queue() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/attachment_game_root/_eurotools_out/extracted_main/robots/binary/_bin_pc/m00_fron.edb");
        if !path.is_file() { return; }
        let file = File::open(path).unwrap();
        let mut edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).unwrap();
        assert_eq!(edb.header.hashcode, FRONTEND_SCENE_UID);
        let scripts = UXGeoScript::read_all(&mut edb).unwrap();
        assert!(scripts.iter().any(|script| script.commands.iter().any(|command|
            matches!(command.data, UXGeoScriptCommandData::Particle { .. }))),
            "Shipped M00 contains a scripted particle emitter");
        let map_data = crate::maps::read_from_file(&mut edb);
        for map in &map_data {
            eprintln!("M00 map={:#010x} cameras={} zones={} zone_entities={} placements={} skies={}",
                map.hashcode, map.cameras.len(), map.zones.len(), map.mapzone_entities.len(),
                map.placements.len(), map.skies.len());
            for camera in &map.cameras { eprintln!("  camera={camera:?}"); }
            for (index, placement) in map.placements.iter().enumerate() {
                eprintln!("  placement#{index} hash={:#010x} object={:#010x} position={:?} rotation={:?} scale={:?} flags={:#x}",
                    placement.hashcode, placement.object_ref, placement.position,
                    placement.rotation, placement.scale, placement.engine_flags);
            }
        }
        let (entities, _, ref_entities) = entities::read_from_file(&mut edb, None).unwrap();
        assert_eq!(map_data.len(), 1);
        assert_eq!(map_data[0].cameras.len(), 0);
        assert_eq!(map_data[0].placements.len(), 2);
        assert_eq!(map_data[0].placements[0].object_ref, 0x8200_0001);
        assert_eq!(map_data[0].placements[1].object_ref, 0x8200_0000);
        for map in &map_data {
            for (index, zone) in map.mapzone_entities.iter().enumerate() {
                let mesh = ref_entities.iter().find(|ref_entity| ref_entity.hashcode == zone.entity_refptr)
                    .and_then(|ref_entity| ref_entity.data.as_ref().ok()).map(|(_, mesh)| mesh);
                eprintln!("M00 zone#{index} refptr={} vertices={:?} strips={:?}",
                    zone.entity_refptr, mesh.map(|mesh| mesh.vertex_data.len()), mesh.map(|mesh| mesh.strips.len()));
                assert_eq!(mesh.map(|mesh| mesh.vertex_data.len()), Some(32789));
                assert_eq!(mesh.map(|mesh| mesh.strips.len()), Some(787));
            }
        }
        assert!(entities.iter().any(|(_, entity)| entity.data.is_ok()),
            "M00 must contain at least one decodable Entity mesh");
        let mut store = RenderStore::new();
        for script in &scripts { store.insert_script(FRONTEND_SCENE_UID, script.clone()); }
        let mut results = Vec::new();
        for script in &scripts {
            if !script.commands.iter().any(|cmd|
                matches!(cmd.data, UXGeoScriptCommandData::Entity { .. })) { continue; }
            for time in [0.0_f32, 0.5, 1.5] {
                let mut queue = Vec::new();
                render_script(Vec3::ZERO, Quat::IDENTITY, Vec3::ONE,
                    FRONTEND_SCENE_UID, script.hashcode, time, &store,
                    &mut |entry| queue.push(entry), vec![]);
                results.push((script.hashcode, time, queue.len()));
            }
        }
        eprintln!("M00 queued native Entity commands: {results:?}");
        assert!(results.iter().any(|(_, _, len)| *len > 0),
            "M00 scripts must yield actual native Entity requests");
    }
}
