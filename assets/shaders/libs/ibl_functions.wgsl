#define_import_path ibl_functions

#import global_bindings::{
    env_cubemap, env_cubemap_sampler, dfg_lut
}
#import pbr_type::PBRSurface


fn irradiance_sh(normal: f32) -> vec3<f32>{
    //todo
}

fn prefiltered_dfg_lut(perceptual_roughness: f32, nDotV: f32) -> vec2<f32> {
    return textureSample(dfg_lut, env_cubemap_sampler, vec2(nDotV, perceptual_roughness)).xy;
}

fn evaluate_ibl_spectular(reflect: vec3<f32>, perceptual_roughness: f32) -> vec3<f32>{
    let level = 5.0 * perceptual_roughness;
    return textureSampleLevel(env_cubemap, env_cubemap_sampler, reflect, level).xyz;
}

/// IBL 仍然由 Specular + Diffuse 构成
/// ## Specular = Specular Color * Indirect Specular
/// - Specular Color: 采样 DFG lookup-table 后计算快速获得
/// - Indirect Specular: 采样预滤波好的环境贴图获得
///
/// ## Diffuse = Diffuse Color * Indirect Diffuse
/// - Diffuse Color: abldo
/// - Indirect Diffuse: 通过 Spherical Harmonics 获得，只取决于法线
fn evaluate_ibl(normal: vec3<f32>, world2camera: vec3<f32>, diffuse_color: vec3<f32>, f0: vec3<f32>, f90: vec3<f32>, perceptual_roughness: f32)
    -> vec3<f32>
{
    let nDotV = max(dot(normal, world2camera), 0.0); // Check neg pos
    let reflect = reflect(-world2camera, normal);

    let indirect_specular: vec3<f32> = evaluate_ibl_spectular(reflect, perceptual_roughness);
    let dfg: vec2<f32> = prefiltered_dfg_lut(perceptual_roughness, nDotV);
    let specular_color: vec3<f32> = f0 * dfg.x + f90 * dfg.y;

    let indirect_diffuse: vec3<f32> = vec3<f32>(0.0); //todo

    return diffuse_color * indirect_diffuse + specular_color * indirect_specular;
}
