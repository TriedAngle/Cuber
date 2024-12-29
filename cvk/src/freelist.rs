use std::{
    cell::Cell,
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use ash::vk;
use parking_lot::{Mutex, RwLock};

use crate::{Buffer, BufferInfo, Device, Queue};

pub struct GPUFreeList {
    buffer: Cell<Buffer>,
    offset: AtomicU64,
    capacity: AtomicU64,
    free: RwLock<BTreeMap<u64, Vec<u64>>>,
    sharing: vk::SharingMode,
    lock: Mutex<()>,
    label: Option<String>,
    device: Arc<Device>,
    queue: Arc<Queue>,
}

impl GPUFreeList {
    pub fn new(
        device: Arc<Device>,
        queue: Arc<Queue>,
        capacity: u64,
        sharing: vk::SharingMode,
        label: Option<String>,
    ) -> Self {
        let buffer = Self::create_buffer(&device, capacity, sharing, label.as_deref());

        Self {
            buffer: Cell::new(buffer),
            offset: AtomicU64::new(0),
            capacity: AtomicU64::new(0),
            free: RwLock::new(BTreeMap::new()),
            sharing,
            lock: Mutex::new(()),
            label,
            device,
            queue,
        }
    }

    fn create_buffer(
        device: &Device,
        capacity: u64,
        sharing: vk::SharingMode,
        label: Option<&str>,
    ) -> Buffer {
        device.create_buffer(&BufferInfo {
            size: capacity,
            usage: vk::BufferUsageFlags::STORAGE_BUFFER
                | vk::BufferUsageFlags::TRANSFER_SRC
                | vk::BufferUsageFlags::TRANSFER_DST,
            usage_locality: vkm::MemoryUsage::AutoPreferDevice,
            sharing,
            allocation_locality: vk::MemoryPropertyFlags::DEVICE_LOCAL,
            host_access: None,
            label: label.as_deref(),
            tag: None,
        })
    }

    fn create_staging_buffer(device: &Device, size: u64, label: Option<&str>) -> Buffer {
        device.create_buffer(&BufferInfo {
            size,
            sharing: vk::SharingMode::EXCLUSIVE,
            usage: vk::BufferUsageFlags::TRANSFER_SRC,
            usage_locality: vkm::MemoryUsage::AutoPreferHost,
            allocation_locality: vk::MemoryPropertyFlags::HOST_VISIBLE
                | vk::MemoryPropertyFlags::HOST_COHERENT,
            host_access: Some(vkm::AllocationCreateFlags::HOST_ACCESS_SEQUENTIAL_WRITE),
            label: Some(&format!("{:?} Staging Buffer", label)),
            ..Default::default()
        })
    }

    fn try_grow<F: FnOnce(Buffer, &Buffer, u64)>(&self, requried_capacity: u64, resize: F) -> bool {
        log::debug!("Resizing: {:?}", self.label);

        let current_capacity = self.capacity.load(Ordering::Relaxed);
        let new_capacity = current_capacity * 2;

        if new_capacity < requried_capacity {
            return false;
        }

        let new_buffer = Self::create_buffer(
            &self.device,
            new_capacity,
            self.sharing,
            self.label.as_deref(),
        );

        let mut recorder = self.queue.record();

        let old = self.buffer.replace(new_buffer);
        let new = unsafe { self.buffer.as_ptr().as_ref().unwrap() };
        recorder.copy_buffer(&old, new, 0, 0, current_capacity as usize);

        let submission = self.queue.submit_express(&[recorder.finish()]).unwrap();

        resize(old, new, submission);

        true
    }

    fn try_shrink<F: FnOnce(Buffer, &Buffer, u64)>(&self, resize: F) -> bool {
        let current_offset = self.offset.load(Ordering::SeqCst);
        let current_capacity = self.capacity.load(Ordering::SeqCst);

        if current_offset >= current_capacity / 4 {
            return false;
        }

        let new_capacity = current_capacity / 2;
        if new_capacity < 1024 * 1024 {
            return false;
        }

        let new_buffer = Self::create_buffer(
            &self.device,
            new_capacity,
            self.sharing,
            self.label.as_deref(),
        );

        let mut recorder = self.queue.record();

        let old = self.buffer.replace(new_buffer);
        let new = unsafe { self.buffer.as_ptr().as_ref().unwrap() };
        recorder.copy_buffer(&old, new, 0, 0, current_offset as usize);

        let submission = self.queue.submit_express(&[recorder.finish()]).unwrap();

        resize(old, new, submission);

        true
    }

    pub fn allocate<T, F: FnOnce(Buffer, &Buffer, u64)>(&self, resize: F) -> Option<u64> {
        let size = std::mem::size_of::<T>() as u64;
        self.allocate_size(size, resize)
    }

    pub fn allocate_size<F: FnOnce(Buffer, &Buffer, u64)>(
        &self,
        size: u64,
        resize: F,
    ) -> Option<u64> {
        if let Some(block) = self.find_free_block(size) {
            return Some(block);
        }

        let offset = self.offset.fetch_add(size, Ordering::SeqCst);
        let capacity = self.capacity.load(Ordering::Acquire);

        if offset + size <= capacity {
            Some(offset)
        } else {
            self.offset.fetch_sub(size, Ordering::SeqCst);

            if self.try_grow(offset + size, resize) {
                let new_offset = self.offset.fetch_add(size, Ordering::SeqCst);
                Some(new_offset)
            } else {
                None
            }
        }
    }

    pub fn deallocate<T, F: FnOnce(Buffer, &Buffer, u64)>(&self, offset: u64, resize: F) {
        let size = std::mem::size_of::<T>() as u64;
        self.deallocate_size(offset, size, resize);
    }

    pub fn deallocate_size<F: FnOnce(Buffer, &Buffer, u64)>(
        &self,
        offset: u64,
        size: u64,
        resize: F,
    ) {
        let mut free_blocks = self.free.write();

        free_blocks
            .entry(size)
            .or_insert_with(Vec::new)
            .push(offset);

        self.try_shrink(resize);
    }

    pub fn write<T: bytemuck::Pod>(&self, offset: u64, data: &T) -> (Buffer, u64) {
        let data_slice = std::slice::from_ref(data);
        let slice = bytemuck::cast_slice(data_slice);
        self.write_slice(offset, slice)
    }

    pub fn write_slice(&self, offset: u64, data: &[u8]) -> (Buffer, u64) {
        let staging =
            Self::create_staging_buffer(&self.device, data.len() as u64, self.label.as_deref());
        staging.upload(data, 0);

        let mut recorder = self.queue.record();

        let buffer = self.buffer();
        recorder.copy_buffer(&staging, buffer, 0, offset as usize, data.len());
        let submit = self.queue.submit_express(&[recorder.finish()]).unwrap();

        (staging, submit)
    }

    pub fn allocate_and_write<T: bytemuck::Pod, F: FnOnce(Buffer, &Buffer, u64)>(
        &self,
        data: &T,
        resize: F,
    ) -> (Buffer, u64) {
        let offset = self.allocate::<T, _>(resize).unwrap();
        self.write(offset, data)
    }

    pub fn allocate_and_write_slice<F: FnOnce(Buffer, &Buffer, u64)>(
        &self,
        data: &[u8],
        resize: F,
    ) -> (Buffer, u64) {
        let offset = self.allocate_size(data.len() as u64, resize).unwrap();
        self.write_slice(offset, data)
    }

    pub fn find_free_block(&self, size: u64) -> Option<u64> {
        let mut free_blocks = self.free.write();
        if let Some((_size, blocks)) = free_blocks.range_mut(size..).next() {
            if !blocks.is_empty() {
                return Some(blocks.remove(blocks.len() - 1));
            }
        }
        None
    }

    pub fn buffer(&self) -> &Buffer {
        unsafe { self.buffer.as_ptr().as_ref().unwrap() }
    }
}
