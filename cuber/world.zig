const std = @import("std");
const mem = std.mem;
const heap = std.heap;
const rand = std.rand;

pub const Chunk = extern struct {
    const Self = @This();
    voxels: [16]u32,

    pub fn empty() Self {
        var voxels: [16]u32 = undefined;
        @memset(&voxels, 0);
        return Self{
            .voxels = voxels,
        };
    }

    pub fn is_block(self: *Self, x: u32, y: u32, z: u32) bool {
        const index = x * 64 + y * 8 + z;
        const pack_index: u5 = @intCast(index % 32);
        const array_index = index / 32;
        const pack = self.voxels[array_index];
        const mask = @as(u32, 1) << pack_index;
        const value = (pack & mask) != 0;
        return value;
    }

    pub fn set_block(self: *Self, x: u32, y: u32, z: u32, value: bool) void {
        const index = x * 64 + y * 8 + z;
        const pack_index: u5 = @intCast(index % 32);
        const array_index = index / 32;
        const mask = @as(u32, 1) << pack_index;

        if (value) {
            self.voxels[array_index] |= mask;
        } else {
            self.voxels[array_index] &= mask;
        }
    }

    pub fn new_random(random: rand.Random) Self {
        var self = Self.empty();
        randomize_array(random, &self.voxels);
        return self;
    }
};

fn randomize_array(random: rand.Random, array: []u32) void {
    for (array) |*item| {
        item.* = random.int(u32);
    }
}
