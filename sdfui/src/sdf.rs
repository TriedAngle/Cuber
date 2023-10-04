//! **blazingly fast** :rocket: :fire: SDF based UI primitives and their compose functions
use crate::funny_vec::FunnyVec;
use crate::{safe, UVec2, Vec2, RENDER_SIZE};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::arch::x86_64::*;
use std::ptr;

pub struct SDF {
    pub underlying: FunnyVec<f32>,
    pub x: isize,
    pub y: isize,
    pub width: usize,
    pub height: usize,
}

impl SDF {
    pub fn new_empty(x: isize, y: isize, width: usize, height: usize) -> Self {
        return Self {
            underlying: FunnyVec::with_capacity(width * height),
            x,
            y,
            width,
            height,
        };
    }

    pub fn new_by_bounds(sdf1: &SDF, sdf2: &SDF) -> Self {
        let min_x = isize::min(sdf1.x, sdf2.x);
        let min_y = isize::min(sdf1.y, sdf2.y);
        let max_x = usize::max(sdf1.x as usize + sdf1.width, sdf2.x as usize + sdf2.width);
        let max_y = usize::max(sdf1.y as usize + sdf1.height, sdf2.y as usize + sdf2.height);

        return Self::new_empty(min_x, min_y, max_x - min_x as usize, max_y - min_y as usize);
    }

