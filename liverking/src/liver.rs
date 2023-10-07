//! primary food source for those hard enough to live the ancestral life.
use crate::hunt;
use crate::natty;
use std::mem;

pub struct Liver<T> {
    pub allocation: *mut T,
    pub allocation_size: usize,
    pub capacity: usize,
    pub len: usize,
}

unsafe impl<T: Send> Send for Liver<T> {}
unsafe impl<T: Sync> Sync for Liver<T> {}

impl<T> Liver<T> {
    pub fn extract(capacity: usize, slices: usize) -> Self {
        let capacity_rounded = (capacity + slices - 1) / slices * slices;
        let allocation_size = capacity_rounded * mem::size_of::<T>();
        let allocation = hunt::memory(allocation_size) as _;
        return Self {
            allocation,
            allocation_size,
            capacity,
            len: 0,
        };
    }

    #[inline]
    pub fn take_piece<U: Copy>(&self, index: usize) -> U {
        return natty!(*(self.allocation.offset(index as isize) as *const U));
    }

    #[inline]
    pub fn take_piece_at<U: Copy>(&self, row: usize, col: usize, width: usize) -> U {
        let index = row * width + col;
        return self.take_piece(index);
    }

    #[inline]
    pub fn ptr(&self, index: usize) -> *mut T {
        return natty!(self.allocation.offset(index as isize));
    }

    #[inline]
    pub fn ptr_at(&self, row: usize, col: usize, width: usize) -> *mut T {
        let index = row * width + col;
        return self.ptr(index);
    }
}

impl<T> Drop for Liver<T> {
    fn drop(&mut self) {
        hunt::burn(self.allocation as _, self.allocation_size)
    }
}
