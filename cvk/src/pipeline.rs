use ash::vk;
use std::marker::PhantomData;
use std::sync::Arc;

use crate::{DescriptorSetLayout, Device, ShaderFunction};

pub struct ComputePipelineInfo<'a> {
    pub shader: ShaderFunction<'a>,
    pub descriptor_layouts: &'a [&'a DescriptorSetLayout],
    pub push_constant_size: Option<u32>,
    pub cache: Option<vk::PipelineCache>,
    pub label: Option<&'a str>,
    pub tag: Option<(u64, &'a [u8])>,
}

pub struct RenderPipelineInfo<'a> {
    pub vertex_shader: ShaderFunction<'a>,
    pub fragment_shader: ShaderFunction<'a>,
    pub color_formats: &'a [vk::Format],
    pub depth_format: Option<vk::Format>,
    pub descriptor_layouts: &'a [&'a DescriptorSetLayout],
    pub push_constant_size: Option<u32>,
    pub blend_states: Option<&'a [vk::PipelineColorBlendAttachmentState]>,
    pub vertex_input_state: Option<vk::PipelineVertexInputStateCreateInfo<'a>>,
    pub topology: vk::PrimitiveTopology,
    pub polygon: vk::PolygonMode,
    pub cull: vk::CullModeFlags,
    pub front_face: vk::FrontFace,
    pub label: Option<&'a str>,
    pub tag: Option<(u64, &'a [u8])>,
}

pub trait Pipeline {
    const BIND_POINT: vk::PipelineBindPoint;
    const SHADER_STAGE_FLAGS: vk::ShaderStageFlags;

    fn handle(&self) -> vk::Pipeline;
    fn layout(&self) -> vk::PipelineLayout;
    fn bind_point(&self) -> vk::PipelineBindPoint {
        Self::BIND_POINT
    }
    fn flags(&self) -> vk::ShaderStageFlags {
        Self::SHADER_STAGE_FLAGS
    }
}

pub struct ComputePipeline {
    pub handle: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    pub shader: vk::ShaderModule,
    device: Arc<ash::Device>,
}

pub struct RenderPipeline {
    pub handle: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    pub vertex_shader: vk::ShaderModule,
    pub fragment_shader: vk::ShaderModule,
    device: Arc<ash::Device>,
}

impl Pipeline for ComputePipeline {
    const BIND_POINT: vk::PipelineBindPoint = vk::PipelineBindPoint::COMPUTE;
    const SHADER_STAGE_FLAGS: vk::ShaderStageFlags = vk::ShaderStageFlags::COMPUTE;
    fn handle(&self) -> vk::Pipeline {
        self.handle
    }
    fn layout(&self) -> vk::PipelineLayout {
        self.layout
    }
}

impl Pipeline for RenderPipeline {
    const BIND_POINT: vk::PipelineBindPoint = vk::PipelineBindPoint::GRAPHICS;
    // this is so silly
    const SHADER_STAGE_FLAGS: vk::ShaderStageFlags = vk::ShaderStageFlags::from_raw(
        vk::ShaderStageFlags::VERTEX.as_raw() | vk::ShaderStageFlags::FRAGMENT.as_raw(),
    );
    fn handle(&self) -> vk::Pipeline {
        self.handle
    }
    fn layout(&self) -> vk::PipelineLayout {
        self.layout
    }
}

impl Device {
    pub fn create_compute_pipeline(&self, info: &ComputePipelineInfo<'_>) -> ComputePipeline {
        let module = self.create_shader_module(info.shader).unwrap();
        let mut push_constant_ranges = Vec::new();
        if let Some(size) = info.push_constant_size {
            push_constant_ranges.push(
                vk::PushConstantRange::default()
                    .stage_flags(vk::ShaderStageFlags::COMPUTE)
                    .size(size),
            );
        }

        let layouts = info
            .descriptor_layouts
            .iter()
            .map(|l| l.handle)
            .collect::<Vec<_>>();

        let layout_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(&layouts)
            .push_constant_ranges(&push_constant_ranges);

        let layout = unsafe {
            self.handle
                .create_pipeline_layout(&layout_info, None)
                .unwrap()
        };

        let stage_name = std::ffi::CString::new(info.shader.entry_point).unwrap();
        let stage = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::COMPUTE)
            .module(module)
            .name(&stage_name);

