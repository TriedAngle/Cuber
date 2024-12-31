use std::{mem, sync::Arc};

use game::{
    brick::{BrickMap, ExpandedBrick, MaterialBrickMeta, TraceBrick},
    material::{ExpandedMaterialMapping, MaterialId, MaterialRegistry},
    palette::PaletteRegistry,
    BrickHandle, MaterialBrick,
};
use parking_lot::Mutex;

use crate::GPUContext;

pub struct GPUBrickMap {
    pub context: Arc<GPUContext>,
    pub brickmap: cvk::Buffer,
    pub trace_bricks: cvk::GPUFreeList,
    pub bricks: cvk::GPUFreeList,
    pub materials: cvk::GPUFreeList,
    pub palettes: cvk::GPUFreeList,
    pub cpu: Arc<BrickMap>,
    pub material_registry: Arc<MaterialRegistry>,
    pub palette_registry: Arc<PaletteRegistry>,
    pub device: Arc<cvk::Device>,
    pub queue: Arc<cvk::Queue>,
    pub staging_buffers: Mutex<Vec<(cvk::Buffer, u64)>>,
}

impl GPUBrickMap {
    pub fn new(
        context: Arc<GPUContext>,
        brickmap: Arc<BrickMap>,
        palette_registry: Arc<PaletteRegistry>,
        material_registry: Arc<MaterialRegistry>,
    ) -> Self {
        let dims = brickmap.dimensions();
        let device = context.device.clone();
        let queue = context.transfer_queue.clone();

        let brickmap_buffer = Self::create_brickmap_buffer(&device, dims);
        let trace_bricks = cvk::GPUFreeList::new(
            device.clone(),
            queue.clone(),
            1024 << 20,
            cvk::SharingMode::EXCLUSIVE,
            Some("Trace brick Buffer".to_string()),
        );

        let bricks = cvk::GPUFreeList::new(
            device.clone(),
            queue.clone(),
            1024 << 20,
            cvk::SharingMode::EXCLUSIVE,
            Some("Brick Buffer".to_string()),
        );

        let materials = cvk::GPUFreeList::new(
            device.clone(),
            queue.clone(),
            512 << 20,
            cvk::SharingMode::EXCLUSIVE,
            Some("Material Buffer".to_string()),
        );

        let palettes = cvk::GPUFreeList::new(
            device.clone(),
            queue.clone(),
            512 << 20,
            cvk::SharingMode::EXCLUSIVE,
            Some("Palette Buffer".to_string()),
        );

        let new = Self {
            context,
            brickmap: brickmap_buffer,
            trace_bricks,
            bricks,
            materials,
            palettes,
            cpu: brickmap,
            material_registry,
            palette_registry,
            device,
            queue,
            staging_buffers: Mutex::new(Vec::new()),
        };

        new.rebind_brick_descriptors();

        new
    }

    fn create_brickmap_buffer(device: &cvk::Device, dims: na::Vector3<u32>) -> cvk::Buffer {
        let size = (dims.x * dims.y * dims.z) as u64 * mem::size_of::<BrickHandle>() as u64;

        device.create_buffer(&cvk::BufferInfo {
            size,
            usage: cvk::BufferUsageFlags::STORAGE_BUFFER
                | cvk::BufferUsageFlags::TRANSFER_SRC
                | cvk::BufferUsageFlags::TRANSFER_DST,
            usage_locality: cvk::MemoryUsage::AutoPreferDevice,
            allocation_locality: cvk::MemoryPropertyFlags::DEVICE_LOCAL,
            host_access: None,
            label: Some("BrickMap Buffer"),
            tag: None,
            sharing: cvk::SharingMode::EXCLUSIVE,
        })
    }

    fn create_staging_buffer(device: &cvk::Device, size: u64, label: &str) -> cvk::Buffer {
        device.create_buffer(&cvk::BufferInfo {
            size,
            sharing: cvk::SharingMode::EXCLUSIVE,
            usage: cvk::BufferUsageFlags::TRANSFER_SRC,
            usage_locality: cvk::MemoryUsage::AutoPreferHost,
            allocation_locality: cvk::MemoryPropertyFlags::HOST_VISIBLE
                | cvk::MemoryPropertyFlags::HOST_COHERENT,
            host_access: Some(cvk::AllocationCreateFlags::HOST_ACCESS_SEQUENTIAL_WRITE),
            label: Some(label),
            ..Default::default()
        })
    }

    pub fn try_drop_staging(&self) {
        let mut buffers = self.staging_buffers.lock();
        let timeline = self.queue.current_timeline(); 

        buffers.retain(|(_buffer, submit)| *submit > timeline);
    }

