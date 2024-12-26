struct PushConstants {
    camera: mat4x4<f32>,
    camera_inverse: mat4x4<f32>,
    dimensions: vec3<u32>,
    packed_resolution: u32,
    flags0: u32,
    flags1: u32,
    dt: f32,
    depth_boost: f32,
    brick_hit: vec3<u32>,
    _padding0: u32,
    voxel_hit: vec3<u32>,
    _padding1: u32,
}

struct BrickHandle {
    raw: u32,
}

struct TraceBrick {
    raw: array<u32, 16>,
    brick_offset: u32,
    palette: u32,
}

var<push_constant> pc: PushConstants;

@group(0) @binding(0)
var<storage, read> brick_handles: array<BrickHandle>;

@group(0) @binding(1)
var<storage, read> trace_bricks: array<TraceBrick>;


@group(0) @binding(2)
var images: binding_array<texture_storage_2d<rgba8unorm, write>, 10>; 

fn get_resolution() -> vec2<f32> {
    let width = (pc.packed_resolution & 0xFFFFu) >> 0u;
    let height = (pc.packed_resolution & 0xFFFF0000u) >> 16u;
    return vec2<f32>(f32(width), f32(height));
}


@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let resolution = get_resolution();
    
    if (global_id.x >= u32(resolution.x) || global_id.y >= u32(resolution.y)) {
        return;
    }
    
    let checker_size = 32u; 
    let is_white = (global_id.x / checker_size + global_id.y / checker_size) % 2u == 0u;
    
    let color = select(
        vec4<f32>(0.0, 0.0, 0.0, 1.0),
        vec4<f32>(1.0, 1.0, 1.0, 1.0),
        is_white
    );
    

    textureStore(images[0], vec2<i32>(global_id.xy), color);
}
