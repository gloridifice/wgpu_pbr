
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
) -> VertexOutput {
    var out: VertexOutput;
    if (vertex_index == 0u) {
        out.clip_position = vec4<f32>(-1.0, -1.0, 0.0, 1.0);
    } else {
        if (vertex_index == 1u) {
            out.clip_position = vec4<f32>(3.0, -1.0, 0.0, 1.0);
        } else {
            out.clip_position = vec4<f32>(-1.0, 3.0, 0.0, 1.0);
        }
    }
    out.uv = out.clip_position.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5);
    return out;
}