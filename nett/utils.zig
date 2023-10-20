const std = @import("std");
const mem = std.mem;

pub const TypeId = extern struct {
    value: u32,

    pub fn equals(self: TypeId, other: TypeId) bool {
        return self.value == other.value;
    }
};

// Fowler–Noll–Vo hash
pub export fn hash_type(name: [*:0]const u8) TypeId {
    const prime: u32 = 16777619;
    var hash: u32 = 2166136261;

    var index: usize = 0;
    while (name[index] != 0) : (index += 1) {
        hash *%= prime;
        hash ^= @as(u32, @intCast(name[index]));
    }

    return TypeId{
        .value = hash,
    };
}

pub fn typeId(comptime T: type) TypeId {
    const name = @typeName(T);
    return hash_type(name);
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
