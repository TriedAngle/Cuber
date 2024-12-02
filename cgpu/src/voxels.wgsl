const MAX_RAY_STEPS: i32 = 64;

struct ComputeUniforms { 
    resolution: vec2<f32>,
    dt: f32,
    _padding: f32,
}

@group(0) @binding(0)
var<uniform> uniforms: ComputeUniforms;

@group(0) @binding(1)
var OutputTexture: texture_storage_2d<rgba8unorm, write>;

fn sd_sphere(p: vec3<f32>, d: f32) -> f32 { 
    return length(p) - d;
}

fn sd_box(p: vec3<f32>, b: vec3<f32>) -> f32 { 
    let d = abs(p) - b;
    return min(max(d.x, max(d.y, d.z)), 0.0) + length(max(d, vec3<f32>(0.0)));
}

fn get_voxel(c: vec3<i32>) -> bool { 
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

    var camera_dir = vec3<f32>(0.0, 0.0, 0.8);
    var camera_plane_u = vec3<f32>(1.0, 0.0, 0.0);
    var camera_plane_v = vec3<f32>(0.0, 1.0, 0.0) * uniforms.resolution.y / uniforms.resolution.x;
    var ray_dir = camera_dir + screen_pos.x * camera_plane_u + screen_pos.y * camera_plane_v;
    var ray_pos = vec3<f32>(0.0, 2.0 * sin(uniforms.dt * 2.7), -12.0);

    let rotation = uniforms.dt;
    let ray_pos_xz = rotate2d(ray_pos.xz, rotation);
    ray_pos.x = ray_pos_xz.x;
    ray_pos.z = ray_pos_xz.y;

    let ray_dir_xz = rotate2d(ray_dir.xz, rotation);
    ray_dir.x = ray_dir_xz.x;
    ray_dir.z = ray_dir_xz.y;

    var map_pos = vec3<i32>(floor(ray_pos));

    var delta_dist = abs(vec3<f32>(length(ray_dir)) / ray_dir);
    var ray_step = vec3<i32>(sign(ray_dir));
    var side_dist = (sign(ray_dir) * (vec3<f32>(map_pos) - ray_pos) + (sign(ray_dir) * 0.5) + 0.5) * delta_dist;

    var mask = vec3<bool>(false);
    for (var i = 0; i < MAX_RAY_STEPS; i = i + 1) {
        if (get_voxel(map_pos)) {
            break;
        }
        mask = side_dist <= min(side_dist.yzx, side_dist.zxy);

        let float_mask = select(vec3<f32>(0.0), vec3<f32>(1.0), mask);
        side_dist = side_dist + delta_dist * float_mask;

        let int_mask = select(vec3<i32>(0), vec3<i32>(1), mask);
        map_pos = map_pos + ray_step * int_mask;
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

    textureStore(OutputTexture, vec2<i32>(global_id.xy), vec4<f32>(color, 1.0));
}

