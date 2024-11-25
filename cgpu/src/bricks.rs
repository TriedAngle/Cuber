#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct Brick {
    raw: [u8; 64],
}

unsafe impl bytemuck::Pod for Brick {}
unsafe impl bytemuck::Zeroable for Brick {}

impl Brick {
    pub const fn empty() -> Self {
        Self { raw: [0; 64] }
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
