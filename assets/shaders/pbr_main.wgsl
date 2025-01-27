struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>
}

struct CameraUniform {
    view_proj: mat4x4<f32>,
    position: vec3<f32>,
    direction: vec3<f32>,
}

struct LightUniform {
    direction: vec3<f32>,
    color: vec4<f32>,
    view_proj: mat4x4<f32>,
    intensity: f32,
    lights_nums: vec4<u32>,
}

struct PointLight {
    color: vec4<f32>,
    position: vec4<f32>,
    intensity: f32,
    distance: f32,
    decay: f32,
}

struct PBRSurfaceContext {
    base_color: vec3<f32>,
    normal: vec3<f32>,
    metallic: f32,
    roughness: f32,
    reflectance: f32,
}

@group(0) @binding(0) var g_samp: sampler;
@group(0) @binding(1) var world_pos_tex: texture_2d<f32>;
@group(0) @binding(2) var normal_tex: texture_2d<f32>;
// @group(0) @binding(3) var tex_coord_tex: texture_2d<f32>;
@group(0) @binding(3) var base_color_tex: texture_2d<f32>;
@group(0) @binding(4) var pbr_parameters_tex: texture_2d<f32>;

@group(1) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(1) var<uniform> light: LightUniform;

@group(2) @binding(0) var<storage, read> point_lights: array<PointLight>;

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
    surface: PBRSurfaceContext,
    world2light: vec3<f32>,
    world2camera: vec3<f32>,
) -> vec3<f32> {
    let reflectance = surface.reflectance;
    let roughness = clamp(surface.roughness, 0.089, 1.0);
    let metallic = surface.metallic;
    let normal = surface.normal;
    let base_color = surface.base_color;

    let nDotL = max(dot(normal, world2light), 0.0);
    let half = normalize(world2light + world2camera);
    let nDotH = max(dot(normal, half), 0.0);
    let nDotV = max(dot(normal, world2camera), 0.0);
    let hDotV = max(dot(half, world2camera), 0.0);

    let diffuse_color = (1.0 - metallic) * base_color;

    // Schlick Fresnel Function
    let f0: vec3<f32> = vec3<f32>(0.16 * pow2(reflectance) * (1.0 - metallic)) + base_color * metallic;
    let fresnel: vec3<f32> = f0 + (vec3<f32>(1.0) + f0) * pow5(1.0 - hDotV);

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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let world_pos: vec3<f32> = textureSample(world_pos_tex, g_samp, in.uv).xyz;
    let normal: vec3<f32> = textureSample(normal_tex, g_samp, in.uv).xyz;
    // let tex_coord: vec2<f32> = textureSample(tex_coord_tex, g_samp, in.uv).xy;
    let base_color: vec4<f32> = textureSample(base_color_tex, g_samp, in.uv);
    let pbr_parameters = textureSample(pbr_parameters_tex, g_samp, in.uv);
    let metallic: f32 = pbr_parameters.x;
    let roughness: f32 = pbr_parameters.y;
    let reflectance: f32 = pbr_parameters.z;
    let ambient_occlusion: f32 = pbr_parameters.w;

    if base_color.a == 0.0 {
        discard;
    }

    let surface = PBRSurfaceContext(base_color.xyz, normal, metallic, roughness, reflectance);

    var surface_color = vec3<f32>(0.0);

    // Parallel Light
    surface_color += calculate_light(
        light.color.xyz,
        light.intensity,
        surface,
        -light.direction,
        -camera.direction
    );

    // Point Lights
    let point_lights_num = light.lights_nums.x;

    for (var i = 0u; i < point_lights_num; i += 1u) {
        let li = point_lights[i];
        let world2light_unnorm = li.position.xyz - world_pos;
        let world2camera_unnorm = camera.position - world_pos;
        let dist = length(world2light_unnorm);
        if dist > li.distance { continue; }
        let dir = normalize(world2light_unnorm);

        let radiance = li.intensity / ((li.decay * pow2(dist)) + 0.001); // + 0.001 for division safety
        surface_color += calculate_light(
            li.color.xyz,
            radiance,
            surface,
            dir,
            normalize(world2camera_unnorm),
        );
    }
    surface_color += vec3<f32>(0.1);

    return vec4<f32>(surface_color, base_color.a);
}

