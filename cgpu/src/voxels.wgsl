const MAX_RAY_STEPS: i32 = 256;
const CHUNK_SIZE: i32 = 8;

struct ComputeUniforms { 
    resolution: vec2<f32>,
    dt: f32, 
    render_mode: u32, // 0 game, 1 depth, 2 normals, 3 traversal
    brick_grid_dimension: vec3<i32>,
    depth_boost: f32,
    view_projection: mat4x4<f32>,
    inverse_view_projection: mat4x4<f32>,
    camera_position: vec3<f32>,
    _padding1: f32
}

struct TraceBrick { 
    raw: array<u32, 16>,
    brick: u32, // upper 3 bits store bits per voxel, other 29 store handle to bricks
    palette: u32,
}

struct PbrMaterial {
    color: vec4<f32>,
    metallic: f32,
    roughness: f32,
    _padding1: vec2<f32>,
    emissive: vec4<f32>,
}

const FLAG_MASK = 0xE0000000u;  // 111 in top 3 bits
const SEEN_BIT = 0x80000000u;   // 1 in top bit
const STATE_MASK = 0x60000000u;  // 11 in bits 30-29

const STATE_EMPTY = 0x00000000u;    // x00
const STATE_DATA = 0x20000000u;     // x01
const STATE_LOADING = 0x40000000u;  // x10
const STATE_LOD = 0x60000000u;      // x11

@group(0) @binding(0)
var<uniform> uniforms: ComputeUniforms;

@group(0) @binding(1)
var OutputTexture: texture_storage_2d<rgba8unorm, write>;

@group(0) @binding(2)
var DepthTexture: texture_storage_2d<r32float, write>;

@group(1) @binding(0)
var<storage, read_write> handles: array<u32>;

@group(1) @binding(1)
var<storage, read> trace_bricks: array<TraceBrick>;

@group(1) @binding(2)
var<storage, read> palettes: array<u32>;

@group(1) @binding(3)
var<storage, read> materials: array<PbrMaterial>;

@group(1) @binding(4)
var<storage, read> bricks: array<u32>;

var<private> brightness: f32 = 0.0;

struct Hit { 
    hit: bool,
    mask: vec3<f32>,
    pos: vec4<f32>,
    color: vec4<f32>,
}

fn new_hit(hit: bool, mask: vec3<f32>) -> Hit { 
    return Hit(hit, mask, vec4<f32>(0.0), vec4<f32>(0.0));
}

fn brick_index(pos: vec3<i32>) -> u32 {
    return u32(
        pos.x + 
        pos.y * uniforms.brick_grid_dimension.x + 
        pos.z * (uniforms.brick_grid_dimension.x * uniforms.brick_grid_dimension.y)
    );
}

fn is_brick_seen(id: u32) -> bool {
    return (id & SEEN_BIT) != 0u;
}

fn is_brick_data(id: u32) -> bool {
    return (id & STATE_MASK) == STATE_DATA;
}

fn is_brick_loading(id: u32) -> bool {
    return (id & STATE_MASK) == STATE_LOADING;
}

fn is_brick_lod(id: u32) -> bool {
    return (id & STATE_MASK) == STATE_LOD;
}

fn get_brick_handle_offset(brick: u32) -> u32 {
    return brick & ~FLAG_MASK;
}

fn get_brick_handle_sdf(brick: u32) -> u32 {
    return brick & ~FLAG_MASK;
}

fn get_material_handle(brick: u32) -> u32 {
    return brick & ~FLAG_MASK;
}

fn set_brick_seen(pos: vec3<i32>) {
    let idx = brick_index(pos);
    var id = handles[idx];
    id = id | SEEN_BIT;
    handles[idx] = id;
}

fn get_brick_handle(pos: vec3<i32>) -> u32 { 
    if (any(pos < vec3<i32>(0)) || any(pos >= uniforms.brick_grid_dimension)) {
        return 0u;
    }

    let idx = brick_index(pos);
    let id = handles[idx];
    return id;
}

fn get_trace_voxel(id: u32, local_pos: vec3<i32>) -> bool {
    let voxel_idx = local_pos.x 
            + local_pos.y * CHUNK_SIZE 
            + local_pos.z * CHUNK_SIZE * CHUNK_SIZE;
    let u32_index = voxel_idx / 32;
    let bit_index = voxel_idx % 32;
    let voxel_data = trace_bricks[id].raw[u32(u32_index)];
    return (voxel_data & (1u << u32(bit_index))) != 0u;
}

