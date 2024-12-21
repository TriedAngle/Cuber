extern crate nalgebra as na;
extern crate vk_mem as vkm;
use std::{mem, ops::Deref, sync::Arc};

use anyhow::Result;
use bytemuck::Zeroable;
use cvk::{raw::vk, utils, Device};

use game::Camera;
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::KeyCode,
    window::{Window, WindowAttributes},
};

const COMPUTE_SHADER: &str = r#"
struct PushConstants {
    window: vec2<u32>,
    mouse: vec2<f32>,
}

var<push_constant> pc: PushConstants;

@group(0) @binding(0)
var output_texture: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let coords = vec2<u32>(global_id.xy);
    
    let uv = vec2<f32>(
        f32(coords.x) / f32(pc.window.x),
        f32(coords.y) / f32(pc.window.y),
    );
    
    let mouse = vec2<f32>(
        pc.mouse.x / f32(pc.window.x),
        pc.mouse.y / f32(pc.window.y),
    );
    
    let dist = distance(uv, mouse);
    
    let radius = 0.1;
    let glow = smoothstep(radius, 0.0, dist);
    
    let checker_size = 32u;
    let is_white = ((coords.x / checker_size) + (coords.y / checker_size)) % 2u == 0u;
    let base_color = select(
        vec4<f32>(1.0, 0.0, 0.0, 1.0),
        vec4<f32>(1.0, 1.0, 1.0, 1.0),
        is_white
    );
    
    let glow_color = vec4<f32>(0.0, 0.5, 1.0, 1.0);
    let final_color = mix(base_color, glow_color, glow);
    
    textureStore(output_texture, coords, final_color);
}"#;

const PRSENT_SHADER: &str = r#"
struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(0) @binding(1) var tex: texture_2d<f32>;
@group(0) @binding(2) var tex_sampler: sampler;

@vertex
fn vmain(@builtin(vertex_index) vert_idx: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),  // bottom-left
        vec2<f32>(-1.0,  1.0),  // top-left
        vec2<f32>( 1.0, -1.0),  // bottom-right
        
        // Second triangle (CCW)
        vec2<f32>(-1.0,  1.0),  // top-left
        vec2<f32>( 1.0,  1.0),  // top-right
        vec2<f32>( 1.0, -1.0)   // bottom-right
    );
    
    var uvs = array<vec2<f32>, 6>(
        // First triangle
        vec2<f32>(0.0, 1.0),  // bottom-left
        vec2<f32>(0.0, 0.0),  // top-left
        vec2<f32>(1.0, 1.0),  // bottom-right
        
        // Second triangle
        vec2<f32>(0.0, 0.0),  // top-left
        vec2<f32>(1.0, 0.0),  // top-right
        vec2<f32>(1.0, 1.0)   // bottom-right
    );

    var output: VertexOutput;
    output.pos = vec4<f32>(positions[vert_idx], 0.0, 1.0);
    output.uv = uvs[vert_idx];
    return output;
}

