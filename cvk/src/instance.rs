use anyhow::Result;
use ash::vk;
use std::{ffi, sync::Arc};

use crate::{Adapter, Device, Queue, QueueRequest};

pub struct Instance {
    #[allow(unused)]
    pub entry: ash::Entry,
    pub handle: ash::Instance,
}

impl Instance {
    pub fn new(app: &str, engine: &str) -> Result<Arc<Self>> {
        let entry = unsafe { ash::Entry::load()? };
        let app_name = ffi::CString::new(app)?;
        let engine_name = ffi::CString::new(engine)?;

        let afo = vk::ApplicationInfo::default()
            .application_name(&app_name)
            .application_version(vk::make_api_version(0, 0, 1, 0))
            .engine_name(&engine_name)
            .engine_version(vk::make_api_version(0, 0, 1, 0))
            .api_version(vk::API_VERSION_1_3);

        let mut instance_extensions = vec![
            ash::ext::debug_utils::NAME.as_ptr(),
            ash::khr::surface::NAME.as_ptr(),
        ];

        // Add platform-specific surface extensions
        #[cfg(target_os = "windows")]
        instance_extensions.push(ash::khr::win32_surface::NAME.as_ptr());
        #[cfg(all(unix, not(target_os = "android"), not(target_os = "macos")))]
        instance_extensions.push(ash::khr::xlib_surface::NAME.as_ptr());
        #[cfg(target_os = "macos")]
        instance_extensions.push(ash::ext::metal_surface::NAME.as_ptr());

        let validation_layer = ffi::CString::new("VK_LAYER_KHRONOS_validation")?;
        let instance_layers = vec![validation_layer.as_ptr()];

        let mut dfo = vk::DebugUtilsMessengerCreateInfoEXT::default()
            .message_severity(
                vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                    | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                    | vk::DebugUtilsMessageSeverityFlagsEXT::INFO,
            )
            .message_type(
                vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                    | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                    | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
            )
            .pfn_user_callback(Some(debug_callback));

        let ifo = vk::InstanceCreateInfo::default()
            .application_info(&afo)
            .enabled_extension_names(&instance_extensions)
            .enabled_layer_names(&instance_layers)
            .push_next(&mut dfo);

        let handle = unsafe { entry.create_instance(&ifo, None)? };

        let new = Self { entry, handle };
        Ok(Arc::new(new))
    }

    pub fn adapters(&self, formats: &[vk::Format]) -> Result<Vec<Arc<Adapter>>> {
        let pdevs = unsafe { self.handle.enumerate_physical_devices()? };

        let adapters = pdevs
            .into_iter()
            .map(|physical_device| Arc::new(Adapter::new(&self, physical_device, formats)))
            .collect::<Vec<_>>();

        Ok(adapters)
    }

    pub fn request_device(
        self: Arc<Instance>,
        adapter: Arc<Adapter>,
        queue_requestes: &[QueueRequest],
    ) -> Result<(Arc<Device>, Vec<Arc<Queue>>)> {
        Device::new(self.clone(), adapter, queue_requestes)
    }

    pub fn handle(&self) -> &ash::Instance {
        &self.handle
    }

    pub fn entry(&self) -> &ash::Entry {
        &self.entry
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        unsafe {
            self.handle.destroy_instance(None);
        }
    }
}

unsafe extern "system" fn debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    _message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _user_data: *mut ffi::c_void,
) -> vk::Bool32 {
    let callback_data = *p_callback_data;
    let message = std::ffi::CStr::from_ptr(callback_data.p_message);

    match message_severity {
        vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => {
            log::error!("Validation Layer: {:?}", message);
        }
        vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => {
            log::warn!("Validation Layer: {:?}", message);
        }
        vk::DebugUtilsMessageSeverityFlagsEXT::INFO => {
            log::info!("Validation Layer: {:?}", message);
        }
        _ => {
            log::debug!("Validation Layer: {:?}", message);
        }
    }

    vk::FALSE
}
