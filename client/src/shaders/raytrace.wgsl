struct PushConstants {
    camera: mat4x4<f32>,
    camera_inverse: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _padding0: u32,
    dimensions: vec3<i32>,
    packed_resolution: u32,
    flags0: u32,
    flags1: u32,
    dt: f32,
    depth_boost: f32,
    brick_hit: vec3<u32>,
    _padding1: u32,
    voxel_hit: vec3<u32>,
    _padding2: u32,
}

struct BrickHandle {
    raw: u32,
}

struct TraceBrick {
    raw: array<u32, 16>,
    brick_offset: u32,
}

struct PbrMaterial {
    color: vec4<f32>,
    emissive: vec3<f32>,
    opaque: f32,
    metallic: f32,
    roughtness: f32,
}

struct Hit {
    color: vec4<f32>,
    pos: vec3<f32>,
    hit: bool,
    mask: vec3<f32>,
}

fn new_empty_hit() -> Hit {
    return Hit(
        vec4<f32>(0.0, 0.0, 0.0, 0.0),
        vec3<f32>(0.0, 0.0, 0.0),
        false,
        vec3<f32>(0.0, 0.0, 0.0),
    );
}

fn new_hit(color: vec4<f32>, pos: vec3<f32>, mask: vec3<f32>) -> Hit {
    return Hit(color, pos, true, mask);
}

var<push_constant> pc: PushConstants;

@group(0) @binding(0)
var<storage, read> brick_handles: array<BrickHandle>;

@group(0) @binding(1)
var<storage, read> trace_bricks: array<TraceBrick>;

@group(0) @binding(2)
var<storage, read> materials: array<PbrMaterial>;

@group(0) @binding(3)
var<storage, read> palettes: array<u32>;

@group(0) @binding(4)
var images: binding_array<texture_storage_2d<rgba8unorm, write>, 10>; 

const MAX_RAY_STEPS: u32 = 64;
const EPSILON: f32 = 0.00001;

const DATA_BIT: u32 = 0x80000000u;  // Bit 31
const LOD_BIT: u32  = 0x40000000u;  // Bit 30 
const DATA_MASK: u32 = 0x7FFFFFFFu;  // Bits 0-30 for data
const EMPTY_DATA_MASK: u32 = 0x3FFFFFFFu; // Bits 0-29 for empty handle values

fn get_resolution() -> vec2<f32> {
    let width = (pc.packed_resolution & 0xFFFFu) >> 0u;
    let height = (pc.packed_resolution & 0xFFFF0000u) >> 16u;
    return vec2<f32>(f32(width), f32(height));
}

fn brick_handle_is_data(brick_handle: BrickHandle) -> bool {
    return (brick_handle.raw & DATA_BIT) != 0u;
}

fn brick_handle_is_empty(brick_handle: BrickHandle) -> bool {
    return (brick_handle.raw & DATA_BIT) == 0;
}

fn brick_handle_is_lod(brick_handle: BrickHandle) -> bool {
    return brick_handle_is_empty(brick_handle) && (brick_handle.raw & LOD_BIT) != 0;
}

fn brick_handle_get_data(brick_handle: BrickHandle) -> u32 {
    return brick_handle.raw & DATA_MASK;
}

fn brick_handle_get_empty_value(brick_handle: BrickHandle) -> u32 {
    return brick_handle.raw & EMPTY_DATA_MASK;
}

fn brick_handle_index(pos: vec3<i32>) -> u32 {
    return u32(
        pos.x + pos.y * pc.dimensions.x + pos.z * pc.dimensions.x * pc.dimensions.y
    );
}

fn get_brick_handle(pos: vec3<i32>) -> BrickHandle {
    if any(pos < vec3<i32>(0)) || any(pos >= pc.dimensions) {
        return BrickHandle(0u);
    }

    let idx = brick_handle_index(pos);
    let brick_handle: BrickHandle = brick_handles[idx];
    return brick_handle;
}

