const std = @import("std");
const sdfui = @import("sdfui");
const glfw = @import("mach-glfw");
const gl = @import("gl");

const shaders = @import("shaders.zig");
const textures = @import("textures.zig");

const c = @cImport({
    @cInclude("microui/src/microui.h");
});

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

fn text_width(font: c.mu_Font, text: [*c]const u8, len: i32) callconv(.C) i32 {
    _ = len;
    _ = text;
    _ = font;
    return 16;
}

fn text_height(font: c.mu_Font) callconv(.C) i32 {
    _ = font;
    return 18;
}

pub fn main() !void {
    var allocator = std.heap.GeneralPurposeAllocator(.{}){};
    defer std.debug.assert(allocator.deinit() == .ok);
    const gpa = allocator.allocator();
    _ = gpa;

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

    // var mux = gpa.create(c.mu_Context) catch unreachable;
    // c.mu_init(mux);
    // mux.text_width = text_width;
    // mux.text_height = text_height;
    // defer gpa.destroy(mux);

    const present_program = shaders.make_simple_program(vertex_shader, fragment_shader);
    const compute_program = shaders.make_compute_program(compute_shader);
    defer gl.deleteProgram(present_program);
    defer gl.deleteProgram(compute_program);

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

    var present_texture = textures.make_present(1280, 720);
    defer textures.free(present_texture);
    gl.bindImageTexture(0, present_texture, 0, gl.FALSE, 0, gl.READ_WRITE, gl.RGBA32F);

    const loc = gl.getUniformLocation(present_program, "tex");
    gl.uniform1i(loc, 0);

    while (!window.shouldClose()) {
        gl.bindTextureUnit(0, present_texture);
        gl.useProgram(compute_program);
        gl.dispatchCompute(1280, 720, 1);
        gl.memoryBarrier(gl.SHADER_IMAGE_ACCESS_BARRIER_BIT);
        gl.useProgram(0);

        gl.clear(gl.DEPTH_BUFFER_BIT | gl.COLOR_BUFFER_BIT);

        gl.useProgram(present_program);
        gl.bindVertexArray(vao);
        gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
        gl.bindVertexArray(0);
        gl.useProgram(0);

        // c.mu_begin(mux);
        // c.mu_label(mux, "First:");
        // if (c.mu_button(mux, "Button1") == 1) {
        //     std.debug.print("Button1 pressed\n", .{});
        // }

        // c.mu_end(mux);

        // if (c.mu_begin_window(mux, "My Window", c.mu_rect(10, 10, 140, 86)) == 1) {
        //     // mu_layout_row(ctx, 2, (int[]) { 60, -1 }, 0);

        //     c.mu_label(mux, "First:");
        //     if (c.mu_button(mux, "Button1") == 1) {
        //         std.debug.print("Button1 pressed\n", .{});
        //     }

        //     c.mu_label(mux, "Second:");
        //     if (c.mu_button(mux, "Button2") == 1) {
        //         c.mu_open_popup(mux, "My Popup");
        //     }

        //     if (c.mu_begin_popup(mux, "My Popup") == 1) {
        //         c.mu_label(mux, "Hello world!");
        //         c.mu_end_popup(mux);
        //     }

        //     c.mu_end_window(mux);
        // }

        window.swapBuffers();
        glfw.pollEvents();
    }
}