    pub fn transfer_all_handles(&self) {
        let dims = self.cpu.dimensions();
        let size = (dims.x * dims.y * dims.z) as u64 * mem::size_of::<BrickHandle>() as u64;
        let staging = Self::create_staging_buffer(&self.device, size, "Handles Staging Buffer");
        let handles = self.cpu.handles();
        staging.upload(bytemuck::cast_slice(handles), 0);

        let mut recorder = self.queue.record();

        recorder.copy_buffer(&staging, &self.brickmap, 0, 0, size as usize);

        let submission = self.queue.submit_express(&[recorder.finish()]).unwrap();

        let mut staging_buffers = self.staging_buffers.lock();

        staging_buffers.push((staging, submission));
    }

    pub fn transfer_all_materials(&self) {
        let data: &[u8] = bytemuck::cast_slice(self.material_registry.materials());
        let staging = Self::create_staging_buffer(&self.device, data.len() as u64, "All Materials Staging Buffer");
        staging.upload(data, 0);

        let mut recorder = self.queue.record();
        recorder.copy_buffer(&staging, self.materials.buffer(),0, 0, data.len());
        let submission = self.queue.submit_express(&[recorder.finish()]).unwrap();

        let mut staging_buffers = self.staging_buffers.lock();

        staging_buffers.push((staging, submission));
    }

    pub fn transfer_all_palettes(&self) {
        let data: &[u8] = bytemuck::cast_slice(self.palette_registry.palette_data());
        let staging = Self::create_staging_buffer(&self.device, data.len() as u64, "All Palettes Staging Buffer");
        staging.upload(data, 0);

        let mut recorder = self.queue.record();
        recorder.copy_buffer(&staging, self.materials.buffer(),0, 0, data.len());
        let submission = self.queue.submit_express(&[recorder.finish()]).unwrap();

        let mut staging_buffers = self.staging_buffers.lock();

        staging_buffers.push((staging, submission));
    }

    pub fn prepare_transfer_handle(
        &self,
        handle: BrickHandle,
        at: na::Point3<u32>,
    ) -> (Vec<BrickHandle>, u64) {
        let target_index = self.cpu.index(at) as u64;
        let byte_offset = target_index * mem::size_of::<BrickHandle>() as u64;
        let aligned_offset = byte_offset & !(32 - 1);
        let start_handle_index = aligned_offset / mem::size_of::<BrickHandle>() as u64;

        let dims = self.cpu.dimensions().cast::<u64>();
        let total_elements = dims.x * dims.y * dims.z;

        // Calculate how many handles we need to write, but ensure we don't exceed the buffer
        let handles_to_write =
            (32 + byte_offset - aligned_offset) / mem::size_of::<BrickHandle>() as u64;
        let handles_to_write = handles_to_write.min((total_elements - start_handle_index) as u64);

        let mut handles = Vec::with_capacity(handles_to_write as usize);

        for i in 0..handles_to_write {
            let current_index = start_handle_index + i;
            if current_index == target_index {
                handles.push(handle);
            } else {
                let x = current_index % dims.x;
                let y = (current_index / dims.x) % dims.y;
                let z = current_index / (dims.x * dims.y);
                let pos = na::Point3::new(x, y, z);
                let existing_handle = self.cpu.get_handle(pos.cast::<u32>());
                handles.push(existing_handle);
            }
        }

        (handles, aligned_offset)
    }

