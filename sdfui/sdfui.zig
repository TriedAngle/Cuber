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
    material: u32 = 0,
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
    const Self = @This();
    kind: i32,
    idx: i32,

    pub fn sdfop(idx: i32) Self {
        return .{ .kind = 1, .idx = idx };
    }

    pub fn shape(idx: i32) Self {
        return .{ .kind = 2, .idx = idx };
    }
};

const GLBuffers = struct {
    const Self = @This();
    commands: u32,
    sdfops: u32,
    materials: u32,
    shapes: u32,
    sizes: struct {
        commands: usize = 0,
        sdfops: usize = 0,
        materials: usize = 0,
        shapes: usize = 0,
    } = .{},

    fn init() Self {
        var buffers: [4]u32 = undefined;
        gl.createBuffers(4, &buffers);
        return Self{
            .commands = buffers[0],
            .sdfops = buffers[1],
            .materials = buffers[2],
            .shapes = buffers[3],
            .sizes = .{},
        };
    }

    fn deinit(self: *Self) void {
        const buffers = [_]u32{ self.commands, self.sdfops, self.materials, self.shapes };
        gl.deleteBuffers(4, &buffers);
    }

    fn bind(self: *Self) void {
        const buffers = [_]u32{ self.commands, self.sdfops, self.materials, self.shapes };
        for (buffers, 0..) |buffer, i| {
            gl.bindBufferBase(gl.SHADER_STORAGE_BUFFER, @intCast(i), buffer);
        }
    }

    fn set_material(self: *Self, material: Material, id: usize) void {
        if (id >= self.sizes.materials) {
            var new_size: usize = self.sizes.materials * 2;
            var new_buffer = glu.buffers.resize(
                self.commands,
                Material,
                self.sizes.materials,
                new_size,
                gl.DYNAMIC_DRAW,
            );
            self.materials = new_buffer;
            self.sizes.materials = new_size;
        }
        glu.buffers.set_at(self.materials, Material, id, &material);
    }

    fn reset_dynamic_buffers(self: *Self, commands: usize, sdfops: usize, shapes: usize) void {
        self.sizes = .{ .commands = commands, .sdfops = sdfops, .shapes = shapes };
        glu.buffers.reset(self.commands, Command, commands, gl.DYNAMIC_DRAW);
        glu.buffers.reset(self.sdfops, SDFOp, sdfops, gl.DYNAMIC_DRAW);
        glu.buffers.reset(self.shapes, Shape, shapes, gl.DYNAMIC_DRAW);
    }
};

const GLResources = struct {
    const Self = @This();
    const ComputeUniforms = struct {
        command_offset: i32,
        command_count: i32,
        resolution: i32,
        cursor: i32,
        time: i32,
    };
    const PresentUniforms = struct {
        present_texture: i32,
    };

    buffers: GLBuffers,
    vao: u32,
    vbo: u32,
    compute_program: u32,
    present_program: u32,
    compute_texture: u32 = 0,
    present_texture: u32 = 0,
    compute_uniforms: ComputeUniforms,
    present_uniforms: PresentUniforms,

    fn init() Self {
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

        const compute_uniforms = .{
            .command_offset = glu.uniform_location(compute_program, "command_offset"),
            .command_count = glu.uniform_location(compute_program, "command_count"),
            .resolution = glu.uniform_location(compute_program, "resolution"),
            .cursor = glu.uniform_location(compute_program, "cursor"),
            .time = glu.uniform_location(compute_program, "time"),
        };

        const present_uniforms = .{
            .present_texture = glu.uniform_location(present_program, "tex"),
        };

        const buffers = GLBuffers.init();

        return Self{
            .buffers = buffers,
            .vao = vao,
            .vbo = vbo,
            .compute_program = compute_program,
            .present_program = present_program,
            .compute_uniforms = compute_uniforms,
            .present_uniforms = present_uniforms,
        };
    }

    fn deinit(self: *Self) void {
        gl.deleteTextures(2, &[_]u32{ self.compute_texture, self.present_texture });
        gl.deleteVertexArrays(1, &self.vao);
        gl.deleteBuffers(1, &self.vbo);
        gl.deleteProgram(self.compute_program);
        gl.deleteProgram(self.present_program);
        self.buffers.deinit();
    }

    fn bind_buffers(self: *Self) void {
        self.buffers.bind();
    }

    fn reset_textures(self: *Self, width: u32, height: u32) void {
        gl.deleteTextures(2, &.{ self.compute_texture, self.present_texture });
        self.compute_texture = textures.make(width, height, gl.R32F);
        self.present_program = textures.make(width, height, gl.RGBA32F);
    }

    fn bind_compute_uniforms(
        self: *Self,
        counters: *const Offsets,
        resolution: [2]i32,
        cursor: [2]f32,
        time: f32,
    ) void {
        const comp = self.compute_program;

        const cu = &self.compute_uniforms;

        gl.programUniform1i(comp, cu.command_offset, 0);
        gl.programUniform1ui(comp, cu.command_count, @intCast(counters.commands));
        gl.programUniform2i(comp, cu.resolution, resolution[0], resolution[1]);
        gl.programUniform2f(comp, cu.cursor, cursor[0], cursor[1]);
        gl.programUniform1f(comp, cu.time, time);

        gl.bindImageTexture(
            0,
            self.present_texture,
            0,
            gl.FALSE,
            0,
            gl.READ_WRITE,
            gl.RGBA16F,
        );

        gl.bindImageTexture(
            1,
            self.compute_texture,
            0,
            gl.FALSE,
            0,
            gl.READ_WRITE,
            gl.R16F,
        );
    }

    fn bind_present_uniforms(self: *Self) void {
        const pu = &self.present_uniforms;
        const prep = self.present_program;
        gl.bindTextureUnit(0, self.present_texture);
        gl.programUniform1i(prep, pu.present_texture, 0);
    }

    fn bind_vao(self: *Self) void {
        gl.vertexArrayVertexBuffer(self.vao, 0, self.vbo, 0, 5 * @sizeOf(f32));
        gl.bindVertexArray(self.vao);
        gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
    }
};

