
struct Vertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coord: vec2<f32>,
}

struct V2F {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local_position: vec3<f32>,
}

@group(0) @binding(0) var<uniform> view_proj: mat4x4<f32>;

@vertex
fn vs_main(in: Vertex) -> V2F{
    var ret: V2F;
    ret.local_position = in.position;
    ret.clip_position = view_proj * vec4<f32>(in.position, 1.0);
    return ret;
}

@group(1) @binding(0) var samp: sampler;
@group(1) @binding(1) var tex: texture_2d<f32>;

const inv_atan: vec2<f32> = vec2<f32>(0.1591, 0.3183);

fn sample_spherical_map(dir: vec3<f32>) -> vec2<f32> {
    var uv = vec2<f32>(atan(dir.z / dir.x), asin(dir.y));
    uv *= inv_atan;
    uv += 0.5;
    return uv;
}

@fragment
fn fs_main(in: V2F) -> @location(0) vec4<f32>{
    let uv = sample_spherical_map(normalize(in.local_position));
    let color = textureSample(tex, samp, uv);
    return color;
}
