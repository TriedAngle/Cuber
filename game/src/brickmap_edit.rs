// use nalgebra as na;
//
// use crate::brick::{BrickHandle, BrickMap};
//
// impl BrickMap {
//     pub fn draw_sphere_sdf(
//         &self,
//         center: na::Point3<f32>,
//         radius: f32,
//     ) -> (Vec<BrickHandle>, Vec<(BrickHandle, na::Point3<u32>)>) {
//         let mut modified_handles = Vec::new();
//         let mut emptied_bricks = Vec::new();
//
//         let min_corner = na::Point3::new(
//             (center.x - radius - 1.0).floor() as i32,
//             (center.y - radius - 1.0).floor() as i32,
//             (center.z - radius - 1.0).floor() as i32,
//         );
//         let max_corner = na::Point3::new(
//             (center.x + radius + 1.0).ceil() as i32,
//             (center.y + radius + 1.0).ceil() as i32,
//             (center.z + radius + 1.0).ceil() as i32,
//         );
//
//         let dims = self.dimensions();
//         for bx in min_corner.x..=max_corner.x {
//             for by in min_corner.y..=max_corner.y {
//                 for bz in min_corner.z..=max_corner.z {
//                     if bx < 0
//                         || by < 0
//                         || bz < 0
//                         || bx >= dims.x as i32
//                         || by >= dims.y as i32
//                         || bz >= dims.z as i32
//                     {
//                         continue;
//                     }
//
//                     let brick_pos = na::Point3::new(bx as u32, by as u32, bz as u32);
//                     let handle = self.get_handle(brick_pos);
//
//                     if handle.is_empty() {
//                         continue;
//                     }
//
//                     let brick_world_pos = na::Point3::new(bx as f32, by as f32, bz as f32);
//
//                     let closest = na::Point3::new(
//                         (center.x.clamp(brick_world_pos.x, brick_world_pos.x + 1.0) - center.x)
//                             .abs(),
//                         (center.y.clamp(brick_world_pos.y, brick_world_pos.y + 1.0) - center.y)
//                             .abs(),
//                         (center.z.clamp(brick_world_pos.z, brick_world_pos.z + 1.0) - center.z)
//                             .abs(),
//                     );
//
//                     let distance =
//                         (closest.x * closest.x + closest.y * closest.y + closest.z * closest.z)
//                             .sqrt();
//
//                     if distance > radius + 0.866 {
//                         continue;
//                     }
//
//                     if handle.is_lod() {
//                         let corners = [
//                             na::Point3::new(
//                                 brick_world_pos.x,
//                                 brick_world_pos.y,
//                                 brick_world_pos.z,
//                             ),
//                             na::Point3::new(
//                                 brick_world_pos.x + 1.0,
//                                 brick_world_pos.y,
//                                 brick_world_pos.z,
//                             ),
//                             na::Point3::new(
//                                 brick_world_pos.x,
//                                 brick_world_pos.y + 1.0,
//                                 brick_world_pos.z,
//                             ),
//                             na::Point3::new(
//                                 brick_world_pos.x,
//                                 brick_world_pos.y,
//                                 brick_world_pos.z + 1.0,
//                             ),
//                             na::Point3::new(
//                                 brick_world_pos.x + 1.0,
//                                 brick_world_pos.y + 1.0,
//                                 brick_world_pos.z,
//                             ),
//                             na::Point3::new(
//                                 brick_world_pos.x + 1.0,
//                                 brick_world_pos.y,
//                                 brick_world_pos.z + 1.0,
//                             ),
//                             na::Point3::new(
//                                 brick_world_pos.x,
//                                 brick_world_pos.y + 1.0,
//                                 brick_world_pos.z + 1.0,
//                             ),
//                             na::Point3::new(
//                                 brick_world_pos.x + 1.0,
//                                 brick_world_pos.y + 1.0,
//                                 brick_world_pos.z + 1.0,
//                             ),
//                         ];
//
//                         let fully_contained = corners
//                             .iter()
//                             .all(|corner| na::distance(&center, corner) <= radius);
//
//                         if fully_contained {
//                             emptied_bricks.push((handle, brick_pos));
//                         }
//                         continue;
//                     }
//
//                     // Regular brick processing
//                     let mut any_change = false;
//                     for vx in 0..8u32 {
//                         for vy in 0..8u32 {
//                             for vz in 0..8u32 {
//                                 let voxel_world_pos = na::Point3::new(
//                                     brick_world_pos.x + vx as f32 / 8.0,
//                                     brick_world_pos.y + vy as f32 / 8.0,
//                                     brick_world_pos.z + vz as f32 / 8.0,
//                                 );
//
//                                 let dist = na::distance(&center, &voxel_world_pos);
//                                 if dist <= radius {
//                                     self.edit_brick_no_resize(
//                                         handle,
//                                         Some(na::Point3::new(vx, vy, vz)),
//                                         0,
//                                     );
//                                     any_change = true;
//                                 }
//                             }
//                         }
//                     }
//
//                     if any_change {
//                         let is_now_empty = self
//                             .get_brick(handle)
//                             .map(|brick| brick.is_empty())
//                             .unwrap_or(false);
//
//                         if is_now_empty {
//                             let empty_handle = BrickHandle::empty();
//                             self.set_handle(empty_handle, brick_pos);
//                             emptied_bricks.push((handle, brick_pos));
//                         } else {
//                             modified_handles.push(handle);
//                         }
//                     }
//                 }
//             }
//         }
//
//         (modified_handles, emptied_bricks)
//     }
// }