const Recorder = struct {
    const Self = @This();
    allocator: mem.Allocator,
    commands: std.ArrayList(Command),
    sdfops: std.ArrayList(SDFOp),
    shapes: std.ArrayList(Shape),

    fn init(
        allocator: mem.Allocator,
    ) Self {
        const commands = std.ArrayList(Command).init(allocator);
        const sdfops = std.ArrayList(SDFOp).init(allocator);
        const shapes = std.ArrayList(Shape).init(allocator);
        return Self{
            .allocator = allocator,
            .commands = commands,
            .sdfops = sdfops,
            .shapes = shapes,
        };
    }

    fn deinit(self: *Self) void {
        self.commands.deinit();
        self.shapes.deinit();
    }

    fn add_sdfop(self: *Self, op: SDFOp, id: u32) void {
        self.sdfops.append(op);
        self.commands.append(Command.sdfop(id));
    }

    fn add_shape(self: *Self, shape: Shape, id: u32) void {
        self.shapes.append(shape);
        self.commands.append(Command.shape(id));
    }
};

const Offsets = struct {
    commands: usize = 0,
    sdfops: usize = 0,
    materials: usize = 0,
    shapes: usize = 0,
};

pub const Context = struct {
    const Self = @This();
    allocator: mem.Allocator,
    resources: GLResources,
    last_records: std.ArrayList(Recorder),
    recorders: std.ArrayList(Recorder),
    active: ?Recorder = null,
    record_counter: usize = 0,
    counters: Offsets = .{},
    resolution: [2]i32 = undefined,
    cursor: [2]f32 = [_]f32{ 0, 0 },
    time: f32 = 0,

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
        self.counters = .{};
    }

    fn set_buffers(self: *Self) void {
        var buffers = &self.resources.buffers;
        const sizes = self.counters;
        buffers.reset_dynamic_buffers(sizes.commands, sizes.sdfops, sizes.shapes);
        var offsets: Offsets = .{};

        for (self.recorders.items) |*rec| {
            glu.buffers.write(buffers.commands, Command, offsets.commands, rec.commands.items);
            glu.buffers.write(buffers.commands, SDFOp, offsets.sdfops, rec.sdfops.items);
            glu.buffers.write(buffers.commands, Shape, offsets.shapes, rec.shapes.items);
            offsets.commands += rec.commands.items.len;
            offsets.sdfops += rec.sdfops.items.len;
            offsets.shapes += rec.shapes.items.len;
        }
    }

    pub fn render(self: *Self) void {
        self.set_buffers();
        var resources = &self.resources;

        resources.bind_buffers();

        gl.useProgram(resources.compute_program);
        resources.bind_compute_uniforms(
            &self.counters,
            self.resolution,
            self.cursor,
            self.time,
        );
        gl.dispatchCompute(
            @intCast(self.resolution[0]),
            @intCast(self.resolution[1]),
            1,
        );
        gl.memoryBarrier(gl.SHADER_IMAGE_ACCESS_BARRIER_BIT);
        gl.useProgram(0);

        gl.useProgram(resources.present_program);
        resources.bind_present_uniforms();
        resources.bind_vao();
        gl.bindVertexArray(0);
        gl.useProgram(0);
    }

    pub fn record(self: *Self) void {
        self.active = Recorder.init(self.allocator);
    }

    pub fn finish(self: *Self) void {
        self.recorders.append(self.active.?) catch unreachable;
        self.active = null;
        self.record_counter +%= 1;
    }

    pub fn record_sdfop(self: *Self, op: SDFOp) void {
        self.active.?.add_sdfop(op, self.counters.sdfops);
        self.counters.sdfops +%= 1;
        self.counters.commands +%= 1;
    }

    pub fn record_shape(self: *Self, shape: Shape) void {
        self.active.?.add_shape(shape, self.counters.shapes);
        self.counters.shapes +%= 1;
        self.counters.commands +%= 1;
    }
};
