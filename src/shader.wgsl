struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) tex_coord: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coord: vec2<f32>,
};

struct CameraUniform {
    view_proj: mat4x4<f32>
}
struct TransformUniform {
    model: mat4x4<f32>
}
@group(0) @binding(0)
var<uniform> transform: TransformUniform;

@group(1) @binding(0)
var<uniform> camera: CameraUniform;

@vertex
fn vs_main(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.color = model.color;
    out.clip_position =  camera.view_proj * transform.model * vec4<f32>(model.position, 1.0);
    out.normal = model.normal;
    out.tex_coord = model.tex_coord;
    return out;
}

// Fragment shader

@group(2) @binding(0)
var tex_0: texture_2d<f32>;
@group(2) @binding(1)
var samp_0: sampler;


@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    return textureSample(tex_0, samp_0, in.tex_coord);
}
