const std = @import("std");
const mem = std.mem;
const math = std.math;

// in memory:
// u8 material_count
// <n bytes> data
// NOTE: the pointer "points directly to itself"
pub const CompressedChunk = struct {
    const Self = @This();
    material_count: u8,
    storage: *const u8,

    // NOTE: this does not allocate, lifetime CompressedChunk and memory location must be the same!
    pub fn deserialize(ptr: *const u8) Self {
        const material_count = ptr.*;
        const storage = ptr + @sizeOf(u8);
        return Self{ .material_count = material_count, .storage = storage };
    }

    pub fn serialize(self: Self, ptr: *u8) void {
        const bits = min_compression_bits(self.material_count);
        const size = compression_memory_size(bits);
        const len = size / 8;
        const dest_pointer = ptr + @sizeOf(u8);
        var dest = dest_pointer[0..len];
        var source = self.storage[0..len];
        ptr.* = bits;
        @memcpy(dest, source);
    }

    // pub fn uncompress(self: Self, allocator: mem.Allocator) UncompressedChunk {
    //     _ = allocator;
    //     _ = self;

    // }
};

// pub const UncompressedChunk = struct { 
//     voxels: [512]u8 
// };

fn compression_memory_size(bits: u8) usize {
    const bit: usize = @intCast(bits);
    return bit * 512;
}

fn min_compression_bits(count: u8) u8 {
    return @intFromFloat(@ceil(@log2(@as(f32, @floatFromInt(count)))));
}

test "bit calculations" {
    try std.testing.expect(min_compression_bits(2) == 1);
    try std.testing.expect(min_compression_bits(3) == 2);
    try std.testing.expect(min_compression_bits(4) == 2);
    try std.testing.expect(min_compression_bits(20) == 5);
    try std.testing.expect(min_compression_bits(80) == 7);
    try std.testing.expect(min_compression_bits(128) == 7);
}

test "save chunk" {}
