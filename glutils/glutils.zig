const gl = @import("gl");

pub const shaders = @import("shaders.zig");
pub const textures = @import("textures.zig");
pub const buffers = @import("buffers.zig");

pub const Program = shaders.Program;
pub const Buffer = buffers.Buffer;
pub const Texture = textures.Texture;

// TODO: remove this
pub fn uniform_location(program: u32, name: [*c]const u8) i32 {
    return gl.getUniformLocation(program, name);
}
