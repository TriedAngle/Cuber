use anyhow::{Context, Result};
use ash::vk;
use std::sync::Arc;

use crate::{Adapter, Instance, Queue, QueueRequest, Shader};

pub struct Device {
    pub(crate) handle: Arc<ash::Device>,
    pub(crate) adapter: Arc<Adapter>,
    pub(crate) allocator: Arc<vkm::Allocator>,
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
        instance: &Instance,
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
            ash::khr::timeline_semaphore::NAME.as_ptr(),
            ash::khr::dynamic_rendering::NAME.as_ptr(),
        ];

        let queue_family_infos =
            Queue::find_queue_families(&instance, &adapter, queue_requests).context("Queues")?;

        let queue_create_infos = queue_family_infos
            .iter()
            .map(|info| {
                vk::DeviceQueueCreateInfo::default()
                    .queue_family_index(info.family_index)
                    .queue_priorities(&[1.0])
            })
            .collect::<Vec<_>>();

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


        let new = Self { handle: Arc::new(handle), adapter, allocator: Arc::new(allocator) };

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
            self.handle.destroy_device(None);   
        }
    }
}
