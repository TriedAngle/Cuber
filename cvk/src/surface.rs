use std::sync::Arc;
use anyhow::Result;

use ash::vk;

use crate::{Adapter, Queue};

pub struct Surface { 
    handle: vk::SurfaceKHR,
    instance: ash::khr::surface::Instance,
    format: vk::SurfaceFormatKHR,
    capabilities: vk::SurfaceCapabilitiesKHR,
    formats: Arc<[vk::SurfaceFormatKHR]>,
    present_modes: Arc<[vk::PresentModeKHR]>,
}

impl Surface { 
    pub fn new(handle: vk::SurfaceKHR, instance: ash::khr::surface::Instance, adapter: &Adapter,
    choose_format: impl Fn(&[vk::SurfaceFormatKHR]) -> vk::SurfaceFormatKHR) -> Result<Self> { 
        let formats = unsafe { instance.get_physical_device_surface_formats(adapter.handle(), handle)? };
        let capabilities = unsafe { instance.get_physical_device_surface_capabilities(adapter.handle(), handle)? };
        let present_modes = unsafe { instance.get_physical_device_surface_present_modes(adapter.handle(), handle)? };

        let format = choose_format(&formats);
        Ok(Self { handle, instance, format, capabilities, formats: Arc::from(formats), present_modes: Arc::from(present_modes) })
    }

    pub fn is_compatible(&self, adapter: &Adapter, queue: &Queue) -> bool { 
        unsafe { 
            self.instance.get_physical_device_surface_support(adapter.handle(), queue.queue_family(), self.handle).unwrap()
        }
    }

    pub fn capabilities(&self) -> vk::SurfaceCapabilitiesKHR { 
        self.capabilities
    }

    pub fn format(&self) -> vk::SurfaceFormatKHR { 
        self.format
    }

    pub fn formats(&self) -> &[vk::SurfaceFormatKHR] { 
        &self.formats
    }

    pub fn present_modes(&self) -> &[vk::PresentModeKHR] { 
        &self.present_modes
    }
}



impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            self.instance.destroy_surface(self.handle, None);
        }
    }
}



