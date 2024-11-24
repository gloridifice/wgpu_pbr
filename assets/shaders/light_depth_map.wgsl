struct VertexInput {
    @location(0) position: vec3<f32>,
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
var<uniform> light: LightUniform;

@group(1) @binding(0)
var<uniform> transform: TransformUniform;

@vertex
fn vs_main(
    model: VertexInput,
) -> @builtin(position) vec4<f32> {
    var clip_position = light.view_proj * transform.model * vec4<f32>(model.position, 1.0);
    return clip_position;
}