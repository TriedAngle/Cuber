const std = @import("std");
const m = @import("math");
const math = std.math;

const yaw: f32 = -90;
const pitch: f32 = 0.0;
const speed: f32 = 2.5;
const sensitivity: f32 = 0.1;
const zoom: f32 = 45.0;

pub const Camera = struct {
    const Self = @This();

    position: m.Vec3 = m.vec3(0, 0, 0),
    front: m.Vec3 = m.vec3(0, 0, -1),
    up: m.Vec3 = m.vec3(0, 1, 0),
    right: m.Vec3 = m.vec3(1, 0, 0),
    world_up: m.Vec3,
    yaw: f32 = yaw,
    pitch: f32 = pitch,
    speed: f32 = speed,
    sensitivity: f32 = sensitivity,
    zoom: f32 = zoom,

    pub fn new(position: m.Vec3, up: m.Vec3) Self {
        var self = Camera{ .position = position, .world_up = up };
        self.update();
        return self;
    }

    pub fn matrix(self: *Self) m.Mat4 {
        m.Mat4.look_at(self.position, self.position.add(self.front), self.up);
    }

    pub fn change_zoom(self: *Self, value: f32) void {
        self.zoom -= value;
    }

    pub fn update(self: *Self) void {
        self.front = m.vec3(
            math.cos(math.degreesToRadians(f32, self.yaw)) * math.cos(math.degreesToRadians(f32, self.pitch)),
            math.sin(math.degreesToRadians(f32, self.pitch)),
            math.sin(math.degreesToRadians(f32, self.yaw)) * math.cos(math.degreesToRadians(f32, self.pitch)),
        ).normalize();
        self.right = self.front.cross(self.world_up).normalize();
        self.up = self.right.cross(self.front).normalize();
    }
};
