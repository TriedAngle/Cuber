use crate::brick::BrickMap;
use nalgebra as na;
use rayon::prelude::*;

pub fn distance_field_parallel_pass(
    brickmap: &BrickMap,
    from: na::Point3<u32>,
    to: na::Point3<u32>,
) {
    let coords: Vec<_> = (from.x..to.x)
        .flat_map(|x| (from.y..to.y).flat_map(move |y| (from.z..to.z).map(move |z| (x, y, z))))
        .collect();

    let boundary_points: Vec<_> = coords
        .par_iter()
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
            } else {
                None
            }
        })
        .collect();

    let boundary_points = boundary_points;

    coords
        .par_iter()
        .filter(|&&coord| {
            let at = na::Point3::<u32>::new(coord.0, coord.1, coord.2);
            let brick = brickmap.get_handle(at);
            brick.is_empty() && !boundary_points.contains(&coord)
        })
        .for_each(|&(x, y, z)| {
            let at = na::Point3::<u32>::new(x, y, z);
            let brick = brickmap.get_handle(at);

            // Chebyshev distance
            let mut min_distance = boundary_points
                .par_iter()
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
        });
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
