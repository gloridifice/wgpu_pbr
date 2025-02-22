#import global_bindings::{
    camera, env_cubemap, env_cubemap_sampler,
}
#import vertex::{ CubeVertexInput }

struct V2F {
    @builtin(position) clip_position: vec4<f32>,
    @location(1) local_position: vec3<f32>,
}

@vertex
fn vs_main(
    in: CubeVertexInput
) -> V2F{
    var o: V2F;
    let mat4 = camera.view_proj;
    let camViewProj3x3 = mat3x3(mat4[0].xyz, mat4[1].xyz, mat4[2].xyz);
    o.clip_position = vec4f(camViewProj3x3 * in.position, 1.0);
    o.local_position = in.position;
    return o;
}

@fragment
fn fs_main(in: V2F) -> @location(0) vec4<f32> {
    return textureSample(env_cubemap, env_cubemap_sampler, in.local_position);
}
