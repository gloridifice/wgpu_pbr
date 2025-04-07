#define_import_path pbr_functions

#import pbr_type

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
    surface: pbr_type::PBRSurface,
    world2light: vec3<f32>,
    world2camera: vec3<f32>,
    f0: vec3<f32>,
    f90: vec3<f32>,
) -> vec3<f32> {
    let reflectance: f32 = surface.material.reflectance;
    let roughness: f32 = clamp(surface.roughness, 0.089, 1.0);
    let metallic: f32 = surface.material.metallic;
    let normal: vec3<f32> = surface.normal;
    let base_color: vec3<f32> = surface.material.base_color.xyz;

    let nDotL = max(dot(normal, world2light), 0.0);
    let half = normalize(world2light + world2camera);
    let nDotH = max(dot(normal, half), 0.0);
    let nDotV = max(dot(normal, world2camera), 0.0);
    let hDotV = max(dot(half, world2camera), 0.0);

    let diffuse_color = (1.0 - metallic) * base_color;

    // Schlick Fresnel Function, f90 = vec3f(1.0)
    // todo check '- f0 or + f0' ----> v here
    let fresnel: vec3<f32> = f0 + (f90 - f0) * pow5(1.0 - hDotV);

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