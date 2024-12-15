use parking_lot::RwLock;
use rand::Rng;

use crate::{
    material::{ExpandedMaterialMapping, MaterialId},
    palette::PaletteId,
};

#[repr(transparent)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BrickHandle(pub u32);

impl BrickHandle {
    pub const FLAG_MASK: u32 = 0xE0000000; // 111 in top 3 bits
    const SEEN_BIT: u32 = 0x80000000; // 1 in top bit
    const STATE_MASK: u32 = 0x60000000; // 11 in bits 30-29

    const STATE_EMPTY: u32 = 0x00000000; // x00
    const STATE_DATA: u32 = 0x20000000; // x01
    const STATE_LOADING: u32 = 0x40000000; // x10
    const STATE_LOD: u32 = 0x60000000; // x11

    const DATA_MASK: u32 = !Self::FLAG_MASK; // Lower 29 bits

    pub fn zero() -> Self {
        Self(0)
    }

    pub fn empty() -> Self {
        Self::zero()
    }

    pub fn write_data(&mut self, data: u32) {
        let seen_bits = self.0 & Self::SEEN_BIT;
        let masked_data = data & Self::DATA_MASK;
        self.0 = masked_data | Self::STATE_DATA | seen_bits;
    }

    pub fn write_sdf(&mut self, data: u32) {
        let seen_bits = self.0 & Self::SEEN_BIT;
        let masked_data = data & Self::DATA_MASK;
        self.0 = masked_data | Self::STATE_EMPTY | seen_bits;
    }

    pub fn set_loading(&mut self) {
        let seen_bits = self.0 & Self::SEEN_BIT;
        let data_bits = self.0 & Self::DATA_MASK;
        self.0 = data_bits | Self::STATE_LOADING | seen_bits;
    }

    pub fn write_lod(&mut self, material: MaterialId) {
        let seen_bits = self.0 & Self::SEEN_BIT;
        let masked_data = material.0 & Self::DATA_MASK;
        self.0 = masked_data | Self::STATE_LOD | seen_bits;
    }

    pub fn mark_seen(&mut self) {
        self.0 |= Self::SEEN_BIT;
    }

    pub fn mark_unseen(&mut self) {
        self.0 &= !Self::SEEN_BIT;
    }

    pub fn new(offset: u32) -> Self {
        Self(offset | Self::STATE_DATA) // Starts as unseen (0xx)
    }

    pub fn is_empty(&self) -> bool {
        (self.0 & Self::STATE_MASK) == Self::STATE_EMPTY
    }

    pub fn is_seen(&self) -> bool {
        (self.0 & Self::SEEN_BIT) != 0
    }

    pub fn is_data(&self) -> bool {
        (self.0 & Self::STATE_MASK) == Self::STATE_DATA
    }

    pub fn is_loading(&self) -> bool {
        (self.0 & Self::STATE_MASK) == Self::STATE_LOADING
    }

