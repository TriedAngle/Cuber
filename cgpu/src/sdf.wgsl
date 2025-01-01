struct SDFCompute {
    dimensions: vec3<u32>,
    padding: u32,
    num_steps: u32,
    current_step: u32,
    padding2: vec2<u32>,
}

@group(0) @binding(0)
var<uniform> params: SDFCompute;

@group(1) @binding(0)
var<storage, read_write> bricks: array<u32>;

const DISTANCE_MASK: u32 = 0x1FFFFFFF;  // Lower 29 bits for distance
const FLAGS_MASK: u32 = 0xE0000000;     // Upper 3 bits for flags
const MAX_DISTANCE: u32 = DISTANCE_MASK;

const STATE_MASK = 0x60000000u;  // 11 in bits 30-29
const STATE_EMPTY = 0x00000000u;
const STATE_DATA =  0x20000000u;
const STATE_LOADING = 0x40000000u;
const STATE_LOD =   0x60000000u;

fn get_index_from_pos(pos: vec3<u32>) -> u32 {
    return pos.z * params.dimensions.x * params.dimensions.y +
           pos.y * params.dimensions.x +
           pos.x;
}

fn get_flags(value: u32) -> u32 {
    return (value >> 29u) & 0x7u;
}

fn is_solid(value: u32) -> bool {
    let flags = value & STATE_MASK;
    return (flags == STATE_DATA) || (flags == STATE_LOD);
}

fn get_distance(value: u32) -> u32 {
    return value & DISTANCE_MASK;
}

fn make_brick(distance: u32, flags: u32) -> u32 {
    return (flags << 29u) | (distance & DISTANCE_MASK);
}

@compute @workgroup_size(8, 8, 4)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    if (any(global_id >= params.dimensions)) {
        return;
    }
    
    let index = get_index_from_pos(global_id);
    let current = bricks[index];
    
    if ((current & STATE_MASK) != STATE_EMPTY) {
        return;
    }
    
    if (params.current_step == 0u) {
        bricks[index] = STATE_EMPTY | MAX_DISTANCE;
        return;
    }
    
    var min_dist: f32 = f32(MAX_DISTANCE);
    
    let search_radius = max(
        max(params.dimensions.x, params.dimensions.y),
        params.dimensions.z
    ) >> (params.current_step - 1u);
    
    if (search_radius == 0u) {
        return;
    }
    
    for (var z: i32 = -1; z <= 1; z++) {
        for (var y: i32 = -1; y <= 1; y++) {
            for (var x: i32 = -1; x <= 1; x++) {
                let offset = vec3<i32>(x, y, z);
                let pos = vec3<i32>(global_id) + offset * i32(search_radius);
                
                // Skip if out of bounds
                if (any(pos < vec3<i32>(0)) || 
                    any(pos >= vec3<i32>(params.dimensions))) {
                    continue;
                }
                
                let neighbor_index = get_index_from_pos(vec3<u32>(pos));
                let neighbor = bricks[neighbor_index];
                
                if (is_solid(neighbor)) {
                    let dist = length(vec3<f32>(offset) * f32(search_radius));
                    min_dist = min(min_dist, dist);
                } else if ((neighbor & STATE_MASK) == STATE_EMPTY) {
                    let stored_dist = f32(neighbor & DISTANCE_MASK);
                    if (stored_dist < f32(MAX_DISTANCE)) {
                        let dist = length(vec3<f32>(offset) * f32(search_radius)) + stored_dist;
                        min_dist = min(min_dist, dist);
                    }
                }
            }
        }
    }
    
    if (min_dist < f32(MAX_DISTANCE)) {
        bricks[index] = STATE_EMPTY | u32(min_dist);
    }
}
