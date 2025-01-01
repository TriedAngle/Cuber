use parking_lot::RwLock;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::brick::MaterialBrick;

#[derive(Debug, Clone, Copy)]
struct FreeBlock {
    offset: usize,
}

pub struct DenseBuffer {
    buffer: RwLock<Vec<u8>>,
    pub current_offset: AtomicUsize,
    pub capacity: AtomicUsize,
    free_blocks: RwLock<BTreeMap<usize, Vec<FreeBlock>>>,
}

impl DenseBuffer {
    pub fn new(initial_capacity: usize) -> Self {
        Self {
            buffer: RwLock::new(vec![0; initial_capacity]),
            current_offset: AtomicUsize::new(0),
            capacity: AtomicUsize::new(initial_capacity),
            free_blocks: RwLock::new(BTreeMap::new()),
        }
    }

    fn try_grow(&self, required_size: usize) -> bool {
        let current_capacity = self.capacity.load(Ordering::Relaxed);
        let new_capacity = current_capacity.checked_mul(2).unwrap_or(current_capacity);

        if new_capacity < required_size {
            return false;
        }

        let mut buffer = self.buffer.write();
        buffer.resize(new_capacity, 0);
        self.capacity.store(new_capacity, Ordering::Release);

        true
    }

    fn try_shrink(&self) -> bool {
        let current_offset = self.current_offset.load(Ordering::SeqCst);
        let current_capacity = self.capacity.load(Ordering::Relaxed);

        if current_offset >= current_capacity / 4 {
            return false;
        }

        let new_capacity = current_capacity / 2;
        if new_capacity < 1024 * 1024 {
            return false;
        }

        let mut buffer = self.buffer.write();
        buffer.truncate(new_capacity);
        buffer.shrink_to_fit();
        self.capacity.store(new_capacity, Ordering::Release);

        true
    }

    pub fn allocate<T>(&self) -> Option<usize> {
        let size = std::mem::size_of::<T>();

        if let Some(block) = self.find_free_block(size) {
            return Some(block.offset);
        }

        let current = self.current_offset.fetch_add(size, Ordering::SeqCst);
        let capacity = self.capacity.load(Ordering::Acquire);

        if current + size <= capacity {
            Some(current)
        } else {
            self.current_offset.fetch_sub(size, Ordering::SeqCst);

            if self.try_grow(current + size) {
                let new_current = self.current_offset.fetch_add(size, Ordering::SeqCst);
                Some(new_current)
            } else {
                None
            }
        }
    }

    fn find_free_block(&self, size: usize) -> Option<FreeBlock> {
        let mut free_blocks = self.free_blocks.write();

        if let Some((&_block_size, blocks)) = free_blocks.range_mut(size..).next() {
            if !blocks.is_empty() {
                return Some(blocks.remove(blocks.len() - 1));
            }
        }
        None
    }

    pub fn deallocate<T>(&self, offset: usize) {
        let size = std::mem::size_of::<T>();
        let mut free_blocks = self.free_blocks.write();

        free_blocks
            .entry(size)
            .or_insert_with(Vec::new)
            .push(FreeBlock { offset });

        self.try_shrink();
    }

    
    pub fn deallocate_size(&self, offset: usize, size: usize) {
        let mut free_blocks = self.free_blocks.write();

        free_blocks
            .entry(size)
            .or_insert_with(Vec::new)
            .push(FreeBlock { offset });

        self.try_shrink();
    }

    pub fn write<T: Copy>(&self, offset: usize, data: &T) {
        let size = std::mem::size_of::<T>();
        let buffer = self.buffer.read();

        unsafe {
            std::ptr::copy_nonoverlapping(
                data as *const T as *const u8,
                buffer.as_ptr().add(offset) as *mut u8,
                size,
            );
        }
    }

