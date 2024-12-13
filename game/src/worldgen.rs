use fastnoise_lite::{FastNoiseLite, FractalType, NoiseType};
use rayon::prelude::*;

use crate::{
    brick::{BrickHandle, BrickMap, ExpandedBrick},
    material::{ExpandedMaterialMapping, MaterialId},
};

#[derive(Debug, Clone, Copy)]
pub enum LodSamples {
    A1, // 1 sample (1x1x1)
    A2, // 8 samples (2x2x2)
    A3, // 27 samples (3x3x3)
    A4, // 64 samples (4x4x4)
    A5, // 125 samples (5x5x5)
}

pub enum GeneratedBrick {
    Brick(ExpandedBrick),
    Lod(MaterialId),
    None,
}

impl LodSamples {
    pub fn samples_per_axis(&self) -> u32 {
        match self {
            Self::A1 => 1,
            Self::A2 => 2,
            Self::A3 => 3,
            Self::A4 => 4,
            Self::A5 => 5,
        }
    }
}

pub struct WorldGenerator {
    terrain_noise: FastNoiseLite,
    continent_noise: FastNoiseLite,
    cave_noise: FastNoiseLite,
}

impl WorldGenerator {
    pub fn new() -> Self {
        let mut terrain_noise = FastNoiseLite::new();
        terrain_noise.set_noise_type(Some(NoiseType::Perlin));
        terrain_noise.set_seed(Some(2324));
        terrain_noise.set_frequency(Some(0.005));
        terrain_noise.set_fractal_type(Some(FractalType::FBm));
        terrain_noise.set_fractal_octaves(Some(4));
        terrain_noise.set_fractal_lacunarity(Some(2.0));
        terrain_noise.set_fractal_gain(Some(0.5));

        let mut continent_noise = FastNoiseLite::new();
        continent_noise.set_noise_type(Some(NoiseType::Perlin));
        continent_noise.set_seed(Some(9999));
        continent_noise.set_frequency(Some(0.0005));
        continent_noise.set_fractal_type(Some(FractalType::FBm));
        continent_noise.set_fractal_octaves(Some(3));
        continent_noise.set_fractal_lacunarity(Some(2.0));
        continent_noise.set_fractal_gain(Some(0.5));

        let mut cave_noise = FastNoiseLite::new();
        cave_noise.set_noise_type(Some(NoiseType::Perlin));
        cave_noise.set_seed(Some(12345));
        cave_noise.set_frequency(Some(0.03));
        cave_noise.set_fractal_type(Some(FractalType::FBm));
        cave_noise.set_fractal_octaves(Some(3));
        cave_noise.set_fractal_lacunarity(Some(2.0));
        cave_noise.set_fractal_gain(Some(0.5));

        let generator = Self {
            terrain_noise,
            continent_noise,
            cave_noise,
        };

        generator
    }

    fn generate_block(&self, materials: &ExpandedMaterialMapping, x: u32, y: u32, z: u32) -> u8 {
        let air = materials.get("air");
        let stone = materials.get("stone");
        let bedrock = materials.get("bedrock");
        let dirt = materials.get("dirt");
        let grass = materials.get("grass");
        let snow = materials.get("snow");

        let continent_val = self.continent_noise.get_noise_2d(x as f32, z as f32);
        let terrain_val = self.terrain_noise.get_noise_2d(x as f32, z as f32);
        let final_height = (100.0 + (continent_val * 150.0) + (terrain_val * 180.0).round()) as u32;
        let height_diff = final_height - y;

        if y == 0 {
            bedrock
        } else if y == final_height {
            if final_height >= 200 {
                snow
            } else {
                grass
            }
        } else if y <= final_height {
            if height_diff <= 3 {
                dirt
            } else {
                let cave_val = self.cave_noise.get_noise_3d(x as f32, y as f32, z as f32);
                let cave_val = (cave_val + 1.0) / 2.0;

                if (0.7 > cave_val && cave_val > 0.5) && height_diff > 6 {
                    air
                } else if ((0.72 >= cave_val && cave_val >= 0.7)
                    || (0.5 >= cave_val && cave_val >= 0.48))
                    && height_diff > 6
                {
                    bedrock
                } else {
                    stone
                }
            }
        } else {
            air
        }
    }

