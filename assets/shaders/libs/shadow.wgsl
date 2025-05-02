#define_import_path shadow
#import global_bindings::{
    light, directional_shadow_map, directional_shadow_map_comparison_sampler,
}

fn sample_directional_shadow(world_pos: vec3<f32>) -> f32{
    let pos = light.view_proj * vec4<f32>(world_pos, 1.0);
    let light_space_clip_pos = pos.xyz / pos.w;
    let coords = light_space_clip_pos.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5);
    let current_depth = light_space_clip_pos.z;
    var sample: f32 = 0.0;
    for (var i = -1; i <= 1; i++) {
        for (var j = -1; j <= 1; j++) {
            sample += textureSampleCompare(
                directional_shadow_map,
                directional_shadow_map_comparison_sampler,
                coords + vec2f(vec2(i, j)) / 4096.0,
                current_depth
            );
        }
    }
    return sample / 9.;
}
