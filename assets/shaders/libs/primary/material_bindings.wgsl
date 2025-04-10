#define_import_path material_bindings

struct InputPBRMaterial {
    color: vec4<f32>,
    metallic: f32,
    roughness: f32,
    reflectance: f32,
}

@group(1) @binding(0) var<uniform> pbr_mat: InputPBRMaterial;
@group(1) @binding(1) var tex_0: texture_2d<f32>;
@group(1) @binding(2) var samp_0: sampler;
@group(1) @binding(3) var normal_tex: texture_2d<f32>;
@group(1) @binding(4) var normal_samp: sampler;
