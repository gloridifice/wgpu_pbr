struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) tex_coord: vec2<f32>,
};

struct TransformUniform {
    model: mat4x4<f32>,
    rotation: mat3x3<f32>,
}

struct LightUniform {
    direction: vec3<f32>,
    color: vec4<f32>,
    view_proj: mat4x4<f32>,
    intensity: f32,
}

@group(0) @binding(0)
var<uniform> light: mat4x4<f32>;

@group(1) @binding(0)
var<uniform> transform: TransformUniform;

@vertex
fn vs_main(
    in: VertexInput,
) -> @builtin(position) vec4<f32> {
    var clip_position = light * transform.model * vec4<f32>(in.position, 1.0);
    return clip_position;
}
