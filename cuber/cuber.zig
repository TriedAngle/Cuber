const std = @import("std");
const sdfui = @import("sdfui");
const glfw = @import("mach-glfw");
const gl = @import("gl");
const glu = @import("glutils");

const m = @import("math");
const cam = @import("camera.zig");
const gen = @import("worldgen.zig");
const mat = @import("materials.zig");

const DebugTexture = enum {
    Albedo,
    Depth,
    Normal,

    fn next(current: DebugTexture) DebugTexture {
        const enumInt = @intFromEnum(current);
        const enumCount = @typeInfo(DebugTexture).Enum.fields.len;
        const nextInt = (enumInt + 1) % enumCount;
        return @enumFromInt(nextInt);
    }
};
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

    var window = glfw.Window.create(1280, 720, "Cuber", null, null, .{
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

    var camera = cam.Camera.new(m.vec3(0, 0, 0), m.vec3(0, 1, 0));
    var delta_time: f32 = 1.0;
    var debug_texture: DebugTexture = .Albedo;
    var window_data = WindowData{
        .camera = &camera,
        .time = &delta_time,
        .debug_texture = &debug_texture,
    };

    window.setUserPointer(&window_data);
    window.setCursorPosCallback(cursorMoveCallback);
    window.setKeyCallback(keyCallback);
    window.setInputModeCursor(.disabled);

    var frame_idx: u32 = 0;
    var xoshiro = std.rand.DefaultPrng.init(69420666);
    var random = xoshiro.random();
    var world_gen = gen.WorldGenerator.new();
    const test_chunk = world_gen.new_random_chunk();

    var chunk_buffer = glu.Buffer.new_data(gen.Chunk, &[_]gen.Chunk{test_chunk}, gl.STATIC_DRAW);

    var albedo_texture = glu.Texture.new(1280, 720, gl.TEXTURE_2D, gl.RGBA32F, 1);
    defer albedo_texture.deinit();
    var depth_texture = glu.Texture.new(1280, 720, gl.TEXTURE_2D, gl.R16F, 1);
    defer depth_texture.deinit();
    var normal_texture = glu.Texture.new(1280, 720, gl.TEXTURE_2D, gl.RGBA16, 1);
    defer normal_texture.deinit();

    var last_time = std.time.milliTimestamp();
    while (!window.shouldClose()) {
        const current_time = std.time.milliTimestamp();
        const dtime: f32 = @floatFromInt(current_time - last_time);
        delta_time = dtime;
        last_time = current_time;
        processKeyboard(&window);
        compute.use();
        chunk_buffer.bind(0);
        albedo_texture.bind(0, 0, 0);
        depth_texture.bind(1, 0, 0);
        normal_texture.bind(2, 0, 0);
        compute.uniform("cameraPos", m.Vec3, camera.position);
        compute.uniform("cameraDir", m.Vec3, camera.front);
        compute.uniform("cameraU", m.Vec3, camera.right);
        compute.uniform("cameraV", m.Vec3, camera.up);
        compute.uniform("timer", u32, frame_idx);
        compute.uniform("randomSeed", f32, random.float(f32));
        compute.dispatch(1280, 720, 1);
        gl.memoryBarrier(gl.SHADER_IMAGE_ACCESS_BARRIER_BIT);
        compute.unuse();

        gl.clear(gl.DEPTH_BUFFER_BIT | gl.COLOR_BUFFER_BIT);

        present.use();
        var present_texture: *glu.Texture = undefined;
        switch (debug_texture) {
            .Albedo => present_texture = &albedo_texture,
            .Depth => present_texture = &depth_texture,
            .Normal => present_texture = &normal_texture,
        }
        
        present_texture.bind(0, 0, 0);
        present.uniform("tex", *glu.Texture, present_texture);
        gl.bindVertexArray(vao);
        gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
        gl.bindVertexArray(0);
        present.unuse();

        frame_idx +%= 1;
        window.swapBuffers();
        glfw.pollEvents();
    }
}

const WindowData = struct {
    camera: *cam.Camera,
    time: *f32,
    debug_texture: *DebugTexture,
};

fn processKeyboard(window: *glfw.Window) void {
    const maybe_data = window.getUserPointer(WindowData);
    if (maybe_data == null) {
        return;
    }
    var data = maybe_data.?;
    var dtime = data.time.*;
    dtime /= 100;
    if (window.getKey(.escape) == .press) {
        window.setShouldClose(true);
    }
    if (window.getKey(.w) == .press) {
        data.camera.update_direction(.Front, dtime);
    }
    if (window.getKey(.s) == .press) {
        data.camera.update_direction(.Back, dtime);
    }
    if (window.getKey(.a) == .press) {
        data.camera.update_direction(.Left, dtime);
    }
    if (window.getKey(.d) == .press) {
        data.camera.update_direction(.Right, dtime);
    }
    if (window.getKey(.space) == .press) {
        data.camera.update_direction(.Up, dtime);
    }
    if (window.getKey(.left_shift) == .press) {
        data.camera.update_direction(.Down, dtime);
    }
}

fn cursorMoveCallback(window: glfw.Window, xpos_in: f64, ypos_in: f64) void {
    const maybe_data = window.getUserPointer(WindowData);
    if (maybe_data == null) {
        return;
    }
    var data = maybe_data.?;

    const xpos: f32 = @floatCast(xpos_in);
    const ypos: f32 = @floatCast(ypos_in);
    var camera = data.camera;

    if (camera.first_enter) {
        camera.last_cursor_x = xpos;
        camera.last_cursor_y = ypos;
        camera.first_enter = false;
    }

    const xoffset = xpos - camera.last_cursor_x;
    const yoffset = camera.last_cursor_y - ypos; // reversed! y-coordinates go bottom->top in gl while window is top->bottom

    camera.last_cursor_x = xpos;
    camera.last_cursor_y = ypos;

    camera.update_rotation(xoffset, yoffset);
}

fn keyCallback(window: glfw.Window, key: glfw.Key, scancode: i32, action: glfw.Action, mods: glfw.Mods) void {
    _ = mods;
    _ = scancode;
    const maybe_data = window.getUserPointer(WindowData);
    if (maybe_data == null) {
        return;
    }
    var data = maybe_data.?;
    var dtime = data.time.*;
    dtime /= 100;
    // Check if the 'N' key was pressed
    if (key == .n and action == .press) {
        data.debug_texture.* = data.debug_texture.next();
    }
}