fn step_mask(side_dist: vec3<f32>) -> vec3<f32> {
    var mask: vec3<bool>;
    let b1 = side_dist < side_dist.yzx;
    let b2 = side_dist <= side_dist.zxy;

    mask.z = b1.z && b2.z;
    mask.x = b1.x && b2.x;
    mask.y = b1.y && b2.y;

    if !any(mask) {
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


var<private> ray_steps: u32 = 0;

fn traverse_brickmap(ray_pos: vec3<f32>, ray_dir: vec3<f32>) -> Hit {
    let world_min = vec3<f32>(0.0);
    let world_max = vec3<f32>(pc.dimensions);
    // let bounds = intersect_box(ray_pos, ray_dir, world_min, world_max);
    //
    // if bounds.x > bounds.y || bounds.y < 0.0 {
    //     return new_empty_hit();
    // }
    //
    // var current_pos = ray_pos + ray_dir * max(bounds.x, 0.0);

    var current_pos = ray_pos;
    var map_pos = floor(current_pos);
    let ray_sign = sign(ray_dir);
    let delta_dist = 1.0 / ray_dir;
    var side_dist = ((map_pos - ray_pos) + 0.5 + ray_sign * 0.5) * delta_dist;
    var mask = step_mask(side_dist);

    for (var steps = 0u; steps < MAX_RAY_STEPS; steps++) {
        ray_steps = ray_steps + 1;
        let brick_pos = vec3<i32>(floor(map_pos));
        // let brick_handle = get_brick_handle(brick_pos);
        //
        // let is_data = brick_handle_is_empty(brick_handle);
        // let is_lod = brick_handle_is_lod(brick_handle);
        //
        let sdf_hit = get_sdf_voxel(brick_pos);
        if sdf_hit {
            return new_hit(vec4<f32>(1.0, 0.0, 1.0, 1.0), map_pos, mask);
        }

        // if is_data {
        //     let trace_brick_offset = brick_handle_get_data(brick_handle);
        //
        //     return Hit(vec4<f32>(1.0, 0.0, 1.0, 1.0), map_pos, mask);
        // } else if is_lod {
        // } else {
        // }
        //
        mask = step_mask(side_dist);

        let t = min(side_dist.x, min(side_dist.y, side_dist.z));
        current_pos = current_pos + (ray_dir * t);

        map_pos = map_pos + (mask * ray_sign);
        side_dist = ((map_pos - current_pos) + 0.5 + (ray_sign * 0.5)) * delta_dist;

        // if any(map_pos >= world_max) || any(map_pos < world_min) {
        //     break;
        // }
    }

    return new_empty_hit();
}


@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let resolution = get_resolution();

    let pixel_pos = vec2<f32>(global_id.xy);

    if any(pixel_pos >= resolution) {
        return;
    }

    let ndc = vec3<f32>(
        (pixel_pos / resolution) * 2.0 - 1.0,
        1.0
    );

    let clip_pos = vec4<f32>(ndc, 1.0);
    let world_pos_2d = pc.camera_inverse * clip_pos;
    let world_pos = world_pos_2d.xyz / world_pos_2d.w;

    var ray_dir = normalize(world_pos - pc.camera_pos);
    let ray_pos = pc.camera_pos;

    ray_dir = select(ray_dir, ray_dir + EPSILON, ray_dir == vec3<f32>(0.0));

    let hit = traverse_brickmap(ray_pos, ray_dir);

    let color = hit.color;

    let depth = calculate_depth(hit, ray_pos, ray_dir);
    let depth_color = vec4<f32>(depth, depth, depth, 1.0);

    let normal = calculate_normal(hit.mask, ray_dir);
    let normal_color = vec4<f32>(normal, 1.0);

    let intensity = calculate_steps_intensity();
    let intensity_color = vec4<f32>(intensity, intensity, intensity, 1.0);
    
    textureStore(images[0], vec2<i32>(global_id.xy), color);
    textureStore(images[1], vec2<i32>(global_id.xy), normal_color);
    textureStore(images[2], vec2<i32>(global_id.xy), depth_color);
    textureStore(images[3], vec2<i32>(global_id.xy), intensity_color);
}

fn sd_sphere(p: vec3<f32>, d: f32) -> f32 {
    return length(p) - d;
}

fn sd_box(p: vec3<f32>, b: vec3<f32>) -> f32 {
    let d = abs(p) - b;
    return min(max(d.x, max(d.y, d.z)), 0.0) + length(max(d, vec3<f32>(0.0)));
}

fn get_sdf_voxel(c: vec3<i32>) -> bool {
    let p = vec3<f32>(c) + vec3<f32>(0.5);
    let d = max(-sd_sphere(p, 7.5), sd_box(p, vec3<f32>(6.0)));

    return d < 0.0;
}

fn calculate_normal(mask: vec3<f32>, ray_dir: vec3<f32>) -> vec3<f32> {
    var normal: vec3<f32>;

    let ray_sign = sign(ray_dir);
    
    if (mask.x > 0.0) {
        normal = select(
            vec3<f32>(1.0, 0.0, 0.0),  // positive x: red
            vec3<f32>(0.0, 1.0, 1.0),  // negative x: cyan
            ray_sign.x > 0.0
        );
    } else if (mask.y > 0.0) {
        normal = select(
            vec3<f32>(0.0, 1.0, 0.0),  // positive y: green
            vec3<f32>(1.0, 0.0, 1.0),  // negative y: magenta
            ray_sign.y > 0.0
        );
    } else if (mask.z > 0.0) {
        normal = select(
            vec3<f32>(0.0, 0.0, 1.0),  // positive z: blue
            vec3<f32>(1.0, 1.0, 0.0),  // negative z: yellow
            ray_sign.z > 0.0
        );
    }
    
    return normal;
}


fn calculate_depth(hit: Hit, ray_pos: vec3<f32>, ray_dir: vec3<f32>) -> f32 {
    if (!hit.hit) {
        return 1.0;
    }
    
    let hit_distance = length(hit.pos - ray_pos);
    
    let max_distance = length(vec3<f32>(pc.dimensions));
    let normalized_depth = saturate(hit_distance / max_distance);
    
    return normalized_depth;
}

fn calculate_steps_intensity() -> f32 {
    // MAX_RAY_STEPS * 8 would be the theoretical maximum for one additional level
    // slightly lower normalization factor to make the visualization more visible
    let max_expected_steps = MAX_RAY_STEPS * 6u;
    
    let normalized = f32(ray_steps) / f32(max_expected_steps);

    // Using sqrt makes small step counts more distinguishable
    return sqrt(saturate(normalized));
}
