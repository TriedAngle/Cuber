use ash::vk;
use bytemuck::offset_of;
use std::{collections::HashMap, sync::Arc};
use winit::dpi::PhysicalSize;

use crate::{
    Buffer, CommandRecorder, DescriptorBinding, DescriptorSet, DescriptorSetLayout, DescriptorType,
    DescriptorWrite, Device, Frame, Image, ImageInfo, ImageTransition, ImageViewInfo, Queue,
    RenderPipeline, Sampler, SamplerInfo,
};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct EguiPushConstants {
    size: [f32; 2],
    texture: u32,
    _padding: u32,
}

pub struct FrameResources {
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
}

pub struct EguiTextures {
    pub textures: HashMap<egui::TextureId, (Image, u32)>,
    pub descriptor: DescriptorSet,
    pub descriptor_layout: DescriptorSetLayout,
    pub sampler: Sampler,
    pub next_binding: u32,
    pub max_textures: u32,
    device: Arc<Device>,
}

#[derive(Clone, Copy)]
pub struct TextureAllocation {
    pub layer: u32,
    pub size: [u32; 2],
}

impl EguiTextures {
    fn new(device: Arc<Device>, max_textures: u32) -> Self {
        let descriptor_layout =
            device.create_descriptor_set_layout(&crate::DescriptorSetLayoutInfo {
                bindings: &[
                    DescriptorBinding {
                        binding: 0,
                        ty: DescriptorType::SampledImage,
                        count: max_textures,
                        stages: vk::ShaderStageFlags::FRAGMENT,
                        flags: Some(vk::DescriptorBindingFlags::PARTIALLY_BOUND),
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

        let sampler = device.create_sampler(&SamplerInfo {
            min: vk::Filter::LINEAR,
            mag: vk::Filter::LINEAR,
            mipmap: vk::SamplerMipmapMode::LINEAR,
            address_u: vk::SamplerAddressMode::CLAMP_TO_EDGE,
            address_v: vk::SamplerAddressMode::CLAMP_TO_EDGE,
            address_w: vk::SamplerAddressMode::CLAMP_TO_EDGE,
            anisotropy: None,
            min_lod: 0.0,
            max_lod: vk::LOD_CLAMP_NONE,
            ..Default::default()
        });

        let descriptor_pool = device.create_descriptor_pool(&crate::DescriptorPoolInfo {
            max_sets: 1,
            layouts: &[&descriptor_layout],
            ..Default::default()
        });

        let descriptor = device.create_descriptor_set(descriptor_pool, &descriptor_layout);

        descriptor.write(&[DescriptorWrite::Sampler {
            binding: 1,
            sampler: &sampler,
            array_element: None,
        }]);

        Self {
            textures: HashMap::new(),
            descriptor,
            descriptor_layout,
            sampler,
            device,
            next_binding: 0,
            max_textures,
        }
    }

    fn create_texture(&self, size: [u32; 2]) -> Image {
        self.device.create_image(&ImageInfo {
            format: vk::Format::R8G8B8A8_UNORM,
            width: size[0],
            height: size[1],
            layers: 1,
            usage: vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
            sharing: vk::SharingMode::EXCLUSIVE,
            usage_locality: vkm::MemoryUsage::AutoPreferDevice,
            allocation_locality: vk::MemoryPropertyFlags::DEVICE_LOCAL,
            view: ImageViewInfo {
                aspect: vk::ImageAspectFlags::COLOR,
                ..Default::default()
            },
            layout: vk::ImageLayout::UNDEFINED,
            sampler: None,
            label: Some("Egui Texture"),
            ..Default::default()
        })
    }

    pub fn update_texture(
        &mut self,
        id: egui::TextureId,
        image_delta: &egui::epaint::ImageDelta,
        recorder: &mut CommandRecorder,
    ) -> Option<Buffer> {
        let size = [
            image_delta.image.width() as u32,
            image_delta.image.height() as u32,
        ];

        if self
            .textures
            .get(&id)
            .map(|(texture, _)| [texture.details().width, texture.details().height])
            != Some(size)
        {
            if !self.textures.contains_key(&id) {
                if self.next_binding >= self.max_textures {
                    log::error!("Maximum number of textures reached");
                    return None;
                }
                self.next_binding += 1;
            }
            let binding_index = self
                .textures
                .get(&id)
                .map(|(_, idx)| *idx)
                .unwrap_or(self.next_binding - 1);
            let texture = self.create_texture(size);
            self.textures.insert(id, (texture, binding_index));
        }

        let (texture, binding_index) = self.textures.get(&id).unwrap();

        let pixels: Vec<u8> = match &image_delta.image {
            egui::ImageData::Color(color_image) => color_image
                .pixels
                .iter()
                .flat_map(|color| color.to_array())
                .collect(),
            egui::ImageData::Font(font_image) => font_image
                .srgba_pixels(None)
                .flat_map(|color| color.to_array())
                .collect(),
        };

        let staging_buffer = self.device.create_buffer(&crate::BufferInfo {
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

        recorder.image_transition(texture, ImageTransition::TransferDst);

        let mut region = vk::BufferImageCopy::default()
            .buffer_offset(0)
            .buffer_image_height(0)
            .image_subresource(vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                mip_level: 0,
                base_array_layer: 0,
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

        recorder.copy_buffer_image(
            &staging_buffer,
            &texture,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &[region],
        );

        recorder.image_transition(texture, ImageTransition::FragmentRead);

        self.descriptor.write(&[DescriptorWrite::SampledImage {
            binding: 0,
            image_view: texture.view,
            image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            array_element: Some(*binding_index),
        }]);

        Some(staging_buffer)
    }

    pub fn get_binding_index(&self, id: &egui::TextureId) -> Option<u32> {
        self.textures
            .get(id)
            .map(|(_, binding_index)| *binding_index)
    }

    pub fn free_texture(&mut self, id: egui::TextureId) {
        self.textures.remove(&id);
    }
}

pub struct EguiState {
    pub ctx: egui::Context,
    pub state: egui_winit::State,
    pub device: Arc<Device>,
    pub pipeline: RenderPipeline,
    pub frame_resources: Vec<FrameResources>,
    pub textures: EguiTextures,
    pub size: PhysicalSize<u32>,
    pub scale_factor: f64,
    pub staging_buffers: Vec<(Buffer, u64)>,
}

impl EguiState {
    pub fn new(
        device: Arc<Device>,
        window: &winit::window::Window,
        format: vk::Format,
        frames: u32,
        scale_factor: f64,
        fonts: egui::FontDefinitions,
        style: egui::Style,
    ) -> Self {
        let ctx = egui::Context::default();
        let size = window.inner_size();
        ctx.set_fonts(fonts);
        ctx.set_style(style);

        let state = egui_winit::State::new(
            ctx.clone(),
            egui::ViewportId::default(),
            window,
            Some(scale_factor as f32),
            None,
            None,
        );
        ctx.set_visuals(egui::Visuals::default());

        let textures = EguiTextures::new(device.clone(), 512);

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
            blend_states: Some(&[vk::PipelineColorBlendAttachmentState::default()
                .blend_enable(true)
                .src_color_blend_factor(vk::BlendFactor::ONE)
                .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
                .color_write_mask(vk::ColorComponentFlags::RGBA)]),
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
            scale_factor,
            size,
            staging_buffers: Vec::new(),
        }
    }

    pub fn update_textures(
        &mut self,
        recorder: &mut CommandRecorder,
        textures_delta: &egui::TexturesDelta,
        queue: &Queue,
    ) {
        let current_index = queue.current_index();

        for (id, image_delta) in &textures_delta.set {
            let staging = self.textures.update_texture(*id, image_delta, recorder);
            if let Some(staging) = staging {
                self.staging_buffers.push((staging, current_index));
            }
        }

        for &_id in &textures_delta.free {
            // TODO: implement this in the future
        }
    }

    pub fn render(
        &mut self,
        recorder: &mut CommandRecorder,
        output: egui::FullOutput,
        queue: &Queue,
        frame: &Frame,
    ) {
        let egui::FullOutput {
            textures_delta,
            shapes,
            ..
        } = output;
        self.update_textures(recorder, &textures_delta, queue);

        let timeline = queue.current_timeline();
        self.staging_buffers
            .retain(|(_, submission)| *submission > timeline);

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

        frame_resources
            .vertex_buffer
            .upload(&vertex_buffer_writer, 0);
        frame_resources.index_buffer.upload(&index_buffer_writer, 0);

        let color_attachment = vk::RenderingAttachmentInfo::default()
            .image_view(frame.image.view)
            .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::LOAD)
            .store_op(vk::AttachmentStoreOp::STORE);

        recorder.begin_rendering(
            &[color_attachment],
            vk::Extent2D {
                width: self.size.width,
                height: self.size.height,
            },
        );

        recorder.bind_pipeline(&self.pipeline);
        recorder.viewport(vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: self.size.width as f32,
            height: self.size.height as f32,
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
                    if mesh.vertices.is_empty() && mesh.indices.is_empty() {
                        continue;
                    }
                    let clip_rect = primitive.clip_rect;

                    let ppi = self.scale_factor as f32;
                    let clip_min_x = clip_rect.min.x * ppi;
                    let clip_min_y = clip_rect.min.y * ppi;
                    let clip_max_x = clip_rect.max.x * ppi;
                    let clip_max_y = clip_rect.max.y * ppi;

                    let clip_min_x = clip_min_x.round() as u32;
                    let clip_min_y = clip_min_y.round() as u32;
                    let clip_max_x = clip_max_x.round() as u32;
                    let clip_max_y = clip_max_y.round() as u32;

                    let clip_min_x = clip_min_x.clamp(0, self.size.width);
                    let clip_min_y = clip_min_y.clamp(0, self.size.height);
                    let clip_max_x = clip_max_x.clamp(clip_min_x, self.size.width);
                    let clip_max_y = clip_max_y.clamp(clip_min_y, self.size.height);

                    let scissor = vk::Rect2D {
                        offset: vk::Offset2D {
                            x: clip_min_x as i32,
                            y: clip_min_y as i32,
                        },
                        extent: vk::Extent2D {
                            width: (clip_max_x - clip_min_x),
                            height: (clip_max_y - clip_min_y),
                        },
                    };

                    recorder.scissor(scissor);

                    let texture_index = self
                        .textures
                        .get_binding_index(&mesh.texture_id)
                        .unwrap_or(0);

                    let pc = EguiPushConstants {
                        size: [self.size.width as f32 / ppi, self.size.height as f32 / ppi],
                        texture: texture_index,
                        _padding: 0,
                    };

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

    pub fn begin_frame(&mut self, window: &winit::window::Window) {
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
        }
    }
}

const EGUI_SHADER: &str = r#"
struct VertexInput {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

struct PushConstants {
    size: vec2<f32>,
    texture: u32,
    _padding: u32,
}

var<push_constant> pc: PushConstants;

@group(0) @binding(0) 
var textures: binding_array<texture_2d<f32>, 512>; 
@group(0) @binding(1) 
var t_sampler: sampler;

@vertex
fn vmain(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    let pos = (in.pos / pc.size) * 2.0 - 1.0;
    out.pos = vec4<f32>(pos.x, pos.y, 0.0, 1.0);

    out.uv = in.uv;
    out.color = in.color;

    return out;
}

@fragment
fn fmain(in: VertexOutput) -> @location(0) vec4<f32> {
    let texture_color = textureSample(textures[pc.texture], t_sampler, in.uv);
    return in.color * texture_color;
}
"#;
