@group(0) @binding(0) var preview_tex: texture_2d<f32>;
@group(0) @binding(1) var preview_sampler: sampler;

@fragment
fn fs_main(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    return textureSample(preview_tex, preview_sampler, uv);
}
