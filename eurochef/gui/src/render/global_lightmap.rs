use glam::{Vec2, Vec3Swizzles};
use glow::HasContext;

use super::NativeLightingTriangle;

pub struct GpuGlobalLightmap {
    pub texture: glow::Texture,
    pub min: Vec2,
    pub span: Vec2,
}

pub unsafe fn bake(
    gl: &glow::Context,
    shader: glow::Program,
    triangles: &[NativeLightingTriangle],
) -> Option<GpuGlobalLightmap> {
    if triangles.is_empty() {
        return None;
    }
    let mut min = Vec2::splat(f32::INFINITY);
    let mut max = Vec2::splat(f32::NEG_INFINITY);
    for triangle in triangles {
        for position in triangle.positions {
            min = min.min(position.xz());
            max = max.max(position.xz());
        }
    }
    let span = (max - min).max(Vec2::splat(0.001));
    let mut vertices = Vec::<f32>::with_capacity(triangles.len() * 15);
    for triangle in triangles {
        for index in 0..3 {
            let position = (triangle.positions[index].xz() - min) / span * 2.0 - Vec2::ONE;
            vertices.extend_from_slice(&[position.x, position.y]);
            vertices.extend_from_slice(&triangle.colours[index].truncate().to_array());
        }
    }
    const SIZE: i32 = 1024;
    let texture = gl.create_texture().ok()?;
    gl.bind_texture(glow::TEXTURE_2D, Some(texture));
    gl.tex_image_2d(
        glow::TEXTURE_2D,
        0,
        glow::RGBA as i32,
        SIZE,
        SIZE,
        0,
        glow::RGBA,
        glow::UNSIGNED_BYTE,
        glow::PixelUnpackData::Slice(None),
    );
    gl.tex_parameter_i32(
        glow::TEXTURE_2D,
        glow::TEXTURE_MIN_FILTER,
        glow::LINEAR as i32,
    );
    gl.tex_parameter_i32(
        glow::TEXTURE_2D,
        glow::TEXTURE_MAG_FILTER,
        glow::LINEAR as i32,
    );
    gl.tex_parameter_i32(
        glow::TEXTURE_2D,
        glow::TEXTURE_WRAP_S,
        glow::CLAMP_TO_EDGE as i32,
    );
    gl.tex_parameter_i32(
        glow::TEXTURE_2D,
        glow::TEXTURE_WRAP_T,
        glow::CLAMP_TO_EDGE as i32,
    );
    let framebuffer = gl.create_framebuffer().ok()?;
    let vao = gl.create_vertex_array().ok()?;
    let buffer = gl.create_buffer().ok()?;
    gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));
    gl.framebuffer_texture_2d(
        glow::FRAMEBUFFER,
        glow::COLOR_ATTACHMENT0,
        glow::TEXTURE_2D,
        Some(texture),
        0,
    );
    if gl.check_framebuffer_status(glow::FRAMEBUFFER) != glow::FRAMEBUFFER_COMPLETE {
        gl.delete_framebuffer(framebuffer);
        gl.delete_vertex_array(vao);
        gl.delete_buffer(buffer);
        gl.delete_texture(texture);
        return None;
    }
    gl.viewport(0, 0, SIZE, SIZE);
    gl.disable(glow::DEPTH_TEST);
    gl.disable(glow::BLEND);
    gl.clear_color(0.5, 0.5, 0.5, 1.0);
    gl.clear(glow::COLOR_BUFFER_BIT);
    gl.use_program(Some(shader));
    gl.bind_vertex_array(Some(vao));
    gl.bind_buffer(glow::ARRAY_BUFFER, Some(buffer));
    gl.buffer_data_u8_slice(
        glow::ARRAY_BUFFER,
        bytemuck::cast_slice(&vertices),
        glow::STATIC_DRAW,
    );
    gl.enable_vertex_attrib_array(0);
    gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 20, 0);
    gl.enable_vertex_attrib_array(1);
    gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, 20, 8);
    gl.draw_arrays(glow::TRIANGLES, 0, (vertices.len() / 5) as i32);
    gl.bind_framebuffer(glow::FRAMEBUFFER, None);
    gl.delete_framebuffer(framebuffer);
    gl.delete_vertex_array(vao);
    gl.delete_buffer(buffer);
    Some(GpuGlobalLightmap { texture, min, span })
}
