const std = @import("std");
const sdfui = @import("sdfui");

const glfw = @import("mach-glfw");
const gl = @import("gl");

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

    const window = glfw.Window.create(640, 480, "sdfui", null, null, .{
        .opengl_profile = .opengl_core_profile,
        .context_version_major = 4,
        .context_version_minor = 6,
    }) orelse {
        std.log.err("failed to create GLFW window: {?s}", .{glfw.getErrorString()});
        std.process.exit(1);
    };
    defer window.destroy();

    glfw.makeContextCurrent(window);

    const proc: glfw.GLProc = undefined;
    try gl.load(proc, glGetProcAddress);

    var sctx = sdfui.Context.init(gpa);
    defer sctx.deinit();

    sctx.update_resolution([_]i32{ 1280, 720 });

    // var circle = sdfui.Shape{
    //     .position = [_]f32{ 3, 2 },
    //     .shape = .{ .Circle = .{ .radius = 10.0 } },
    // };
    // _ = circle;
    // var box = sdfui.Shape{
    //     .position = [_]f32{ 3, 2 },
    //     .shape = .{ .Box = .{ .width = 20, .height = 10 } },
    // };
    // _ = box;
    // const tag_circle = read_tag_value(&circle);
    // std.debug.print("circle: {}  tag: {}\n", .{ circle, tag_circle });

    // const tag_box = read_tag_value(&box);
    // std.debug.print("box: {}  tag: {}\n", .{ box, tag_box });

    while (!window.shouldClose()) {
        gl.clearColor(1, 0, 1, 1);
        gl.clear(gl.COLOR_BUFFER_BIT);

        sctx.frame();
        sctx.record();

        sctx.finish();
        sctx.render();

        sctx.time += 1;
        glfw.pollEvents();
        window.swapBuffers();
    }
}

fn read_tag_value(ptr: *const sdfui.Shape) u32 {
    const offset_ptr: *const u32 = @ptrFromInt(@intFromPtr(ptr) + @sizeOf([4]f32));
    return offset_ptr.*;
}
