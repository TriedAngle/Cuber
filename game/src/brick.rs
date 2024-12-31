use parking_lot::{Mutex, RwLock};
use rand::Rng;

use crate::material::{ExpandedMaterialMapping, MaterialId};

#[repr(transparent)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BrickHandle(pub u32);

impl BrickHandle {
    const DATA_BIT: u32 = 0x8000_0000; // Bit 31
    const LOD_BIT: u32 = 0x4000_0000; // Bit 30 (only used when DATA_BIT is 0)
    const DATA_MASK: u32 = 0x7FFF_FFFF; // Bits 0-30 for data
    const EMPTY_MASK: u32 = 0x3FFF_FFFF; // Bits 0-29 for empty handle values

    const EMPTY: Self = Self::empty();

    pub const fn empty() -> Self {
        Self(0)
    }

    pub fn new_empty_with_value(value: u32) -> Self {
        assert!(
            value <= Self::EMPTY_MASK,
            "Value exceeds maximum allowed for empty handle"
        );
        BrickHandle(value & Self::EMPTY_MASK)
    }

    pub fn new_data(value: u32) -> Self {
        assert!(
            value <= Self::DATA_MASK,
            "Value exceeds maximum allowed for data"
        );
        BrickHandle(Self::DATA_BIT | (value & Self::DATA_MASK))
    }

    pub fn is_data(&self) -> bool {
        (self.0 & Self::DATA_BIT) != 0
    }

    pub fn is_empty(&self) -> bool {
        !self.is_data()
    }

    pub fn is_lod(&self) -> bool {
        self.is_empty() && (self.0 & Self::LOD_BIT) != 0
    }

    pub fn set_lod(&mut self, is_lod: bool) {
        assert!(self.is_empty(), "Cannot set LOD bit on data handle");
        self.0 = (self.0 & !Self::LOD_BIT) | (u32::from(is_lod) << 30);
    }

    pub fn get_data_value(&self) -> u32 {
        self.0 & Self::DATA_MASK
    }

    pub fn set_data_value(&mut self, value: u32) {
        assert!(
            value <= Self::DATA_MASK,
            "Value exceeds maximum allowed for data"
        );
        self.0 = Self::DATA_BIT | (value & Self::DATA_MASK);
    }

    pub fn get_empty_value(&self) -> u32 {
        self.0 & Self::EMPTY_MASK
    }

    pub fn set_empty_value(&mut self, value: u32) {
        assert!(self.is_empty(), "Cannot set empty value on data handle");
        assert!(
            value <= Self::EMPTY_MASK,
            "Value exceeds maximum allowed for empty handle"
        );
        let lod_bit = self.0 & Self::LOD_BIT;
        self.0 = lod_bit | (value & Self::EMPTY_MASK);
    }

    pub fn as_raw(&self) -> u32 {
        self.0
    }

