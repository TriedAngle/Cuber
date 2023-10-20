const std = @import("std");



// pub const TypeMap = struct {
//   map: std.AutoHashMap(comptime K: type, comptime V: type)
// }

fn typeId(comptime T: type) usize {
  _ = T;
    return @intFromPtr(&struct { var x: u8 = 0; }.x);
}