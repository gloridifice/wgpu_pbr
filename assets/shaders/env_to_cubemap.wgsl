#import vertex::{ CubeVertexInput, CubemapVertexOutput }

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
fn fs_main(in: CubemapVertexOutput) -> @location(0) vec4<f32>{
    let uv = sample_spherical_map(normalize(in.local_position));
    let color = textureSample(tex, samp, uv);
    return color;
}
