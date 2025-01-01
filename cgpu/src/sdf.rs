use std::sync::Arc;

use crate::bricks::BrickState;
pub struct SDFGenerator {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    bricks: Arc<BrickState>,
    pipeline: wgpu::ComputePipeline,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct SdfComputeUniform {
    dimensions: [u32; 3],
    _padding: u32,
    num_steps: u32,
    current_step: u32,
    _padding2: [u32; 2],
}

impl SDFGenerator {
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        bricks: Arc<BrickState>,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SDF Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!("sdf.wgsl"))),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SDF Parameters Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("SDF Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout, bricks.layout()],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("SDF Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            cache: None,
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SDF Uniform Buffer"),
            size: std::mem::size_of::<SdfComputeUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SDF Uniform Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        Self {
            device,
            queue,
            bricks,
            pipeline,
            uniform_buffer,
            uniform_bind_group,
        }
    }

    pub fn generate(&self, num_steps: u32) {
        let dims = self.bricks.brickmap.dimensions(); // [u32; 3]
        let wg_size = [8, 8, 4];
        let groups_x = (dims[0] + wg_size[0] - 1) / wg_size[0];
        let groups_y = (dims[1] + wg_size[1] - 1) / wg_size[1];
        let groups_z = (dims[2] + wg_size[2] - 1) / wg_size[2];

        let init_uniform = SdfComputeUniform {
            dimensions: *dims.as_ref(),
            _padding: 0,
            num_steps,
            current_step: 0,
            _padding2: [0; 2],
        };

        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&init_uniform));

        {
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Initialization Encoder"),
                });
            {
                let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("Init Compute Pass"),
                    ..Default::default()
                });
                cpass.set_pipeline(&self.pipeline);
                cpass.set_bind_group(0, &self.uniform_bind_group, &[]);
                cpass.set_bind_group(1, self.bricks.bind_group(), &[]);
                cpass.dispatch_workgroups(groups_x, groups_y, groups_z);
            }
            self.queue.submit(Some(encoder.finish()));
        }

        for step in 1..=num_steps {
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("SDF Compute Encoder"),
                });
            let uniform_data = SdfComputeUniform {
                dimensions: *dims.as_ref(),
                _padding: 0,
                num_steps,
                current_step: step,
                _padding2: [0, 0],
            };

            // Update the uniform buffer
            self.queue
                .write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniform_data));

            {
                let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("SDF Compute Pass"),
                    ..Default::default()
                });
                cpass.set_pipeline(&self.pipeline);
                cpass.set_bind_group(0, &self.uniform_bind_group, &[]);
                cpass.set_bind_group(1, self.bricks.bind_group(), &[]);
                cpass.dispatch_workgroups(groups_x, groups_y, groups_z);
            }
            // Finish and submit all steps at once
            self.queue.submit(Some(encoder.finish()));
            self.device.poll(wgpu::Maintain::Wait);
        }
    }
}