        let create_info = vk::ComputePipelineCreateInfo::default()
            .stage(stage)
            .layout(layout);

        let handle = unsafe {
            let cache = info.cache.unwrap_or(vk::PipelineCache::null());
            self.handle
                .create_compute_pipelines(cache, &[create_info], None)
                .unwrap()[0]
        };

        self.set_object_debug_info(handle, info.label, info.tag);

        ComputePipeline {
            handle,
            layout,
            shader: module,
            device: self.handle.clone(),
        }
    }

    pub fn create_render_pipeline(&self, info: &RenderPipelineInfo<'_>) -> RenderPipeline {
        let vertex_shader = self.create_shader_module(info.vertex_shader).unwrap();
        let fragment_shader = self.create_shader_module(info.fragment_shader).unwrap();

        let mut push_constant_ranges = Vec::new();
        if let Some(size) = info.push_constant_size {
            push_constant_ranges.push(
                vk::PushConstantRange::default()
                    .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
                    .size(size),
            );
        }

        let layouts = info
            .descriptor_layouts
            .iter()
            .map(|l| l.handle)
            .collect::<Vec<_>>();

        let layout_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(&layouts)
            .push_constant_ranges(&push_constant_ranges);

        let layout = unsafe {
            self.handle
                .create_pipeline_layout(&layout_info, None)
                .unwrap()
        };

        let vertex_stage_name = std::ffi::CString::new(info.vertex_shader.entry_point).unwrap();
        let fragment_stage_name = std::ffi::CString::new(info.fragment_shader.entry_point).unwrap();

        let stages = [
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(vertex_shader)
                .name(&vertex_stage_name),
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(fragment_shader)
                .name(&fragment_stage_name),
        ];

        let vertex_input = info
            .vertex_input_state
            .unwrap_or_else(|| vk::PipelineVertexInputStateCreateInfo::default());

        let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(info.topology)
            .primitive_restart_enable(false);

        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);

        let rasterization = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(info.polygon)
            .line_width(1.0)
            .cull_mode(info.cull)
            .front_face(info.front_face);

        let multisample = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);

        let color_blend_attachment = info.blend_states.as_ref().map_or_else(
            || {
                vec![vk::PipelineColorBlendAttachmentState::default()
                    .color_write_mask(vk::ColorComponentFlags::RGBA)
                    .blend_enable(false)]
            },
            |&states| states.iter().copied().collect::<Vec<_>>(),
        );

        let color_blend = vk::PipelineColorBlendStateCreateInfo::default()
            .logic_op_enable(false)
            .attachments(&color_blend_attachment);

        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];

        let dynamic_state =
            vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

        let mut rendering_info = vk::PipelineRenderingCreateInfo::default()
            .color_attachment_formats(&info.color_formats);

        if let Some(format) = info.depth_format {
            rendering_info = rendering_info.depth_attachment_format(format);
        }

        let create_info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&stages)
            .vertex_input_state(&vertex_input)
            .input_assembly_state(&input_assembly)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterization)
            .multisample_state(&multisample)
            .color_blend_state(&color_blend)
            .dynamic_state(&dynamic_state)
            .layout(layout)
            .push_next(&mut rendering_info);

        let handle = unsafe {
            self.handle
                .create_graphics_pipelines(vk::PipelineCache::null(), &[create_info], None)
                .unwrap()[0]
        };

        self.set_object_debug_info(handle, info.label, info.tag);

        RenderPipeline {
            handle,
            layout,
            vertex_shader,
            fragment_shader,
            device: self.handle.clone(),
        }
    }
}

// In a new file vertex.rs:

pub trait VertexAttribute {
    fn format() -> vk::Format;
    fn size() -> u32;
}

