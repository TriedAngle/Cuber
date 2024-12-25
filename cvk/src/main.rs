extern crate nalgebra as na;
extern crate vk_mem as vkm;
use std::{mem, sync::Arc};

use anyhow::Result;
use cvk::{
    egui::EguiState, raw::vk, utils, Device, ImageViewInfo, SwapchainConfig, SwapchainStatus,
};

use game::Camera;
use rand::Rng;
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::KeyCode,
    window::{Window, WindowAttributes},
};

const COMPUTE_SHADER: &str = r#"
struct Particle {
    position: vec2<f32>,
    velocity: vec2<f32>,
    color: vec4<f32>,
}

struct PushConstants {
    window: vec2<u32>,
    mouse: vec2<f32>,
    delta_time: f32,
}

@group(0) @binding(0)
var output_texture: texture_storage_2d<rgba8unorm, read_write>;

@group(0) @binding(1)
var<storage, read_write> particles: array<Particle>;

var<push_constant> pc: PushConstants;

@compute @workgroup_size(16, 16, 1)
fn clear(@builtin(global_invocation_id) global_id: vec3<u32>) {
    if (global_id.x >= pc.window.x || global_id.y >= pc.window.y) {
        return;
    }
    
    let current = textureLoad(output_texture, vec2<i32>(global_id.xy));
    
    let fade_speed = 0.95; 
    let faded = vec4<f32>(current.rgb * fade_speed, current.a);
    
    textureStore(output_texture, vec2<i32>(global_id.xy), faded);
}


@compute @workgroup_size(256, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    var particle = particles[global_id.x];
    
    particle.position += particle.velocity * pc.delta_time;
    
    // Bounce off screen edges
    if (particle.position.x <= 0.0 || particle.position.x >= f32(pc.window.x)) {
        particle.velocity.x = -particle.velocity.x;
    }
    if (particle.position.y <= 0.0 || particle.position.y >= f32(pc.window.y)) {
        particle.velocity.y = -particle.velocity.y;
    }
    
    let mouse_pos = pc.mouse;
    let to_mouse = mouse_pos - particle.position;
    let dist = length(to_mouse);
    
    let min_dist = -2.0;
    if (dist > min_dist) {
        let force = normalize(to_mouse) * 800.0 / (dist);
        particle.velocity += force * pc.delta_time;
    } else {
        let repel = normalize(-to_mouse) * 400.0;
        particle.velocity += repel * pc.delta_time;
    }
    
    particle.velocity *= 0.995;
    
    let max_speed = 400.0;
    let current_speed = length(particle.velocity);
    if (current_speed > max_speed) {
        particle.velocity = normalize(particle.velocity) * max_speed;
    }
    
    particles[global_id.x] = particle;
    
    let pos = vec2<i32>(particle.position);
    if (pos.x >= 0 && pos.x < i32(pc.window.x) && 
        pos.y >= 0 && pos.y < i32(pc.window.y)) {
        let current = textureLoad(output_texture, pos);
        let blended = max(current, particle.color);  // Additive blending
        textureStore(output_texture, pos, blended);
    }
}
"#;

const PRSENT_SHADER: &str = r#"
struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(0) @binding(2)
var tex: texture_2d<f32>;
@group(0) @binding(3)
var tex_sampler: sampler;

@vertex
fn vmain(@builtin(vertex_index) vert_idx: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),  // bottom-left
        vec2<f32>(-1.0,  1.0),  // top-left
        vec2<f32>( 1.0, -1.0),  // bottom-right
        
        vec2<f32>(-1.0,  1.0),  // top-left
        vec2<f32>( 1.0,  1.0),  // top-right
        vec2<f32>( 1.0, -1.0)   // bottom-right
    );
    
    var uvs = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 1.0),  // bottom-left
        vec2<f32>(0.0, 0.0),  // top-left
        vec2<f32>(1.0, 1.0),  // bottom-right
        
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
    dt: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Particle {
    position: [f32; 2],
    velocity: [f32; 2],
    color: [f32; 4],
}

impl Default for Particle {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0],
            velocity: [0.0, 0.0],
            color: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

const PARTICLE_COUNT: usize = 32000;

