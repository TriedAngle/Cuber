const std = @import("std");

const ZigImGuiBuild = @import("ZigImGui");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const glfw = b.dependency("mach_glfw", .{
        .target = target,
        .optimize = optimize,
    });

    const glfw_dep = glfw.builder.dependency("glfw", .{
        .target = target,
        .optimize = optimize,
    });

    const gl_headers = b.dependency("opengl_headers", .{
        .target = target,
        .optimize = optimize,
    });

    const gl = b.addModule("gl", .{
        .root_source_file = .{ .path = "libs/gl.zig" },
    });

    const math = b.addModule("math", .{
        .root_source_file = .{ .path = "math/math.zig" },
    });

    const glutils = b.addModule("glutils", .{
        .root_source_file = .{ .path = "glutils/glutils.zig" },
    });

    glutils.addImport("gl", gl);
    glutils.addImport("math", math);

    const sdfui = b.addModule("sdfui", .{ .root_source_file = .{
        .path = "sdfui/sdfui.zig",
    } });

    sdfui.addImport("math", math);
    sdfui.addImport("gl", gl);
    sdfui.addImport("glutils", glutils);

    const fastnoise = create_fastnoise_lib(b, target, optimize);

    const ZigImGui = b.dependency("ZigImGui", .{
        .target = target,
        .optimize = optimize,
        .enable_opengl = true,
        .enable_freetype = false,
        .enable_lunasvg = false,
    });

    const imgui_dep = ZigImGui.builder.dependency("imgui", .{
        .target = target,
        .optimize = optimize,
    });

    const imgui_glfw = create_imgui_glfw_lib(
        b,
        target,
        optimize,
        glfw_dep,
        gl_headers,
        imgui_dep,
        ZigImGui,
    );

    const imgui_opengl = create_imgui_opengl_lib(
        b,
        target,
        optimize,
        imgui_dep,
        ZigImGui,
    );

    build_cuber(
        b,
        target,
        optimize,
        sdfui,
        gl,
        glutils,
        math,
        glfw,
        ZigImGui,
        imgui_glfw,
        imgui_opengl,
        fastnoise,
    );

    build_sdfui_samples(
        b,
        target,
        optimize,
        sdfui,
        gl,
        glfw,
    );
}

