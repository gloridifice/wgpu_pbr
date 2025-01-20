struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) tex_coord: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coord: vec2<f32>,
    @location(3) world_pos: vec3<f32>,
    @location(4) light_space_clip_pos: vec4<f32>,
};

struct FragmentOutput{
    @location(0) world_pos: vec4<f32>,
    @location(1) normal: vec4<f32>,
    // @location(2) tex_coord: vec2<f32>,
    // For PBR
    @location(2) base_color: vec4<f32>,
    @location(3) pbr_parameters: vec4<f32>, // 0: Metallic, 1: Roughness, 2: Reflectance, 3: Ambient occlusion
    @location(4) emissive: vec4<f32>,
}

struct CameraUniform {
    view_proj: mat4x4<f32>,
    position: vec3<f32>,
    direction: vec3<f32>,
}

struct TransformUniform {
    model: mat4x4<f32>,
    normal: mat3x3<f32>,
}

struct LightUniform {
    direction: vec3<f32>,
    color: vec4<f32>,
    view_proj: mat4x4<f32>,
    intensity: f32,
    lights_nums: vec4<u32>,
}

struct PBRMaterial {
    metallic: f32,
    roughness: f32,
    reflectance: f32,
}

// Global -----
@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(0) @binding(1)
var<uniform> light: LightUniform;

@group(0) @binding(2)
var tex_shadow_map: texture_depth_2d;

@group(0) @binding(3)
var samp_shadow_map: sampler_comparison;

// Material -----
@group(1) @binding(0)
var<uniform> pbr_mat: PBRMaterial;

@group(1) @binding(1)
var tex_0: texture_2d<f32>;

@group(1) @binding(2)
var samp_0: sampler;

// Object -----
@group(2) @binding(0)
var<uniform> transform: TransformUniform;

@vertex
fn vs_main(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.color = model.color;
    out.world_pos = (transform.model * vec4<f32>(model.position, 1.0)).xyz;
    out.normal = transform.normal * model.normal;
    out.tex_coord = model.tex_coord;
    out.light_space_clip_pos = light.view_proj * vec4<f32>(out.world_pos, 1.0);
    out.clip_position = camera.view_proj * vec4<f32>(out.world_pos, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    // let lightFactor = (dot(light.direction, in.normal) + 1.0) / 2.0 * light.color * light.intensity;
    let baseColor = textureSample(tex_0, samp_0, in.tex_coord);

    // var light_space_pos = in.light_space_clip_pos;
    // var proj_coords = light_space_pos.xyz / light_space_pos.w;

    // let flip_correction = vec2<f32>(0.5, -0.5);

    // var uv = proj_coords.xy * flip_correction + vec2<f32>(0.5); // reverse y and map [-1, 1] to [0, 1]
    // var shadow = textureSampleCompare(tex_shadow_map, samp_shadow_map, uv, proj_coords.z);
    // let shadow_color = vec3<f32>(0.5);
    // var shadow_factor = mix(shadow_color, vec3<f32>(1.0), shadow);

    var o: FragmentOutput;
    o.world_pos = vec4<f32>(in.world_pos, 1.0);
    o.base_color = baseColor;
    o.normal = vec4<f32>(in.normal, 1.0);
    // o.tex_coord = in.tex_coord;

    let metallic = pbr_mat.metallic;
    let roughness = pbr_mat.roughness;
    let reflectance = pbr_mat.reflectance;
    o.pbr_parameters = vec4<f32>(metallic, roughness, reflectance, 0.0);
    o.emissive = vec4<f32>(0.0);

    return o;
}

// Range [0.0, 1.0]: 0.0 in shadow, 1.0 not in shadow
fn calculate_shadow(light_space_pos: vec4<f32>) -> f32 {
    // var proj_coords = light_space_pos.xyz / light_space_pos.w;
    // proj_coords = proj_coords * 0.5 + 0.5;
    // var closest_depth = textureSample(tex_shadow_map, samp_shadow_map, proj_coords.xy).x;
    // var current_depth = proj_coords.z;

    // var shadow = select(1., 0., current_depth > closest_depth);
    return 1.0;
}
