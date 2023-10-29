const std = @import("std");
const sdfui = @import("sdfui");
const glfw = @import("mach-glfw");
const gl = @import("gl");
const glu = @import("glutils");

const m = @import("math");
const cam = @import("camera.zig");
const shaders = @import("shaders.zig");
const textures = @import("textures.zig");

// const c = @cImport({
//     @cInclude("microui/src/microui.h");
// });

fn glGetProcAddress(p: glfw.GLProc, proc: [:0]const u8) ?gl.FunctionPointer {
    _ = p;
    return glfw.getProcAddress(proc);
}

fn errorCallback(error_code: glfw.ErrorCode, description: [:0]const u8) void {
    std.log.err("glfw: {}: {s}\n", .{ error_code, description });
}

const vertex_shader = @embedFile("shaders/shader.vert");
const fragment_shader = @embedFile("shaders/shader.frag");
const compute_shader = @embedFile("shaders/shader.comp");

const present_vertices = [_]f32{
    -1.0, 1.0, 0.0, 0.0, 1.0, //noformat
    -1.0, -1.0, 0.0, 0.0, 0.0, //noformat
    1.0, 1.0, 0.0, 1.0, 1.0, //noformat
    1.0, -1.0, 0.0, 1.0, 0.0, //noformat
};

pub fn main() !void {
    var allocator = std.heap.GeneralPurposeAllocator(.{}){};
    defer std.debug.assert(allocator.deinit() == .ok);
    const gpa = allocator.allocator();

    glfw.setErrorCallback(errorCallback);
    if (!glfw.init(.{})) {
        std.log.err("failed to initialize GLFW: {?s}", .{glfw.getErrorString()});
        std.process.exit(1);
    }
    defer glfw.terminate();

    const window = glfw.Window.create(1280, 720, "Cuber", null, null, .{
        .opengl_profile = .opengl_core_profile,
        .context_version_major = 4,
        .context_version_minor = 5,
    }) orelse {
        std.log.err("failed to create GLFW window: {?s}", .{glfw.getErrorString()});
        std.process.exit(1);
    };
    defer window.destroy();

    glfw.makeContextCurrent(window);
    const proc: glfw.GLProc = undefined;
    try gl.load(proc, glGetProcAddress);

    var present = glu.Program.new_simple(gpa, vertex_shader, fragment_shader);
    var compute = glu.Program.new_compute(gpa, compute_shader);
    defer present.deinit();
    defer compute.deinit();

    var vao: u32 = undefined;
    var vbo: u32 = undefined;

    gl.createVertexArrays(1, &vao);
    defer gl.deleteVertexArrays(1, &[_]u32{vao});
    gl.createBuffers(1, &vbo);
    defer gl.deleteBuffers(1, &[_]u32{vbo});

    gl.namedBufferData(vbo, present_vertices.len * @sizeOf(f32), &present_vertices, gl.DYNAMIC_DRAW);
    gl.vertexArrayVertexBuffer(vao, 0, vbo, 0, 5 * @sizeOf(f32));

    gl.enableVertexArrayAttrib(vao, 0);
    gl.vertexArrayAttribFormat(vao, 0, 3, gl.FLOAT, gl.FALSE, 0);
    gl.vertexArrayAttribBinding(vao, 0, 0);

    gl.enableVertexArrayAttrib(vao, 1);
    gl.vertexArrayAttribFormat(vao, 1, 2, gl.FLOAT, gl.FALSE, 3 * @sizeOf(f32));
    gl.vertexArrayAttribBinding(vao, 1, 0);

    var present_texture = glu.Texture.new(1280, 720, gl.TEXTURE_2D, gl.RGBA32F, 1);
    defer present_texture.deinit();

    var camera = cam.Camera.new(m.vec3(0, 0, 0), m.vec3(0, 1, 0));
    camera.update_resolution(1280, 720);

    while (!window.shouldClose()) {
        present_texture.bind(0, 0, 0);
        present.uniform("tex", *glu.Texture, &present_texture);

        const matrix = camera.matrix();
        compute.uniform("viewMatrix", m.Mat4, matrix.view);
        compute.uniform("projectionMatrix", m.Mat4, matrix.projection);

        compute.use();
        compute.dispatch(1280, 720, 1);
        gl.memoryBarrier(gl.SHADER_IMAGE_ACCESS_BARRIER_BIT);
        compute.unuse();

        gl.clear(gl.DEPTH_BUFFER_BIT | gl.COLOR_BUFFER_BIT);

        present.use();
        gl.bindVertexArray(vao);
        gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
        gl.bindVertexArray(0);
        present.unuse();

        window.swapBuffers();
        glfw.pollEvents();
    }
}
