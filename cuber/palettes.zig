const std = @import("std");
const mem = std.mem;

pub const PaletteTrie = struct {
    const Self = @This();
    allocator: mem.Allocator,
    root: Node,

    pub fn init(allocator: mem.Allocator) Self {
        return .{
            .allocator = allocator,
            .root = .{ .id = null, .children = null },
        };
    }

    pub fn deinit(self: *Self) void {
        if (self.root.children) |children| {
            for (children) |*child| {
                self.deinit_recursive(child);
            }
        }
    }

    fn find_material(node: *Node, material: u32) ?u32 {
        _ = material;
        _ = node;
        if (node.children?)    
    }

    pub fn palette_lookup(self: *Self, materials: []const u32) void {
        _ = materials;
        var current = self.root;
        _ = current;

    }

    fn deinit_recursive(self: *Self, node: *Node) void {
        if (node.children) |children| {
            for (children) |*child| {
                self.deinit_recursive(child);
            }
            self.allocator.free(children);
        }
    }
};

const Node = struct {
    material_id: u32 = 0,
    palette: ?u32,
    children: ?[]Node,
};
