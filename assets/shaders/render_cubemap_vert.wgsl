#import vertex::{ CubeVertexInput, CubemapVertexOutput }

@group(0) @binding(0) var<uniform> view_proj: mat4x4<f32>;

@vertex
fn vs_main(in: CubeVertexInput) -> CubemapVertexOutput{
    var ret: CubemapVertexOutput;
    ret.local_position = in.position;
    ret.clip_position = view_proj * vec4<f32>(in.position, 1.0);
    return ret;
}
