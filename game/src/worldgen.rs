use std::collections::HashMap;

use fastnoise_lite::{FastNoiseLite, FractalType, NoiseType};
use parking_lot::RwLock;
use rayon::prelude::*;

use crate::{
    brick::{BrickHandle, BrickMap, ExpandedBrick},
    material::{ExpandedMaterialMapping, MaterialId, MaterialRegistry},
};

pub struct WorldGenerator {
    terrain_noise: FastNoiseLite,
    continent_noise: FastNoiseLite,
    cave_noise: FastNoiseLite,
}

impl WorldGenerator {
    pub fn new() -> Self {
        // Terrain noise setup remains the same
        let mut terrain_noise = FastNoiseLite::new();
        terrain_noise.set_noise_type(Some(NoiseType::Perlin));
        terrain_noise.set_seed(Some(2324));
        terrain_noise.set_frequency(Some(0.005));
        terrain_noise.set_fractal_type(Some(FractalType::FBm));
        terrain_noise.set_fractal_octaves(Some(4));
        terrain_noise.set_fractal_lacunarity(Some(2.0));
        terrain_noise.set_fractal_gain(Some(0.5));

        // Continent noise setup remains the same
        let mut continent_noise = FastNoiseLite::new();
        continent_noise.set_noise_type(Some(NoiseType::Perlin));
        continent_noise.set_seed(Some(9999));
        continent_noise.set_frequency(Some(0.0005));
        continent_noise.set_fractal_type(Some(FractalType::FBm));
        continent_noise.set_fractal_octaves(Some(3));
        continent_noise.set_fractal_lacunarity(Some(2.0));
        continent_noise.set_fractal_gain(Some(0.5));

        // Cave noise setup remains the same
        let mut cave_noise = FastNoiseLite::new();
        cave_noise.set_noise_type(Some(NoiseType::Perlin));
        cave_noise.set_seed(Some(12345));
        cave_noise.set_frequency(Some(0.03));
        cave_noise.set_fractal_type(Some(FractalType::FBm));
        cave_noise.set_fractal_octaves(Some(3));
        cave_noise.set_fractal_lacunarity(Some(2.0));
        cave_noise.set_fractal_gain(Some(0.5));

        // Initialize material mapping
        let generator = Self {
            terrain_noise,
            continent_noise,
            cave_noise,
        };

        generator
    }

    pub fn generate_chunk(
        &self,
        materials: &ExpandedMaterialMapping,
        chunk_x: u32,
        chunk_y: u32,
        chunk_z: u32,
    ) -> ExpandedBrick {
        let mut brick = ExpandedBrick::empty();
        let world_x = chunk_x as f32 * 8.0;
        let world_y = chunk_y as f32 * 8.0;
        let world_z = chunk_z as f32 * 8.0;

        // Get local IDs
        let air_id = materials.get("air");
        let stone_id = materials.get("stone");
        let bedrock_id = materials.get("bedrock");
        let dirt_id = materials.get("dirt");
        let grass_id = materials.get("grass");
        let snow_id = materials.get("snow");

        // Generation logic remains the same but uses local IDs
        for z in 0..8 {
            for x in 0..8 {
                let sample_x = world_x + x as f32;
                let sample_z = world_z + z as f32;

                let continent_val = self.continent_noise.get_noise_2d(sample_x, sample_z);
                let terrain_val = self.terrain_noise.get_noise_2d(sample_x, sample_z);

                let final_height =
                    (100.0 + (continent_val * 150.0) + (terrain_val * 180.0).round()) as i32;

                for y in 0..8 {
                    let world_block_y = world_y as i32 + y as i32;
                    let height_diff = final_height - world_block_y;
                    let mut block_material = air_id;

                    if world_block_y == 0 {
                        block_material = bedrock_id;
                    } else if world_block_y == final_height {
                        block_material = grass_id;
                        if final_height >= 200 {
                            block_material = snow_id;
                        }
                    } else if world_block_y <= final_height {
                        block_material = stone_id;
                        if height_diff <= 3 {
                            block_material = dirt_id;
                        }
                    }

                    if block_material == stone_id {
                        let cave_val =
                            self.cave_noise
                                .get_noise_3d(sample_x, world_block_y as f32, sample_z);
                        let cave_val = (cave_val + 1.0) / 2.0;
                        if (0.7 > cave_val && cave_val > 0.5) && height_diff > 6 {
                            block_material = air_id;
                        }
                        if ((0.72 >= cave_val && cave_val >= 0.7)
                            || (0.5 >= cave_val && cave_val >= 0.48))
                            && height_diff > 6
                        {
                            block_material = bedrock_id
                        }
                    }

                    brick.set(x, y, z, block_material);
                }
            }
        }

        brick
    }

    pub fn generate_volume<F>(
        &self,
        brickmap: &BrickMap,
        from: na::Vector3<u32>,
        to: na::Vector3<u32>,
        materials: &ExpandedMaterialMapping,
        callback: F,
    ) where
        F: Fn(&ExpandedBrick, na::Vector3<u32>, BrickHandle) + Send + Sync,
    {
        let coords: Vec<_> = (from.x..to.x)
            .flat_map(|x| (from.y..to.y).flat_map(move |y| (from.z..to.z).map(move |z| (x, y, z))))
            .collect();

        // Process chunks in parallel
        coords.par_iter().for_each(|&(x, y, z)| {
            let expanded_brick = self.generate_chunk(materials, x, y, z);
            let brick = expanded_brick.to_trace_brick();

            let at = na::Vector3::new(x, y, z);

            let handle = if brick.is_empty() {
                let handle = BrickHandle::empty();
                brickmap.set_handle(handle, at);
                handle
            } else {
                brickmap.get_or_push_brick(brick, at)
            };

            callback(&expanded_brick, at, handle);
        });
    }
}
