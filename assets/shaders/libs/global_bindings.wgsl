#define_import_path global_bindings

// Global -----
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

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(0) @binding(1) var<uniform> light: LightUniform;
@group(0) @binding(2) var directional_shadow_map: texture_depth_2d;
@group(0) @binding(3) var directional_shadow_map_comparison_sampler: sampler_comparison;

@group(0) @binding(4) var dfg_lut: texture_2d<f32>;
@group(0) @binding(5) var env_cubemap: texture_cube<f32>;
@group(0) @binding(6) var env_cubemap_sampler: sampler;