    pub fn setup_full_brick(
        &self,
        at: na::Point3<u32>,
        expanded_brick: Option<&ExpandedBrick>,
        material: Option<MaterialId>,
        material_mapping: &ExpandedMaterialMapping,
    ) {
        let mut recorder = self.queue.record();
        let Some(expanded_brick) = expanded_brick else {
            let handle = if let Some(material) = material {
                let mut handle = BrickHandle::empty();
                handle.set_lod(true);
                handle.set_empty_value(material.0);
                handle
            } else {
                BrickHandle::empty()
            };
            self.cpu.set_handle(handle, at);
            let (aligned_handles, aligned_handle_offset) = self.prepare_transfer_handle(handle, at);
            let aligned_handles_size = aligned_handles.len() * mem::size_of::<BrickHandle>();
            let staging_buffer = Self::create_staging_buffer(
                &self.device,
                aligned_handles_size as u64,
                "Handles Staging Buffer",
            );
            staging_buffer.upload(bytemuck::cast_slice(&aligned_handles), 0);
            recorder.copy_buffer(
                &staging_buffer,
                &self.brickmap,
                0,
                aligned_handle_offset as usize,
                aligned_handles_size,
            );
            let submit = self.queue.submit_express(&[recorder.finish()]).unwrap();
            let mut staging_buffers = self.staging_buffers.lock();
            staging_buffers.push((staging_buffer, submit));
            return;
        };

        let mut trace_brick = expanded_brick.to_trace_brick();
        let trace_brick_size = mem::size_of::<TraceBrick>();

        let (mut material_brick, materials) = expanded_brick.compress(material_mapping);

        let palette_id = self.palette_registry.register_palette(materials);
        material_brick.set_meta_value(palette_id.0);

        let material_brick_data = material_brick.data();

        let material_brick_size = material_brick.size();
        let material_brick_offset = self
            .bricks
            .allocate_size(material_brick_size as u64, |old, _new, submit| {
                let mut staging = self.staging_buffers.lock();
                staging.push((old, submit));
                self.rebind_brick_descriptors();
            })
            .unwrap();

        trace_brick.set_brick_offset(material_brick_offset as u32);

        let (handle, needs_alloc) = self.cpu.set_brick(trace_brick, at);

        if needs_alloc {
            self.trace_bricks
                .allocate::<TraceBrick, _>(|old, _new, submit| {
                    let mut staging = self.staging_buffers.lock();
                    staging.push((old, submit));
                    self.rebind_brick_descriptors();
                })
                .unwrap();
        }

        let trace_brick_offset_sized = handle.get_data_value();
        let trace_brick_offset = trace_brick_offset_sized * mem::size_of::<TraceBrick>() as u32;

        let (aligned_handles, aligned_handle_offset) = self.prepare_transfer_handle(handle, at);
        let aligned_handles_size = mem::size_of::<BrickHandle>() * aligned_handles.len();

        let required_size = material_brick_size + trace_brick_size + aligned_handles_size;

        let staging_buffer = Self::create_staging_buffer(
            &self.device,
            required_size as u64,
            "Full Brick Staging Buffer",
        );
        staging_buffer.upload(bytemuck::cast_slice(&[material_brick.meta()]), 0);
        staging_buffer.upload(material_brick_data, mem::size_of::<u32>());
        staging_buffer.upload(bytemuck::cast_slice(&[trace_brick]), material_brick_size);
        staging_buffer.upload(
            bytemuck::cast_slice(&aligned_handles),
            material_brick_size + trace_brick_size,
        );

        recorder.copy_buffer(
            &staging_buffer,
            self.bricks.buffer(),
            0,
            material_brick_offset as usize,
            material_brick_size,
        );
        recorder.copy_buffer(
            &staging_buffer,
            self.trace_bricks.buffer(),
            material_brick_size,
            trace_brick_offset as usize,
            trace_brick_size,
        );
        recorder.copy_buffer(
            &staging_buffer,
            &self.brickmap,
            material_brick_size + trace_brick_size,
            aligned_handle_offset as usize,
            aligned_handles_size,
        );

        let submit = self.queue.submit_express(&[recorder.finish()]).unwrap();
        self.staging_buffers.lock().push((staging_buffer, submit));
    }

    pub fn rebind_brick_descriptors(&self) {
        self.queue.wait_idle();
        self.context.render_queue.wait_idle();
        self.device.wait_idle();
        let _lock_queue = self.queue.lock();
        let _lock_render = self.context.render_queue.lock();

        self.context.descriptors.write(&[
            cvk::DescriptorWrite::StorageBuffer {
                binding: 0,
                buffer: &self.brickmap,
                offset: 0,
                range: self.brickmap.size,
                array_element: None,
            },
            cvk::DescriptorWrite::StorageBuffer {
                binding: 1,
                buffer: &self.trace_bricks.buffer(),
                offset: 0,
                range: self.trace_bricks.buffer().size,
                array_element: None,
            },
            cvk::DescriptorWrite::StorageBuffer {
                binding: 2,
                buffer: &self.bricks.buffer(),
                offset: 0,
                range: self.bricks.buffer().size,
                array_element: None,
            },
        ])
    }

    pub fn rebind_material_descriptors(&self) {
        self.context.descriptors.write(&[
            cvk::DescriptorWrite::StorageBuffer {
                binding: 3,
                buffer: &self.materials.buffer(),
                offset: 0,
                range: self.materials.buffer().size,
                array_element: None,
            },
            cvk::DescriptorWrite::StorageBuffer {
                binding: 4,
                buffer: &self.palettes.buffer(),
                offset: 0,
                range: self.palettes.buffer().size,
                array_element: None,
            },
        ]);
    }
}
