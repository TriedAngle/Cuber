use std::sync::Arc;

use game::brick::BrickMap;

use crate::dense::GPUDenseBuffer;

struct BrickState { 
    brickmap: Arc<BrickMap>,
    handle_buffer: wgpu::Buffer, // fixed size
    trace_buffer: wgpu::Buffer, // fixed size
    bricks: GPUDenseBuffer,
}