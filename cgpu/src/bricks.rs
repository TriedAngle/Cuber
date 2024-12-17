use std::{mem, sync::Arc};

use game::{
    brick::{BrickHandle, BrickMap, MaterialBrick, TraceBrick},
    palette::PaletteId,
};
use parking_lot::RwLock;
use wgpu::util::DeviceExt;

use crate::{dense::GPUDenseBuffer, freelist::GPUFreeListBuffer};

pub struct BrickState {
    pub brickmap: Arc<BrickMap>,
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
        let count = count.min(3728268) as u64;

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

    pub fn update_all_material_bricks(&self) {
        self.brick_buffer.clear();
        self.brick_buffer
            .reset_copy_from_cpu(self.brickmap.material_bricks());
    }

    pub fn transfer_handle(&self, handle: BrickHandle, at: na::Point3<u32>) {
        let target_index = self.brickmap.index(at) as u64;
        
        let byte_offset = target_index * mem::size_of::<BrickHandle>() as u64;
        
        let aligned_offset = byte_offset & !(32 - 1);
        
        let start_handle_index = aligned_offset / std::mem::size_of::<BrickHandle>() as u64;
        let handles_to_write = (32 + byte_offset - aligned_offset) / std::mem::size_of::<BrickHandle>() as u64;
        
        let mut handles = Vec::with_capacity(handles_to_write as usize);
        
        let dims = self.brickmap.dimensions().cast::<u64>();
        for i in 0..handles_to_write {
            let current_index = start_handle_index + i;
            
            if current_index == target_index {
                handles.push(handle);
            } else {
                let x = current_index % dims.x;
                let y = (current_index / dims.x) % dims.y;
                let z = current_index / (dims.x * dims.y);
                let pos = na::Point3::new(x, y, z);
                
                let existing_handle = self.brickmap.get_handle(pos.cast::<u32>());
                handles.push(existing_handle);
            }
        }
        
        self.queue.write_buffer(
            &self.handle_buffer.write(),
            aligned_offset,
            bytemuck::cast_slice(&handles)
        );
    }


    pub fn transfer_brick(&self, handle: BrickHandle) {
        if handle.is_data() {
            let brick = self.brickmap.get_brick(handle).unwrap();
            let offset = brick.get_brick_offset();
            let bits_per_elment = brick.get_brick_size();
            let total_size = (bits_per_elment + 1) * 512;
            let _ = self.trace_buffer.write(handle.data() as u64, &brick);
            self.brick_buffer.copy_from_cpu(
                self.brickmap.material_bricks(),
                offset as usize,
                total_size as usize,
                offset as u64,
            );
        }
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

    pub fn allocate_bricks(
        &self,
        bricks: &[MaterialBrick],
        handles: &[BrickHandle],
        palettes: &[PaletteId],
    ) -> Option<Vec<(u64, u64)>> {
        let offsets = self.brick_buffer.allocate_bricks(bricks, |_buffer| {
            self.recreate_bind_group();
        })?;

        offsets.iter().zip(handles).zip(palettes).for_each(
            |((&(offset, size), &handle), &palette)| {
                let _ = self.brickmap.modify_brick(handle, |trace| {
                    trace.set_brick_info(size as u32 - 1, offset as u32);
                    trace.set_palette(palette);
                });
            },
        );

        Some(offsets)
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

    pub fn submit(&self) -> wgpu::SubmissionIndex {
        self.queue.submit([])
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        unsafe { self.bind_group.data_ptr().as_ref().unwrap() }
    }

    pub fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }
}
