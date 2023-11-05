const std = @import("std");
const mem = std.mem;
const heap = std.heap;
const rand = std.rand;

// this is basically UncompressedChunk
// NOTE: while it's used more often than not right now,
//       in the future this should be only be used temporarily,
//       instead compressed should be used.
pub const Chunk = extern struct {
    const Self = @This();
    voxels: [512]u8,

    pub fn empty() Self {
        var voxels: [512]u8 = undefined;
        @memset(&voxels, 0);
        return Self{
            .voxels = voxels,
        };
    }

    pub fn is_block(self: *Self, x: u32, y: u32, z: u32) bool {
        const index = x * 64 + y * 8 + z;
        return self.voxels[index] == 0;
    }

    pub fn set_block(self: *Self, x: u32, y: u32, z: u32, value: u8) void {
        const index = x * 64 + y * 8 + z;
        self.voxels[index] = value;
    }

    pub fn new_random(random: rand.Random, from: u8, to: u8) Self {
        var self = Self.empty();
        randomize_array(random, &self.voxels, from, to);
        return self;
    }

    pub fn most_common_material(self: *Self) u8 {
        var frequencies: [256]u8 = undefined;
        @memset(&frequencies, 0);
        for (self.voxels) |material| {
            frequencies[material] += 1;
        }

        var material: u8 = 0;
        var count: u8 = 0;

        for (frequencies, 0..) |material_frequency, mat_id| {
            if (material_frequency >= count) {
                material = mat_id;
                count = material_frequency;
            }
        }

        return material;
    }
};

fn randomize_array(random: rand.Random, array: []u8, from: u8, to: u8) void {
    for (array) |*item| {
        item.* = random.intRangeAtMost(u8, from, to);
    }
}
