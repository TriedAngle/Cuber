const std = @import("std");
const mem = std.mem;
const heap = std.heap;
const rand = std.rand;

// testing packing/unpacking for gpu packed dda usability
pub fn main() void {
    const allocator = heap.page_allocator;
    _ = allocator;
    var xashoiro = rand.DefaultPrng.init(69420333666);
    var random = xashoiro.random();
    var data: [16]u32 = undefined;

    randomize_array(&random, &data);
    const val = data[2]; // (1 * 64 + 3 * 8 + 4)/32 = 2.875 = 2
    std.debug.print("{b}\n", .{val});
    const pack = index_3d_packed(&data, 1, 3, 4);
    std.debug.print("{b}\n", .{pack});
    const value = index_3d(&data, 1, 3, 4);
    std.debug.print("{} ", .{value});
}

fn index_3d_packed(data: *const [16]u32, x: u32, y: u32, z: u32) u32 {
    const index = x * 64 + y * 8 + z;
    const pack = data[index / 32];
    return pack;
}

fn index_3d(data: *const [16]u32, x: u32, y: u32, z: u32) bool {
    const index = x * 64 + y * 8 + z;
    const pack = data[index / 32];
    const pack_index: u5 = @intCast(index % 32);
    const mask = @as(u32, 1) << pack_index;
    const value = (pack & mask) != 0;
    return value;
}

fn randomize_array(random: *rand.Random, array: []u32) void {
    for (array) |*item| {
        item.* = random.int(u32);
    }
}
