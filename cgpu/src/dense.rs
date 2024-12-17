#![allow(unused)]
use game::{brick::MaterialBrick, material::MaterialId, DenseBuffer};
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
        log::debug!("Resizing Buffer");
        let current_capacity = self.capacity.load(Ordering::Relaxed);
        let new_capacity = current_capacity.checked_mul(2).unwrap_or(current_capacity);

        if new_capacity < required_size {
            return false;
        }

        let new_buffer = Self::create_buffer(&self.device, new_capacity);

        let mut copy_command =
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Buffer Growth Copy Encoder"),
                });

        copy_command.copy_buffer_to_buffer(
            &self.buffer.read(),
            0,
            &new_buffer,
            0,
            self.current_offset.load(Ordering::SeqCst),
        );

        self.queue.submit(Some(copy_command.finish()));

        *self.buffer.write() = new_buffer;
        self.capacity.store(new_capacity, Ordering::Release);

        on_resize(&self.buffer.read());

        true
    }

    fn try_shrink<F: FnOnce(&wgpu::Buffer)>(&self, on_resize: F) -> bool {
        let current_offset = self.current_offset.load(Ordering::SeqCst);
        let current_capacity = self.capacity.load(Ordering::Relaxed);

        if current_offset >= current_capacity / 4 {
            return false;
        }

        let new_capacity = current_capacity / 2;
        if new_capacity < 1024 * 1024 {
            return false;
        }

        let new_buffer = Self::create_buffer(&self.device, new_capacity);

        let mut copy_command =
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Buffer Shrink Copy Encoder"),
                });

        copy_command.copy_buffer_to_buffer(&self.buffer.read(), 0, &new_buffer, 0, current_offset);

        self.queue.submit(Some(copy_command.finish()));

        *self.buffer.write() = new_buffer;
        self.capacity.store(new_capacity, Ordering::Release);

        on_resize(&self.buffer.read());

        true
    }

    pub fn allocate<T, F: FnOnce(&wgpu::Buffer)>(&self, on_resize: F) -> Option<u64> {
        let size = std::mem::size_of::<T>() as u64;

        if let Some(block) = self.find_free_block(size) {
            return Some(block.offset);
        }

        let current = self.current_offset.fetch_add(size, Ordering::SeqCst);
        let capacity = self.capacity.load(Ordering::Acquire);

        if current + size <= capacity {
            Some(current)
        } else {
            self.current_offset.fetch_sub(size, Ordering::SeqCst);

            if self.try_grow(current + size, on_resize) {
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

        self.try_shrink(on_resize);
    }

    pub fn write<T: bytemuck::Pod>(&self, offset: u64, data: &T) {
        self.queue.write_buffer(
            &self.buffer(),
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

    pub fn allocate_many<T, F: FnOnce(&wgpu::Buffer)>(
        &self,
        count: usize,
        on_resize: F,
    ) -> Option<Vec<u64>> {
        let size = std::mem::size_of::<T>() as u64;
        let total_size = size.checked_mul(count as u64)?;

        let mut offsets = Vec::with_capacity(count);
        {
            let mut free_blocks = self.free_blocks.write();
            if let Some((&_block_size, blocks)) = free_blocks.range_mut(size..).next() {
                while offsets.len() < count && !blocks.is_empty() {
                    offsets.push(blocks.remove(blocks.len() - 1).offset);
                }
            }
        }

        let remaining = count - offsets.len();
        if remaining > 0 {
            let remaining_size = size * remaining as u64;
            let current = self
                .current_offset
                .fetch_add(remaining_size, Ordering::SeqCst);
            let capacity = self.capacity.load(Ordering::Acquire);

            if current + remaining_size > capacity {
                self.current_offset
                    .fetch_sub(remaining_size, Ordering::SeqCst);

                if !self.try_grow(current + remaining_size, on_resize) {
                    return None;
                }

                let new_current = self
                    .current_offset
                    .fetch_add(remaining_size, Ordering::SeqCst);
                for i in 0..remaining {
                    offsets.push(new_current + (i as u64 * size));
                }
            } else {
                for i in 0..remaining {
                    offsets.push(current + (i as u64 * size));
                }
            }
        }

        Some(offsets)
    }

    pub fn write_many<T: bytemuck::Pod>(&self, offsets: &[u64], data: &[T]) {
        assert_eq!(offsets.len(), data.len(), "Offset and data length mismatch");

        for (offset, item) in offsets.iter().zip(data.iter()) {
            self.queue.write_buffer(
                &self.buffer.read(),
                *offset,
                bytemuck::cast_slice(std::slice::from_ref(item)),
            );
        }
        self.queue.submit([]);
    }

    pub fn allocate_and_write_many<T: bytemuck::Pod, F: FnOnce(&wgpu::Buffer)>(
        &self,
        data: &[T],
        on_resize: F,
    ) -> Option<Vec<u64>> {
        let offsets = self.allocate_many::<T, F>(data.len(), on_resize)?;
        self.write_many(&offsets, data);
        Some(offsets)
    }

    pub fn allocate_dense<T, F: FnOnce(&wgpu::Buffer)>(
        &self,
        count: usize,
        on_resize: F,
    ) -> Option<u64> {
        let size = std::mem::size_of::<T>() as u64;
        let total_size = size.checked_mul(count as u64)?;

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
            // Revert the offset change
            self.current_offset.fetch_sub(total_size, Ordering::SeqCst);

            // Try to grow the buffer
            if self.try_grow(current + total_size, on_resize) {
                let new_current = self.current_offset.fetch_add(total_size, Ordering::SeqCst);
                Some(new_current)
            } else {
                None
            }
        }
    }

    pub fn write_dense<T: bytemuck::Pod>(&self, offset: u64, data: &[T]) {
        self.queue
            .write_buffer(&self.buffer.read(), offset, bytemuck::cast_slice(data));
        self.queue.submit([]);
    }

    pub fn allocate_and_write_dense<T: bytemuck::Pod, F: FnOnce(&wgpu::Buffer)>(
        &self,
        data: &[T],
        on_resize: F,
    ) -> Option<u64> {
        let offset = self.allocate_dense::<T, F>(data.len(), on_resize)?;
        self.write_dense(offset, data);
        Some(offset)
    }

    pub fn deallocate_dense<T, F: FnOnce(&wgpu::Buffer)>(
        &self,
        offset: u64,
        count: usize,
        on_resize: F,
    ) {
        let size = std::mem::size_of::<T>() as u64;
        let total_size = size.checked_mul(count as u64).unwrap();

        let mut free_blocks = self.free_blocks.write();
        free_blocks
            .entry(total_size)
            .or_insert_with(Vec::new)
            .push(FreeBlock { offset });

        self.try_shrink(on_resize);
    }

    pub fn clear(&self) {
        self.current_offset.store(0, Ordering::SeqCst);

        self.free_blocks.write().clear();
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

    pub fn allocate_bricks<F: FnOnce(&wgpu::Buffer)>(
        &self,
        bricks: &[MaterialBrick],
        on_resize: F,
    ) -> Option<Vec<(u64, u64)>> {
        if bricks.is_empty() {
            return Some(Vec::new());
        }

        let total_size: u64 = bricks
            .iter()
            .map(|brick| match brick {
                MaterialBrick::Size1(_) => 64,  // 512 bits = 64 bytes
                MaterialBrick::Size2(_) => 128, // 1024 bits = 128 bytes
                MaterialBrick::Size4(_) => 256, // 2048 bits = 256 bytes
                MaterialBrick::Size8(_) => 512, // 4096 bits = 512 bytes
            })
            .sum();

        let base_offset = self.allocate_dense::<u8, F>(total_size as usize, on_resize)?;

        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Brick Staging Buffer"),
            size: total_size,
            usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::MAP_WRITE,
            mapped_at_creation: true,
        });

        let mut current_offset = 0;
        let mut offsets = Vec::with_capacity(bricks.len());
        {
            let mut staging_view = staging_buffer.slice(..).get_mapped_range_mut();

            for brick in bricks {
                let bits = brick.element_size();
                let size = match brick {
                    MaterialBrick::Size1(_) => 64,
                    MaterialBrick::Size2(_) => 128,
                    MaterialBrick::Size4(_) => 256,
                    MaterialBrick::Size8(_) => 512,
                };

                // Copy brick data to staging buffer
                staging_view[current_offset as usize..current_offset as usize + size as usize]
                    .copy_from_slice(brick.data());

                offsets.push((base_offset + current_offset, bits));
                current_offset += size;
            }
        }

        staging_buffer.unmap();

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Brick Copy Encoder"),
            });

        // Copy from staging buffer to main buffer
        encoder.copy_buffer_to_buffer(&staging_buffer, 0, self.buffer(), base_offset, total_size);

        // Submit copy command
        self.queue.submit(Some(encoder.finish()));

        Some(offsets)
    }

    pub fn allocate_palette<F: FnOnce(&wgpu::Buffer)>(
        &self,
        palette: &[MaterialId],
        on_resize: F,
    ) -> Option<u64> {
        self.allocate_and_write_dense(palette, on_resize)
    }

    pub fn reset_copy_from_cpu(&self, cpu_buffer: &DenseBuffer) {
        let mut buffer = self.buffer.write();
        let new = Self::create_buffer(&self.device, cpu_buffer.data().len() as u64);

        self.queue.write_buffer(&new, 0, &cpu_buffer.data());
        self.queue.submit([]);

        *buffer = new;
        self.current_offset.store(
            cpu_buffer.current_offset.load(Ordering::SeqCst) as u64,
            Ordering::SeqCst,
        );
    }

    pub fn copy_from_cpu(
        &self,
        cpu_buffer: &DenseBuffer,
        src_offset: usize,
        size: usize,
        dst_offset: u64,
    ) {
        self.queue.write_buffer(
            &self.buffer.read(),
            dst_offset,
            &cpu_buffer.data()[src_offset..src_offset + size],
        );
        self.queue.submit([]);
    }
}
