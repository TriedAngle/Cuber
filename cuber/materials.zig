const std = @import("std");

pub const Material = extern struct {
    albedo: [3]f32 = [3]f32{ 0, 0, 0 },
    _padding0: f32 = 0,
    reflectivity: f32 = 0,
    metallicity: f32 = 0,
    transparency: f32 = 0,
    emission: f32 = 0,
};
