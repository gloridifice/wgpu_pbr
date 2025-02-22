#import vertex::{ FullscreenV2F }
#import pbr_type
#import pbr_type::{ PBRSurface }
#import global_bindings::{
    camera, light, directional_shadow_map, directional_shadow_map_comparison_sampler,
    env_cubemap, env_cubemap_sampler,
}

struct PointLight {
    color: vec4<f32>,
    position: vec4<f32>,
    intensity: f32,
    distance: f32,
    decay: f32,
}

@group(1) @binding(0) var g_samp: sampler;
@group(1) @binding(1) var world_pos_tex: texture_2d<f32>;
@group(1) @binding(2) var g_buffer_tex: texture_2d<u32>;

@group(2) @binding(0) var<storage, read> point_lights: array<PointLight>;

const PI: f32 = radians(180.0);

fn pow2(a: f32) -> f32 {
    return a * a;
}

fn pow5(a: f32) -> f32 {
    let a2 = a * a;
    return a2 * a2 * a;
}

fn V_smith_ggx_correlated_fast(nDotV: f32, nDotL: f32, roughness: f32) -> f32 {
    let GGXV = nDotL * (nDotV * (1.0 - roughness) + roughness);
    let GGXL = nDotV * (nDotL * (1.0 - roughness) + roughness);
    return 0.5 / (0.001 + GGXL + GGXV);
}

fn calculate_light(
    light_color: vec3<f32>,
    light_diffuse_intensity: f32,
    surface: PBRSurface,
    world2light: vec3<f32>,
    world2camera: vec3<f32>,
) -> vec3<f32> {
    let reflectance: f32 = surface.material.reflectance;
    let roughness: f32 = clamp(surface.roughness, 0.089, 1.0);
    let metallic: f32 = surface.material.metallic;
    let normal: vec3<f32> = surface.normal;
    let base_color: vec3<f32> = surface.material.base_color;

    let nDotL = max(dot(normal, world2light), 0.0);
    let half = normalize(world2light + world2camera);
    let nDotH = max(dot(normal, half), 0.0);
    let nDotV = max(dot(normal, world2camera), 0.0);
    let hDotV = max(dot(half, world2camera), 0.0);

    let diffuse_color = (1.0 - metallic) * base_color;

    // Schlick Fresnel Function
    let f0: vec3<f32> = vec3<f32>(0.16 * pow2(reflectance) * (1.0 - metallic)) + base_color * metallic;
    let fresnel: vec3<f32> = f0 + (vec3<f32>(1.0) + f0) * pow5(1.0 - hDotV);

    // ! Diffuse BRDF -------------
    let diffuse_brdf = diffuse_color / PI;

    // ! Specular BRDF ------------
    // - GGX Normal Distribution Function
    let roughness2 = pow2(roughness);
    let D_GGX = roughness2 / (PI * pow2(pow2(nDotH) * (roughness2 - 1.0) + 1.0));

    // - Geometry Function
    // V = G / (4.0 * nDotL * nDotV);
    let V_SmithGGX = V_smith_ggx_correlated_fast(nDotV, nDotL, roughness);

    // final specular BRDF
    let specular_brdf = fresnel * (D_GGX * V_SmithGGX);

    let light_intensity = light_color * light_diffuse_intensity;

    let ret = (specular_brdf + diffuse_brdf) * light_intensity * nDotL;

    return ret;
}

@fragment
fn fs_main(in: FullscreenV2F) -> @location(0) vec4<f32> {
    let world_pos: vec3<f32> = textureSample(world_pos_tex, g_samp, in.uv).xyz;
    let g_buffer: vec4<u32> = textureLoad(g_buffer_tex, vec2<i32>(in.clip_position.xy), 0);

    let surface: PBRSurface = pbr_type::unpack_g_buffer(g_buffer);

    var surface_color = vec3<f32>(0.0);

    // Parallel Light
    surface_color += calculate_light(
        light.color.xyz,
        light.intensity,
        surface,
        -light.direction,
        -camera.direction
    );

    // Point Lights
    let point_lights_num = light.lights_nums.x;

    for (var i = 0u; i < point_lights_num; i += 1u) {
        let li = point_lights[i];
        let world2light_unnorm = li.position.xyz - world_pos;
        let world2camera_unnorm = camera.position - world_pos;
        let dist = length(world2light_unnorm);
        if dist > li.distance { continue; }
        let dir = normalize(world2light_unnorm);

        let radiance = li.intensity / ((li.decay * pow2(dist)) + 0.001); // + 0.001 for division safety
        surface_color += calculate_light(
            li.color.xyz,
            radiance,
            surface,
            dir,
            normalize(world2camera_unnorm),
        );
    }

    surface_color += vec3<f32>(0.1);

    let shadow = sample_directional_shadow(world_pos);
    surface_color *= mix(vec3<f32>(0.5), vec3<f32>(1.0), shadow);

    return vec4<f32>(surface_color, 1.0);
    // return vec4<f32>(surface.material.base_color, 1.0);
    // return vec4<f32>(world_pos, 1.0);
    // return vec4<f32>(normal * 0.5 + vec3<f32>(0.5), 1.0);
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
                coords + vec2f(vec2(i, j)) / 2048.0,
                current_depth
            );
        }
    }
    return sample / 9.;
}
