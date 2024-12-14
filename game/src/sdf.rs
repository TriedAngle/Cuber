use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::SystemTime,
};

use crate::brick::BrickMap;
use nalgebra as na;
use rayon::prelude::*;

pub fn distance_field_parallel_pass(
    brickmap: &BrickMap,
    from: na::Point3<u32>,
    to: na::Point3<u32>,
    progress_interval: u64,
    progress_callback: impl Fn(u64) + Send + Sync,
) {
    let coords: Vec<_> = (from.x..to.x)
        .flat_map(|x| (from.y..to.y).flat_map(move |y| (from.z..to.z).map(move |z| (x, y, z))))
        .collect();
    let total_volume = coords.len() as u64;
    let start = SystemTime::now();

    log::info!(
        "Starting Distance Field Generation [{}] {} -> {}",
        total_volume,
        from,
        to
    );

    // Progress tracking for boundary points pass
    let boundary_processed = Arc::new(AtomicU64::new(0));
    let boundary_last_percentage = Arc::new(AtomicU64::new(0));
    let points_to_process = Arc::new(AtomicU64::new(0));

    // Convert boundary points to a more efficient data structure
    let boundary_points: Vec<_> = coords
        .par_iter()
        .inspect(|_| {
            let current = boundary_processed.fetch_add(1, Ordering::Relaxed);
            let percentage = (current * 100) / total_volume;
            let last = boundary_last_percentage.load(Ordering::Relaxed);

            if percentage > last
                && boundary_last_percentage
                    .compare_exchange(last, percentage, Ordering::Relaxed, Ordering::Relaxed)
                    .is_ok()
            {
                log::info!("Boundary Points Pass Progress: {}%", percentage);
            }
        })
        .filter_map(|&(x, y, z)| {
            let at = na::Point3::<u32>::new(x, y, z);
            let brick = brickmap.get_handle(at);
            if !brick.is_empty() {
                return None;
            }
            let neighbors = [
                (x.wrapping_sub(1), y, z),
                (x + 1, y, z),
                (x, y.wrapping_sub(1), z),
                (x, y + 1, z),
                (x, y, z.wrapping_sub(1)),
                (x, y, z + 1),
            ];
            let is_boundary = neighbors.iter().any(|&(nx, ny, nz)| {
                if nx < from.x
                    || nx >= to.x
                    || ny < from.y
                    || ny >= to.y
                    || nz < from.z
                    || nz >= to.z
                {
                    return false;
                }
                let nat = na::Point3::<u32>::new(nx, ny, nz);
                let neighbor = brickmap.get_handle(nat);
                !neighbor.is_empty()
            });
            if is_boundary {
                let mut brick = brick;
                brick.write_sdf(0);
                brickmap.set_handle(brick, at);
                Some((x, y, z))
            } else if brick.is_empty() {
                points_to_process.fetch_add(1, Ordering::Relaxed);
                None
            } else {
                None
            }
        })
        .collect();

    let total_to_process = points_to_process.load(Ordering::Relaxed);
    let boundary_time = start.elapsed().unwrap();
    log::info!("Completed Boundary Points Pass in {:.3}s, found {} boundary points, {} points need processing", 
        boundary_time.as_secs_f64(),
        boundary_points.len(),
        total_to_process
    );

    // Progress tracking for distance calculation pass
    let distance_processed = Arc::new(AtomicU64::new(0));
    let distance_last_percentage = Arc::new(AtomicU64::new(0));
    let distance_start = SystemTime::now();

    // Create chunks for better cache locality
    const CHUNK_SIZE: u32 = 8;
    let chunks: Vec<_> = (from.x..to.x)
        .step_by(CHUNK_SIZE as usize)
        .flat_map(|chunk_x| {
            (from.y..to.y)
                .step_by(CHUNK_SIZE as usize)
                .flat_map(move |chunk_y| {
                    (from.z..to.z)
                        .step_by(CHUNK_SIZE as usize)
                        .map(move |chunk_z| (chunk_x, chunk_y, chunk_z))
                })
        })
        .collect();

    chunks.par_iter().for_each(|&(chunk_x, chunk_y, chunk_z)| {
        // Pre-calculate boundary distances for this chunk
        let chunk_bounds = na::Point3::new(
            chunk_x.saturating_add(CHUNK_SIZE).min(to.x),
            chunk_y.saturating_add(CHUNK_SIZE).min(to.y),
            chunk_z.saturating_add(CHUNK_SIZE).min(to.z),
        );

        let mut chunk_processed = 0u64;

        // Process each point in the chunk
        for x in chunk_x..chunk_bounds.x {
            for y in chunk_y..chunk_bounds.y {
                for z in chunk_z..chunk_bounds.z {
                    let at = na::Point3::<u32>::new(x, y, z);
                    let brick = brickmap.get_handle(at);
                    if !brick.is_empty() || boundary_points.contains(&(x, y, z)) {
                        continue;
                    }

                    // Early termination optimization for Chebyshev distance
                    let mut min_distance = i32::MAX;
                    for &(bx, by, bz) in &boundary_points {
                        let dx = (x as i32 - bx as i32).abs();
                        if dx >= min_distance {
                            continue;
                        }

                        let dy = (y as i32 - by as i32).abs();
                        if dy >= min_distance {
                            continue;
                        }

                        let dz = (z as i32 - bz as i32).abs();
                        if dz >= min_distance {
                            continue;
                        }

                        let dist = dx.max(dy).max(dz);
                        min_distance = min_distance.min(dist);
                    }

                    let mut brick = brick;
                    brick.write_sdf(min_distance.max(0) as u32);
                    brickmap.set_handle(brick, at);

                    chunk_processed += 1;
                }
            }
        }

        if chunk_processed > 0 {
            let current = distance_processed.fetch_add(chunk_processed, Ordering::Relaxed);
            let percentage = (current * 100) / total_to_process;
            let last = distance_last_percentage.load(Ordering::Relaxed);

            if percentage > last
                && percentage >= last + progress_interval
                && distance_last_percentage
                    .compare_exchange(last, percentage, Ordering::Relaxed, Ordering::Relaxed)
                    .is_ok()
            {
                progress_callback(percentage);
                log::info!("Distance Calculation Pass Progress: {}%", percentage);
            }
        }
    });

    let distance_time = distance_start.elapsed().unwrap();
    log::info!("Finished Distance Field Generation [{}] boundary pass: {:.3}s, distance pass: {:.3}s, total: {:.3}s", 
        total_volume,
        boundary_time.as_secs_f64(),
        distance_time.as_secs_f64(),
        start.elapsed().unwrap().as_secs_f64()
    );
}

