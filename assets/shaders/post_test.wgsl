
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>
}

@group(0) @binding(0)
var main_tex: texture_2d<f32>;
@group(0) @binding(1)
var main_sampler: sampler;

// @group(0) @binding(0)
// var depth_tex: texture_2d;
// @group(0) @binding(1)
// var depth_sampler: sampler;

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(main_tex, main_sampler, input.uv);

    return vec4<f32>(vec3<f32>(1.0) - color.xyz, 1.0);
}