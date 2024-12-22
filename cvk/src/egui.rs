use ash::vk;
use bytemuck::offset_of;
use std::{collections::HashMap, sync::Arc};

use crate::{
    Buffer, CommandRecorder, DescriptorBinding, DescriptorSet, DescriptorSetLayout, DescriptorType,
    DescriptorWrite, Device, Frame, ImageTransition, Queue, RenderPipeline, SamplerInfo, Texture,
};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct EguiPushConstants {
    viewport_size: [f32; 2],
    texture_layer: u32,
    scale_factor: f32,
}

impl EguiPushConstants {
    fn new(viewport_size: [f32; 2], texture_layer: u32, scale_factor: f32) -> Self {
        Self {
            viewport_size,
            texture_layer,
            scale_factor,
        }
    }
}

pub struct FrameResources {
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
    pub staging: Vec<Arc<Buffer>>,
}

pub struct EguiIntegration {
    pub ctx: egui::Context,
    pub state: egui_winit::State,
    pub device: Arc<Device>,
    pub pipeline: RenderPipeline,
    pub frame_resources: Vec<FrameResources>,
    pub textures: EguiTextureArray,
    pub viewport: [u32; 2],
}

pub struct EguiTextureArray {
    pub texture: Texture,
    pub descriptor: DescriptorSet,
    pub descriptor_layout: DescriptorSetLayout,
    pub allocation_info: HashMap<egui::TextureId, TextureAllocation>,
    pub next_layer: u32,
    pub size: [u32; 2],
}

#[derive(Clone, Copy)]
pub struct TextureAllocation {
    pub layer: u32,
    pub size: [u32; 2],
}

impl EguiTextureArray {
    fn new(
        device: &Device,
        initial_size: [u32; 2],
        max_layers: u32,
        format: vk::Format,
        queue: &Queue,
    ) -> Self {
        let descriptor_layout =
            device.create_descriptor_set_layout(&crate::DescriptorSetLayoutInfo {
                bindings: &[
                    DescriptorBinding {
                        binding: 0,
                        ty: DescriptorType::SampledImage,
                        count: 1,
                        stages: vk::ShaderStageFlags::FRAGMENT,
                        flags: None,
                    },
                    DescriptorBinding {
                        binding: 1,
                        ty: DescriptorType::Sampler,
                        count: 1,
                        stages: vk::ShaderStageFlags::FRAGMENT,
                        flags: None,
                    },
                ],
                ..Default::default()
            });

        let texture = device.create_texture(&crate::TextureInfo {
            format,
            width: initial_size[0],
            height: initial_size[1],
            layers: max_layers,
            usage: vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
            sharing: vk::SharingMode::EXCLUSIVE,
            usage_locality: vkm::MemoryUsage::AutoPreferDevice,
            allocation_locality: vk::MemoryPropertyFlags::DEVICE_LOCAL,
            aspect_mask: vk::ImageAspectFlags::COLOR,
            layout: vk::ImageLayout::UNDEFINED,
            view_type: vk::ImageViewType::TYPE_2D_ARRAY,
            sampler: Some(SamplerInfo {
                min: vk::Filter::LINEAR,
                mag: vk::Filter::LINEAR,
                mipmap: vk::SamplerMipmapMode::NEAREST,
                address_u: vk::SamplerAddressMode::CLAMP_TO_EDGE,
                address_v: vk::SamplerAddressMode::CLAMP_TO_EDGE,
                address_w: vk::SamplerAddressMode::CLAMP_TO_EDGE,
                min_lod: 0.0,
                max_lod: 0.0,
                ..Default::default()
            }),
            label: Some("Egui Texture"),
            ..Default::default()
        });

        let mut recorder = queue.record();
        recorder.image_transition(
            &texture,
            ImageTransition::Custom {
                old_layout: vk::ImageLayout::UNDEFINED,
                new_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                src_stage: vk::PipelineStageFlags::TOP_OF_PIPE,
                dst_stage: vk::PipelineStageFlags::FRAGMENT_SHADER,
                src_access: vk::AccessFlags::empty(),
                dst_access: vk::AccessFlags::SHADER_WRITE,
            },
        );
        let cmd = recorder.finish();
        let _ = queue.submit(&[cmd], &[], &[], &[], &[]);

        let descriptor_pool = device.create_descriptor_pool(&crate::DescriptorPoolInfo {
            max_sets: 1,
            layouts: &[&descriptor_layout],
            ..Default::default()
        });

        let descriptor = device.create_descriptor_set(descriptor_pool, &descriptor_layout);

        descriptor.write(&[
            DescriptorWrite::SampledImage {
                binding: 0,
                image_view: texture.view,
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            },
            DescriptorWrite::Sampler {
                binding: 1,
                sampler: texture.sampler.unwrap(),
            },
        ]);

        Self {
            texture,
            descriptor,
            descriptor_layout,
            allocation_info: HashMap::new(),
            next_layer: 0,
            size: initial_size,
        }
    }

