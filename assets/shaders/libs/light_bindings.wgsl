#define_import_path light_bindings

struct PointLight {
    color: vec4<f32>,
    position: vec4<f32>,
    intensity: f32,
    distance: f32,
    decay: f32,
}

@group(3) @binding(0) var<storage, read> point_lights: array<PointLight>;