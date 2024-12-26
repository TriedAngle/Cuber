struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

struct FragmentOutput {
    @location(0) color: vec4<f32>,   
}

struct PushConstants {
    mode: u32
}

var<push_constant> pc: PushConstants;

@group(0) @binding(3)
var images: binding_array<texture_2d<f32>, 10>; 

@group(0) @binding(4)
var samplers: binding_array<sampler, 10>;


@vertex
fn vmain(@builtin(vertex_index) index: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(1.0, 1.0),
    );

    var out: VertexOutput;
    out.pos = vec4<f32>(positions[index], 0.0, 1.0);
    out.uv = (positions[index] + vec2<f32>(1.0)) * 0.5;
    return out;
}

@fragment
fn fmain(in: VertexOutput) -> FragmentOutput {
    let image = images[pc.mode];
    let image_sampler = samplers[pc.mode];

    let color = textureSample(image, image_sampler, in.uv);
    return FragmentOutput(color);
}

