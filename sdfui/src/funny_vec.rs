//! high performant parallel vector implementation
use crate::salloc::{allocard, freebie};
use crate::{safe, UVec2};
use std::arch::x86_64::{__m256, _mm256_load_ps};
use std::mem::size_of;

/// Vec intended to be used for SIMD
/// this will will pad up to size_of::<T>() * 7 at the end
/// why do I allocate myself instead of using Vec<T>::with_capacity ?
/// I don't know either tbh. it just looks funny
/// and it gives me more allocation control
/// which might be important in C-FFI
/// when AVX512 becomes more mainstream increase padding?
pub struct FunnyVec<T> {
    pub allocation: *mut T,
    pub allocation_size: usize,
    pub capacity: usize,
    pub len: usize,
}

unsafe impl<T: Send> Send for FunnyVec<T> {}
unsafe impl<T: Sync> Sync for FunnyVec<T> {}

impl<T> FunnyVec<T> {
    pub fn with_capacity(capacity: usize) -> Self {
        let capacity_rounded = (capacity + 7) / 8 * 8;
        let allocation_size = capacity_rounded * size_of::<T>();
        let allocation = allocard(allocation_size) as _;
        return Self {
            allocation,
            allocation_size,
            capacity,
            len: 0,
        };
    }

    /// very safe quick transmute index
    #[inline]
    pub fn transmute_at<U: Copy>(&self, row: usize, col: usize, width: usize) -> U {
        let index = row * width + col ;
        safe! {
            let addr = self.allocation.offset(index as isize);
            *(addr as *const U)
        }
    }

    /// very safe
    #[inline]
    pub fn ptr_at(&self, row: usize, col: usize, width: usize) -> *mut T {
        let index = row * width + col ;
        return safe!(self.allocation.offset(index as isize));
    }
}

impl<T> Drop for FunnyVec<T> {
    fn drop(&mut self) {
        freebie(self.allocation as _, self.allocation_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rounder() {
        let cap_calc = |capacity| -> i32 { (capacity + 7) / 8 * 8 };
        let val = cap_calc(21);
        assert_eq!(val, 24);
    }

    #[test]
    fn create_vec() {
        let mut vec = FunnyVec::<f32>::with_capacity(4096);
    }
}
