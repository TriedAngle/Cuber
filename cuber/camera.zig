const std = @import("std");
const m = @import("math");
const math = std.math;

const yaw: f32 = -90;
const pitch: f32 = 0.0;
const speed: f32 = 2.5;
const sensitivity: f32 = 0.1;
const zoom: f32 = 45.0;

pub const Direction = enum { Front, Back, Left, Right };
pub const Directions = std.EnumSet(Direction);

pub const Camera = struct {
    const Self = @This();

    position: m.Vec3 = m.vec3(0, 0, 0),
    front: m.Vec3 = m.vec3(0, 0, -1),
    up: m.Vec3 = m.vec3(0, 1, 0),
    right: m.Vec3 = m.vec3(1, 0, 0),
    width: f32 = 0,
    height: f32 = 0,
    world_up: m.Vec3,
    yaw: f32 = yaw,
    pitch: f32 = pitch,
    speed: f32 = speed,
    sensitivity: f32 = sensitivity,
    zoom: f32 = zoom,
    z_near: f32 = 0.1,
    z_far: f32 = 100.0,

    pub fn new(position: m.Vec3, up: m.Vec3) Self {
        var self = Camera{ .position = position, .world_up = up };
        self.update();
        return self;
    }

    pub fn matrix(self: *Self) struct { view: m.Mat4, projection: m.Mat4 } {
        return .{
            .view = m.look_at(self.position, self.position.add(self.front), self.up),
            .projection = m.perspective(self.zoom, self.width / self.height, self.z_near, self.z_far),
        };
    }

    pub fn update_direction(self: *Self, directions: Directions, time: f32) void {
        const velocity = self.speed * time;
        if (directions.contains(Direction.Front)) {
            self.position += self.front * velocity;
        }
        if (directions.contains(Direction.Front)) {
            self.position -= self.front * velocity;
        }
        if (directions.contains(Direction.Front)) {
            self.position -= self.right * velocity;
        }
        if (directions.contains(Direction.Front)) {
            self.position += self.right * velocity;
        }
    }

    pub fn update_rotation(self: *Self, x: f32, y: f32) void {
        x *= self.sensitivity;
        y *= self.sensitivity;
        self.yaw += x;
        self.pitch += y;
        if (self.pitch > 89.5) {
            self.pitch = 89.5;
        }
        if (self.pitch < -89.5) {
            self.pitch = -89.5;
        }
        self.update();
    }

    pub fn change_zoom(self: *Self, value: f32) void {
        self.zoom -= value;
    }

    pub fn update_resolution(self: *Self, width: u32, height: u32) void {
        self.width = @floatFromInt(width);
        self.height = @floatFromInt(height);
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
