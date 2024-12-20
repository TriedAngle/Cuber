extern crate nalgebra as na;
extern crate vk_mem as vkm;
use std::{ops::Deref, sync::Arc};

use anyhow::Result;
use ash::khr::swapchain;
use cvk::{raw::vk, utils, Device};

use game::Camera;
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::KeyCode,
    window::{Window, WindowAttributes},
};

// WGSL shader that will be converted to SPIR-V
const COMPUTE_SHADER: &str = r#"
@group(0) @binding(0) var output_texture: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(output_texture);
    let pos = vec2<f32>(f32(global_id.x) / f32(dims.x), 
                        f32(global_id.y) / f32(dims.y));
    
    textureStore(output_texture, global_id.xy, vec4<f32>(1.0 - pos.x, 1.0 - pos.y, 1.0, 1.0));
}
"#;

const PRSENT_SHADER: &str = r#"
@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var tex_sampler: sampler;

@vertex
fn vmain(@builtin(vertex_index) vert_idx: u32) -> @builtin(position) vec4<f32> {
    var pos = array<vec2<f32>, 4>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0,  1.0)
    );
    return vec4<f32>(pos[vert_idx], 0.0, 1.0);
}

@fragment
fn fmain(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    return textureSample(tex, tex_sampler, pos.xy);
}
"#;

struct Render {
    surface: Arc<cvk::Surface>,
    swapchain: cvk::Swapchain,
    instance: Arc<cvk::Instance>,
    device: Arc<cvk::Device>,
    compute_queue: Arc<cvk::Queue>,
    present_queue: Arc<cvk::Queue>,
    transfer_queue: Arc<cvk::Queue>,
}

impl Render {
    pub fn new(
        instance: Arc<cvk::Instance>,
        device: Arc<cvk::Device>,
        compute_queue: Arc<cvk::Queue>,
        present_queue: Arc<cvk::Queue>,
        transfer_queue: Arc<cvk::Queue>,
        window: &Window,
    ) -> Result<Self> {
        let size = window.inner_size();
        let surface = instance.create_surface(&device.adapter(), &window, |formats| {
            formats
                .iter()
                .find(|f| f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR)
                .map(|f| *f)
                .inspect(|f| log::debug!("using {:?}", f))
                .unwrap_or(formats[0])
        })?;

        println!("formats: {:?}", surface.formats());
        println!("caps: {:?}", surface.capabilities());

        assert!(surface.is_compatible(&device.adapter(), &present_queue));

        let compute_shader = device.create_shader(COMPUTE_SHADER)?;
        let present_shader = device.create_shader(PRSENT_SHADER)?;
        let present_texture =
            device.create_texture(surface.format().format, size.width, size.height);

        let swapchain = device.create_swapchain(surface.clone(), 3, vk::PresentModeKHR::MAILBOX)?;

        let new = Self {
            instance,
            surface,
            device,
            compute_queue,
            present_queue,
            transfer_queue,
            swapchain,
        };

        Ok(new)
    }

    fn render(&mut self) {
        let (frame, signals, _suboptimal) = self.swapchain.acquire_next_frame(None);

        let mut recorder = self.present_queue.record();

        recorder.image_barrier(
            frame.image,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            vk::AccessFlags::empty(),
            vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
        );

        let color_attachment = vk::RenderingAttachmentInfo::default()
            .image_view(frame.view)
            .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.0, 0.2, 0.4, 1.0],
                },
            });

        recorder.begin_rendering(&[color_attachment], self.swapchain.extent);

        recorder.end_rendering();

        recorder.image_barrier(
            frame.image,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            vk::ImageLayout::PRESENT_SRC_KHR,
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            vk::PipelineStageFlags::BOTTOM_OF_PIPE,
            vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            vk::AccessFlags::empty(),
        );

        let _ = self.present_queue.submit(
            &[recorder.finish()],
            &[(
                signals.available,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            )],
            &[],
            &[signals.finished],
            &[],
        );

        self.swapchain.present_frame(&self.present_queue, frame);
    }
}

struct VulkanApp {
    camera: Camera,
    instance: Arc<cvk::Instance>,
    device: Arc<cvk::Device>,
    compute_queue: Arc<cvk::Queue>,
    present_queue: Arc<cvk::Queue>,
    transfer_queue: Arc<cvk::Queue>,
    window: Option<Window>,
    render: Option<Render>,
}

impl VulkanApp {
    pub fn new(_event_loop: &EventLoop<()>) -> Result<Self> {
        let instance = Arc::new(cvk::Instance::new("Cuber", "Cuber Engine")?);

        let formats = &[
            cvk::Format::R8G8B8_UNORM,
            cvk::Format::R8G8B8_SRGB,
            cvk::Format::D16_UNORM,
            cvk::Format::D32_SFLOAT,
            cvk::Format::R32_SFLOAT,
        ];

        let adapters = instance.adapters(formats)?;
        let adapter = adapters[0].clone();
        utils::print_queues_pretty(&adapter);

        let (device, queues) = Device::new(
            instance.clone(),
            adapter.clone(),
            &[
                cvk::QueueRequest {
                    required_flags: cvk::QueueFlags::COMPUTE | cvk::QueueFlags::TRANSFER,
                    exclude_flags: cvk::QueueFlags::GRAPHICS,
                    strict: true,
                    allow_fallback_share: true,
                },
                cvk::QueueRequest {
                    required_flags: cvk::QueueFlags::GRAPHICS | cvk::QueueFlags::TRANSFER,
                    exclude_flags: cvk::QueueFlags::COMPUTE,
                    strict: true,
                    allow_fallback_share: true,
                },
                cvk::QueueRequest {
                    required_flags: cvk::QueueFlags::TRANSFER,
                    exclude_flags: cvk::QueueFlags::COMPUTE | cvk::QueueFlags::GRAPHICS,
                    strict: true,
                    allow_fallback_share: true,
                },
            ],
        )?;

        let compute_queue = queues[0].clone();
        let present_queue = queues[1].clone();
        let transfer_queue = queues[2].clone();

        let camera = Camera::new(
            na::Point3::new(0., 0., 0.),
            na::UnitQuaternion::identity(),
            0.5,
            0.05,
            45.,
            16. / 9.,
            0.1,
            1000.,
        );

        let new = Self {
            camera,
            instance,
            device,
            compute_queue,
            present_queue,
            transfer_queue,
            window: None,
            render: None,
        };

        Ok(new)
    }
}

impl ApplicationHandler for VulkanApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let attributes = WindowAttributes::default().with_title("Cuber");
        let window = event_loop.create_window(attributes).unwrap();
        let render = Render::new(
            self.instance.clone(),
            self.device.clone(),
            self.compute_queue.clone(),
            self.present_queue.clone(),
            self.transfer_queue.clone(),
            &window,
        )
        .unwrap();
        self.window = Some(window);
        self.render = Some(render);
    }

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, _cause: winit::event::StartCause) {}

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        match event {
            DeviceEvent::Key(key) => {
                if key.physical_key == KeyCode::Escape && key.state.is_pressed() {
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        match event {
            WindowEvent::RedrawRequested => {
                if let Some(render) = &mut self.render {
                    render.render();
                }
            }
            _ => {}
        }
    }
}

pub fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .init();

    let event_loop = EventLoop::builder().build().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = VulkanApp::new(&event_loop).unwrap();
    event_loop.run_app(&mut app).unwrap();
}
