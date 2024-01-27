const std = @import("std");
const mem = std.mem;

// TODO: fix this, no idea how sort function works, ask on discord lol
pub fn dedupsort(allocator: mem.Allocator, list: []const u32) ![]u32 {
    const sorted = try allocator.alloc(u32, list.len);
    defer allocator.free(sorted);
    @memcpy(sorted, list);
    std.mem.sort(u32, sorted, .{}, std.mem.lessThan);
    
    var unique = try allocator.alloc(u32, sorted.len);
    var unique_len: usize = 0;
    for (sorted, 0..) |item, index| {
        if (index == 0 or item != sorted[index - 1]) {
            unique[unique_len] = item;
            unique_len += 1;
        }
    }

    unique = try allocator.realloc(u32, unique, unique_len);
    return unique;
}