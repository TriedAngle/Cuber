const std = @import("std");
const Builder = std.build.Builder;

pub fn build(b: *Builder) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const sdfui_module = b.addModule("sdfui", .{
        .source_file = std.build.FileSource{ .path = "src/sdfui.zig" },
    });

    const examples = [_][2][]const u8{
        [_][]const u8{ "basic", "examples/basic.zig" },
    };

    for (examples, 0..) |example, i| {
        const name = example[0];
        const source = example[1];
        var exe = b.addExecutable(.{
            .name = name,
            .root_source_file = std.build.FileSource{ .path = source },
            .optimize = optimize,
        });
        exe.addModule("sdfui", sdfui_module);
        exe.linkLibC();

        const docs = exe;
        const doc = b.step(b.fmt("{s}-docs", .{name}), "Generate documentation");
        doc.dependOn(&docs.step);

        const run_cmd = b.addRunArtifact(exe);
        const exe_step = b.step(name, b.fmt("run {s}.zig", .{name}));
        exe_step.dependOn(&run_cmd.step);
        if (i == 0) {
            const run_exe_step = b.step("run", b.fmt("run {s}.zig", .{name}));
            run_exe_step.dependOn(&run_cmd.step);
        }
    }

    const tests = b.addTest(.{ .root_source_file = std.build.FileSource{ .path = "src/tests.zig" }, .optimize = optimize, .target = target, .name = "tests" });
    tests.addModule("sdfui", sdfui_module);
    b.installArtifact(tests);

    const test_cmd = b.step("test", "Run the tests");
    test_cmd.dependOn(b.getInstallStep());
    test_cmd.dependOn(&b.addRunArtifact(tests).step);
}