#[allow(unused)]
struct Render {
    window: Arc<Window>,
    swapchain: cvk::Swapchain,
    compute_queue: Arc<cvk::Queue>,
    render_queue: Arc<cvk::Queue>,
    transfer_queue: Arc<cvk::Queue>,
    egui: EguiState,
    compute_pipeline: cvk::ComputePipeline,
    clear_pipeline: cvk::ComputePipeline,
    present_pipeline: cvk::RenderPipeline,
    descriptor_layout: cvk::DescriptorSetLayout,
    descriptor_pool: Arc<cvk::DescriptorPool>,
    descriptor_set: cvk::DescriptorSet,
    present_texture: cvk::Image,
    pc: PushConstants,
    particle_buffer: cvk::Buffer,
    compute_complete: cvk::Semaphore,
    instance: Arc<cvk::Instance>,
    device: Arc<cvk::Device>,
    editor_text: String,
    max_frame: Option<u64>,
    frame_count: u64,
}

impl Render {
    pub fn new(
        instance: Arc<cvk::Instance>,
        device: Arc<cvk::Device>,
        compute_queue: Arc<cvk::Queue>,
        render_queue: Arc<cvk::Queue>,
        transfer_queue: Arc<cvk::Queue>,
        window: Arc<Window>,
        max_frame: Option<u64>,
    ) -> Result<Self> {
        let size = window.inner_size();

        let config = SwapchainConfig {
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
        };

        let swapchain = device.create_swapchain(config, &window)?;

        // assert!(swapchain.is_compatible(&device.adapter(), &render_queue));

        let mut particles = vec![Particle::default(); PARTICLE_COUNT];

        let rng = &mut rand::thread_rng();

        for particle in &mut particles {
            particle.position = [
                rng.gen_range(0.0..size.width as f32),
                rng.gen_range(0.0..size.height as f32),
            ];
            particle.velocity = [rng.gen_range(-30.0..30.0), rng.gen_range(-30.0..30.0)];
            particle.color = [
                rng.gen_range(0.5..1.0),
                rng.gen_range(0.5..1.0),
                rng.gen_range(0.5..1.0),
                1.0,
            ];
        }

        let particle_buffer = device.create_buffer(&cvk::BufferInfo {
            size: (std::mem::size_of::<Particle>() * PARTICLE_COUNT) as u64,
            usage: vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
            sharing: vk::SharingMode::EXCLUSIVE,
            usage_locality: vkm::MemoryUsage::AutoPreferDevice,
            allocation_locality: vk::MemoryPropertyFlags::HOST_VISIBLE
                | vk::MemoryPropertyFlags::HOST_COHERENT
                | vk::MemoryPropertyFlags::DEVICE_LOCAL,
            host_access: Some(vkm::AllocationCreateFlags::HOST_ACCESS_SEQUENTIAL_WRITE),
            label: Some("Particle Buffer"),
            ..Default::default()
        });

        particle_buffer.upload(bytemuck::cast_slice(&particles), 0);

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
                    ty: cvk::DescriptorType::StorageBuffer,
                    count: 1,
                    stages: vk::ShaderStageFlags::COMPUTE,
                    flags: None,
                },
                cvk::DescriptorBinding {
                    binding: 2,
                    ty: cvk::DescriptorType::SampledImage,
                    count: 1,
                    stages: vk::ShaderStageFlags::FRAGMENT,
                    flags: None,
                },
                cvk::DescriptorBinding {
                    binding: 3,
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

        let present_texture = device.create_texture(&cvk::ImageInfo {
            format: swapchain.format().format,
            width: size.width,
            height: size.height,
            usage: vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::SAMPLED,
            sharing: vk::SharingMode::EXCLUSIVE,
            usage_locality: vkm::MemoryUsage::AutoPreferDevice,
            allocation_locality: vk::MemoryPropertyFlags::DEVICE_LOCAL,
            view: ImageViewInfo {
                aspect: vk::ImageAspectFlags::COLOR,
                ..Default::default()
            },
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
                array_element: None,
            },
            cvk::DescriptorWrite::StorageBuffer {
                binding: 1,
                buffer: &particle_buffer,
                offset: 0,
                range: vk::WHOLE_SIZE,
                array_element: None,
            },
            cvk::DescriptorWrite::SampledImage {
                binding: 2,
                image_view: present_texture.view,
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                array_element: None,
            },
            cvk::DescriptorWrite::Sampler {
                binding: 3,
                sampler: present_texture.sampler(),
                array_element: None,
            },
        ]);

        let compute_shader = device.create_shader(COMPUTE_SHADER)?;
        let present_shader = device.create_shader(PRSENT_SHADER)?;

        let compute_pipeline = device.create_compute_pipeline(&cvk::ComputePipelineInfo {
            shader: compute_shader.entry("main"),
            descriptor_layouts: &[&layout],
            push_constant_size: Some(mem::size_of::<PushConstants>() as u32),
            cache: None,
            label: Some("Compute Pipeline"),
            tag: None,
        });

