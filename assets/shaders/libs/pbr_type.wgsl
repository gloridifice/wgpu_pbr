#define_import_path pbr_type

struct StandardMaterial {
    base_color: vec3<f32>,
    emissive: vec4<f32>,
    perceptual_roughness: f32,
    metallic: f32,
    reflectance: f32,
    clear_coat: f32,
    clear_coat_perceptual_roughness: f32,
}

fn standard_material_new() -> StandardMaterial{
    var material: StandardMaterial;

    material.base_color = vec3<f32>(1.0);
    material.emissive = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    material.perceptual_roughness = 0.5;
    material.metallic = 0.0;
    material.reflectance = 0.5;
    material.clear_coat = 0.0;
    material.clear_coat_perceptual_roughness = 0.5;

    return material;
}

struct PBRSurface {
    material: StandardMaterial,
    roughness: f32,
    clear_coat_roughness: f32,
    normal: vec3<f32>,
}

fn perceptual_roughness_to_roughness(perceptual_roughness: f32) -> f32 {
    let clamped = clamp(perceptual_roughness, 0.089, 1.0);
    return clamped * clamped;
}

// x: mapped_normal (3),
// y: metallic, reflectance, clear_coat_perceptual_roughness, clear_coat,
// z: base_color (3), perceptual_roughness,
// w: emissive (3),

fn pack_g_buffer(in: PBRSurface) -> vec4<u32> {
    return vec4<u32>(
        pack4x8unorm(vec4<f32>(in.normal * 0.5 + vec3<f32>(0.5), 1.0)),
        pack4x8unorm(vec4<f32>(
            in.material.metallic,
            in.material.reflectance,
            in.material.clear_coat_perceptual_roughness,
            in.material.clear_coat)),
        pack4x8unorm(vec4<f32>(in.material.base_color, in.material.perceptual_roughness)),
        pack4x8unorm(in.material.emissive),
    );
}

fn unpack_g_buffer(in: vec4<u32>) -> PBRSurface {
    var material = standard_material_new();
    var ret: PBRSurface;
    let raw_normal = unpack4x8unorm(in.x).xyz;
    let props = unpack4x8unorm(in.y);
    material.metallic = props.x;
    material.reflectance = props.y;
    material.clear_coat_perceptual_roughness = props.z;
    material.clear_coat = props.w;
    let color_rou = unpack4x8unorm(in.z);
    material.base_color = color_rou.xyz;
    material.perceptual_roughness = color_rou.w;

    ret.material = material;
    ret.normal = normalize((raw_normal - vec3<f32>(0.5)) * 2.0);
    ret.roughness = perceptual_roughness_to_roughness(material.perceptual_roughness);
    ret.clear_coat_roughness = perceptual_roughness_to_roughness(material.clear_coat_perceptual_roughness);

    return ret;
}