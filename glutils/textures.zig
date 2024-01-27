const std = @import("std");
const gl = @import("gl");

pub const Texture = struct {
    const Self = @This();
    handle: gl.GLuint,
    kind: u32,
    format: u32,
    levels: i32,
    width: i32 = 0,
    height: i32 = 0,
    binding: u32 = 0,

    pub fn new(width: i32, height: i32, kind: u32, format: u32, levels: i32) Self {
        var self = Self.new_empty(kind, format, levels);
        self.width = width;
        self.height = height;
        gl.textureStorage2D(self.handle, levels, format, width, height);
        self.set_default_parameters();
        return self;
    }

    pub fn new_empty(kind: u32, format: u32, levels: i32) Self {
        var texture: u32 = undefined;
        gl.createTextures(kind, 1, &texture);
        return Self{ .handle = texture, .kind = kind, .format = format, .levels = levels };
    }

    pub fn new_dummy(kind: u32, format: u32, levels: i32) Self {
        return Self{ .handle = 0, .kind = kind, .format = format, .levels = levels };
    }

    pub fn deinit(self: *Self) void {
        gl.deleteTextures(1, &[_]u32{self.handle});
    }

    pub fn set_default_parameters(self: *Self) void {
        gl.textureParameteri(self.handle, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
        gl.textureParameteri(self.handle, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
        gl.textureParameteri(self.handle, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
        gl.textureParameteri(self.handle, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    }

    pub fn bind(self: *Self, binding: u32, level: i32, layer: i32) void {
        self.binding = binding;
        var layered: u8 = 0;
        if (layer != 0) {
            layered = 1;
        }
        gl.bindTextureUnit(binding, self.handle);
        gl.bindImageTexture(binding, self.handle, level, layered, layer, gl.READ_WRITE, self.format);
    }

    pub fn resize(self: *Self, width: i32, height: i32) void {
        self.deinit();
        self.* = Self.new(width, height, self.kind, self.format, self.levels);
    }
};

// TODO: remove the functions below
pub fn make_empty() u32 {
    var texture: u32 = undefined;
    gl.createTextures(gl.TEXTURE_2D, 1, &texture);
    return texture;
}

pub fn make(width: i32, height: i32, format: u32) u32 {
    const texture = make_empty();
    gl.textureStorage2D(texture, 1, format, width, height);
    gl.textureParameteri(texture, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.textureParameteri(texture, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.textureParameteri(texture, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.textureParameteri(texture, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    return texture;
}

pub fn free(texture: u32) void {
    gl.deleteTextures(1, &[_]u32{texture});
}