    pub fn read<T: Copy>(&self, offset: usize) -> T {
        let size = std::mem::size_of::<T>();
        let buffer = self.buffer.read();
        let mut result = std::mem::MaybeUninit::uninit();

        unsafe {
            std::ptr::copy_nonoverlapping(
                buffer.as_ptr().add(offset),
                result.as_mut_ptr() as *mut u8,
                size,
            );
            result.assume_init()
        }
    }

    pub fn allocate_and_write<T: Copy>(&self, data: &T) -> Option<usize> {
        let offset = self.allocate::<T>()?;
        self.write(offset, data);
        Some(offset)
    }

    pub fn allocate_many<T>(&self, count: usize) -> Option<Vec<usize>> {
        let size = std::mem::size_of::<T>();
        let total_size = size.checked_mul(count)?;

        let mut offsets = Vec::with_capacity(count);
        {
            let mut free_blocks = self.free_blocks.write();
            if let Some((&_block_size, blocks)) = free_blocks.range_mut(total_size..).next() {
                while offsets.len() < count && !blocks.is_empty() {
                    offsets.push(blocks.remove(blocks.len() - 1).offset);
                }
            }
        }

        let remaining = count - offsets.len();
        if remaining > 0 {
            let remaining_size = size * remaining;
            let current = self
                .current_offset
                .fetch_add(remaining_size, Ordering::SeqCst);
            let capacity = self.capacity.load(Ordering::Acquire);

            if current + remaining_size > capacity {
                self.current_offset
                    .fetch_sub(remaining_size, Ordering::SeqCst);

                if !self.try_grow(current + remaining_size) {
                    return None;
                }

                let new_current = self
                    .current_offset
                    .fetch_add(remaining_size, Ordering::SeqCst);
                for i in 0..remaining {
                    offsets.push(new_current + (i * size));
                }
            } else {
                for i in 0..remaining {
                    offsets.push(current + (i * size));
                }
            }
        }

        Some(offsets)
    }

    pub fn write_many<T: Copy>(&self, offsets: &[usize], data: &[T]) {
        assert_eq!(offsets.len(), data.len(), "Offset and data length mismatch");
        let size = std::mem::size_of::<T>();
        let buffer = self.buffer.read();

        for (offset, item) in offsets.iter().zip(data.iter()) {
            unsafe {
                std::ptr::copy_nonoverlapping(
                    item as *const T as *const u8,
                    buffer.as_ptr().add(*offset) as *mut u8,
                    size,
                );
            }
        }
    }

    pub fn allocate_and_write_many<T: Copy>(&self, data: &[T]) -> Option<Vec<usize>> {
        let offsets = self.allocate_many::<T>(data.len())?;
        self.write_many(&offsets, data);
        Some(offsets)
    }

    pub fn allocate_dense<T>(&self, count: usize) -> Option<usize> {
        let size = std::mem::size_of::<T>();
        let total_size = size.checked_mul(count)?;

        {
            let mut free_blocks = self.free_blocks.write();
            if let Some((&_block_size, blocks)) = free_blocks.range_mut(total_size..).next() {
                if !blocks.is_empty() {
                    return Some(blocks.remove(blocks.len() - 1).offset);
                }
            }
        }

        let current = self.current_offset.fetch_add(total_size, Ordering::SeqCst);
        let capacity = self.capacity.load(Ordering::Acquire);

        if current + total_size <= capacity {
            Some(current)
        } else {
            self.current_offset.fetch_sub(total_size, Ordering::SeqCst);

            if self.try_grow(current + total_size) {
                let new_current = self.current_offset.fetch_add(total_size, Ordering::SeqCst);
                Some(new_current)
            } else {
                None
            }
        }
    }

    pub fn write_dense<T: Copy>(&self, offset: usize, data: &[T]) {
        let size = std::mem::size_of::<T>() * data.len();
        let buffer = self.buffer.read();

        unsafe {
            std::ptr::copy_nonoverlapping(
                data.as_ptr() as *const u8,
                buffer.as_ptr().add(offset) as *mut u8,
                size,
            );
        }
    }

