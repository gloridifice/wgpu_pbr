import package::libs::primary::pbr_type;

const PI: f32 = radians(180.0);

fn pow2(a: f32) -> f32 {
    return a * a;
}

fn pow5(a: f32) -> f32 {
    let a2 = a * a;
    return a2 * a2 * a;
}

fn V_smith_ggx_correlated(nDotV: f32, nDotL: f32, roughness: f32) -> f32 {
    let a2 = roughness * roughness;
    let GGXV = nDotL * sqrt(nDotV * nDotV * (1.0 - a2) + a2);
    let GGXL = nDotV * sqrt(nDotL * nDotL * (1.0 - a2) + a2);
    return 0.5 / max(GGXL + GGXV, 1e-7);
}

fn diffuse_brdf(metallic: f32, base_color: vec3<f32>) -> vec3<f32> {
    let diffuse_color = (1.0 - metallic) * base_color;
    // - Lambertan diffuse brdf
    return diffuse_color / PI;
}

fn specular_brdf(f0: vec3<f32>, f90: vec3<f32>, roughness: f32, nDotH: f32, nDotL: f32, nDotV: f32, hDotV: f32) -> vec3<f32>{
    // Schlick Fresnel; the caller uses the physical grazing endpoint F90=1.
    let fresnel: vec3<f32> = f0 + (f90 - f0) * pow5(1.0 - hDotV);

    // ! Specular BRDF ------------
    // - GGX Normal Distribution Function
    let roughness2 = pow2(roughness);
    let D_GGX = roughness2 / (PI * pow2(pow2(nDotH) * (roughness2 - 1.0) + 1.0));

    // - Geometry Function
    // V = G / (4.0 * nDotL * nDotV);
    let V_SmithGGX = V_smith_ggx_correlated(nDotV, nDotL, roughness);

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
    // Derive alpha from the material in both forward and deferred paths.
    let roughness = pbr_type::perceptual_roughness_to_roughness(surface.material.perceptual_roughness);
    let metallic: f32 = surface.material.metallic;
    let normal: vec3<f32> = surface.normal;
    let base_color: vec3<f32> = surface.material.base_color.xyz;

    let nDotL = max(dot(normal, world2light), 0.0);
    if nDotL <= 0.0 || dot(normal, world2camera) <= 0.0 {
        return LightCalculationResult(vec3f(0.0), vec3f(0.0));
    }
    let half = normalize(world2light + world2camera);
    let nDotH = max(dot(normal, half), 0.0);
    let nDotV = max(dot(normal, world2camera), 0.0);
    let hDotV = max(dot(half, world2camera), 0.0);
    let f90 = vec3f(1.0);
    let fresnel = f0 + (f90 - f0) * pow5(1.0 - hDotV);

    let diffuse_brdf = diffuse_brdf(metallic, base_color) * (vec3f(1.0) - fresnel);
    let specular_brdf = specular_brdf(f0, f90, roughness, nDotH, nDotL, nDotV, hDotV);

    let light_intensity = light_color * light_diffuse_intensity;

    return LightCalculationResult(
        diffuse_brdf * light_intensity * nDotL,
        specular_brdf * light_intensity * nDotL,
    );
}
