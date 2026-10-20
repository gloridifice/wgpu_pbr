import package::libs::vertex::{ VertexInput };
import package::libs::primary::global_bindings::{ camera };

// Transform uniform (matches ObjectBindGroupLayout at group 2).
struct TransformUniform {
    model: mat4x4<f32>,
    normal_matrix: mat4x4<f32>,
};
@group(2) @binding(0) var<uniform> transform: TransformUniform;

// Per-entity picking ID (group 1, binding 0).
struct PickIDUniform {
    id: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
};
@group(1) @binding(0) var<uniform> pick_id: PickIDUniform;

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

// Only the pixel under the cursor is rasterised (the pass sets a 1x1 scissor
// rect), so every surviving fragment writes its own entity ID.
@fragment
fn fs_main() -> @location(0) vec4<u32> {
    return vec4<u32>(pick_id.id, 0u, 0u, 0u);
}
