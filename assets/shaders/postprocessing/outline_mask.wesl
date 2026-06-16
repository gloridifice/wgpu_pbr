import package::libs::vertex::{ VertexInput };
import package::libs::primary::global_bindings::{ camera };
import package::libs::primary::object_bindings::{ transform };

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
};

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    let world_pos = (transform.model * vec4<f32>(model.position, 1.0)).xyz;
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.clip_position.xy, 0.0, 1.0);
}
