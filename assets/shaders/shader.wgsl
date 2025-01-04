struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>
}

struct CameraUniform {
    view_proj: mat4x4<f32>
}
struct LightUniform {
    direction: vec3<f32>,
    color: vec4<f32>,
    view_proj: mat4x4<f32>,
    intensity: f32,
}

@group(0) @binding(0) var g_samp: sampler;
@group(0) @binding(1) var world_pos_tex: texture_2d<f32>;
@group(0) @binding(2) var normal_tex: texture_2d<f32>;
@group(0) @binding(3) var color_tex: texture_2d<f32>;
@group(0) @binding(4) var tex_coord_tex: texture_2d<f32>;
@group(1) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(1) var<uniform> light: LightUniform;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let world_pos = textureSample(world_pos_tex, g_samp, in.uv).xyz;
    let normal = textureSample(normal_tex, g_samp, in.uv).xyz;
    let color = textureSample(color_tex, g_samp, in.uv);

    let lightFactor = (dot(light.direction, normal) + 1.0) / 2.0 * light.color * light.intensity;

    return vec4<f32>(color.xyz * lightFactor.xyz, 1.0);
}
