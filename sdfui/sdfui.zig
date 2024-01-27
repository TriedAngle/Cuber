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

pub const Circle = packed struct { tag: u32 = 1, radius: f32, padding: u96 = 0 };

pub const Box = packed struct { tag: u32 = 2, width: f32, height: f32, padding: u64 = 0 };

pub const Shape = packed struct {
    x: f32,
    y: f32,
    material: u32 = 0,
    shape: packed union {
        EmptyShape: packed struct { tag: u32 = 0, padding: u128 = 0 },
        Circle: Circle,
        Box: Box,
    },
};

// 32 bytes
pub const Material = packed union {
    Color: packed struct { tag: u32 = 1, r: f32 = 0, g: f32 = 0, b: f32 = 0, a: f32 = 1, padding: i96 = 0 },
};

pub const SDFKind = enum(u32) {
    Min = 1,
    Max,
    SmoothMin,
    SmoothMax,
};

pub const SDFOp = struct {
    kind: SDFKind,
    padding: i32 = 0,
    data0: f32 = 0,
    data1: f32 = 0,
    data: [4]f32 = [_]f32{ 0, 0, 0, 0 },
};

// 1: mov
// 2: math
// 3: sdfop
// 4: shape
// 5: draw
pub const Command = struct {
    const Self = @This();
    kind: u32,
    idx: i32,
    data1: i32 = 0,
    data2: i32 = 0,

    // 1: d1->sdf
    // 2: d2->sdf
    // 3: sdf->d1
    // 4: sdf->d2
    // 5: d1<->d2
    pub fn mov(idx: i32) Self {
        return .{ .kind = 1, .idx = idx };
    }

    // 1 + 2: neg  | -d      -> d
    // 3 + 4: sqrt | sqrt d  -> d
    // 5 + 6: exp  | exp d   -> d
    // 7 + 8: ln   | ln d    -> d
    // 9:     +    | d1 + d2 -> d1
    // 10:    -    | d1 - d2 -> d1
    pub fn math(idx: i32) Self {
        return .{
            .kind = 2,
            .idx = idx,
        };
    }

    pub fn sdfop(idx: i32) Self {
        return .{ .kind = 3, .idx = idx };
    }

    // 1: d1, 2: d2
    pub fn shape(idx: i32, reg: i32) Self {
        return .{ .kind = 4, .idx = idx, .data1 = reg };
    }

    pub fn draw() Self {
        return .{ .kind = 5, .idx = 0 };
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
        materials: usize = 0,
        sdfops: usize = 0,
        shapes: usize = 0,
    } = .{},

    fn init() Self {
        var buffers: [4]u32 = undefined;
        gl.createBuffers(4, &buffers);
        glu.buffers.reset(buffers[1], Material, 8, gl.DYNAMIC_COPY);
        return Self{
            .commands = buffers[0],
            .materials = buffers[1],
            .sdfops = buffers[2],
            .shapes = buffers[3],
            .sizes = .{ .materials = 10 },
        };
    }

    fn deinit(self: *Self) void {
        const buffers = [_]u32{ self.commands, self.sdfops, self.materials, self.shapes };
        gl.deleteBuffers(4, &buffers);
    }

    fn bind(self: *Self) void {
        gl.bindBufferBase(gl.SHADER_STORAGE_BUFFER, 0, self.commands);
        gl.bindBufferBase(gl.SHADER_STORAGE_BUFFER, 1, self.materials);
        gl.bindBufferBase(gl.SHADER_STORAGE_BUFFER, 2, self.sdfops);
        gl.bindBufferBase(gl.SHADER_STORAGE_BUFFER, 3, self.shapes);
    }

    fn set_material(self: *Self, material: Material, id: usize) void {
        if (id >= self.sizes.materials) {
            const new_size: usize = self.sizes.materials * 2;
            const new_buffer = glu.buffers.resize(
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
        glu.buffers.reset(self.commands, Command, commands, gl.DYNAMIC_COPY);
        glu.buffers.reset(self.sdfops, SDFOp, sdfops, gl.DYNAMIC_COPY);
        glu.buffers.reset(self.shapes, Shape, shapes, gl.DYNAMIC_COPY);
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
        compute_texture: i32,
        present_texture: i32,
        weight_texture: i32,
    };

    buffers: GLBuffers,
    vao: u32,
    vbo: u32,
    compute_program: u32,
    present_program: u32,
    compute_texture: u32 = 0,
    present_texture: u32 = 0,
    weight_texture: u32 = 0,
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

        const present_uniforms = .{ .compute_texture = glu.uniform_location(present_program, "sdf"), .present_texture = glu.uniform_location(present_program, "present"), .weight_texture = glu.uniform_location(present_program, "weights") };

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

    fn reset_textures(self: *Self, resolution: [2]i32) void {
        const texs = [_]u32{ self.compute_texture, self.present_texture };
        gl.deleteTextures(2, &texs);
        self.compute_texture = textures.make(resolution[0], resolution[1], gl.R32F);
        self.present_texture = textures.make(resolution[0], resolution[1], gl.RGBA32F);
        self.weight_texture = textures.make(resolution[0], resolution[1], gl.R32F);
    }

    fn set_buffers(self: *Self, recorders: []const Recorder, maxs: Offsets) void {
        var bufs = &self.buffers;
        bufs.reset_dynamic_buffers(maxs.commands, maxs.sdfops, maxs.shapes);
        var offsets: Offsets = .{};

        for (recorders) |*rec| {
            glu.buffers.write(bufs.commands, Command, offsets.commands, rec.commands.items);
            glu.buffers.write(bufs.sdfops, SDFOp, offsets.sdfops, rec.sdfops.items);
            glu.buffers.write(bufs.shapes, Shape, offsets.shapes, rec.shapes.items);
            offsets.commands += rec.commands.items.len;
            offsets.sdfops += rec.sdfops.items.len;
            offsets.shapes += rec.shapes.items.len;
        }
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
        gl.bindImageTexture(0, self.compute_texture, 0, gl.FALSE, 0, gl.READ_WRITE, gl.R32F);
        gl.bindImageTexture(1, self.present_texture, 0, gl.FALSE, 0, gl.READ_WRITE, gl.RGBA32F);
        gl.bindImageTexture(2, self.weight_texture, 0, gl.FALSE, 0, gl.READ_WRITE, gl.R32F);
    }

    fn bind_present_uniforms(self: *Self) void {
        const pu = &self.present_uniforms;
        const prep = self.present_program;
        gl.bindTextureUnit(0, self.compute_texture);
        gl.programUniform1i(prep, pu.compute_texture, 0);

        gl.bindTextureUnit(1, self.present_texture);
        gl.programUniform1i(prep, pu.present_texture, 1);

        gl.bindTextureUnit(2, self.weight_texture);
        gl.programUniform1i(prep, pu.weight_texture, 2);
    }

    fn draw_vao(self: *Self) void {
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
        self.sdfops.deinit();
        self.shapes.deinit();
    }

    fn add_sdfop(self: *Self, op: SDFOp, id: usize) void {
        self.sdfops.append(op) catch unreachable;
        self.commands.append(Command.sdfop(@intCast(id))) catch unreachable;
    }

    fn add_shape(self: *Self, shape: Shape, id: usize) void {
        self.shapes.append(shape) catch unreachable;
        const reg: usize = 2;
        const command = Command.shape(@intCast(id), @intCast(reg));
        self.commands.append(command) catch unreachable;
    }

    fn add_draw(self: *Self) void {
        self.commands.append(Command.draw()) catch unreachable;
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
    materials: std.AutoHashMap(u256, u32),
    material_count: u32 = 0,
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
        const materials = std.AutoHashMap(u256, u32).init(allocator);
        return Self{
            .resources = resources,
            .materials = materials,
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
        self.materials.deinit();
        self.resources.deinit();
    }

    pub fn update_resolution(self: *Self, resolution: [2]i32) void {
        self.resolution = resolution;
        self.resources.reset_textures(resolution);
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

    pub fn render(self: *Self) void {
        var resources = &self.resources;
        resources.set_buffers(self.recorders.items, self.counters);
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

        gl.clear(gl.DEPTH_BUFFER_BIT);

        gl.useProgram(resources.present_program);
        resources.bind_present_uniforms();
        resources.draw_vao();
        gl.bindVertexArray(0);
        gl.useProgram(0);
    }

    pub fn record(self: *Self) void {
        self.active = Recorder.init(self.allocator);
    }

    pub fn finish(self: *Self) void {
        self.recorders.append(self.active.?) catch unreachable;
        self.active = null;
        self.record_counter += 1;
    }

    pub fn record_sdfop(self: *Self, op: SDFOp) void {
        self.active.?.add_sdfop(op, self.counters.sdfops);
        self.counters.sdfops += 1;
        self.counters.commands += 1;
    }

    pub fn record_shape(self: *Self, shape: Shape) void {
        self.active.?.add_shape(shape, self.counters.shapes);
        self.counters.shapes += 1;
        self.counters.commands += 1;
    }

    pub fn draw(self: *Self) void {
        self.active.?.add_draw();
        self.counters.commands += 1;
    }

    pub fn material(self: *Self, mat: Material) u32 {
        const roflmao: u256 = @bitCast(mat);
        if (self.materials.get(roflmao)) |id| {
            return id;
        }
        const id = self.material_count;
        self.materials.put(roflmao, id) catch unreachable;
        self.resources.buffers.set_material(mat, id);
        self.material_count += 1;
        return id;
    }
};
