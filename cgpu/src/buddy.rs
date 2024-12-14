#![allow(unused)]
use std::collections::BTreeMap;

use game::brick::MaterialBrick;
use parking_lot::RwLock;

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

struct InnerState {
    blocks: Vec<Block>,
    free_blocks: BTreeMap<u64, Vec<usize>>,
    capacity: u64,
}

pub struct GPUBuddyBuffer {
    buffer: RwLock<wgpu::Buffer>,
    state: RwLock<InnerState>,
    min_block_size: u64,
    max_block_size: u64,
}

impl GPUBuddyBuffer {
    // this staging buffer is intended for small copies
    pub fn new(
        device: &wgpu::Device,
        block_size_limits: (u64, u64),
        initial_capacity: u64,
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

        let mut initial_free_blocks = BTreeMap::new();
        initial_free_blocks.insert(capacity, vec![0]);

        let state = InnerState {
            blocks: vec![Block {
                offset: 0,
                size: capacity,
                is_free: true,
            }],
            free_blocks: initial_free_blocks,
            capacity,
        };

        Self {
            buffer: RwLock::new(buffer),
            state: RwLock::new(state),
            min_block_size: block_size_limits.0,
            max_block_size: block_size_limits.1,
        }
    }

    pub fn allocate<T, F>(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        on_buffer_update: F,
    ) -> Option<u64>
    where
        F: Fn(&wgpu::Buffer),
    {
        let type_size = std::mem::size_of::<T>() as u64;
        // let size = round_up_pow2(tylpe_size);
        let size = type_size;

        if size < self.min_block_size || self.max_block_size < size {
            return None;
        }

        let mut state = self.state.write();

        if let Some(block_idx) = self.find_free_block(&mut state, size) {
            state.blocks[block_idx].is_free = false;

            if let Some(free_list) = state.free_blocks.get_mut(&size) {
                free_list.retain(|&x| x != block_idx);
            }

            Some(state.blocks[block_idx].offset)
        } else {
            if self.try_grow_buffer(&mut state, device, queue, on_buffer_update) {
                // Retry allocation after growing
                self.find_free_block(&mut state, size).map(|block_idx| {
                    state.blocks[block_idx].is_free = false;
                    if let Some(free_list) = state.free_blocks.get_mut(&size) {
                        free_list.retain(|&x| x != block_idx);
                    }
                    state.blocks[block_idx].offset
                })
            } else {
                None
            }
        }
    }

    pub fn deallocate<F>(
        &self,
        offset: u64,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        on_buffer_update: F,
    ) where
        F: Fn(&wgpu::Buffer),
    {
        let mut state = self.state.write();

        if let Some(block_idx) = self.find_block_by_offset(&state, offset) {
            state.blocks[block_idx].is_free = true;
            let size = state.blocks[block_idx].size;

            state.free_blocks.entry(size).or_default().push(block_idx);

            self.merge_adjacent_free_blocks(&mut state);
            self.try_shrink_buffer(&mut state, device, queue, on_buffer_update);
        }
    }

    pub fn write<T: bytemuck::Pod>(&self, queue: &wgpu::Queue, offset: u64, data: &T) {
        queue.write_buffer(
            &self.buffer.read(),
            offset,
            bytemuck::cast_slice(std::slice::from_ref(data)),
        );
        queue.submit([]);
    }

    pub fn allocate_and_write<T: bytemuck::Pod, F>(
        &self,
        data: &T,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        on_buffer_update: F,
    ) -> Option<u64>
    where
        F: Fn(&wgpu::Buffer),
    {
        let offset = self.allocate::<T, F>(device, queue, on_buffer_update)?;
        self.write(queue, offset, data);
        Some(offset)
    }

    pub fn buffer(&self) -> &wgpu::Buffer {
        unsafe { &self.buffer.data_ptr().as_ref().unwrap() }
    }

