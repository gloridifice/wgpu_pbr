import package::libs::primary::global_bindings::{
    light, csm, csm_sampler, csm_info,
};

fn shadow(world_pos: vec3<f32>, z_ndc: f32) -> f32 {
    return sample_csm(world_pos, z_ndc);
}

fn sample_csm(world_pos: vec3<f32>, z_ndc: f32) -> f32 {
    for (var layer = 0; layer < 8; layer++) {
        let bound = csm_info.bounds[layer];
        if z_ndc < bound.far && z_ndc > bound.near {
            let pos = bound.light_space_matrix * vec4<f32>(world_pos, 1.0);
            let light_space_clip_pos = pos.xyz / pos.w;
            let coords = light_space_clip_pos.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5);
            let light_space_z = light_space_clip_pos.z;
            var sample: f32 = 0.0;
            for (var i = -1; i <= 1; i++) {
                for (var j = -1; j <= 1; j++) {
                    let uv = coords + vec2f(vec2(i, j)) / csm_info.texture_size;
                    sample += textureSampleCompare(csm, csm_sampler, uv, layer, light_space_z);
                }
            }
            sample /= 9.0;

            //return light_space_z;
            return sample;
        }
    }

    return 1.0;
}

///fn sample_directional_shadow(world_pos: vec3<f32>) -> f32 {
///    let pos = light.view_proj * vec4<f32>(world_pos, 1.0);
///    let light_space_clip_pos = pos.xyz / pos.w;
///    let coords = light_space_clip_pos.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5);
///    let current_depth = light_space_clip_pos.z;
///    var sample: f32 = 0.0;
///    for (var i = -1; i <= 1; i++) {
///        for (var j = -1; j <= 1; j++) {
///            sample += textureSampleCompare(
///                directional_shadow_map,
///                directional_shadow_map_comparison_sampler,
///                coords + vec2f(vec2(i, j)) / 4096.0,
///                current_depth
///            );
///        }
///    }
///    return sample / 9.;
///}