    pub fn data(&self) -> &[u8] {
        unsafe { &*self.buffer.data_ptr().as_ref().unwrap() }
    }

    pub fn data_mut(&self) -> &mut [u8] {
        unsafe { &mut *self.buffer.data_ptr().as_mut().unwrap() }
    }

    pub fn allocate_and_write_dense<T: Copy>(&self, data: &[T]) -> Option<usize> {
        let offset = self.allocate_dense::<T>(data.len())?;
        self.write_dense(offset, data);
        Some(offset)
    }

    pub fn deallocate_dense<T>(&self, offset: usize, count: usize) {
        let size = std::mem::size_of::<T>();
        let total_size = size.checked_mul(count).unwrap();

        let mut free_blocks = self.free_blocks.write();
        free_blocks
            .entry(total_size)
            .or_insert_with(Vec::new)
            .push(FreeBlock { offset });

        self.try_shrink();
    }

    pub fn clear(&self) {
        self.current_offset.store(0, Ordering::SeqCst);
        self.free_blocks.write().clear();
    }

    pub fn allocate_brick(&self, brick: MaterialBrick) -> Option<usize> {
        match brick {
            MaterialBrick::Size1(b) => self.allocate_and_write(&b),
            MaterialBrick::Size2(b) => self.allocate_and_write(&b),
            MaterialBrick::Size4(b) => self.allocate_and_write(&b),
            MaterialBrick::Size8(b) => self.allocate_and_write(&b),
        }
    }

    pub fn allocate_bricks(&self, bricks: &[MaterialBrick]) -> Option<Vec<(usize, u64)>> {
        if bricks.is_empty() {
            return Some(Vec::new());
        }

        let total_size: usize = bricks
            .iter()
            .map(|brick| match brick {
                MaterialBrick::Size1(_) => 64,
                MaterialBrick::Size2(_) => 128,
                MaterialBrick::Size4(_) => 256,
                MaterialBrick::Size8(_) => 512,
            })
            .sum();

        let base_offset = self.allocate_dense::<u8>(total_size)?;

        let mut current_offset = 0;
        let mut offsets = Vec::with_capacity(bricks.len());
        {
            let mut buffer = self.buffer.write(); // Get write access to buffer

            for brick in bricks {
                let bits = brick.element_size();
                let size = match brick {
                    MaterialBrick::Size1(_) => 64,
                    MaterialBrick::Size2(_) => 128,
                    MaterialBrick::Size4(_) => 256,
                    MaterialBrick::Size8(_) => 512,
                };

                // Copy brick data directly to our buffer
                buffer[base_offset + current_offset..base_offset + current_offset + size]
                    .copy_from_slice(brick.data());

                offsets.push((base_offset + current_offset, bits));
                current_offset += size;
            }
        }

        Some(offsets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_allocation() {
        let buffer = DenseBuffer::new(1024);

        // Allocate and write a value
        let value = 42u32;
        let offset = buffer.allocate_and_write(&value).unwrap();

        // Read it back
        let read_value: u32 = buffer.read(offset);
        assert_eq!(value, read_value);
    }

    #[test]
    fn test_multiple_allocations() {
        let buffer = DenseBuffer::new(1024);
        let values = vec![1u32, 2, 3, 4, 5];

        let offsets = buffer.allocate_and_write_many(&values).unwrap();

        // Read back and verify
        for (i, offset) in offsets.iter().enumerate() {
            let read_value: u32 = buffer.read(*offset);
            assert_eq!(values[i], read_value);
        }
    }

    #[test]
    fn test_deallocation_and_reuse() {
        let buffer = DenseBuffer::new(1024);

        // Allocate and deallocate
        let offset1 = buffer.allocate::<u32>().unwrap();
        buffer.deallocate::<u32>(offset1);

        // New allocation should reuse the space
        let offset2 = buffer.allocate::<u32>().unwrap();
        assert_eq!(offset1, offset2);
    }
}
