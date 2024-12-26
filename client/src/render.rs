use std::{mem, sync::Arc, time};

use anyhow::Result;
use cvk;
use winit::window::Window;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PresentPushConstants {
    pub mode: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RayTracePushConstants {
    pub camera: [[f32; 4]; 4],
    pub camera_inverse: [[f32; 4]; 4],
    pub dimensions: [u32; 3],
    pub packed_resolution: u32,
    pub flags0: u32,
    pub flags1: u32,
    pub dt: f32,
    pub depth_boost: f32,
    pub brick_hit: [u32; 3],
    pub _padding0: u32,
    pub voxel_hit: [u32; 3],
    pub _padding1: u32,
}

impl PresentPushConstants {
    pub fn empty() -> Self {
        Self { mode: 0 }
    }
}

impl RayTracePushConstants {
    pub fn empty() -> Self {
        Self {
            camera: [[0.; 4]; 4],
            camera_inverse: [[0.; 4]; 4],
            dimensions: [0; 3],
            packed_resolution: 0,
            flags0: 0,
            flags1: 0,
            dt: 0.,
            depth_boost: 0.,
            brick_hit: [0; 3],
            _padding0: 0,
            voxel_hit: [0; 3],
            _padding1: 0,
        }
    }

    pub fn set_resolution(&mut self, width: u32, height: u32) {
        self.packed_resolution = (width & 0xFFFF) | (height << 16);
    }
}

pub struct RenderContext {
    gpu: Arc<cgpu::GPUContext>,
    queue: Arc<cvk::Queue>,
    device: Arc<cvk::Device>,
    pub window: Arc<Window>,
    swapchain: cvk::Swapchain,
    present_image: cvk::Image,
    normal_image: cvk::Image,
    depth_image: cvk::Image,
    depth_test_image: cvk::Image,
    raytrace_pipeline: cvk::ComputePipeline,
    present_pipeline: cvk::RenderPipeline,
    rtpc: RayTracePushConstants,
    ppc: PresentPushConstants,
    pub egui: cvk::egui::EguiState,
}

impl RenderContext {
    pub fn new(gpu: Arc<cgpu::GPUContext>, window: Arc<Window>) -> Result<Self> {
        let device = gpu.device.clone();
        let queue = gpu.render_queue.clone();

        let raytrace_shader = device.create_shader(include_str!("shaders/raytrace.wgsl"))?;
        let present_shader = device.create_shader(include_str!("shaders/present.wgsl"))?;

        let raytrace_pipeline = device.create_compute_pipeline(&cvk::ComputePipelineInfo {
            label: Some("Raytrace Pipeline"),
            shader: raytrace_shader.entry("main"),
            descriptor_layouts: &[&gpu.layout],
            push_constant_size: Some(mem::size_of::<RayTracePushConstants>() as u32),
            ..Default::default()
        });

        let present_pipeline = device.create_render_pipeline(&cvk::RenderPipelineInfo {
            label: Some("Present Pipeline"),
            vertex_shader: present_shader.entry("vmain"),
            fragment_shader: present_shader.entry("fmain"),
            descriptor_layouts: &[&gpu.layout],
            push_constant_size: Some(mem::size_of::<PresentPushConstants>() as u32),
            cull: cvk::CullModeFlags::NONE,
            topology: cvk::PrimitiveTopology::TRIANGLE_LIST,
            polygon: cvk::PolygonMode::FILL,

            ..Default::default()
        });

        let rtpc = RayTracePushConstants::empty();
        let ppc = PresentPushConstants::empty();

        let swapchain = device.create_swapchain(
            cvk::SwapchainConfig {
                preferred_image_count: 3,
                preferred_present_mode: cvk::PresentModeKHR::MAILBOX,
                format_selector: Box::new(|formats| {
                    formats
                        .iter()
                        .find(|f| {
                            f.format == cvk::Format::R8G8B8A8_UNORM
                                && f.color_space == cvk::ColorSpaceKHR::SRGB_NONLINEAR
                        })
                        .copied()
                        .unwrap_or(formats[0])
                }),
            },
            &window,
        )?;

        let scale_factor = window.scale_factor();
        let fonts = egui::FontDefinitions::default();
        let style = egui::Style::default();

        let egui = cvk::egui::EguiState::new(
            device.clone(),
            &window,
            swapchain.format.format,
            swapchain.frames_in_flight,
            scale_factor,
            fonts,
            style,
        );

        let (present_image, normal_image, depth_image, depth_test_image) =
            Self::create_image_resources(&device, &swapchain);

        let mut new = Self {
            gpu,
            device,
            queue,
            window,
            swapchain,
            present_image,
            normal_image,
            depth_image,
            depth_test_image,
            raytrace_pipeline,
            present_pipeline,
            rtpc,
            ppc,
            egui,
        };

        let size = new.window.inner_size();
        new.rtpc.set_resolution(size.width, size.height);
        new.rebind_descriptors();

        Ok(new)
    }

    pub fn render(&mut self) {
        let (frame, signals, _status) = match self.swapchain.acquire_next_frame(None) {
            Ok((frame, signals, status)) => {
                match status {
                    cvk::SwapchainStatus::OutOfDate => {
                        log::debug!("Swapchain Out of Date");
                        if let Err(e) = self.swapchain.rebuild() {
                            log::error!("Failed to rebuild swapchain: {:?}", e);
                            return;
                        }
                    }
                    cvk::SwapchainStatus::Suboptimal => {
                        log::debug!("Suboptimal swapchain");
                        return;
                    }
                    cvk::SwapchainStatus::Optimal => {}
                }
                (frame, signals, status)
            }
            Err(e) => {
                log::error!("Failed to acquire next frame: {:?}", e);
                return;
            }
        };
        let mut recorder = self.queue.record();

        recorder.image_transition(&self.present_image, cvk::ImageTransition::Compute);
        recorder.image_transition(&self.normal_image, cvk::ImageTransition::Compute);
        recorder.image_transition(&self.depth_image, cvk::ImageTransition::Compute);

        recorder.bind_pipeline(&self.raytrace_pipeline);
        recorder.bind_descriptor_set(&self.gpu.descriptors, 0, &[]);
        recorder.push_constants(self.rtpc);

        let size = self.window.inner_size();
        let workgroup_size = 8;
        let workgroup_x = (size.width + workgroup_size - 1) / workgroup_size;
        let workgroup_y = (size.height + workgroup_size - 1) / workgroup_size;

        recorder.dispatch(workgroup_x, workgroup_y, 1);

        recorder.image_transition(&self.present_image, cvk::ImageTransition::FragmentRead);
        recorder.image_transition(&self.normal_image, cvk::ImageTransition::FragmentRead);
        recorder.image_transition(&self.depth_image, cvk::ImageTransition::FragmentRead);

        let color_attachment = cvk::RenderingAttachmentInfo::default()
            .image_view(frame.image.view)
            .image_layout(cvk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(cvk::AttachmentLoadOp::CLEAR)
            .store_op(cvk::AttachmentStoreOp::STORE)
            .clear_value(cvk::ClearValue {
                color: cvk::ClearColorValue {
                    float32: [0., 0., 0., 1.0],
                },
            });

        recorder.bind_pipeline(&self.present_pipeline);
        recorder.bind_descriptor_set(&self.gpu.descriptors, 0, &[]);
        recorder.push_constants(self.ppc);
        recorder.begin_rendering(&[color_attachment], self.swapchain.extent);

        recorder.viewport(cvk::Viewport {
            x: 0.,
            y: 0.,
            width: self.swapchain.extent.width as f32,
            height: self.swapchain.extent.height as f32,
            min_depth: 0.,
            max_depth: 1.,
        });

        recorder.scissor(cvk::Rect2D {
            offset: cvk::Offset2D { x: 0, y: 0 },
            extent: self.swapchain.extent,
        });

        recorder.draw(0..6, 0..1);
        recorder.end_rendering();

        self.egui.begin_frame(&self.window);
        egui::Window::new("Debug")
            .resizable(true)
            .show(&self.egui.ctx, |ui| {
                ui.label("Hello from egui!");
            });

        egui::Window::new("egui stuff")
            .resizable(true)
            .show(&self.egui.ctx, |ui| {
                ui.label("This is window 1");
                if ui.button("Click me!").clicked() {
                    println!("Button clicked!");
                }
                ui.text_edit_multiline(&mut String::new());
            });

        let output = self.egui.end_frame(&self.window);

        self.egui.render(&mut recorder, output, &self.queue, &frame);

        recorder.image_transition(&frame.image, cvk::ImageTransition::Present);

        let _ = self.queue.submit(
            &[recorder.finish()],
            &[(signals.available, cvk::PipelineStageFlags::TOP_OF_PIPE)],
            &[],
            &[signals.finished],
            &[],
        );

        match self.swapchain.present_frame(&self.queue, frame) {
            Ok(status) => match status {
                cvk::SwapchainStatus::OutOfDate => {
                    if let Err(e) = self.swapchain.rebuild() {
                        log::error!("Failed to rebuild swapchain: {:?}", e);
                    }
                    if let Err(e) = self.handle_resize() {
                        log::error!("Failed to handle resize: {:?}", e);
                    }
                }
                cvk::SwapchainStatus::Suboptimal => {
                    log::warn!("Suboptimal swapchain after present");
                }
                cvk::SwapchainStatus::Optimal => {}
            },
            Err(e) => {
                log::error!("Failed to present frame: {:?}", e);
            }
        }

        self.queue.wait(10);
    }

    fn rebind_descriptors(&self) {
        self.queue.wait_idle();
        let _lock = self.queue.lock();

        self.gpu.descriptors.write(&[
            cvk::DescriptorWrite::StorageImage {
                binding: 2,
                image_view: self.present_image.view,
                image_layout: cvk::ImageLayout::GENERAL,
                array_element: Some(0),
            },
            cvk::DescriptorWrite::StorageImage {
                binding: 2,
                image_view: self.normal_image.view,
                image_layout: cvk::ImageLayout::GENERAL,
                array_element: Some(1),
            },
            cvk::DescriptorWrite::StorageImage {
                binding: 2,
                image_view: self.depth_image.view,
                image_layout: cvk::ImageLayout::GENERAL,
                array_element: Some(2),
            },
            cvk::DescriptorWrite::SampledImage {
                binding: 3,
                image_view: self.present_image.view,
                image_layout: cvk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                array_element: Some(0),
            },
            cvk::DescriptorWrite::SampledImage {
                binding: 3,
                image_view: self.normal_image.view,
                image_layout: cvk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                array_element: Some(1),
            },
            cvk::DescriptorWrite::SampledImage {
                binding: 3,
                image_view: self.depth_image.view,
                image_layout: cvk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                array_element: Some(2),
            },
            cvk::DescriptorWrite::Sampler {
                binding: 4,
                sampler: &self.present_image.sampler(),
                array_element: Some(0),
            },
            cvk::DescriptorWrite::Sampler {
                binding: 4,
                sampler: &self.normal_image.sampler(),
                array_element: Some(1),
            },
            cvk::DescriptorWrite::Sampler {
                binding: 4,
                sampler: &self.depth_image.sampler(),
                array_element: Some(2),
            },
        ]);
    }

    fn handle_resize(&mut self) -> Result<()> {
        log::debug!("Resize Render Resources");
        let size = self.window.inner_size();
        self.egui.size = size;

        self.rtpc.set_resolution(size.width, size.height);
        let (present, normal, depth, depth_test) =
            Self::create_image_resources(&self.device, &self.swapchain);
        self.present_image = present;
        self.normal_image = normal;
        self.depth_image = depth;
        self.depth_test_image = depth_test;

        Ok(())
    }

    pub fn update_delta_time(&mut self, dt: time::Duration) {
        self.rtpc.dt = dt.as_secs_f32();
    }

    fn create_image_resources(
        device: &cvk::Device,
        sc: &cvk::Swapchain,
    ) -> (cvk::Image, cvk::Image, cvk::Image, cvk::Image) {
        let (width, height) = (
            sc.capabilities.current_extent.width,
            sc.capabilities.current_extent.height,
        );

        let present_image = device.create_image(&cvk::ImageInfo {
            label: Some("Present Image"),
            format: sc.format.format,
            width,
            height,
            usage: cvk::ImageUsageFlags::STORAGE | cvk::ImageUsageFlags::SAMPLED,
            view: cvk::ImageViewInfo {
                label: Some("Present Image View"),
                aspect: cvk::ImageAspectFlags::COLOR,
                ..Default::default()
            },
            sampler: Some(cvk::SamplerInfo {
                label: Some("Present Image Sampler"),
                max_lod: 100.,
                ..Default::default()
            }),
            ..Default::default()
        });

        let normal_image = device.create_image(&cvk::ImageInfo {
            label: Some("Normal Image"),
            format: sc.format.format,
            width,
            height,
            usage: cvk::ImageUsageFlags::STORAGE | cvk::ImageUsageFlags::SAMPLED,
            view: cvk::ImageViewInfo {
                label: Some("Normal Image View"),
                aspect: cvk::ImageAspectFlags::COLOR,
                ..Default::default()
            },
            sampler: Some(cvk::SamplerInfo {
                label: Some("Normal Image Sampler"),
                max_lod: 100.,
                ..Default::default()
            }),
            ..Default::default()
        });

        let depth_image = device.create_image(&cvk::ImageInfo {
            label: Some("Depth Image"),
            format: cvk::Format::R32_SFLOAT,
            width,
            height,
            usage: cvk::ImageUsageFlags::STORAGE | cvk::ImageUsageFlags::SAMPLED,
            view: cvk::ImageViewInfo {
                label: Some("Depth Image View"),
                aspect: cvk::ImageAspectFlags::COLOR,
                ..Default::default()
            },
            sampler: Some(cvk::SamplerInfo {
                label: Some("Depth Image Sampler"),
                max_lod: 100.,
                ..Default::default()
            }),
            ..Default::default()
        });

        let depth_test_image = device.create_image(&cvk::ImageInfo {
            label: Some("Depth Test Image"),
            format: cvk::Format::D32_SFLOAT,
            width,
            height,
            usage: cvk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            view: cvk::ImageViewInfo {
                label: Some("Depth Test Image View"),
                aspect: cvk::ImageAspectFlags::DEPTH,
                ..Default::default()
            },
            sampler: Some(cvk::SamplerInfo {
                label: Some("Depth Test Image Sampler"),
                max_lod: 100.,
                compare: Some(cvk::CompareOp::LESS_OR_EQUAL),
                ..Default::default()
            }),
            ..Default::default()
        });

        (present_image, normal_image, depth_image, depth_test_image)
    }
}
