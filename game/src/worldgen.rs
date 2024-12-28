use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::SystemTime,
};

use fastnoise_lite::{FastNoiseLite, FractalType, NoiseType};
use rayon::prelude::*;

use crate::{
    brick::{BrickHandle, BrickMap, ExpandedBrick, MaterialBrick},
    material::{ExpandedMaterialMapping, MaterialId},
    palette::{PaletteId, PaletteRegistry},
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
    base_terrain: FastNoiseLite,
    mountain_noise: FastNoiseLite,
    mountain_mask: FastNoiseLite,
    mountain_blend: FastNoiseLite,
    cheese_cave_noise: FastNoiseLite,
    spaghetti_cave_noise: FastNoiseLite,
    spaghetti_size_noise: FastNoiseLite,
    seed: i32,
}

impl WorldGenerator {
    pub fn new(seed: Option<i32>) -> Self {
        let base_seed = seed.unwrap_or(420);
        let mut generator = WorldGenerator {
            base_terrain: FastNoiseLite::new(),
            mountain_noise: FastNoiseLite::new(),
            mountain_mask: FastNoiseLite::new(),
            mountain_blend: FastNoiseLite::new(),
            cheese_cave_noise: FastNoiseLite::new(),
            spaghetti_cave_noise: FastNoiseLite::new(),
            spaghetti_size_noise: FastNoiseLite::new(),
            seed: base_seed,
        };

        generator
            .base_terrain
            .set_noise_type(Some(NoiseType::Perlin));
        generator.base_terrain.set_seed(Some(base_seed));
        generator.base_terrain.set_frequency(Some(0.0025));
        generator
            .base_terrain
            .set_fractal_type(Some(FractalType::FBm));
        generator.base_terrain.set_fractal_octaves(Some(4));
        generator.base_terrain.set_fractal_lacunarity(Some(2.0));
        generator.base_terrain.set_fractal_gain(Some(0.5));

        generator
            .mountain_noise
            .set_noise_type(Some(NoiseType::Perlin));
        generator.mountain_noise.set_seed(Some(base_seed + 1));
        generator.mountain_noise.set_frequency(Some(0.005));
        generator
            .mountain_noise
            .set_fractal_type(Some(FractalType::FBm));
        generator.mountain_noise.set_fractal_octaves(Some(5));
        generator.mountain_noise.set_fractal_lacunarity(Some(2.5));
        generator.mountain_noise.set_fractal_gain(Some(0.6));

        generator
            .mountain_mask
            .set_noise_type(Some(NoiseType::Perlin));
        generator.mountain_mask.set_seed(Some(base_seed + 2));
        generator.mountain_mask.set_frequency(Some(0.00125));
        generator
            .mountain_mask
            .set_fractal_type(Some(FractalType::FBm));
        generator.mountain_mask.set_fractal_octaves(Some(2));

        generator
            .mountain_blend
            .set_noise_type(Some(NoiseType::Perlin));
        generator.mountain_blend.set_seed(Some(base_seed + 3));
        generator.mountain_blend.set_frequency(Some(0.00375));

        generator
            .cheese_cave_noise
            .set_noise_type(Some(NoiseType::Perlin));
        generator.cheese_cave_noise.set_seed(Some(base_seed + 4));
        generator.cheese_cave_noise.set_frequency(Some(0.005));
        generator
            .cheese_cave_noise
            .set_fractal_type(Some(FractalType::FBm));
        generator.cheese_cave_noise.set_fractal_octaves(Some(3));
        generator
            .cheese_cave_noise
            .set_fractal_lacunarity(Some(2.0));
        generator.cheese_cave_noise.set_fractal_gain(Some(0.5));

        generator
            .spaghetti_cave_noise
            .set_noise_type(Some(NoiseType::Perlin));
        generator.spaghetti_cave_noise.set_seed(Some(base_seed + 5));
        generator.spaghetti_cave_noise.set_frequency(Some(0.0125));
        generator
            .spaghetti_cave_noise
            .set_fractal_type(Some(FractalType::FBm));
        generator.spaghetti_cave_noise.set_fractal_octaves(Some(2));

        generator
            .spaghetti_size_noise
            .set_noise_type(Some(NoiseType::Perlin));
        generator.spaghetti_size_noise.set_seed(Some(base_seed + 6));
        generator.spaghetti_size_noise.set_frequency(Some(0.0075));

        generator
    }

