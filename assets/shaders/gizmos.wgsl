struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) tex_coord: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
};

struct FragmentOutput{
    @location(0) color: vec4<f32>,
}

struct CameraUniform {
    view_proj: mat4x4<f32>,
}
struct TransformUniform {
    model: mat4x4<f32>,
    normal: mat3x3<f32>,
}
struct LightUniform {
    direction: vec3<f32>,
    color: vec4<f32>,
    view_proj: mat4x4<f32>,
    intensity: f32,
}

struct MaterialUnifrom {
    color: vec4<f32>
}
@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(0) @binding(1)
var<uniform> light: LightUniform;

@group(1) @binding(0)
var<uniform> material: MaterialUnifrom;

@group(2) @binding(0)
var<uniform> transform: TransformUniform;

@vertex
fn vs_main(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * transform.model * vec4<f32>(model.position, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    var o: FragmentOutput;
    o.color = material.color;
    return o;
}
