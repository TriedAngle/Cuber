use std::collections::HashMap;

use parking_lot::RwLock;
use rand::Rng;

#[repr(transparent)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BrickHandle(pub u32);

impl BrickHandle {
    pub fn zero() -> Self {
        Self(0)
    }

    pub fn empty() -> Self {
        Self::zero()
    }

    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }

    pub fn next(&self) -> Self {
        Self(self.0 + 1)
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
    bricks: RwLock<Vec<Brick>>,
    do_cache: bool,
    cache: RwLock<HashMap<Brick, BrickHandle>>,
}

impl BrickMap {
    pub fn new(size: na::Vector3<u32>, do_cache: bool) -> Self {
        let volume = size.x * size.y * size.z;
        let handles = RwLock::new(vec![BrickHandle::empty(); volume as usize]);
        let bricks = RwLock::new(vec![]);
        let cache = RwLock::new(HashMap::new());

        Self {
            size,
            handles,
            bricks,
            do_cache,
            cache,
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

    pub fn bricks(&self) -> &[Brick] {
        let ptr = self.bricks.data_ptr();

        let bricks = unsafe { ptr.as_ref().unwrap() };

        bricks
    }

    pub fn handles(&self) -> &[BrickHandle] {
        let ptr = self.handles.data_ptr();

        let handles = unsafe { ptr.as_ref().unwrap() };

        handles
    }

    pub fn dimensions(&self) -> na::Vector3<u32> {
        self.size
    }

    pub fn cache_get(&self, brick: &Brick) -> Option<BrickHandle> {
        let cache = self.cache.read();
        cache.get(brick).copied()
    }

    pub fn cache_set(&self, brick: Brick, handle: BrickHandle) {
        let mut cache = self.cache.write();
        cache.insert(brick, handle);
    }

    pub fn brick_push(&self, brick: Brick) -> BrickHandle {
        let offset = self.bricks.read().len();
        let mut bricks = self.bricks.write();
        bricks.push(brick);
        BrickHandle(offset as u32)
    }

    pub fn get_or_push_brick(&self, brick: Brick, at: na::Vector3<u32>) -> BrickHandle {
        let mut cache = self.cache.write();

        if self.do_cache {
            if let Some(handle) = cache.get(&brick).copied() {
                return handle;
            }
        }

        let id = self.index(at);
        let mut handles = self.handles.write();
        let mut bricks = self.bricks.write();

        let handle = BrickHandle(bricks.len() as u32);
        bricks.push(brick);
        cache.insert(brick, handle);
        handles[id] = handle;
        handle
    }

    pub fn volume(&self) -> u32 {
        self.size.x * self.size.y * self.size.z
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable, Eq, Hash, PartialEq)]
pub struct Brick {
    raw: [u8; 64],
}

impl Brick {
    pub const EMPTY: Self = Self::empty();
    pub const fn empty() -> Self {
        Self { raw: [0; 64] }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brick() {
        let mut brick = Brick::empty();
        assert_eq!(brick.get(0, 0, 0), false);
        brick.set(0, 0, 0, true);
        brick.set(3, 5, 2, true);
        assert_eq!(brick.get(0, 0, 0), true);
        assert_eq!(brick.get(3, 5, 2), true);
    }
}