impl VertexAttribute for [f32; 2] {
    fn format() -> vk::Format {
        vk::Format::R32G32_SFLOAT
    }
    fn size() -> u32 {
        8
    }
}

impl VertexAttribute for [f32; 3] {
    fn format() -> vk::Format {
        vk::Format::R32G32B32_SFLOAT
    }
    fn size() -> u32 {
        12
    }
}

impl VertexAttribute for [f32; 4] {
    fn format() -> vk::Format {
        vk::Format::R32G32B32A32_SFLOAT
    }
    fn size() -> u32 {
        16
    }
}

impl VertexAttribute for [u8; 4] {
    fn format() -> vk::Format {
        vk::Format::R8G8B8A8_UNORM
    }
    fn size() -> u32 {
        4
    }
}

#[derive(Default)]
pub struct VertexAttributeBuilder {
    attributes: Vec<vk::VertexInputAttributeDescription>,
    current_offset: u32,
}

impl VertexAttributeBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add<T: VertexAttribute>(mut self, location: u32, binding: u32) -> Self {
        self.attributes.push(
            vk::VertexInputAttributeDescription::default()
                .location(location)
                .binding(binding)
                .format(T::format())
                .offset(self.current_offset),
        );
        self.current_offset += T::size();
        self
    }

    pub fn build(self) -> (Vec<vk::VertexInputAttributeDescription>, u32) {
        (self.attributes, self.current_offset)
    }
}

pub struct VertexFormat<V> {
    bindings: Vec<vk::VertexInputBindingDescription>,
    attributes: Vec<vk::VertexInputAttributeDescription>,
    _phantom: PhantomData<V>,
}

impl<V> VertexFormat<V> {
    pub fn new(binding: u32, input_rate: vk::VertexInputRate) -> Self {
        Self {
            bindings: vec![vk::VertexInputBindingDescription::default()
                .binding(binding)
                .stride(0) // Will be set when attributes are added
                .input_rate(input_rate)],
            attributes: Vec::new(),
            _phantom: PhantomData,
        }
    }

    pub fn with_attributes<F>(mut self, f: F) -> Self
    where
        F: FnOnce(VertexAttributeBuilder) -> VertexAttributeBuilder,
    {
        let builder = VertexAttributeBuilder::new();
        let (attributes, stride) = f(builder).build();
        self.attributes = attributes;
        self.bindings[0].stride = stride;
        self
    }

    // Instead of create_info, provide the raw Vulkan components that the pipeline needs
    pub fn get_vertex_input_state(&self) -> vk::PipelineVertexInputStateCreateInfo {
        vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(&self.bindings)
            .vertex_attribute_descriptions(&self.attributes)
    }

    // Helper method to get just the binding descriptions
    pub fn get_bindings(&self) -> &[vk::VertexInputBindingDescription] {
        &self.bindings
    }

    // Helper method to get just the attribute descriptions
    pub fn get_attributes(&self) -> &[vk::VertexInputAttributeDescription] {
        &self.attributes
    }
}

// Example vertex types using the new abstractions
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex2D {
    pub position: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

impl Vertex2D {
    pub fn format() -> VertexFormat<Self> {
        VertexFormat::new(0, vk::VertexInputRate::VERTEX).with_attributes(|builder| {
            builder
                .add::<[f32; 2]>(0, 0) // position
                .add::<[f32; 2]>(1, 0) // uv
                .add::<[f32; 4]>(2, 0) // color
        })
    }
}

impl Drop for ComputePipeline {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_shader_module(self.shader, None);
            self.device.destroy_pipeline_layout(self.layout, None);
            self.device.destroy_pipeline(self.handle, None);
        }
    }
}

impl Drop for RenderPipeline {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_pipeline(self.handle, None);
            self.device.destroy_pipeline_layout(self.layout, None);
            self.device.destroy_shader_module(self.vertex_shader, None);
            self.device
                .destroy_shader_module(self.fragment_shader, None);
        }
    }
}
