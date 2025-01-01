use crate::material::MaterialId;
use parking_lot::RwLock;
use std::collections::HashMap;

// #[repr(C)]
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PaletteId(pub u32);
impl PaletteId {
    pub const EMPTY: Self = Self(0);
}

#[derive(Debug)]
pub struct PaletteRegistry {
    palette_data: RwLock<Vec<MaterialId>>,
    palette_map: RwLock<HashMap<Vec<MaterialId>, (PaletteId, u32)>>,
    freelist: RwLock<HashMap<u32, Vec<PaletteId>>>,
}

impl PaletteRegistry {
    pub fn new() -> Self {
        Self {
            palette_data: RwLock::new(Vec::new()),
            palette_map: RwLock::new(HashMap::new()),
            freelist: RwLock::new(HashMap::new()),
        }
    }

    pub fn register_palette(&self, materials: Vec<MaterialId>) -> PaletteId {
        let count = materials.len() as u32;

        {
            let palette_map = self.palette_map.read();
            if let Some(&(id, _)) = palette_map.get(&materials) {
                return id;
            }
        }

        let id = {
            let mut freelist = self.freelist.write();
            freelist.get_mut(&count).and_then(|free_ids| free_ids.pop())
        };

        let id = if let Some(id) = id {
            let mut palette_data = self.palette_data.write();
            for (i, material) in materials.iter().enumerate() {
                palette_data[id.0 as usize + i] = *material;
            }
            id
        } else {
            let mut palette_data = self.palette_data.write();
            let index = palette_data.len() as u32;
            palette_data.extend_from_slice(&materials);
            PaletteId(index)
        };

        let mut palette_map = self.palette_map.write();
        palette_map.insert(materials, (id, count));

        id
    }

    pub fn dealloc_palette(&self, id: PaletteId) {
        let size = {
            let palette_map = self.palette_map.read();
            let size = palette_map
                .iter()
                .find(|(_, &(index, _))| index == id)
                .map(|(_, &(_, count))| count);

            match size {
                Some(size) => size,
                None => return,
            }
        };

        let mut freelist = self.freelist.write();
        freelist.entry(size).or_insert_with(Vec::new).push(id);
    }

    pub fn get_palette(&self, id: PaletteId) -> Option<Vec<MaterialId>> {
        let palette_map = self.palette_map.read();
        let palette_data = self.palette_data.read();

        let count = palette_map
            .iter()
            .find(|(_, &(index, _))| index == id)
            .map(|(_, &(_, count))| count)?;

        let start = id.0 as usize;
        let end = start + count as usize;

        if end <= palette_data.len() {
            Some(palette_data[start..end].to_vec())
        } else {
            None
        }
    }

    pub fn palette_data(&self) -> &[MaterialId] {
        unsafe { self.palette_data.data_ptr().as_ref().unwrap() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_palette_lifecycle() {
        let registry = PaletteRegistry::new();

        // Register a palette
        let palette1 = vec![MaterialId(0), MaterialId(1)];
        let id1 = registry.register_palette(palette1.clone());

        // Register same palette again - should get same ID
        let id2 = registry.register_palette(palette1.clone());
        assert_eq!(id1, id2);

        // Deallocate the palette
        registry.dealloc_palette(id1);

        // Register new palette of same size - should reuse the space
        let palette2 = vec![MaterialId(2), MaterialId(3)];
        let id3 = registry.register_palette(palette2.clone());

        // Should reuse the same space
        assert_eq!(id1.0, id3.0);

        // Verify the new palette is stored correctly
        let retrieved = registry.get_palette(id3).unwrap();
        assert_eq!(retrieved, palette2);
    }
}
