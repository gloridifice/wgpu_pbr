#import vertex::{VertexInput}
#import pbr_type::{ StandardMaterial, PBRSurface }
#import pbr_type
#import pbr_functions
#import pbr_functions::{
    pow2,
}
#import light_bindings::{
    PointLight, point_lights
}
#import shadow
#import global_bindings::{
    camera, light, rendered_image, rendered_sampler
}
#import material_bindings::{
    pbr_mat, tex_0, samp_0, normal_tex, normal_samp,
}
#import object_bindings::{
    transform
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) vertex_color: vec4<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec3<f32>,
    @location(3) tex_coord: vec2<f32>,
    @location(4) world_pos: vec3<f32>,
};

@vertex
fn vs_main(
    model: VertexInput,
) -> VertexOutput {
    let model_mat = transform.model;

    var out: VertexOutput;
    out.vertex_color = model.color;
    out.world_pos = (model_mat * vec4<f32>(model.position, 1.0)).xyz;
    out.normal = transform.normal * model.normal;
    out.tangent = transform.normal * model.tangent;
    out.tex_coord = model.tex_coord;
    out.clip_position = camera.view_proj * vec4<f32>(out.world_pos, 1.0);
    return out;
}

@fragment
fn fs_main(
    in: VertexOutput,
) -> @location(0) vec4<f32> {
    let frag_coord = in.clip_position;
    let base_color4 = textureSample(tex_0, samp_0, in.tex_coord) * pbr_mat.color;

    let n_normal = normalize(in.normal);
    let n_tangent = normalize(in.tangent);
    let bitangent = cross(n_normal, n_tangent);
    let tbn = mat3x3<f32>(n_tangent, bitangent, n_normal);
    let tangent_space_normal = textureSample(normal_tex, normal_samp, in.tex_coord).xyz * 2.0 - 1.0;
    let normal = normalize(tbn * tangent_space_normal);

    var surface: PBRSurface = pbr_type::pbr_surface_new();
    var material: StandardMaterial = pbr_type::standard_material_new();
    surface.normal = normal;
    material.base_color = base_color4;
    material.metallic = pbr_mat.metallic;
    material.perceptual_roughness = pbr_mat.roughness;
    material.reflectance = pbr_mat.reflectance;
    surface.material = material;

    // End get world pos and PBRSurface
    let metallic = surface.material.metallic;
    let base_color = base_color4.xyz;
    let world_pos = in.world_pos;

    let f0: vec3<f32> =
        vec3<f32>(0.16 * pow2(surface.material.reflectance) * (1.0 - metallic))
         + base_color * metallic;

    var diffuse_color = vec3f(0.0);
    var specular_color = vec3f(0.0);

    let world2camera = -camera.direction;

    // + Parallel Lighting
    let parallel_light_result = pbr_functions::calculate_light_separately(
        light.color.xyz,
        light.intensity,
        surface,
        -light.direction,
        world2camera,
        f0,
    );
    diffuse_color += parallel_light_result.diffuse;
    specular_color += parallel_light_result.specular;

    // + Point Lighting
    // let point_lights_num = light.lights_nums.x;

    // for (var i = 0u; i < point_lights_num; i += 1u) {
    //     let li = point_lights[i];
    //     let world2light_unnorm = li.position.xyz - world_pos;
    //     let dist = length(world2light_unnorm);
    //     if dist > li.distance { continue; }
    //     let dir = normalize(world2light_unnorm);

    //     let radiance = li.intensity / ((li.decay * pow2(dist)) + 0.001); // + 0.001 for division safety
    //     surface_color += pbr_functions::calculate_light(
    //         li.color.xyz,
    //         radiance,
    //         surface,
    //         dir,
    //         world2camera,
    //         f0,
    //         f90,
    //     );
    // }

    // + Image based Lighting
    let ibl_result = ibl_functions::evaluate_ibl_separately(
                        surface.normal,
                        world2camera,
                        base_color,
                        f0,
                        surface.material.perceptual_roughness
                    );

    diffuse_color += ibl_result.diffuse;
    specular_color += ibl_result.specular;

    /// -- Shadowing --
    let shadow = shadow::sample_directional_shadow(world_pos);
    let shadow_factor = mix(vec3<f32>(0.5), vec3<f32>(1.0), shadow);
    diffuse_color *= shadow_factor;
    specular_color *= shadow_factor;

    /// Blending
    let normal_ndc = normalize((camera.view_proj * vec4<f32>(normal, 1.0)).xyz);
    let uv = frag_coord.xy / camera.screen_resolution - normal_ndc.xy * 0.1;
    let prev_color4 = textureSample(rendered_image, rendered_sampler, uv);

    let alpha = base_color4.a;
    let ret_color = diffuse_color * alpha + prev_color4.xyz * (1.0 - alpha) + specular_color;
    return vec4f(ret_color, 1.0);
}
