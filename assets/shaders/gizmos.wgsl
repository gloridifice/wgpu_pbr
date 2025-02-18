#import vertex::{VertexInput}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
};

struct CameraUniform {
    view_proj: mat4x4<f32>,
}
struct TransformUniform {
    model: mat4x4<f32>,
    normal: mat3x3<f32>,
}

struct MaterialUnifrom {
    color: vec4<f32>
}

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

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
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(1.0);
}
