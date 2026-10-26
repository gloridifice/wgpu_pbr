import package::libs::vertex::FullscreenV2F;

@group(0) @binding(0) var scene_color: texture_2d<f32>;
@group(0) @binding(1) var<uniform> exposure: vec4<f32>;

@fragment
fn fs_main(in: FullscreenV2F) -> @location(0) vec4<f32> {
    let hdr = max(textureLoad(scene_color, vec2<i32>(in.clip_position.xy), 0).rgb, vec3f(0.0));
    let exposed = hdr * exposure.x;
    let mapped = exposed / (vec3f(1.0) + exposed);
    // The SDR sRGB attachment performs the encoding, exactly once.
    return vec4f(mapped, 1.0);
}