    pub fn new_circle(center: Vec2, radius: f32) -> Self {
        let width = radius as usize * 3;
        let height = radius as usize * 3;
        let offset_x = (center.x - radius * 1.5) as isize;
        let offset_y = (center.y - radius * 1.5) as isize;
        let mut sdf = Self::new_empty(offset_x, offset_y, width, height);
        safe! {
            let x_offsets = [0.0f32, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
            let xos = _mm256_load_ps(x_offsets.as_ptr());
            let c_xs = _mm256_set1_ps(center.x);
            let c_ys = _mm256_set1_ps(center.y);
            let rs = _mm256_set1_ps(radius);
            // iterate over rows and inside of them from left to right
            (0..sdf.height).into_par_iter().for_each(|row| {
                let actual_y = row + offset_y as usize;
                let p_ys = _mm256_set1_ps(actual_y as f32);
                for x_start in (0..sdf.width).step_by(8) {
                    let actual_x = x_start + offset_x as usize;
                    let p_x = _mm256_set1_ps(actual_x as f32);
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
            });
        }
        return sdf;
    }

    pub fn compose(sdf1: &SDF, sdf2: &SDF, f: fn(sdf1: &SDF, sdf2: &SDF, out: &mut SDF)) -> Self {
        let mut out = SDF::new_by_bounds(sdf1, sdf2);
        f(sdf1, sdf2, &mut out);
        return out;
    }

    fn min(sdf1: &SDF, sdf2: &SDF, out: &mut SDF) {
        let (sdf1_min_x, sdf1_max_x) = (sdf1.x as usize, sdf1.x as usize + sdf1.width);
        let (sdf1_min_y, sdf1_max_y) = (sdf1.y as usize, sdf1.y as usize + sdf1.height);
        let (sdf2_min_x, sdf2_max_x) = (sdf2.x as usize, sdf2.x as usize + sdf2.width);
        let (sdf2_min_y, sdf2_max_y) = (sdf2.y as usize, sdf2.y as usize + sdf2.height);
        safe! {
            let fake_distances = [100.0f32, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0];
            let x_offsets = [0.0f32, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
            let xos = _mm256_load_ps(x_offsets.as_ptr());
            (0..out.height).into_par_iter().for_each(|row| {
                let out_y = row + out.y as usize;
                let y_in_sdf1 = out_y >= sdf1_min_y && out_y < sdf1_max_y;
                let y_in_sdf2 = out_y >= sdf2_min_y && out_y < sdf2_max_y;

                for x_start in (0..out.width).step_by(8) {
                    let out_x = x_start + out.x as usize;
                    let x_in_sdf1 = out_x >= sdf1_min_x && out_x < sdf1_max_x;
                    let x_in_sdf2 = out_x >= sdf2_min_x && out_x < sdf2_max_x;
                    let in_sdf1 = x_in_sdf1 && y_in_sdf1;
                    let in_sdf2 = x_in_sdf2 && y_in_sdf2;

                    if in_sdf1 && in_sdf2 {
                        let sdf1_x = out_x - sdf1_min_x ;
                        let sdf1_y = out_y - sdf1_min_y ;
                        let sdf2_x = out_x - sdf2_min_x ;
                        let sdf2_y = out_y - sdf2_min_y ;
                        let ptr1 = sdf1.underlying.ptr_at(sdf1_y, sdf1_x, sdf1.width);
                        let ptr2 = sdf2.underlying.ptr_at(sdf2_y, sdf2_x, sdf2.width);
                        let vs1 = _mm256_loadu_ps(ptr1);
                        let vs2 = _mm256_loadu_ps(ptr2);
                        let mins = _mm256_min_ps(vs1, vs2);
                        let out_ptr = out.underlying.ptr_at(row, x_start, out.width);
                        _mm256_storeu_ps(out_ptr, mins);
                    } else if (in_sdf1) || (in_sdf2) {
                        if (in_sdf1) {
                            let sdf1_x = out_x - sdf1_min_x ;
                            let sdf1_y = out_y - sdf1_min_y ;
                            ptr::copy_nonoverlapping(
                                sdf1.underlying.ptr_at(sdf1_y, sdf1_x, sdf1.width),
                                out.underlying.ptr_at(row, x_start, out.width),
                                8
                            );
                        } else {
                            let sdf2_x = out_x - sdf2_min_x ;
                            let sdf2_y = out_y - sdf2_min_y ;
                            ptr::copy_nonoverlapping(
                                sdf2.underlying.ptr_at(sdf2_y, sdf2_x, sdf2.width),
                                out.underlying.ptr_at(row, x_start, out.width),
                                8
                            );
                        }
                    } else {
                        ptr::copy_nonoverlapping(
                            fake_distances.as_ptr(),
                            out.underlying.ptr_at(row, x_start, out.width),
                            8
                        );
                    }
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

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

    fn sdf_to_png(sdf: &SDF, path: impl AsRef<Path>) {
        use image::{GrayImage, Luma};
        let mut img = GrayImage::new(sdf.width as u32, sdf.height as u32);
        for y in 0..sdf.height {
            for x in 0..sdf.width {
                let val = safe!(sdf.underlying.transmute_at::<f32>(y, x, sdf.width));
                let pix = if val > 0.0 { 0u8 } else { 255u8 };
                img.put_pixel(x as u32, y as u32, Luma([pix]));
            }
        }
        img.save(path).unwrap();
    }

    #[test]
    #[ignore]
    fn create_and_write_circle() {
        use std::time::Instant;
        let start = Instant::now();
        let sdf = SDF::new_circle((600.0, 500.0).into(), 60.0);
        let duration = start.elapsed();
        println!("Took (create_write): {:?}", duration);
        sdf_to_png(&sdf, "tmp/circle.png")
    }

    #[test]
    #[ignore]
    fn compose_circles() {
        use std::time::Instant;
        let start = Instant::now();
        let sdf1 = SDF::new_circle((600.0, 500.0).into(), 60.0);
        let sdf2 = SDF::new_circle((670.0, 550.0).into(), 30.0);
        let sdf = SDF::compose(&sdf1, &sdf2, SDF::min);
        let duration = start.elapsed();
        println!("Took (compose): {:?}", duration);
        sdf_to_png(&sdf1, "tmp/compose_circle1.png");
        sdf_to_png(&sdf2, "tmp/compose_circle2.png");
        sdf_to_png(&sdf, "tmp/compose_circle3RES.png");
    }
}
// this is what the api might look like at the end? idk yet
// UI::root(1920, 1080)
//     .add(compose(
//         UI::rect(400, 600, 200, 200).color([1.0, 0.0, 0.0, 1.0]),
//         UI::circle(600, 600, 10).color([0.0, 1.0, 0.0, 1.0])
//     ), smooth_union)
//     .add(UI::text("This is a test", 300, 200, 20, FF::Default))
