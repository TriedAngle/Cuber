#![allow(unused)]
use game::brick::MaterialBrick;
use parking_lot::RwLock;
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
use wgpu;

#[derive(Debug, Clone, Copy)]
struct FreeBlock {
    offset: u64,
}

pub struct GPUDenseBuffer {
    buffer: RwLock<wgpu::Buffer>,
    current_offset: AtomicU64,
    capacity: AtomicU64,
    free_blocks: RwLock<BTreeMap<u64, Vec<FreeBlock>>>,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
}

impl GPUDenseBuffer {
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>, capacity: u64) -> Self {
        let buffer = Self::create_buffer(&device, capacity);

        Self {
            buffer: RwLock::new(buffer),
            current_offset: AtomicU64::new(0),
            capacity: AtomicU64::new(capacity),
            free_blocks: RwLock::new(BTreeMap::new()),
            device,
            queue,
        }
    }

    fn create_buffer(device: &wgpu::Device, capacity: u64) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dense Buffer Allocator"),
            size: capacity,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        })
    }

    fn try_grow<F: FnOnce(&wgpu::Buffer)>(&self, required_size: u64, on_resize: F) -> bool {
        let current_capacity = self.capacity.load(Ordering::Relaxed);
        let new_capacity = current_capacity.checked_mul(2).unwrap_or(current_capacity);

        if new_capacity < required_size {
            return false;
        }

        // Create new buffer
        let new_buffer = Self::create_buffer(&self.device, new_capacity);

        // Copy existing data
        let mut copy_command =
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Buffer Growth Copy Encoder"),
                });

        // Copy old data to new buffer
        copy_command.copy_buffer_to_buffer(
            &self.buffer.read(),
            0,
            &new_buffer,
            0,
            self.current_offset.load(Ordering::SeqCst),
        );

        // Submit copy command
        self.queue.submit(Some(copy_command.finish()));

        // Update buffer and capacity
        *self.buffer.write() = new_buffer;
        self.capacity.store(new_capacity, Ordering::Release);

        // Notify caller about the buffer change
        on_resize(&self.buffer.read());

        true
    }

    fn try_shrink<F: FnOnce(&wgpu::Buffer)>(&self, on_resize: F) -> bool {
        let current_offset = self.current_offset.load(Ordering::SeqCst);
        let current_capacity = self.capacity.load(Ordering::Relaxed);

        // Only shrink if we're using less than 25% of the buffer
        if current_offset >= current_capacity / 4 {
            return false;
        }

        let new_capacity = current_capacity / 2;
        // Don't shrink below some minimum size (e.g., 1MB)
        if new_capacity < 1024 * 1024 {
            return false;
        }

        // Create new buffer
        let new_buffer = Self::create_buffer(&self.device, new_capacity);

        // Copy existing data
        let mut copy_command =
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Buffer Shrink Copy Encoder"),
                });

        // Copy old data to new buffer
        copy_command.copy_buffer_to_buffer(&self.buffer.read(), 0, &new_buffer, 0, current_offset);

        // Submit copy command
        self.queue.submit(Some(copy_command.finish()));

        // Update buffer and capacity
        *self.buffer.write() = new_buffer;
        self.capacity.store(new_capacity, Ordering::Release);

        // Notify caller about the buffer change
        on_resize(&self.buffer.read());

        true
    }

    pub fn allocate<T, F: FnOnce(&wgpu::Buffer)>(&self, on_resize: F) -> Option<u64> {
        let size = std::mem::size_of::<T>() as u64;

        if let Some(block) = self.find_free_block(size) {
            return Some(block.offset);
        }

        // Atomically increment the offset and get the previous value
        let current = self.current_offset.fetch_add(size, Ordering::SeqCst);
        let capacity = self.capacity.load(Ordering::Acquire);

        if current + size <= capacity {
            Some(current)
        } else {
            // Rollback the allocation
            self.current_offset.fetch_sub(size, Ordering::SeqCst);

            // Try to grow the buffer
            if self.try_grow(current + size, on_resize) {
                // Retry allocation after growth
                let new_current = self.current_offset.fetch_add(size, Ordering::SeqCst);
                Some(new_current)
            } else {
                None
            }
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

    pub fn deallocate<T, F: FnOnce(&wgpu::Buffer)>(&self, offset: u64, on_resize: F) {
        let size = std::mem::size_of::<T>() as u64;
        let mut free_blocks = self.free_blocks.write();

        free_blocks
            .entry(size)
            .or_insert_with(Vec::new)
            .push(FreeBlock { offset });

        // Try to shrink after deallocation
        self.try_shrink(on_resize);
    }

    pub fn write<T: bytemuck::Pod>(&self, offset: u64, data: &T) {
        self.queue.write_buffer(
            &self.buffer.read(),
            offset,
            bytemuck::cast_slice(std::slice::from_ref(data)),
        );
        self.queue.submit([]);
    }

    pub fn allocate_and_write<T: bytemuck::Pod, F: FnOnce(&wgpu::Buffer)>(
        &self,
        data: &T,
        on_resize: F,
    ) -> Option<u64> {
        let offset = self.allocate::<T, F>(on_resize)?;
        self.write(offset, data);
        Some(offset)
    }

    pub fn buffer(&self) -> &wgpu::Buffer {
        unsafe { self.buffer.data_ptr().as_ref().unwrap() }
    }

    pub fn allocate_brick<F: FnOnce(&wgpu::Buffer)>(
        &self,
        brick: MaterialBrick,
        on_resize: F,
    ) -> Option<u64> {
        match brick {
            MaterialBrick::Size1(b) => self.allocate_and_write(&b, on_resize),
            MaterialBrick::Size2(b) => self.allocate_and_write(&b, on_resize),
            MaterialBrick::Size4(b) => self.allocate_and_write(&b, on_resize),
            MaterialBrick::Size8(b) => self.allocate_and_write(&b, on_resize),
        }
    }
}
