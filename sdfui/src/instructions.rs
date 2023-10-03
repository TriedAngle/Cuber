use std::arch::x86_64::*;

// macro_rules! avx2_impl {
//     (binop => $name:ident, $op:ident) => {
//         pub unsafe fn $name(slice1: &[f32], slice2: &[f32]) -> [f32; 8] {
//             avx2_impl!(slice1, slice2, $op)
//         }
//     };
//     ($slice1:expr, $slice2:expr, $op:ident) => {{
//         let reg1 = _mm256_loadu_ps($slice1.as_ptr());
//         let reg2 = _mm256_loadu_ps($slice2.as_ptr());
//         let res = $op(reg1, reg2);
//         let mut result = [0.0f32; 8];
//         _mm256_storeu_ps(result.as_mut_ptr(), res);
//         result
//     }};
// }
//
// avx2_impl!(binop => add_f32x8, _mm256_add_ps);
// avx2_impl!(binop => add_f32x8, _mm256_sub_ps);
// avx2_impl!(binop => add_f32x8, _mm256_mul_ps);
// avx2_impl!(binop => add_f32x8, _mm256_div_ps);
//
// avx2_impl!(binop => add_f32x8, _mm256_min_ps);
// avx2_impl!(binop => add_f32x8, _mm256_max_ps);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::safe;

    #[test]
    fn test_min() {
        safe! {
            let array1 = [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 1.0];
            let array2 = [8.0f32, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 3.0];
            let xs1 = _mm256_loadu_ps(array1.as_ptr());
            let xs2 = _mm256_loadu_ps(array2.as_ptr());
            let res = _mm256_min_ps(xs1, xs2);
            let mut result = [0.0f32; 8];
            _mm256_storeu_ps(result.as_mut_ptr(), res);
            assert_eq!(result, [1.0f32, 2.0, 3.0, 4.0, 4.0, 3.0, 2.0, 1.0]);
        }
    }

    #[test]
    fn test_add() {
        safe! {
            let array1 = vec![1.0f32, 2.0, 3.0, 2.0, 5.0, 6.0, 1.0, 8.0];
            let array2 = vec![8.0f32, 7.0, 6.0, 5.0, 5.0, 3.0, 2.0, 8.0];
            let xs1 = _mm256_loadu_ps(array1.as_ptr());
            let xs2 = _mm256_loadu_ps(array2.as_ptr());
            let res = _mm256_add_ps(xs1, xs2);
            let mut result = [0.0f32; 8];
            _mm256_storeu_ps(result.as_mut_ptr(), res);
            assert_eq!(result, [9.0f32, 9.0, 9.0, 7.0, 10.0, 9.0, 3.0, 16.0]);
        }
    }
}
