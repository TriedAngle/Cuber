const MAX_RAY_STEPS: i32 = 64;

struct ComputeUniforms { 
    resolution: vec2<f32>,
    dt: f32,
    _padding: f32,
}

@group(0) @binding(0)
var<uniform> uniforms: ComputeUniforms;

@group(0) @binding(1)
var outputTexture: texture_storage_2d<rgba8unorm, write>;

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
    textureStore(outputTexture, vec2<i32>(global_id.xy), vec4<f32>(0.5, 0.5, 1.0, 1.0));
}

