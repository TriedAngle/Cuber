const std = @import("std");

fn implMatrix(comptime Self: type, comptime m: comptime_int, comptime n: comptime_int) type {
    return struct {
        pub fn new_raw(data: [m][n]f32) Self {
            return .{ .inner = data };
        }

        pub fn all(value: f32) Self {
            var result: Self = undefined;
            inline for (0..n) |col| {
                inline for (0..m) |row| {
                    result.inner[col][row] = value;
                }
            }
            return result;
        }

        pub fn add(a: Self, b: Self) Self {
            var result: Self = undefined;
            inline for (0..n) |col| {
                inline for (0..m) |row| {
                    result.inner[col][row] = a.inner[col][row] + b.inner[col][row];
                }
            }
            return result;
        }

        pub fn sub(a: Self, b: Self) Self {
            var result: Self = undefined;
            inline for (0..n) |col| {
                inline for (0..m) |row| {
                    result.inner[col][row] = a.inner[col][row] - b.inner[col][row];
                }
            }
            return result;
        }

        pub fn scale(a: Self, s: f32) Self {
            var result: Self = undefined;
            inline for (0..n) |col| {
                inline for (0..m) |row| {
                    result.inner[col][row] = s * a.inner[col][row];
                }
            }
            return result;
        }

        pub fn eql(a: Self, b: Self) bool {
            return std.meta.eql(a, b);
        }
    };
}

fn implSquareMatrix(comptime Self: type, comptime n: comptime_int) type {
    return struct {
        pub fn identity() Self {
            var result: Self = Self.all(0);
            inline for (0..n) |diag| {
                result.inner[diag][diag] = 1;
            }
            return result;
        }

        // todo make this for all matrices mxn * nxp
        pub fn mul(a: Self, b: Self) Self {
            var result: Self = undefined;
            inline for (0..n) |col| {
                inline for (0..n) |row| {
                    var sum: f32 = 0;
                    inline for (0..n) |left_col| {
                        sum += a.inner[left_col][row] * b.inner[col][left_col];
                    }
                    result.inner[col][row] = sum;
                }
            }
            return result;
        }
    };
}

fn implVector(comptime Self: type, comptime n: comptime_int) type {
    return struct {
        pub fn dot(a: Self, b: Self) f32 {
            var result: f32 = 0;
            inline for (0..n) |col| {
                result += a.inner[0][col] * b.inner[0][col];
            }
            return result;
        }

        pub fn length_squared(a: Self) f32 {
            return a.dot(a);
        }

        pub fn length(a: Self) f32 {
            return @sqrt(a.length_squared());
        }

        pub fn raw(a: Self) [n]f32 {
            return a.inner[0];
        }

        pub fn normalize(a: Self) Self {
            const l = a.length();
            const result = a.scale(1 / l);
            return result;
        }
    };
}

pub const Vec2 = struct {
    const Self = @This();
    inner: [1][2]f32,
    pub usingnamespace implMatrix(Self, 2, 1);
    pub usingnamespace implVector(Self, 2);

    pub fn new(x: f32, y: f32) Self {
        return .{ .inner = .{.{ x, y }} };
    }
    pub fn format(a: Self, comptime _: []const u8, _: std.fmt.FormatOptions, stream: anytype) !void {
        try stream.print("Vec2({d:.2}, {d:.2})", .{ a.gx(), a.gy() });
    }
    pub fn gx(a: Self) f32 {
        return a.inner[0][0];
    }
    pub fn gy(a: Self) f32 {
        return a.inner[0][1];
    }
    pub fn sx(a: Self, value: f32) Self {
        var result = a;
        result.inner[0][0] = value;
        return result;
    }
    pub fn sy(a: Self, value: f32) Self {
        var result = a;
        result.inner[0][1] = value;
        return result;
    }
};

