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
    material: u32,
}

struct Brick1 { 
    raw: array<u32, 16>,
}
struct Brick2 {
    raw: array<u32, 32>,
}
struct Brick4 {
    raw: array<u32, 64>,
}
struct Brick8 {
    raw: array<u32, 128>,
}

const MATERIAL_AIR: u32 = 0;
const MATERIAL_STONE: u32 = 1;
const MATERIAL_BEDROCK: u32 = 2;
const MATERIAL_DIRT: u32 = 3;
const MATERIAL_GRASS: u32 = 4;
const MATERIAL_SNOW: u32 = 5;

// Using upper 3 bits
const FLAG_MASK = 0xE0000000u;  // 111 in top 3 bits
const SEEN_BIT = 0x80000000u;   // 1 in top bit
const STATE_MASK = 0x60000000u;  // 11 in bits 30-29

const STATE_EMPTY = 0x00000000u;
const STATE_DATA = 0x20000000u;     // 01 in bits 30-29
const STATE_UNLOADED = 0x40000000u; // 10 in bits 30-29
const STATE_LOADING = 0x60000000u;  // 11 in bits 30-29

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
var<storage, read> bricks1: array<Brick1>; 
@group(1) @binding(3)
var<storage, read> bricks2: array<Brick2>; 
@group(1) @binding(4)
var<storage, read> bricks4: array<Brick4>; 
@group(1) @binding(5)
var<storage, read> bricks8: array<Brick8>; 

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


// TODO: make this runtime defined
fn get_material_color(material: u32) -> vec4<f32> {
    switch material {
        case MATERIAL_AIR: {
            return vec4<f32>(0.0, 0.0, 0.0, 0.0);
        }
        case MATERIAL_STONE: {
            return vec4<f32>(0.5, 0.5, 0.5, 1.0);
        }
        case MATERIAL_BEDROCK: {
            return vec4<f32>(0.3, 0.3, 0.3, 1.0);
        }
        case MATERIAL_DIRT: {
            return vec4<f32>(0.59, 0.29, 0.0, 1.0); // Rich brown color
        }
        case MATERIAL_GRASS: {
            return vec4<f32>(0.33, 0.63, 0.22, 1.0); // Vibrant grass green
        }
        case MATERIAL_SNOW: {
            return vec4<f32>(0.95, 0.95, 0.95, 1.0); // Bright white with slight off-white tint
        }
        default: {
            return vec4<f32>(1.0, 0.0, 1.0, 1.0); // Magenta for undefined materials
        }
    }
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

fn is_brick_unloaded(id: u32) -> bool {
    return (id & STATE_MASK) == STATE_UNLOADED;
}

fn is_brick_loading(id: u32) -> bool {
    return (id & STATE_MASK) == STATE_LOADING;
}

fn get_brick_handle_offset(brick: u32) -> u32 {
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

fn get_brick_voxel(trace_brick_handle: u32, local_pos: vec3<i32>) -> u32 {
    let brick_handle = trace_brick_handle;

    // Extract bits per voxel from upper 3 bits
    let bits_per_voxel = (brick_handle >> 29u) & 0x7u;
    
    // Get index from lower 29 bits
    let brick_index = brick_handle & 0x1FFFFFFFu;
    
    // Calculate voxel index in the brick
    let voxel_idx = local_pos.x + 
                    local_pos.y * CHUNK_SIZE + 
                    local_pos.z * CHUNK_SIZE * CHUNK_SIZE;
    
    // Common calculations for all brick types
    let bits_per_u32 = 32u;
    var u32_index: u32;
    var bit_offset: u32;
    var data: u32;
    var mask: u32;
    
    switch bits_per_voxel {
        case 1u: {
            let voxels_per_u32 = 32u;
            u32_index = u32(voxel_idx) / voxels_per_u32;
            bit_offset = u32(voxel_idx) % voxels_per_u32;
            data = bricks1[brick_index].raw[u32_index];
            mask = 1u;
        }
        case 2u: {
            let voxels_per_u32 = 16u;
            u32_index = u32(voxel_idx) / voxels_per_u32;
            bit_offset = (u32(voxel_idx) % voxels_per_u32) * 2u;
            data = bricks2[brick_index].raw[u32_index];
            mask = 3u;
        }
        case 4u: {
            let voxels_per_u32 = 8u;
            u32_index = u32(voxel_idx) / voxels_per_u32;
            bit_offset = (u32(voxel_idx) % voxels_per_u32) * 4u;
            data = bricks4[brick_index].raw[u32_index];
            mask = 15u;
        }
        case 8u: {
            let voxels_per_u32 = 4u;
            u32_index = u32(voxel_idx) / voxels_per_u32;
            bit_offset = (u32(voxel_idx) % voxels_per_u32) * 8u;
            data = bricks8[brick_index].raw[u32_index];
            mask = 255u;
        }
        default: {
            return 0u;
        }
    }
    
    return (data >> bit_offset) & mask;
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

    var steps = 0u;
    while all(vec3<f32>(0.0) <= map_pos) && all(map_pos <= vec3<f32>(7.0)) { 
        brightness = brightness + 0.01;
        let vox = get_trace_voxel(brick_handle, vec3<i32>(map_pos));
        if vox {
            let trace = trace_bricks[brick_handle];
            let material_vox = get_brick_voxel(trace.brick, vec3<i32>(map_pos));
            let color = get_material_color(material_vox);

            var hit = new_hit(true, mask);
            hit.pos = vec4<f32>(floor(map_pos) / 8.0, 1.0);
            // hit.color = vec4<f32>(floor(map_pos) / 8.0, 1.0);
            hit.color = color;
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
    let world_min = vec3<f32>(0.0);
    let world_max = vec3<f32>(uniforms.brick_grid_dimension);
    
    let bounds = intersect_box(ray_pos, ray_dir, world_min, world_max);
    
    if bounds.x > bounds.y || bounds.y < 0.0 {
        return new_hit(false, vec3<f32>(0.0));
    }
    
    var adjusted_pos = ray_pos;
    if bounds.x > 0.0 {
        adjusted_pos = ray_pos + ray_dir * bounds.x;
    }
    
    var map_pos = floor(adjusted_pos);
    let ray_sign = sign(ray_dir);
    let delta_dist = 1.0 / ray_dir;
    var side_dist = ((map_pos - ray_pos) + 0.5 + (ray_sign * 0.5)) * delta_dist;
    var mask = step_mask(side_dist);
    
    var steps = 0u;
    for (var i = 0; i < MAX_RAY_STEPS; i++) { 
        let brick_handle_raw = get_brick_handle(vec3<i32>(floor(map_pos)));
        let brick_handle = get_brick_handle_offset(brick_handle_raw);
        let is_data = is_brick_data(brick_handle_raw);
        brightness = brightness + (1.0 / f32(MAX_RAY_STEPS));

        if is_data && all(map_pos >= vec3<f32>(0.0)) {
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
        mask = step_mask(side_dist);
        map_pos = map_pos + (mask * ray_sign);
        side_dist = side_dist + (mask * ray_sign * delta_dist);
        steps = steps + 1;

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