    pub fn allocate_texture(
        &mut self,
        id: egui::TextureId,
        size: [u32; 2],
    ) -> Option<TextureAllocation> {
        if self.next_layer >= self.texture.details.layers {
            return None; // No more layers available
        }

        let allocation = TextureAllocation {
            layer: self.next_layer,
            size,
        };

        self.allocation_info.insert(id, allocation);
        self.next_layer += 1;

        Some(allocation)
    }

    pub fn update_texture(
        &mut self,
        device: &Device,
        id: egui::TextureId,
        image_delta: &egui::epaint::ImageDelta,
        recorder: &mut CommandRecorder,
        frame_resource: &mut FrameResources,
    ) {
        let size = [
            image_delta.image.width() as u32,
            image_delta.image.height() as u32,
        ];
        log::debug!(
            "Updating texture {:?}, size: {:?}, is_font: {}",
            id,
            size,
            matches!(image_delta.image, egui::ImageData::Font(_))
        );
        let allocation = if !self.allocation_info.contains_key(&id) {
            match self.allocate_texture(id, size) {
                Some(alloc) => alloc,
                None => {
                    log::error!("Failed to allocate texture for {:?}", id);
                    return;
                }
            }
        } else {
            self.allocation_info[&id]
        };

        let pixels: Vec<u8> = match &image_delta.image {
            egui::ImageData::Color(color_image) => color_image
                .pixels
                .iter()
                .flat_map(|color| {
                    let srgba = color.to_array();
                    srgba
                })
                .collect(),
            egui::ImageData::Font(font_image) => font_image
                .srgba_pixels(None)
                .flat_map(|color| {
                    let srgba = color.to_array();
                    srgba
                })
                .collect(),
        };

        let staging_buffer = device.create_buffer(&crate::BufferInfo {
            size: pixels.len() as u64,
            usage: vk::BufferUsageFlags::TRANSFER_SRC,
            sharing: vk::SharingMode::EXCLUSIVE,
            usage_locality: vkm::MemoryUsage::AutoPreferHost,
            allocation_locality: vk::MemoryPropertyFlags::HOST_VISIBLE
                | vk::MemoryPropertyFlags::HOST_COHERENT,
            host_access: Some(vkm::AllocationCreateFlags::HOST_ACCESS_SEQUENTIAL_WRITE),
            label: Some("Egui Texture Upload Buffer"),
            ..Default::default()
        });

        staging_buffer.upload(&pixels, 0);

        // Transition image layout for transfer
        recorder.image_transition(
            &self.texture,
            ImageTransition::Custom {
                old_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                src_stage: vk::PipelineStageFlags::FRAGMENT_SHADER,
                dst_stage: vk::PipelineStageFlags::TRANSFER,
                src_access: vk::AccessFlags::SHADER_READ,
                dst_access: vk::AccessFlags::TRANSFER_WRITE,
            },
        );

        let mut region = vk::BufferImageCopy::default()
            .image_subresource(vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                mip_level: 0,
                base_array_layer: allocation.layer,
                layer_count: 1,
            })
            .image_extent(vk::Extent3D {
                width: size[0],
                height: size[1],
                depth: 1,
            });

        if let Some(pos) = image_delta.pos {
            region = region.image_offset(vk::Offset3D {
                x: pos[0] as i32,
                y: pos[1] as i32,
                z: 0,
            });
        }