    fn try_grow_buffer<F>(
        &self,
        state: &mut InnerState,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        on_buffer_update: F,
    ) -> bool
    where
        F: Fn(&wgpu::Buffer),
    {
        let new_capacity = state.capacity * 2;
        log::debug!(
            "Growing Brick Buffer {} -> {}",
            state.capacity,
            new_capacity
        );

        let new_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Buddy Buffer"),
            size: new_capacity,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Buddy Buffer Encoder"),
        });

        let mut buffer = self.buffer.write();

        encoder.copy_buffer_to_buffer(&buffer, 0, &new_buf, 0, state.capacity);

        queue.submit([encoder.finish()]);
        device.poll(wgpu::Maintain::Wait);

        on_buffer_update(&buffer);

        *buffer = new_buf;

        state.blocks.push(Block {
            offset: state.capacity,
            size: state.capacity,
            is_free: true,
        });

        state
            .free_blocks
            .entry(state.capacity)
            .or_default()
            .push(state.blocks.len() - 1);

        state.capacity = new_capacity;
        true
    }

    fn try_shrink_buffer<F>(
        &self,
        state: &mut InnerState,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        on_buffer_update: F,
    ) -> bool
    where
        F: Fn(&wgpu::Buffer),
    {
        let total_free_size: u64 = state
            .free_blocks
            .iter()
            .flat_map(|(size, blocks)| std::iter::repeat(*size).take(blocks.len()))
            .sum();

        if !(((total_free_size as f32 / state.capacity as f32) < 0.75)
            && (state.capacity > self.max_block_size * 2))
        {
            return false;
        }

        let new_capacity = state.capacity / 2;

        let new_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Buddy Buffer"),
            size: new_capacity,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Buddy Buffer Encoder"),
        });

        let mut buffer = self.buffer.write();

        encoder.copy_buffer_to_buffer(&buffer, 0, &new_buf, 0, new_capacity);

        *buffer = new_buf;

        queue.submit([encoder.finish()]);

        on_buffer_update(&buffer);

        state.capacity = new_capacity;

        self.rebuild_blocks(state);
        true
    }

    fn rebuild_blocks(&self, state: &mut InnerState) {
        state
            .blocks
            .retain(|block| block.offset + block.size <= state.capacity);

        state.free_blocks.clear();
        for (idx, block) in state.blocks.iter().enumerate() {
            if block.is_free {
                state.free_blocks.entry(block.size).or_default().push(idx);
            }
        }
    }

    fn merge_adjacent_free_blocks(&self, state: &mut InnerState) {
        let mut i = 0;
        while i < state.blocks.len() {
            if !state.blocks[i].is_free {
                i += 1;
                continue;
            }

            let buddy_offset = self.find_buddy_offset(state, i);
            if let Some(buddy_idx) = self.find_block_by_offset(state, buddy_offset) {
                if state.blocks[buddy_idx].is_free
                    && state.blocks[buddy_idx].size == state.blocks[i].size
                {
                    // Merge the blocks
                    let new_size = state.blocks[i].size * 2;
                    state.blocks[i].size = new_size;
                    state.blocks.remove(buddy_idx);

                    // Update free blocks tracking
                    self.update_free_blocks_after_merge(state, i, buddy_idx, new_size);
                    continue;
                }
            }
            i += 1;
        }
    }

    fn find_buddy_offset(&self, state: &InnerState, block_idx: usize) -> u64 {
        let block = &state.blocks[block_idx];
        block.offset ^ block.size
    }

    fn update_free_blocks_after_merge(
        &self,
        state: &mut InnerState,
        remaining_idx: usize,
        removed_idx: usize,
        new_size: u64,
    ) {
        // Remove both original blocks from their free list
        let old_size = new_size / 2;
        if let Some(free_list) = state.free_blocks.get_mut(&old_size) {
            free_list.retain(|&x| x != remaining_idx && x != removed_idx);
        }

        // Add the merged block to the new size's free list
        state
            .free_blocks
            .entry(new_size)
            .or_default()
            .push(remaining_idx);
    }

    fn find_free_block(&self, state: &mut InnerState, size: u64) -> Option<usize> {
        if let Some(free_list) = state.free_blocks.get(&size) {
            if !free_list.is_empty() {
                return Some(free_list[0]);
            }
        }

        for (&_block_size, free_list) in state.free_blocks.range_mut(size..) {
            if !free_list.is_empty() {
                let block_idx = free_list[0];
                return Some(self.split_block(state, block_idx, size));
            }
        }

        None
    }

    fn find_block_by_offset(&self, state: &InnerState, offset: u64) -> Option<usize> {
        state.blocks.iter().position(|block| block.offset == offset)
    }

    fn split_block(&self, state: &mut InnerState, block_idx: usize, target_size: u64) -> usize {
        let offset = state.blocks[block_idx].offset;
        let original_size = state.blocks[block_idx].size;

        let mut planned_splits = Vec::new();
        let mut current_size = original_size;

        while current_size > target_size {
            current_size /= 2;
            let new_offset = offset + current_size;
            planned_splits.push((new_offset, current_size));
        }

        state.blocks[block_idx].size = current_size;

        for (offset, size) in planned_splits {
            state.blocks.push(Block {
                offset,
                size,
                is_free: true,
            });

            state
                .free_blocks
                .entry(size)
                .or_default()
                .push(state.blocks.len() - 1);
        }

        block_idx
    }

    pub fn allocate_brick<F>(
        &self,
        brick: MaterialBrick,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        on_buffer_update: F,
    ) -> Option<u64>
    where
        F: Fn(&wgpu::Buffer),
    {
        match brick {
            MaterialBrick::Size1(b) => self.allocate_and_write(&b, device, queue, on_buffer_update),
            MaterialBrick::Size2(b) => self.allocate_and_write(&b, device, queue, on_buffer_update),
            MaterialBrick::Size4(b) => self.allocate_and_write(&b, device, queue, on_buffer_update),
            MaterialBrick::Size8(b) => self.allocate_and_write(&b, device, queue, on_buffer_update),
        }
    }
}
