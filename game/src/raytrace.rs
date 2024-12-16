use crate::{
    brick::{BrickMap, TraceBrick},
    Camera,
};

use nalgebra as na;

#[derive(Debug, Clone, Copy)]
pub struct RayHit {
    pub position: na::Point3<f32>,
    pub normal: na::Vector3<f32>,
    pub distance: f32,
    pub pos: na::Point3<f32>,
    pub brick_pos: na::Point3<u32>,
    pub voxel_local_pos: Option<na::Point3<u32>>,
    pub mask: na::Vector3<f32>,
}

fn step_mask(side_dist: &na::Vector3<f32>) -> na::Vector3<f32> {
    let mut mask = na::Vector3::zeros();

    let less_than_yzx = na::Vector3::new(
        side_dist.x < side_dist.y,
        side_dist.y < side_dist.z,
        side_dist.z < side_dist.x,
    );

    let less_than_zxy = na::Vector3::new(
        side_dist.x <= side_dist.z,
        side_dist.y <= side_dist.x,
        side_dist.z <= side_dist.y,
    );

    mask.x = if less_than_yzx.x && less_than_zxy.x {
        1.0
    } else {
        0.0
    };
    mask.y = if less_than_yzx.y && less_than_zxy.y {
        1.0
    } else {
        0.0
    };
    mask.z = if less_than_yzx.z && less_than_zxy.z {
        1.0
    } else {
        0.0
    };

    // If no mask was set, set z to true (fallback case)
    if mask.x == 0.0 && mask.y == 0.0 && mask.z == 0.0 {
        mask.z = 1.0;
    }

    mask
}

fn trace_brick(
    ray_pos: na::Point3<f32>,
    ray_dir: na::Vector3<f32>,
    world_mask: na::Vector3<f32>,
    brick: &TraceBrick,
) -> Option<(na::Point3<f32>, na::Vector3<f32>)> {
    // Clamp starting position to brick bounds
    let ray_pos = na::Point3::new(
        ray_pos.x.clamp(0.0001, 7.9999),
        ray_pos.y.clamp(0.0001, 7.9999),
        ray_pos.z.clamp(0.0001, 7.9999),
    );

    let mut map_pos = ray_pos.map(|x| x.floor());
    let ray_sign = ray_dir.map(|x| x.signum());
    let delta_dist = ray_dir.map(|x| 1.0 / x);

    // Calculate initial side distances
    let mut side_dist = ((map_pos - ray_pos)
        + na::Vector3::from_element(0.5)
        + (ray_sign.component_mul(&na::Vector3::from_element(0.5))))
    .component_mul(&delta_dist);

    let mut mask = world_mask;

    while map_pos.x <= 7.0
        && map_pos.x >= 0.0
        && map_pos.y <= 7.0
        && map_pos.y >= 0.0
        && map_pos.z <= 7.0
        && map_pos.z >= 0.0
    {
        let x = map_pos.x as u32;
        let y = map_pos.y as u32;
        let z = map_pos.z as u32;

        if brick.get(x, y, z) {
            return Some((na::Point3::from(map_pos.map(|x| x / 8.0)), mask));
        }

        mask = step_mask(&side_dist);
        map_pos += mask.component_mul(&ray_sign);
        side_dist += mask.component_mul(&ray_sign).component_mul(&delta_dist);
    }

    None
}

