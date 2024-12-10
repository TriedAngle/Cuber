use fastnoise_lite::{FastNoiseLite, FractalType, NoiseType};
use rayon::prelude::*;

use crate::brick::{BrickHandle, BrickMap, ExpandedBrick};

pub const MATERIAL_AIR: u8 = 0;
pub const MATERIAL_STONE: u8 = 1;
pub const MATERIAL_BEDROCK: u8 = 2;
// pub const MATERIAL_GRASS: u8 = 1;
// pub const MATERIAL_DIRT: u8 = 2;

// pub const MATERIAL_LIGHTSTONE: u8 = 4;
// pub const MATERIAL_SNOW: u8 = 5;


pub struct WorldGenerator {
    terrain_noise: FastNoiseLite,
    cave_noise: FastNoiseLite,
    mountain_noise: FastNoiseLite,
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

        let mut cave_noise = FastNoiseLite::new();
        cave_noise.set_noise_type(Some(NoiseType::OpenSimplex2));
        cave_noise.set_seed(Some(2325));
        cave_noise.set_frequency(Some(0.02));
        cave_noise.set_fractal_octaves(Some(1));

        let mut mountain_noise = FastNoiseLite::new();
        mountain_noise.set_noise_type(Some(NoiseType::Perlin));
        mountain_noise.set_seed(Some(2326));
        mountain_noise.set_frequency(Some(0.002)); // Increased scale of mountains
        mountain_noise.set_fractal_octaves(Some(2));

        Self {
            terrain_noise,
            cave_noise,
            mountain_noise,
        }
    }

    pub fn generate_chunk(&self, chunk_x: u32, chunk_y: u32, chunk_z: u32) -> ExpandedBrick {
        let mut brick = ExpandedBrick::empty();
        let world_x = chunk_x as f32 * 8.0;
        let world_y = chunk_y as f32 * 8.0;
        let world_z = chunk_z as f32 * 8.0;

        for x in 0..8 {
            for z in 0..8 {
                let wx = world_x + x as f32;
                let wz = world_z + z as f32;

                let base_height = (self.terrain_noise.get_noise_2d(wx, wz) * 30.0 + 500.0) as i32;

                let mountain_factor = self.mountain_noise.get_noise_2d(wx, wz);
                let mountain_height = if mountain_factor > 0.0 { // Lowered threshold from 0.3 to 0.0
                    let factor = (mountain_factor + 0.2) / 1.2; // Adjusted normalization for new range
                    (factor * 200.0) as i32
                } else {
                    0
                };

                let height = base_height + mountain_height;

                for y in 0..8 {
                    let wy = world_y + y as f32;
                    let world_height = wy as i32;

                    // Enhanced cave generation
                    let cave_value = self.cave_noise.get_noise_3d(wx, wy, wz);
                    
                    // More varied cave shapes and ground connections
                    let cave_threshold = if world_height < 300 {
                        0.8
                    } else if world_height < height - 10 {
                        0.4
                    } else {
                        0.1
                    };
                    
                    let is_cave = cave_value > cave_threshold || cave_value < -cave_threshold;
                    
                    let material = if world_height > height {
                        MATERIAL_AIR
                    } else if is_cave { // Removed upper height check to allow surface caves
                        MATERIAL_AIR
                    // } else if world_height <= 5 {
                    //     MATERIAL_BEDROCK
                    // } else if world_height == height {
                    //     MATERIAL_GRASS
                    // } else if world_height >= height - 3 {
                    //     MATERIAL_DIRT
                    // } else if world_height <= height - 10 {
                    //     MATERIAL_STONE
                    } else {
                        MATERIAL_STONE
                    };

                    brick.set(x as u32, y as u32, z as u32, material);
                }
            }
        }

        brick
    }

    #[allow(dead_code)]
    pub fn set_seed(&mut self, seed: i32) {
        self.terrain_noise.set_seed(Some(seed));
        self.cave_noise.set_seed(Some(seed + 1));
        self.mountain_noise.set_seed(Some(seed + 2));
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
            if brick.is_empty() { 
                return;
            }
            let at = na::Vector3::new(x, y, z);
            let handle = brickmap.get_or_push_brick(brick, at);
            callback(&expanded_brick, at, handle);
        });
    }
}

