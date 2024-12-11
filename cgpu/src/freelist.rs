use parking_lot::RwLock;
use std::collections::VecDeque;
use wgpu::util::DeviceExt;

pub struct FreeListBuffer {
    buffer: RwLock<wgpu::Buffer>,
    free_list: RwLock<VecDeque<u32>>,
    element_size: u32,
    size: RwLock<u32>,
    capacity: RwLock<u32>,
    usage: wgpu::BufferUsages,
}

#[derive(Debug)]
pub enum BufferError {
    AllocationFailed,
    IndexOutOfBounds,
}

impl FreeListBuffer {
    pub fn new(
        device: &wgpu::Device,
        element_size: u32,
        initial_capacity: u32,
        usage: wgpu::BufferUsages,
    ) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Managed Buffer"),
            size: (element_size * initial_capacity) as u64,
            usage,
            mapped_at_creation: false,
        });

        Self {
            buffer: RwLock::new(buffer),
            free_list: RwLock::new(VecDeque::new()),
            element_size,
            size: RwLock::new(0),
            capacity: RwLock::new(initial_capacity),
            usage,
        }
    }

    pub fn allocate(&self) -> Result<u32, BufferError> {
        let mut free_list = self.free_list.write();

        if let Some(index) = free_list.pop_front() {
            return Ok(index);
        }

        let mut size = self.size.write();

        let capacity = *self.capacity.read();

        if *size >= capacity {
            return Err(BufferError::AllocationFailed);
        }

        let index = *size;
        *size += 1;
        Ok(index)
    }

    pub fn write(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        index: u32,
        data: &[u8],
    ) -> Result<(), BufferError> {
        // if data.len() != self.element_size as usize {
        // return Err(BufferError::IndexOutOfBounds);
        // }
        let capacity = *self.capacity.read();
        if index >= capacity {
            return Err(BufferError::IndexOutOfBounds);
        }

        let staging_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Staging Buffer"),
            contents: data,
            usage: wgpu::BufferUsages::COPY_SRC,
        });

        // Create command encoder and copy data
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Write Command Encoder"),
        });

        encoder.copy_buffer_to_buffer(
            &staging_buffer,
            0,
            &self.buffer.read(),
            (index * self.element_size) as u64,
            data.len() as u64,
        );

        queue.submit(std::iter::once(encoder.finish()));

        Ok(())
    }

    pub fn allocate_and_write(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        data: &[u8],
    ) -> Result<u32, BufferError> {
        let index = match self.allocate() {
            Ok(index) => index,
            _ => {
                let _ = self.try_grow(device, queue);
                self.allocate()?
            }
        };
        match self.write(device, queue, index, data) {
            Ok(_) => Ok(index),
            Err(e) => {
                // If write fails, we should add the index back to free list
                self.free_list.write().push_front(index);
                Err(e)
            }
        }
    }

    pub fn try_grow(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Result<(), BufferError> {
        let mut capacity = self.capacity.write();
        let old_capacity = *capacity;
        let new_capacity = old_capacity * 2;

        log::debug!("ManagedBuffer Grow: {} -> {}", old_capacity, new_capacity);

        let new_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Managed Buffer"),
            size: (self.element_size * new_capacity) as u64,
            usage: self.usage,
            mapped_at_creation: false,
        });

        // Copy old data to new buffer
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Grow Command Encoder"),
        });

        encoder.copy_buffer_to_buffer(
            &self.buffer.read(),
            0,
            &new_buffer,
            0,
            (self.element_size * old_capacity) as u64,
        );

        queue.submit(std::iter::once(encoder.finish()));

        *self.buffer.write() = new_buffer;
        *capacity = new_capacity;
        Ok(())
    }

    pub fn deallocate(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        index: u32,
    ) -> Result<(), BufferError> {
        let capacity = *self.capacity.read();
        if index >= capacity {
            return Err(BufferError::IndexOutOfBounds);
        }

        let mut free_list = self.free_list.write();
        free_list.push_back(index);

        // Check if we should shrink (25% utilization threshold)
        let used_count = capacity - free_list.len() as u32;
        if used_count < capacity / 4 && capacity > 1 {
            drop(free_list); // Release lock before shrinking
            self.try_shrink(device, queue)?;
        }

        Ok(())
    }

    fn try_shrink(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Result<(), BufferError> {
        let old_capacity = *self.capacity.read();
        let new_capacity = old_capacity / 2;

        let new_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Managed Buffer"),
            size: (self.element_size * new_capacity) as u64,
            usage: self.usage,
            mapped_at_creation: false,
        });

        // Copy data to new buffer, excluding freed elements
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Shrink Command Encoder"),
        });

        encoder.copy_buffer_to_buffer(
            &self.buffer.read(),
            0,
            &new_buffer,
            0,
            (self.element_size * new_capacity) as u64,
        );

        queue.submit(std::iter::once(encoder.finish()));

        *self.buffer.write() = new_buffer;
        *self.capacity.write() = new_capacity;

        // Update free list to remove indices beyond new capacity
        let mut free_list = self.free_list.write();
        free_list.retain(|&index| index < new_capacity);

        Ok(())
    }

    pub fn get_buffer(&self) -> &wgpu::Buffer {
        unsafe { self.buffer.data_ptr().as_ref().unwrap() }
    }

    pub fn get_capacity(&self) -> u32 {
        *self.capacity.read()
    }
}