        unsafe {
            device.handle.cmd_copy_buffer_to_image(
                recorder.buffer,
                staging_buffer.handle,
                self.texture.image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[region],
            );
        }

        frame_resource.staging.push(Arc::new(staging_buffer));

        recorder.image_transition(
            &self.texture,
            ImageTransition::Custom {
                old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                new_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                src_stage: vk::PipelineStageFlags::TRANSFER,
                dst_stage: vk::PipelineStageFlags::FRAGMENT_SHADER,
                src_access: vk::AccessFlags::TRANSFER_WRITE,
                dst_access: vk::AccessFlags::SHADER_READ,
            },
        );
    }
}

impl EguiIntegration {
    pub fn new(
        device: Arc<Device>,
        window: &winit::window::Window,
        format: vk::Format,
        frames: u32,
        queue: &Queue,
    ) -> Self {
        let ctx = egui::Context::default();
        let size = window.inner_size();
        let scale_factor = window.scale_factor();
        let viewport = [size.width, size.height];

        // Set up initial context
        ctx.set_pixels_per_point(scale_factor as f32);
        let fonts = egui::FontDefinitions::default();
        ctx.set_fonts(fonts);

        // Set up style
        let style = (*ctx.style()).clone();
        ctx.set_style(style);

        // Create initial state
        let mut state = egui_winit::State::new(
            ctx.clone(),
            egui::ViewportId::default(),
            window,
            Some(scale_factor as f32),
            None,
            None,
        );

        // Run a dummy frame to force texture creation
        let raw_input = state.take_egui_input(window);
        ctx.begin_pass(raw_input);
        let _ = ctx.run(egui::RawInput::default(), |_| {});
        let full_output = ctx.end_pass();

        // Now create the texture array with knowledge of required textures
        let textures = EguiTextureArray::new(&device, [2048, 2048], 32, format, queue);
        let frame_resources = (0..frames)
            .map(|_| Self::create_frame_resources(&device))
            .collect();

        let shader = device.create_shader(EGUI_SHADER).unwrap();

        let vertex_binding_descs = [vk::VertexInputBindingDescription::default()
            .binding(0)
            .stride(std::mem::size_of::<egui::epaint::Vertex>() as u32)
            .input_rate(vk::VertexInputRate::VERTEX)];

        let vertex_attribute_descs = [
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(0)
                .format(vk::Format::R32G32_SFLOAT)
                .offset(offset_of!(egui::epaint::Vertex, pos) as u32),
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(1)
                .format(vk::Format::R32G32_SFLOAT)
                .offset(offset_of!(egui::epaint::Vertex, uv) as u32),
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(2)
                .format(vk::Format::R8G8B8A8_UNORM)
                .offset(offset_of!(egui::epaint::Vertex, color) as u32),
        ];

        let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(&vertex_binding_descs)
            .vertex_attribute_descriptions(&vertex_attribute_descs);

        let pipeline = device.create_render_pipeline(&crate::RenderPipelineInfo {
            vertex_shader: shader.entry("vmain"),
            fragment_shader: shader.entry("fmain"),
            color_formats: &[format],
            depth_format: None,
            descriptor_layouts: &[&textures.descriptor_layout],
            push_constant_size: Some(std::mem::size_of::<EguiPushConstants>() as u32),
            blend_states: Some(&[vk::PipelineColorBlendAttachmentState {
                blend_enable: vk::TRUE,
                src_color_blend_factor: vk::BlendFactor::ONE,
                dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
                color_blend_op: vk::BlendOp::ADD,
                src_alpha_blend_factor: vk::BlendFactor::ONE,
                dst_alpha_blend_factor: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
                alpha_blend_op: vk::BlendOp::ADD,
                color_write_mask: vk::ColorComponentFlags::RGBA,
            }]),
            vertex_input_state: Some(vertex_input),
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            polygon: vk::PolygonMode::FILL,
            cull: vk::CullModeFlags::NONE,
            front_face: vk::FrontFace::COUNTER_CLOCKWISE,
            label: Some("Egui Pipeline"),
            tag: None,
        });

        Self {
            ctx,
            state,
            device,
            pipeline,
            frame_resources,
            textures,
            viewport,
        }
    }

