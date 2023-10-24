const std = @import("std");
const mem = std.mem;
const gl = @import("gl");
const glu = @import("glutils");
const textures = glu.textures;
const shaders = glu.shaders;

const vertex_shader = @embedFile("shaders/shader.vert");
const fragment_shader = @embedFile("shaders/shader.frag");
const compute_shader = @embedFile("shaders/shader.comp");

const present_vertices = [_]f32{
    -1.0, 1.0, 0.0, 0.0, 1.0, //noformat
    -1.0, -1.0, 0.0, 0.0, 0.0, //noformat
    1.0, 1.0, 0.0, 1.0, 1.0, //noformat
    1.0, -1.0, 0.0, 1.0, 0.0, //noformat
};

pub const Circle = struct { radius: f32, padding: [3]f32 = [3]f32{ 0, 0, 0 } };

pub const Box = struct { width: f32, height: f32, padding: [2]f32 = [2]f32{ 0, 0 } };

pub const Shape = struct {
    position: [2]f32,
    padding: u32 = 0,
    shape: union(enum(u32)) {
        Circle: Circle,
        Box: Box,
    },
};

pub const Material = struct {
    kind: u32,
    data: [3]f32,
    color: [4]f32,
};

pub const SDFOp = struct {
    kind: i32,
    padding: i32,
    dat: [2]f32,
    data: [4]f32,
};

pub const Command = struct {
    kind: i32,
    idx: i32,
};

const GLBuffers = struct {
    const Self = @This();
    commands: u32,
    sdfops: u32,
    materials: u32,
    shapes: u32,

    pub fn init() Self {
        var buffers: [4]u32 = undefined;
        gl.createBuffers(4, &buffers);
        return Self{
            .commands = buffers[0],
            .sdfops = buffers[1],
            .materials = buffers[2],
            .shapes = buffers[3],
        };
    }

    pub fn deinit(self: *Self) void {
        const buffers = [_]u32{ self.commands, self.sdfops, self.materials, self.shapes };
        gl.deleteBuffers(4, &buffers);
    }

    pub fn bind(self: *Self) void {
        const buffers = [_]u32{ self.commands, self.sdfops, self.materials, self.shapes };
        for (buffers, 0..) |buffer, i| {
            gl.bindBufferBase(gl.SHADER_STORAGE_BUFFER, i, buffer);
        }
    }
};

const GLResources = struct {
    const Self = @This();
    buffers: GLBuffers,
    vao: u32,
    vbo: u32,
    compute_program: u32,
    present_program: u32,
    compute_texture: u32 = 0,
    present_texture: u32 = 0,

    pub fn init() Self {
        var vao: u32 = undefined;
        var vbo: u32 = undefined;
        gl.createVertexArrays(1, &vao);
        gl.createBuffers(1, &vbo);

        gl.namedBufferData(vbo, present_vertices.len * @sizeOf(f32), &present_vertices, gl.STATIC_DRAW);
        gl.vertexArrayVertexBuffer(vao, 0, vbo, 0, 5 * @sizeOf(f32));

        gl.enableVertexArrayAttrib(vao, 0);
        gl.vertexArrayAttribFormat(vao, 0, 3, gl.FLOAT, gl.FALSE, 0);
        gl.vertexArrayAttribBinding(vao, 0, 0);

        gl.enableVertexArrayAttrib(vao, 1);
        gl.vertexArrayAttribFormat(vao, 1, 2, gl.FLOAT, gl.FALSE, 3 * @sizeOf(f32));
        gl.vertexArrayAttribBinding(vao, 1, 0);

        const compute_program = shaders.make_compute_program(compute_shader);
        const present_program = shaders.make_simple_program(vertex_shader, fragment_shader);

        const buffers = GLBuffers.init();

        return Self{
            .buffers = buffers,
            .vao = vao,
            .vbo = vbo,
            .compute_program = compute_program,
            .present_program = present_program,
        };
    }

    pub fn deinit(self: *Self) void {
        gl.deleteTextures(2, &[_]u32{ self.compute_texture, self.present_texture });
        gl.deleteVertexArrays(1, &self.vao);
        gl.deleteBuffers(1, &self.vbo);
        gl.deleteProgram(self.compute_program);
        gl.deleteProgram(self.present_program);
        self.buffers.deinit();
    }

    pub fn reset_textures(self: *Self, width: u32, height: u32) void {
        gl.deleteTextures(2, &.{ self.compute_texture, self.present_texture });
        self.compute_texture = textures.make(width, height, gl.R32F);
        self.present_program = textures.make(width, height, gl.RGBA32F);
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
    resources: GLResources,
    last_records: std.ArrayList(Recorder),
    recorders: std.ArrayList(Recorder),
    active: ?Recorder = null,
    record_counter: usize = 0,

    pub fn init(allocator: mem.Allocator) Self {
        const resources = GLResources.init();
        const recorders = std.ArrayList(Recorder).init(allocator);
        return Self{
            .resources = resources,
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

        self.resources.deinit();
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

    pub fn record_shape(self: *Self, shape: Shape, op: ?SDFOp) void {
        _ = op;
        _ = shape;
        if (self.active.?.shapes % 2 == 1) {}
    }
};
