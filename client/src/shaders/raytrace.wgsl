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

struct MaterialBrickMeta {
    raw: u32,
}

struct PaletteHandle {
    raw: u32,
}

struct MaterialHandle {
    raw: u32,
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
var<storage, read> material_bricks: array<u32>;

@group(0) @binding(3)
var<storage, read> materials: array<PbrMaterial>;

@group(0) @binding(4)
var<storage, read> palettes: array<u32>;

@group(0) @binding(5)
var images: binding_array<texture_storage_2d<rgba8unorm, write>, 10>; 

const BRICK_SIZE: u32 = 8;
const MAX_RAY_STEPS: u32 = 256;
const EPSILON: f32 = 0.00001;

const DATA_BIT: u32 = 0x80000000u;  // Bit 31
const LOD_BIT: u32  = 0x40000000u;  // Bit 30 
const DATA_MASK: u32 = 0x7FFFFFFFu;  // Bits 0-30 for data
const EMPTY_DATA_MASK: u32 = 0x1FFFFFFFu; // Bits 0-29 for empty handle values
const MAX_DISTANCE: u32 = EMPTY_DATA_MASK;

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
        pos.x + (pos.y * pc.dimensions.x) + (pos.z * pc.dimensions.x * pc.dimensions.y)
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

fn get_trace_voxel(brick_handle: BrickHandle, local_pos: vec3<i32>) -> bool {
    let offset = brick_handle_get_data(brick_handle);

    let pos = vec3<u32>(local_pos);
    let voxel_idx = pos.x + pos.y * BRICK_SIZE + pos.z * BRICK_SIZE * BRICK_SIZE;
    let u32_index = voxel_idx / 32;
    let bit_index = voxel_idx % 32;
    let voxel_data = trace_bricks[offset].raw[u32_index];
    return (voxel_data & (1u << u32(bit_index))) != 0u;
}

fn get_material(material_handle: MaterialHandle) -> PbrMaterial {
    let offset = material_handle.raw;
    let material = materials[offset];
    return material;
}

fn get_brick_offset(brick_handle: BrickHandle) -> u32 {
    let trace_offset = brick_handle_get_data(brick_handle);
    let offset = trace_bricks[trace_offset].brick_offset;
    return offset;
}

fn get_brick_meta(brick_offset: u32) -> MaterialBrickMeta {
    let offset = brick_offset;
    let raw = material_bricks[offset];
    return MaterialBrickMeta(raw);
}

fn get_brick_meta_size(brick_meta: MaterialBrickMeta) -> u32 {
    // let format = brick_meta.raw >> 29u;
    return 1u << ((brick_meta.raw >> 29u) & 0x7u);
}

fn get_brick_meta_palette(brick_meta: MaterialBrickMeta) -> PaletteHandle {
    return PaletteHandle(brick_meta.raw & 0x1FFFFFFFu);
}

fn get_brick_voxel(brick_offset: u32, local_pos: vec3<i32>) -> MaterialHandle {
    let brick_meta = get_brick_meta(brick_offset);
    let offset = brick_offset + 1;
    let pos = vec3<u32>(local_pos);

    let element_size = get_brick_meta_size(brick_meta);
    let palette_handle = get_brick_meta_palette(brick_meta);
    let voxel_idx = pos.x + pos.y * BRICK_SIZE + pos.z * BRICK_SIZE * BRICK_SIZE;

    let elements_per_u32 = 32u / element_size;
    let element_idx = offset + (voxel_idx / elements_per_u32);
    let bit_offset = (voxel_idx % elements_per_u32) * element_size;
    let mask = (1u << element_size) - 1u;

    let packed = material_bricks[element_idx];
    let palette_idx = (packed >> bit_offset) & mask;
    let material_handle_raw = palettes[palette_handle.raw + palette_idx];
    return MaterialHandle(material_handle_raw);
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


fn trace_brick(brick_handle: BrickHandle, brick_pos: vec3<i32>, in_ray_pos: vec3<f32>, ray_dir: vec3<f32>, world_mask: vec3<f32>) -> Hit {
    let ray_pos = clamp(in_ray_pos, vec3<f32>(EPSILON), vec3<f32>(8.0 - EPSILON));
    var map_pos = floor(ray_pos);
    let ray_sign = sign(ray_dir);
    let delta_dist = 1.0 / ray_dir;
    var side_dist = ((map_pos - ray_pos) + 0.5 + (ray_sign * 0.5)) * delta_dist;
    var mask = world_mask;

    while all(vec3<f32>(0.0) <= map_pos) && all(map_pos <= vec3<f32>(7.0)) {
        let pos = vec3<i32>(floor(map_pos));
        ray_steps = ray_steps + 1;
        let vox = get_trace_voxel(brick_handle, pos);
        if vox {
            let offset = get_brick_offset(brick_handle);
            let material_handle = get_brick_voxel(offset, pos);
            let material = get_material(material_handle);

            let pos = floor(map_pos) / 8.0;
            // let color = vec4<f32>(floor(map_pos) / 8.0, 1.0);
            let color = material.color;
            return Hit(color, map_pos, true, mask);
        }
        mask = step_mask(side_dist);
        map_pos += mask * ray_sign;
        side_dist += mask * ray_sign * delta_dist;
    }

    return new_empty_hit();
}

fn traverse_brickmap(ray_pos: vec3<f32>, ray_dir: vec3<f32>) -> Hit {
    let world_min = vec3<f32>(0.0);
    let world_max = vec3<f32>(pc.dimensions);
    let bounds = intersect_box(ray_pos, ray_dir, world_min, world_max);

    if bounds.x > bounds.y || bounds.y < 0.0 {
        return new_empty_hit();
    }

    var current_pos = ray_pos;
    if bounds.x > 0.0 {
        current_pos = ray_pos + ray_dir * bounds.x;
    }

    var map_pos = floor(current_pos);
    let ray_sign = sign(ray_dir);
    let delta_dist = 1.0 / ray_dir;
    var side_dist = ((map_pos - current_pos) + 0.5 + ray_sign * 0.5) * delta_dist;
    var mask = step_mask(side_dist);

    for (var steps = 0u; steps < MAX_RAY_STEPS; steps++) {
        ray_steps = ray_steps + 1;
        let brick_pos = vec3<i32>(floor(map_pos));
        let brick_handle = get_brick_handle(brick_pos);

        let is_data = brick_handle_is_data(brick_handle);
        let is_lod = brick_handle_is_lod(brick_handle);

        if is_data {
            let intersect = ((map_pos - ray_pos) + 0.5 - (0.5 * ray_sign)) * delta_dist;
            let dist = max(intersect.x, max(intersect.y, intersect.z));
            let hit_point = ray_pos + (ray_dir * dist);
            var local_block_coord = hit_point - map_pos;

            if all(map_pos == floor(ray_pos)) {
                local_block_coord = ray_pos - map_pos;
            }

            var hit = trace_brick(brick_handle, vec3<i32>(floor(map_pos)), local_block_coord * 8.0, ray_dir, mask);

            if hit.hit {
                let hit_local = hit.pos;
                hit.pos = map_pos + (hit_local / 8.0);
                return hit;
            }
        } else if is_lod {
            let material_offset = brick_handle_get_empty_value(brick_handle);
            let material_handle = MaterialHandle(material_offset);
            let material = get_material(material_handle);
            return Hit(material.color, map_pos, true, mask);
        } else {
            let sdf_value = brick_handle_get_empty_value(brick_handle);

            if sdf_value > 1 && sdf_value != MAX_DISTANCE {
                let sdf = sdf_value - 1;
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

    if mask.x > 0.0 {
        normal = select(
            vec3<f32>(1.0, 0.0, 0.0),  // positive x: red
            vec3<f32>(0.0, 1.0, 1.0),  // negative x: cyan
            ray_sign.x > 0.0
        );
    } else if mask.y > 0.0 {
        normal = select(
            vec3<f32>(0.0, 1.0, 0.0),  // positive y: green
            vec3<f32>(1.0, 0.0, 1.0),  // negative y: magenta
            ray_sign.y > 0.0
        );
    } else if mask.z > 0.0 {
        normal = select(
            vec3<f32>(0.0, 0.0, 1.0),  // positive z: blue
            vec3<f32>(1.0, 1.0, 0.0),  // negative z: yellow
            ray_sign.z > 0.0
        );
    }

    return normal;
}


fn calculate_depth(hit: Hit, ray_pos: vec3<f32>, ray_dir: vec3<f32>) -> f32 {
    if !hit.hit {
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
