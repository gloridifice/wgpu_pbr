import package::libs::vertex::{VertexInput};

struct TransformUniform {
    model: mat4x4<f32>,
    rotation: mat3x3<f32>,
}

struct CSM {
    view_proj: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> csm: CSM;

@group(1) @binding(0)
var<uniform> transform: TransformUniform;

@vertex
fn vs_main(
    in: VertexInput,
) -> @builtin(position) vec4<f32> {
    var clip_position = csm.view_proj * transform.model * vec4f(in.position, 1.0);
    return clip_position;
}
