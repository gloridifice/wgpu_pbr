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

fn diffuse_brdf(metallic: f32, base_color: vec3<f32>) -> vec3<f32> {
    let diffuse_color = (1.0 - metallic) * base_color;
    // - Lambertan diffuse brdf
    return diffuse_color / PI;
}

fn specular_brdf(f0: vec3<f32>, f90: vec3<f32>, roughness: f32, nDotH: f32, nDotL: f32, nDotV: f32, hDotV: f32) -> vec3<f32>{
    // Schlick Fresnel Function, f90 = vec3f(1.0)
    // todo check '- f0 or + f0' ----> v here
    let fresnel: vec3<f32> = f0 + (f90 - f0) * pow5(1.0 - hDotV);

    // ! Specular BRDF ------------
    // - GGX Normal Distribution Function
    let roughness2 = pow2(roughness);
    let D_GGX = roughness2 / (PI * pow2(pow2(nDotH) * (roughness2 - 1.0) + 1.0));

    // - Geometry Function
    // V = G / (4.0 * nDotL * nDotV);
    let V_SmithGGX = V_smith_ggx_correlated_fast(nDotV, nDotL, roughness);

    // final specular BRDF
    let specular_brdf = fresnel * (D_GGX * V_SmithGGX);
    return specular_brdf;
}

struct LightCalculationResult {
    diffuse: vec3<f32>,
    specular: vec3<f32>,
}

fn calculate_light(
    light_color: vec3<f32>,
    light_diffuse_intensity: f32,
    surface: pbr_type::PBRSurface,
    world2light: vec3<f32>,
    world2camera: vec3<f32>,
    f0: vec3<f32>,
) -> vec3<f32> {
    let result = calculate_light_separately(
        light_color,
        light_diffuse_intensity,
        surface,
        world2light,
        world2camera,
        f0,
    );
    return result.diffuse + result.specular;
}

fn calculate_light_separately(
    light_color: vec3<f32>,
    light_diffuse_intensity: f32,
    surface: pbr_type::PBRSurface,
    world2light: vec3<f32>,
    world2camera: vec3<f32>,
    f0: vec3<f32>,
) -> LightCalculationResult {
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
    let lDotH = max(dot(world2light, half), 0.0);
    let f90 = vec3f(0.5 + 2.0 * roughness * lDotH * lDotH);

    let diffuse_brdf = diffuse_brdf(metallic, base_color);
    let specular_brdf = specular_brdf(f0, f90, roughness, nDotH, nDotL, nDotV, hDotV);

    let light_intensity = light_color * light_diffuse_intensity;

    return LightCalculationResult(
        diffuse_brdf * light_intensity * nDotL,
        specular_brdf * light_intensity * nDotL,
    );
}
