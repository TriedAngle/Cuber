const std = @import("std");
const gl = @import("gl");

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
    var new_buffer = empty(T, new, usage);
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