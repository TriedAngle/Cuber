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
                    inline for (0..n) |k| {
                        sum += a.inner[k][col] * b.inner[row][k];
                    }
                    result.inner[row][col] = sum;
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
                result += a.inner[col][0] * b.inner[col][0];
            }
            return result;
        }

        pub fn length_squared(a: Self) f32 {
            return a.dot(a);
        }

        pub fn length(a: Self) f32 {
            return @sqrt(a.length_squared());
        }
    };
}

pub const Vec2 = struct {
    const Self = @This();
    inner: [1][2]f32,
    usingnamespace implMatrix(Self, 2, 1);
    usingnamespace implVector(Self, 2);

    pub fn new(x: f32, y: f32) Self {
        return .{ .inner = .{.{ x, y }} };
    }

    pub fn gx(a: Self) f32 {
        return a.inner[0][0];
    }
    pub fn gy(a: Self) f32 {
        return a.inner[1][0];
    }
};

pub const Vec3 = struct {
    const Self = @This();
    inner: [1][3]f32,
    usingnamespace implMatrix(Self, 3, 1);
    usingnamespace implVector(Self, 3);

    pub fn new(x: f32, y: f32, z: f32) Self {
        return .{ .inner = .{.{ x, y, z }} };
    }

    pub fn gx(a: Self) f32 {
        return a.inner[0][0];
    }
    pub fn gy(a: Self) f32 {
        return a.inner[1][0];
    }
    pub fn gz(a: Self) f32 {
        return a.inner[2][0];
    }
};

pub const Vec4 = struct {
    const Self = @This();
    inner: [1][4]f32,
    usingnamespace implMatrix(Self, 4, 1);
    usingnamespace implVector(Self, 4);

    pub fn new(x: f32, y: f32, z: f32, w: f32) Self {
        return .{ .inner = .{.{ x, y, z, w }} };
    }

    pub fn gx(a: Self) f32 {
        return a.inner[0][0];
    }
    pub fn gy(a: Self) f32 {
        return a.inner[1][0];
    }
    pub fn gz(a: Self) f32 {
        return a.inner[2][0];
    }
    pub fn gw(a: Self) f32 {
        return a.inner[3][0];
    }
};

pub const Matrix2x2 = struct {
    const Self = @This();
    inner: [2][2]f32, // [column][row] / [n][m]
    usingnamespace implMatrix(Self, 2, 2);
    usingnamespace implSquareMatrix(Self, 2);
};

pub const Matrix3x3 = struct {
    const Self = @This();
    inner: [3][3]f32, // [column][row] / [n][m]
    usingnamespace implMatrix(Self, 3, 3);
    usingnamespace implSquareMatrix(Self, 3);
};

pub const Matrix4x4 = struct {
    const Self = @This();
    inner: [4][4]f32, // [column][row] / [n][m]
    usingnamespace implMatrix(Self, 4, 4);
    usingnamespace implSquareMatrix(Self, 4);
};

test "math" {
    const mat1 = Matrix2x2.new_raw(.{ .{ 1, 2 }, .{ 3, 4 } });
    const mat2 = Matrix2x2.new_raw(.{ .{ 0, 4 }, .{ 2, 4 } });
    const mat_res = mat1.add(mat2);
    try std.testing.expectEqual(mat_res, Matrix2x2.new_raw(.{ .{ 1, 6 }, .{ 5, 8 } }));
    const vec1 = Vec3.new(1, 2, 3);
    const vec2 = Vec3.new(4, 5, 6);
    const vec_res = vec1.add(vec2);
    try std.testing.expectEqual(Vec3.new(5, 7, 9), vec_res);
}
