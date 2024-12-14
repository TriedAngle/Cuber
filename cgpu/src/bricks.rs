use std::{mem, sync::Arc};

use game::brick::{BrickHandle, BrickMap};

use crate::dense::GPUDenseBuffer;

struct BrickState { 
    brickmap: Arc<BrickMap>,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    handle_buffer: wgpu::Buffer, // fixed size
    trace_buffer: wgpu::Buffer, // fixed size
    bricks: GPUDenseBuffer,

    layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
}


// impl BrickState { 
//     pub fn new(
//         brickmap: Arc<BrickMap>,
//         device: Arc<wgpu::Device>,
//         queue: Arc<wgpu::Queue>
//     ) -> Self { 
//
//         let dims = brickmap.dimensions();
//         let count = (dims.x * dims.y * dims.z) as u64;
//
//         let handle_size = mem::size_of::<BrickHandle>() as u64 * count;
//
//         let handle_buffer = device.create_buffer(&wgpu::BufferDescriptor {
//             label: Some("Handle Buffer"),
//             size: handle_size,
//             usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
//             mapped_at_creation: false,
//         });
//
//         let trace_buffer
//
//         Self { 
//             brickmap,
//             device,
//             queue,
//
//         }
//     }
// }
