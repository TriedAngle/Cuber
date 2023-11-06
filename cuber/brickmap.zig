const std = @import("std");
const mem = std.mem;
const mat = @import("materials.zig");
const world = @import("world.zig");
const utils = @import("utils.zig");

// TODO: maybe consider not doing this indirection here
// NOTE: ids sorted from low to high, no duplicates!
// NOTE: this struct is not uploaded to gpu directly but only it's contents!
pub const Palette = struct {
    material_ids: []u32,

    pub fn new(allocator: mem.Allocator, materials: []const u32) Palette {
        var ids = utils.dedupsort(allocator, materials) catch unreachable;
        return Palette{ .material_ids = ids };
    }

    // prefer to use this function, other places should have it safeguarded tbh.
    pub fn new_unchecked(allocator: mem.Allocator, materials: []const u32) Palette {
        var ids = allocator.alloc(u32, materials.len) catch unreachable;
        @memcpy(ids, materials);
        return Palette{ .material_ids = ids };
    }

    pub fn free(self: Palette, allocator: mem.Allocator) void {
        allocator.free(self.material_ids);
    }

    // this is and storage is the reason for the unique + sort is required.
    // TODO: investigate if this is actualy doing something this way lol
    pub fn eql(self: Palette, other: Palette) bool {
        return std.mem.eql(u32, self.material_ids, other.material_ids);
    }
};

// TODO: investigate if hashmap would work, I'm too lazy to test if it could use the `eql` function
pub const Palettes = struct {
    const Self = @This();
    allocator: mem.Allocator,
    palettes: std.ArrayList(Palette),

    pub fn init(allocator: mem.Allocator) Self {
        const palettes = std.ArrayList(Palette).init(allocator);
        return Self{
            .allocator = allocator,
            .palettes = palettes,
        };
    }

    pub fn insert_palette(self: *Self, new: Palette) u32 {
        for (self.palettes.items, 0..) |palette, id| {
            if (palette.eql(new)) {
                return @intCast(id);
            }
        }
        const id: u32 = @intCast(self.palettes.items.len);
        self.palettes.append(new) catch unreachable;
        return id;
    }

    pub fn deinit(self: *Self) void {
        for (self.palettes.items) |palette| {
            palette.free(self.allocator);
        }
        self.palettes.deinit();
    }
};

// TODO: due to how palettes work, it can be compressed to only require ceil(log2(size)) bits per voxel.
// this wold also scale to 9bits in case of more than 255 colors per chunk.
// to do this instead of indexing, offsetting would need to be used.
// an additional issue is spacial reuse, but I think we already know the sizes this could take
// so a simple free-list tracking on the cpu side could hopefully do the job.
pub const PaletteChunk = extern struct {
    material_mappings: [512]u8,
    palette: u32,
    lod_material: u32 = 0, // TODO: implement this
};

pub const BrickChunk = extern struct {
    voxels: [16]u32,
    palette_chunk: u32,
    lod: u32 = 0, // TODO: implement this
};

pub fn construct_palette_chunk(chunk: *const world.Chunk, palette_id: u32) PaletteChunk {
    return PaletteChunk{
        .palette = palette_id,
        .material_mappings = chunk.voxels,
    };
}

pub fn construct_brick_chunk(chunk: *const world.Chunk, palette_chunk: u32) BrickChunk {
    var result: BrickChunk = undefined;
    result.palette_chunk = palette_chunk;
    result.voxels = [_]u32{0} ** 16;
    @memset(&result.voxels, 0);
    for (chunk.voxels, 0..) |voxel, i| {
        if (voxel != 0) {
            const index = i / 32;
            const bitIndex = i % 32;
            result.voxels[index] |= @as(u32, 1) << @intCast(bitIndex);
        }
    }
    return result;
}
