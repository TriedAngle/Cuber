const gl = @import("gl");

pub const shaders = @import("shaders.zig");
pub const textures = @import("textures.zig");
pub const buffers = @import("buffers.zig");

pub fn uniform_location(program: u32, name: [*c]const u8) i32 {
    return gl.getUniformLocation(program, name);
}
