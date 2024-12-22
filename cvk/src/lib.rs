extern crate nalgebra as na;
extern crate vk_mem as vkm;

mod adapter;
mod buffer;
mod command;
mod descriptor;
mod device;
mod instance;
mod pipeline;
mod queues;
mod semaphore;
mod swapchain;
mod texture;

pub mod egui;
pub mod utils;

pub use adapter::Adapter;
pub use device::Device;
pub use instance::Instance;

pub use buffer::{Buffer, BufferInfo};
pub use command::CommandRecorder;
pub use descriptor::*;
pub use pipeline::*;
pub use queues::{Queue, QueueRequest};
pub use semaphore::Semaphore;
pub use swapchain::{FrameSignals, Surface, Swapchain};
pub use texture::{Frame, Image, ImageTransition, SamplerInfo, Texture, TextureInfo};

pub use ash as raw;
use ash::vk;
pub use ash::vk::{Format, QueueFlags};

use anyhow::Result;
use naga::back::spv;

pub struct Shader {
    module: naga::Module,
    info: naga::valid::ModuleInfo,
    source: String,
}

impl Shader {
    pub fn entry<'a>(&'a self, name: &'a str) -> ShaderFunction<'a> {
        ShaderFunction {
            shader: self,
            entry_point: name,
        }
    }
}

#[derive(Clone, Copy)]
pub struct ShaderFunction<'a> {
    pub shader: &'a Shader,
    pub entry_point: &'a str,
}

impl ShaderFunction<'_> {
    pub fn entry_point_idx(&self) -> Result<usize> {
        self.shader
            .module
            .entry_points
            .iter()
            .position(|ep| ep.name == self.entry_point)
            .ok_or_else(|| anyhow::anyhow!("Entry Point not found in the Shader"))
    }

    pub fn to_spirv(&self) -> Result<Vec<u32>> {
        let entry_point_idx = self.entry_point_idx()?;
        let entry = &self.shader.module.entry_points[entry_point_idx];

        let flags = spv::WriterFlags::empty();
        let options = spv::Options {
            flags,
            lang_version: (1, 3),
            bounds_check_policies: naga::proc::BoundsCheckPolicies::default(),
            debug_info: None,
            ..Default::default()
        };

        let pipeline_options = spv::PipelineOptions {
            entry_point: entry.name.clone(),
            shader_stage: entry.stage,
        };
        let mut writer = spv::Writer::new(&options).map_err(|e| anyhow::anyhow!("{:?}", e))?;

        let mut compiled = Vec::new();
        writer
            .write(
                &self.shader.module,
                &self.shader.info,
                Some(&pipeline_options),
                &None,
                &mut compiled,
            )
            .map_err(|e| anyhow::anyhow!("{:?}", e))?;

        Ok(compiled)
    }
}

impl Device {
    pub fn create_shader(&self, source: &str) -> Result<Shader> {
        let module = naga::front::wgsl::parse_str(source)?;

        let flags = naga::valid::ValidationFlags::all();
        let capabilities = naga::valid::Capabilities::all(); // why not

        let info = naga::valid::Validator::new(flags, capabilities).validate(&module)?;

        Ok(Shader {
            module,
            info,
            source: source.to_owned(),
        })
    }

    pub fn create_shader_module(&self, shader: ShaderFunction<'_>) -> Result<vk::ShaderModule> {
        let spirv = shader.to_spirv()?;

        let info = vk::ShaderModuleCreateInfo::default().code(&spirv);

        let module = unsafe { self.handle.create_shader_module(&info, None)? };

        Ok(module)
    }
}
