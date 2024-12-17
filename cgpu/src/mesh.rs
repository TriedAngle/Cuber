use std::{marker::PhantomData, mem};

use game::Transform;
use wgpu::util::DeviceExt;

use crate::ModelUniform;

pub trait Vertex: bytemuck::Pod + bytemuck::Zeroable {
    fn layout() -> wgpu::VertexBufferLayout<'static>;
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ColorVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TexVertex {
    pub position: [f32; 3],
    pub tex_coords: [f32; 2],
}

pub struct SimpleMesh<V: Vertex> {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub indices: u32,
    pub transform: Transform,
    pub uniform_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    _marker: PhantomData<V>,
}

pub type SimpleTextureMesh = SimpleMesh<TexVertex>;
#[allow(unused)]
pub type SimpleColorMesh = SimpleMesh<ColorVertex>;

impl<V: Vertex> SimpleMesh<V> {
    pub fn new(
        device: &wgpu::Device,
        transform: Transform,
        vertices: &[V],
        indices: &[u32],
        bind_group_layout: &wgpu::BindGroupLayout,
        vertex_label: Option<&str>,
        index_label: Option<&str>,
    ) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: vertex_label,
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: index_label,
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let indices = indices.len() as u32;

        let model_matrix = transform.to_homogeneous();
        let model_uniform = ModelUniform::new(&model_matrix);

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Model Uniform Buffer"),
            contents: bytemuck::bytes_of(&model_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
            label: Some("Mesh Bind Group"),
        });

        Self {
            vertex_buffer,
            index_buffer,
            indices,
            transform,
            uniform_buffer,
            bind_group,
            _marker: PhantomData,
        }
    }
}

impl Vertex for ColorVertex {
    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

impl Vertex for TexVertex {
    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}
