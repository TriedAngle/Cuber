#![allow(unused)]

use std::convert::Into;

pub mod fearless;
pub mod funny_vec;
pub mod salloc;
pub mod sdf;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Vec2Base<T> {
    pub x: T,
    pub y: T,
}

impl<T> Vec2Base<T> {
    pub const fn new(x: T, y: T) -> Self {
        return Self { x, y };
    }
}

impl<T> From<(T, T)> for Vec2Base<T> {
    fn from(tuple: (T, T)) -> Self {
        Self {
            x: tuple.0,
            y: tuple.1,
        }
    }
}

pub type Vec2 = Vec2Base<f32>;
pub type IVec2 = Vec2Base<isize>;
pub type UVec2 = Vec2Base<usize>;

#[no_mangle]
pub static mut RENDER_SIZE: UVec2 = UVec2::new(1920, 1080);
