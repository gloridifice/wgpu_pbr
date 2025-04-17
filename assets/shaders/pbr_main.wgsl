#import vertex::{ FullscreenV2F }
#import pbr_type::{ PBRSurface, unpack_g_buffer }
#import global_bindings::{
    camera, light, directional_shadow_map, directional_shadow_map_comparison_sampler,
    env_cubemap, env_cubemap_sampler,
}
#import ibl_functions
#import pbr_functions
#import pbr_functions::{
    pow2
}
#import shadow
#import light_bindings::{
    PointLight, point_lights
}

@group(1) @binding(0) var g_samp: sampler;
@group(1) @binding(1) var world_pos_tex: texture_2d<f32>;
@group(1) @binding(2) var g_buffer_tex: texture_2d<u32>;


@fragment
fn fs_main(in: FullscreenV2F) -> @location(0) vec4<f32> {
    let world_pos: vec3<f32> = textureSample(world_pos_tex, g_samp, in.uv).xyz;
    let g_buffer: vec4<u32> = textureLoad(g_buffer_tex, vec2<i32>(in.clip_position.xy), 0);

    let surface: PBRSurface = unpack_g_buffer(g_buffer);

    if(all(surface.normal == vec3f(0.0))) {
        discard;
    }

    let metallic = surface.material.metallic;
    let base_color = surface.material.base_color.xyz;

    let f0: vec3<f32> =
        vec3<f32>(0.16 * pow2(surface.material.reflectance) * (1.0 - metallic))
         + base_color * metallic;
    let f90 = vec3<f32>(1.0);

    var surface_color = vec3<f32>(0.0);

    let world2camera = -camera.direction;
    // + Parallel Lighting
    surface_color += pbr_functions::calculate_light(
        light.color.xyz,
        light.intensity,
        surface,
        -light.direction,
        world2camera,
        f0,
        f90,
    );

    // + Point Lighting
    let point_lights_num = light.lights_nums.x;

    for (var i = 0u; i < point_lights_num; i += 1u) {
        let li = point_lights[i];
        let world2light_unnorm = li.position.xyz - world_pos;
        let dist = length(world2light_unnorm);
        if dist > li.distance { continue; }
        let dir = normalize(world2light_unnorm);

        let radiance = li.intensity / ((li.decay * pow2(dist)) + 0.001); // + 0.001 for division safety
        surface_color += pbr_functions::calculate_light(
            li.color.xyz,
            radiance,
            surface,
            dir,
            world2camera,
            f0,
            f90,
        );
    }

    /// + Image based Lighting
    let ibl = ibl_functions::evaluate_ibl(
                        surface.normal,
                        world2camera,
                        base_color,
                        f0,
                        f90,
                        surface.material.perceptual_roughness
                    );

    surface_color += ibl;
    surface_color += vec3<f32>(0.1);

    /// -- Shadowing --
    let shadow = shadow::sample_directional_shadow(world_pos);
    surface_color *= mix(vec3<f32>(0.5), vec3<f32>(1.0), shadow);

    return vec4<f32>(surface_color, 1.0);
    // return vec4<f32>(ibl, 1.0);
    // return vec4<f32>(surface.material.base_color.xyz, 1.0);
    // return vec4<f32>(world_pos, 1.0);
    // return vec4<f32>(surface.normal * 0.5 + vec3<f32>(0.5), 1.0);
    // a.z = 1.0;
    // return a;
}
