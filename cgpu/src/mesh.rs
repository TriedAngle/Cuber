use game::Transform;

pub struct Mesh { 
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub indices: u32,
    pub transform: Transform,
}