    pub fn update_textures(
        &mut self,
        recorder: &mut CommandRecorder,
        textures_delta: &egui::TexturesDelta,
        frame: Frame,
    ) {
        let frame_resource = &mut self.frame_resources[frame.index as usize];

        for (id, image_delta) in &textures_delta.set {
            self.textures
                .update_texture(&self.device, *id, image_delta, recorder, frame_resource);
        }

        for &id in &textures_delta.free {
            if let Some(_) = self.textures.allocation_info.remove(&id) {
                log::trace!("Freed egui texture {:?}", id);
            }
        }
    }

    pub fn render(
        &mut self,
        recorder: &mut CommandRecorder,
        shapes: Vec<egui::epaint::ClippedShape>,
        frame: Frame,
    ) {
        let frame_resources = &self.frame_resources[frame.index as usize];
        let scale = self.ctx.pixels_per_point();

        let primitives = self.ctx.tessellate(shapes, scale);

        let mut vertex_buffer_writer = Vec::<u8>::new();
        let mut index_buffer_writer = Vec::<u8>::new();

        for primitive in &primitives {
            if let egui::epaint::Primitive::Mesh(mesh) = &primitive.primitive {
                vertex_buffer_writer.extend_from_slice(bytemuck::cast_slice(&mesh.vertices));
                index_buffer_writer.extend_from_slice(bytemuck::cast_slice(&mesh.indices));
            }
        }

        if vertex_buffer_writer.is_empty() || index_buffer_writer.is_empty() {
            return;
        }

        frame_resources
            .vertex_buffer
            .upload(&vertex_buffer_writer, 0);
        frame_resources.index_buffer.upload(&index_buffer_writer, 0);

        let color_attachment = vk::RenderingAttachmentInfo::default()
            .image_view(frame.view)
            .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::LOAD)
            .store_op(vk::AttachmentStoreOp::STORE);

        recorder.begin_rendering(
            &[color_attachment],
            vk::Extent2D {
                width: self.viewport[0],
                height: self.viewport[1],
            },
        );

