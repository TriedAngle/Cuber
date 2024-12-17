use parking_lot::RwLock;
use std::collections::VecDeque;
use std::marker::PhantomData;
use std::sync::Arc;
use wgpu::util::DeviceExt;

pub struct GPUFreeListBuffer<T: bytemuck::Pod + bytemuck::Zeroable> {
    buffer: RwLock<wgpu::Buffer>,
    free_list: RwLock<VecDeque<u64>>,
    size: RwLock<u64>,
    capacity: RwLock<u64>,
    usage: wgpu::BufferUsages,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    _phantom: PhantomData<T>,
}

#[allow(unused)]
impl<T: bytemuck::Pod + bytemuck::Zeroable> GPUFreeListBuffer<T> {
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        initial_capacity: u64,
        usage: wgpu::BufferUsages,
    ) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Managed Buffer"),
            size: (std::mem::size_of::<T>() as u64) * initial_capacity,
            usage,
            mapped_at_creation: false,
        });

        Self {
            buffer: RwLock::new(buffer),
            free_list: RwLock::new(VecDeque::new()),
            size: RwLock::new(0),
            capacity: RwLock::new(initial_capacity),
            usage,
            device,
            queue,
            _phantom: PhantomData,
        }
    }

    pub fn allocate<F>(&self, on_resize: F) -> Option<u64>
    where
        F: Fn(&wgpu::Buffer),
    {
        let mut free_list = self.free_list.write();

        if let Some(index) = free_list.pop_front() {
            return Some(index);
        }

        let mut size = self.size.write();
        let capacity = *self.capacity.read();

        if *size >= capacity {
            // Drop locks before trying to grow
            drop(free_list);
            drop(size);

            // Try to grow the buffer
            self.try_grow(on_resize)?;

            // Reacquire locks after growing
            size = self.size.write();
        }

        let index = *size;
        *size += 1;
        Some(index)
    }

    pub fn allocate_many<F>(&self, count: usize, on_resize: F) -> Option<Vec<u64>>
    where
        F: Fn(&wgpu::Buffer),
    {
        let mut indices = Vec::with_capacity(count);
        let mut free_list = self.free_list.write();
        let mut size = self.size.write();
        let mut capacity = *self.capacity.read();

        // First use any available slots from the free list
        while indices.len() < count && !free_list.is_empty() {
            indices.push(free_list.pop_front().unwrap());
        }

        let remaining = count - indices.len();
        while *size + remaining as u64 > capacity {
            // Drop locks before trying to grow
            drop(free_list);
            drop(size);

            // Try to grow the buffer
            self.try_grow(&on_resize)?;

            // Reacquire locks and update capacity
            free_list = self.free_list.write();
            size = self.size.write();
            capacity = *self.capacity.read();
        }

        // Allocate remaining indices from the end
        for _ in 0..remaining {
            indices.push(*size);
            *size += 1;
        }

        Some(indices)
    }

    pub fn write(&self, index: u64, data: &T) -> Option<()> {
        let capacity = *self.capacity.read();
        if index >= capacity {
            return None;
        }

        let bytes = bytemuck::bytes_of(data);
        let staging_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Staging Buffer"),
                contents: bytes,
                usage: wgpu::BufferUsages::COPY_SRC,
            });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Write Command Encoder"),
            });

        encoder.copy_buffer_to_buffer(
            &staging_buffer,
            0,
            &self.buffer.read(),
            index * std::mem::size_of::<T>() as u64,
            std::mem::size_of::<T>() as u64,
        );

        self.queue.submit(std::iter::once(encoder.finish()));
        Some(())
    }

    pub fn write_many(&self, offsets: &[u64], data: &[T]) -> Option<()> {
        if offsets.len() != data.len() {
            return None;
        }

        let capacity = *self.capacity.read();
        if offsets.iter().any(|&offset| offset >= capacity) {
            return None;
        }

        // Create a single staging buffer for all data
        let bytes: &[u8] = bytemuck::cast_slice(data);
        let staging_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Staging Buffer"),
                contents: bytes,
                usage: wgpu::BufferUsages::COPY_SRC,
            });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Write Many Command Encoder"),
            });

        let item_size = std::mem::size_of::<T>() as u64;

        // Copy each item from the appropriate offset in staging buffer to its destination
        for (i, &offset) in offsets.iter().enumerate() {
            encoder.copy_buffer_to_buffer(
                &staging_buffer,
                (i as u64) * item_size,
                &self.buffer.read(),
                offset * item_size,
                item_size,
            );
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        Some(())
    }

    pub fn allocate_write<F>(&self, data: &T, on_resize: F) -> Option<u64>
    where
        F: Fn(&wgpu::Buffer),
    {
        let index = self.allocate(on_resize)?;
        match self.write(index, data) {
            Some(_) => Some(index),
            None => {
                self.free_list.write().push_front(index);
                None
            }
        }
    }

    pub fn allocate_write_many<F>(&self, data: &[T], on_resize: F) -> Option<Vec<u64>>
    where
        F: Fn(&wgpu::Buffer),
    {
        let indices = self.allocate_many(data.len(), on_resize)?;
        match self.write_many(&indices, data) {
            Some(_) => Some(indices),
            None => {
                let mut free_list = self.free_list.write();
                for index in indices {
                    free_list.push_front(index);
                }
                None
            }
        }
    }

    pub fn try_grow<F>(&self, on_resize: F) -> Option<()>
    where
        F: Fn(&wgpu::Buffer),
    {
        let mut capacity = self.capacity.write();
        let old_capacity = *capacity;
        let new_capacity = old_capacity * 2;

        log::debug!("ManagedBuffer Grow: {} -> {}", old_capacity, new_capacity);

        let new_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Managed Buffer"),
            size: (std::mem::size_of::<T>() as u64) * new_capacity,
            usage: self.usage,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Grow Command Encoder"),
            });

        encoder.copy_buffer_to_buffer(
            &self.buffer.read(),
            0,
            &new_buffer,
            0,
            (std::mem::size_of::<T>() as u64) * old_capacity,
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        *self.buffer.write() = new_buffer;
        *capacity = new_capacity;
        on_resize(&self.buffer.read());
        Some(())
    }

    pub fn deallocate<F>(&self, index: u64, on_resize: F) -> Option<()>
    where
        F: Fn(&wgpu::Buffer),
    {
        let capacity = *self.capacity.read();
        if index >= capacity {
            return None;
        }

        let mut free_list = self.free_list.write();
        free_list.push_back(index);

        let used_count = capacity - free_list.len() as u64;
        if used_count < capacity / 4 && capacity > 1 {
            drop(free_list);
            self.try_shrink(on_resize)?;
        }

        Some(())
    }

    fn try_shrink<F>(&self, on_resize: F) -> Option<()>
    where
        F: Fn(&wgpu::Buffer),
    {
        let old_capacity = *self.capacity.read();
        let new_capacity = old_capacity / 2;

        let new_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Managed Buffer"),
            size: (std::mem::size_of::<T>() as u64) * new_capacity,
            usage: self.usage,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Shrink Command Encoder"),
            });

        encoder.copy_buffer_to_buffer(
            &self.buffer.read(),
            0,
            &new_buffer,
            0,
            (std::mem::size_of::<T>() as u64) * new_capacity,
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        *self.buffer.write() = new_buffer;
        *self.capacity.write() = new_capacity;

        on_resize(&self.buffer.read());

        let mut free_list = self.free_list.write();
        free_list.retain(|&index| index < new_capacity);

        Some(())
    }

    pub fn buffer(&self) -> &wgpu::Buffer {
        unsafe { self.buffer.data_ptr().as_ref().unwrap() }
    }

    pub fn get_capacity(&self) -> u64 {
        *self.capacity.read()
    }

    pub fn clear(&self) {
        let mut size = self.size.write();
        let mut free_list = self.free_list.write();
        *size = 0;

        free_list.clear();
    }
}
