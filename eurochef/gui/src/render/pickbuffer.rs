use glam::{IVec2, Mat4};
use glow::HasContext;

use super::viewer::RenderContext;

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickBufferType {
    Trigger = 1,
    Sound = 2,
}

#[derive(Clone)]
pub struct PickBuffer {
    pub framebuffer: Option<glow::Framebuffer>,
    color_texture: Option<glow::Texture>,
    size: IVec2,
}

impl PickBuffer {
    pub fn new(_gl: &glow::Context) -> Self {
        Self {
            framebuffer: None,
            color_texture: None,
            size: IVec2::ZERO,
        }
    }

    pub fn init_draw(&mut self, gl: &glow::Context, size: IVec2) {
        let size = size.max(IVec2::ONE);
        unsafe {
            if self.framebuffer.is_some() && self.color_texture.is_some() && self.size == size {
                self.prepare_draw(gl, size, true);
                gl.bind_framebuffer(glow::FRAMEBUFFER, None);
                return;
            }

            if let Some(framebuffer) = self.framebuffer.take() {
                gl.delete_framebuffer(framebuffer);
            }
            if let Some(texture) = self.color_texture.take() {
                gl.delete_texture(texture);
            }

            let framebuffer = gl
                .create_framebuffer()
                .expect("Failed to create framebuffer");
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));

            let color_texture = gl.create_texture().expect("Failed to create color texture");
            gl.bind_texture(glow::TEXTURE_2D, Some(color_texture));
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::NEAREST as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::NEAREST as i32,
            );
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA8 as i32,
                size.x,
                size.y,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None),
            );
            gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(color_texture),
                0,
            );

            if gl.check_framebuffer_status(glow::FRAMEBUFFER) != glow::FRAMEBUFFER_COMPLETE {
                panic!("Framebuffer is not complete");
            }

            self.framebuffer = Some(framebuffer);
            self.color_texture = Some(color_texture);
            self.size = size;
            self.prepare_draw(gl, size, true);
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        }
    }

    unsafe fn prepare_draw(&self, gl: &glow::Context, size: IVec2, clear: bool) {
        gl.bind_framebuffer(glow::FRAMEBUFFER, self.framebuffer);
        gl.viewport(0, 0, size.x, size.y);
        gl.disable(glow::BLEND);
        gl.disable(glow::SAMPLE_ALPHA_TO_COVERAGE);
        gl.disable(glow::DITHER);
        gl.disable(glow::DEPTH_TEST);
        gl.color_mask(true, true, true, true);
        if clear {
            gl.clear_color(0.0, 0.0, 0.0, 0.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
        }
    }

    pub fn draw<F>(
        &self,
        context: &RenderContext,
        gl: &glow::Context,
        model: Mat4,
        id: (PickBufferType, u32),
        draw_callback: F,
    ) where
        F: Fn(&glow::Context),
    {
        unsafe {
            self.prepare_draw(gl, self.size.max(IVec2::ONE), false);

            let shader = context.shaders.pickbuffer;
            gl.use_program(Some(shader));
            gl.uniform_matrix_4_f32_slice(
                gl.get_uniform_location(shader, "u_view").as_ref(),
                false,
                &context.uniforms.view.to_cols_array(),
            );
            gl.uniform_matrix_4_f32_slice(
                gl.get_uniform_location(shader, "u_model").as_ref(),
                false,
                &model.to_cols_array(),
            );
            gl.uniform_1_u32(
                gl.get_uniform_location(shader, "u_type").as_ref(),
                id.0 as u32,
            );
            gl.uniform_1_u32(gl.get_uniform_location(shader, "u_id").as_ref(), id.1);

            draw_callback(gl);
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        }
    }
}

pub fn decode_pick_value(pixel: [u8; 4]) -> Option<(u32, u32)> {
    let packed = u32::from_le_bytes(pixel);
    let object_type = (packed >> 20) & 0x0f;
    (object_type != 0).then_some((object_type, packed & 0x000f_ffff))
}

#[cfg(test)]
mod tests {
    use super::{decode_pick_value, PickBufferType};

    #[test]
    fn pick_value_round_trips_trigger_type_and_twenty_bit_id() {
        let packed = ((PickBufferType::Trigger as u32) << 20) | 0x0005_4321;
        assert_eq!(
            decode_pick_value(packed.to_le_bytes()),
            Some((PickBufferType::Trigger as u32, 0x0005_4321))
        );
        assert_eq!(decode_pick_value([0; 4]), None);
    }
}
