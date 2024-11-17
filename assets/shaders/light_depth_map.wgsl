struct VertexInput {
    @location(0) position: vec3<f32>,
};

struct TransformUniform {
    model: mat4x4<f32>,
    rotation: mat3x3<f32>,
}
@group(0) @binding(0)
var<uniform> transform: TransformUniform;

@group(1) @binding(0)
var<uniform> light_space: mat4x4<f32>;


@vertex
fn vs_main(
    model: VertexInput,
) -> @builtin(position) vec4<f32> {
    var clip_position = light_space * transform.model * vec4<f32>(model.position, 1.0);
    return clip_position;
}
