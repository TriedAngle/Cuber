use std::collections::HashMap;

use parking_lot::RwLock;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialId(pub u32);

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PbrMaterial {
    pub color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    _padding1: [f32; 2],
    pub emissive: [f32; 4],
}

#[derive(Debug)]
pub struct MaterialRegistry {
    materials: RwLock<Vec<PbrMaterial>>,
    name_to_id: RwLock<HashMap<String, MaterialId>>,
}

impl MaterialRegistry {
    pub fn new() -> Self {
        Self {
            materials: RwLock::new(Vec::new()),
            name_to_id: RwLock::new(HashMap::new()),
        }
    }

    pub fn register_default_materials(&self) {
        self.register_named_material(
            "air",
            PbrMaterial::new([1.0, 0.0, 1.0, 0.0], 0.0, 0.0, [0.0; 3], 0.0),
        );
        self.register_named_material("stone", PbrMaterial::stone(0.5));
        self.register_named_material("bedrock", PbrMaterial::stone(1.5));
        self.register_named_material("dirt", PbrMaterial::dry_dirt());
        self.register_named_material("grass", PbrMaterial::lush_grass());
        self.register_named_material(
            "snow",
            PbrMaterial::new([0.95, 0.95, 0.95, 1.0], 0.0, 0.3, [0.0; 3], 1.0),
        );
    }

    pub fn register_material(&self, material: PbrMaterial) -> MaterialId {
        let mut materials = self.materials.write();
        let id = MaterialId(materials.len() as u32);
        materials.push(material);
        id
    }

    pub fn register_name(&self, name: &str, id: MaterialId) {
        if (id.0 as usize) >= self.materials.read().len() {
            panic!("Attempted to register name for non-existent material ID");
        }

        let mut names = self.name_to_id.write();
        names.insert(name.to_string(), id);
    }

    pub fn register_named_material(&self, name: &str, material: PbrMaterial) -> MaterialId {
        let id = self.register_material(material);
        self.register_name(name, id);
        id
    }

    pub fn get_material_id(&self, name: &str) -> Option<MaterialId> {
        self.name_to_id.read().get(name).copied()
    }

    pub fn get_material(&self, id: MaterialId) -> Option<PbrMaterial> {
        self.materials.read().get(id.0 as usize).copied()
    }

    pub fn get_material_by_name(&self, name: &str) -> Option<PbrMaterial> {
        self.get_material_id(name)
            .and_then(|id| self.get_material(id))
    }

    pub fn materials(&self) -> &[PbrMaterial] {
        unsafe { self.materials.data_ptr().as_ref().unwrap() }
    }
}

pub struct ExpandedMaterialMapping {
    voxel_to_id: HashMap<u8, MaterialId>,
    string_to_voxel: HashMap<String, u8>,
}

impl ExpandedMaterialMapping {
    pub fn new() -> Self {
        Self {
            voxel_to_id: HashMap::new(),
            string_to_voxel: HashMap::new(),
        }
    }

    pub fn add_from_registry(
        &mut self,
        registry: &MaterialRegistry,
        name: &str,
        voxel: u8,
    ) -> Option<()> {
        let id = registry.get_material_id(name)?;
        self.voxel_to_id.insert(voxel, id);
        self.string_to_voxel.insert(name.to_string(), voxel);
        Some(())
    }

    pub fn get(&self, name: &str) -> u8 {
        self.string_to_voxel.get(name).copied().unwrap()
    }

    pub fn material(&self, voxel: u8) -> MaterialId {
        self.voxel_to_id.get(&voxel).copied().unwrap()
    }
}

#[allow(unused)]
impl PbrMaterial {
    /// Create a new material with full control over all parameters
    pub fn new(
        color: [f32; 4],
        metallic: f32,
        roughness: f32,
        emissive: [f32; 3],
        alpha_cutoff: f32,
    ) -> Self {
        Self {
            color,
            metallic: metallic.clamp(0.0, 1.0),
            roughness: roughness.clamp(0.0, 1.0),
            _padding1: [0.0; 2],
            emissive: [
                emissive[0],
                emissive[1],
                emissive[2],
                alpha_cutoff.clamp(0.0, 1.0),
            ],
        }
    }

    pub fn metal(color: [f32; 3], roughness: f32) -> Self {
        Self::new(
            [color[0], color[1], color[2], 1.0],
            1.0,
            roughness,
            [0.0; 3],
            1.0,
        )
    }

    pub fn ceramic(color: [f32; 3]) -> Self {
        Self::new(
            [color[0], color[1], color[2], 1.0],
            0.0,
            0.3, // Slightly glossy
            [0.0; 3],
            1.0,
        )
    }

    pub fn stone(variation: f32) -> Self {
        let smoothness = 1.0 - variation;
        let base_grey = 0.5 + (smoothness * 0.2); // Smoother stone is slightly lighter

        Self::new(
            [base_grey, base_grey, base_grey, 1.0],
            0.0,                      // Non-metallic
            0.75 + (variation * 0.2), // Rougher as variation increases
            [0.0; 3],
            1.0,
        )
    }

    pub fn granite() -> Self {
        Self::new(
            [0.8, 0.8, 0.8, 1.0], // Light grey base
            0.1,                  // Slight metallic sheen for crystalline structure
            0.4,                  // Relatively smooth when polished
            [0.0; 3],
            1.0,
        )
    }

    pub fn dirt(moisture: f32) -> Self {
        let moisture = moisture.clamp(0.0, 1.0);
        let darkening = moisture * 0.3; // Dirt gets darker when wet

        Self::new(
            [
                0.6 - darkening, // Reddish-brown base
                0.4 - darkening,
                0.2 - darkening,
                1.0,
            ],
            0.0,
            1.0 - (moisture * 0.4), // Gets slightly smoother when wet
            [0.0; 3],
            1.0,
        )
    }

    pub fn grass(dryness: f32) -> Self {
        let dryness = dryness.clamp(0.0, 1.0);

        Self::new(
            [
                0.3 + (dryness * 0.4), // Gets yellower as it dries
                0.5 + (dryness * 0.2),
                0.1,
                1.0,
            ],
            0.0,
            0.95, // Grass is generally rough
            [0.0; 3],
            1.0,
        )
    }

    pub fn dry_dirt() -> Self {
        PbrMaterial::dirt(0.0)
    }
    pub fn wet_dirt() -> Self {
        PbrMaterial::dirt(1.0)
    }

    pub fn lush_grass() -> Self {
        PbrMaterial::grass(0.0)
    }
    pub fn dead_grass() -> Self {
        PbrMaterial::grass(1.0)
    }
}
