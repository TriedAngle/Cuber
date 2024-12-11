use fastnoise_lite::{FastNoiseLite, FractalType, NoiseType};
use rayon::prelude::*;

use crate::brick::{BrickHandle, BrickMap, ExpandedBrick};

// Example material IDs (you can change these as needed)
const AIR: u8 = 0;
const STONE: u8 = 1;
const BEDROCK: u8 = 2;
const DIRT: u8 = 3;
const GRASS: u8 = 4;
const SNOW: u8 = 5;

pub struct WorldGenerator {
    terrain_noise: FastNoiseLite,
    continent_noise: FastNoiseLite,
    cave_noise: FastNoiseLite,
}

impl WorldGenerator {
    pub fn new() -> Self {
        // Terrain noise: controls overall height variations (like Minecraft terrain)
        let mut terrain_noise = FastNoiseLite::new();
        terrain_noise.set_noise_type(Some(NoiseType::Perlin));
        terrain_noise.set_seed(Some(2324));
        terrain_noise.set_frequency(Some(0.005));
        terrain_noise.set_fractal_type(Some(FractalType::FBm));
        terrain_noise.set_fractal_octaves(Some(4));
        terrain_noise.set_fractal_lacunarity(Some(2.0));
        terrain_noise.set_fractal_gain(Some(0.5));

        // Continent noise: Large-scale continentalness factor
        // This can be used to modulate the terrain height on a larger scale,
        // making some areas have higher "continents" and others be lower (like oceans).
        let mut continent_noise = FastNoiseLite::new();
        continent_noise.set_noise_type(Some(NoiseType::Perlin));
        continent_noise.set_seed(Some(9999));
        continent_noise.set_frequency(Some(0.0005));
        continent_noise.set_fractal_type(Some(FractalType::FBm));
        continent_noise.set_fractal_octaves(Some(3));
        continent_noise.set_fractal_lacunarity(Some(2.0));
        continent_noise.set_fractal_gain(Some(0.5));

        // Cave noise: 3D noise that carves out caves inside the terrain
        let mut cave_noise = FastNoiseLite::new();
        cave_noise.set_noise_type(Some(NoiseType::Perlin));
        cave_noise.set_seed(Some(12345));
        cave_noise.set_frequency(Some(0.03));
        cave_noise.set_fractal_type(Some(FractalType::FBm));
        cave_noise.set_fractal_octaves(Some(3));
        cave_noise.set_fractal_lacunarity(Some(2.0));
        cave_noise.set_fractal_gain(Some(0.5));

        Self {
            terrain_noise,
            continent_noise,
            cave_noise,
        }
    }

    pub fn generate_chunk(&self, chunk_x: u32, chunk_y: u32, chunk_z: u32) -> ExpandedBrick {
        let mut brick = ExpandedBrick::empty();
        let world_x = chunk_x as f32 * 8.0;
        let world_y = chunk_y as f32 * 8.0;
        let world_z = chunk_z as f32 * 8.0;

        // We'll generate a heightmap and fill materials accordingly.
        // Average height is around 100. We'll add continental and terrain variation.
        // Terrain noise is 2D (x,z). We'll use world_y and cave noise to determine caves.

        // We can consider:
        // height = 100 (base)
        //         + (continent_noise * 100) for large-scale variation
        //         + (terrain_noise * 50) for local hills and mountains
        //
        // The final height might be something like:
        //   final_height = 100 + continent_val * 100.0 + terrain_val * 50.0
        //
        // If world_y + y < final_height, normally fill with stone/dirt/grass/snow.
        // Near the surface, put grass or snow if high enough.
        // y=0 bedrock layer.
        // For caves: if cave_noise at a voxel > threshold, carve out to air.

        // return ExpandedBrick::random(5);
        for z in 0..8 {
            for x in 0..8 {
                // Sample the 2D noises for height determination
                let sample_x = world_x + x as f32;
                let sample_z = world_z + z as f32;

                let continent_val = self.continent_noise.get_noise_2d(sample_x, sample_z);
                let terrain_val = self.terrain_noise.get_noise_2d(sample_x, sample_z);

                let final_height =
                    (100.0 + (continent_val * 100.0) + (terrain_val * 50.0).round()) as i32;

                for y in 0..8 {
                    let world_block_y = world_y as i32 + y as i32;

                    let height_diff = final_height - world_block_y;

                    let mut block_material = AIR;

                    if world_block_y == 0 {
                        block_material = BEDROCK;
                    } else if world_block_y <= final_height {
                        block_material = STONE;

                        if height_diff <= 3 {
                            block_material = DIRT;
                        }
                        //         if final_height > 150 {
                        //             block_material = SNOW;
                        //         } else if final_height >= 95 {
                        //             block_material = GRASS;
                        //         } else {
                        //             block_material = GRASS;
                        //         }
                        //     } else if height_diff == 1 {
                        //         block_material = DIRT;
                        //     } else if height_diff > 1 {
                        //         block_material = STONE;
                        //     }
                        // } else {
                        //     block_material = AIR;
                    }

                    if block_material == STONE || block_material == DIRT {
                        let cave_val =
                            self.cave_noise
                                .get_noise_3d(sample_x, world_block_y as f32, sample_z);
                        let cave_val = (cave_val + 1.0) / 2.0;
                        // If cave_val is high enough, we carve out a cave
                        if 0.9 > cave_val && cave_val > 0.5 {
                            block_material = AIR;
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
        callback: F,
    ) where
        F: Fn(&ExpandedBrick, na::Vector3<u32>, BrickHandle) + Send + Sync,
    {
        let coords: Vec<_> = (from.x..to.x)
            .flat_map(|x| (from.y..to.y).flat_map(move |y| (from.z..to.z).map(move |z| (x, y, z))))
            .collect();

        // Process chunks in parallel
        coords.par_iter().for_each(|&(x, y, z)| {
            let expanded_brick = self.generate_chunk(x, y, z);
            let brick = expanded_brick.to_trace_brick();

            let at = na::Vector3::new(x, y, z);

            if brick.is_empty() {
                brickmap.set_handle(BrickHandle::empty(), at);
                return;
            }

            let handle = brickmap.get_or_push_brick(brick, at);
            callback(&expanded_brick, at, handle);
        });
    }
}
