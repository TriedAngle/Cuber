const std = @import("std");
const m = @import("math");
const math = std.math;

const yaw: f32 = -90;
const pitch: f32 = 0.0;
const speed: f32 = 2.5;
const sensitivity: f32 = 0.1;
const zoom: f32 = 45.0;

pub const Direction = enum { Front, Back, Left, Right, Up, Down };
pub const Directions = std.EnumSet(Direction);

pub const Camera = struct {
    const Self = @This();

    position: m.Vec3 = m.vec3(0, 0, 0),
    front: m.Vec3 = m.vec3(1, 0, 0),
    up: m.Vec3 = m.vec3(0, 1, 0),
    right: m.Vec3 = m.vec3(1, 0, 0),
    width: f32 = 0,
    height: f32 = 0,
    world_up: m.Vec3,
    yaw: f32 = yaw,
    pitch: f32 = pitch,
    speed: f32 = speed,
    last_cursor_x: f32 = 0,
    last_cursor_y: f32 = 0,
    first_enter: bool = true,
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

    // pub fn info(self: *Self) struct { pos: m.Vec3, dir: m.Vec3, right: m.Vec3, up: m.Vec3 } {
    //     return .{
    //         .pos = self.position,
    //         .dir = self.front,
    //         .
    //     };
    // }

    pub fn update_direction(self: *Self, direction: Direction, time: f32) void {
        const velocity = self.speed * time;
        if (direction == .Front) {
            self.position = self.position.add(self.front.scale(velocity));
        }
        if (direction == .Back) {
            self.position = self.position.sub(self.front.scale(velocity));
        }
        if (direction == .Left) {
            self.position = self.position.sub(self.right.scale(velocity));
        }
        if (direction == .Right) {
            self.position = self.position.add(self.right.scale(velocity));
        }
        if (direction == .Up) {
            self.position = self.position.add(self.up.scale(velocity));
        }
        if (direction == .Down) {
            self.position = self.position.sub(self.up.scale(velocity));
        }
    }

    pub fn update_rotation(self: *Self, x_in: f32, y_in: f32) void {
        const x = x_in * self.sensitivity;
        const y = y_in * self.sensitivity;
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
