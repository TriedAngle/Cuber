use parking_lot::RwLock;
use rand::Rng;

#[repr(transparent)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BrickHandle(pub u32);

impl BrickHandle {
    const FLAG_MASK: u32 = 0xE0000000;  // 111 in top 3 bits
    const SEEN_BIT: u32 = 0x80000000;   // 1 in top bit
    const STATE_MASK: u32 = 0x60000000;  // 11 in bits 30-29
    
    const STATE_EMPTY: u32 = 0x00000000;
    const STATE_DATA: u32 = 0x20000000;     // 01 in bits 30-29
    const STATE_UNLOADED: u32 = 0x40000000; // 10 in bits 30-29
    const STATE_LOADING: u32 = 0x60000000;  // 11 in bits 30-29

    const DATA_MASK: u32 = !Self::FLAG_MASK; // Lower 29 bits


    pub fn zero() -> Self {
        Self(0)
    }

    pub fn empty() -> Self {
        Self::zero()
    }
    
    pub fn write_data(&mut self, data: u32) {
        let masked_data = data & Self::DATA_MASK;
        self.0 = (self.0 & Self::FLAG_MASK) | masked_data;
    }

    pub fn new(offset: u32) -> Self {
        Self(offset | Self::STATE_DATA)
    }

    pub fn is_empty(&self) -> bool {
        (self.0 & !Self::FLAG_MASK) == 0
    }

    pub fn is_seen(&self) -> bool {
        (self.0 & Self::SEEN_BIT) != 0
    }

    pub fn is_data(&self) -> bool {
        (self.0 & Self::STATE_MASK) == Self::STATE_DATA
    }

    pub fn is_unloaded(&self) -> bool {
        (self.0 & Self::STATE_MASK) == Self::STATE_UNLOADED
    }

    pub fn is_loading(&self) -> bool {
        (self.0 & Self::STATE_MASK) == Self::STATE_LOADING
    }

    pub fn set_seen(&mut self) {
        self.0 |= Self::SEEN_BIT;
    }

    pub fn set_unseen(&mut self) {
        self.0 &= !Self::SEEN_BIT;
    }

    pub fn set_state_empty(&mut self) {
        self.0 = (self.0 & !Self::STATE_MASK) | Self::STATE_EMPTY;
    }

    pub fn set_state_data(&mut self) {
        self.0 = (self.0 & !Self::STATE_MASK) | Self::STATE_DATA;
    }

    pub fn set_state_unloaded(&mut self) {
        self.0 = (self.0 & !Self::STATE_MASK) | Self::STATE_UNLOADED;
    }

    pub fn set_state_loading(&mut self) {
        self.0 = (self.0 & !Self::STATE_MASK) | Self::STATE_LOADING;
    }
}

impl From<u32> for BrickHandle {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

pub struct BrickMap {
    size: na::Vector3<u32>,
    handles: RwLock<Vec<BrickHandle>>,
    bricks: RwLock<Vec<TraceBrick>>,
}

impl BrickMap {
    pub fn new(size: na::Vector3<u32>) -> Self {
        let volume = size.x * size.y * size.z;
        let handles = RwLock::new(vec![BrickHandle::empty(); volume as usize]);
        let bricks = RwLock::new(vec![]);

        Self {
            size,
            handles,
            bricks,
        }
    }

    pub fn index(&self, at: na::Vector3<u32>) -> usize {
        let id = at.x + (at.y * self.size.x) + (at.z * self.size.x * self.size.y);
        id as usize
    }

    pub fn get_handle(&self, at: na::Vector3<u32>) -> BrickHandle {
        let id = self.index(at);
        let handles = self.handles.read();
        handles[id]
    }

    pub fn set_handle(&self, handle: BrickHandle, at: na::Vector3<u32>) {
        let id = self.index(at);
        let mut handles = self.handles.write();
        handles[id] = handle;
    }

    pub fn set_empty(&self, at: na::Vector3<u32>) {
        let id = self.index(at);
        let mut handles = self.handles.write();
        handles[id] = BrickHandle::empty();
    }

    pub fn is_empty(&self, at: na::Vector3<u32>) -> bool {
        let handle = self.get_handle(at);
        handle.is_empty()
    }

    pub fn bricks(&self) -> &[TraceBrick] {
        let ptr = self.bricks.data_ptr();

        let bricks = unsafe { ptr.as_ref().unwrap() };

        bricks
    }

    pub fn handles(&self) -> &[BrickHandle] {
        let ptr = self.handles.data_ptr();

        let handles = unsafe { ptr.as_ref().unwrap() };

        handles
    }

    pub fn modify_brick<F>(&self, handle: BrickHandle, modifier: F) -> Option<()>
    where
        F: FnOnce(&mut TraceBrick)
    {
        if !handle.is_data() {
            return None;
        }

        let offset = (handle.0 & BrickHandle::DATA_MASK) as usize;
        
        let mut bricks = self.bricks.write();
        
        if offset >= bricks.len() {
            return None;
        }

        modifier(&mut bricks[offset]);
        
        Some(())
    }

    pub fn dimensions(&self) -> na::Vector3<u32> {
        self.size
    }

    pub fn brick_push(&self, brick: TraceBrick) -> BrickHandle {
        let offset = self.bricks.read().len();
        let mut bricks = self.bricks.write();
        bricks.push(brick);
        BrickHandle(offset as u32)
    }

