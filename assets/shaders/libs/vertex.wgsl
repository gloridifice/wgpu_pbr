#define_import_path vertex

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec3<f32>,
    @location(3) color: vec4<f32>,
    @location(4) tex_coord: vec2<f32>,
};

struct CubeVertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coord: vec2<f32>,
}

struct FullscreenV2F {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>
}
