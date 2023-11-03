const std = @import("std");
const sdfui = @import("sdfui");
const glfw = @import("mach-glfw");
const gl = @import("gl");
const glu = @import("glutils");

const m = @import("math");
const cam = @import("camera.zig");
const gen = @import("worldgen.zig");
const mat = @import("materials.zig");
const render = @import("render.zig");

// const DebugTexture = enum {
//     Albedo,
//     Depth,
//     Normal,

//     fn next(current: DebugTexture) DebugTexture {
//         const enumInt = @intFromEnum(current);
//         const enumCount = @typeInfo(DebugTexture).Enum.fields.len;
//         const nextInt = (enumInt + 1) % enumCount;
//         return @enumFromInt(nextInt);
//     }
// };
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

// const vertex_shader = @embedFile("shaders/shader.vert");
// const fragment_shader = @embedFile("shaders/shader.frag");
// const compute_shader = @embedFile("shaders/shader.comp");

// const present_vertices = [_]f32{
//     -1.0, 1.0, 0.0, 0.0, 1.0, //noformat
//     -1.0, -1.0, 0.0, 0.0, 0.0, //noformat
//     1.0, 1.0, 0.0, 1.0, 1.0, //noformat
//     1.0, -1.0, 0.0, 1.0, 0.0, //noformat
// };

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

    
    window.setCursorPosCallback(cursorMoveCallback);
    window.setKeyCallback(keyCallback);
    window.setInputModeCursor(.disabled);
    window.setFramebufferSizeCallback(resizeCallback);

    glfw.makeContextCurrent(window);
    const proc: glfw.GLProc = undefined;
    try gl.load(proc, glGetProcAddress);

    var camera = cam.Camera.new(m.vec3(0, 0, 0), m.vec3(0, 1, 0));
    var dtime: f32 = 1.0;

    var renderer = render.Renderer.init(gpa, .{
        .debug_texture = .Albedo,
    });
    renderer.resize(1280, 720);
    defer renderer.deinit();

    var window_data = WindowData{
        .camera = &camera,
        .renderer = &renderer,
        .dtime = dtime,
    };
    window.setUserPointer(&window_data);

    var world_gen = gen.WorldGenerator.new();
    const test_chunk = world_gen.new_random_chunk();
    renderer.add_chunk(test_chunk);

    var last_time = std.time.milliTimestamp();
    while (!window.shouldClose()) {
        const current_time = std.time.milliTimestamp();
        const delta_time = current_time - last_time;
        dtime = @floatFromInt(delta_time);
        last_time = current_time;
        processKeyboard(&window);

        renderer.update(delta_time);
        renderer.render(&camera);

        window.swapBuffers();
        glfw.pollEvents();
    }
}

const WindowData = struct {
    camera: *cam.Camera,
    renderer: *render.Renderer,
    dtime: f32,
};

fn processKeyboard(window: *glfw.Window) void {
    const maybe_data = window.getUserPointer(WindowData);
    if (maybe_data == null) {
        return;
    }
    var data = maybe_data.?;
    var dtime = data.dtime;
    dtime /= 10;
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
    // Check if the 'N' key was pressed
    if (data.renderer.config.debug_texture) |*debug_texture| {
        if (key == .n and action == .press) {
            debug_texture.* = debug_texture.next();
        }
    }
}

fn resizeCallback(window: glfw.Window, width: u32, height: u32) void {
    _ = height;
    _ = width;
    _ = window;
}