    pub fn get_or_push_brick(&self, brick: TraceBrick, at: na::Vector3<u32>) -> BrickHandle {
        let id = self.index(at);
        let mut handles = self.handles.write();
        let mut bricks = self.bricks.write();

        let handle = BrickHandle::new(bricks.len() as u32);
        bricks.push(brick);
        handles[id] = handle;
        handle
    }

    pub fn volume(&self) -> u32 {
        self.size.x * self.size.y * self.size.z
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TraceBrick {
    raw: [u8; 64],
    brick: u32, // top 3 bits for size, rest for offset
    material: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialBrick {
    // Dynamic sized type - actual implementation determined at runtime
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ExpandedBrick {
    raw: [u8; 512],
}

impl TraceBrick {
    pub const EMPTY: Self = Self::empty();
    pub const fn empty() -> Self {
        Self { raw: [0; 64], brick: 0, material: 0, }
    }

    pub fn is_empty(&self) -> bool {
        self.raw == Self::EMPTY.raw
    }

    pub fn random() -> Self {
        let mut new = Self::empty();
        rand::thread_rng().fill(&mut new.raw);
        new
    }

    pub const fn data(&self) -> &[u8] {
        &self.raw
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.raw
    }

    pub fn index(x: u32, y: u32, z: u32) -> (usize, usize) {
        let index = x + (y * 8) + (z * 64);
        let byte_index = index / 8;
        let bit_index = index % 8;
        (byte_index as usize, bit_index as usize)
    }

    pub fn set(&mut self, x: u32, y: u32, z: u32, val: bool) {
        let (byte_index, bit_index) = Self::index(x, y, z);
        if val {
            self.raw[byte_index] |= 1 << bit_index;
        } else {
            self.raw[byte_index] &= !(1 << bit_index);
        }
    }

    pub fn get(&self, x: u32, y: u32, z: u32) -> bool {
        let (byte_index, bit_index) = Self::index(x, y, z);
        self.raw[byte_index] & (1 << bit_index) != 0
    }

    pub fn toggle(&mut self, x: u32, y: u32, z: u32) {
        let (byte_index, bit_index) = Self::index(x, y, z);
        self.raw[byte_index] ^= 1 << bit_index
    }

    // Brick handle methods
    pub fn get_brick_size(&self) -> u32 {
        (self.brick >> 29) & 0b111
    }

    pub fn set_brick_size(&mut self, size: u32) {
        debug_assert!(size <= 0b111);
        self.brick = (self.brick & !(0b111 << 29)) | (size << 29);
    }

    pub fn get_brick_offset(&self) -> u32 {
        self.brick & 0x1FFFFFFF
    }

    pub fn set_brick_offset(&mut self, offset: u32) {
        debug_assert!(offset <= 0x1FFFFFFF);
        self.brick = (self.brick & (0b111 << 29)) | offset;
    }
}

impl ExpandedBrick {
    pub const EMPTY: Self = Self::empty();
    
    pub const fn empty() -> Self {
        Self { raw: [0; 512] }
    }

    pub fn random(limit: u8) -> Self {
        let mut new = Self::empty();
        let mut rng = rand::thread_rng();

        for byte in new.raw.iter_mut() { 
            *byte = rng.gen_range(0..limit);
        }
        new
    }

    pub fn is_empty(&self) -> bool {
        self.raw == Self::EMPTY.raw
    }

    pub fn data(&self) -> &[u8] {
        &self.raw
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.raw
    }

    pub fn index(x: u32, y: u32, z: u32) -> usize {
        (x + (y * 8) + (z * 64)) as usize
    }

    pub fn set(&mut self, x: u32, y: u32, z: u32, val: u8) {
        let index = Self::index(x, y, z);
        self.raw[index] = val;
    }

    pub fn get(&self, x: u32, y: u32, z: u32) -> u8 {
        let index = Self::index(x, y, z);
        self.raw[index]
    }

    pub fn to_trace_brick(&self) -> TraceBrick {
        let mut trace = TraceBrick::empty();
        
        for i in 0..512 {
            if self.raw[i] != 0 {
                let byte_index = i / 8;
                let bit_index = i % 8;
                trace.raw[byte_index] |= 1 << bit_index;
            }
        }
        
        trace
    }

    pub fn get_required_bits(&self) -> u32 {
        let mut state_mask = 0u32;
        
        for &value in self.raw.iter() {
            state_mask |= 1 << value;
        }
        
        let state_count = state_mask.count_ones();
        
        match state_count {
            0..=2 => 1,
            3..=4 => 2,
            5..=16 => 4,
            _ => 8,
        }
    }

    pub fn write_compressed(&self, buffer: &mut [u8]) -> Option<u32> {
        let bits_per_voxel = self.get_required_bits() as usize;
        let required_size = 512 * bits_per_voxel as usize / 8;

        let values_per_byte = 8 / bits_per_voxel;
        let mask = (1 << bits_per_voxel) - 1;
        
        for (i, &value) in self.raw.iter().enumerate() {
            let byte_index = i / values_per_byte;
            let bit_position = (i % values_per_byte) * bits_per_voxel;
            
            buffer[byte_index] |= (value & mask) << bit_position;
        }

        Some(required_size as u32)
    }

}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compress_brick() { 
        let brick = ExpandedBrick::random(14);
        let mut buffer: [u8; 512] = [0; 512];

        let trace = brick.to_trace_brick();
        let size = brick.write_compressed(&mut buffer).unwrap();
    }
}