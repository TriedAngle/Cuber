const std = @import("std");

pub const TypeId = struct {
    value: u32,

    pub fn equals(self: TypeId, other: TypeId) bool {
        return self.value == other.value;
    }
};

// Fowler–Noll–Vo hash
pub fn typeId(comptime T: type) TypeId {
    const name = @typeName(T);
    const prime: u32 = 16777619;
    var hash: u32 = 2166136261;

    for (name) |c| {
        hash *%= prime;
        hash ^= @as(u32, @intCast(c));
    }

    return TypeId{
        .value = hash,
    };
}

test "typeid" {
    const Vector = struct { x: f32, y: f32, z: f32 };
    const t1 = typeId(u32);
    const t2 = typeId(u64);
    const t3 = typeId(TypeId);
    const t4 = typeId([4]Vector);
    try std.testing.expect(!t1.equals(t2));
    try std.testing.expect(!t2.equals(t3));
    try std.testing.expect(!t3.equals(t4));
}
