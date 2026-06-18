@group(0) @binding(0) var depth_tex: texture_depth_2d;
@group(0) @binding(1) var out_tex: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let depth_val = textureLoad(depth_tex, id.xy, 0);
    // Invert so geometry (closer, smaller depth) appears bright and the
    // empty far plane (depth = 1.0) appears black.  The power curve
    // amplifies contrast for the tightly-clustered depth values produced
    // by CSM's orthographic projections.
    let v = pow(1.0 - depth_val, 0.5);
    textureStore(out_tex, id.xy, vec4<f32>(v, v, v, 1.0));
}
