use std::{mem, sync::Arc};

use anyhow::Result;
use cvk::{egui as cvkui, raw::vk};
use winit::window::Window;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ComputeUniforms {
    pub resolution: [f32; 2],
    pub dt: f32,
    pub render_mode: u32,
    pub brick_grid_dimension: [u32; 3],
    pub depth_boost: f32,
    pub view_projection: [[f32; 4]; 4],
    pub inverse_view_projection: [[f32; 4]; 4],
    pub camera_position: [f32; 3],
    pub brick_hit_flags: u32,
    pub brick_hit: [u32; 3], // TODO: probably merge these
    pub voxel_hit_flags: u32,
    pub voxel_hit: [u32; 3],
    pub disable_sdf: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RayTracePushConstants {
    pub camera: [[f32; 4]; 4],
    pub camera_inverse: [[f32; 4]; 4],
    pub dimensions: [u32; 3],
    pub resolution: u32, // packed
    pub flags: u64,
    pub dt: f32,
    pub depth_boost: f32,
    pub brick_hit: [u32; 3],
    pub _padding0: u32,
    pub voxel_hit: [u32; 3],
    pub _padding1: u32,
}

impl RayTracePushConstants {
    pub fn empty() -> Self {
        Self {
            camera: [[0.; 4]; 4],
            camera_inverse: [[0.; 4]; 4],
            dimensions: [0; 3],
            resolution: 0,
            flags: 0,
            dt: 0.,
            depth_boost: 0.,
            brick_hit: [0; 3],
            _padding0: 0,
            voxel_hit: [0; 3],
            _padding1: 0,
        }
    }
}

pub struct RenderContext {
    gpu: Arc<cgpu::GPUContext>,
    queue: Arc<cvk::Queue>,
    device: Arc<cvk::Device>,
    pub window: Arc<Window>,
    swapchain: cvk::Swapchain,
    // raytrace_pipeline: cvk::ComputePipeline,
    // present_pipeline: cvk::RenderPipeline,
    rt_complete: cvk::Semaphore,
    rtpc: RayTracePushConstants,
    pub egui: cvkui::EguiState,
}

impl RenderContext {
    pub fn new(gpu: Arc<cgpu::GPUContext>, window: Arc<Window>) -> Result<Self> {
        let device = gpu.device.clone();
        let queue = gpu.render_queue.clone();

        // let raytrace_shader = device.create_shader(include_str!("shaders/raytrace.wgsl"))?;
        // let present_shader = device.create_shader(include_str!("shaders/present.wgsl"))?;
        //
        // let raytrace_pipeline = device.create_compute_pipeline(&cvk::ComputePipelineInfo {
        //     label: Some("Raytrace Pipeline"),
        //     shader: raytrace_shader.entry("main"),
        //     descriptor_layouts: &[&gpu.layout],
        //     push_constant_size: Some(mem::size_of::<RayTracePushConstants>() as u32),
        //     ..Default::default()
        // });
        //
        // let present_pipeline = device.create_render_pipeline(&cvk::RenderPipelineInfo {
        //     label: Some("Present Pipeline"),
        //     vertex_shader: present_shader.entry("vmain"),
        //     fragment_shader: present_shader.entry("fmain"),
        //     descriptor_layouts: &[&gpu.layout],
        //     push_constant_size: Some(mem::size_of::<RayTracePushConstants>() as u32),
        //     cull: cvk::CullModeFlags::BACK,
        //     ..Default::default()
        // });
        //
        let rt_complete = device.create_binary_semaphore(false);

        let rtpc = RayTracePushConstants::empty();

        let swapchain = device.create_swapchain(
            cvk::SwapchainConfig {
                preferred_image_count: 3,
                preferred_present_mode: vk::PresentModeKHR::MAILBOX,
                format_selector: Box::new(|formats| {
                    formats
                        .iter()
                        .find(|f| {
                            f.format == vk::Format::R8G8B8A8_UNORM
                                && f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
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

        let egui = cvkui::EguiState::new(
            device.clone(),
            &window,
            swapchain.format.format,
            swapchain.frames_in_flight,
            scale_factor,
            fonts,
            style,
        );

        Ok(Self {
            gpu,
            device,
            queue,
            window,
            swapchain,
            // raytrace_pipeline,
            // present_pipeline,
            rt_complete,
            rtpc,
            egui,
        })
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
                    // if let Err(e) = self.handle_resize() {
                    //     log::error!("Failed to handle resize: {:?}", e);
                    // }
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
}
