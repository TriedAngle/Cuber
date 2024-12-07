use rand::Rng;

#[repr(transparent)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BrickHandle(pub u32);

impl BrickHandle {
    pub fn empty() -> Self {
        Self(0)
    }

    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }
}

impl From<u32> for BrickHandle {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

pub struct BrickMap {
    size: na::Vector3<u32>,
    handles: Vec<BrickHandle>,
}

impl BrickMap {
    pub fn new(size: na::Vector3<u32>) -> Self {
        let volume = size.x * size.y * size.z;
        let handles = vec![BrickHandle::empty(); volume as usize];
        Self { size, handles }
    }

    pub fn index(&self, at: na::Vector3<u32>) -> usize {
        let id = at.x + (at.y * self.size.x) + (at.z * self.size.x * self.size.y);
        id as usize
    }

    pub fn get_handle(&self, at: na::Vector3<u32>) -> BrickHandle {
        let id = self.index(at);
        self.handles[id]
    }

    pub fn set_brick(&mut self, handle: BrickHandle, at: na::Vector3<u32>) {
        let id = self.index(at);
        self.handles[id] = handle;
    }

    pub fn set_empty(&mut self, at: na::Vector3<u32>) {
        let id = self.index(at);
        self.handles[id] = BrickHandle::empty();
    }

    pub fn is_empty(&self, at: na::Vector3<u32>) -> bool {
        let handle = self.get_handle(at);
        handle.is_empty()
    }

    pub fn handles(&self) -> &[BrickHandle] {
        &self.handles
    }

    pub fn dimensions(&self) -> na::Vector3<u32> {
        self.size
    }

    pub fn volume(&self) -> u32 {
        self.size.x * self.size.y * self.size.z
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Brick {
    raw: [u8; 64],
}

impl Brick {
    pub const fn empty() -> Self {
        Self { raw: [0; 64] }
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

    pub fn index(x: usize, y: usize, z: usize) -> (usize, usize) {
        let index = x + (y * 8) + (z * 64);
        let byte_index = index / 8;
        let bit_index = index % 8;
        (byte_index, bit_index)
    }

    pub fn set(&mut self, x: usize, y: usize, z: usize, val: bool) {
        let (byte_index, bit_index) = Self::index(x, y, z);
        if val {
            self.raw[byte_index] |= 1 << bit_index;
        } else {
            self.raw[byte_index] &= !(1 << bit_index);
        }
    }

    pub fn get(&self, x: usize, y: usize, z: usize) -> bool {
        let (byte_index, bit_index) = Self::index(x, y, z);
        self.raw[byte_index] & (1 << bit_index) != 0
    }

    pub fn toggle(&mut self, x: usize, y: usize, z: usize) {
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
