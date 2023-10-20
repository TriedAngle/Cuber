const std = @import("std");
const mem = std.mem;
const utils = @import("utils.zig");

pub const TypeMap = struct {
    map: std.AutoHashMap(utils.TypeId, []u8),
    allocator: mem.Allocator,

    const Self = @This();

    pub fn init(allocator: mem.Allocator) Self {
        return Self{
            .map = std.AutoHashMap(utils.TypeId, []u8).init(allocator),
            .allocator = allocator,
        };
    }

    pub fn deinit(self: *Self) void {
        var iter = self.map.iterator();
        while (iter.next()) |entry| {
            self.allocator.free(entry.value_ptr.*);
        }
        self.map.deinit();
    }

    pub fn add(self: *Self, instance: anytype) void {
        const ty = @TypeOf(instance);
        const id = utils.typeId(ty);
        var bytes = self.allocator.alloc(u8, @sizeOf(ty)) catch unreachable;
        mem.copy(u8, bytes, mem.asBytes(&instance));
        self.map.put(id, bytes) catch unreachable;
    }

    pub fn remove(self: *Self, comptime T: type) void {
        const id = utils.typeId(T);
        if (self.map.get(id)) |bytes| {
            self.allocator.free(bytes);
            _ = self.map.remove(utils.typeId(T));
        }
    }

    pub fn get(self: *Self, comptime T: type) *T {
        const id = utils.typeId(T);
        if (self.map.get(id)) |bytes| {
            return @as(*T, @ptrCast(@alignCast(bytes)));
        }
        unreachable;
    }

    pub fn getConst(self: *Self, comptime T: type) T {
        return self.get(T).*;
    }

    pub fn has(self: *Self, comptime T: type) bool {
        const id = utils.typeId(T);
        return self.map.contains(id);
    }
};

test "TypeMap" {
    const Vector = struct { x: f32, y: f32, z: f32 };
    var ray = Vector{ .x = 1.0, .y = 0.0, .z = 0.0 };
    var map = TypeMap.init(std.testing.allocator);
    defer map.deinit();

    try std.testing.expect(!map.has(Vector));

    map.add(ray);
    try std.testing.expect(map.has(Vector));

    var vec = map.get(Vector);
    try std.testing.expectEqual(ray, vec.*);
    vec.z = 666;

    var vec2 = map.get(Vector);
    try std.testing.expectEqual(Vector{ .x = 1.0, .y = 0.0, .z = 666.0 }, vec2.*);

    map.remove(Vector);
    try std.testing.expect(!map.has(Vector));
}
