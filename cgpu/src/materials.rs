use std::sync::Arc;

use game::{
    material::{MaterialRegistry, PbrMaterial},
    palette::PaletteRegistry,
};
use parking_lot::RwLock;

use crate::{dense::GPUDenseBuffer, freelist::GPUFreeListBuffer};

pub struct MaterialState {
    palettes: Arc<PaletteRegistry>,
    materials: Arc<MaterialRegistry>,
    device: Arc<wgpu::Device>,
    #[allow(unused)]
    queue: Arc<wgpu::Queue>,
    palette_buffer: GPUDenseBuffer,
    material_buffer: GPUFreeListBuffer<PbrMaterial>,
    layout: wgpu::BindGroupLayout,
    bind_group: RwLock<wgpu::BindGroup>,
}

impl MaterialState {
    pub fn new(
        palettes: Arc<PaletteRegistry>,
        materials: Arc<MaterialRegistry>,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        initial_capacity: u64,
    ) -> Self {
        let palette_buffer = GPUDenseBuffer::new(device.clone(), queue.clone(), initial_capacity);

        let material_buffer = GPUFreeListBuffer::new(
            device.clone(),
            queue.clone(),
            materials.materials().len() as u64,
            wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
        );

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Material Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let bind_group = Self::create_bind_group(
            &device,
            &layout,
            palette_buffer.buffer(),
            material_buffer.buffer(),
        );

        Self {
            palettes,
            materials,
            device,
            queue,
            palette_buffer,
            material_buffer,
            layout,
            bind_group: RwLock::new(bind_group),
        }
    }

    pub fn update_all_materials(&self) {
        self.material_buffer.clear();
        self.material_buffer
            .allocate_write_many(self.materials.materials(), |_buffer| {
                self.recreate_bind_group();
            });
    }

    pub fn update_all_palettes(&self) {
        self.palette_buffer.clear();
        self.palette_buffer
            .allocate_and_write_dense(&self.palettes.palette_data(), |_buffer| {
                self.recreate_bind_group();
            });
    }

    pub fn recreate_bind_group(&self) {
        let new = Self::create_bind_group(
            &self.device,
            &self.layout,
            self.palette_buffer.buffer(),
            self.material_buffer.buffer(),
        );

        let mut bind_group = self.bind_group.write();
        *bind_group = new;
    }

    pub fn create_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        palette_buffer: &wgpu::Buffer,
        material_buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Brickmap Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: palette_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: material_buffer.as_entire_binding(),
                },
            ],
        })
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        unsafe { self.bind_group.data_ptr().as_ref().unwrap() }
    }

    pub fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }
}
