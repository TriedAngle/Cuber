const std = @import("std");
const mem = std.mem;
const gl = @import("gl");
const m = @import("math");
const tex = @import("textures.zig");

pub const Program = struct {
    const Self = @This();
    handle: gl.GLuint,
    uniforms: std.AutoHashMap([*c]const u8, gl.GLint),
    active: bool = false,

    pub fn new_simple(allocator: std.mem.Allocator, vertex_source: [:0]const u8, fragment_source: [:0]const u8) Self {
        const program = make_simple_program(vertex_source, fragment_source);
        const uniforms = std.AutoHashMap([*c]const u8, gl.GLint).init(allocator);
        return Self{ .handle = program, .uniforms = uniforms };
    }

    pub fn new_compute(allocator: std.mem.Allocator, source: [:0]const u8) Self {
        const program = make_compute_program(source);
        const uniforms = std.AutoHashMap([*c]const u8, gl.GLint).init(allocator);
        return Self{ .handle = program, .uniforms = uniforms };
    }

    pub fn deinit(self: *Self) void {
        gl.deleteProgram(self.handle);
        self.uniforms.deinit();
    }

    pub fn uniform(self: *Self, name: [*c]const u8, comptime T: type, value: T) void {
        if (!self.uniforms.contains(name)) {
            self.load_uniform(name);
        }
        const location = self.uniforms.get(name).?;

        switch (T) {
            u32 => gl.programUniform1ui(self.handle, location, value),
            i32 => gl.programUniform1i(self.handle, location, value),
            f32 => gl.programUniform1f(self.handle, location, value),
            [2]u32 => gl.programUniform2ui(self.handle, location, value[0], value[1]),
            [2]i32 => gl.programUniform2i(self.handle, location, value[0], value[1]),
            [2]f32 => gl.programUniform2f(self.handle, location, value[0], value[1]),
            m.Vec2 => gl.programUniform2f(self.handle, location, value.gx(), value.gy()),
            m.Vec3 => gl.programUniform3f(self.handle, location, value.gx(), value.gy(), value.gz()),
            m.Vec4 => gl.programUniform4f(self.handle, location, value.gx(), value.gy(), value.gz(), value.gw()),
            m.Mat2 => gl.programUniformMatrix2fv(self.handle, location, 1, gl.FALSE, &value.inner[0][0]),
            m.Mat3 => gl.programUniformMatrix3fv(self.handle, location, 1, gl.FALSE, &value.inner[0][0]),
            m.Mat4 => gl.programUniformMatrix4fv(self.handle, location, 1, gl.FALSE, &value.inner[0][0]),
            *tex.Texture => gl.programUniform1ui(self.handle, location, value.binding),
            else => unreachable,
        }
    }

    pub fn load_uniform(self: *Self, name: [*c]const u8) void {
        const id = gl.getUniformLocation(self.handle, name);
        self.uniforms.put(name, id) catch unreachable;
    }

    pub fn use(self: *Self) void {
        self.active = true;
        gl.useProgram(self.handle);
    }

    pub fn unuse(self: *Self) void {
        self.active = false;
        gl.useProgram(0);
    }

    pub fn dispatch(self: *Self, groups_x: u32, groups_y: u32, groups_z: u32) void {
        _ = self;
        gl.dispatchCompute(groups_x, groups_y, groups_z);
    }
};

pub fn check_shader(shader: u32) !void {
    var status: i32 = undefined;
    gl.getShaderiv(shader, gl.COMPILE_STATUS, &status);
    if (status != 1) {
        var log_length: i32 = undefined;
        gl.getShaderiv(shader, gl.INFO_LOG_LENGTH, &log_length);
        var error_log = try std.heap.c_allocator.alloc(u8, @as(usize, @intCast(log_length)));
        defer std.heap.c_allocator.free(error_log);
        gl.getShaderInfoLog(shader, log_length, null, error_log.ptr);
        std.debug.panic("OpengGL Error: {s}", .{error_log});
    }
}

pub fn check_program(program: u32) !void {
    var status: i32 = undefined;
    gl.getProgramiv(program, gl.LINK_STATUS, &status);
    if (status != 1) {
        var log_length: i32 = undefined;
        gl.getProgramiv(program, gl.INFO_LOG_LENGTH, &log_length);
        var error_log = try std.heap.c_allocator.alloc(u8, @as(usize, @intCast(log_length)));
        defer std.heap.c_allocator.free(error_log);
        gl.getProgramInfoLog(program, log_length, null, error_log.ptr);
        std.debug.panic("OpengGL Error: {s}", .{error_log});
    }
}

pub fn make(source: [:0]const u8, kind: u32) u32 {
    const shader = gl.createShader(kind);
    const sources = [_][*]const u8{source.ptr};
    gl.shaderSource(shader, 1, &sources, null);
    gl.compileShader(shader);
    return shader;
}

pub fn make_vertex(source: [:0]const u8) u32 {
    const shader = make(source, gl.VERTEX_SHADER);
    check_shader(shader) catch std.process.exit(1);
    return shader;
}

pub fn make_fragment(source: [:0]const u8) u32 {
    const shader = make(source, gl.FRAGMENT_SHADER);
    check_shader(shader) catch std.process.exit(1);
    return shader;
}

pub fn make_compute(source: [:0]const u8) u32 {
    const shader = make(source, gl.COMPUTE_SHADER);
    check_shader(shader) catch std.process.exit(1);
    return shader;
}

pub fn make_program(shaders: []const u32) u32 {
    var program = gl.createProgram();
    for (shaders) |shader| {
        gl.attachShader(program, shader);
    }
    gl.linkProgram(program);
    check_program(program) catch std.process.exit(1);
    return program;
}

pub fn make_simple_program(vertex_source: [:0]const u8, fragment_source: [:0]const u8) u32 {
    const vertex = make_vertex(vertex_source);
    defer gl.deleteShader(vertex);
    const fragment = make_fragment(fragment_source);
    defer gl.deleteShader(fragment);
    const shaders = [_]u32{ vertex, fragment };
    const program = make_program(&shaders);
    return program;
}

pub fn make_compute_program(compute_source: [:0]const u8) u32 {
    const compute = make_compute(compute_source);
    defer gl.deleteShader(compute);
    const shaders = [_]u32{compute};
    const program = make_program(&shaders);
    return program;
}
