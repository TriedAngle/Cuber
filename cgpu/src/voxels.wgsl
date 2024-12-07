const MAX_RAY_STEPS: i32 = 64;
const CHUNK_SIZE: i32 = 8;

struct ComputeUniforms { 
    resolution: vec2<f32>,
    dt: f32, 
    render_mode: u32, // 0 normal, 1 depth
    brick_grid_dimension: vec3<i32>,
    depth_boost: f32,
    view_projection: mat4x4<f32>,
    inverse_view_projection: mat4x4<f32>,
    camera_position: vec3<f32>,
    _padding1: f32
}

struct Brick { 
    raw: array<u32, 16>,
}

struct BrickGrid { 
    handles: array<u32>,
}

struct Hit { 
    hit: bool,
    big_steps: u32,
    smol_steps: u32,
    mask: vec3<f32>,
    pos: vec4<f32>,
}

fn new_hit(hit: bool, mask: vec3<f32>) -> Hit { 
    return Hit(hit, 0, 0, mask, vec4<f32>(0.0));
}

@group(0) @binding(0)
var<uniform> uniforms: ComputeUniforms;

@group(0) @binding(1)
var OutputTexture: texture_storage_2d<rgba8unorm, write>;

@group(0) @binding(2)
var DepthTexture: texture_storage_2d<r32float, write>;

@group(1) @binding(0)
var<storage, read_write> handles: array<u32>;

@group(1) @binding(1)
var<storage, read_write> bricks: array<Brick>; 

fn get_brick_handle(pos: vec3<i32>) -> u32 { 
    if (any(pos < vec3<i32>(0)) || any(pos >= uniforms.brick_grid_dimension)) {
            return 0u;
    }

    let grid_index = pos.x 
                    + pos.y * uniforms.brick_grid_dimension.x
                    + pos.z * (uniforms.brick_grid_dimension.x * uniforms.brick_grid_dimension.y);

    let id = handles[u32(grid_index)];
    return id;
}

fn get_brick_voxel(id: u32, local_pos: vec3<i32>) -> bool {
    let voxel_idx = local_pos.x 
                  + local_pos.y * CHUNK_SIZE 
                  + local_pos.z * CHUNK_SIZE * CHUNK_SIZE;
    let u32_index = voxel_idx / 32;
    let bit_index = voxel_idx % 32;
    let voxel_data = bricks[id].raw[u32(u32_index)];
    return (voxel_data & (1u << u32(bit_index))) != 0u;
}

fn sd_sphere(p: vec3<f32>, d: f32) -> f32 { 
    return length(p) - d;
}

fn sd_box(p: vec3<f32>, b: vec3<f32>) -> f32 { 
    let d = abs(p) - b;
    return min(max(d.x, max(d.y, d.z)), 0.0) + length(max(d, vec3<f32>(0.0)));
}

fn step_mask(sideDist : vec3<f32>) -> vec3<f32> {
    var mask : vec3<bool>;
    let b1 = sideDist < sideDist.yzx;
    let b2 = sideDist <= sideDist.zxy;
    
    mask.z = b1.z && b2.z;
    mask.x = b1.x && b2.x;
    mask.y = b1.y && b2.y;
    
    if (!any(mask)) {
        mask.z = true;
    }
    
    return vec3<f32>(f32(mask.x), f32(mask.y), f32(mask.z));
}

fn trace_brick(brick_handle: u32, in_ray_pos: vec3<f32>, ray_dir: vec3<f32>, world_mask: vec3<f32>) -> Hit {
    let ray_pos = clamp(in_ray_pos, vec3<f32>(0.0001), vec3<f32>(7.9999));
    var map_pos = floor(ray_pos);
    let ray_sign = sign(ray_dir);
    let delta_dist = 1.0 / ray_dir;
    var side_dist = ((map_pos - ray_pos) + 0.5 + (ray_sign * 0.5)) * delta_dist;
    var mask = world_mask;

    var steps = 0u;
    while all(vec3<f32>(0.0) <= map_pos) && all(map_pos <= vec3<f32>(7.0)) { 
        let vox = get_brick_voxel(brick_handle, vec3<i32>(map_pos));
        if vox { 
            var hit = new_hit(true, mask);
            hit.pos = vec4<f32>(floor(map_pos) / 8.0, 1.0);
            hit.smol_steps = steps;
            return hit;
        }
        mask = step_mask(side_dist);
        map_pos += mask * ray_sign;
        side_dist += mask * ray_sign * delta_dist;
        steps = steps + 1;
    }

    return new_hit(false, vec3<f32>(0.0));
}