pub fn distance_field_sequential_pass(
    brickmap: &BrickMap,
    from: na::Point3<u32>,
    to: na::Point3<u32>,
) {
    let coords: Vec<_> = (from.x..to.x)
        .flat_map(|x| (from.y..to.y).flat_map(move |y| (from.z..to.z).map(move |z| (x, y, z))))
        .collect();

    let mut boundary_points = Vec::new();
    for &(x, y, z) in &coords {
        let at = na::Point3::<u32>::new(x, y, z);
        let brick = brickmap.get_handle(at);

        if !brick.is_empty() {
            continue;
        }

        let neighbors = [
            (x.wrapping_sub(1), y, z),
            (x + 1, y, z),
            (x, y.wrapping_sub(1), z),
            (x, y + 1, z),
            (x, y, z.wrapping_sub(1)),
            (x, y, z + 1),
        ];

        let is_boundary = neighbors.iter().any(|&(nx, ny, nz)| {
            if nx < from.x || nx >= to.x || ny < from.y || ny >= to.y || nz < from.z || nz >= to.z {
                return false;
            }
            let nat = na::Point3::<u32>::new(nx, ny, nz);
            let neighbor = brickmap.get_handle(nat);
            !neighbor.is_empty()
        });

        if is_boundary {
            boundary_points.push((x, y, z));
            let mut brick = brick;
            brick.write_sdf(0);
            brickmap.set_handle(brick, at);
        }
    }

    for &(x, y, z) in &coords {
        let at = na::Point3::<u32>::new(x, y, z);
        let brick = brickmap.get_handle(at);

        if !brick.is_empty() || boundary_points.contains(&(x, y, z)) {
            continue;
        }
        // Chebyshev distance
        let mut min_distance = boundary_points
            .iter()
            .map(|&(bx, by, bz)| {
                let dx = (x as i32 - bx as i32).abs();
                let dy = (y as i32 - by as i32).abs();
                let dz = (z as i32 - bz as i32).abs();
                dx.max(dy).max(dz)
            })
            .min()
            .unwrap_or(0);
        if min_distance < 0 {
            min_distance = 0;
        }
        let mut brick = brick;
        brick.write_sdf(min_distance as u32);
        brickmap.set_handle(brick, at);
    }
}
