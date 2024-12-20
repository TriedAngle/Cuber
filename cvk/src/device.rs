use anyhow::{Context, Result};
use ash::vk;
use std::sync::Arc;
use std::{collections::HashMap, mem};

use crate::{Adapter, Instance, Queue, QueueRequest};

pub struct Device {
    pub handle: Arc<ash::Device>,
    pub instance: Arc<Instance>,
    pub adapter: Arc<Adapter>,
    pub allocator: Arc<vkm::Allocator>,
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

        let allocator = unsafe {
            vkm::Allocator::new(vkm::AllocatorCreateInfo::new(
                instance.handle(),
                &handle,
                adapter.handle(),
            ))
        }?;

        let new = Self {
            handle: Arc::new(handle),
            instance,
            adapter,
            allocator: Arc::new(allocator),
        };

        let new = Arc::new(new);

        let queues = queue_family_infos
            .iter()
            .map(|info| Queue::new(new.clone(), info))
            .collect::<Vec<_>>();

        Ok((new, queues))
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
            let allocator = mem::replace(Arc::get_mut(&mut self.allocator).unwrap(), mem::zeroed());
            mem::drop(allocator);
            let _ = self.handle.device_wait_idle();
            self.handle.destroy_device(None);
        }
    }
}
