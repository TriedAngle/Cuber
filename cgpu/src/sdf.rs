use std::{mem, sync::Arc};

use crate::GPUBrickMap;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
pub struct SDFPushConstants {
    pub dimensions: [u32; 3],
    pub num_steps: u32,
    pub current_step: u32,
}

pub struct SDFOptimizer {
    pipeline: cvk::ComputePipeline,
    brickmap: Arc<GPUBrickMap>,
    queue: Arc<cvk::Queue>,
    #[allow(unused)]
    device: Arc<cvk::Device>,
}

unsafe impl Send for SDFOptimizer {}
unsafe impl Sync for SDFOptimizer {}

impl SDFOptimizer {
    pub fn new(
        device: Arc<cvk::Device>,
        brickmap: Arc<GPUBrickMap>,
        queue: Arc<cvk::Queue>,
    ) -> Self {
        let shader = device
            .create_shader(include_str!("shaders/sdf.wgsl"))
            .unwrap();

        let pipeline = device.create_compute_pipeline(&cvk::ComputePipelineInfo {
            label: Some("SDF Pipeline"),
            shader: shader.entry("main"),
            descriptor_layouts: &[&brickmap.context.layout],
            push_constant_size: Some(mem::size_of::<SDFPushConstants>() as u32),
            ..Default::default()
        });

        Self {
            pipeline,
            brickmap,
            queue,
            device,
        }
    }

    pub fn run(&self) {
        let dims: [u32; 3] = *self.brickmap.cpu.dimensions().as_ref();
        let wg_size = [8, 8, 4];
        let groups_x = (dims[0] + wg_size[0] - 1) / wg_size[0];
        let groups_y = (dims[1] + wg_size[1] - 1) / wg_size[1];
        let groups_z = (dims[2] + wg_size[2] - 1) / wg_size[2];

        let max_dim = dims[0].max(dims[1]).max(dims[2]);
        let steps = (max_dim as f32).log2().ceil() as u32 + 1;

        let mut pc = SDFPushConstants {
            dimensions: *self.brickmap.cpu.dimensions().as_ref(),
            num_steps: steps,
            current_step: 0,
        };

        {
            let mut recorder = self.queue.record();
            recorder.bind_pipeline(&self.pipeline);
            recorder.bind_descriptor_set(&self.brickmap.context.descriptors, 0, &[]);
            recorder.push_constants(pc);
            recorder.dispatch(groups_x, groups_y, groups_z);
            let _ = self.queue.submit_express(&[recorder.finish()]);
        }
        for step in 1..=steps {
            pc.current_step = step;
            let mut recorder = self.queue.record();
            recorder.bind_pipeline(&self.pipeline);
            recorder.bind_descriptor_set(&self.brickmap.context.descriptors, 0, &[]);
            recorder.push_constants(pc);
            recorder.dispatch(groups_x, groups_y, groups_z);
            let _ = self.queue.submit_express(&[recorder.finish()]);
        }
    }
}
