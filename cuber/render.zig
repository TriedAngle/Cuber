const std = @import("std");
const mem = std.mem;
const gl = @import("gl");
const glu = @import("glutils");

const m = @import("math");
const cam = @import("camera.zig");
const mat = @import("materials.zig");
const brick = @import("brickmap.zig");

const vertex_shader = @embedFile("shaders/shader.vert");
const fragment_shader = @embedFile("shaders/shader.frag");
const compute_shader = @embedFile("shaders/shader.comp");

const present_vertices = [_]f32{
    -1.0, 1.0, 0.0, 0.0, 1.0, //noformat
    -1.0, -1.0, 0.0, 0.0, 0.0, //noformat
    1.0, 1.0, 0.0, 1.0, 1.0, //noformat
    1.0, -1.0, 0.0, 1.0, 0.0, //noformat
};

const Resources = struct {
    const Self = @This();
    allocator: mem.Allocator,

    albedo_texture: glu.Texture,
    depth_texture: glu.Texture,
    normal_texture: glu.Texture,

    chunk_buffer: glu.Buffer,
    palette_chunk_buffer: glu.Buffer,
    brick_buffer: glu.Buffer,
    palette_buffer: glu.Buffer,
    material_buffer: glu.Buffer,
    materials: std.AutoHashMap(u256, u32),
    material_count: u32 = 0,

    fn init(allocator: mem.Allocator) Self {
        const albedo_texture = glu.Texture.new_dummy(gl.TEXTURE_2D, gl.RGBA32F, 1);
        const depth_texture = glu.Texture.new_dummy(gl.TEXTURE_2D, gl.R16F, 1);
        const normal_texture = glu.Texture.new_dummy(gl.TEXTURE_2D, gl.RGBA16F, 1);

        const chunk_buffer = glu.Buffer.new(brick.BrickChunk, gl.DYNAMIC_COPY);
        const palette_chunk_buffer = glu.Buffer.new(brick.PaletteChunk, gl.DYNAMIC_COPY);
        const brick_buffer = glu.Buffer.new(brick.Brick, gl.DYNAMIC_COPY);
        const palette_buffer = glu.Buffer.new(u32, gl.DYNAMIC_COPY);
        const material_buffer = glu.Buffer.new_sized(mat.Material, 32, gl.DYNAMIC_COPY);
        const materials = std.AutoHashMap(u256, u32).init(allocator);
        return Self{
            .allocator = allocator,
            .albedo_texture = albedo_texture,
            .depth_texture = depth_texture,
            .normal_texture = normal_texture,
            .chunk_buffer = chunk_buffer,
            .palette_chunk_buffer = palette_chunk_buffer,
            .brick_buffer = brick_buffer,
            .palette_buffer = palette_buffer,
            .material_buffer = material_buffer,
            .materials = materials,
        };
    }

    fn deinit(self: *Self) void {
        self.albedo_texture.deinit();
        self.depth_texture.deinit();
        self.normal_texture.deinit();
        self.chunk_buffer.deinit();
        self.palette_chunk_buffer.deinit();
        self.brick_buffer.deinit();
        self.palette_buffer.deinit();
        self.material_buffer.deinit();
        self.materials.deinit();
    }

    fn resize_screen_textures(self: *Self, width: i32, height: i32) void {
        self.albedo_texture.resize(width, height);
        self.depth_texture.resize(width, height);
        self.normal_texture.resize(width, height);
    }

    fn add_material_buffer(self: *Self, material: *const mat.Material, offset: u32) void {
        if (offset >= self.material_buffer.size) {
            self.material_buffer.resize(self.material_buffer.size * 2);
        }
        self.material_buffer.write_at(offset, material);
    }

    fn add_material(self: *Self, material: *const mat.Material) u32 {
        const hash = material.hash();
        if (self.materials.get(hash)) |id| {
            return id;
        }
        const id = self.material_count;
        self.materials.put(hash, id) catch unreachable;
        self.add_material_buffer(material, id);
        self.material_count += 1;
        return id;
    }
};

pub const RenderConfig = struct {
    debug_texture: ?DebugTexture,
    initial_brickgrid: struct { x: u32, y: u32, z: u32 } = .{ .x = 64, .y = 64, .z = 64 },
};

