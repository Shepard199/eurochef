use std::{fmt::Display, sync::Arc};

use glam::Vec3;
use instant::Instant;

use super::{
    camera::{ArcBallCamera, Camera3D, FpsCamera, NativeViewCamera},
    grid::GridRenderer,
    shaders::Shaders,
    RenderUniforms,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraType {
    Orbit,
    Fly,
}

impl Display for CameraType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CameraType::Orbit => f.write_str("Orbit"),
            CameraType::Fly => f.write_str("Fly"),
        }
    }
}

pub struct RenderContext<'r> {
    pub shaders: &'r Shaders,
    pub uniforms: &'r RenderUniforms,
    pub lighting_key: u64,
}

pub struct BaseViewer {
    pub show_grid: bool,
    pub orthographic: bool,
    pub camera_orbit: ArcBallCamera,
    pub camera_fly: FpsCamera,
    native_camera: Option<NativeViewCamera>,
    pub selected_camera: CameraType,
    pub grid: GridRenderer,
    pub uniforms: RenderUniforms,
    pub shaders: Arc<Shaders>,

    last_frame: Instant,
}

impl BaseViewer {
    pub fn new(gl: &glow::Context) -> Self {
        Self {
            camera_orbit: ArcBallCamera::default(),
            camera_fly: FpsCamera::default(),
            native_camera: None,
            selected_camera: CameraType::Orbit,
            show_grid: true,
            orthographic: false,
            grid: GridRenderer::new(gl, 30),
            uniforms: RenderUniforms {
                native_light_strength: 1.0,
                ..Default::default()
            },
            shaders: Arc::new(Shaders::load_shaders(gl)),
            last_frame: Instant::now(),
        }
    }

    pub fn show_toolbar(&mut self, ui: &mut egui::Ui) {
        if self.selected_camera == CameraType::Orbit {
            ui.checkbox(&mut self.orthographic, "Orthographic");
        } else {
            ui.checkbox(&mut self.camera_fly.invert_mouse_y, "Invert mouse Y");
        }
        ui.checkbox(&mut self.show_grid, "Show grid");

        let mut requested_camera = self.selected_camera;
        egui::ComboBox::from_label("Camera")
            .selected_text(self.selected_camera.to_string())
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut requested_camera, CameraType::Orbit, "Orbit");
                ui.selectable_value(&mut requested_camera, CameraType::Fly, "Fly");
            });
        if requested_camera != self.selected_camera {
            self.set_camera_type_preserve_view(requested_camera);
        }
    }

    fn set_camera_type_preserve_view(&mut self, requested: CameraType) {
        if requested == self.selected_camera {
            return;
        }

        match (self.selected_camera, requested) {
            (CameraType::Orbit, CameraType::Fly) => {
                let inverse = self.camera_orbit.calculate_matrix().inverse();
                let position = inverse.transform_point3(Vec3::ZERO);
                let direction = inverse.transform_vector3(Vec3::Z).normalize_or_zero();
                self.camera_fly.set_pose_from_direction(position, direction);
            }
            (CameraType::Fly, CameraType::Orbit) => {
                self.camera_orbit
                    .set_pose_from_direction(self.camera_fly.position, self.camera_fly.front);
            }
            _ => {}
        }
        self.selected_camera = requested;
    }

    pub fn camera(&self) -> &dyn Camera3D {
        if let Some(camera) = &self.native_camera {
            return camera;
        }
        match self.selected_camera {
            CameraType::Fly => &self.camera_fly,
            CameraType::Orbit => &self.camera_orbit,
        }
    }

    pub fn camera_mut(&mut self) -> &mut dyn Camera3D {
        if let Some(camera) = &mut self.native_camera {
            return camera;
        }
        match self.selected_camera {
            CameraType::Fly => &mut self.camera_fly,
            CameraType::Orbit => &mut self.camera_orbit,
        }
    }

    pub fn set_native_camera(&mut self, camera: NativeViewCamera) {
        self.native_camera = Some(camera);
    }

    pub fn clear_native_camera(&mut self) {
        self.native_camera = None;
    }

    pub fn set_fly_camera_pose(&mut self, position: Vec3, direction: Vec3) {
        self.selected_camera = CameraType::Fly;
        self.camera_fly.set_pose_from_direction(position, direction);
    }

    pub fn show_statusbar(&mut self, ui: &mut egui::Ui) {
        if self.selected_camera == CameraType::Fly {
            ui.strong("Speed:");
        } else {
            ui.strong("Zoom:");
        }
        ui.label(format!("{:.2}", self.camera().zoom()));
    }

    pub fn update(&mut self, ui: &mut egui::Ui, response: &egui::Response) {
        if ui.input(|i| i.key_pressed(egui::Key::F)) {
            let requested = match self.selected_camera {
                CameraType::Orbit => CameraType::Fly,
                CameraType::Fly => CameraType::Orbit,
            };
            self.set_camera_type_preserve_view(requested);
        }

        if ui.input(|i| i.key_pressed(egui::Key::G)) {
            self.show_grid = !self.show_grid;
        }

        if ui.input(|i| i.key_pressed(egui::Key::O) || i.key_pressed(egui::Key::Num5)) {
            self.orthographic = !self.orthographic;
        }

        let delta = (Instant::now() - self.last_frame).as_secs_f32();
        let camera = self.camera_mut();
        camera.update(ui, Some(response), delta);
        self.last_frame = Instant::now();
    }

    pub fn start_render(&mut self, gl: &glow::Context, aspect_ratio: f32, time: f32) {
        unsafe {
            super::start_render(gl);
        }

        let orthographic = self.native_camera.is_none()
            && self.selected_camera == CameraType::Orbit
            && self.orthographic;
        let uniforms = &mut self.uniforms;
        if let Some(camera) = self.native_camera.as_mut() {
            uniforms.update(false, camera, aspect_ratio, time);
        } else {
            match self.selected_camera {
                CameraType::Fly => uniforms.update(false, &mut self.camera_fly, aspect_ratio, time),
                CameraType::Orbit => {
                    uniforms.update(orthographic, &mut self.camera_orbit, aspect_ratio, time)
                }
            }
        }

        if self.show_grid {
            unsafe { self.grid.draw(&self.render_context(), gl) }
        }
    }

    pub fn focus_on_point(&mut self, point: Vec3, dist_scale: f32) {
        self.camera_mut().focus_on_point(point, dist_scale);
    }

    pub fn render_context(&self) -> RenderContext<'_> {
        RenderContext {
            shaders: &self.shaders,
            uniforms: &self.uniforms,
            lighting_key: 0,
        }
    }
}
