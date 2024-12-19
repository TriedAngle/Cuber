use ash::vk::{self, QueueFamilyProperties};
use std::sync::Arc;

use crate::Instance;

#[derive(Debug, Clone)]
pub struct Adapter {
    pub handle: vk::PhysicalDevice,
    pub properties: vk::PhysicalDeviceProperties,
    pub queue_properties: Arc<[QueueFamilyProperties]>,
    pub features: vk::PhysicalDeviceFeatures,
    pub formats: Arc<[(vk::Format, vk::FormatProperties)]>,
}

impl Adapter {
    pub fn new(
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
        formats: &[vk::Format],
    ) -> Self {
        let properties = unsafe {
            instance
                .handle()
                .get_physical_device_properties(physical_device)
        };
        let features = unsafe {
            instance
                .handle()
                .get_physical_device_features(physical_device)
        };

        let queue_properties = unsafe {
            instance
                .handle()
                .get_physical_device_queue_family_properties(physical_device)
        };
        let format_properties = formats
            .iter()
            .map(|&format| unsafe {
                let props = instance
                    .handle()
                    .get_physical_device_format_properties(physical_device, format);
                (format, props)
            })
            .collect::<Vec<_>>();

        Adapter {
            handle: physical_device,
            properties,
            queue_properties: Arc::from(queue_properties),
            features,
            formats: Arc::from(format_properties),
        }
    }

    pub fn handle(&self) -> vk::PhysicalDevice {
        self.handle
    }
}
