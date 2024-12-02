extern crate nalgebra as na;

use std::{mem, sync::Arc, time::Duration};

use camera::Camera;
use game::{input::Input, Transform};
use mesh::{SimpleTextureMesh, TexVertex, Vertex};
use texture::Texture;
use wgpu::util::{DeviceExt, RenderEncoder};
use winit::{dpi::PhysicalSize, window::Window};

mod bricks;
mod camera;
mod mesh;
mod texture;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    pub view_projection: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ModelUniform {
    pub transform: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct VoxelUniforms {
    pub resolution: [f32; 2],
    pub dt: f32,
    pub padding: f32,
}

impl ModelUniform {
    pub fn new(matrix: &na::Matrix4<f32>) -> Self {
        Self {
            transform: *matrix.as_ref(),
        }
    }
}

impl CameraUniform {
    fn new() -> Self {
        Self {
            view_projection: *na::Matrix4::identity().as_ref(),
        }
    }

    fn update(&mut self, camera: &Camera) {
        let view_projection = camera.view_projection_matrix();
        self.view_projection = *view_projection.as_ref();
    }
}

pub struct RenderContext {
    pub window: Arc<Window>,
    pub instance: wgpu::Instance,
    pub surface: wgpu::Surface<'static>,
    pub surface_config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,

    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub render_pipeline: wgpu::RenderPipeline,

    pub encoder: Option<wgpu::CommandEncoder>,
    pub output_texture: Option<wgpu::SurfaceTexture>,
    pub meshes: Vec<SimpleTextureMesh>,
    pub model_bind_group_layout: wgpu::BindGroupLayout,
    diffuse_bind_group: wgpu::BindGroup,
    texture: Texture,
    pub camera: Camera,
    camera_uniform: CameraUniform,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    depth_texture: Texture,

    compute_uniforms: VoxelUniforms,
    compute_uniforms_buffer: wgpu::Buffer,
    compute_render_texture: Texture,
    compute_bind_group: wgpu::BindGroup,
    compute_pipeline: wgpu::ComputePipeline,

    compute_present_bind_group: wgpu::BindGroup,
    compute_present_pipeline: wgpu::RenderPipeline,
}

const TEX_VERTICES: &[TexVertex] = &[
    TexVertex {
        position: [-0.0868241, 0.49240386, 0.0],
        tex_coords: [0.4131759, 0.99240386],
    }, // A
    TexVertex {
        position: [-0.49513406, 0.06958647, 0.0],
        tex_coords: [0.0048659444, 0.56958647],
    }, // B
    TexVertex {
        position: [-0.21918549, -0.44939706, 0.0],
        tex_coords: [0.28081453, 0.05060294],
    }, // C
    TexVertex {
        position: [0.35966998, -0.3473291, 0.0],
        tex_coords: [0.85967, 0.1526709],
    }, // D
    TexVertex {
        position: [0.44147372, 0.2347359, 0.0],
        tex_coords: [0.9414737, 0.7347359],
    }, // E
];

const TEX_INDICES: &[u32] = &[0, 1, 4, 1, 2, 4, 2, 3, 4];

const TEX_VERTICES2: &[TexVertex] = &[
    TexVertex {
        position: [0.0, 0.5, 0.0],
        tex_coords: [0.0, 0.0],
    },
    TexVertex {
        position: [-0.5, -0.5, 0.0],
        tex_coords: [1.0, 1.0],
    },
    TexVertex {
        position: [0.5, -0.5, 0.0],
        tex_coords: [0.3, 0.8],
    },
];

const TEX_INDICES2: &[u32] = &[0, 1, 2];

impl RenderContext {
    pub async fn new(window: Arc<Window>) -> RenderContext {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::GL,
            ..Default::default()
        });

        let static_window: &'static Window = unsafe { mem::transmute(&*window) };
        let surface = match instance.create_surface(static_window) {
            Ok(surface) => surface,
            Err(e) => panic!("Error creating surface: {:?}", e),
        };

        let adapter = match instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
        {
            Some(adapter) => adapter,
            None => panic!("Error creating adapter"),
        };

        let features = adapter.features();

        if !features.contains(wgpu::Features::TIMESTAMP_QUERY) {
            panic!("TIMESTAMP QUERY REQUIRED");
        };