pub const Vec3 = struct {
    const Self = @This();
    inner: [1][3]f32,
    pub usingnamespace implMatrix(Self, 3, 1);
    pub usingnamespace implVector(Self, 3);

    pub fn new(x: f32, y: f32, z: f32) Self {
        return .{ .inner = .{.{ x, y, z }} };
    }
    pub fn format(a: Self, comptime _: []const u8, _: std.fmt.FormatOptions, stream: anytype) !void {
        try stream.print("Vec3({d:.2}, {d:.2}, {d:.2})", .{ a.gx(), a.gy(), a.gz() });
    }
    pub fn cross(a: Self, b: Self) Self {
        var result: Self = undefined;
        result.inner[0][0] = a.gy() * b.gz() - a.gz() * b.gy();
        result.inner[0][1] = a.gz() * b.gx() - a.gx() * b.gz();
        result.inner[0][2] = a.gx() * b.gy() - a.gy() * b.gx();
        return result;
    }
    pub fn gx(a: Self) f32 {
        return a.inner[0][0];
    }
    pub fn gy(a: Self) f32 {
        return a.inner[0][1];
    }
    pub fn gz(a: Self) f32 {
        return a.inner[0][2];
    }
    pub fn sx(a: Self, value: f32) Self {
        var result = a;
        result.inner[0][0] = value;
        return result;
    }
    pub fn sy(a: Self, value: f32) Self {
        var result = a;
        result.inner[0][1] = value;
        return result;
    }
    pub fn sz(a: Self, value: f32) Self {
        var result = a;
        result.inner[0][2] = value;
        return result;
    }
};

pub const Vec4 = struct {
    const Self = @This();
    inner: [1][4]f32,
    pub usingnamespace implMatrix(Self, 4, 1);
    pub usingnamespace implVector(Self, 4);

    pub fn new(x: f32, y: f32, z: f32, w: f32) Self {
        return .{ .inner = .{.{ x, y, z, w }} };
    }
    pub fn format(a: Self, comptime _: []const u8, _: std.fmt.FormatOptions, stream: anytype) !void {
        try stream.print("Vec4({d:.2}, {d:.2}, {d:.2}, {d:.2})", .{ a.gx(), a.gy(), a.gz(), a.gw() });
    }
    pub fn gx(a: Self) f32 {
        return a.inner[0][0];
    }
    pub fn gy(a: Self) f32 {
        return a.inner[0][1];
    }
    pub fn gz(a: Self) f32 {
        return a.inner[0][2];
    }
    pub fn gw(a: Self) f32 {
        return a.inner[0][3];
    }
    pub fn sx(a: Self, value: f32) Self {
        var result = a;
        result.inner[0][0] = value;
        return result;
    }
    pub fn sy(a: Self, value: f32) Self {
        var result = a;
        result.inner[0][1] = value;
        return result;
    }
    pub fn sz(a: Self, value: f32) Self {
        var result = a;
        result.inner[0][2] = value;
        return result;
    }
    pub fn sw(a: Self, value: f32) Self {
        var result = a;
        result.inner[0][3] = value;
        return result;
    }
};

pub const Mat2 = struct {
    const Self = @This();
    inner: [2][2]f32, // [column][row] / [n][m]
    pub usingnamespace implMatrix(Self, 2, 2);
    pub usingnamespace implSquareMatrix(Self, 2);

    pub fn new(r0: Vec2, r1: Vec2) Self {
        return Self.new_raw(.{
            .{ r0.gx(), r1.gx() },
            .{ r0.gy(), r1.gy() },
        });
    }

    pub fn format(a: Self, comptime _: []const u8, _: std.fmt.FormatOptions, stream: anytype) !void {
        const r0 = a.row(0);
        const r1 = a.row(1);
        try stream.print("|{d:.2}, {d:.2}|\n", .{ r0.gx(), r0.gy() });
        try stream.print("|{d:.2}, {d:.2}|", .{ r1.gx(), r1.gy() });
    }

    pub fn row(s: Self, r: usize) Vec2 {
        return Vec2.new(s.inner[0][r], s.inner[1][r]);
    }

    pub fn column(s: Self, c: usize) Vec2 {
        return Vec2.new_raw(.{s.inner[c]});
    }
};

