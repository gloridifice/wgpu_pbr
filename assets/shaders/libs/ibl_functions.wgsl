#define_import_path ibl_functions

#import global_bindings::{
    env_cubemap, env_cubemap_sampler, dfg_lut, env_sh_coefficients,
}
#import pbr_type::PBRSurface


fn irradiance_sh(normal: vec3<f32>) -> vec3<f32>{
    return env_sh_coefficients[0]
    + env_sh_coefficients[1] * (normal.y)
    + env_sh_coefficients[2] * (normal.z)
    + env_sh_coefficients[3] * (normal.x)
    + env_sh_coefficients[4] * (normal.y * normal.x)
    + env_sh_coefficients[5] * (normal.y * normal.z)
    + env_sh_coefficients[6] * (3.0 * normal.z * normal.z - 1.0)
    + env_sh_coefficients[7] * (normal.z * normal.x)
    + env_sh_coefficients[8] * (normal.x * normal.x - normal.y * normal.y);
}

fn prefiltered_dfg_lut(perceptual_roughness: f32, nDotV: f32) -> vec2<f32> {
    return textureSample(dfg_lut, env_cubemap_sampler, vec2(nDotV, perceptual_roughness)).xy;
}

fn evaluate_ibl_spectular(reflect: vec3<f32>, perceptual_roughness: f32) -> vec3<f32>{
    let level = 5.0 * perceptual_roughness;
    return clamp(textureSampleLevel(env_cubemap, env_cubemap_sampler, reflect, level).xyz, vec3f(0.0), vec3f(2.0));
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
    let L = normal;
    let half = normalize(L + world2camera);
    let nDotV = max(dot(normal, world2camera), 0.0);
    let lDotH = max(dot(L, half), 0.0);
    let f90 = vec3f(0.5 + 0.2 * perceptual_roughness * perceptual_roughness * lDotH * lDotH);
    let reflect = reflect(-world2camera, normal);

    let indirect_specular: vec3<f32> = evaluate_ibl_spectular(reflect, perceptual_roughness);
    let dfg: vec2<f32> = prefiltered_dfg_lut(perceptual_roughness, nDotV);
    let specular_color: vec3<f32> = f0 * dfg.x + f90 * dfg.y;

    let indirect_diffuse: vec3<f32> = max(irradiance_sh(normal), vec3<f32>(0.0)) / 3.1415926;

    return IBLEvaluationResult(diffuse_color * indirect_diffuse, specular_color * indirect_specular);
}

fn evaluate_ibl(normal: vec3<f32>, world2camera: vec3<f32>, diffuse_color: vec3<f32>, f0: vec3<f32>, perceptual_roughness: f32)
    -> vec3<f32>
{
    let result = evaluate_ibl_separately(normal, world2camera, diffuse_color, f0, perceptual_roughness);
    return result.diffuse + result.specular;
    // return result.diffuse;
    // return result.specular;
}
