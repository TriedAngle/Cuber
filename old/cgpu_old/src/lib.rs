extern crate nalgebra as na;

use std::{mem, sync::Arc, time::Duration};

use game::raytrace::RayHit;
use game::Camera;
use game::{Diagnostics, Transform};
use mesh::{SimpleTextureMesh, TexVertex, Vertex};
use state::GPUState;
use texture::Texture;
use wgpu::util::DeviceExt;
use winit::{dpi::PhysicalSize, window::Window};

pub mod bricks;
mod buddy;
mod dense;
mod freelist;
pub mod materials;
mod mesh;
pub mod sdf;
pub mod state;
mod texture;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ModelUniform {
    pub transform: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ComputeUniforms {
    pub resolution: [f32; 2],
    pub dt: f32,
    pub render_mode: u32,
    pub brick_grid_dimension: [u32; 3],
    pub depth_boost: f32,
    pub view_projection: [[f32; 4]; 4],
    pub inverse_view_projection: [[f32; 4]; 4],
    pub camera_position: [f32; 3],
    pub brick_hit_flags: u32,
    pub brick_hit: [u32; 3], // TODO: probably merge these
    pub voxel_hit_flags: u32,
    pub voxel_hit: [u32; 3],
    pub disable_sdf: u32,
}

impl ComputeUniforms {
    pub fn new(resolution: [f32; 2], dt: f32, brick_grid_dimension: [u32; 3]) -> Self {
        Self {
            resolution,
            dt,
            render_mode: 0,
            depth_boost: 15.0,
            brick_grid_dimension,
            view_projection: *na::Matrix4::identity().as_ref(),
            inverse_view_projection: *na::Matrix4::identity().try_inverse().unwrap().as_ref(),
            camera_position: [0.; 3],
            brick_hit_flags: 0,
            brick_hit: [0; 3],
            voxel_hit_flags: 0,
            voxel_hit: [0; 3],
            disable_sdf: 0,
        }
    }
    pub fn update_camera(&mut self, camera: &Camera) {
        let view_projection = camera.view_projection_matrix();
        let inverse_view_projection = match view_projection.try_inverse() {
            Some(inv) => inv,
            None => na::Matrix4::identity().try_inverse().unwrap(),
        };

        self.camera_position = camera.position.into();
        self.view_projection = *view_projection.as_ref();
        self.inverse_view_projection = *inverse_view_projection.as_ref();
    }

    pub fn update(&mut self, resolution: [f32; 2], dt: f32) {
        self.resolution = resolution;
        self.dt = dt;
    }

    pub fn update_brick_hit(&mut self, hit: Option<RayHit>) {
        if let Some(hit) = hit {
            self.brick_hit_flags = 1;
            self.brick_hit = *hit.brick_pos.coords.as_ref();
            if let Some(voxel) = hit.voxel_local_pos {
                self.voxel_hit_flags = 1;
                self.voxel_hit = *voxel.coords.as_ref();
            } else {
                self.voxel_hit_flags = 0;
                self.voxel_hit = [0; 3];
            }
        } else {
            self.brick_hit_flags = 0;
            self.brick_hit = [0; 3];
            self.voxel_hit_flags = 0;
            self.voxel_hit = [0; 3];
        }
    }

    pub fn toggle_sdf(&mut self) {
        if self.disable_sdf == 0 {
            self.disable_sdf = 1;
        } else {
            self.disable_sdf = 0;
        }
    }
}

impl ModelUniform {
    pub fn new(matrix: &na::Matrix4<f32>) -> Self {
        Self {
            transform: *matrix.as_ref(),
        }
    }
}

pub struct RenderContext {
    pub gpu: Arc<GPUState>,

    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,

    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub surface_config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,

    pub render_pipeline: wgpu::RenderPipeline,

    pub encoder: Option<wgpu::CommandEncoder>,
    pub output_texture: Option<wgpu::SurfaceTexture>,

    pub query_count: u32,
    pub query_set: wgpu::QuerySet,
    pub query_buffer: wgpu::Buffer,
    pub query_staging_buffer: wgpu::Buffer,

    pub meshes: Vec<SimpleTextureMesh>,
    pub model_bind_group_layout: wgpu::BindGroupLayout,
    diffuse_bind_group: wgpu::BindGroup,
    depth_texture: Texture,

    pub compute_uniforms: ComputeUniforms,
    compute_uniforms_buffer: wgpu::Buffer,
    compute_bind_group_layout: wgpu::BindGroupLayout,
    compute_bind_group: wgpu::BindGroup,
    compute_pipeline: wgpu::ComputePipeline,
    compute_depth_texture_bind_group: wgpu::BindGroup,

