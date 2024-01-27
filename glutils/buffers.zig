const std = @import("std");
const gl = @import("gl");

pub const Buffer = struct {
    const Self = @This();
    handle: gl.GLuint = 0,
    t_size: usize,
    size: usize = 0,
    usage: u32,

    pub fn new(comptime T: type, usage: u32) Self {
        var self = Self{ .t_size = @sizeOf(T), .usage = usage };
        gl.createBuffers(1, &self.handle);
        return self;
    }

    pub fn new_sized(comptime T: type, size: usize, usage: u32) Self {
        var self = Self.new(T, usage);
        self.size = size;
        gl.namedBufferData(self.handle, @intCast(size * self.t_size), null, self.usage);
        return self;
    }

    pub fn new_data(comptime T: type, data: []const T, usage: u32) Self {
        var self = Self.new(T, usage);
        self.reset(data);
        return self;
    }

    pub fn deinit(self: *Self) void {
        gl.deleteBuffers(1, &[_]u32{self.handle});
    }

    pub fn resize(self: *Self, size: usize) void {
        var next = Self{
            .t_size = self.t_size,
            .size = size,
            .usage = self.usage,
        };
        gl.createBuffers(1, &next.handle);
        gl.namedBufferData(next.handle, @intCast(size * next.t_size), null, next.usage);

        gl.copyNamedBufferSubData(
            self.handle,
            next.handle,
            0,
            0,
            @intCast(self.size * self.t_size),
        );

        self.deinit();
        self.* = next;
    }

    pub fn reset(self: *Self, data: anytype) void {
        self.size = data.len;
        gl.namedBufferData(self.handle, @intCast(data.len * self.t_size), data.ptr, self.usage);
    }

    pub fn bind(self: *const Self, index: u32) void {
        gl.bindBufferBase(gl.SHADER_STORAGE_BUFFER, index, self.handle);
    }

    pub fn write_at(self: *Self, offset: u32, value: *const anyopaque) void {
        gl.namedBufferSubData(
            self.handle,
            offset * self.t_size,
            @intCast(self.t_size),
            value,
        );
    }
};

// TODO: remove the functions
pub fn reset(buffer: u32, comptime T: type, size: usize, usage: u32) void {
    gl.namedBufferData(buffer, @intCast(size * @sizeOf(T)), null, usage);
}

pub fn empty(comptime T: type, size: usize, usage: u32) u32 {
    var buffer: u32 = undefined;
    gl.createBuffers(1, &buffer);
    reset(buffer, T, size, usage);
    return buffer;
}

pub fn set(buffer: u32, comptime T: type, data: []const T, usage: u32) void {
    gl.namedBufferData(
        buffer,
        data.len * @sizeOf(T),
        data.ptr,
        usage,
    );
}

pub fn write(buffer: u32, comptime T: type, offset: usize, data: []const T) void {
    gl.namedBufferSubData(
        buffer,
        offset * @sizeOf(T),
        @intCast(data.len * @sizeOf(T)),
        data.ptr,
    );
}

pub fn resize(
    buffer: u32,
    comptime T: type,
    current: usize,
    new: usize,
    usage: u32,
) u32 {
    const new_buffer = empty(T, new, usage);
    gl.copyNamedBufferSubData(
        buffer,
        new_buffer,
        0,
        0,
        @intCast(current * @sizeOf(T)),
    );
    // gl.deleteBuffers(1, buffer);
    return new_buffer;
}

pub fn set_at(
    buffer: u32,
    comptime T: type,
    offset: usize,
    value: *const T,
) void {
    gl.namedBufferSubData(
        buffer,
        offset * @sizeOf(T),
        @sizeOf(T),
        value,
    );
}

pub fn vao() u32 {
    return 0;
}

pub fn vaobo() void {}
