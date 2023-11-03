const std = @import("std");
const mem = std.mem;
const heap = std.heap;
const rand = std.rand;
const world = @import("world.zig");

pub const WorldGenerator = struct {
    const Self = @This();
    xoshiro: rand.DefaultPrng,

    pub fn new() Self {
        var xoshiro = rand.DefaultPrng.init(69420333666);
        return Self{
            .xoshiro = xoshiro,
        };
    }

    pub fn new_random_chunk(self: *Self) world.Chunk {
        return world.Chunk.new_random(self.xoshiro.random());
    }
};
