use game::brick::{MaterialBrick, MaterialBrick1};
use parking_lot::RwLock;
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicU64, Ordering},
};
use wgpu;

#[derive(Debug, Clone, Copy)]
struct FreeBlock {
    offset: u64,
    size: u64,
}

pub struct DenseBuffer {
    buffer: wgpu::Buffer,
    current_offset: AtomicU64,
    capacity: u64,
    free_blocks: RwLock<BTreeMap<u64, Vec<FreeBlock>>>,
}

impl DenseBuffer {
    pub fn new(device: &wgpu::Device, capacity: u64) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Material Buffer Allocator"),
            size: capacity,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        Self {
            buffer,
            // Initialize atomic counter to 0
            current_offset: AtomicU64::new(0),
            capacity,
            free_blocks: RwLock::new(BTreeMap::new()),
        }
    }

    fn find_free_block(&self, size: u64) -> Option<FreeBlock> {
        let mut free_blocks = self.free_blocks.write();

        if let Some((&_block_size, blocks)) = free_blocks.range_mut(size..).next() {
            if !blocks.is_empty() {
                return Some(blocks.remove(blocks.len() - 1));
            }
        }
        None
    }

    pub fn allocate<T>(&self) -> Option<u64> {
        let size = std::mem::size_of::<T>() as u64;

        if let Some(block) = self.find_free_block(size) {
            return Some(block.offset);
        }

        // Atomically increment the offset and get the previous value
        let current = self.current_offset.fetch_add(size, Ordering::SeqCst);

        if current + size <= self.capacity {
            Some(current)
        } else {
            // Rollback the allocation if we exceeded capacity
            self.current_offset.fetch_sub(size, Ordering::SeqCst);
            None
        }
    }

    pub fn write<T: bytemuck::Pod>(&self, queue: &wgpu::Queue, offset: u64, data: &T) {
        queue.write_buffer(
            &self.buffer,
            offset,
            bytemuck::cast_slice(std::slice::from_ref(data)),
        );
        queue.submit([]);
    }

    pub fn allocate_and_write<T: bytemuck::Pod>(
        &self,
        queue: &wgpu::Queue,
        data: &T,
    ) -> Option<u64> {
        let offset = self.allocate::<T>()?;
        self.write(queue, offset, data);
        Some(offset)
    }

    pub fn deallocate<T>(&self, offset: u64) {
        let size = std::mem::size_of::<T>() as u64;
        let mut free_blocks = self.free_blocks.write();

        free_blocks
            .entry(size)
            .or_insert_with(Vec::new)
            .push(FreeBlock { offset, size });
    }

    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    pub fn allocate_brick(&self, brick: MaterialBrick, queue: &wgpu::Queue) -> Option<u64> {
        match brick {
            MaterialBrick::Size1(b) => self.allocate_and_write(queue, &b),
            MaterialBrick::Size2(b) => self.allocate_and_write(queue, &b),
            MaterialBrick::Size4(b) => self.allocate_and_write(queue, &b),
            MaterialBrick::Size8(b) => self.allocate_and_write(queue, &b),
        }
    }
}