        let clear_pipeline = device.create_compute_pipeline(&cvk::ComputePipelineInfo {
            shader: compute_shader.entry("clear"),
            descriptor_layouts: &[&layout],
            push_constant_size: Some(mem::size_of::<PushConstants>() as u32),
            cache: None,
            label: Some("Compute Pipeline"),
            tag: None,
        });

        let present_pipeline = device.create_render_pipeline(&cvk::RenderPipelineInfo {
            vertex_shader: present_shader.entry("vmain"),
            fragment_shader: present_shader.entry("fmain"),
            color_formats: &[swapchain.format.format],
            depth_format: None,
            descriptor_layouts: &[&layout],
            push_constant_size: None,
            blend_states: None,
            vertex_input_state: None,
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            polygon: vk::PolygonMode::FILL,
            cull: vk::CullModeFlags::BACK,
            front_face: vk::FrontFace::COUNTER_CLOCKWISE,
            label: Some("Present Pipeline"),
            tag: None,
        });

        let compute_complete = device.create_binary_semaphore(false);

        let pc = PushConstants {
            window: [size.width, size.height],
            mouse: [0.; 2],
            dt: 0.,
        };

        let scale_factor = window.scale_factor();
        let fonts = egui::FontDefinitions::default();
        let style = egui::Style::default();

        let egui = EguiState::new(
            device.clone(),
            &window,
            swapchain.format.format,
            swapchain.frames_in_flight,
            scale_factor,
            fonts,
            style,
        );

        let new = Self {
            instance,
            window,
            device,
            compute_queue,
            render_queue,
            transfer_queue,
            swapchain,
            egui,
            compute_pipeline,
            clear_pipeline,
            present_pipeline,
            descriptor_layout: layout,
            descriptor_pool: pool,
            descriptor_set,
            present_texture,
            compute_complete,
            pc,
            particle_buffer,
            editor_text: String::new(),
            max_frame,
            frame_count: 0,
        };

