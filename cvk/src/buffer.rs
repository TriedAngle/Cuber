use std::{ffi, mem, ptr, sync::Arc};

use crate::Device;
use ash::vk;
use vkm::Alloc;

pub struct Buffer {
    pub handle: vk::Buffer,
    pub allocation: vkm::Allocation,
    pub size: vk::DeviceSize,
    pub device: Arc<ash::Device>,
    pub allocator: Arc<vkm::Allocator>,
}

pub struct BufferInfo<'a> {
    pub size: vk::DeviceSize,
    pub usage: vk::BufferUsageFlags,
    pub sharing: vk::SharingMode,
    pub usage_locality: vkm::MemoryUsage,
    pub allocation_locality: vk::MemoryPropertyFlags,
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

        let allocation_info = vkm::AllocationCreateInfo {
            usage: info.usage_locality,
            required_flags: info.allocation_locality,
            ..Default::default()
        };

        let (handle, allocation) = unsafe {
            allocator
                .create_buffer(&buffer_info, &allocation_info)
                .unwrap()
        };

        self.set_object_debug_info(handle, info.label, info.tag);

        Buffer {
            handle,
            allocation,
            size: info.size,
            device: self.handle.clone(),
            allocator,
        }
    }
}

impl Buffer {
    pub unsafe fn map(&mut self, offset: usize) -> *mut u8 {
        self.allocator
            .map_memory(&mut self.allocation)
            .unwrap()
            .add(offset)
    }

    pub unsafe fn unmap(&mut self) {
        self.allocator.unmap_memory(&mut self.allocation);
    }

    pub fn upload(&mut self, data: &[u8], offset: usize) {
        let size = data.len();
        unsafe {
            let mapping = self.map(offset);

            ptr::copy(data.as_ptr(), mapping, size);

            self.unmap();
        }
    }

    pub fn download(&mut self, buffer: &mut [u8], offset: usize, size: usize) {
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
            self.allocator
                .destroy_buffer(self.handle, &mut self.allocation);
        }
    }
}
