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
    camera, light
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
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let base_color4 = textureSample(tex_0, samp_0, in.tex_coord);

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

    return vec4<f32>(surface_color, base_color4.a);
}
