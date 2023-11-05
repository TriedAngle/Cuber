const std = @import("std");

pub const Material = extern struct {
    albedo: [3]f32 = [3]f32{ 0, 0, 0 },
    _padding0: f32 = 0,
    reflectivity: f32 = 0,
    metallicity: f32 = 0,
    transparency: f32 = 0,
    emission: f32 = 0,

    pub fn hash(self: *const Material) u256 {
        var bytes: [32]u8 = undefined;
        const ptr: [*]const u8 = @ptrCast(self);
        @memcpy(&bytes, ptr);
        return @bitCast(bytes);
    }
};