        Ok(new)
    }

    fn render(&mut self) {
        let (frame, signals, _status) = match self.swapchain.acquire_next_frame(None) {
            Ok((frame, signals, status)) => {
                match status {
                    SwapchainStatus::OutOfDate => {
                        log::debug!("Swapchain Out of Date");
                        if let Err(e) = self.swapchain.rebuild() {
                            log::error!("Failed to rebuild swapchain: {:?}", e);
                            return;
                        }
                        if let Err(e) = self.handle_resize() {
                            log::error!("Failed to handle resize: {:?}", e);
                            return;
                        }
                        return;
                    }
                    SwapchainStatus::Suboptimal => {
                        log::debug!("Suboptimal swapchain");
                        if let Err(e) = self.handle_resize() {
                            log::error!("Failed to handle resize: {:?}", e);
                            return;
                        }
                        return;
                    }
                    SwapchainStatus::Optimal => {}
                }
                (frame, signals, status)
            }
            Err(e) => {
                log::error!("Failed to acquire next frame: {:?}", e);
                return;
            }
        };

        let mut recorder = self.render_queue.record();

        recorder.image_transition(&self.present_texture, cvk::ImageTransition::Compute);

        let particle_groups = (PARTICLE_COUNT + 255) / 256;
        recorder.bind_pipeline(&self.compute_pipeline);
        recorder.bind_descriptor_set(&self.descriptor_set, 0, &[]);
        recorder.push_constants(self.pc);
        recorder.dispatch(particle_groups as u32, 1, 1);

        let width = (self.swapchain.extent.width + 15) / 16;
        let height = (self.swapchain.extent.height + 15) / 16;
        recorder.bind_pipeline(&self.clear_pipeline);
        recorder.bind_descriptor_set(&self.descriptor_set, 0, &[]);
        recorder.push_constants(self.pc);
        recorder.dispatch(width, height, 1);

        recorder.image_transition(&self.present_texture, cvk::ImageTransition::FragmentRead);

        let color_attachment = vk::RenderingAttachmentInfo::default()
            .image_view(frame.image.view)
            .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 1.0],
                },
            });

        recorder.bind_pipeline(&self.present_pipeline);

        recorder.bind_descriptor_set(&self.descriptor_set, 0, &[]);

        recorder.begin_rendering(&[color_attachment], self.swapchain.extent);

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

        let _ = self.render_queue.submit(
            &[recorder.finish()],
            &[(signals.available, vk::PipelineStageFlags::TOP_OF_PIPE)],
            &[],
            &[],
            &[],
        );

        let mut egui_recorder = self.render_queue.record();

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
                ui.text_edit_multiline(&mut self.editor_text);
            });

        let egui_output = self.egui.end_frame(&self.window);

        self.egui
            .render(&mut egui_recorder, egui_output, &self.render_queue, &frame);

        egui_recorder.image_transition(&frame.image, cvk::ImageTransition::Present);

        let _ = self.render_queue.submit(
            &[egui_recorder.finish()],
            &[],
            &[],
            &[signals.finished],
            &[],
        );

        match self.swapchain.present_frame(&self.render_queue, frame) {
            Ok(status) => match status {
                SwapchainStatus::OutOfDate => {
                    if let Err(e) = self.swapchain.rebuild() {
                        log::error!("Failed to rebuild swapchain: {:?}", e);
                    }
                    if let Err(e) = self.handle_resize() {
                        log::error!("Failed to handle resize: {:?}", e);
                    }
                }
                SwapchainStatus::Suboptimal => {
                    log::warn!("Suboptimal swapchain after present");
                }
                SwapchainStatus::Optimal => {}
            },
            Err(e) => {
                log::error!("Failed to present frame: {:?}", e);
            }
        }

        self.render_queue.wait(10);

        self.window.request_redraw();
    }

    fn handle_resize(&mut self) -> Result<()> {
        let size = self.window.inner_size();

        self.pc.window = [size.width, size.height];
        self.egui.size = size;

        let new_texture = self.device.create_texture(&cvk::ImageInfo {
            format: self.swapchain.format().format,
            width: size.width,
            height: size.height,
            usage: vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::SAMPLED,
            sharing: vk::SharingMode::EXCLUSIVE,
            usage_locality: vkm::MemoryUsage::AutoPreferDevice,
            allocation_locality: vk::MemoryPropertyFlags::DEVICE_LOCAL,
            view: ImageViewInfo {
                aspect: vk::ImageAspectFlags::COLOR,
                ..Default::default()
            },
            layout: vk::ImageLayout::UNDEFINED,
            sampler: Some(cvk::SamplerInfo::default()),
            label: Some("Debug Present Texture"),
            ..Default::default()
        });

        self.descriptor_set.write(&[
            cvk::DescriptorWrite::StorageImage {
                binding: 0,
                image_view: new_texture.view,
                image_layout: vk::ImageLayout::GENERAL,
                array_element: None,
            },
            cvk::DescriptorWrite::SampledImage {
                binding: 2,
                image_view: new_texture.view,
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                array_element: None,
            },
            cvk::DescriptorWrite::Sampler {
                binding: 3,
                sampler: new_texture.sampler(),
                array_element: None,
            },
        ]);

        self.present_texture = new_texture;

        Ok(())
    }
}

#[allow(unused)]
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
        let instance = cvk::Instance::new("CVK", "CVK")?;

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
            None,
        )
        .unwrap();
        self.window = Some(window);
        self.render = Some(render);
    }

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: winit::event::StartCause) {
        if let Some(render) = &mut self.render {
            match cause {
                winit::event::StartCause::ResumeTimeReached {
                    requested_resume, ..
                } => {
                    render.pc.dt = requested_resume.elapsed().as_secs_f32();
                }
                winit::event::StartCause::Poll => {
                    render.pc.dt = 1.0 / 60.0;
                }
                _ => {}
            }
        }
    }

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        if let Some(render) = &mut self.render {
            render.egui.handle_device_events(&event);
        }

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
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        if let Some(render) = &mut self.render {
            let _consoom = render.egui.handle_window_events(&render.window, &event);
        }

        match event {
            WindowEvent::RedrawRequested => {
                if let Some(render) = &mut self.render {
                    if let Some(max) = render.max_frame {
                        if max <= render.frame_count {
                            event_loop.exit();
                            return;
                        }
                    }
                    render.render();
                    render.frame_count += 1;
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(render) = &mut self.render {
                    let height = render.window.inner_size().height as f32;
                    render.pc.mouse = [position.x as f32, height - position.y as f32];
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                log::debug!("WindowEvent: Scale Factor Change: {:?}", scale_factor);
                if let Some(render) = &mut self.render {
                    render.egui.ctx.set_pixels_per_point(scale_factor as f32);
                }
            }
            _ => {}
        }
    }
}

pub fn main() {
    env_logger::builder()
        .filter_module("naga", log::LevelFilter::Warn)
        .filter_level(log::LevelFilter::Debug)
        .init();

    let event_loop = EventLoop::builder().build().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = VulkanApp::new(&event_loop).unwrap();
    event_loop.run_app(&mut app).unwrap();
}
