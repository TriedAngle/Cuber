use std::{cell::UnsafeCell, ptr, sync::Arc};

use crate::Device;
use ash::vk;
use vkm::Alloc;

pub struct Buffer {
    pub handle: vk::Buffer,
    pub allocation: UnsafeCell<vkm::Allocation>,
    pub size: vk::DeviceSize,
    pub device: Arc<ash::Device>,
    pub allocator: Arc<vkm::Allocator>,
}

unsafe impl Send for Buffer {}
unsafe impl Sync for Buffer {}

pub struct BufferInfo<'a> {
    pub size: vk::DeviceSize,
    pub usage: vk::BufferUsageFlags,
    pub sharing: vk::SharingMode,
    pub usage_locality: vkm::MemoryUsage,
    pub allocation_locality: vk::MemoryPropertyFlags,
    pub host_access: Option<vkm::AllocationCreateFlags>,
    pub label: Option<&'a str>,
    pub tag: Option<(u64, &'a [u8])>,
}

impl Default for BufferInfo<'_> {
    fn default() -> Self {
        Self {
            size: 0 as vk::DeviceSize,
            usage: vk::BufferUsageFlags::empty(),
            sharing: vk::SharingMode::EXCLUSIVE,
            usage_locality: vkm::MemoryUsage::Auto,
            allocation_locality: vk::MemoryPropertyFlags::empty(),
            host_access: None,
            label: None,
            tag: None,
        }
    }
}

impl Device {
    pub fn create_buffer(&self, info: &BufferInfo<'_>) -> Buffer {
        let allocator = self.allocator.clone();
        let buffer_info = vk::BufferCreateInfo::default()
            .size(info.size)
            .usage(info.usage)
            .sharing_mode(info.sharing);

        let mut allocation_info = vkm::AllocationCreateInfo {
            usage: info.usage_locality,
            required_flags: info.allocation_locality,
            ..Default::default()
        };

        if let Some(host_access) = info.host_access {
            allocation_info.flags |= host_access;
        }

        let (handle, allocation) = unsafe {
            allocator
                .create_buffer(&buffer_info, &allocation_info)
                .unwrap()
        };

        self.set_object_debug_info(handle, info.label, info.tag);

        Buffer {
            handle,
            allocation: UnsafeCell::new(allocation),
            size: info.size,
            device: self.handle.clone(),
            allocator,
        }
    }
}

impl Buffer {
    pub unsafe fn map(&self, offset: usize) -> *mut u8 {
        let allocation = self.allocation.get().as_mut().unwrap();
        self.allocator.map_memory(allocation).unwrap().add(offset)
    }

    pub unsafe fn unmap(&self) {
        let allocation = self.allocation.get().as_mut().unwrap();
        self.allocator.unmap_memory(allocation);
    }

    pub fn upload(&self, data: &[u8], offset: usize) {
        let size = data.len();
        unsafe {
            let mapping = self.map(offset);

            ptr::copy(data.as_ptr(), mapping, size);

            self.unmap();
        }
    }

    pub fn download(&self, buffer: &mut [u8], offset: usize, size: usize) {
        unsafe {
            let mapping = self.map(offset);
            std::ptr::copy_nonoverlapping(mapping, buffer.as_mut_ptr(), size);
            self.unmap();
        }
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe {
            let allocation = self.allocation.get().as_mut().unwrap();
            self.allocator.destroy_buffer(self.handle, allocation);
        }
    }
}
