import package::libs::vertex::{FullscreenV2F};
import package::libs::primary::pbr_type::{PBRSurface, unpack_g_buffer};
import package::libs::primary::global_bindings::{
    camera, light, env_cubemap, env_cubemap_sampler,
};
import package::libs::ibl_functions;
import package::libs::pbr_functions;
import package::libs::pbr_functions::{
    pow2,
};
import package::libs::shadow;
import package::libs::light_bindings::{
    PointLight, point_lights,
};

@group(1) @binding(0) var depth_tex: texture_depth_2d;
@group(1) @binding(1) var g_buffer_tex: texture_2d<u32>;

fn skybox(ndc_xy: vec2<f32>) -> vec3<f32> {
    let world_far = camera.inv_view_proj * vec4f(ndc_xy, 1.0, 1.0);
    let dir = normalize(world_far.xyz / world_far.w - camera.position);
    return textureSampleLevel(env_cubemap, env_cubemap_sampler, dir, 0.0).xyz;
}

/// 由屏幕 uv + 深度缓冲读取的 NDC z 重建世界坐标。
fn reconstruct_world_pos(uv: vec2<f32>, z_ndc: f32) -> vec3<f32> {
    // uv 原点在左上，y 朝下；NDC y 朝上，因此对 y 取反。
    let ndc_xy = (uv * 2.0 - vec2<f32>(1.0)) * vec2<f32>(1.0, -1.0);
    let h = camera.inv_view_proj * vec4<f32>(ndc_xy, z_ndc, 1.0);
    return h.xyz / h.w;
}

@fragment
fn fs_main(in: FullscreenV2F) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(in.clip_position.xy);
    let z_ndc = textureLoad(depth_tex, pixel, 0);

    // 深度缓冲清值为 1.0：未被几何写入的像素当作天空背景。
    if z_ndc >= 1.0 {
        return vec4f(skybox((in.uv - vec2f(0.5)) * vec2f(1.0, -1.0)), 1.0);
    }

    let world_pos = reconstruct_world_pos(in.uv, z_ndc);

    let g_buffer: vec4<u32> = textureLoad(g_buffer_tex, pixel, 0);
    let surface: PBRSurface = unpack_g_buffer(g_buffer);

    var surface_color = vec3f(0.0);

    let metallic = surface.material.metallic;
    let base_color = surface.material.base_color.xyz;

    let f0: vec3<f32> = vec3<f32>(0.16 * pow2(surface.material.reflectance) * (1.0 - metallic))
         + base_color * metallic;

    let world2camera = -camera.direction;
    // + Parallel Lighting
    surface_color += pbr_functions::calculate_light(
        light.color.xyz,
        light.intensity,
        surface,
        -light.direction,
        world2camera,
        f0,
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
        );
    }

    /// + Image based Lighting
    let ibl = ibl_functions::evaluate_ibl(
        surface.normal,
        surface.normal,
        base_color,
        f0,
        surface.material.perceptual_roughness
    );

    surface_color += ibl;

    /// -- Shadowing --
    let shadow_v = shadow::shadow(world_pos, z_ndc);
    surface_color *= mix(vec3<f32>(0.5), vec3<f32>(1.0), shadow_v);

    return vec4<f32>(surface_color, 1.0);
}
