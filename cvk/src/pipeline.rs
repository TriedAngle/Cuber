use ash::vk;
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
    pub topology: vk::PrimitiveTopology,
    pub polygon: vk::PolygonMode,
    pub cull: vk::CullModeFlags,
    pub front_face: vk::FrontFace,
    pub label: Option<&'a str>,
    pub tag: Option<(u64, &'a [u8])>,
}

pub trait Pipeline {
    const BIND_POINT: vk::PipelineBindPoint;
    fn handle(&self) -> vk::Pipeline;
    fn layout(&self) -> vk::PipelineLayout;
    fn bind_point(&self) -> vk::PipelineBindPoint {
        Self::BIND_POINT
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
    fn handle(&self) -> vk::Pipeline {
        self.handle
    }
    fn layout(&self) -> vk::PipelineLayout {
        self.layout
    }
}

impl Pipeline for RenderPipeline {
    const BIND_POINT: vk::PipelineBindPoint = vk::PipelineBindPoint::GRAPHICS;
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

        let vertex_input = vk::PipelineVertexInputStateCreateInfo::default();

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
