struct TransformUniform {
    model: mat4x4<f32>,
    normal: mat3x3<f32>,
}

@group(2) @binding(0)
var<uniform> transform: TransformUniform;
