use anyhow::{Context, Result};
use ash::vk;
use std::sync::Arc;
use std::{collections::HashMap, mem};

use crate::command::ThreadCommandPools;
use crate::{Adapter, Instance, Queue, QueueRequest};

pub struct Device {
    pub handle: Arc<ash::Device>,
    pub instance: Arc<Instance>,
    pub adapter: Arc<Adapter>,
    pub debug_utils: Arc<ash::ext::debug_utils::Device>,
    pub allocator: Arc<vkm::Allocator>,
    pub command_pools: ThreadCommandPools,
}

impl std::fmt::Debug for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Device")
            .field("handle", &self.handle.handle())
            .finish()
    }
}

impl Device {
    pub fn new(
        instance: Arc<Instance>,
        adapter: Arc<Adapter>,
        queue_requests: &[QueueRequest],
    ) -> Result<(Arc<Self>, Vec<Arc<Queue>>)> {
        let mut pdev_features2 = vk::PhysicalDeviceFeatures2::default().features(
            vk::PhysicalDeviceFeatures::default()
                .shader_sampled_image_array_dynamic_indexing(true)
                .shader_storage_image_array_dynamic_indexing(true)
                .shader_storage_buffer_array_dynamic_indexing(true)
                .shader_uniform_buffer_array_dynamic_indexing(true),
        );

        let mut timeline_semaphore_features =
            vk::PhysicalDeviceTimelineSemaphoreFeatures::default().timeline_semaphore(true);

        let mut dynamic_rendering_features =
            vk::PhysicalDeviceDynamicRenderingFeatures::default().dynamic_rendering(true);

        let device_extensions = [
            ash::khr::swapchain::NAME.as_ptr(),
            ash::khr::timeline_semaphore::NAME.as_ptr(),
            ash::khr::dynamic_rendering::NAME.as_ptr(),
        ];

        let queue_family_infos =
            Queue::find_queue_families(&instance, &adapter, queue_requests).context("Queues")?;

        let mut family_queue_counts: HashMap<u32, u32> = HashMap::new();
        for info in &queue_family_infos {
            let count = family_queue_counts.entry(info.family_index).or_default();
            *count = (*count).max(info.queue_index + 1);
        }

        let queue_families: Vec<(u32, Vec<f32>)> = family_queue_counts
            .into_iter()
            .map(|(family_index, count)| {
                let priorities = vec![1.0; count as usize];
                (family_index, priorities)
            })
            .collect();

        let queue_create_infos: Vec<vk::DeviceQueueCreateInfo> = queue_families
            .iter()
            .map(|(family_index, priorities)| {
                vk::DeviceQueueCreateInfo::default()
                    .queue_family_index(*family_index)
                    .queue_priorities(priorities)
            })
            .collect();

        let device_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_create_infos)
            .enabled_extension_names(&device_extensions)
            .push_next(&mut pdev_features2)
            .push_next(&mut timeline_semaphore_features)
            .push_next(&mut dynamic_rendering_features);

        let handle = unsafe {
            instance
                .handle()
                .create_device(adapter.handle(), &device_info, None)?
        };

        let debug_utils = ash::ext::debug_utils::Device::new(&instance.handle, &handle);

        let allocator = unsafe {
            vkm::Allocator::new(vkm::AllocatorCreateInfo::new(
                instance.handle(),
                &handle,
                adapter.handle(),
            ))
        }?;

        let handle = Arc::new(handle);

        let command_pools = ThreadCommandPools::new(handle.clone());

        let new = Self {
            handle,
            instance,
            adapter,
            debug_utils: Arc::new(debug_utils),
            command_pools,
            allocator: Arc::new(allocator),
        };

        let new = Arc::new(new);

        let queues = queue_family_infos
            .iter()
            .map(|info| Queue::new(new.clone(), info))
            .collect::<Vec<_>>();

        Ok((new, queues))
    }

    pub fn set_object_name<T: vk::Handle>(&self, handle: T, name: &str) {
        let name = std::ffi::CString::new(name).unwrap();
        let info = vk::DebugUtilsObjectNameInfoEXT::default()
            .object_handle(handle)
            .object_name(&name);

        unsafe {
            self.debug_utils.set_debug_utils_object_name(&info).unwrap();
        }
    }

    pub fn set_object_tag<T: vk::Handle>(&self, handle: T, tag_name: u64, tag_data: &[u8]) {
        let info = vk::DebugUtilsObjectTagInfoEXT::default()
            .object_handle(handle)
            .tag_name(tag_name)
            .tag(tag_data);

        unsafe {
            self.debug_utils.set_debug_utils_object_tag(&info).unwrap();
        }
    }

    pub fn set_object_debug_info<T: vk::Handle + Copy>(
        &self,
        handle: T,
        label: Option<&str>,
        tag: Option<(u64, &[u8])>,
    ) {
        if let Some(name) = label {
            self.set_object_name(handle, name);
        }

        if let Some((tag_id, tag_data)) = tag {
            self.set_object_tag(handle, tag_id, tag_data);
        }
    }

    pub fn handle(&self) -> &ash::Device {
        &self.handle
    }

    pub fn adapter(&self) -> &Adapter {
        &self.adapter
    }

    pub fn allocator(&self) -> &vkm::Allocator {
        &self.allocator
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            let _ = self.handle.device_wait_idle();
            let allocator = mem::replace(Arc::get_mut(&mut self.allocator).unwrap(), mem::zeroed());
            mem::drop(allocator);

            let pools = self.command_pools.pools.lock();
            for (_, &pool) in pools.iter() {
                self.handle.destroy_command_pool(pool, None);
            }
            self.handle.destroy_device(None);
        }
    }
}
