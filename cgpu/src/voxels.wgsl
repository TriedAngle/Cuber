const MAX_RAY_STEPS: i32 = 64;

struct ComputeUniforms { 
    resolution: vec2<f32>,
    dt: f32, 
    render_mode: u32, // 0 normal, 1 depth
    depth_boost: f32,
    _padding0: f32,
    _padding2: f32,
    _padding3: f32,
    view_projection: mat4x4<f32>,
    inverse_view_projection: mat4x4<f32>,
    camera_position: vec3<f32>,
    _padding1: f32
}

struct Brick { 
    raw: array<u32, 16>,
}

struct BrickBuffer { 
    bricks: array<Brick>,
}


@group(0) @binding(0)
var<uniform> uniforms: ComputeUniforms;

@group(0) @binding(1)
var OutputTexture: texture_storage_2d<rgba8unorm, write>;

@group(0) @binding(2)
var DepthTexture: texture_storage_2d<r32float, write>;

@group(0) @binding(3)
var<storage, read_write> bricks: BrickBuffer; 

fn sd_sphere(p: vec3<f32>, d: f32) -> f32 { 
    return length(p) - d;
}

fn sd_box(p: vec3<f32>, b: vec3<f32>) -> f32 { 
    let d = abs(p) - b;
    return min(max(d.x, max(d.y, d.z)), 0.0) + length(max(d, vec3<f32>(0.0)));
}

fn get_voxel(c: vec3<i32>) -> bool { 
    if (c.x >= 8 && c.x < 16 && c.y >= 0 && c.y < 8 && c.z >= 0 && c.z < 8) {
        let x_local = c.x - 8;
        let y_local = c.y;
        let z_local = c.z;

        let voxel_idx = x_local + y_local * 8 + z_local * 64;
        let u32_index = voxel_idx / 32;
        let bit_index = voxel_idx % 32;

        let voxel_data = bricks.bricks[0].raw[u32(u32_index)];
        let voxel_set = (voxel_data & (1u << u32(bit_index))) != 0u;
        if (voxel_set) {
            return true;
        }
    }
    let p = vec3<f32>(c) + vec3<f32>(0.5);
    let d = min(
        max(-sd_sphere(p, 7.5), sd_box(p, vec3<f32>(6.0))),
        -sd_sphere(p, 25.0),
    );
    return d < 0.0;
}

fn rotate2d(v: vec2<f32>, a: f32) -> vec2<f32> {
    let sin_a = sin(a);
    let cos_a = cos(a);
    return vec2<f32>(
        v.x * cos_a - v.y * sin_a,
        v.y * cos_a + v.x * sin_a,
    );
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

    let ray_dir = normalize(world_pos_3D - uniforms.camera_position);

    let ray_pos = uniforms.camera_position;

    var map_pos = vec3<i32>(floor(ray_pos));

    var delta_dist = abs(vec3<f32>(length(ray_dir)) / ray_dir);
    var ray_step = vec3<i32>(sign(ray_dir));
    var side_dist = (sign(ray_dir) * (vec3<f32>(map_pos) - ray_pos) + (sign(ray_dir) * 0.5) + 0.5) * delta_dist;

    var depth = 1.0;
    var hit = false;

    var mask = vec3<bool>(false);
    var i = 0;
    for (i = 0; i < MAX_RAY_STEPS; i = i + 1) {
        if (get_voxel(map_pos)) {
            hit = true;
            break;
        }
        mask = side_dist <= min(side_dist.yzx, side_dist.zxy);

        let float_mask = select(vec3<f32>(0.0), vec3<f32>(1.0), mask);
        side_dist = side_dist + delta_dist * float_mask;

        let int_mask = select(vec3<i32>(0), vec3<i32>(1), mask);
        map_pos = map_pos + ray_step * int_mask;
    }

    let hit_pos = ray_pos + ray_dir * f32(i);

    if hit {
        let clip_space_hit_pos = uniforms.view_projection * vec4<f32>(hit_pos, 1.0);
        let ndc_hit_pos = clip_space_hit_pos.xyz / clip_space_hit_pos.w;
        depth = ndc_hit_pos.z;
    }

    var color = vec3<f32>(0.0);
    if (mask.x) {
        color = vec3<f32>(0.5);
    }
    if (mask.y) {
        color = vec3<f32>(1.0);
    }
    if (mask.z) {
        color = vec3<f32>(0.75);
    }

    
    if uniforms.render_mode == 0 { 
        textureStore(OutputTexture, vec2<u32>(global_id.xy), vec4<f32>(color, 1.0));
    } else if uniforms.render_mode == 1 { 
        let depth2 = pow(depth, uniforms.depth_boost); 
        textureStore(OutputTexture, vec2<u32>(global_id.xy), vec4<f32>(depth2, depth2, depth2, 1.0));
    } else { 
        textureStore(OutputTexture, vec2<u32>(global_id.xy), vec4<f32>(color, 1.0));
    }

    textureStore(DepthTexture, vec2<u32>(global_id.xy), vec4<f32>(depth, 0.0, 0.0, 0.0));
}

