use anyhow::Result;
use ash::vk;
use std::{cell::UnsafeCell, rc::Rc, sync::Arc, time::Duration, u64};
use winit::{
    raw_window_handle::{HasDisplayHandle, HasWindowHandle},
    window::Window,
};

use crate::{CustomImageViewInfo, Device, Image, ImageDetails, Queue};

#[derive(Clone)]
pub struct Frame {
    pub image: Rc<Image>,
    pub index: u32,
}

#[derive(Clone, Copy)]
pub struct FrameSignals {
    pub available: vk::Semaphore,
    pub finished: vk::Semaphore,
}

pub struct SwapchainConfig {
    pub preferred_image_count: u32,
    pub preferred_present_mode: vk::PresentModeKHR,
    pub format_selector: Box<dyn Fn(&[vk::SurfaceFormatKHR]) -> vk::SurfaceFormatKHR>,
}

impl Default for SwapchainConfig {
    fn default() -> Self {
        Self {
            preferred_image_count: 3,
            preferred_present_mode: vk::PresentModeKHR::MAILBOX,
            format_selector: Box::new(|formats| {
                formats
                    .iter()
                    .find(|f| {
                        f.format == vk::Format::B8G8R8A8_SRGB
                            && f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
                    })
                    .copied()
                    .unwrap_or(formats[0])
            }),
        }
    }
}

#[derive(Debug)]
pub enum SwapchainStatus {
    Optimal,
    Suboptimal,
    OutOfDate,
}

pub struct SwapchainResources {
    pub handle: vk::SwapchainKHR,
    pub frames: Vec<Frame>,
    pub signals: Vec<FrameSignals>,
    pub extent: vk::Extent2D,
    pub frames_in_flight: u32,
    pub capabilities: vk::SurfaceCapabilitiesKHR,
}

pub struct Swapchain {
    device: Arc<ash::Device>,
    pub handle: vk::SwapchainKHR,
    device_loader: ash::khr::swapchain::Device,
    surface_handle: vk::SurfaceKHR,
    surface_loader: ash::khr::surface::Instance,
    adapter_handle: vk::PhysicalDevice,
    pub format: vk::SurfaceFormatKHR,
    pub capabilities: vk::SurfaceCapabilitiesKHR,
    pub formats: Arc<[vk::SurfaceFormatKHR]>,
    pub present_modes: Arc<[vk::PresentModeKHR]>,
    pub frames: Vec<Frame>,
    pub frames_in_flight: u32,
    pub signals: Vec<FrameSignals>,
    pub current_frame: usize,
    pub extent: vk::Extent2D,
    pub config: SwapchainConfig,
}