    compute_present_bind_group_layout: wgpu::BindGroupLayout,
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
    pub async fn new(window: Arc<Window>, gpu: Arc<GPUState>) -> RenderContext {
        let size = window.inner_size();

        let device = gpu.device.clone();
        let queue = gpu.queue.clone();

        let static_window: &'static Window = unsafe { mem::transmute(&*window) };
        let surface = gpu.instance.create_surface(static_window).unwrap();

        let surface_caps = surface.get_capabilities(&gpu.adapter);

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

        let query_count = 4;
        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("Timestamp Query Set"),
            ty: wgpu::QueryType::Timestamp,
            count: query_count,
        });

        let query_buffer_size = std::mem::size_of::<u64>() as u64 * query_count as u64;
        let query_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Query Resolve Buffer"),
            size: query_buffer_size,
            usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::QUERY_RESOLVE,
            mapped_at_creation: false,
        });

        let query_staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: query_buffer_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let diffuse_bytes = include_bytes!("../../assets/happy-tree.png");
        let texture =
            Texture::from_bytes(&device, &queue, diffuse_bytes, Some("Happy Tree Texture"));

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
        model_transform.position(&na::Vector3::new(107., 40., 143.));
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
        model_transform.position(&na::Vector3::new(107., 40., 143.));
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

        let compute_uniforms = ComputeUniforms::new(
            [size.width as f32, size.height as f32],
            0.,
            *gpu.bricks.brickmap.dimensions().as_ref(),
        );

        let compute_uniforms_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Voxel Uniforms"),
                contents: bytemuck::cast_slice(&[compute_uniforms]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let diffuse_bind_group_layout =
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
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        let diffuse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &diffuse_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(texture.sampler()),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: compute_uniforms_buffer.as_entire_binding(),
                },
            ],
            label: Some("diffuse_bind_group"),
        });

        let compute_render_texture = Texture::create_storage_texture(
            &device,
            &surface_config,
            Texture::COLOR_FORMAT,
            Some("Compute Texture"),
            true,
        );

        let mut compute_depth_texture = Texture::create_storage_texture(
            &device,
            &surface_config,
            Texture::FLOAT_FORMAT,
            Some("Compute Depth Texture"),
            false,
        );

        compute_depth_texture.sampler = Some(device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Compute Depth Texture"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            compare: None,
            ..Default::default()
        }));

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
                            format: Texture::COLOR_FORMAT,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: Texture::FLOAT_FORMAT,
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
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&compute_depth_texture.view),
                },
            ],
        });

        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Compute Pipeline Layout"),
                bind_group_layouts: &[
                    &compute_bind_group_layout,
                    &gpu.materials.layout(),
                    &gpu.bricks.layout(),
                ],
                push_constant_ranges: &[],
            });

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Compute Pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &voxel_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let compute_present_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Compute Present Bind Group Layout"),
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
            label: Some("Compute Present Bind Group"),
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

        let compute_present_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Compute Present Pipeline Layout"),
                bind_group_layouts: &[&compute_present_bind_group_layout],
                push_constant_ranges: &[],
            });

        let compute_present_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Compute Present Pipeline"),
                layout: Some(&compute_present_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &compute_present_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &compute_present_shader,
                    entry_point: Some("fs_main"),
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

        let compute_depth_texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Depth Texture Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                        count: None,
                    },
                ],
            });

        let compute_depth_texture_bind_group =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Depth Texture Bind Group"),
                layout: &compute_depth_texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&compute_depth_texture.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(compute_depth_texture.sampler()),
                    },
                ],
            });
        // COMPUTE PARTS END
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[
                    &diffuse_bind_group_layout,
                    &compute_depth_texture_bind_group_layout,
                    &model_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[TexVertex::layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
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
            gpu,
            device,
            queue,
            window,
            surface,
            surface_config,
            size,

            render_pipeline,
            diffuse_bind_group,
            meshes,
            model_bind_group_layout,
            depth_texture,
            encoder: None,
            output_texture: None,
            query_count,
            query_set,
            query_buffer,
            query_staging_buffer,
            compute_uniforms,
            compute_uniforms_buffer,
            compute_bind_group_layout,
            compute_bind_group,
            compute_depth_texture_bind_group,
            compute_pipeline,
            compute_present_bind_group_layout,
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

        let compute_render_texture = Texture::create_storage_texture(
            &self.device,
            &self.surface_config,
            Texture::COLOR_FORMAT,
            Some("Compute Texture"),
            true,
        );

        let mut compute_depth_texture = Texture::create_storage_texture(
            &self.device,
            &self.surface_config,
            Texture::FLOAT_FORMAT,
            Some("Compute Depth Texture"),
            false,
        );

        compute_depth_texture.sampler =
            Some(self.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("Compute Depth Texture"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Nearest,
                min_filter: wgpu::FilterMode::Nearest,
                mipmap_filter: wgpu::FilterMode::Nearest,
                compare: None,
                ..Default::default()
            }));

        self.compute_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Compute Bind Group"),
            layout: &self.compute_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.compute_uniforms_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&compute_render_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&compute_depth_texture.view),
                },
            ],
        });

        self.compute_present_bind_group =
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Compute Present Bind Group"),
                layout: &self.compute_present_bind_group_layout,
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

    pub fn finish_render(&mut self, diagnostics: &mut Diagnostics) {
        let output = self.output_texture.take().expect("Render must be prepared");
        let mut encoder = self.encoder.take().expect("Render must be prepared");

        encoder.resolve_query_set(&self.query_set, 0..self.query_count, &self.query_buffer, 0);

        let query_buffer_size = std::mem::size_of::<u64>() as u64 * self.query_count as u64;
        encoder.copy_buffer_to_buffer(
            &self.query_buffer,
            0,
            &self.query_staging_buffer,
            0,
            query_buffer_size,
        );

        self.queue.submit([encoder.finish()]);

        output.present();

        {
            let buffer_slice = self.query_staging_buffer.slice(..);

            buffer_slice.map_async(wgpu::MapMode::Read, |_| {});

            self.queue.submit([]);
            self.device.poll(wgpu::Maintain::Wait);

            let timestamp_period = self.queue.get_timestamp_period() as f64; // in nanoseconds

            let data = buffer_slice.get_mapped_range();
            let timestamps: &[u64] = bytemuck::cast_slice(&data);

            let compute_pass_duration_ns =
                (timestamps[1] - timestamps[0]) as f64 * timestamp_period;
            let render_pass_duration_ns = (timestamps[3] - timestamps[2]) as f64 * timestamp_period;

            // Convert from nanoseconds to seconds for Duration
            let compute_duration = Duration::from_secs_f64(compute_pass_duration_ns / 1e9);
            let render_duration = Duration::from_secs_f64(render_pass_duration_ns / 1e9);

            diagnostics.insert("ComputePass", compute_duration);
            diagnostics.insert("RasterPass", render_duration);
        }
        self.query_staging_buffer.unmap();
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

        encoder.write_timestamp(&self.query_set, 0);
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
            compute_pass.set_bind_group(1, self.gpu.materials.bind_group(), &[]);
            compute_pass.set_bind_group(2, self.gpu.bricks.bind_group(), &[]);
            compute_pass.dispatch_workgroups(workgroup_x, workgroup_y, 1);
        }

        encoder.write_timestamp(&self.query_set, 1);
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Compute Present Pass"),
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

        encoder.write_timestamp(&self.query_set, 2);
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.diffuse_bind_group, &[]);
            render_pass.set_bind_group(1, &self.compute_depth_texture_bind_group, &[]);

            // TODO: one uniform buffer for all meshes
            for mesh in &self.meshes {
                render_pass.set_bind_group(2, &mesh.bind_group, &[]);
                render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                render_pass
                    .set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

                render_pass.draw_indexed(0..mesh.indices, 0, 0..1);
            }
        }
        encoder.write_timestamp(&self.query_set, 3);
    }

    pub fn update_uniforms(&mut self, camera: &Camera, delta_time: Duration) {
        if camera.updated() {
            self.compute_uniforms.update_camera(&camera);
        }

        let dt = delta_time.as_secs_f32();
        let (width, height) = (self.surface_config.width, self.surface_config.height);
        self.compute_uniforms
            .update([width as f32, height as f32], dt);
        self.compute_uniforms.brick_grid_dimension =
            *self.gpu.bricks.brickmap.dimensions().as_ref();
        self.queue.write_buffer(
            &self.compute_uniforms_buffer,
            0,
            bytemuck::cast_slice(&[self.compute_uniforms]),
        );
    }

    pub fn update_brick_hit(&mut self, hit: Option<RayHit>) {
        self.compute_uniforms.update_brick_hit(hit);
    }

    pub fn cycle_compute_render_mode(&mut self) {
        if self.compute_uniforms.render_mode == 3 {
            self.compute_uniforms.render_mode = 0;
            return;
        }

        self.compute_uniforms.render_mode += 1;
    }
}