@fragment
fn fmain(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    return textureSample(tex, tex_sampler, uv);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct PushConstants {
    window: [u32; 2],
    mouse: [f32; 2],
}

struct Render {
    surface: Arc<cvk::Surface>,
    window: Arc<Window>,
    swapchain: cvk::Swapchain,
    compute_queue: Arc<cvk::Queue>,
    render_queue: Arc<cvk::Queue>,
    transfer_queue: Arc<cvk::Queue>,
    compute_pipeline: cvk::ComputePipeline,
    present_pipeline: cvk::RenderPipeline,
    descriptor_layout: cvk::DescriptorSetLayout,
    descriptor_pool: Arc<cvk::DescriptorPool>,
    descriptor_set: cvk::DescriptorSet,
    present_texture: cvk::Texture,
    compute_complete: cvk::Semaphore,
    instance: Arc<cvk::Instance>,
    device: Arc<cvk::Device>,
    pc: PushConstants,
}

impl Render {
    pub fn new(
        instance: Arc<cvk::Instance>,
        device: Arc<cvk::Device>,
        compute_queue: Arc<cvk::Queue>,
        render_queue: Arc<cvk::Queue>,
        transfer_queue: Arc<cvk::Queue>,
        window: Arc<Window>,
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

        assert!(surface.is_compatible(&device.adapter(), &render_queue));

        let compute_shader = device.create_shader(COMPUTE_SHADER)?;
        let present_shader = device.create_shader(PRSENT_SHADER)?;

        let layout = device.create_descriptor_set_layout(&cvk::DescriptorSetLayoutInfo {
            flags: vk::DescriptorSetLayoutCreateFlags::empty(),
            label: Some("Present Descriptor Set"),
            bindings: &[
                cvk::DescriptorBinding {
                    binding: 0,
                    ty: cvk::DescriptorType::StorageImage,
                    count: 1,
                    stages: vk::ShaderStageFlags::COMPUTE,
                    flags: None,
                },
                cvk::DescriptorBinding {
                    binding: 1,
                    ty: cvk::DescriptorType::SampledImage,
                    count: 1,
                    stages: vk::ShaderStageFlags::FRAGMENT,
                    flags: None,
                },
                cvk::DescriptorBinding {
                    binding: 2,
                    ty: cvk::DescriptorType::Sampler,
                    count: 1,
                    stages: vk::ShaderStageFlags::FRAGMENT,
                    flags: None,
                },
            ],

            ..Default::default()
        });

        let pool = device.create_descriptor_pool(&cvk::DescriptorPoolInfo {
            max_sets: 1,
            layouts: &[&layout],
            flags: vk::DescriptorPoolCreateFlags::empty(),
            label: Some("Present Descriptor Set"),
            tag: None,
        });

        let descriptor_set = device.create_descriptor_set(pool.clone(), &layout);

        let present_texture = device.create_texture(&cvk::TextureInfo {
            format: surface.format().format,
            width: size.width,
            height: size.height,
            usage: vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::SAMPLED,
            sharing: vk::SharingMode::EXCLUSIVE,
            usage_locality: vkm::MemoryUsage::AutoPreferDevice,
            allocation_locality: vk::MemoryPropertyFlags::DEVICE_LOCAL,
            aspect_mask: vk::ImageAspectFlags::COLOR,
            layout: vk::ImageLayout::UNDEFINED,
            sampler: Some(cvk::SamplerInfo::default()),
            label: Some("Debug Present Texture"),
            ..Default::default()
        });

        descriptor_set.write(&[
            cvk::DescriptorWrite::StorageImage {
                binding: 0,
                image_view: present_texture.view,
                image_layout: vk::ImageLayout::GENERAL,
            },
            cvk::DescriptorWrite::SampledImage {
                binding: 1,
                image_view: present_texture.view,
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            },
            cvk::DescriptorWrite::Sampler {
                binding: 2,
                sampler: present_texture.sampler.unwrap(),
            },
        ]);

        let compute_pipeline = device.create_compute_pipeline(&cvk::ComputePipelineInfo {
            shader: compute_shader.entry("main"),
            descriptor_layouts: &[&layout],
            push_constant_size: Some(mem::size_of::<PushConstants>() as u32),
            cache: None,
            label: Some("Compute Pipeline"),
            tag: None,
        });

        let present_pipeline = device.create_render_pipeline(&cvk::RenderPipelineInfo {
            vertex_shader: present_shader.entry("vmain"),
            fragment_shader: present_shader.entry("fmain"),
            color_formats: &[surface.format().format],
            depth_format: None,
            descriptor_layouts: &[&layout],
            push_constant_size: None,
            blend_states: None,
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            polygon: vk::PolygonMode::FILL,
            cull: vk::CullModeFlags::BACK,
            front_face: vk::FrontFace::COUNTER_CLOCKWISE,
            label: Some("Present Pipeline"),
            tag: None,
        });

        let swapchain = device.create_swapchain(surface.clone(), 3, vk::PresentModeKHR::MAILBOX)?;

        let compute_complete = device.create_binary_semaphore(false);

        let pc = PushConstants {
            window: [size.width, size.height],
            mouse: [0.; 2],
        };

        let new = Self {
            instance,
            surface,
            window,
            device,
            compute_queue,
            render_queue,
            transfer_queue,
            swapchain,

            compute_pipeline,
            present_pipeline,
            descriptor_layout: layout,
            descriptor_pool: pool,
            descriptor_set,
            present_texture,
            compute_complete,
            pc,
        };

        Ok(new)
    }

    fn render(&mut self) {
        let (frame, signals, _suboptimal) = self.swapchain.acquire_next_frame(None);

        let mut recorder = self.render_queue.record();

        recorder.image_transition(&self.present_texture, cvk::ImageTransition::General);

        recorder.bind_pipeline(&self.compute_pipeline);

        recorder.bind_descriptor_set(&self.descriptor_set, 0, &[]);

        recorder.push_constants(self.pc);

        let width = (self.swapchain.extent.width + 15) / 16;
        let height = (self.swapchain.extent.height + 15) / 16;

        recorder.dispatch(width, height, 1);

        recorder.image_transition(&self.present_texture, cvk::ImageTransition::ShaderRead);

        recorder.image_transition(&frame, cvk::ImageTransition::ColorAttachment);

        let color_attachment = vk::RenderingAttachmentInfo::default()
            .image_view(frame.view)
            .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 1.0],
                },
            });

        recorder.begin_rendering(&[color_attachment], self.swapchain.extent);

        recorder.bind_pipeline(&self.present_pipeline);

        recorder.bind_descriptor_set(&self.descriptor_set, 0, &[]);

        recorder.viewport(vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: self.swapchain.extent.width as f32,
            height: self.swapchain.extent.height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        });

        recorder.scissor(vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: self.swapchain.extent,
        });

        recorder.draw(0..6, 0..1);

        recorder.end_rendering();

        recorder.image_transition(&frame, cvk::ImageTransition::Present);

        let command_buffer = recorder.finish();

        let _ = self.render_queue.submit(
            &[command_buffer],
            &[(signals.available, vk::PipelineStageFlags::TOP_OF_PIPE)],
            &[],
            &[signals.finished],
            &[],
        );

        self.swapchain.present_frame(&self.render_queue, frame);
        self.window.request_redraw();
    }
}

struct VulkanApp {
    camera: Camera,
    instance: Arc<cvk::Instance>,
    device: Arc<cvk::Device>,
    compute_queue: Arc<cvk::Queue>,
    render_queue: Arc<cvk::Queue>,
    transfer_queue: Arc<cvk::Queue>,
    window: Option<Arc<Window>>,
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
                    exclude_flags: cvk::QueueFlags::empty(),
                    strict: false,
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
        let render_queue = queues[1].clone();
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

        Ok(Self {
            camera,
            instance,
            device,
            compute_queue,
            render_queue,
            transfer_queue,
            window: None,
            render: None,
        })
    }
}

impl ApplicationHandler for VulkanApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let attributes = WindowAttributes::default().with_title("Cuber");
        let window = event_loop.create_window(attributes).unwrap();
        let window = Arc::new(window);
        let render = Render::new(
            self.instance.clone(),
            self.device.clone(),
            self.compute_queue.clone(),
            self.render_queue.clone(),
            self.transfer_queue.clone(),
            window.clone(),
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
                    render.render()
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(render) = &mut self.render {
                    let height = render.swapchain.extent.height as f32;
                    render.pc.mouse = [position.x as f32, height - position.y as f32];
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
