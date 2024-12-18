extern crate nalgebra as na;
extern crate vk_mem as vkm;

mod instance;
mod device;
mod queues;
mod adapter;
mod surface;
mod texture;

pub mod utils;

pub use instance::Instance;
pub use adapter::Adapter;
pub use device::Device;
pub use queues::{Queue, QueueRequest};
pub use surface::Surface;

pub use ash::vk::{Format, QueueFlags};
pub use ash as raw;

use anyhow::Result;


pub struct Shader {
    module: naga::Module,
    info: naga::valid::ModuleInfo,
    source: String,
}

#[derive(Clone, Copy)]
pub struct ShaderFunction<'a> {
    pub shader: &'a Shader,
    pub entry_point: &'a str,
}

impl ShaderFunction<'_> { 
    pub fn entry_point_idx(&self) -> usize { 
        self.shader
            .module
            .entry_points
            .iter()
            .position(|ep| ep.name == self.entry_point)
            .expect("Entry Point not found in the Shader")
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
}
