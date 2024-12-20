use anyhow::Result;
use std::{sync::Arc, time::Duration, u64};

use ash::vk;

use crate::{Adapter, Device, Queue};

#[derive(Clone, Copy)]
pub struct Frame {
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub index: u32,
}

#[derive(Clone, Copy)]
pub struct FrameSignals {
    pub available: vk::Semaphore,
    pub finished: vk::Semaphore,
}

pub struct Swapchain {
    device: Arc<ash::Device>,
    pub handle: vk::SwapchainKHR,
    pub instance_loader: ash::khr::swapchain::Instance,
    pub device_loader: ash::khr::swapchain::Device,
    pub surface: Arc<Surface>,
    pub frames: Vec<Frame>,
    pub frames_in_flight: u32,
    pub signals: Vec<FrameSignals>,
    pub current_frame: usize,
    pub extent: vk::Extent2D,
    pub needs_rebuild: bool,
}

impl Device {
    pub fn create_swapchain(
        &self,
        surface: Arc<Surface>,
        image_count: u32,
        mode: vk::PresentModeKHR,
    ) -> Result<Swapchain> {
        let instance_loader =
            ash::khr::swapchain::Instance::new(&self.instance.entry, &self.instance.handle);
        let device_loader = ash::khr::swapchain::Device::new(&self.instance.handle, &self.handle);

        let capabilities = surface.capabilities;

        let min_images = capabilities.min_image_count;
        let max_images = if capabilities.max_image_count == 0 {
            image_count
        } else {
            capabilities.max_image_count
        };

        let image_count = image_count.max(min_images).min(max_images);

        let present_mode = if surface.present_modes.contains(&mode) {
            mode
        } else {
            log::warn!(
                "Present mode: {:?} not found, falling back to Mailbox",
                mode
            );
            surface
                .present_modes()
                .iter()
                .copied()
                .find(|&mode| mode == vk::PresentModeKHR::MAILBOX)
                .or_else(|| {
                    log::warn!("Present mode: MAILBOX not found, falling back to Immidiate");
                    surface
                        .present_modes()
                        .iter()
                        .copied()
                        .find(|&mode| mode == vk::PresentModeKHR::IMMEDIATE)
                })
                .unwrap_or_else(|| {
                    log::warn!("Present mode: IMMEDIATE not found, falling back to Fifo");
                    vk::PresentModeKHR::FIFO
                })
        };

        let format = surface.format;
        let extent = capabilities.current_extent;

        let info = vk::SwapchainCreateInfoKHR::default()
            .surface(surface.handle)
            .min_image_count(image_count)
            .image_format(format.format)
            .image_color_space(format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present_mode)
            .clipped(true);

        let handle = unsafe { device_loader.create_swapchain(&info, None)? };

        let frames = unsafe {
            device_loader
                .get_swapchain_images(handle)?
                .into_iter()
                .enumerate()
                .map(|(index, image)| Frame::new(self, image, format.format, index as u32))
                .collect::<Vec<_>>()
        };

        let signals = frames
            .iter()
            .enumerate()
            .map(|_| unsafe {
                let info = vk::SemaphoreCreateInfo::default();
                let available = self.handle.create_semaphore(&info, None).unwrap();
                let finished = self.handle.create_semaphore(&info, None).unwrap();
                FrameSignals {
                    available,
                    finished,
                }
            })
            .collect::<Vec<_>>();

        let new = Swapchain {
            device: self.handle.clone(),
            handle,
            instance_loader,
            device_loader,
            surface,
            frames_in_flight: image_count,
            current_frame: 0,
            frames,
            signals,
            extent,
            needs_rebuild: false,
        };

        Ok(new)
    }
}

impl Swapchain {
    pub fn acquire_next_frame(&mut self, timeout: Option<Duration>) -> (Frame, FrameSignals, bool) {
        let timeout_ns = timeout.map_or(u64::MAX, |d| d.as_nanos() as u64);
        let semaphore = self.signals[self.current_frame].available;

        let (index, suboptimal) = unsafe {
            self.device_loader
                .acquire_next_image(self.handle, timeout_ns, semaphore, vk::Fence::null())
                .unwrap()
        };

        if suboptimal {
            self.needs_rebuild = suboptimal;
        }

        (
            self.frames[index as usize],
            self.signals[self.current_frame],
            suboptimal,
        )
    }

    pub fn present_frame(&mut self, queue: &Queue, frame: Frame) -> bool {
        let wait_semaphores = [self.signals[self.current_frame].finished];
        let swapchains = [self.handle];
        let image_indices = [frame.index];

        let info = vk::PresentInfoKHR::default()
            .wait_semaphores(&wait_semaphores)
            .swapchains(&swapchains)
            .image_indices(&image_indices);

        let result = unsafe {
            let _lock = queue.lock();
            self.device_loader
                .queue_present(queue.handle, &info)
                .unwrap()
        };

        if !result {
            self.needs_rebuild = true;
            return false;
        }

        self.current_frame = (self.current_frame + 1) % self.frames_in_flight as usize;
        return true;
    }
}

impl Frame {
    pub fn new(device: &Device, image: vk::Image, format: vk::Format, index: u32) -> Self {
        let info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(format)
            .components(vk::ComponentMapping {
                r: vk::ComponentSwizzle::IDENTITY,
                g: vk::ComponentSwizzle::IDENTITY,
                b: vk::ComponentSwizzle::IDENTITY,
                a: vk::ComponentSwizzle::IDENTITY,
            })
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });

        let view = unsafe { device.handle.create_image_view(&info, None).unwrap() };

        Self { image, view, index }
    }
}

pub struct Surface {
    handle: vk::SurfaceKHR,
    instance: ash::khr::surface::Instance,
    format: vk::SurfaceFormatKHR,
    capabilities: vk::SurfaceCapabilitiesKHR,
    formats: Arc<[vk::SurfaceFormatKHR]>,
    present_modes: Arc<[vk::PresentModeKHR]>,
}

impl Surface {
    pub fn new(
        handle: vk::SurfaceKHR,
        instance: ash::khr::surface::Instance,
        adapter: &Adapter,
        choose_format: impl Fn(&[vk::SurfaceFormatKHR]) -> vk::SurfaceFormatKHR,
    ) -> Result<Self> {
        let formats =
            unsafe { instance.get_physical_device_surface_formats(adapter.handle(), handle)? };
        let capabilities =
            unsafe { instance.get_physical_device_surface_capabilities(adapter.handle(), handle)? };
        let present_modes = unsafe {
            instance.get_physical_device_surface_present_modes(adapter.handle(), handle)?
        };

        let format = choose_format(&formats);
        Ok(Self {
            handle,
            instance,
            format,
            capabilities,
            formats: Arc::from(formats),
            present_modes: Arc::from(present_modes),
        })
    }

    pub fn is_compatible(&self, adapter: &Adapter, queue: &Queue) -> bool {
        unsafe {
            self.instance
                .get_physical_device_surface_support(
                    adapter.handle(),
                    queue.queue_family(),
                    self.handle,
                )
                .unwrap()
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

impl Drop for Swapchain {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();
            for image in &self.frames {
                self.device.destroy_image_view(image.view, None);
            }
            for signal in &self.signals {
                self.device.destroy_semaphore(signal.available, None);
                self.device.destroy_semaphore(signal.finished, None);
            }
            self.device_loader.destroy_swapchain(self.handle, None);
        }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            self.instance.destroy_surface(self.handle, None);
        }
    }
}
