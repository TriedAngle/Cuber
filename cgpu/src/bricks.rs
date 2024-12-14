use std::{mem, sync::Arc};

use game::{
    brick::{BrickHandle, BrickMap, MaterialBrick, TraceBrick},
    palette::PaletteId,
};
use parking_lot::RwLock;
use wgpu::util::DeviceExt;

use crate::{dense::GPUDenseBuffer, freelist::GPUFreeListBuffer};

pub struct BrickState {
    brickmap: Arc<BrickMap>,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    handle_buffer: RwLock<wgpu::Buffer>,
    trace_buffer: GPUFreeListBuffer<TraceBrick>,
    brick_buffer: GPUDenseBuffer,

    layout: wgpu::BindGroupLayout,
    bind_group: RwLock<wgpu::BindGroup>,
}

impl BrickState {
    pub fn new(
        brickmap: Arc<BrickMap>,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        initial_capacity: u64,
    ) -> Self {
        let dims = brickmap.dimensions();
        let count = (dims.x * dims.y * dims.z) as u64;

        let handle_size = mem::size_of::<BrickHandle>() as u64 * count;

        let handle_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Handle Buffer"),
            size: handle_size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let dims = brickmap.dimensions();
        let count = dims.x * dims.y * dims.z;
        let count = count as u64;

        let trace_buffer = GPUFreeListBuffer::new(
            device.clone(),
            queue.clone(),
            count,
            wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
        );

        let brick_buffer = GPUDenseBuffer::new(device.clone(), queue.clone(), initial_capacity);

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Brickmap Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
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
            &handle_buffer,
            trace_buffer.buffer(),
            brick_buffer.buffer(),
        );

        Self {
            brickmap,
            device,
            queue,
            handle_buffer: RwLock::new(handle_buffer),
            trace_buffer,
            brick_buffer,
            layout,
            bind_group: RwLock::new(bind_group),
        }
    }

    pub fn update_all_handles(&self) {
        {
            let mut handles = self.handle_buffer.write();

            let new = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Brick Handle Buffer"),
                    contents: bytemuck::cast_slice(&self.brickmap.handles()),
                    usage: wgpu::BufferUsages::STORAGE
                        | wgpu::BufferUsages::COPY_SRC
                        | wgpu::BufferUsages::COPY_DST,
                });

            *handles = new;
        }
        self.recreate_bind_group();
    }

    pub fn update_all_bricks(&self) {
        self.trace_buffer.clear();
        self.trace_buffer
            .allocate_write_many(self.brickmap.bricks(), |_buffer| {
                self.recreate_bind_group();
            });
    }

    pub fn allocate_brick(
        &self,
        brick: MaterialBrick,
        handle: BrickHandle,
        palette: PaletteId,
    ) -> Option<u64> {
        let size = brick.element_size();

        let offset = self.brick_buffer.allocate_brick(brick, |_buffer| {
            self.recreate_bind_group();
        })?;

        let _ = self.brickmap.modify_brick(handle, |trace| {
            trace.set_brick_info(size as u32 - 1, offset as u32);
            trace.set_palette(palette);
        });

        Some(offset)
    }

    pub fn recreate_bind_group(&self) {
        let mut bind_group = self.bind_group.write();

        let new = Self::create_bind_group(
            &self.device,
            &self.layout,
            &self.handle_buffer.read(),
            self.trace_buffer.buffer(),
            self.brick_buffer.buffer(),
        );

        *bind_group = new;
    }

    pub fn create_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        handle_buffer: &wgpu::Buffer,
        trace_buffer: &wgpu::Buffer,
        brick_buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Brickmap Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: handle_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: trace_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: brick_buffer.as_entire_binding(),
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
