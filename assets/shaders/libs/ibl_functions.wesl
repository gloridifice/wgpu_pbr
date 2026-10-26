import package::libs::primary::global_bindings::{
    env_cubemap, env_cubemap_sampler, dfg_lut, env_sh_coefficients,
};
import package::libs::primary::pbr_type::PBRSurface;


fn irradiance_sh(normal: vec3<f32>) -> vec3<f32>{
    // CPU stores radiance SH coefficients in an orthonormal basis.
    // Reconstruct with that basis and the clamped-cosine convolution per band.
    return env_sh_coefficients[0] * (0.282095 * 3.14159265)
    + (env_sh_coefficients[1] * normal.y
       + env_sh_coefficients[2] * normal.z
       + env_sh_coefficients[3] * normal.x) * (0.488603 * 2.09439510)
    + (env_sh_coefficients[4] * (normal.y * normal.x) * 1.092548
       + env_sh_coefficients[5] * (normal.y * normal.z) * 1.092548
       + env_sh_coefficients[6] * (3.0 * normal.z * normal.z - 1.0) * 0.315392
       + env_sh_coefficients[7] * (normal.z * normal.x) * 1.092548
       + env_sh_coefficients[8] * (normal.x * normal.x - normal.y * normal.y) * 0.546274) * 0.78539816;
}

fn prefiltered_dfg_lut(perceptual_roughness: f32, nDotV: f32) -> vec2<f32> {
    return textureSample(dfg_lut, env_cubemap_sampler, vec2(nDotV, perceptual_roughness)).xy;
}

fn evaluate_ibl_spectular(reflect: vec3<f32>, perceptual_roughness: f32) -> vec3<f32>{
    let level = f32(textureNumLevels(env_cubemap) - 1u) * clamp(perceptual_roughness, 0.0, 1.0);
    return max(textureSampleLevel(env_cubemap, env_cubemap_sampler, reflect, level).xyz, vec3f(0.0));
}


struct IBLEvaluationResult {
    diffuse: vec3<f32>,
    specular: vec3<f32>,
}

/// IBL 仍然由 Specular + Diffuse 构成
/// ## Specular = Specular Color * Indirect Specular
/// - Specular Color: 采样 DFG lookup-table 后计算快速获得
/// - Indirect Specular: 采样预滤波好的环境贴图获得
///
/// ## Diffuse = Diffuse Color * Indirect Diffuse
/// - Diffuse Color: abldo
/// - Indirect Diffuse: 通过 Spherical Harmonics 获得，只取决于法线
fn evaluate_ibl_separately(normal: vec3<f32>, world2camera: vec3<f32>, diffuse_color: vec3<f32>, f0: vec3<f32>, perceptual_roughness: f32)
    -> IBLEvaluationResult {
    let nDotV = clamp(dot(normal, world2camera), 0.0, 1.0);
    let reflect = reflect(-world2camera, normal);

    let indirect_specular: vec3<f32> = evaluate_ibl_spectular(reflect, perceptual_roughness);
    let dfg: vec2<f32> = prefiltered_dfg_lut(perceptual_roughness, nDotV);
    // LUT: R=A, G=B for Schlick F90=1, with roughness increasing downwards.
    let specular_color: vec3<f32> = f0 * dfg.x + vec3f(dfg.y);

    let indirect_diffuse: vec3<f32> = max(irradiance_sh(normal), vec3<f32>(0.0)) / 3.1415926;

    // diffuse_color already excludes metals. Reserve the integrated specular
    // energy before adding diffuse (a split-sum layering approximation).
    return IBLEvaluationResult(diffuse_color * (vec3f(1.0) - specular_color) * indirect_diffuse, specular_color * indirect_specular);
}

fn evaluate_ibl(normal: vec3<f32>, world2camera: vec3<f32>, diffuse_color: vec3<f32>, f0: vec3<f32>, perceptual_roughness: f32)
    -> vec3<f32>
{
    let result = evaluate_ibl_separately(normal, world2camera, diffuse_color, f0, perceptual_roughness);
    return result.diffuse + result.specular;
}
