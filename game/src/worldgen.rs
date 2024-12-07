use fastnoise_lite::{FastNoiseLite, FractalType, NoiseType};
use parking_lot::Mutex;
use rayon::prelude::*;

use crate::brick::{Brick, BrickHandle, BrickMap};

pub struct WorldGenerator {
    terrain_noise: FastNoiseLite,
    mountain_noise: FastNoiseLite,
    mountain_shape_noise: FastNoiseLite,
    cave_noise: FastNoiseLite,
}

impl WorldGenerator {
    pub fn new(seed: Option<i32>) -> Self {
        // Base terrain noise
        let mut terrain_noise = FastNoiseLite::new();
        terrain_noise.set_noise_type(Some(NoiseType::Perlin));
        terrain_noise.set_seed(seed);
        terrain_noise.set_frequency(Some(0.005)); // Lower frequency for gentler terrain
        terrain_noise.set_fractal_type(Some(FractalType::FBm));
        terrain_noise.set_fractal_octaves(Some(4));
        terrain_noise.set_fractal_lacunarity(Some(2.0));
        terrain_noise.set_fractal_gain(Some(0.5));

        // Mountain noise (separate to make mountains more sparse)
        let mut mountain_noise = FastNoiseLite::new();
        mountain_noise.set_noise_type(Some(NoiseType::OpenSimplex2));
        mountain_noise.set_seed(seed.map(|s| s + 1));
        mountain_noise.set_frequency(Some(0.003)); // Much lower frequency for sparse mountains
        mountain_noise.set_fractal_type(Some(FractalType::FBm));
        mountain_noise.set_fractal_octaves(Some(3));
        mountain_noise.set_fractal_lacunarity(Some(2.0));
        mountain_noise.set_fractal_gain(Some(0.5));

        let mut mountain_shape_noise = FastNoiseLite::new();
        mountain_shape_noise.set_noise_type(Some(NoiseType::OpenSimplex2));
        mountain_shape_noise.set_seed(seed.map(|s| s + 2));
        mountain_shape_noise.set_frequency(Some(0.004));

        // Cave noise
        let mut cave_noise = FastNoiseLite::new();
        cave_noise.set_noise_type(Some(NoiseType::OpenSimplex2));
        cave_noise.set_seed(seed.map(|s| s + 3));
        cave_noise.set_frequency(Some(0.05)); // Higher frequency for smaller cave features

        Self {
            terrain_noise,
            mountain_noise,
            mountain_shape_noise,
            cave_noise,
        }
    }

    pub fn generate_chunk(&self, chunk_x: u32, chunk_y: u32, chunk_z: u32) -> Brick {
        let mut brick = Brick::empty();
        let world_x = chunk_x as f32 * 8.0;
        let world_z = chunk_z as f32 * 8.0;

        // Calculate base heights once for the chunk
        let heights = [
            self.get_height(world_x, world_z),
            self.get_height(world_x + 8.0, world_z),
            self.get_height(world_x, world_z + 8.0),
            self.get_height(world_x + 8.0, world_z + 8.0),
        ];

        for x in 0..8 {
            for z in 0..8 {
                let fx = x as f32 / 8.0;
                let fz = z as f32 / 8.0;

                // Bilinear interpolation for smooth height
                let h1 = heights[0] * (1.0 - fx) + heights[1] * fx;
                let h2 = heights[2] * (1.0 - fx) + heights[3] * fx;
                let height = (h1 * (1.0 - fz) + h2 * fz) as u32;

                let local_height = if height > chunk_y * 8 {
                    std::cmp::min(8, height - chunk_y * 8)
                } else {
                    0
                };

                // Cave generation with 3D noise and improved cave system
                for y in 0..local_height {
                    let world_y = chunk_y * 8 + y;

                    // Get main cave noise
                    let cave_value = self.cave_noise.get_noise_3d(
                        world_x + x as f32,
                        world_y as f32,
                        world_z + z as f32,
                    );

                    // Create larger caves by combining multiple noise samples
                    let cave_thickness = self.cave_noise.get_noise_3d(
                        (world_x + x as f32) * 1.5,
                        world_y as f32 * 1.5,
                        (world_z + z as f32) * 1.5,
                    );

                    // Cave size increases with depth
                    let depth_factor = ((height as f32 - world_y as f32) / 100.0).min(1.0);
                    let cave_threshold = 0.6 - (depth_factor * 0.2); // Bigger caves deeper down

                    // Combine both noise values for more interesting cave shapes
                    let is_cave = world_y < height &&
                                world_y > 50 && // Don't generate caves too close to surface
                                (cave_value + cave_thickness * 0.5) > cave_threshold;

                    brick.set(x, y, z, !is_cave);
                }
            }
        }

        brick
    }

    fn get_height(&self, x: f32, z: f32) -> f32 {
        let base_height = 200.0;

        // Get base terrain with some variation
        let terrain_val = self.terrain_noise.get_noise_2d(x, z);
        let terrain_height = ((terrain_val + 1.0) * 0.5) * 100.0;

        // Mountain generation with more variety
        let mountain_val = self.mountain_noise.get_noise_2d(x, z);
        let shape_val = self.mountain_shape_noise.get_noise_2d(x * 1.5, z * 1.5);

        let mountain_height = if mountain_val > 0.2 {
            // Create different mountain types based on shape noise
            let shape_factor = ((shape_val + 1.0) * 0.5).powf(0.7);

            // Different mountain types
            let peak_height = ((mountain_val - 0.2) * 1.25).powf(1.8) * 400.0; // Sharp peaks
            let plateau_height = ((mountain_val - 0.2) * 1.25).powf(0.5) * 250.0; // Plateaus
            let rolling_height = ((mountain_val - 0.2) * 1.25).powf(1.2) * 300.0; // Rolling hills

            // Mix mountain types based on shape noise
            let mix1 = shape_factor * peak_height + (1.0 - shape_factor) * plateau_height;
            let mix2 = shape_factor * rolling_height + (1.0 - shape_factor) * mix1;

            // Add some noise to the final height
            mix2 * (1.0 + shape_val * 0.2)
        } else {
            0.0
        };

        // Smoother mountain bases
        let mountain_base = if mountain_val > 0.0 {
            mountain_val.powf(0.6) * 80.0
        } else {
            0.0
        };

        base_height + terrain_height + mountain_height + mountain_base
    }

    pub fn generate_volume(
        &self,
        brickmap: &BrickMap,
        from: na::Vector3<u32>,
        to: na::Vector3<u32>,
    ) {
        let coords: Vec<_> = (from.x..to.x)
            .flat_map(|x| (from.y..to.y).flat_map(move |y| (from.z..to.z).map(move |z| (x, y, z))))
            .collect();

        // Process chunks in parallel
        coords.par_iter().for_each(|&(x, y, z)| {
            let brick = self.generate_chunk(x, y, z);

            if !brick.is_empty() {
                let at = na::Vector3::new(x, y, z);
                let _ = brickmap.get_or_push_brick(brick, at);
            }
        });
    }
}
