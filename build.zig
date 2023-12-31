const std = @import("std");
const builtin = std.builtin;
const Builder = std.build.Builder;

// libraries
const opengl_path = "libs/gl.zig";

// modules
const sdfui_path = "sdfui/sdfui.zig";
const cuber_path = "cuber/cuber.zig";
const glutils_path = "glutils/glutils.zig";
const math_path = "math/math.zig";

// examples
const sdfui_samples = [_][2][]const u8{
    [_][]const u8{ "basic", "samples/sdfui/basic.zig" },
};

// tests
const sdfui_tests = "sdfui/tests.zig";

pub fn build(b: *Builder) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const glfw = b.dependency("mach_glfw", .{
        .target = target,
        .optimize = optimize,
    });

    const opengl = b.addModule("gl", .{
        .source_file = .{ .path = opengl_path },
    });

    const math = b.addModule("math", .{
        .source_file = .{ .path = math_path },
    });

    const glutils = b.addModule("glutils", .{
        .source_file = .{ .path = glutils_path },
        .dependencies = &.{
            .{ .name = "gl", .module = opengl },
            .{ .name = "math", .module = math },
        },
    });

    const sdfui = b.addModule("sdfui", .{
        .source_file = .{ .path = sdfui_path },
        .dependencies = &.{
            .{ .name = "gl", .module = opengl },
            .{ .name = "glutils", .module = glutils },
            .{ .name = "math", .module = math },
        },
    });

    build_sdfui(b, optimize, target, sdfui, opengl, glfw);

    build_cuber(b, optimize, target, sdfui, opengl, glfw, math, glutils);
}

pub fn executable_name(b: *Builder, module: []const u8, name: []const u8) []const u8 {
    return b.fmt("{s}-{s}", .{ module, name });
}

pub fn build_sdfui(
    b: *Builder,
    optimize: builtin.OptimizeMode,
    target: std.zig.CrossTarget,
    sdfui: *std.Build.Module,
    opengl: *std.Build.Module,
    glfw: *std.Build.Dependency,
) void {
    for (sdfui_samples) |example| {
        const name = example[0];
        const source = example[1];
        var exe = b.addExecutable(.{
            .name = executable_name(b, "sdfui", name),
            .root_source_file = .{ .path = source },
            .optimize = optimize,
        });
        exe.linkLibC();
        exe.addModule("sdfui", sdfui);
        exe.addModule("mach-glfw", glfw.module("mach-glfw"));
        @import("mach_glfw").link(glfw.builder, exe);

        exe.addModule("gl", opengl);

        const docs = exe;
        const doc = b.step(
            b.fmt("{s}-docs", .{name}),
            "Generate documentation",
        );
        doc.dependOn(&docs.step);

        const run_cmd = b.addRunArtifact(exe);
        b.installArtifact(exe);
        const exe_step = b.step(
            executable_name(b, "sdfui", name),
            b.fmt("run {s}.zig", .{name}),
        );
        exe_step.dependOn(&run_cmd.step);
    }

    const tests = b.addTest(.{
        .root_source_file = std.build.FileSource{ .path = sdfui_tests },
        .optimize = optimize,
        .target = target,
        .name = "tests",
    });
    tests.addModule("sdfui", sdfui);
    b.installArtifact(tests);
    const test_cmd = b.step(
        executable_name(b, "sdfui", "tests"),
        "Run the tests",
    );
    test_cmd.dependOn(b.getInstallStep());
    test_cmd.dependOn(&b.addRunArtifact(tests).step);
}

pub fn build_cuber(
    b: *Builder,
    optimize: builtin.OptimizeMode,
    target: std.zig.CrossTarget,
    sdfui: *std.Build.Module,
    opengl: *std.Build.Module,
    glfw: *std.Build.Dependency,
    math: *std.Build.Module,
    glutils: *std.Build.Module,
) void {
    var cflags = [_][]const u8{ "-std=c99", "-fno-sanitize=undefined", "-g", "-O3" };

    var cfiles = [_][]const u8{
        "libs/FastNoiseLite/FastNoiseLite.c",
    };

    var exe = b.addExecutable(.{
        .name = "cuber",
        .root_source_file = .{ .path = cuber_path },
        .optimize = optimize,
        .target = target,
    });
    exe.linkLibC();
    exe.addModule("sdfui", sdfui);
    exe.addModule("mach-glfw", glfw.module("mach-glfw"));
    @import("mach_glfw").link(glfw.builder, exe);
    exe.addModule("gl", opengl);
    exe.addModule("math", math);
    exe.addModule("glutils", glutils);


    const ZigImGui = b.dependency("ZigImGui", .{
        .target = target,
        .optimize = optimize,
        .enable_freetype = true,
        .enable_lunasvg = false,
        .enable_opengl = true,
    });

    exe.addIncludePath(ZigImGui.path("zig-imgui/vendor/cimgui/imgui/"));
    exe.addIncludePath(.{ .path = "libs/imgui/imgui_impl_glfw.h" });

    exe.addCSourceFile(.{
        .file = .{ .path = "libs/imgui/imgui_impl_glfw.cpp" },
        .flags = &.{
            "-std=c++11",
            "-fno-sanitize=undefined",
            "-fvisibility=hidden",
            "-O3",
        },
    });

    exe.addModule("Zig-ImGui", ZigImGui.module("Zig-ImGui"));
    exe.linkLibrary(ZigImGui.artifact("cimgui"));

    exe.addIncludePath(.{ .path = "libs/" });
    exe.addIncludePath(.{ .path = "libs/FastNoiseLite" });
    exe.addCSourceFiles(.{
        .files = &cfiles,
        .flags = &cflags,
    });

    const docs = exe;
    const doc = b.step(
        "cuber-docs",
        "Generate documentation",
    );
    doc.dependOn(&docs.step);

    const run_cmd = b.addRunArtifact(exe);
    b.installArtifact(exe);
    const exe_step = b.step(
        "cuber",
        "run cuber",
    );
    exe_step.dependOn(&run_cmd.step);
}
