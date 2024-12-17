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

struct CameraUniform {
    view_proj: mat4x4<f32>
}
struct TransformUniform {
    model: mat4x4<f32>,
    rotation: mat3x3<f32>,
}
struct LightUniform {
    direction: vec3<f32>,
    color: vec4<f32>,
    view_proj: mat4x4<f32>,
    intensity: f32,
}
@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(0) @binding(1)
var<uniform> light: LightUniform;

@group(0) @binding(2)
var tex_shadow_map: texture_2d<f32>;

@group(0) @binding(3)
var samp_shadow_map: sampler;

@group(1) @binding(0)
var tex_0: texture_2d<f32>;

@group(1) @binding(1)
var samp_0: sampler;

@group(2) @binding(0)
var<uniform> transform: TransformUniform;

@vertex
fn vs_main(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.color = model.color;
    out.world_pos = (transform.model * vec4<f32>(model.position, 1.0)).xyz;
    out.normal = transform.rotation * model.normal;
    out.tex_coord = model.tex_coord;
    out.light_space_clip_pos = light.view_proj * vec4<f32>(out.world_pos, 1.0);
    out.clip_position = camera.view_proj * vec4<f32>(out.world_pos, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    let lightFactor = (dot(light.direction, in.normal) + 1.0) / 2.0 * light.color * light.intensity;
    let baseColor = textureSample(tex_0, samp_0, in.tex_coord).xyz;
    // let shadow = calculate_shadow(in.light_space_clip_pos);

    var light_space_pos = in.light_space_clip_pos;
    var proj_coords = light_space_pos.xyz / light_space_pos.w;
    var uv = proj_coords.xy * 0.5 + 0.5; // map [-1, 1] to [0, 1]
    var closest_depth = textureSample(tex_shadow_map, samp_shadow_map, uv).x;
    var current_depth = proj_coords.z;

    var shadow = select(1., 0.5, current_depth > closest_depth);

    return vec4<f32>(baseColor * shadow, 1.0);
}

// Range [0.0, 1.0]: 0.0 in shadow, 1.0 not in shadow
fn calculate_shadow(light_space_pos: vec4<f32>) -> f32 {
    var proj_coords = light_space_pos.xyz / light_space_pos.w;
    proj_coords = proj_coords * 0.5 + 0.5;
    var closest_depth = textureSample(tex_shadow_map, samp_shadow_map, proj_coords.xy).x;
    var current_depth = proj_coords.z;

    var shadow = select(1., 0., current_depth > closest_depth);
    return shadow;
}