    pub fn is_lod(&self) -> bool {
        (self.0 & Self::STATE_MASK) == Self::STATE_LOD
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

    pub fn index(&self, at: na::Point3<u32>) -> usize {
        let id = at.x + (at.y * self.size.x) + (at.z * self.size.x * self.size.y);
        id as usize
    }

    pub fn get_handle(&self, at: na::Point3<u32>) -> BrickHandle {
        let id = self.index(at);
        let handles = self.handles.read();
        handles[id]
    }

    pub fn set_handle(&self, handle: BrickHandle, at: na::Point3<u32>) {
        let id = self.index(at);
        let mut handles = self.handles.write();
        handles[id] = handle;
    }

    pub fn set_empty(&self, at: na::Point3<u32>) {
        let id = self.index(at);
        let mut handles = self.handles.write();
        handles[id] = BrickHandle::empty();
    }

    pub fn is_empty(&self, at: na::Point3<u32>) -> bool {
        let handle = self.get_handle(at);
        handle.is_empty()
    }

    pub fn bricks(&self) -> &[TraceBrick] {
        let ptr = self.bricks.data_ptr();

        let bricks = unsafe { ptr.as_ref().unwrap() };

        bricks
    }

    pub fn as_ptr(&self) -> *mut BrickHandle {
        self.handles.data_ptr() as *mut _
    }

    pub fn handles(&self) -> &[BrickHandle] {
        let ptr = self.handles.data_ptr();

        let handles = unsafe { ptr.as_ref().unwrap() };

        handles
    }

    pub fn get_brick(&self, handle: BrickHandle) -> Option<TraceBrick> {
        if !handle.is_data() {
            return None;
        }
        let bricks = self.bricks.read();

        let offset = (handle.0 & BrickHandle::DATA_MASK) as usize;

        if offset >= bricks.len() {
            return None;
        }

        Some(bricks[offset])
    }

    pub fn modify_brick<F>(&self, handle: BrickHandle, modifier: F) -> Option<()>
    where
        F: FnOnce(&mut TraceBrick),
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

    pub fn get_or_push_brick(&self, brick: TraceBrick, at: na::Point3<u32>) -> BrickHandle {
        let mut bricks = self.bricks.write();

        let handle = BrickHandle::new(bricks.len() as u32);
        bricks.push(brick);
        self.set_handle(handle, at);
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
    palette: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialBrick1 {
    pub raw: [u32; 16], // 64 bytes = 16 u32s (1 bit per value)
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialBrick2 {
    pub raw: [u32; 32], // 128 bytes = 32 u32s (2 bits per value)
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialBrick4 {
    pub raw: [u32; 64], // 256 bytes = 64 u32s (4 bits per value)
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialBrick8 {
    pub raw: [u32; 128], // 512 bytes = 128 u32s (8 bits per value)
}

impl MaterialBrick1 {
    pub fn from_expanded_brick(expanded: &ExpandedBrick) -> Self {
        let mut brick = Self { raw: [0; 16] };
        Self::pack_values(expanded, &mut brick.raw);
        brick
    }
}

impl MaterialBrick2 {
    pub fn from_expanded_brick(expanded: &ExpandedBrick) -> Self {
        let mut brick = Self { raw: [0; 32] };
        Self::pack_values(expanded, &mut brick.raw);
        brick
    }
}

impl MaterialBrick4 {
    pub fn from_expanded_brick(expanded: &ExpandedBrick) -> Self {
        let mut brick = Self { raw: [0; 64] };
        Self::pack_values(expanded, &mut brick.raw);
        brick
    }
}

impl MaterialBrick8 {
    pub fn from_expanded_brick(expanded: &ExpandedBrick) -> Self {
        let mut brick = Self { raw: [0; 128] };
        Self::pack_values(expanded, &mut brick.raw);
        brick
    }
}

// Common trait for shared functionality
trait MaterialBrickOps {
    const BITS_PER_VALUE: usize;
    const MASK: u32;

    fn pack_values(expanded: &ExpandedBrick, raw: &mut [u32]) {
        for i in 0..512 {
            let value = expanded.raw[i] as u32;
            let values_per_u32 = 32 / Self::BITS_PER_VALUE;
            let word_index = i / values_per_u32;
            let shift = (i % values_per_u32) * Self::BITS_PER_VALUE;

            raw[word_index] |= (value & Self::MASK) << shift;
        }
    }

    fn get_value(raw: &[u32], x: u32, y: u32, z: u32) -> u8 {
        let index = (x + y * 8 + z * 64) as usize;
        let values_per_u32 = 32 / Self::BITS_PER_VALUE;
        let word_index = index / values_per_u32;
        let shift = (index % values_per_u32) * Self::BITS_PER_VALUE;

        ((raw[word_index] >> shift) & Self::MASK) as u8
    }

    fn set_value(raw: &mut [u32], x: u32, y: u32, z: u32, val: u8) {
        let index = (x + y * 8 + z * 64) as usize;
        let values_per_u32 = 32 / Self::BITS_PER_VALUE;
        let word_index = index / values_per_u32;
        let shift = (index % values_per_u32) * Self::BITS_PER_VALUE;

        let mask = Self::MASK << shift;
        raw[word_index] = (raw[word_index] & !mask) | (((val as u32) & Self::MASK) << shift);
    }
}

// Implement for each brick type
impl MaterialBrickOps for MaterialBrick1 {
    const BITS_PER_VALUE: usize = 1;
    const MASK: u32 = 0b1;
}

impl MaterialBrickOps for MaterialBrick2 {
    const BITS_PER_VALUE: usize = 2;
    const MASK: u32 = 0b11;
}

impl MaterialBrickOps for MaterialBrick4 {
    const BITS_PER_VALUE: usize = 4;
    const MASK: u32 = 0b1111;
}

impl MaterialBrickOps for MaterialBrick8 {
    const BITS_PER_VALUE: usize = 8;
    const MASK: u32 = 0b11111111;
}

// Implement common methods for each brick type
macro_rules! impl_material_brick_methods {
    ($type:ty) => {
        impl $type {
            pub fn get(&self, x: u32, y: u32, z: u32) -> u8 {
                Self::get_value(&self.raw, x, y, z)
            }

            pub fn set(&mut self, x: u32, y: u32, z: u32, val: u8) {
                Self::set_value(&mut self.raw, x, y, z, val)
            }
        }
    };
}

impl_material_brick_methods!(MaterialBrick1);
impl_material_brick_methods!(MaterialBrick2);
impl_material_brick_methods!(MaterialBrick4);
impl_material_brick_methods!(MaterialBrick8);

#[derive(Debug)]
pub enum MaterialBrick {
    Size1(MaterialBrick1),
    Size2(MaterialBrick2),
    Size4(MaterialBrick4),
    Size8(MaterialBrick8),
}

impl MaterialBrick {
    pub fn data(&self) -> &[u8] {
        match self {
            Self::Size1(b) => bytemuck::cast_slice(&b.raw),
            Self::Size2(b) => bytemuck::cast_slice(&b.raw),
            Self::Size4(b) => bytemuck::cast_slice(&b.raw),
            Self::Size8(b) => bytemuck::cast_slice(&b.raw),
        }
    }

    pub fn element_size(&self) -> u64 {
        match self {
            Self::Size1(_) => 1,
            Self::Size2(_) => 2,
            Self::Size4(_) => 4,
            Self::Size8(_) => 8,
        }
    }

    pub fn get(&self, x: u32, y: u32, z: u32) -> u8 {
        match self {
            Self::Size1(b) => b.get(x, y, z),
            Self::Size2(b) => b.get(x, y, z),
            Self::Size4(b) => b.get(x, y, z),
            Self::Size8(b) => b.get(x, y, z),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ExpandedBrick {
    raw: [u8; 512],
}

impl TraceBrick {
    pub const EMPTY: Self = Self::empty();
    pub const fn empty() -> Self {
        Self {
            raw: [0; 64],
            brick: 0,
            palette: 0,
        }
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
        assert!(size <= 0b111);
        self.brick = (self.brick & !(0b111 << 29)) | (size << 29);
        // let masked_value = size & 0b111;
        // self.brick &= 0x1FFF_FFFF;
        // // Shift new value into position and set
        // self.brick |= masked_value << 29;
    }

    pub fn get_brick_offset(&self) -> u32 {
        self.brick & 0x1FFFFFFF
    }

    pub fn set_brick_offset(&mut self, offset: u32) {
        assert!(offset <= 0x1FFFFFFF);
        self.brick = (self.brick & (0b111 << 29)) | offset;
        // let masked_value = offset & 0x1FFF_FFFF;
        // // Clear lower 29 bits of existing value
        // self.brick &= 0xE000_0000;
        // // Set new value
        // self.brick |= masked_value;
    }

    pub fn set_brick_info(&mut self, size: u32, offset: u32) {
        assert!(size <= 0b111);
        self.brick = (size << 29) | offset;
    }

    pub fn set_palette(&mut self, palette: PaletteId) {
        self.palette = palette.0;
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
}

impl ExpandedBrick {
    pub fn compress(
        &self,
        material_mapping: &ExpandedMaterialMapping,
    ) -> (MaterialBrick, Vec<MaterialId>) {
        // Get unique values and sort them
        let mut unique_values: Vec<u8> = self.raw.iter().copied().collect();
        unique_values.sort_unstable();
        unique_values.dedup();

        let mut value_map = [0u8; 256];
        let mut material_ids = vec![MaterialId(0)]; // Always start with MaterialId(0)
        let mut next_value = 1u8; // Start from 1 for non-zero values

        for &val in unique_values.iter() {
            if val != 0 {
                // Skip 0 as it must map to 0
                value_map[val as usize] = next_value;
                material_ids.push(material_mapping.material(val));
                next_value += 1;
            }
        }

        let unique_count = next_value - 1;

        let material_brick = match unique_count {
            0 => MaterialBrick::Size1(MaterialBrick1 { raw: [0; 16] }),
            1..=1 => {
                let mut brick = MaterialBrick1 { raw: [0; 16] };
                for x in 0..8 {
                    for y in 0..8 {
                        for z in 0..8 {
                            let old_val = self.get(x, y, z);
                            let new_val = value_map[old_val as usize];
                            brick.set(x, y, z, new_val);
                        }
                    }
                }
                MaterialBrick::Size1(brick)
            }
            2..=3 => {
                let mut brick = MaterialBrick2 { raw: [0; 32] };
                for x in 0..8 {
                    for y in 0..8 {
                        for z in 0..8 {
                            let old_val = self.get(x, y, z);
                            let new_val = value_map[old_val as usize];
                            brick.set(x, y, z, new_val);
                        }
                    }
                }
                MaterialBrick::Size2(brick)
            }
            4..=15 => {
                let mut brick = MaterialBrick4 { raw: [0; 64] };
                for x in 0..8 {
                    for y in 0..8 {
                        for z in 0..8 {
                            let old_val = self.get(x, y, z);
                            let new_val = value_map[old_val as usize];
                            brick.set(x, y, z, new_val);
                        }
                    }
                }
                MaterialBrick::Size4(brick)
            }
            _ => {
                let mut brick = MaterialBrick8 { raw: [0; 128] };
                for x in 0..8 {
                    for y in 0..8 {
                        for z in 0..8 {
                            let old_val = self.get(x, y, z);
                            let new_val = value_map[old_val as usize];
                            brick.set(x, y, z, new_val);
                        }
                    }
                }
                MaterialBrick::Size8(brick)
            }
        };

        (material_brick, material_ids)
    }
}
