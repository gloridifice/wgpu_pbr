// Global -----
struct CameraUniform {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    position: vec3<f32>,
    direction: vec3<f32>,
    screen_resolution: vec2<f32>,
}

struct LightUniform {
    direction: vec3<f32>,
    color: vec4<f32>,
    view_proj: mat4x4<f32>,
    intensity: f32,
    lights_nums: vec4<u32>,
}

struct CsmBound {
    invert_light_space: mat4x4<f32>,
    near: f32,
    far: f32,
}

struct CsmUniform {
    texture_size: f32,
    bounds: array<CsmBound, 8>,
}

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(0) @binding(1) var<uniform> light: LightUniform;
@group(0) @binding(2) var csm: texture_depth_2d_array;
@group(0) @binding(3) var csm_sampler: sampler_comparison;

@group(0) @binding(4) var dfg_lut: texture_2d<f32>;
@group(0) @binding(5) var env_cubemap: texture_cube<f32>;
@group(0) @binding(6) var env_cubemap_sampler: sampler;
@group(0) @binding(7) var<uniform> env_sh_coefficients: array<vec3<f32>, 9>;

@group(0) @binding(8) var rendered_image: texture_2d<f32>;
@group(0) @binding(9) var rendered_sampler: sampler;

@group(0) @binding(10) var<uniform> csm_info: CsmUniform;
