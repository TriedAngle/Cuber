use crate::{material::MaterialId, palette::PaletteId};

#[derive(Debug, Clone, Copy)]
pub struct SexagintaQuattourHandle(pub u32);

impl SexagintaQuattourHandle {
    pub const EMPTY: Self = Self(0);
    pub const HANDLE: Self = Self(1 << 31);
    pub const MATERIAL: Self = Self(1 << 30);
}

impl SexagintaQuattourHandle {
    pub fn handle(offset: u32) -> Self {
        Self(Self::HANDLE.0 | offset)
    }
    pub fn material(material: MaterialId) -> Self {
        Self(Self::MATERIAL.0 | material.0)
    }
}

pub struct SexagintaQuattourNode {
    pub meta: u32,
}

impl SexagintaQuattourNode {
    const EMPTY: Self = Self { meta: 0 };
    pub fn new(children: u8) -> Self {
        let mut new = Self::EMPTY;
        new.set_children(children);
        new
    }

    pub fn set_children(&mut self, children: u8) {
        self.meta = (self.meta & 0xFFFFFF00) | (children as u32);
    }

    pub fn get_children(&self) -> u8 {
        (self.meta & 0xFF) as u8
    }

    pub fn get_child_nodes(&self) -> [SexagintaQuattourHandle; 8] {
        let children_ptr = unsafe {
            (self as *const Self as *const u8).add(std::mem::size_of::<u32>())
                as *const SexagintaQuattourHandle
        };

        let mut children = [SexagintaQuattourHandle::EMPTY; 8];

        unsafe {
            std::ptr::copy_nonoverlapping(children_ptr, children.as_mut_ptr(), 8);
        }

        children
    }

    pub fn get_child_nodes_mut(&mut self) -> &mut [SexagintaQuattourHandle; 8] {
        unsafe {
            let children_ptr = (self as *mut Self as *mut u8).add(std::mem::size_of::<u32>())
                as *mut &mut [SexagintaQuattourHandle; 8];
            &mut *children_ptr
        }
    }

    pub fn get_child_indexed(&self, index: u8) -> Option<SexagintaQuattourHandle> {
        if index >= 64 {
            return None;
        }

        let octant = index / 8;
        let children = self.get_child_nodes();
        Some(children[octant as usize])
    }

    pub fn set_child_indexed(&mut self, index: u8, handle: SexagintaQuattourHandle) -> bool {
        if index >= 64 {
            return false;
        }

        let octant = index / 8;
        let children = self.get_child_nodes_mut();
        children[octant as usize] = handle;
        true
    }

    pub fn get_child(&self, x: u8, y: u8, z: u8) -> Option<SexagintaQuattourHandle> {
        let index = x + y * 4 + z * 4 * 4;
        self.get_child_indexed(index)
    }

    pub fn set_child(&mut self, x: u8, y: u8, z: u8, handle: SexagintaQuattourHandle) -> bool {
        let index = x + y * 4 + z * 4 * 4;
        self.set_child_indexed(index, handle)
    }
}

pub struct UncompressedSexagintaQuattourOuterNode {
    pub palette: PaletteId,
    pub voxels: [u32; 12],
}

impl UncompressedSexagintaQuattourOuterNode {
    pub const EMPTY: Self = Self {
        palette: PaletteId::EMPTY,
        voxels: [0; 12],
    };

    #[inline]
    pub fn new() -> Self {
        Self::EMPTY
    }

    #[inline]
    pub fn set_index(&mut self, position: u8, value: u8) {
        let array_pos = (position * 6 / 32) as usize;
        let bit_pos = (position * 6 % 32) as u32;

        self.voxels[array_pos] &= !(0x3f << bit_pos);
        self.voxels[array_pos] |= (value as u32) << bit_pos;

        if bit_pos > 26 {
            let bits_in_next = 6 - (32 - bit_pos);
            self.voxels[array_pos + 1] &= !(0x3f >> (6 - bits_in_next));
            self.voxels[array_pos + 1] |= (value as u32) >> (32 - bit_pos);
        }
    }

    #[inline]
    pub fn get_index(&self, position: u8) -> u8 {
        let array_pos = (position * 6 / 32) as usize;
        let bit_pos = (position * 6 % 32) as u32;

        if bit_pos <= 26 {
            ((self.voxels[array_pos] >> bit_pos) & 0x3f) as u8
        } else {
            let bits_in_next = 6 - (32 - bit_pos);
            let first_part = self.voxels[array_pos] >> bit_pos;
            let second_part =
                (self.voxels[array_pos + 1] & ((1_u32 << bits_in_next) - 1)) << (6 - bits_in_next);
            ((first_part | second_part) & 0x3f) as u8
        }
    }
}
