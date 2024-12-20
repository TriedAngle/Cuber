extern crate nalgebra as na;
extern crate vk_mem as vkm;
use std::{ops::Deref, sync::Arc};

use anyhow::Result;
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
    let coords = vec2<u32>(global_id.xy);
    
    // Create a checkerboard pattern - this will make it very obvious if it works
    let checker_size = 32u;
    let is_white = ((coords.x / checker_size) + (coords.y / checker_size)) % 2u == 0u;
    
    let color = select(
        vec4<f32>(1.0, 0.0, 0.0, 1.0),  // red
        vec4<f32>(1.0, 1.0, 1.0, 1.0),  // white
        is_white
    );
    
    textureStore(output_texture, coords, color);
}
"#;

const PRSENT_SHADER: &str = r#"
struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vmain(@builtin(vertex_index) vert_idx: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 3>(
    vec2<f32>( 0.0, -0.5),   // top center (changed from 0.5 to -0.5)
    vec2<f32>(-0.5,  0.5),   // bottom left (changed from -0.5 to 0.5)
    vec2<f32>( 0.5,  0.5)    // bottom right (changed from -0.5 to 0.5)
    );
    
    var colors = array<vec4<f32>, 3>(
        vec4<f32>(1.0, 0.0, 0.0, 1.0),  // red
        vec4<f32>(0.0, 1.0, 0.0, 1.0),  // green
        vec4<f32>(0.0, 0.0, 1.0, 1.0)   // blue
    );

    var output: VertexOutput;
    output.pos = vec4<f32>(positions[vert_idx], 0.0, 1.0);
    output.color = colors[vert_idx];
    return output;
}

@fragment
fn fmain(@location(0) color: vec4<f32>) -> @location(0) vec4<f32> {
    return color;
}
"#;

struct Render {
    surface: Arc<cvk::Surface>,
    window: Arc<Window>,
    swapchain: cvk::Swapchain,
    compute_queue: Arc<cvk::Queue>,
    present_queue: Arc<cvk::Queue>,
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
}

impl Render {
    pub fn new(
        instance: Arc<cvk::Instance>,
        device: Arc<cvk::Device>,
        compute_queue: Arc<cvk::Queue>,
        present_queue: Arc<cvk::Queue>,
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

        assert!(surface.is_compatible(&device.adapter(), &present_queue));

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
            sampler: Some(cvk::SamplerInfo {
                mag: vk::Filter::NEAREST,
                min: vk::Filter::NEAREST,
                mipmap: vk::SamplerMipmapMode::NEAREST,
                address_u: vk::SamplerAddressMode::CLAMP_TO_EDGE,
                address_v: vk::SamplerAddressMode::CLAMP_TO_EDGE,
                address_w: vk::SamplerAddressMode::CLAMP_TO_EDGE,
                label: Some("Debug Sampler"),
                ..Default::default()
            }),
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
            push_constant_size: None,
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

        let new = Self {
            instance,
            surface,
            window,
            device,
            compute_queue,
            present_queue,
            transfer_queue,
            swapchain,

            compute_pipeline,
            present_pipeline,
            descriptor_layout: layout,
            descriptor_pool: pool,
            descriptor_set,
            present_texture,
            compute_complete,
        };

        Ok(new)
    }

