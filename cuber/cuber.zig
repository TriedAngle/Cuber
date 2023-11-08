const std = @import("std");
const rand = std.rand;
const sdfui = @import("sdfui");
const glfw = @import("mach-glfw");
const gl = @import("gl");
const glu = @import("glutils");

const m = @import("math");
const cam = @import("camera.zig");
const world = @import("world.zig");
const gen = @import("worldgen.zig");
const mat = @import("materials.zig");
const render = @import("render.zig");
const brick = @import("brickmap.zig");
const c = @cImport(
    @cInclude("FastNoiseLite.h"),
);

fn glGetProcAddress(p: glfw.GLProc, proc: [:0]const u8) ?gl.FunctionPointer {
    _ = p;
    return glfw.getProcAddress(proc);
}

fn errorCallback(error_code: glfw.ErrorCode, description: [:0]const u8) void {
    std.log.err("glfw: {}: {s}\n", .{ error_code, description });
}

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

    var camera = cam.Camera.new(m.vec3(200, 200, 200), m.vec3(0, 1, 0));
    var dtime: f32 = 1.0;

    var renderer = render.Renderer.init(gpa, .{
        .debug_texture = .Albedo,
        .initial_brickgrid = .{ .x = 16, .y = 16, .z = 16 },
    });
    renderer.resize(1280, 720);
    defer renderer.deinit();

    var window_data = WindowData{
        .camera = &camera,
        .renderer = &renderer,
        .dtime = dtime,
    };
    window.setUserPointer(&window_data);

    _ = renderer.add_material(&mat.Material{}); // offset by 1, empty material so indices make sense lol
    const violet_material = renderer.add_material(&mat.Material{
        .albedo = [_]f32{ 0.70, 0.08, 0.42 },
    });
    const green_material = renderer.add_material(&mat.Material{
        .albedo = [_]f32{ 0.10, 0.8, 0.22 },
    });
    const blue_material = renderer.add_material(&mat.Material{
        .albedo = [_]f32{ 0.0, 0.3, 0.9 },
    });
    const white_material = renderer.add_material(&mat.Material{
        .albedo = [_]f32{ 1.0, 1.0, 1.0 },
    });

    var palettes = brick.Palettes.init(gpa);
    defer palettes.deinit();

    const test_palette_materials = [_]u32{ violet_material, green_material, blue_material, white_material };
    const test_palette = brick.Palette.new_unchecked(gpa, &test_palette_materials);
    const test_palette_id = palettes.insert_palette(test_palette);
    _ = test_palette_id;
    renderer.resources.palette_buffer.reset(&test_palette_materials);

    var xoshiro: rand.DefaultPrng = rand.DefaultPrng.init(420);
    var random = xoshiro.random();
    _ = random;

    var noise = c.fnlCreateState();
    const testing = c.fnlGetNoise3D(&noise, 5, 6, 3);
    _ = testing;

    var world_gen = gen.WorldGenerator.new();

    var chunks = std.ArrayList(world.Chunk).init(gpa);
    var palette_chunks = std.ArrayList(brick.PaletteChunk).init(gpa);
    var brick_chunks = std.ArrayList(brick.BrickChunk).init(gpa);
    defer chunks.deinit();
    defer palette_chunks.deinit();
    defer brick_chunks.deinit();

    for (0..16) |x| {
        for (0..16) |y| {
            for (0..16) |z| {
                const chunk = world_gen.new_random_chunk(0, 4);
                chunks.append(chunk) catch unreachable;
                const palette_chunk_id = palette_chunks.items.len;
                const palette_chunk = brick.construct_palette_chunk(&chunk, 0);
                palette_chunks.append(palette_chunk) catch unreachable;
                const brick_chunk_id = brick_chunks.items.len;
                const brick_chunk = brick.construct_brick_chunk(&chunk, @intCast(palette_chunk_id));
                brick_chunks.append(brick_chunk) catch unreachable;
                renderer.grid.set_at(
                    @intCast(x),
                    @intCast(y),
                    @intCast(z),
                    brick.Brick.chunk(@intCast(brick_chunk_id)),
                );
            }
        }
    }
    renderer.resources.palette_chunk_buffer.reset(&palette_chunks.items);
    renderer.resources.chunk_buffer.reset(&brick_chunks.items);
    renderer.resources.brick_buffer.reset(&renderer.grid.bricks);

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
    if (data.renderer.config.debug_texture) |*debug_texture| {
        if (key == .n and action == .press) {
            debug_texture.* = debug_texture.next();
        }
    }
}

fn resizeCallback(window: glfw.Window, width: u32, height: u32) void {
    std.debug.print("resize: {} {}\n", .{ width, height });
    const maybe_data = window.getUserPointer(WindowData);
    if (maybe_data == null) {
        return;
    }
    var data = maybe_data.?;
    data.renderer.resize(width, height);
}