    pub fn generate_chunk(
        &self,
        materials: &ExpandedMaterialMapping,
        chunk_x: u32,
        chunk_y: u32,
        chunk_z: u32,
    ) -> ExpandedBrick {
        let mut brick = ExpandedBrick::empty();
        let world_x = chunk_x * 8;
        let world_y = chunk_y * 8;
        let world_z = chunk_z * 8;

        for z in 0..8 {
            for x in 0..8 {
                for y in 0..8 {
                    let sample_x = world_x + x;
                    let sample_y = world_y + y;
                    let sample_z = world_z + z;

                    let block_material =
                        self.generate_block(materials, sample_x, sample_y, sample_z);
                    brick.set(x, y, z, block_material);
                }
            }
        }

        brick
    }

    pub fn generate_lod_chunk(
        &self,
        materials: &ExpandedMaterialMapping,
        chunk_x: u32,
        chunk_y: u32,
        chunk_z: u32,
        samples: LodSamples,
    ) -> MaterialId {
        use std::collections::HashMap;

        let world_x = chunk_x * 8;
        let world_y = chunk_y * 8;
        let world_z = chunk_z * 8;

        let samples_per_axis = samples.samples_per_axis();
        let step = 8 / samples_per_axis;

        let mut material_counts = HashMap::new();

        if samples_per_axis == 1 {
            let world_x = chunk_x * 8 + 4; // Center of chunk
            let mut world_y_mid = chunk_y * 8 + 4; // Center of chunk
            let world_z = chunk_z * 8 + 4; // Center of chunk

            let first_mat = self.generate_block(materials, world_x, world_y_mid, world_z);
            let mut mat = first_mat;
            while world_y_mid > world_y && mat == materials.get("air") {
                world_y_mid -= 1;
                mat = self.generate_block(materials, world_x, world_y_mid, world_z);
            }

            return materials.material(mat);
        }
        for x in 0..samples_per_axis {
            let sample_x = world_x + (x * step) + (step / 2);

            for y in 0..samples_per_axis {
                let sample_y = world_y + (y * step) + (step / 2);

                for z in 0..samples_per_axis {
                    let sample_z = world_z + (z * step) + (step / 2);

                    let material = self.generate_block(materials, sample_x, sample_y, sample_z);
                    *material_counts.entry(material).or_insert(0) += 1;
                }
            }
        }

        // Return the most common material
        material_counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(material, _)| materials.material(material))
            .unwrap_or_else(|| materials.material(materials.get("air"))) // Default to air if something goes wrong
    }

    pub fn generate_volume<F>(
        &self,
        brickmap: &BrickMap,
        from: na::Point3<u32>,
        to: na::Point3<u32>,
        center: na::Point3<u32>,
        lod_distance: u32,
        materials: &ExpandedMaterialMapping,
        callback: F,
    ) where
        F: Fn(GeneratedBrick, na::Point3<u32>, BrickHandle) + Send + Sync,
    {
        let coords: Vec<_> = (from.x..to.x)
            .flat_map(|x| (from.y..to.y).flat_map(move |y| (from.z..to.z).map(move |z| (x, y, z))))
            .collect();

        coords.par_iter().for_each(|&(x, y, z)| {
            let at = na::Point3::new(x, y, z);
            let distance = na::distance(&center.cast::<f32>(), &at.cast::<f32>());

            let (brick, handle) = if distance >= lod_distance as f32 {
                let lod = self.generate_lod_chunk(materials, x, y, z, LodSamples::A1);
                if lod == materials.material(materials.get("air")) {
                    let handle = BrickHandle::empty();
                    brickmap.set_handle(handle, at);
                    (GeneratedBrick::None, handle)
                } else {
                    let mut handle = BrickHandle::empty();
                    handle.write_lod(lod);
                    brickmap.set_handle(handle, at);
                    (GeneratedBrick::Lod(lod), handle)
                }
            } else {
                let expanded_brick = self.generate_chunk(materials, x, y, z);
                let brick = expanded_brick.to_trace_brick();

                if brick.is_empty() {
                    let handle = BrickHandle::empty();
                    brickmap.set_handle(handle, at);
                    (GeneratedBrick::None, handle)
                } else {
                    let handle = brickmap.get_or_push_brick(brick, at);
                    (GeneratedBrick::Brick(expanded_brick), handle)
                }
            };

            callback(brick, at, handle);
        });
    }
}
