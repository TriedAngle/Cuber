//! **blazingly fast** :rocket: :fire: SDF based UI primitives and their compose functions
use crate::funny_vec::FunnyVec;
use crate::{safe, UVec2, Vec2, RENDER_SIZE};
use std::arch::x86_64::*;
use rayon::iter::{ParallelIterator, IntoParallelIterator};
use crate::fearless::Boxy;

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

    pub fn new_circle(x: f32, y: f32, radius: f32) -> Self {
        let mut sdf = Self::new_empty(safe!(RENDER_SIZE));
        safe! {
            let x_offsets = [0.0f32, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
            let xos = _mm256_load_ps(x_offsets.as_ptr());
            let c_xs = _mm256_set1_ps(x);
            let c_ys = _mm256_set1_ps(y);
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

    pub fn new_box(x: f32, y: f32, width: f32, height: f32) -> Self {
        let mut sdf = Self::new_empty(safe!(RENDER_SIZE));
        safe! {
            let x_offsets = [0.0f32, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
            let xos = _mm256_load_ps(x_offsets.as_ptr());
            let b_x = _mm256_set1_ps(width / 2.0);
            let b_y = _mm256_set1_ps(height / 2.0);

            (0..sdf.height).into_par_iter().for_each(|row| {
                let p_ys = _mm256_set1_ps(row as f32 - y);
                for x_start in (0..sdf.width).step_by(8) {
                    let p_x = _mm256_add_ps(_mm256_set1_ps(x_start as f32 - x), xos);

                    let d_x = _mm256_sub_ps(_mm256_andnot_ps(_mm256_set1_ps(-0.0), p_x), b_x);
                    let d_y = _mm256_sub_ps(_mm256_andnot_ps(_mm256_set1_ps(-0.0), p_ys), b_y);

                    let max_d_x = _mm256_max_ps(d_x, _mm256_setzero_ps());
                    let max_d_y = _mm256_max_ps(d_y, _mm256_setzero_ps());

                    let length = _mm256_sqrt_ps(_mm256_add_ps(
                        _mm256_mul_ps(max_d_x, max_d_x),
                        _mm256_mul_ps(max_d_y, max_d_y),
                    ));
                    let max_d_xy = _mm256_max_ps(d_x, d_y);
                    let min_max_d_xy = _mm256_min_ps(max_d_xy, _mm256_setzero_ps());
                    let result = _mm256_add_ps(length, min_max_d_xy);

                    let ptr = sdf.underlying.ptr_at(row, x_start, sdf.width);
                    _mm256_storeu_ps(ptr, result);
                }
            });
        }
        return sdf;
    }
    //
    // pub fn new_box(x: f32, y: f32, radius: f32) -> Self {
    //     let mut sdf = Self::new_empty(safe!(RENDER_SIZE));
    //     safe! {
    //         let x_offsets = [0.0f32, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
    //         let xos = _mm256_load_ps(x_offsets.as_ptr());
    //         let c_xs = _mm256_set1_ps(x);
    //         let c_ys = _mm256_set1_ps(y);
    //         let rs = _mm256_set1_ps(radius);
    //         // iterate over rows and inside of them from left to right
    //         (0..sdf.height).into_par_iter().for_each(|row| {
    //             let p_ys = _mm256_set1_ps(row as f32);
    //             for x_start in (0..sdf.width).step_by(8) {
    //                 let p_x = _mm256_set1_ps(x_start as f32);
    //                 let p_xs = _mm256_add_ps(p_x, xos);
    //
    //                 let cp_xs = _mm256_sub_ps(p_xs, c_xs);
    //                 let cp_ys = _mm256_sub_ps(p_ys, c_ys);
    //
    //                 let cp_xs2 = _mm256_mul_ps(cp_xs, cp_xs);
    //                 let cp_ys2 = _mm256_mul_ps(cp_ys, cp_ys);
    //                 let cp_radicants = _mm256_add_ps(cp_xs2, cp_ys2);
    //                 let cp_lengths = _mm256_sqrt_ps(cp_radicants);
    //
    //                 let distances = _mm256_sub_ps(cp_lengths, rs);
    //                 let ptr = sdf.underlying.ptr_at(row, x_start, sdf.width);
    //                 _mm256_storeu_ps(ptr, distances);
    //             }
    //         })
    //     }
    //     return sdf;
    // }


    pub fn compose(&self, other: &SDF, data: Boxy, composer: fn(__m256, __m256, Boxy) -> __m256) -> SDF {
        let mut out = Self::new_empty(safe!(RENDER_SIZE));
        safe! {
            (0..out.height).into_par_iter().for_each(|row| {
                let p_ys = _mm256_set1_ps(row as f32);
                for x_start in (0..out.width).step_by(8) {
                    let ptr1 = self.underlying.ptr_at(row, x_start, out.width);
                    let ptr2 = other.underlying.ptr_at(row, x_start, out.width);
                    let vs1 = _mm256_loadu_ps(ptr1);
                    let vs2 = _mm256_loadu_ps(ptr2);
                    let res = composer(vs1, vs2, data);
                    let ptr = out.underlying.ptr_at(row, x_start, out.width);
                    _mm256_storeu_ps(ptr, res);
                }
            })
        }
        return out;
    }

    fn union(val1: __m256, val2: __m256, _: Boxy) -> __m256 {
        safe!(_mm256_min_ps(val1, val2))
    }

    fn intersect(val1: __m256, val2: __m256, _: Boxy) -> __m256 {
        safe!(_mm256_max_ps(val1, val2))
    }

    // fn smooth_union(val1: __m256, val2: __m256, data: Boxy) -> __m256 {
    //     let data = data.safe::<f32>();
    //     safe!{
    //         let k = _mm256_set1_ps(*data);
    //         let diffs = _mm256_sub_ps(val1, val2);
    //         let abs = _mm256_andnot_ps(_mm256_set1_ps(-0.0), diffs);
    //         let lhs = _mm256_sub_ps(k, abs);
    //         let h = _mm256_max_ps(lhs, _mm256_set1_ps(0.0));
    //
    //         let h2025 = _mm256_mul_ps(_mm256_mul_ps(h, h), _mm256_set1_ps(0.25));
    //         let subbers = _mm256_div_ps(h2025, k);
    //         let mins = _mm256_min_ps(val1, val2);
    //         let res = _mm256_sub_ps(mins, subbers);
    //         return res;
    //     }
    // }
    fn smooth_union(val1: __m256, val2: __m256, data: Boxy) -> __m256 {
        let data = data.safe::<f32>();
        let smax = |v1: f32, v2: f32, k: f32| -> f32 {
            ((v1*k).exp() + (v2 * k).exp()).ln() * k
        };
        let smin = |v1: f32, v2: f32, k: f32| -> f32 { -smax(-v1, -v2,k) };
        safe! {
            let k = *data;
            let mut val1_array = [0.0f32; 8];
            let mut val2_array = [0.0f32; 8];
            _mm256_storeu_ps(val1_array.as_mut_ptr(), val1);
            _mm256_storeu_ps(val2_array.as_mut_ptr(), val2);
            let res = [
                smin(val1_array[0], val2_array[0], k),
                smin(val1_array[1], val2_array[1], k),
                smin(val1_array[2], val2_array[2], k),
                smin(val1_array[3], val2_array[3], k),
                smin(val1_array[4], val2_array[4], k),
                smin(val1_array[5], val2_array[5], k),
                smin(val1_array[6], val2_array[6], k),
                smin(val1_array[7], val2_array[7], k),
            ];
            let result = _mm256_loadu_ps(res.as_ptr());
            return result;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use super::*;

    #[test]
    fn test_range() {
        let mut vec = Vec::new();
        for x in (0..16).step_by(4) {
            vec.push(x);
        }
        assert_eq!(vec, vec![0, 4, 8, 12])
    }

    // #[test]
    // fn create_vec() {
    //     let mut vec = FunnyVec::<f32>::with_capacity(4096);
    // }
    //
    // #[test]
    // fn create_circle() {
    //     let sdf = SDF::new_circle(600.0, 500.0, 10.0);
    // }
    //
    fn sdf_to_png(sdf: &SDF, path: impl AsRef<Path>) {
        use image::{GrayImage, Luma};
        let mut img = GrayImage::new(sdf.width as u32, sdf.height as u32);
        for y in 0..sdf.height {
            for x in 0..sdf.width {
                let val = safe!(sdf.underlying.transmute_at::<f32>(y, x, sdf.width));
                let pix = if val > 1.0 { 0u8 } else { 255u8 };
                img.put_pixel(x as u32, y as u32, Luma([pix]));
            }
        }
        img.save(path).unwrap();
    }
    //
    // #[test]
    // #[ignore]
    // fn create_and_write_circle() {
    //     use image::{GrayImage, Luma};
    //     use std::time::Instant;
    //
    //     let start = Instant::now();
    //     let sdf = SDF::new_circle(600.0, 500.0, 60.0);
    //     let duration = start.elapsed();
    //     println!("Took (create_write): {:?}", duration);
    //     sdf_to_png(&sdf, "tmp/circle.png")
    // }
    //
    // #[test]
    // #[ignore]
    // fn create_and_write_box() {
    //     use image::{GrayImage, Luma};
    //     use std::time::Instant;
    //     let start = Instant::now();
    //     let sdf = SDF::new_box(400.0, 500.0, 500.0, 300.0);
    //     let duration = start.elapsed();
    //     println!("Took (create_write): {:?}", duration);
    //     sdf_to_png(&sdf, "tmp/box.png")
    // }


    #[test]
    #[ignore]
    fn union_circle() {
        use image::{GrayImage, Luma};
        use std::time::Instant;
        let start = Instant::now();
        let sdf1 = SDF::new_box(400.0, 500.0, 500.0, 300.0);
        let sdf2 = SDF::new_circle(680.0, 560.0, 40.0);
        let sdf = SDF::compose(&sdf1, &sdf2, Boxy::null(), SDF::union);
        let duration = start.elapsed();
        println!("Took (union): {:?}", duration);
        sdf_to_png(&sdf, "tmp/union.png")
    }
    //
    // #[test]
    // #[ignore]
    // fn smooth_union() {
    //     use image::{GrayImage, Luma};
    //     use std::time::Instant;
    //     let start = Instant::now();
    //     let sdf1 = SDF::new_box(400.0, 500.0, 500.0, 300.0);
    //     let sdf2 = SDF::new_circle(680.0, 560.0, 40.0);
    //     let smooth: f32 = 0.6;
    //     let sdf = SDF::compose(&sdf1, &sdf2, Boxy::from(&smooth), SDF::smooth_union);
    //     let duration = start.elapsed();
    //     println!("Took (union): {:?}", duration);
    //     sdf_to_png(&sdf, "tmp/smooth_union.png")
    // }
}