    pub fn generate_block(&self, m: &ExpandedMaterialMapping, x: u32, y: u32, z: u32) -> u8 {
        let height = self.get_height(x as f32, z as f32);
        let current_y = y as f32;

        if current_y > height {
            return m.get("air");
        }

        if self.is_cave(x as f32, y as f32, z as f32) {
            return m.get("air");
        }

        let snow_height = height - 5.0;
        if current_y >= snow_height && height > 300.0 {
            return m.get("snow");
        }

        if current_y >= height - 1.0 {
            return m.get("grass");
        }

        if current_y >= height - 4.0 {
            return m.get("dirt");
        }

        if self.is_near_cave(x as f32, y as f32, z as f32) {
            return m.get("bedrock");
        }

        m.get("stone")
    }

    fn is_cave(&self, x: f32, y: f32, z: f32) -> bool {
        let height = self.get_height(x, z);
        if y > height - 10.0 {
            return false;
        }

        let cheese_value = self.cheese_cave_noise.get_noise_3d(x, y, z);
        if cheese_value > 0.7 {
            let size = (cheese_value - 0.7) * 133.33;
            if size > 10.0 {
                return true;
            }
        }

        let spaghetti_value = self.spaghetti_cave_noise.get_noise_3d(x, y, z);
        let size_variation = self.spaghetti_size_noise.get_noise_3d(x, y, z);

        let tunnel_size = 0.5 + (size_variation + 1.0) * 3.75;

        if spaghetti_value >= -0.1 && spaghetti_value <= 0.1 {
            let distance_from_center = spaghetti_value.abs() * 10.0;
            return distance_from_center * 8.0 <= tunnel_size;
        }

        false
    }

    fn get_height(&self, x: f32, z: f32) -> f32 {
        let base_height = self.base_terrain.get_noise_2d(x, z);
        let base_scaled = (base_height + 1.0) * 50.0 + 200.0;

        let mountain_height = self.mountain_noise.get_noise_2d(x, z);
        let mountain_scaled = (mountain_height + 1.0) * 100.0 + 200.0;

        let mask = (self.mountain_mask.get_noise_2d(x, z) + 1.0) * 0.5;

        let blend = (self.mountain_blend.get_noise_2d(x, z) + 1.0) * 0.5;

        let mountain_influence = (mask * blend).powf(2.0);
        let height =
            base_scaled * (1.0 - mountain_influence) + mountain_scaled * mountain_influence;

        let peak_variation = if height > 250.0 {
            let peak_noise = self.mountain_blend.get_noise_2d(x * 2.0, z * 2.0);
            peak_noise * 15.0
        } else {
            0.0
        };

        height + peak_variation
    }