    fn render(&mut self) {
        let (frame, signals, _suboptimal) = self.swapchain.acquire_next_frame(None);
        println!("Acquired frame {}", frame.index);

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
                    float32: [0.1, 0.2, 0.6, 1.0],
                },
            });

        recorder.begin_rendering(&[color_attachment], self.swapchain.extent);

        unsafe {
            self.device.handle.cmd_bind_pipeline(
                recorder.buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.present_pipeline.handle,
            );

            let viewport = vk::Viewport {
                x: 0.0,
                y: 0.0,
                width: self.swapchain.extent.width as f32,
                height: self.swapchain.extent.height as f32,
                min_depth: 0.0,
                max_depth: 1.0,
            };

            let scissor = vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: self.swapchain.extent,
            };

            self.device
                .handle
                .cmd_set_viewport(recorder.buffer, 0, &[viewport]);
            self.device
                .handle
                .cmd_set_scissor(recorder.buffer, 0, &[scissor]);

            self.device.handle.cmd_draw(recorder.buffer, 3, 1, 0, 0);
        }

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

        let command_buffer = recorder.finish();

        self.present_queue
            .submit(
                &[command_buffer],
                &[(
                    signals.available,
                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                )],
                &[],
                &[signals.finished],
                &[],
            )
            .unwrap();

        self.swapchain.present_frame(&self.present_queue, frame);
        self.window.request_redraw();
        // let (frame, signals, _suboptimal) = self.swapchain.acquire_next_frame(None);

        // let mut compute_recorder = self.compute_queue.record();

        // compute_recorder.image_barrier(
        //     self.present_texture.image,
        //     vk::ImageLayout::UNDEFINED,
        //     vk::ImageLayout::GENERAL,
        //     vk::PipelineStageFlags::TOP_OF_PIPE,
        //     vk::PipelineStageFlags::COMPUTE_SHADER,
        //     vk::AccessFlags::empty(),
        //     vk::AccessFlags::SHADER_WRITE,
        // );
        // unsafe {
        //     self.device.handle.cmd_bind_pipeline(
        //         compute_recorder.buffer,
        //         vk::PipelineBindPoint::COMPUTE,
        //         self.compute_pipeline.handle,
        //     );

        //     self.device.handle.cmd_bind_descriptor_sets(
        //         compute_recorder.buffer,
        //         vk::PipelineBindPoint::COMPUTE,
        //         self.compute_pipeline.layout,
        //         0,
        //         &[self.descriptor_set.handle],
        //         &[],
        //     );

        //     self.device.handle.cmd_dispatch(
        //         compute_recorder.buffer,
        //         (self.swapchain.extent.width + 15) / 16,
        //         (self.swapchain.extent.height + 15) / 16,
        //         1,
        //     );
        // }

        // compute_recorder.image_barrier(
        //     self.present_texture.image,
        //     vk::ImageLayout::GENERAL,
        //     vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        //     vk::PipelineStageFlags::COMPUTE_SHADER,
        //     vk::PipelineStageFlags::FRAGMENT_SHADER,
        //     vk::AccessFlags::SHADER_WRITE,
        //     vk::AccessFlags::SHADER_READ,
        // );

        // let compute_buffer = compute_recorder.finish();

        // let mut present_recorder = self.present_queue.record();

        // present_recorder.image_barrier(
        //     frame.image,
        //     vk::ImageLayout::UNDEFINED,
        //     vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        //     vk::PipelineStageFlags::TOP_OF_PIPE,
        //     vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
        //     vk::AccessFlags::empty(),
        //     vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
        // );

        // let color_attachment = vk::RenderingAttachmentInfo::default()
        //     .image_view(frame.view)
        //     .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
        //     .load_op(vk::AttachmentLoadOp::CLEAR)
        //     .store_op(vk::AttachmentStoreOp::STORE)
        //     .clear_value(vk::ClearValue {
        //         color: vk::ClearColorValue {
        //             float32: [0.0, 0.0, 0.0, 1.0],
        //         },
        //     });

        // present_recorder.begin_rendering(&[color_attachment], self.swapchain.extent);

        // unsafe {
        //     self.device.handle.cmd_bind_pipeline(
        //         present_recorder.buffer,
        //         vk::PipelineBindPoint::GRAPHICS,
        //         self.present_pipeline.handle,
        //     );

        //     self.device.handle.cmd_bind_descriptor_sets(
        //         present_recorder.buffer,
        //         vk::PipelineBindPoint::GRAPHICS,
        //         self.present_pipeline.layout,
        //         0,
        //         &[self.descriptor_set.handle],
        //         &[],
        //     );

        //     let viewport = vk::Viewport {
        //         x: 0.0,
        //         y: 0.0,
        //         width: self.swapchain.extent.width as f32,
        //         height: self.swapchain.extent.height as f32,
        //         min_depth: 0.0,
        //         max_depth: 1.0,
        //     };

        //     let scissor = vk::Rect2D {
        //         offset: vk::Offset2D { x: 0, y: 0 },
        //         extent: self.swapchain.extent,
        //     };

        //     self.device
        //         .handle
        //         .cmd_set_viewport(present_recorder.buffer, 0, &[viewport]);
        //     self.device
        //         .handle
        //         .cmd_set_scissor(present_recorder.buffer, 0, &[scissor]);
        //     self.device
        //         .handle
        //         .cmd_draw(present_recorder.buffer, 4, 1, 0, 0);
        // }

        // present_recorder.end_rendering();

        // present_recorder.image_barrier(
        //     frame.image,
        //     vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        //     vk::ImageLayout::PRESENT_SRC_KHR,
        //     vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
        //     vk::PipelineStageFlags::BOTTOM_OF_PIPE,
        //     vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
        //     vk::AccessFlags::empty(),
        // );

        // let present_buffer = present_recorder.finish();

        // self.compute_queue
        //     .submit(
        //         &[compute_buffer],
        //         &[(signals.available, vk::PipelineStageFlags::TOP_OF_PIPE)],
        //         &[],
        //         &[self.compute_complete.handle],
        //         &[],
        //     )
        //     .unwrap();

        // self.present_queue
        //     .submit(
        //         &[present_buffer],
        //         &[(
        //             self.compute_complete.handle,
        //             vk::PipelineStageFlags::FRAGMENT_SHADER
        //                 | vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
        //         )],
        //         &[],
        //         &[signals.finished],
        //         &[],
        //     )
        //     .unwrap();

        // self.swapchain.present_frame(&self.present_queue, frame);
    }
}

struct VulkanApp {
    camera: Camera,
    instance: Arc<cvk::Instance>,
    device: Arc<cvk::Device>,
    compute_queue: Arc<cvk::Queue>,
    present_queue: Arc<cvk::Queue>,
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
        println!("adapters: {:?}", adapters.len());
        let adapter = adapters[1].clone();
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

        Ok(Self {
            camera,
            instance,
            device,
            compute_queue,
            present_queue,
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
            self.present_queue.clone(),
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
