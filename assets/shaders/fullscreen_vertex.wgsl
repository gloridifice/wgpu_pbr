#import vertex::{FullscreenV2F}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
) -> FullscreenV2F {
    let uv = vec2<f32>(f32(vertex_index >> 1u), f32(vertex_index & 1u)) * 2.0;
    let clip_position = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    return FullscreenV2F(clip_position, uv);
}