    fn is_near_cave(&self, x: f32, y: f32, z: f32) -> bool {
        // Check surrounding blocks for cave proximity
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if dx == 0 && dy == 0 && dz == 0 {
                        continue;
                    }
                    if self.is_cave(x + dx as f32, y + dy as f32, z + dz as f32) {
                        return true;
                    }
                }
            }
        }
        false
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

    pub fn generate_volume<F, C>(
        &self,
        brickmap: &BrickMap,
        from: na::Point3<u32>,
        to: na::Point3<u32>,
        center: na::Point3<u32>,
        lod_distance: u32,
        materials: &ExpandedMaterialMapping,
        palettes: &PaletteRegistry,
        chunk_size: u32,
        callback: F,
        chunk_callback: C,
    ) where
        F: Fn(&GeneratedBrick, BrickHandle, na::Point3<u32>, f64) + Send + Sync,
        C: Fn(Vec<MaterialBrick>, Vec<PaletteId>, Vec<BrickHandle>, Vec<na::Point3<u32>>, f64)
            + Send
            + Sync,
    {
        let chunk_dims = na::Point3::new(
            ((to.x - from.x + chunk_size - 1) / chunk_size).max(1),
            ((to.y - from.y + chunk_size - 1) / chunk_size).max(1),
            ((to.z - from.z + chunk_size - 1) / chunk_size).max(1),
        );

        let total_volume = ((to.x - from.x) * (to.y - from.y) * (to.z - from.z)) as u64;
        let start = SystemTime::now();
        log::info!(
            "Starting Generating Volume [{}] {} -> {}",
            total_volume,
            from,
            to
        );

        let processed = Arc::new(AtomicU64::new(0));
        let last_percentage = Arc::new(AtomicU64::new(0));

        let mut chunk_coords: Vec<_> = (0..chunk_dims.x)
            .flat_map(|cx| {
                (0..chunk_dims.y).flat_map(move |cy| {
                    (0..chunk_dims.z).map(move |cz| {
                        let chunk_pos = na::Point3::new(
                            from.x + cx * chunk_size,
                            from.y + cy * chunk_size,
                            from.z + cz * chunk_size,
                        );
                        let dist_squared = (chunk_pos.x as i64 - center.x as i64).pow(2)
                            + (chunk_pos.y as i64 - center.y as i64).pow(2)
                            + (chunk_pos.z as i64 - center.z as i64).pow(2);
                        ((cx, cy, cz), dist_squared)
                    })
                })
            })
            .collect();

        // Sort by distance (closest first)
        chunk_coords.sort_by_key(|&(_, dist)| dist);

        // Process chunks in sorted order
        chunk_coords
            .par_iter()
            .map(|&((cx, cy, cz), _)| (cx, cy, cz))
            .for_each(|(cx, cy, cz)| {
                let chunk_start = na::Point3::new(
                    from.x + cx * chunk_size,
                    from.y + cy * chunk_size,
                    from.z + cz * chunk_size,
                );
                let chunk_end = na::Point3::new(
                    (from.x + (cx + 1) * chunk_size).min(to.x),
                    (from.y + (cy + 1) * chunk_size).min(to.y),
                    (from.z + (cz + 1) * chunk_size).min(to.z),
                );

                let mut chunk_bricks = Vec::new();
                let mut chunk_palettes = Vec::new();
                let mut chunk_positions = Vec::new();
                let mut chunk_handles = Vec::new();

                for x in chunk_start.x..chunk_end.x {
                    for y in chunk_start.y..chunk_end.y {
                        for z in chunk_start.z..chunk_end.z {
                            let at = na::Point3::new(x, y, z);
                            let distance = na::distance(&center.cast::<f32>(), &at.cast::<f32>());
                            let (brick, handle) = if distance >= lod_distance as f32 {
                                let lod =
                                    self.generate_lod_chunk(materials, x, y, z, LodSamples::A1);
                                if lod == materials.material(materials.get("air")) {
                                    let handle = BrickHandle::empty();
                                    brickmap.set_handle(handle, at);
                                    (GeneratedBrick::None, handle)
                                } else {
                                    let mut handle = BrickHandle::empty();
                                    handle.set_lod(true);
                                    handle.set_empty_value(lod.0);
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

                                    let (mut compressed, materials) =
                                        expanded_brick.compress(&materials);

                                    let palette_id = palettes.register_palette(materials);

                                    compressed.set_meta_value(palette_id.0);
                                    chunk_bricks.push(compressed);
                                    chunk_palettes.push(palette_id);
                                    chunk_positions.push(at);
                                    chunk_handles.push(handle);
                                    (GeneratedBrick::Brick(expanded_brick), handle)
                                }
                            };

                            let current = processed.fetch_add(1, Ordering::Relaxed);
                            let percentagef = (current as f64 * 100.0) / total_volume as f64;
                            let percentage = (current * 100) / total_volume;
                            let last = last_percentage.load(Ordering::Relaxed);
                            if percentage > last
                                && last_percentage
                                    .compare_exchange(
                                        last,
                                        percentage,
                                        Ordering::Relaxed,
                                        Ordering::Relaxed,
                                    )
                                    .is_ok()
                            {
                                log::info!("World Generation Progress: {}%", percentage);
                            }

                            callback(&brick, handle, at, percentagef);
                        }
                    }
                }

                let current = processed.load(Ordering::Relaxed);
                let percentagef = (current as f64 * 100.0) / total_volume as f64;
                chunk_callback(
                    chunk_bricks,
                    chunk_palettes,
                    chunk_handles,
                    chunk_positions,
                    percentagef,
                );
            });

        log::info!(
            "Finish Generation of Volume [{}] took: {:.3}s",
            total_volume,
            start.elapsed().unwrap().as_secs_f64()
        );
    }
}