fn trace_world(ray_pos: vec3<f32>, ray_dir: vec3<f32>) -> Hit { 
    var map_pos = floor(ray_pos);
    let ray_sign = sign(ray_dir);
    let delta_dist = 1.0 / ray_dir;
    var side_dist = ((map_pos - ray_pos) + 0.5 + (ray_sign * 0.5)) * delta_dist;
    var mask = step_mask(side_dist);
    
    var steps = 0u;
    for (var i = 0; i < MAX_RAY_STEPS; i++) { 
        let brick_handle = get_brick_handle(vec3<i32>(floor(map_pos)));

        if brick_handle != 0 && all(map_pos >= vec3<f32>(0.0)) {
            let sub = ((map_pos - ray_pos) + 0.5 - (ray_sign * 0.5)) * delta_dist;
            let d = max(sub.x, max(sub.y, sub.z));
            let intersect = ray_pos + (ray_dir * d);
            var sub_space = intersect - map_pos;

            if all(map_pos == floor(ray_pos)) { 
                sub_space = ray_pos - map_pos;
            }

            var hit = trace_brick(brick_handle, sub_space * 8.0, ray_dir, mask);

            if hit.hit { 
                hit.big_steps = steps;
                return hit;
            }
        }
        mask = step_mask(side_dist);
        map_pos = map_pos + (mask * ray_sign);
        side_dist = side_dist + (mask * ray_sign * delta_dist);
        steps = steps + 1;
    }
    return new_hit(false, vec3<f32>(0.0));
}

@compute @workgroup_size(8, 8)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>
) { 
    let frag_coord = vec2<f32>(global_id.xy);

    if (frag_coord.x >= uniforms.resolution.x 
        || frag_coord.y >= uniforms.resolution.y) 
    { 
        return;
    }

    let screen_pos = (frag_coord / uniforms.resolution) * 2.0 - vec2<f32>(1.0);
    
    // Compute normalized device coordinates (NDC)
    let ndc = vec3<f32>(
        (frag_coord.x / uniforms.resolution.x) * 2.0 - 1.0,
        ((frag_coord.y / uniforms.resolution.y) * 2.0 - 1.0),
        1.0 // z in NDC space (far plane)
    );

    let clip_pos = vec4<f32>(ndc, 1.0);

    let world_pos = uniforms.inverse_view_projection * clip_pos;

    let world_pos_3D = world_pos.xyz / world_pos.w;

    var ray_dir = normalize(world_pos_3D - uniforms.camera_position);

    var ray_pos = uniforms.camera_position;

    if (any(ray_dir == vec3<f32>(0.0))) { 
        ray_dir += vec3<f32>(vec3<f32>(ray_dir == vec3<f32>(0.0))) * vec3<f32>(0.00001);
    }    

    let hit = trace_world(ray_pos, ray_dir);

    var depth = 1.0;
    let mask = hit.mask;
    // let hit_pos = ray_pos + ray_dir * f32(i);
    let hit_pos = vec3<f32>(0.0);

    if hit.hit {
        let clip_space_hit_pos = uniforms.view_projection * vec4<f32>(hit_pos, 1.0);
        let ndc_hit_pos = clip_space_hit_pos.xyz / clip_space_hit_pos.w;
        depth = ndc_hit_pos.z;
    }

    var color_prim = vec3<f32>(0.0);
    if (mask.x > 0.0) {
        color_prim = vec3<f32>(0.5);
    }
    if (mask.y > 0.0) {
        color_prim = vec3<f32>(1.0);
    }
    if (mask.z > 0.0) {
        color_prim = vec3<f32>(0.75);
    }

    var color = vec4<f32>(color_prim, 1.0);
    color = hit.pos;

    if uniforms.render_mode == 0 { 
        textureStore(OutputTexture, vec2<u32>(global_id.xy), color);
    } else if uniforms.render_mode == 1 { 
        let depth2 = pow(depth, uniforms.depth_boost); 
        textureStore(OutputTexture, vec2<u32>(global_id.xy), vec4<f32>(depth2, depth2, depth2, 1.0));
    } else { 
        textureStore(OutputTexture, vec2<u32>(global_id.xy), color);
    }

    textureStore(DepthTexture, vec2<u32>(global_id.xy), vec4<f32>(depth, 0.0, 0.0, 0.0));
}
