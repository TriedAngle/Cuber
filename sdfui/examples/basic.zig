const std = @import("std");
const sdfui = @import("sdfui");

pub fn main() !void {
    const val: i32 = sdfui.adding(10, 5);
    std.debug.print("hello world {}", .{val});
}