pub const Mat3 = struct {
    const Self = @This();
    inner: [3][3]f32, // [column][row] / [n][m]
    pub usingnamespace implMatrix(Self, 3, 3);
    pub usingnamespace implSquareMatrix(Self, 3);

    pub fn new(r0: Vec3, r1: Vec3, r2: Vec3) Self {
        return Self.new_raw(.{
            .{ r0.gx(), r1.gx(), r2.gx() },
            .{ r0.gy(), r1.gy(), r2.gy() },
            .{ r0.gz(), r1.gz(), r2.gz() },
        });
    }

    pub fn format(a: Self, comptime _: []const u8, _: std.fmt.FormatOptions, stream: anytype) !void {
        const r0 = a.row(0);
        const r1 = a.row(1);
        const r2 = a.row(2);
        try stream.print("|{d:.2}, {d:.2}, {d:.2}|\n", .{ r0.gx(), r0.gy(), r0.gz() });
        try stream.print("|{d:.2}, {d:.2}, {d:.2}|\n", .{ r1.gx(), r1.gy(), r1.gz() });
        try stream.print("|{d:.2}, {d:.2}, {d:.2}|", .{ r2.gx(), r2.gy(), r2.gz() });
    }

    pub fn row(s: Self, r: usize) Vec3 {
        return Vec3.new(s.inner[0][r], s.inner[1][r], s.inner[2][r]);
    }

    pub fn column(s: Self, c: usize) Vec3 {
        return Vec3.new_raw(.{s.inner[c]});
    }
};

pub const Mat4 = struct {
    const Self = @This();
    inner: [4][4]f32, // [column][row] / [n][m]
    pub usingnamespace implMatrix(Self, 4, 4);
    pub usingnamespace implSquareMatrix(Self, 4);

    pub fn format(a: Self, comptime _: []const u8, _: std.fmt.FormatOptions, stream: anytype) !void {
        const r0 = a.row(0);
        const r1 = a.row(1);
        const r2 = a.row(2);
        const r3 = a.row(2);
        try stream.print("|{d:.2}, {d:.2}, {d:.2}, {d:.2}|\n", .{ r0.gx(), r0.gy(), r0.gz(), r0.gw() });
        try stream.print("|{d:.2}, {d:.2}, {d:.2}, {d:.2}|\n", .{ r1.gx(), r1.gy(), r1.gz(), r1.gw() });
        try stream.print("|{d:.2}, {d:.2}, {d:.2}, {d:.2}|\n", .{ r2.gx(), r2.gy(), r2.gz(), r2.gw() });
        try stream.print("|{d:.2}, {d:.2}, {d:.2}, {d:.2}|", .{ r3.gx(), r3.gy(), r3.gz(), r3.gw() });
    }

    pub fn look_at(eye: Vec3, at: Vec3, up: Vec3) Self {
        var v = at.sub(eye).normalize(); //z
        const n = v.cross(up).normalize(); //x
        const u = n.cross(v); //y
        v = v.scale(-1);
        return Self.new(
            vec4(n.gx(), n.gy(), n.gz(), -n.dot(eye)),
            vec4(u.gx(), u.gy(), u.gz(), -u.dot(eye)),
            vec4(v.gx(), v.gy(), v.gz(), -v.dot(eye)),
            vec4(0, 0, 0, 1),
        );
    }

    /// construction via rows for mental
    pub fn new(r0: Vec4, r1: Vec4, r2: Vec4, r3: Vec4) Self {
        return Self.new_raw(.{
            .{ r0.gx(), r1.gx(), r2.gx(), r3.gx() },
            .{ r0.gy(), r1.gy(), r2.gy(), r3.gy() },
            .{ r0.gz(), r1.gz(), r2.gz(), r3.gz() },
            .{ r0.gw(), r1.gw(), r2.gw(), r3.gw() },
        });
    }

    pub fn row(s: Self, r: usize) Vec4 {
        return Vec4.new(s.inner[0][r], s.inner[1][r], s.inner[2][r], s.inner[3][r]);
    }

    pub fn column(s: Self, c: usize) Vec4 {
        return Vec4.new_raw(.{s.inner[c]});
    }
};

