struct CameraUniform { 
    view_projection: mat4x4<f32>,
}

struct ModelUniform { 
    transform: mat4x4<f32>,
}

@group(1) @binding(0)
var<uniform> camera: CameraUniform;

@group(2) @binding(0)
var<uniform> model: ModelUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
}

struct VertexOutput { 
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(
    vertex: VertexInput,
) -> VertexOutput { 
    var out: VertexOutput;
    var world_position = model.transform * vec4<f32>(vertex.position, 1.0);
    out.clip_position = camera.view_projection * world_position;
    out.tex_coords = vertex.tex_coords;
    return out;
}


@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;

@fragment
fn fs_main(
    in: VertexOutput
) -> @location(0) vec4<f32> { 
    return textureSample(t_diffuse, s_diffuse, in.tex_coords);
}