        recorder.bind_pipeline(&self.pipeline);
        recorder.viewport(vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: self.viewport[0] as f32,
            height: self.viewport[1] as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        });

        recorder.bind_descriptor_set(&self.textures.descriptor, 0, &[]);
        recorder.bind_vertex(&frame_resources.vertex_buffer, 0);
        recorder.bind_index(&frame_resources.index_buffer, vk::IndexType::UINT32);

        let mut vertex_offset = 0;
        let mut index_offset = 0;

        for primitive in primitives {
            match &primitive.primitive {
                egui::epaint::Primitive::Mesh(mesh) => {
                    let clip_min_x = primitive.clip_rect.min.x * scale;
                    let clip_min_y = primitive.clip_rect.min.y * scale;
                    let clip_max_x = primitive.clip_rect.max.x * scale;
                    let clip_max_y = primitive.clip_rect.max.y * scale;

                    // Ensure we stay within the physical viewport bounds
                    let x = clip_min_x.max(0.0).round() as i32;
                    let y = clip_min_y.max(0.0).round() as i32;
                    let width = (clip_max_x - clip_min_x)
                        .min(self.viewport[0] as f32)
                        .round() as u32;
                    let height = (clip_max_y - clip_min_y)
                        .min(self.viewport[1] as f32)
                        .round() as u32;

                    let scissor = vk::Rect2D {
                        offset: vk::Offset2D { x, y },
                        extent: vk::Extent2D { width, height },
                    };

                    recorder.scissor(scissor);

                    let layer = self
                        .textures
                        .allocation_info
                        .get(&mesh.texture_id)
                        .map(|alloc| alloc.layer)
                        .unwrap_or(0);

                    let pc = EguiPushConstants::new(self.viewport.map(|v| v as f32), layer, scale);
                    recorder.push_constants(pc);

                    let index_count = mesh.indices.len() as u32;
                    recorder.draw_indexed(
                        index_offset..index_offset + index_count,
                        0..1,
                        vertex_offset as i32,
                    );
                    vertex_offset += mesh.vertices.len() as u32;
                    index_offset += index_count;
                }
                egui::epaint::Primitive::Callback(_callback) => {
                    log::warn!("Custom paint callbacks are not yet implemented");
                }
            };
        }
        recorder.end_rendering();
    }

    pub fn handle_window_events(
        &mut self,
        window: &winit::window::Window,
        event: &winit::event::WindowEvent,
    ) -> bool {
        self.state.on_window_event(window, event).consumed
    }

    pub fn handle_device_events(&mut self, event: &winit::event::DeviceEvent) {
        if let winit::event::DeviceEvent::MouseMotion { delta } = event {
            self.state.on_mouse_motion(*delta);
        }
    }

    pub fn begin_frame(&mut self, window: &winit::window::Window, frame: Frame) {
        self.frame_resources[frame.index as usize].staging.clear();
        let raw_input = self.state.take_egui_input(window);
        self.ctx.begin_pass(raw_input);
    }

    pub fn end_frame(&mut self, window: &winit::window::Window) -> egui::FullOutput {
        let output = self.ctx.end_pass();
        self.state
            .handle_platform_output(window, output.platform_output.clone());
        output
    }

    fn create_frame_resources(device: &Arc<crate::Device>) -> FrameResources {
        let vertex_buffer = device.create_buffer(&crate::BufferInfo {
            size: 2048 * 2048,
            usage: vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
            sharing: vk::SharingMode::EXCLUSIVE,
            usage_locality: vkm::MemoryUsage::AutoPreferDevice,
            allocation_locality: vk::MemoryPropertyFlags::HOST_VISIBLE
                | vk::MemoryPropertyFlags::HOST_COHERENT
                | vk::MemoryPropertyFlags::DEVICE_LOCAL,
            host_access: Some(vkm::AllocationCreateFlags::HOST_ACCESS_SEQUENTIAL_WRITE),
            label: Some("EGUI Vertex Buffer"),
            ..Default::default()
        });

        let index_buffer = device.create_buffer(&crate::BufferInfo {
            size: 512 * 1024,
            usage: vk::BufferUsageFlags::INDEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
            sharing: vk::SharingMode::EXCLUSIVE,
            usage_locality: vkm::MemoryUsage::AutoPreferDevice,
            allocation_locality: vk::MemoryPropertyFlags::HOST_VISIBLE
                | vk::MemoryPropertyFlags::HOST_COHERENT
                | vk::MemoryPropertyFlags::DEVICE_LOCAL,
            host_access: Some(vkm::AllocationCreateFlags::HOST_ACCESS_SEQUENTIAL_WRITE),
            label: Some("EGUI Index Buffer"),
            ..Default::default()
        });

        FrameResources {
            vertex_buffer,
            index_buffer,
            staging: Vec::new(),
        }
    }
}

const EGUI_SHADER: &str = r#"
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) @interpolate(flat) layer: u32,
}

struct PushConstants {
    viewport_size: vec2<f32>,
    texture_layer: u32,
    scale_factor: f32,
}

var<push_constant> pc: PushConstants;

@group(0) @binding(0) 
var t_diffuse: texture_2d_array<f32>;
@group(0) @binding(1) 
var s_diffuse: sampler;

@vertex
fn vmain(
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>
) -> VertexOutput {
    var output: VertexOutput;
    
    let scaled_pos = pos * pc.scale_factor;
    let clip_pos = vec2<f32>(
        2.0 * scaled_pos.x / pc.viewport_size.x - 1.0,
        2.0 * scaled_pos.y / pc.viewport_size.y - 1.0
    );
    
    output.position = vec4<f32>(clip_pos, 0.0, 1.0);
    output.uv = uv;
    output.color = color;
    output.layer = pc.texture_layer;
    return output;
}

@fragment
fn fmain(
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) @interpolate(flat) layer: u32,
) -> @location(0) vec4<f32> {
    let texture_color = textureSample(t_diffuse, s_diffuse, uv, layer);
    

    return color * texture_color.a;
}
"#;
