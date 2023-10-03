//! **blazingly fast** :rocket: :fire: SDF based UI primitives and their compose functions
use crate::funny_vec::FunnyVec;
use crate::{safe, UVec2, Vec2, RENDER_SIZE};
use std::arch::x86_64::*;
use rayon::iter::{ParallelIterator, IntoParallelIterator};

pub struct SDF {
    pub underlying: FunnyVec<f32>,
    pub width: usize,
    pub height: usize,
}

impl SDF {
    pub fn new_empty(size: UVec2) -> Self {
        return Self {
            underlying: FunnyVec::with_capacity(size.x * size.y),
            width: size.x,
            height: size.y,
        };
    }

    pub fn new_circle(center: Vec2, radius: f32) -> Self {
        let mut sdf = Self::new_empty(safe!(RENDER_SIZE));
        safe! {
            let x_offsets = [0.0f32, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
            let xos = _mm256_load_ps(x_offsets.as_ptr());
            let c_xs = _mm256_set1_ps(center.x);
            let c_ys = _mm256_set1_ps(center.y);
            let rs = _mm256_set1_ps(radius);
            // iterate over rows and inside of them from left to right
            (0..sdf.height).into_par_iter().for_each(|row| {
                let p_ys = _mm256_set1_ps(row as f32);
                for x_start in (0..sdf.width).step_by(8) {
                    let p_x = _mm256_set1_ps(x_start as f32);
                    let p_xs = _mm256_add_ps(p_x, xos);

                    let cp_xs = _mm256_sub_ps(p_xs, c_xs);
                    let cp_ys = _mm256_sub_ps(p_ys, c_ys);

                    let cp_xs2 = _mm256_mul_ps(cp_xs, cp_xs);
                    let cp_ys2 = _mm256_mul_ps(cp_ys, cp_ys);
                    let cp_radicants = _mm256_add_ps(cp_xs2, cp_ys2);
                    let cp_lengths = _mm256_sqrt_ps(cp_radicants);

                    let distances = _mm256_sub_ps(cp_lengths, rs);
                    let ptr = sdf.underlying.ptr_at(row, x_start, sdf.width);
                    _mm256_storeu_ps(ptr, distances);
                }
            })
        }
        return sdf;
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_range() {
        let mut vec = Vec::new();
        for x in (0..16).step_by(4) {
            vec.push(x);
        }
        assert_eq!(vec, vec![0, 4, 8, 12])
    }

    #[test]
    fn create_vec() {
        let mut vec = FunnyVec::<f32>::with_capacity(4096);
    }

    #[test]
    fn create_circle() {
        let sdf = SDF::new_circle((600.0, 500.0).into(), 10.0);
    }
}

#[test]
#[ignore]
fn create_and_write_circle() {
    use image::{GrayImage, Luma};
    use std::time::Instant;

    let start = Instant::now();
    let sdf = SDF::new_circle((600.0, 500.0).into(), 60.0);
    let duration = start.elapsed();
    println!("Took: {:?}", duration);
    let mut img = GrayImage::new(sdf.width as u32, sdf.height as u32);
    for y in (0..sdf.height) {
        for x in (0..sdf.width) {
            let val = safe!(sdf.underlying.transmute_at::<f32>(y, x, sdf.width));
            let pix = if val > 0.0 { 0u8 } else { 255u8 };
            img.put_pixel(x as u32, y as u32, Luma([pix]));
        }
    }
    img.save("tmp/circle.png").unwrap();
}