        let (device, queue) = match adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    required_features: wgpu::Features::empty()
                        | wgpu::Features::TIMESTAMP_QUERY
                        | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS,
                    required_limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                    label: None,
                    memory_hints: Default::default(),
                },
                None,
            )
            .await
        {
            Ok(res) => res,
            Err(e) => panic!("Error requesting device and queue: {:?}", e),
        };

        let surface_caps = surface.get_capabilities(&adapter);

        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        let diffuse_bytes = include_bytes!("../../assets/happy-tree.png");
        let texture =
            Texture::from_bytes(&device, &queue, diffuse_bytes, Some("Happy Tree Texture"));

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        // This should match the filterable field of the
                        // corresponding Texture entry above.
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        let diffuse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(texture.sampler()),
                },
            ],
            label: Some("diffuse_bind_group"),
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("tex_shader.wgsl").into()),
        });

        let model_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("Model Bind Group Layout"),
            });

        let mut model_transform = Transform::identity();
        model_transform.position(&na::Vector3::new(0., 0., -2.));
        model_transform.scale_nonuniform(&na::Vector3::new(2.0, 2.0, 1.0));
        model_transform.rotate_around(&na::Vector3::y_axis(), 15.0);

        let mesh = SimpleTextureMesh::new(
            &device,
            model_transform,
            TEX_VERTICES,
            TEX_INDICES,
            &model_bind_group_layout,
            None,
            None,
        );

        let mut model_transform = Transform::identity();
        model_transform.position(&na::Vector3::new(-0.5, 1., -2.));
        model_transform.scale_nonuniform(&na::Vector3::new(2.0, 1.0, 1.0));
        model_transform.rotate_around(&na::Vector3::z_axis(), -30.0);

        let mesh2 = SimpleTextureMesh::new(
            &device,
            model_transform,
            TEX_VERTICES2,
            TEX_INDICES2,
            &model_bind_group_layout,
            None,
            None,
        );

        let mut meshes = Vec::new();
        meshes.push(mesh);
        meshes.push(mesh2);

        let mut camera = Camera::new(
            na::Point3::new(1., 0., 2.),
            na::UnitQuaternion::from_euler_angles(0., 0., 0.),
            5.,
            0.002,
            45.,
            size.width as f32 / size.height as f32,
            0.1,
            100.,
        );

        camera.look_at(na::Point3::origin(), &na::Vector3::y_axis());

        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update(&camera);

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("CameraUniform"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("camera_bind_group_layout"),
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });

        let depth_texture =
            texture::Texture::create_depth_texture(&device, &surface_config, Some("depth_texture"));

        // COMPUTE PARTS
        let voxel_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("voxels.wgsl").into()),
        });

        let compute_present_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Compute Present Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("compute_present.wgsl").into()),
        });

        let compute_uniforms = VoxelUniforms {
            resolution: [size.width as f32, size.height as f32],
            dt: 0.,
            padding: 0.,
        };

        let compute_uniforms_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Voxel Uniforms"),
                contents: bytemuck::cast_slice(&[compute_uniforms]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let compute_render_texture =
            Texture::create_storage_texture(&device, &surface_config, Some("Compute Texture"));

        let compute_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Compute Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: wgpu::TextureFormat::Rgba8Unorm,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                ],
            });

        let compute_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Compute Bind Group"),
            layout: &compute_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: compute_uniforms_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&compute_render_texture.view),
                },
            ],
        });

        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Compute Pipeline Layout"),
                bind_group_layouts: &[&compute_bind_group_layout],
                push_constant_ranges: &[],
            });

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Compute Pipeline Layout"),
            layout: Some(&compute_pipeline_layout),
            module: &voxel_shader,
            entry_point: "main",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let compute_present_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Render Bind Group Layout"),
            entries: &[
                // Input Texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let compute_present_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Render Bind Group"),
            layout: &compute_present_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&compute_render_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&compute_render_texture.sampler()),
                },
            ],
        });

        let compute_present_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&compute_present_bind_group_layout],
            push_constant_ranges: &[],
        });

        let compute_present_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&compute_present_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &compute_present_shader,
                entry_point: "vs_main",
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &compute_present_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // COMPUTE PARTS END
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[
                    &texture_bind_group_layout,
                    &camera_bind_group_layout,
                    &model_bind_group_layout,
                    &compute_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[TexVertex::layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                // cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        Self {
            instance,
            window,
            surface,
            surface_config,
            size,
            device,
            queue,
            render_pipeline,
            diffuse_bind_group,
            texture,
            meshes,
            model_bind_group_layout,
            camera,
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            depth_texture,
            encoder: None,
            output_texture: None,
            compute_uniforms,
            compute_uniforms_buffer,
            compute_bind_group,
            compute_render_texture,
            compute_pipeline,

            compute_present_bind_group,
            compute_present_pipeline,
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn resize(&mut self, new: PhysicalSize<u32>) {
        log::info!("resize window: {:?}, {:?}", self.size, new);
        self.size = new;
        self.surface_config.width = new.width;
        self.surface_config.height = new.height;
        self.surface.configure(&self.device, &self.surface_config);
        self.depth_texture = texture::Texture::create_depth_texture(
            &self.device,
            &self.surface_config,
            Some("depth_texture"),
        );
    }

    pub fn compute_test(&mut self) {
        let device = &self.device;

        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            count: 2,
            ty: wgpu::QueryType::Timestamp,
            label: None,
        });

        let compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("compute.wgsl").into()),
        });

        let input_floats = &[1.0f32, 2.0f32];
        let input: &[u8] = bytemuck::bytes_of(input_floats);
        let input_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: input,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
        });

        let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: input.len() as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let query_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 16,
            usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::QUERY_RESOLVE,
            mapped_at_creation: false,
        });

        let query_staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 16,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let compute_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&compute_bind_group_layout],
                push_constant_ranges: &[],
            });
        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&compute_pipeline_layout),
            module: &compute_shader,
            entry_point: "main",
            cache: None,
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        });

        let compute_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &compute_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: input_buffer.as_entire_binding(),
            }],
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Compute Encoder"),
        });

        encoder.write_timestamp(&query_set, 0);

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                ..Default::default()
            });
            compute_pass.set_pipeline(&compute_pipeline);
            compute_pass.set_bind_group(0, &compute_bind_group, &[]);
            compute_pass.dispatch_workgroups(input_floats.len() as u32, 1, 1);
        }

        encoder.write_timestamp(&query_set, 1);

        encoder.copy_buffer_to_buffer(&input_buffer, 0, &output_buffer, 0, input.len() as u64);

        encoder.resolve_query_set(&query_set, 0..2, &query_buffer, 0);

        encoder.copy_buffer_to_buffer(&query_buffer, 0, &query_staging_buffer, 0, 16);

        self.queue.submit(Some(encoder.finish()));

        let buffer_slice = output_buffer.slice(..);
        let query_slice = query_staging_buffer.slice(..);
        buffer_slice.map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::Maintain::Wait);
        query_slice.map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::Maintain::Wait);

        let data_raw = &*buffer_slice.get_mapped_range();
        let data: &[f32] = bytemuck::cast_slice(data_raw);
        println!("Data: {:?}", &*data);

        let ts_period = self.queue.get_timestamp_period();
        let ts_data_raw = &*query_slice.get_mapped_range();
        let ts_data: &[u64] = bytemuck::cast_slice(ts_data_raw);
        println!(
            "Compute shader elapsed: {:?}ms",
            (ts_data[1] - ts_data[0]) as f64 * ts_period as f64 * 1e-6
        );
    }

    pub fn update(&mut self) {}

    pub fn prepare_render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        self.output_texture = Some(output);
        self.encoder = Some(encoder);
        Ok(())
    }

    pub fn finish_render(&mut self) {
        let output = self.output_texture.take().expect("Render must be prepared");
        let encoder = self.encoder.take().expect("Render must be prepared");

        self.queue.submit([encoder.finish()]);
        output.present();
    }

    pub fn render(&mut self) {
        let encoder = self.encoder.as_mut().expect("Render must be prepared");
        let output = self
            .output_texture
            .as_ref()
            .expect("Render must be prepared");

        let (width, height) = (self.surface_config.width, self.surface_config.height);

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        {
            let workgroup_size = 8;
            let workgroup_x = (width + workgroup_size - 1) / workgroup_size;
            let workgroup_y = (height + workgroup_size - 1) / workgroup_size;

            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Compute Pass"),
                ..Default::default()
            });

            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(0, &self.compute_bind_group, &[]);
            compute_pass.dispatch_workgroups(workgroup_x, workgroup_y, 1);
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.compute_present_pipeline);
            render_pass.set_bind_group(0, &self.compute_present_bind_group, &[]);
            render_pass.draw(0..6, 0..1); // Draw the full-screen quad
        }

        // {
        //     let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        //         label: Some("Render Pass"),
        //         color_attachments: &[Some(wgpu::RenderPassColorAttachment {
        //             view: &view,
        //             resolve_target: None,
        //             ops: wgpu::Operations {
        //                 load: wgpu::LoadOp::Clear(wgpu::Color {
        //                     r: 0.1,
        //                     g: 0.2,
        //                     b: 0.3,
        //                     a: 1.0,
        //                 }),
        //                 store: wgpu::StoreOp::Store,
        //             },
        //         })],
        //         depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
        //             view: &self.depth_texture.view,
        //             depth_ops: Some(wgpu::Operations {
        //                 load: wgpu::LoadOp::Clear(1.0),
        //                 store: wgpu::StoreOp::Store,
        //             }),
        //             stencil_ops: None,
        //         }),
        //         occlusion_query_set: None,
        //         timestamp_writes: None,
        //     });
        //
        //     render_pass.set_pipeline(&self.render_pipeline);
        //     render_pass.set_bind_group(0, &self.diffuse_bind_group, &[]);
        //     render_pass.set_bind_group(1, &self.camera_bind_group, &[]);
        //
        //     // TODO: one uniform buffer for all meshes
        //     for mesh in &self.meshes {
        //         render_pass.set_bind_group(2, &mesh.bind_group, &[]);
        //         render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        //         render_pass
        //             .set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        //
        //         render_pass.draw_indexed(0..mesh.indices, 0, 0..1);
        //     }
        // }
    }

    pub fn update_camera_keyboard(&mut self, delta_time: Duration, input: &Input) {
        let dt = delta_time.as_secs_f32();
        self.camera.update_keyboard(dt, input);
    }
    pub fn update_camera_mouse(&mut self, delta_time: Duration, input: &Input) {
        let dt = delta_time.as_secs_f32();
        self.camera.update_mouse(dt, input);
    }

    pub fn update_uniforms(&mut self) {
        if self.camera.updated() {
            self.camera_uniform.update(&self.camera);
            self.camera.reset_update();
            self.queue.write_buffer(
                &self.camera_buffer,
                0,
                bytemuck::cast_slice(&[self.camera_uniform]),
            );
        }
    }
}
