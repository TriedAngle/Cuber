const std = @import("std");

pub const Material = extern struct {
    albedo: [3]f32 = []f32{ 0, 0, 0 },
    reflectivity: f32 = 0,
    matallic: f32 = 0,
    transparency: f32 = 0,
    emission: f32 = 0,
};