comptime {
    _ = @import("std").math;
}

pub const vec2 = Vec2.new;
pub const vec3 = Vec3.new;
pub const vec4 = Vec4.new;
pub const mat2 = Mat2.new;
pub const mat3 = Mat3.new;
pub const mat4 = Mat4.new;
pub const look_at = Mat4.look_at;

test "vectors" {
    const v1 = vec3(1, 2, 3);
    const v2 = vec3(3, 0, -4);
    try std.testing.expect(Vec3.eql(v1.add(v2), vec3(4, 2, -1)));
    try std.testing.expect(Vec3.eql(v1.sub(v2), vec3(-2, 2, 7)));

    try std.testing.expect(Vec3.eql(v2.scale(-10), vec3(-30, 0, 40)));

    try std.testing.expectFmt("Vec3(0.27, 0.53, 0.80)", "{}", .{v1.normalize()});

    try std.testing.expectEqual(v1.dot(v1), 14);
    try std.testing.expectEqual(v2.dot(v2), 25);
    try std.testing.expectEqual(v1.dot(v2), -9);

    try std.testing.expect(Vec3.eql(v1.cross(v2), vec3(-8, 13, -6)));
    try std.testing.expectEqual(v1.length(), @sqrt(14.0));
}

test "matrices" {
    const m0 = Mat3.identity();
    const m1 = mat3(
        vec3(1, 2, 3),
        vec3(4, 5, 6),
        vec3(7, 8, 9),
    );
    const m2 = mat3(
        vec3(2, 3, -5),
        vec3(0, -6, 4),
        vec3(9, 1, -1),
    );
    try std.testing.expect(Mat3.eql(m1.add(m2), mat3(
        vec3(3, 5, -2),
        vec3(4, -1, 10),
        vec3(16, 9, 8),
    )));
    try std.testing.expect(Mat3.eql(m1.sub(m2), mat3(
        vec3(-1, -1, 8),
        vec3(4, 11, 2),
        vec3(-2, 7, 10),
    )));
    try std.testing.expect(Mat3.eql(m0.mul(m0), m0));
    try std.testing.expect(Mat3.eql(m1.mul(m0), m1));

    try std.testing.expect(Mat3.eql(m1.mul(m2), mat3(
        vec3(29, -6, 0),
        vec3(62, -12, -6),
        vec3(95, -18, -12),
    )));
}
//*
test "lookAt" {
    const eye = vec3(4, 5, 6);
    const center = vec3(7, 8, 9);
    const up = vec3(0, 1, 0);
    const view = Mat4.look_at(eye, center, up);
    std.debug.print("\n{}\n", .{view});
}

// test "vectors" {
//     const mat_1 = Mat2.new_raw(.{ .{ 1, 2 }, .{ 3, 4 } });
//     const mat_2 = Mat2.new_raw(.{ .{ 0, 4 }, .{ 2, 4 } });
//     const mat_res = mat_1.add(mat_2);
//     try std.testing.expectEqual(mat_res, Mat2.new_raw(.{ .{ 1, 6 }, .{ 5, 8 } }));
//     const vec_1 = Vec3.new(1, 2, 3);
//     const vec_2 = Vec3.new(4, 5, 6);
//     const vec_res = vec_1.add(vec_2);
//     try std.testing.expectEqual(Vec3.new(5, 7, 9), vec_res);
//     const normalized: Vec3 = vec_1.normalize();
//     try std.testing.expectFmt("Vec3(0.27, 0.53, 0.80)", "{}", .{normalized});
// }

// test "lookAt" {}
