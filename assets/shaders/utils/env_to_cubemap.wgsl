#import vertex::{ CubeVertexInput, CubemapVertexOutput }

@group(1) @binding(0) var samp: sampler;
@group(1) @binding(1) var tex: texture_2d<f32>;

const PI: f32 = 3.1415926;

fn sample_spherical_map(dir: vec3<f32>) -> vec2<f32> {
    let phi = atan2(dir.z, dir.x);
    let theta = acos(clamp(dir.y, -1.0, 1.0));
    let uv = vec2<f32>(phi / (2.0 * PI) + 0.5, theta / PI);
    return uv;
}

@fragment
fn fs_main(in: CubemapVertexOutput) -> @location(0) vec4<f32>{
    let uv = sample_spherical_map(normalize(in.local_position));
    let color = textureSample(tex, samp, uv);
    return vec4<f32>(color.xyz, 1.0);
    // return vec4f(1.0);
}