    pub fn from_raw(raw: u32) -> Self {
        BrickHandle(raw)
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
    freelist: Mutex<Vec<u32>>,
}

impl BrickMap {
    pub fn new(size: na::Vector3<u32>) -> Self {
        let volume = size.x * size.y * size.z;
        let handles = RwLock::new(vec![BrickHandle::EMPTY; volume as usize]);
        let bricks = RwLock::new(vec![]);
        let freelist = Mutex::new(Vec::new());

        Self {
            size,
            handles,
            bricks,
            freelist,
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

    pub fn set_empty(&self, at: na::Point3<u32>) -> BrickHandle {
        let id = self.index(at);
        let mut handles = self.handles.write();
        let handle = handles[id];
        self.deallocate_brick(handle);
        let new_handle = BrickHandle::empty();
        handles[id] = new_handle;
        new_handle
    }

    pub fn set_lod(&self, at: na::Point3<u32>, lod: MaterialId) -> BrickHandle {
        let id = self.index(at);
        let mut handles = self.handles.write();
        let handle = handles[id];
        self.deallocate_brick(handle);
        let mut new_handle = BrickHandle::empty();
        new_handle.set_lod(true);
        new_handle.set_empty_value(lod.0);
        handles[id] = new_handle;
        new_handle
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

    pub fn set_brick(&self, brick: TraceBrick, at: na::Point3<u32>) -> (BrickHandle, bool) {
        let mut bricks = self.bricks.write();
        let old_handle = self.get_handle(at);
        if let Some(_old_brick) = self.get_brick(old_handle) {
            let offset = old_handle.get_data_value();
            bricks[offset as usize] = brick;
            return (old_handle, false);
        }

        let mut freelist = self.freelist.lock();
        if let Some(offset) = freelist.pop() {
            bricks[offset as usize] = brick;
            let handle = BrickHandle::new_data(offset);
            self.set_handle(handle, at);
            return (handle, false);
        }

        let offset = bricks.len() as u32;
        bricks.push(brick);
        let handle = BrickHandle::new_data(offset as u32);
        self.set_handle(handle, at);
        (handle, true)
    }

    pub fn deallocate_brick(&self, handle: BrickHandle) -> bool {
        if !handle.is_data() {
            return false;
        }

        let offset = handle.get_data_value() as usize;

        let mut freelist = self.freelist.lock();
        freelist.push(offset as u32);

        true
    }

    pub fn volume(&self) -> u32 {
        self.size.x * self.size.y * self.size.z
    }

    // pub fn edit_brick_no_resize(
    //     &self,
    //     handle: BrickHandle,
    //     at: Option<na::Point3<u32>>,
    //     value: u8,
    // ) {
    //     let mut offset = 0;
    //     let mut bits_per_element = 0;
    //     self.modify_brick(handle, |brick| {
    //         offset = brick.get_brick_offset();
    //         bits_per_element = brick.get_brick_size() + 1;
    //         if let Some(at) = at {
    //             brick.set(at.x, at.y, at.z, value != 0);
    //         }
    //     });
    //
    //     if let Some(at) = at {
    //         let data = self.material_bricks.data_mut();
    //         match bits_per_element {
    //             1 => {
    //                 let brick: &mut MaterialBrick1 = unsafe {
    //                     &mut *(data[offset as usize..].as_mut_ptr() as *mut MaterialBrick1)
    //                 };
    //                 brick.set(at.x, at.y, at.z, value);
    //             }
    //             2 => {
    //                 let brick: &mut MaterialBrick2 = unsafe {
    //                     &mut *(data[offset as usize..].as_mut_ptr() as *mut MaterialBrick2)
    //                 };
    //                 brick.set(at.x, at.y, at.z, value);
    //             }
    //             4 => {
    //                 let brick: &mut MaterialBrick4 = unsafe {
    //                     &mut *(data[offset as usize..].as_mut_ptr() as *mut MaterialBrick4)
    //                 };
    //                 brick.set(at.x, at.y, at.z, value);
    //             }
    //             8 => {
    //                 let brick: &mut MaterialBrick8 = unsafe {
    //                     &mut *(data[offset as usize..].as_mut_ptr() as *mut MaterialBrick8)
    //                 };
    //                 brick.set(at.x, at.y, at.z, value);
    //             }
    //             _ => panic!("Invalid brick size: {}", bits_per_element),
    //         }
    //     }
    // }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TraceBrick {
    raw: [u8; 64],
    brick: u32,
}

impl TraceBrick {
    pub const EMPTY: Self = Self::empty();
    pub const fn empty() -> Self {
        Self {
            raw: [0; 64],
            brick: 0,
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

    pub fn get_brick_offset(&self) -> u32 {
        self.brick
    }

    pub fn set_brick_offset(&mut self, offset: u32) {
        self.brick = offset;
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialBrickMeta {
    pub meta: u32,
}

impl MaterialBrickMeta {
    pub fn decode_meta(&self) -> u32 {
        self.meta & 0x1FFF_FFFF
    }

    pub fn decode_meta_size(&self) -> usize {
        ((self.meta >> 29) as usize) + 1
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialBrick1 {
    pub meta: u32,
    pub raw: [u32; 16], // 64 bytes = 16 u32s (1 bit per value)
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialBrick2 {
    pub meta: u32,
    pub raw: [u32; 32], // 128 bytes = 32 u32s (2 bits per value)
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialBrick4 {
    pub meta: u32,
    pub raw: [u32; 64], // 256 bytes = 64 u32s (4 bits per value)
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialBrick8 {
    pub meta: u32,
    pub raw: [u32; 128], // 512 bytes = 128 u32s (8 bits per value)
}

impl MaterialBrick1 {
    pub fn empty() -> Self {
        Self {
            meta: Self::encode_size_only(Self::BITS_PER_VALUE),
            raw: [0; 16],
        }
    }

    pub fn from_expanded_brick(expanded: &ExpandedBrick, meta: u32) -> Self {
        let mut brick = Self {
            meta: Self::encode_meta(meta),
            raw: [0; 16],
        };
        Self::pack_values(expanded, &mut brick.raw);
        brick
    }
}

impl MaterialBrick2 {
    pub fn empty() -> Self {
        Self {
            meta: Self::encode_size_only(Self::BITS_PER_VALUE),
            raw: [0; 32],
        }
    }
    pub fn from_expanded_brick(expanded: &ExpandedBrick, meta: u32) -> Self {
        let mut brick = Self {
            meta: Self::encode_meta(meta),
            raw: [0; 32],
        };
        Self::pack_values(expanded, &mut brick.raw);
        brick
    }
}

impl MaterialBrick4 {
    pub fn empty() -> Self {
        Self {
            meta: Self::encode_size_only(Self::BITS_PER_VALUE),
            raw: [0; 64],
        }
    }
    pub fn from_expanded_brick(expanded: &ExpandedBrick, meta: u32) -> Self {
        let mut brick = Self {
            meta: Self::encode_meta(meta),
            raw: [0; 64],
        };
        Self::pack_values(expanded, &mut brick.raw);
        brick
    }
}

impl MaterialBrick8 {
    pub fn empty() -> Self {
        Self {
            meta: Self::encode_size_only(Self::BITS_PER_VALUE),
            raw: [0; 128],
        }
    }
    pub fn from_expanded_brick(expanded: &ExpandedBrick, meta: u32) -> Self {
        let mut brick = Self {
            meta: Self::encode_meta(meta),
            raw: [0; 128],
        };
        Self::pack_values(expanded, &mut brick.raw);
        brick
    }
}

pub trait MaterialBrickOps {
    const BITS_PER_VALUE: usize;
    const MASK: u32;

    fn encode_meta(meta_value: u32) -> u32 {
        let meta_value = meta_value & 0x1FFF_FFFF;
        let size_bits = ((Self::BITS_PER_VALUE - 1) as u32) << 29;
        meta_value | size_bits
    }

    fn decode_meta(meta: u32) -> u32 {
        meta & 0x1FFF_FFFF
    }

    fn decode_meta_size(meta: u32) -> usize {
        ((meta >> 29) as usize) + 1
    }

    fn encode_size_only(bits_per_value: usize) -> u32 {
        ((bits_per_value - 1) as u32) << 29
    }

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

    pub fn size(&self) -> usize {
        let element_size = self.element_size() as usize;
        let size = element_size * 512 + std::mem::size_of::<u32>();
        size
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
    pub fn set_meta_value(&mut self, meta_value: u32) {
        match self {
            Self::Size1(b) => b.meta = MaterialBrick1::encode_meta(meta_value),
            Self::Size2(b) => b.meta = MaterialBrick2::encode_meta(meta_value),
            Self::Size4(b) => b.meta = MaterialBrick4::encode_meta(meta_value),
            Self::Size8(b) => b.meta = MaterialBrick8::encode_meta(meta_value),
        }
    }
    pub fn meta_value(&self) -> u32 {
        match self {
            Self::Size1(b) => MaterialBrick1::decode_meta(b.meta),
            Self::Size2(b) => MaterialBrick2::decode_meta(b.meta),
            Self::Size4(b) => MaterialBrick4::decode_meta(b.meta),
            Self::Size8(b) => MaterialBrick8::decode_meta(b.meta),
        }
    }

    pub fn size_from_meta(&self) -> usize {
        match self {
            Self::Size1(b) => MaterialBrick1::decode_meta_size(b.meta),
            Self::Size2(b) => MaterialBrick2::decode_meta_size(b.meta),
            Self::Size4(b) => MaterialBrick4::decode_meta_size(b.meta),
            Self::Size8(b) => MaterialBrick8::decode_meta_size(b.meta),
        }
    }

    pub fn meta(&self) -> u32 {
        match self {
            Self::Size1(b) => b.meta,
            Self::Size2(b) => b.meta,
            Self::Size4(b) => b.meta,
            Self::Size8(b) => b.meta,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ExpandedBrick {
    meta: u32,
    raw: [u8; 512],
}

impl ExpandedBrick {
    pub const EMPTY: Self = Self::empty();

    pub const fn empty() -> Self {
        Self {
            meta: 0,
            raw: [0; 512],
        }
    }

    pub fn random(limit: u8) -> Self {
        let mut new = Self::empty();
        let mut rng = rand::thread_rng();

        for byte in new.raw.iter_mut() {
            *byte = rng.gen_range(0..limit);
        }
        new
    }

    fn encode_meta(&mut self, meta_value: u32) {
        let bits_per_element = Self::get_required_bits(&self);
        let meta_value = meta_value & 0x1FFF_FFFF;
        let size_bits = ((bits_per_element - 1) as u32) << 29;
        self.meta = meta_value | size_bits;
    }

    fn decode_meta(&self) -> u32 {
        self.meta & 0x1FFF_FFFF
    }

    fn decode_meta_size(&self) -> usize {
        ((self.meta >> 29) as usize) + 1
    }

    fn encode_size_only(bits_per_value: usize) -> u32 {
        ((bits_per_value - 1) as u32) << 29
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

    pub fn compress(
        &self,
        material_mapping: &ExpandedMaterialMapping,
    ) -> (MaterialBrick, Vec<MaterialId>) {
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
            0 => {
                let mut brick = MaterialBrick1::empty();
                brick.meta = self.meta;
                MaterialBrick::Size1(brick)
            }
            1..=1 => {
                let mut brick = MaterialBrick1::empty();
                for x in 0..8 {
                    for y in 0..8 {
                        for z in 0..8 {
                            let old_val = self.get(x, y, z);
                            let new_val = value_map[old_val as usize];
                            brick.set(x, y, z, new_val);
                        }
                    }
                }
                brick.meta = self.meta;
                MaterialBrick::Size1(brick)
            }
            2..=3 => {
                let mut brick = MaterialBrick2::empty();
                for x in 0..8 {
                    for y in 0..8 {
                        for z in 0..8 {
                            let old_val = self.get(x, y, z);
                            let new_val = value_map[old_val as usize];
                            brick.set(x, y, z, new_val);
                        }
                    }
                }

                brick.meta = self.meta;
                MaterialBrick::Size2(brick)
            }
            4..=15 => {
                let mut brick = MaterialBrick4::empty();
                for x in 0..8 {
                    for y in 0..8 {
                        for z in 0..8 {
                            let old_val = self.get(x, y, z);
                            let new_val = value_map[old_val as usize];
                            brick.set(x, y, z, new_val);
                        }
                    }
                }
                brick.meta = self.meta;
                MaterialBrick::Size4(brick)
            }
            _ => {
                let mut brick = MaterialBrick8::empty();
                for x in 0..8 {
                    for y in 0..8 {
                        for z in 0..8 {
                            let old_val = self.get(x, y, z);
                            let new_val = value_map[old_val as usize];
                            brick.set(x, y, z, new_val);
                        }
                    }
                }
                brick.meta = self.meta;
                MaterialBrick::Size8(brick)
            }
        };

        (material_brick, material_ids)
    }
}