impl Swapchain {
    fn create_resources(
        device: Arc<ash::Device>,
        device_loader: &ash::khr::swapchain::Device,
        surface_handle: vk::SurfaceKHR,
        surface_loader: &ash::khr::surface::Instance,
        adapter_handle: vk::PhysicalDevice,
        config: &SwapchainConfig,
        format: vk::SurfaceFormatKHR,
        old_swapchain: Option<vk::SwapchainKHR>,
    ) -> Result<SwapchainResources> {
        let capabilities = unsafe {
            surface_loader
                .get_physical_device_surface_capabilities(adapter_handle, surface_handle)?
        };

        let min_images = capabilities.min_image_count;
        let max_images = if capabilities.max_image_count == 0 {
            config.preferred_image_count
        } else {
            capabilities.max_image_count
        };

        let image_count = config.preferred_image_count.max(min_images).min(max_images);

        let present_modes = unsafe {
            surface_loader
                .get_physical_device_surface_present_modes(adapter_handle, surface_handle)?
        };

        let present_mode = if present_modes.contains(&config.preferred_present_mode) {
            config.preferred_present_mode
        } else {
            present_modes
                .iter()
                .copied()
                .find(|&mode| mode == vk::PresentModeKHR::MAILBOX)
                .or_else(|| {
                    log::warn!("Present mode: MAILBOX not found, falling back to Immediate");
                    present_modes
                        .iter()
                        .copied()
                        .find(|&mode| mode == vk::PresentModeKHR::IMMEDIATE)
                })
                .unwrap_or_else(|| {
                    log::warn!("Present mode: IMMEDIATE not found, falling back to Fifo");
                    vk::PresentModeKHR::FIFO
                })
        };

        let info = vk::SwapchainCreateInfoKHR::default()
            .surface(surface_handle)
            .min_image_count(image_count)
            .image_format(format.format)
            .image_color_space(format.color_space)
            .image_extent(capabilities.current_extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present_mode)
            .clipped(true)
            .old_swapchain(old_swapchain.unwrap_or(vk::SwapchainKHR::null()));

        let handle = unsafe { device_loader.create_swapchain(&info, None)? };

        let frames = unsafe {
            device_loader
                .get_swapchain_images(handle)?
                .into_iter()
                .enumerate()
                .map(|(index, image)| {
                    Frame::new(
                        device.clone(),
                        image,
                        format.format,
                        index as u32,
                        capabilities.current_extent,
                    )
                })
                .collect::<Vec<_>>()
        };

        let signals = frames
            .iter()
            .enumerate()
            .map(|_| unsafe {
                let info = vk::SemaphoreCreateInfo::default();
                let available = device.create_semaphore(&info, None)?;
                let finished = device.create_semaphore(&info, None)?;
                Ok(FrameSignals {
                    available,
                    finished,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(SwapchainResources {
            handle,
            frames,
            signals,
            extent: capabilities.current_extent,
            frames_in_flight: image_count,
            capabilities,
        })
    }

    pub fn rebuild(&mut self) -> Result<()> {
        unsafe { self.device.device_wait_idle()? };

        unsafe {
            for frame in &self.frames {
                self.device.destroy_image_view(frame.image.view, None);
            }
            for signal in &self.signals {
                self.device.destroy_semaphore(signal.available, None);
                self.device.destroy_semaphore(signal.finished, None);
            }
        }

        let old_swapchain = self.handle;

        let resources = Self::create_resources(
            self.device.clone(),
            &self.device_loader,
            self.surface_handle,
            &self.surface_loader,
            self.adapter_handle,
            &self.config,
            self.format,
            Some(old_swapchain),
        )?;

        unsafe {
            self.device_loader.destroy_swapchain(old_swapchain, None);
        }

        self.handle = resources.handle;
        self.frames = resources.frames;
        self.signals = resources.signals;
        self.extent = resources.extent;
        self.frames_in_flight = resources.frames_in_flight;
        self.capabilities = resources.capabilities;
        self.current_frame = 0;

        Ok(())
    }

    pub fn acquire_next_frame(
        &mut self,
        timeout: Option<Duration>,
    ) -> Result<(Frame, FrameSignals, SwapchainStatus)> {
        let timeout_ns = timeout.map_or(u64::MAX, |d| d.as_nanos() as u64);
        let current_signal = self.current_frame;
        let semaphore = self.signals[current_signal].available;

        match unsafe {
            self.device_loader.acquire_next_image(
                self.handle,
                timeout_ns,
                semaphore,
                vk::Fence::null(),
            )
        } {
            Ok((index, suboptimal)) => {
                let status = if suboptimal {
                    SwapchainStatus::Suboptimal
                } else {
                    SwapchainStatus::Optimal
                };

                Ok((
                    self.frames[index as usize].clone(),
                    self.signals[current_signal],
                    status,
                ))
            }
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                Ok((
                    self.frames[0].clone(), // Return first frame as placeholder
                    self.signals[current_signal],
                    SwapchainStatus::OutOfDate,
                ))
            }
            Err(e) => Err(e.into()),
        }
    }

    pub fn present_frame(&mut self, queue: &Queue, frame: Frame) -> Result<SwapchainStatus> {
        let wait_semaphores = [self.signals[self.current_frame].finished];
        let swapchains = [self.handle];
        let image_indices = [frame.index];

        let info = vk::PresentInfoKHR::default()
            .wait_semaphores(&wait_semaphores)
            .swapchains(&swapchains)
            .image_indices(&image_indices);

        let status = unsafe {
            let _lock = queue.lock();
            match self.device_loader.queue_present(queue.handle, &info) {
                Ok(suboptimal) => {
                    if suboptimal {
                        SwapchainStatus::Suboptimal
                    } else {
                        SwapchainStatus::Optimal
                    }
                }
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => SwapchainStatus::OutOfDate,
                Err(e) => return Err(e.into()),
            }
        };

        self.current_frame = (self.current_frame + 1) % self.frames_in_flight as usize;
        Ok(status)
    }

    pub fn format(&self) -> vk::SurfaceFormatKHR {
        self.format
    }

    pub fn extent(&self) -> vk::Extent2D {
        self.extent
    }

    pub fn capabilities(&self) -> vk::SurfaceCapabilitiesKHR {
        self.capabilities
    }
}

impl Device {
    pub fn create_swapchain(&self, config: SwapchainConfig, window: &Window) -> Result<Swapchain> {
        let surface_handle = unsafe {
            ash_window::create_surface(
                &self.instance.entry,
                &self.instance.handle,
                window.display_handle()?.as_raw(),
                window.window_handle()?.as_raw(),
                None,
            )?
        };

        let surface_loader =
            ash::khr::surface::Instance::new(&self.instance.entry, &self.instance.handle);

        let device_loader = ash::khr::swapchain::Device::new(&self.instance.handle, &self.handle);

        let formats = unsafe {
            surface_loader
                .get_physical_device_surface_formats(self.adapter.handle, surface_handle)?
        };
        let format = (config.format_selector)(&formats);

        let present_modes = unsafe {
            surface_loader
                .get_physical_device_surface_present_modes(self.adapter.handle, surface_handle)?
        };

        let resources = Swapchain::create_resources(
            self.handle.clone(),
            &device_loader,
            surface_handle,
            &surface_loader,
            self.adapter.handle(),
            &config,
            format,
            None,
        )?;

        Ok(Swapchain {
            device: self.handle.clone(),
            handle: resources.handle,
            device_loader,
            surface_handle,
            surface_loader,
            adapter_handle: self.adapter.handle(),
            format,
            capabilities: resources.capabilities,
            formats: Arc::from(formats),
            present_modes: Arc::from(present_modes),
            frames: resources.frames,
            frames_in_flight: resources.frames_in_flight,
            signals: resources.signals,
            current_frame: 0,
            extent: resources.extent,
            config,
        })
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();
            for frame in &self.frames {
                self.device.destroy_image_view(frame.image.view, None);
            }
            for signal in &self.signals {
                self.device.destroy_semaphore(signal.available, None);
                self.device.destroy_semaphore(signal.finished, None);
            }
            self.device_loader.destroy_swapchain(self.handle, None);
            self.surface_loader
                .destroy_surface(self.surface_handle, None);
        }
    }
}
impl Frame {
    pub fn new(
        device: Arc<ash::Device>,
        image: vk::Image,
        format: vk::Format,
        index: u32,
        extent: vk::Extent2D,
    ) -> Self {
        let info = CustomImageViewInfo {
            image,
            format,
            aspect: vk::ImageAspectFlags::COLOR,
            ..Default::default()
        };
        let view_info = vk::ImageViewCreateInfo::default()
            .image(info.image)
            .view_type(info.ty)
            .format(info.format)
            .components(info.swizzle)
            .subresource_range(
                vk::ImageSubresourceRange::default()
                    .aspect_mask(info.aspect)
                    .base_mip_level(info.mips.start)
                    .level_count(info.mips.len() as u32)
                    .base_array_layer(info.layers.start)
                    .layer_count(info.layers.len() as u32),
            );

        let view = unsafe { device.create_image_view(&view_info, None).unwrap() };

        let details = ImageDetails {
            format,
            layout: vk::ImageLayout::UNDEFINED,
            stage: vk::PipelineStageFlags::TOP_OF_PIPE,
            access: vk::AccessFlags::empty(),
            width: extent.width,
            height: extent.height,
            layers: 1,
        };

        let image = Image {
            handle: image,
            view,
            sampler: None,
            device,
            details: UnsafeCell::new(details),
            allocation: None,
        };

        Self {
            image: Rc::new(image),
            index,
        }
    }
}