fn build_cuber(
    b: *std.Build,
    target: std.Build.ResolvedTarget,
    optimize: std.builtin.OptimizeMode,
    sdfui: *std.Build.Module,
    gl: *std.Build.Module,
    glutils: *std.Build.Module,
    math: *std.Build.Module,
    glfw: *std.Build.Dependency,
    ZigImGui: *std.Build.Dependency,
    imgui_glfw: *std.Build.Step.Compile,
    imgui_opengl: *std.Build.Step.Compile,
    fastnoise: *std.Build.Step.Compile,
) void {
    const exe = b.addExecutable(.{
        .name = "cuber",
        .root_source_file = .{ .path = "cuber/cuber.zig" },
        .target = target,
        .optimize = optimize,
    });
    exe.linkLibC();
    exe.root_module.addImport("sdfui", sdfui);
    exe.root_module.addImport("gl", gl);
    exe.root_module.addImport("glutils", glutils);
    exe.root_module.addImport("math", math);

    exe.root_module.addImport("mach-glfw", glfw.module("mach-glfw"));
    @import("mach_glfw").addPaths(exe);

    exe.root_module.addImport("Zig-ImGui", ZigImGui.module("Zig-ImGui"));
    exe.linkLibrary(imgui_glfw);
    exe.linkLibrary(imgui_opengl);

    exe.linkLibrary(fastnoise);

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

fn build_sdfui_samples(
    b: *std.Build,
    target: std.Build.ResolvedTarget,
    optimize: std.builtin.OptimizeMode,
    sdfui: *std.Build.Module,
    gl: *std.Build.Module,
    glfw: *std.Build.Dependency,
) void {
    const samples = [_][2][]const u8{
        [_][]const u8{ "basic", "samples/sdfui/basic.zig" },
    };
    for (samples) |sample| {
        const name = sample[0];
        const source = sample[1];

        var exe = b.addExecutable(.{
            .name = b.fmt("sdfui-{s}", .{name}),
            .root_source_file = .{ .path = source },
            .target = target,
            .optimize = optimize,
        });
        exe.linkLibC();
        exe.root_module.addImport("sdfui", sdfui);
        exe.root_module.addImport("gl", gl);
        exe.root_module.addImport("mach-glfw", glfw.module("mach-glfw"));
        @import("mach_glfw").addPaths(exe);

        b.installArtifact(exe);

        const run_cmd = b.addRunArtifact(exe);

        if (b.args) |args| {
            run_cmd.addArgs(args);
        }

        const run_step = b.step(
            b.fmt("sdfui-{s}", .{name}),
            "run sdfui sample",
        );
        run_step.dependOn(&run_cmd.step);
    }

    const tests = b.addTest(.{
        .name = "sdfui-tests",
        .root_source_file = .{ .path = "sdfui/tests.zig" },
        .target = target,
        .optimize = optimize,
    });
    tests.root_module.addImport("sdfui", sdfui);

    const run_tests = b.addRunArtifact(tests);

    const test_cmd = b.step("sdfui-tests", "Run sdfui tests");
    test_cmd.dependOn(b.getInstallStep());
    test_cmd.dependOn(&run_tests.step);
}

fn create_imgui_glfw_lib(
    b: *std.Build,
    target: std.Build.ResolvedTarget,
    optimize: std.builtin.OptimizeMode,
    glfw: *std.Build.Dependency,
    gl_headers: *std.Build.Dependency,
    imgui: *std.Build.Dependency,
    ZigImGui: *std.Build.Dependency,
) *std.Build.Step.Compile {
    const imgui_glfw = b.addStaticLibrary(.{
        .name = "imgui_glfw",
        .target = target,
        .optimize = optimize,
    });
    imgui_glfw.linkLibCpp();
    imgui_glfw.linkLibrary(ZigImGui.artifact("cimgui"));

    for (ZigImGuiBuild.IMGUI_C_DEFINES) |c_define| {
        imgui_glfw.root_module.addCMacro(c_define[0], c_define[1]);
    }

    imgui_glfw.addIncludePath(imgui.path("."));
    imgui_glfw.addIncludePath(imgui.path("backends/"));

    imgui_glfw.addIncludePath(gl_headers.path("."));
    imgui_glfw.addIncludePath(glfw.path("include/"));

    imgui_glfw.addCSourceFile(.{
        .file = imgui.path("backends/imgui_impl_glfw.cpp"),
        .flags = ZigImGuiBuild.IMGUI_C_FLAGS,
    });

    return imgui_glfw;
}

fn create_imgui_opengl_lib(
    b: *std.Build,
    target: std.Build.ResolvedTarget,
    optimize: std.builtin.OptimizeMode,
    imgui: *std.Build.Dependency,
    ZigImGui: *std.Build.Dependency,
) *std.Build.Step.Compile {
    const imgui_opengl = b.addStaticLibrary(.{
        .name = "imgui_opengl",
        .target = target,
        .optimize = optimize,
    });
    imgui_opengl.linkLibCpp();
    imgui_opengl.linkLibrary(ZigImGui.artifact("cimgui"));

    for (ZigImGuiBuild.IMGUI_C_DEFINES) |c_define| {
        imgui_opengl.root_module.addCMacro(c_define[0], c_define[1]);
    }

    imgui_opengl.addIncludePath(imgui.path("."));
    imgui_opengl.addIncludePath(imgui.path("backends/"));

    imgui_opengl.addCSourceFile(.{
        .file = imgui.path("backends/imgui_impl_opengl3.cpp"),
        .flags = ZigImGuiBuild.IMGUI_C_FLAGS,
    });

    return imgui_opengl;
}

fn create_fastnoise_lib(
    b: *std.Build,
    target: std.Build.ResolvedTarget,
    optimize: std.builtin.OptimizeMode,
) *std.Build.Step.Compile {
    const fastnoise = b.addStaticLibrary(.{
        .name = "fastnoise",
        .target = target,
        .optimize = optimize,
    });
    fastnoise.linkLibC();

    fastnoise.addIncludePath(.{ .path = "libs/FastNoiseLite" });
    fastnoise.addCSourceFiles(.{
        .files = &[_][]const u8{"libs/FastNoiseLite/FastNoiseLite.c"},
        .flags = &[_][]const u8{ "-std=c99", "-fno-sanitize=undefined", "-g", "-O3" },
    });

    return fastnoise;
}
