const std = @import("std");
const gl = @import("gl");

pub fn make_empty() u32 {
    var texture: u32 = undefined;
    gl.createTextures(gl.TEXTURE, 1, &texture);
    return texture;
}

pub fn make_present(width: i32, height: i32) u32 {
    var texture = make_empty();
    gl.textureParameteri(texture, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.textureParameteri(texture, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.textureParameteri(texture, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.textureParameteri(texture, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.textureStorage2D(texture, 1, gl.RGBA, width, height);
    return texture;
}

pub fn free(texture: u32) void {
    gl.deleteTextures(1, &[_]u32{texture});
}