pub fn trace_world(
    brickmap: &BrickMap,
    ray_pos: na::Point3<f32>,
    ray_dir: na::Vector3<f32>,
    max_steps: u32,
) -> Option<RayHit> {
    let mut map_pos = ray_pos.coords.map(|x| x.floor()); // floor() each component
    let ray_sign = ray_dir.map(|x| x.signum());
    let delta_dist = ray_dir.map(|x| 1.0 / x);

    let mut side_dist = ((map_pos - ray_pos.coords)
        + na::Vector3::from_element(0.5)
        + ray_sign.component_mul(&na::Vector3::from_element(0.5)))
    .component_mul(&delta_dist);

    let mut mask = step_mask(&side_dist);

    let dims = brickmap.dimensions();
    for _ in 0..max_steps {
        let brick_pos = na::Point3::new(map_pos.x as u32, map_pos.y as u32, map_pos.z as u32);

        if brick_pos.x >= dims.x || brick_pos.y >= dims.y || brick_pos.z >= dims.z { 
            return None;
        }

        let handle = brickmap.get_handle(brick_pos);
        if handle.is_data() {
            let brick = brickmap.get_brick(handle).unwrap();

            // Calculate intersection with brick bounds
            let sub = ((map_pos - ray_pos.coords) + na::Vector3::from_element(0.5)
                - ray_sign.component_mul(&na::Vector3::from_element(0.5)))
            .component_mul(&delta_dist);

            let d = sub.x.max(sub.y.max(sub.z));

            let intersect = ray_pos + (ray_dir * d);
            let mut sub_space = intersect - na::Point3::from(map_pos); // Handle case where ray starts inside brick

            if map_pos == ray_pos.coords.map(|x| x.floor()) {
                sub_space = ray_pos - na::Point3::from(map_pos);
            }

            let local_pos = na::Point3::from(sub_space * 8.0);

            if let Some((hit_pos, mask)) = trace_brick(local_pos, ray_dir, mask, &brick) {
                let pos = ray_pos + ray_dir * d;
                let normal = -mask.component_mul(&ray_sign);

                return Some(RayHit {
                    position: hit_pos,
                    normal,
                    distance: d,
                    pos,
                    brick_pos,
                    voxel_local_pos: Some(na::Point3::new(
                        (hit_pos.x * 8.0) as u32,
                        (hit_pos.y * 8.0) as u32,
                        (hit_pos.z * 8.0) as u32,
                    )),
                    mask,
                });
            }
        } else if handle.is_lod() {
            let sub = ((map_pos - ray_pos.coords) + na::Vector3::from_element(0.5)
                - ray_sign.component_mul(&na::Vector3::from_element(0.5)))
            .component_mul(&delta_dist);

            let d = sub.x.max(sub.y.max(sub.z));
            let pos = ray_pos + ray_dir * d;
            let normal = -mask.component_mul(&ray_sign);

            return Some(RayHit {
                position: na::Point3::from(map_pos.map(|x| x / 8.0)),
                normal,
                distance: d,
                pos,
                brick_pos,
                voxel_local_pos: None,
                mask,
            });
        }

        mask = step_mask(&side_dist);
        map_pos += mask.component_mul(&ray_sign);
        side_dist += mask.component_mul(&ray_sign).component_mul(&delta_dist);
    }

    None
}

pub fn cast_center_ray(camera: &Camera, brickmap: &BrickMap, max_steps: u32) -> Option<RayHit> {
    let origin = camera.position;
    let direction = camera.rotation * -na::Vector3::z_axis();

    trace_world(brickmap, origin, *direction, max_steps)
}

pub fn cast_screen_ray(
    camera: &Camera,
    screen_pos: na::Point2<f32>,
    screen_size: na::Vector2<f32>,
    brickmap: &BrickMap,
    max_steps: u32,
) -> Option<RayHit> {
    let ndc = na::Point2::new(
        (2.0 * screen_pos.x) / screen_size.x - 1.0,
        1.0 - (2.0 * screen_pos.y) / screen_size.y, // Flip Y coordinate
    );

    let fov = camera.fov.to_radians();
    let aspect_ratio = screen_size.x / screen_size.y;
    let tan_fov = (fov * 0.5).tan();

    let ray_dir =
        na::Vector3::new(ndc.x * aspect_ratio * tan_fov, ndc.y * tan_fov, -1.0).normalize();

    let world_ray_dir = camera.rotation * ray_dir;

    trace_world(brickmap, camera.position, world_ray_dir, max_steps)
}
