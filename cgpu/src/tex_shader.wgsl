struct CameraUniform { 
    view_projection: mat4x4<f32>,
}

struct ModelUniform { 
    transform: mat4x4<f32>,
}

@group(2) @binding(0)
var<uniform> camera: CameraUniform;

@group(3) @binding(0)
var<uniform> model: ModelUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
}

struct VertexOutput { 
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) frag_depth: f32,
    @location(2) clip_space_position: vec4<f32>,
}

@vertex
fn vs_main(
    vertex: VertexInput,
) -> VertexOutput { 
    var out: VertexOutput;
    var world_position = model.transform * vec4<f32>(vertex.position, 1.0);
    let clip_position = camera.view_projection * world_position;
    out.clip_position = clip_position;
    out.clip_space_position = clip_position;
    out.tex_coords = vertex.tex_coords;
    out.frag_depth = clip_position.z / clip_position.w; // NDC depth in range [-1, 1]
    return out;
}

@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;

@group(1) @binding(0)
var ComputeDepthTexture: texture_2d<f32>;

@group(1) @binding(1)
var ComputeDepthSampler: sampler;

@fragment
fn fs_main(
    in: VertexOutput,
) -> @location(0) vec4<f32> {                               
    let ndc = in.clip_space_position.xyz / in.clip_space_position.w;

    // Map NDC coordinates to UV coordinates [0, 1]
    let uv = ndc.xy * 0.5 + vec2<f32>(0.5);

    let depth_from_raytrace = textureSample(
        ComputeDepthTexture,
        ComputeDepthSampler,
        uv,
    ).x;

    let frag_depth = in.frag_depth;

    let normalized_frag_depth = frag_depth * 0.5 + 0.5;
    let normalized_depth_from_raytrace = depth_from_raytrace;

    if (normalized_depth_from_raytrace < 0.9999 && normalized_depth_from_raytrace < normalized_frag_depth) {
        discard;
    }

    return textureSample(t_diffuse, s_diffuse, in.tex_coords);
}
