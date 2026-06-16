struct InputPBRMaterial {
    color: vec4<f32>,
    metallic: f32,
    roughness: f32,
    reflectance: f32,
}

struct MetallicRoughnessAo {
    ao: f32,
    metallic: f32,
    roughness: f32,
}

@group(1) @binding(0) var<uniform> pbr_mat: InputPBRMaterial;
@group(1) @binding(1) var base_sampler: sampler;
@group(1) @binding(2) var albedo_texture: texture_2d<f32>;
@group(1) @binding(3) var normal_texture: texture_2d<f32>;
@group(1) @binding(4) var metallic_roughness_ao_texture: texture_2d<f32>;
@group(1) @binding(5) var emmisive_texture: texture_2d<f32>;

fn sample_albedo(uv: vec2f) -> vec4f {
    return textureSample(albedo_texture, base_sampler, uv) * pbr_mat.color;
}

fn sample_normal(uv: vec2f) -> vec3f {
    return textureSample(normal_texture, base_sampler, uv).xyz;
}

fn sample_metallic_roughness_ao(uv: vec2f) -> MetallicRoughnessAo {
    let sampled = textureSample(metallic_roughness_ao_texture, base_sampler, uv);
    var ret: MetallicRoughnessAo;
    ret.ao = sampled.r;
    ret.metallic = sampled.b * pbr_mat.metallic;
    ret.roughness = sampled.g * pbr_mat.roughness;
    return ret;
}

fn sample_emmisive(uv: vec2f) -> vec3f {
   return textureSample(emmision_texture, base_sampler, uv).xyz;
}
