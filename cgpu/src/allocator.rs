use std::collections::BTreeMap;

fn round_up_pow2(mut x: u64) -> u64 {
    if x == 0 {
        return 1;
    }
    x -= 1;
    x |= x >> 1;
    x |= x >> 2;
    x |= x >> 4;
    x |= x >> 8;
    x |= x >> 16;
    x |= x >> 32;
    x + 1
}

#[derive(Debug, Clone, Copy)]
struct Block {
    offset: u64,
    size: u64,
    is_free: bool,
}

pub struct GPUBuddyBuffer {
    buffer: wgpu::Buffer,
    staging_buffer: wgpu::Buffer,
    blocks: Vec<Block>,
    free_blocks: BTreeMap<u64, Vec<usize>>,
    capacity: u64,
    min_block_size: u64,
    max_block_size: u64,
}

impl GPUBuddyBuffer {
    // this staging buffer is intended for small copies
    pub fn new(
        device: &wgpu::Device,
        block_size_limits: (u64, u64),
        initial_capacity: u64,
        staging_buffer_capacity: u64,
    ) -> Self {
        let capacity = round_up_pow2(initial_capacity);

        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Buddy Buffer"),
            size: capacity as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Buddy Buffer"),
            size: staging_buffer_capacity as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let mut buf = Self {
            buffer,
            staging_buffer,
            blocks: Vec::new(),
            free_blocks: BTreeMap::new(),
            capacity,
            min_block_size: block_size_limits.0,
            max_block_size: block_size_limits.1,
        };

        buf.blocks.push(Block {
            offset: 0,
            size: capacity,
            is_free: true,
        });

        buf.free_blocks.insert(capacity, vec![0]);

        buf
    }

    pub fn allocate(&mut self, size: u64, device: &wgpu::Device, queue: &wgpu::Queue) -> Option<u64> {
        let size = round_up_pow2(size);

        if size < self.min_block_size || size > self.max_block_size {
            return None;
        }

        if let Some(block_idx) = self.find_free_block(size) {
            self.blocks[block_idx].is_free = false;

            if let Some(free_list) = self.free_blocks.get_mut(&size) {
                free_list.retain(|&x| x != block_idx);
            }

            Some(self.blocks[block_idx].offset)
        } else {
            if self.try_grow_buffer(device, queue) {
                self.allocate(size, device, queue)
            } else {
                None
            }
        }
    }

    pub fn deallocate(&mut self, offset: u64, device: &wgpu::Device, queue: &wgpu::Queue) {
        if let Some(block_idx) = self.find_block_by_offset(offset) {
            self.blocks[block_idx].is_free = true;
            let size = self.blocks[block_idx].size;

            self.free_blocks.entry(size).or_default().push(block_idx);

            self.merge_adjacent_free_blocks();
            self.try_shrink_buffer(device, queue);
        }
    }

    pub fn write_data(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        offset: u64,
        staging_offset: u64,
        data: &[u8],
    ) {
        queue.write_buffer(&self.staging_buffer, staging_offset, data);

        encoder.copy_buffer_to_buffer(
            &self.staging_buffer,
            staging_offset,
            &self.buffer,
            offset,
            data.len() as u64,
        )
    }

    pub fn buffer(&self) -> &wgpu::Buffer { 
        &self.buffer
    }

    fn try_grow_buffer(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> bool {
        let new_capacity = self.capacity *  2;

        let new_buf = device.create_buffer(&wgpu::BufferDescriptor { 
            label: Some("Buddy Buffer"),
            size: new_capacity,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST, 
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Buddy Buffer Encoder"),
        });

        encoder.copy_buffer_to_buffer(&self.buffer, 0, &new_buf, 0, self.capacity);

        queue.submit([encoder.finish()]);

        self.buffer = new_buf;

        self.blocks.push(Block { 
            offset: self.capacity,
            size: self.capacity,
            is_free: true,
        });

        self.free_blocks
            .entry(self.capacity)
            .or_default()
            .push(self.blocks.len() - 1);

        self.capacity = new_capacity;
        return true;
    }

    fn try_shrink_buffer(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> bool {
        let total_free_size: u64 = self.free_blocks
            .iter()
            .flat_map(|(size, blocks)| std::iter::repeat(*size).take(blocks.len()))
            .sum();

        if !(((total_free_size as f32 / self.capacity as f32) < 0.75) && (self.capacity > self.max_block_size * 2)) { 
            return false;
        }


        let new_capacity = self.capacity /  2;

        let new_buf = device.create_buffer(&wgpu::BufferDescriptor { 
            label: Some("Buddy Buffer"),
            size: new_capacity,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST, 
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Buddy Buffer Encoder"),
        });

        encoder.copy_buffer_to_buffer(&self.buffer, 0, &new_buf, 0, self.capacity);

        queue.submit([encoder.finish()]);
        
        self.buffer = new_buf;
        self.capacity = new_capacity;

        self.rebuild_blocks();
        // TODO
        return true;
    }

    fn rebuild_blocks(&mut self) {
        // Keep only blocks that fit within new capacity
        self.blocks.retain(|block| block.offset + block.size <= self.capacity);
        
        // Rebuild free blocks map
        self.free_blocks.clear();
        for (idx, block) in self.blocks.iter().enumerate() {
            if block.is_free {
                self.free_blocks
                    .entry(block.size)
                    .or_default()
                    .push(idx);
            }
        }
    }

    fn merge_adjacent_free_blocks(&mut self) {}

    fn find_free_block(&mut self, size: u64) -> Option<usize> {
        if let Some(free_list) = self.free_blocks.get(&size) {
            if !free_list.is_empty() {
                return Some(free_list[0]);
            }
        }

        for (&block_size, free_list) in self.free_blocks.range_mut(size..) {
            if !free_list.is_empty() {
                let block_idx = free_list[0];
                return Some(self.split_block(block_idx, size));
            }
        }

        None
    }

    fn find_block_by_offset(&self, offset: u64) -> Option<usize> {
        self.blocks.iter().position(|block| block.offset == offset)
    }

    fn split_block(&mut self, block_idx: usize, target_size: u64) -> usize {
        let offset = self.blocks[block_idx].offset;
        let original_size = self.blocks[block_idx].size;

        let mut planned_splits = Vec::new();
        let mut current_size = original_size;

        while current_size > target_size {
            current_size /= 2;

            let new_offset = offset + current_size;
            planned_splits.push((new_offset, current_size));
        }

        self.blocks[block_idx].size = current_size;

        for (offset, size) in planned_splits {
            self.blocks.push(Block {
                offset,
                size,
                is_free: true,
            });

            self.free_blocks
                .entry(size)
                .or_default()
                .push(self.blocks.len() - 1);
        }

        block_idx
    }
}
