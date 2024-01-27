const std = @import("std");
const mem = std.mem;
const heap = std.heap;
const rand = std.rand;
const world = @import("world.zig");
const ns = @import("fastnoise.zig");
const brick = @import("brickmap.zig");

pub const Generation = struct {
    chunks: []brick.Chunk,
    palette_chunks: []brick.PaletteChunk,
};

pub const WorldGenerator = struct {
    const Self = @This();
    allocator: mem.Allocator,
    xoshiro: rand.DefaultPrng,

    pub fn new(allocator: mem.Allocator) Self {
        const xoshiro = rand.DefaultPrng.init(69420333666);
        return Self{
            .allocator = allocator,
            .xoshiro = xoshiro,
        };
    }

    pub fn generate_default_volume(self: *Self, x: u32, y: u32, z: u32, x_max: u32, y_max: u32, z_max: u32) void {

        var noise = ns.fnlCreateState();
        noise.noise_type = ns.FNL_NOISE_OPENSIMPLEX2S;
        noise.octaves = 4;
        noise.lacunarity = 2.0;
        noise.gain = 0.5;

        var state = DefaultGeneratorSate{
            .noise = noise,
        };

        const result = self.generate_volume(&state, default_generator, x, y, z, x_max, y_max, z_max);
        _ = result;

    }

    pub fn generate_volume(
        self: *Self,
        state: *anyopaque,
        generator: fn (data: *anyopaque, x: u32, y: u32, z: u32) u8,
        x: u32,
        y: u32,
        z: u32,
        x_max: u32,
        y_max: u32,
        z_max: u32,
    ) void {

        const width = x_max - x;
        const height = y_max - y;
        const depth = z_max - z;
        const volume = width * height * depth;
        _ = volume;
        _ = generator;
        _ = state;
        _ = self;
    }

    pub fn generate_chunk(
        self: *Self,
        state: *anyopaque,
        generator: fn (data: *anyopaque, x: u32, y: u32, z: u32) u8,
        x: u32,
        y: u32,
        z: u32,
    ) void {
        _ = generator;
        _ = state;
        _ = z;
        _ = y;
        _ = x;
        _ = self;
    }

    pub fn new_random_chunk(self: *Self, from: u8, to: u8) world.Chunk {
        return world.Chunk.new_random(self.xoshiro.random(), from, to);
    }
};

const DefaultGeneratorSate = struct {
    noise: ns.fnl_state,
};

fn default_generator(data: *anyopaque, x: u32, y: u32, z: u32) u8 {
    var state: DefaultGeneratorSate = @ptrCast(data);
    const noise = &state.noise;
    const heightmap = default_heightmap(noise, x, y, z);
    return heightmap;
}

fn default_heightmap(noise: *ns.fnl_state, x: u32, y: u32, z: u32) u8 {
    const y_noise = 100.0 + ns.fnlGetNoise2D(noise, x, z) * 30.0;
    const height: u32 = @intFromFloat(y_noise);
    if (y < height) {
        return 1;
    } else {
        return 0;
    }
}