pub const Renderer = struct {
    const Self = @This();
    allocator: mem.Allocator,
    config: RenderConfig,
    vao: u32,
    vbo: glu.Buffer,
    width: u32 = 0,
    height: u32 = 0,
    resources: Resources,
    grid: brick.BrickGrid,

    compute: glu.Program,
    present: glu.Program,
    randomer: std.rand.DefaultPrng,
    random: std.rand.Random,
    dtime: i64 = 0,
    frame: u32 = 0,

    pub fn init(allocator: mem.Allocator, config: RenderConfig) Self {
        const resources = Resources.init(allocator);
        var randomer = std.rand.DefaultPrng.init(69420666);
        const lol = vbovao_default();
        var present = glu.Program.new_simple(allocator, vertex_shader, fragment_shader);
        var compute = glu.Program.new_compute(allocator, compute_shader);
        const grid_config = config.initial_brickgrid;
        const grid = brick.BrickGrid.new(allocator, grid_config.x, grid_config.y, grid_config.z);
        return Self{
            .allocator = allocator,
            .config = config,
            .vao = lol.vao,
            .vbo = lol.vbo,
            .resources = resources,
            .grid = grid,
            .compute = compute,
            .present = present,
            .randomer = randomer,
            .random = randomer.random(),
        };
    }

    pub fn deinit(self: *Self) void {
        self.resources.deinit();
        self.present.deinit();
        self.compute.deinit();
        self.grid.deinit();
        gl.deleteVertexArrays(1, &[_]u32{self.vao});
    }

    pub fn update(self: *Self, dtime: i64) void {
        self.dtime = dtime;
        self.frame +%= 1;
    }

    pub fn render(self: *Self, camera: *const cam.Camera) void {
        self.execute_compute(camera);
        gl.memoryBarrier(gl.SHADER_IMAGE_ACCESS_BARRIER_BIT);
        self.execute_present();
    }

    fn execute_compute(self: *Self, camera: *const cam.Camera) void {
        var compute = &self.compute;
        var resources = &self.resources;

        const grid = &self.grid;

        compute.use();
        resources.chunk_buffer.bind(0);
        resources.palette_chunk_buffer.bind(1);
        resources.brick_buffer.bind(2);
        resources.palette_buffer.bind(3);
        resources.material_buffer.bind(4);
        resources.albedo_texture.bind(0, 0, 0);
        resources.depth_texture.bind(1, 0, 0);
        resources.normal_texture.bind(2, 0, 0);
        compute.uniform("cameraPos", m.Vec3, camera.position);
        compute.uniform("cameraDir", m.Vec3, camera.front);
        compute.uniform("cameraU", m.Vec3, camera.right);
        compute.uniform("cameraV", m.Vec3, camera.up);
        compute.uniform("timer", u32, @intCast(self.dtime));
        // compute.uniform("randomSeed", f32, self.random.float(f32));
        compute.uniform("resolution", [2]u32, [_]u32{ self.width, self.height });
        compute.uniform("brickGridSize", [3]u32, [_]u32{ grid.x, grid.y, grid.z });
        compute.dispatch(self.width, self.height, 1);
        compute.unuse();
    }

    fn execute_present(self: *Self) void {
        var present_texture: *glu.Texture = undefined;
        if (self.config.debug_texture) |debug_texture| {
            switch (debug_texture) {
                .Albedo => present_texture = &self.resources.albedo_texture,
                .Depth => present_texture = &self.resources.depth_texture,
                .Normal => present_texture = &self.resources.normal_texture,
            }
        }
        var present = &self.present;
        present.use();
        present_texture.bind(0, 0, 0);
        present.uniform("tex", *glu.Texture, present_texture);
        gl.bindVertexArray(self.vao);
        gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
        gl.bindVertexArray(0);
        present.unuse();
    }

    pub fn resize(self: *Self, width: u32, height: u32) void {
        self.width = width;
        self.height = height;
        self.resources.resize_screen_textures(@intCast(width), @intCast(height));
        gl.viewport(0, 0, @intCast(width), @intCast(height));
    }

    pub fn add_chunk(self: *Self, chunk: brick.BrickChunk) void {
        var resources = &self.resources;
        resources.chunk_buffer.deinit();
        resources.chunk_buffer = glu.Buffer.new_data(brick.BrickChunk, &[_]brick.BrickChunk{chunk}, gl.STATIC_DRAW);
    }

    pub fn add_material(self: *Self, material: *const mat.Material) u32 {
        return self.resources.add_material(material);
    }
};

fn vbovao_default() struct { vao: u32, vbo: glu.Buffer } {
    var vao: u32 = undefined;
    var vbo = glu.Buffer.new_data(f32, &present_vertices, gl.DYNAMIC_DRAW);
    gl.createVertexArrays(1, &vao);
    gl.vertexArrayVertexBuffer(vao, 0, vbo.handle, 0, 5 * @sizeOf(f32));
    gl.enableVertexArrayAttrib(vao, 0);
    gl.vertexArrayAttribFormat(vao, 0, 3, gl.FLOAT, gl.FALSE, 0);
    gl.vertexArrayAttribBinding(vao, 0, 0);

    gl.enableVertexArrayAttrib(vao, 1);
    gl.vertexArrayAttribFormat(vao, 1, 2, gl.FLOAT, gl.FALSE, 3 * @sizeOf(f32));
    gl.vertexArrayAttribBinding(vao, 1, 0);
    return .{ .vao = vao, .vbo = vbo };
}

const DebugTexture = enum {
    Albedo,
    Depth,
    Normal,

    pub fn next(current: DebugTexture) DebugTexture {
        const enumInt = @intFromEnum(current);
        const enumCount = @typeInfo(DebugTexture).Enum.fields.len;
        const nextInt = (enumInt + 1) % enumCount;
        return @enumFromInt(nextInt);
    }
};
