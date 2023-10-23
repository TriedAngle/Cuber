const std = @import("std");
const mem = std.mem;
const gl = @import("gl");

const vertex_shader = @embedFile("shaders/shader.vert");
const fragment_shader = @embedFile("shaders/shader.frag");
const compute_shader = @embedFile("shaders/shader.comp");

pub const Circle = struct { radius: f32, padding: [3]f32 = [3]f32{ 0, 0, 0 } };

pub const Box = struct { width: f32, height: f32, padding: 2[f32] = [2]f32{ 0, 0 } };

pub const Shape = struct {
    position: [2]f32,
    angle: f32 = 0.0,
    shape: union(enum(u32)) {
        Circle: Circle,
        Box: Box,
    },
};

pub const Command = struct {
    kind: i32,
    idx: i32,
    fun: i32,
};

pub const GLBuffers = struct {
    const Self = @This();
    commands: u32,
    shapes: u32,

    pub fn init() Self {
        var buffers: [2]u32 = undefined;
        gl.createBuffers(2, &buffers);
        return Self{
            .commands = buffers[0],
            .shapes = buffers[1],
        };
    }

    pub fn deinit(self: *Self) void {
        const buffers = [2]u32{ self.commands, self.shapes };
        gl.deleteBuffers(2, &buffers);
    }

    pub fn bind(self: *Self) void {
        gl.bindBufferBase(gl.SHADER_STORAGE_BUFFER, 0, self.commands);
        gl.bindBufferBase(gl.SHADER_STORAGE_BUFFER, 0, self.shapes);
    }
};

pub const Recorder = struct {
    const Self = @This();
    allocator: mem.Allocator,
    commands: std.ArrayList(Command),
    shapes: std.ArrayList(Shape),

    pub fn init(allocator: mem.Allocator) Self {
        const commands = std.ArrayList(Command).init(allocator);
        const shapes = std.ArrayList(Shape).init(allocator);
        return Self{
            .allocator = allocator,
            .commands = commands,
            .shapes = shapes,
        };
    }

    pub fn deinit(self: *Self) void {
        self.commands.deinit();
        self.shapes.deinit();
    }

    pub fn add(
        self: *Self,
    ) void {
        _ = self;
    }
};

pub const Context = struct {
    const Self = @This();
    allocator: mem.Allocator,
    buffers: GLBuffers,
    last_records: std.ArrayList(Recorder),
    recorders: std.ArrayList(Recorder),
    active: ?Recorder = null,
    record_counter: usize = 0,

    pub fn init(allocator: mem.Allocator) Self {
        const buffers = GLBuffers.init();
        const recorders = std.ArrayList(Recorder).init(allocator);
        return Self{
            .buffers = buffers,
            .allocator = allocator,
            .last_records = recorders.clone() catch unreachable,
            .recorders = recorders,
        };
    }

    pub fn deinit(self: *Self) void {
        for (self.last_records.items) |*item| {
            item.deinit();
        }
        self.last_records.deinit();
        for (self.recorders.items) |*item| {
            item.deinit();
        }
        self.recorders.deinit();

        if (self.active) |*item| {
            item.deinit();
        }

        self.buffers.deinit();
    }

    pub fn frame(self: *Self) void {
        for (self.last_records.items) |*item| {
            item.deinit();
        }
        self.last_records.clearRetainingCapacity();
        const tmp = self.last_records;
        self.last_records = self.recorders;
        self.recorders = tmp;
        self.record_counter = 0;
    }

    pub fn render(self: *Self) void {
        _ = self;
    }

    pub fn record(self: *Self) void {
        self.active = Recorder.init(self.allocator);
    }

    pub fn finish(self: *Self) void {
        self.recorders.append(self.active.?);
        self.active = null;
        self.record_counter +%= 1;
    }

    pub fn record_shape(self: *Self, shape: Shape) void {
        _ = shape;
        _ = self;
    }
};
