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
    noise: ns.fnl_state,

    pub fn new(allocator: mem.Allocator) Self {
        const xoshiro = rand.DefaultPrng.init(69420333666);
        var noise = ns.fnlCreateState();
        noise.noise_type = ns.FNL_NOISE_OPENSIMPLEX2;

        return Self{
            .allocator = allocator,
            .xoshiro = xoshiro,
            .noise = noise,
        };
    }

    // uses "grid space" not actual coordinates, grid space is coordinate space / 8
    pub fn generate_volume(
        self: *Self,
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
        _ = self;
    }

    pub fn generate_chunk(
        self: *Self,
        x: u32,
        y: u32,
        z: u32,
    ) world.Chunk {
        var chunk = world.Chunk.empty();
        generate_heightmap_pass(self, &chunk, x, y, z);
        return chunk;
    }

    fn generate_heightmap_pass(self: *Self, chunk: *world.Chunk, x: u32, y: u32, z: u32) void {
        for (0..8) |chunk_x| {
            for (0..8) |chunk_y| {
                for (0..8) |chunk_z| {
                    const cx: u32 = @intCast(chunk_x);
                    const cy: u32 = @intCast(chunk_y);
                    const cz: u32 = @intCast(chunk_z);

                    const block = heightmap_function(
                        &self.noise,
                        x * 8 + cx,
                        y * 8 + cy,
                        z * 8 + cz,
                    );
                    const value: u8 = @intCast(block);
                    chunk.set_block(cx, cy, cz, value);
                }
            }
        }
    }

    fn heightmap_function(noise: *ns.fnl_state, x: u32, y: u32, z: u32) u32 {
        const y_noise = 100.0 + ns.fnlGetNoise2D(noise, @floatFromInt(x), @floatFromInt(z)) * 40.0;
        const height: u32 = @intFromFloat(y_noise);
        if (y < height) {
            return 1;
        } else {
            return 0;
        }
    }

    pub fn new_random_chunk(self: *Self, from: u8, to: u8) world.Chunk {
        return world.Chunk.new_random(self.xoshiro.random(), from, to);
    }
};

const DefaultGeneratorSate = struct {
    noise: ns.fnl_state,
};