fn get_brick_voxel(brick_handle: u32, local_pos: vec3<i32>) -> u32 {
    let format = (brick_handle >> 29u) & 0x7u;
    let byte_offset = brick_handle & 0x1FFFFFFFu;

    let voxel_idx = local_pos.x 
        + local_pos.y * CHUNK_SIZE 
        + local_pos.z * CHUNK_SIZE * CHUNK_SIZE;

    var element_idx: u32;
    var bit_offset: u32;
    var mask: u32;
    var bits_per_element: u32;

    switch format {
        case 0u: {
            element_idx = byte_offset / 4u + (u32(voxel_idx) / 32u);
            bit_offset = u32(voxel_idx) % 32u;
            mask = 1u;
            bits_per_element = 1u;
        }
        case 1u: {
            element_idx = byte_offset / 4u + (u32(voxel_idx) / 16u);
            bit_offset = (u32(voxel_idx) % 16u) * 2u;
            mask = 3u;
            bits_per_element = 2u;
        }
        case 3u: {
            element_idx = byte_offset / 4u + (u32(voxel_idx) / 8u);
            bit_offset = (u32(voxel_idx) % 8u) * 4u;
            mask = 15u;
            bits_per_element = 4u;
        }
        default: {
            element_idx = byte_offset / 4u + (u32(voxel_idx) / 4u);
            bit_offset = (u32(voxel_idx) % 4u) * 8u;
            mask = 255u;
            bits_per_element = 8u;
        }
    }

    let packed_value = bricks[element_idx];
    
    return (packed_value >> bit_offset) & mask;
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

fn intersect_box(ray_origin: vec3<f32>, ray_dir: vec3<f32>, box_min: vec3<f32>, box_max: vec3<f32>) -> vec2<f32> {
    let t1 = (box_min - ray_origin) / ray_dir;
    let t2 = (box_max - ray_origin) / ray_dir;
    
    let tmin = min(t1, t2);
    let tmax = max(t1, t2);
    
    let t_near = max(max(tmin.x, tmin.y), tmin.z);
    let t_far = min(min(tmax.x, tmax.y), tmax.z);
    
    return vec2<f32>(t_near, t_far);
}

fn trace_brick(brick_handle: u32, in_ray_pos: vec3<f32>, ray_dir: vec3<f32>, world_mask: vec3<f32>) -> Hit {
    let ray_pos = clamp(in_ray_pos, vec3<f32>(0.0001), vec3<f32>(7.9999));
    var map_pos = floor(ray_pos);
    let ray_sign = sign(ray_dir);
    let delta_dist = 1.0 / ray_dir;
    var side_dist = ((map_pos - ray_pos) + 0.5 + (ray_sign * 0.5)) * delta_dist;
    var mask = world_mask;

    while all(vec3<f32>(0.0) <= map_pos) && all(map_pos <= vec3<f32>(7.0)) { 
        brightness = brightness + 0.001;
        let vox = get_trace_voxel(brick_handle, vec3<i32>(map_pos));
        if vox {
            let trace = trace_bricks[brick_handle];

            let palette_vox_id = get_brick_voxel(trace.brick, vec3<i32>(map_pos));

            let palette_offset = trace.palette;
            let material_id = palettes[palette_offset + palette_vox_id];
            let material = materials[material_id];


            var hit = new_hit(true, mask);
            hit.pos = vec4<f32>(floor(map_pos) / 8.0, 1.0);
            hit.color = material.color;
            return hit;
        }
        mask = step_mask(side_dist);
        map_pos += mask * ray_sign;
        side_dist += mask * ray_sign * delta_dist;
    }

    return new_hit(false, vec3<f32>(0.0));
}

fn trace_world(ray_pos: vec3<f32>, ray_dir: vec3<f32>) -> Hit { 
    let world_min = vec3<f32>(0.0);
    let world_max = vec3<f32>(uniforms.brick_grid_dimension);
    
    var bounds = intersect_box(ray_pos, ray_dir, world_min, world_max);
    
    if bounds.x > bounds.y || bounds.y < 0.0 {
        return new_hit(false, vec3<f32>(0.0));
    }
    
    var current_pos = ray_pos;
    if bounds.x > 0.0 {
        current_pos = ray_pos + ray_dir * bounds.x;
    }
    
    var map_pos = floor(current_pos);
    let ray_sign = sign(ray_dir);
    let delta_dist = 1.0 / ray_dir;
    var side_dist = ((map_pos - current_pos) + 0.5 + (ray_sign * 0.5)) * delta_dist;
    var mask = step_mask(side_dist);
    
    for (var steps = 0; steps < MAX_RAY_STEPS; steps++) {
        let brick_handle_raw = get_brick_handle(vec3<i32>(floor(map_pos)));
        let brick_handle = get_brick_handle_offset(brick_handle_raw);

        let is_data = is_brick_data(brick_handle_raw);
        let is_lod = is_brick_lod(brick_handle_raw);

        brightness = brightness + (0.5 / f32(MAX_RAY_STEPS));

        if is_data {
            if all(map_pos >= vec3<f32>(0.0)) {
                let sub = ((map_pos - ray_pos) + 0.5 - (ray_sign * 0.5)) * delta_dist;
                let d = max(sub.x, max(sub.y, sub.z));
                let intersect = ray_pos + (ray_dir * d);
                var sub_space = intersect - map_pos;

                if all(map_pos == floor(ray_pos)) {
                    sub_space = ray_pos - map_pos;
                }

                var hit = trace_brick(brick_handle, sub_space * 8.0, ray_dir, mask);

                if hit.hit {
                    let hit_local_pos = hit.pos.xyz;
                    hit.pos = vec4<f32>(map_pos + hit_local_pos, 1.0);
                    return hit;
                }
            }
        } else if is_lod {
            let material_handle = get_material_handle(brick_handle_raw);
            let material = materials[material_handle];
            var hit = new_hit(true, map_pos);
            hit.color = material.color;
            return hit;
        } else {
            let sdf = get_brick_handle_sdf(brick_handle_raw);
            
            if sdf > 1 {
                current_pos = current_pos + (ray_dir * f32(sdf));
                
                map_pos = floor(current_pos);
                side_dist = ((map_pos - current_pos) + 0.5 + (ray_sign * 0.5)) * delta_dist;
                mask = step_mask(side_dist);                
                continue;
            }
        }

        mask = step_mask(side_dist);
        
        let t = min(side_dist.x, min(side_dist.y, side_dist.z));
        current_pos = current_pos + (ray_dir * t);
        
        map_pos = map_pos + (mask * ray_sign);
        side_dist = ((map_pos - current_pos) + 0.5 + (ray_sign * 0.5)) * delta_dist;

        if any(map_pos >= world_max) || any(map_pos < world_min) {
            break;
        }
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

    if hit.hit {
        let world_hit_pos = hit.pos.xyz;
        let clip_space_hit_pos = uniforms.view_projection * vec4<f32>(world_hit_pos, 1.0);
        let ndc_hit_pos = clip_space_hit_pos.xyz / clip_space_hit_pos.w;
        depth = ndc_hit_pos.z;
    }

    var color = vec4<f32>(0.0);
    
    if uniforms.render_mode == 0 { 
        color = hit.color;
    } else if uniforms.render_mode == 1 { 
        let depth2 = pow(depth, uniforms.depth_boost); 
        color = vec4<f32>(depth2, depth2, depth2, 1.0);
    } else if uniforms.render_mode == 2 { 
        if hit.hit {
            var normal = vec3<f32>(0.0);
            if mask.x > 0.0 {
                normal.x = select(1.0, -1.0, ray_dir.x > 0.0);
            }
            if mask.y > 0.0 {
                normal.y = select(1.0, -1.0, ray_dir.y > 0.0);
            }
            if mask.z > 0.0 {
                normal.z = select(1.0, -1.0, ray_dir.z > 0.0);
            }
            
            // Map normal to color (positive = standard RGB, negative = inverted)
            var normal_color = abs(normal);
            if any(normal < vec3<f32>(0.0)) {
                normal_color = vec3<f32>(1.0) - normal_color;
            }
            
            color = vec4<f32>(normal_color, 1.0);
        } else {
            color = vec4<f32>(0.0, 0.0, 0.0, 1.0);
        }
    } else if uniforms.render_mode == 3 { 
        let brightness = min(brightness, 1.0); // Clamp to avoid over-bright areas
        color = vec4<f32>(brightness, brightness, brightness, 1.0);
    } else { 
        
    }

    textureStore(OutputTexture, vec2<u32>(global_id.xy), color);
    textureStore(DepthTexture, vec2<u32>(global_id.xy), vec4<f32>(depth, 0.0, 0.0, 0.0));
}
