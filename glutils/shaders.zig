const std = @import("std");
const gl = @import("gl");

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
