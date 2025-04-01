#import vertex::{ CubeVertexInput, CubemapVertexOutput }

@group(1) @binding(0) var samp: sampler;
@group(1) @binding(1) var tex: texture_2d<f32>;

const PI: f32 = 3.1415926;

fn sample_spherical_map(dir: vec3<f32>) -> vec2<f32> {
    var u = atan2(dir.x, dir.z);
    var v = asin(-dir.y);
    var uv = vec2f(u, v);

    uv.x = (uv.x + PI) / (2.0 * PI);
    uv.y = (uv.y + PI / 2.0) * PI;
    return uv;
}

@fragment
fn fs_main(in: CubemapVertexOutput) -> @location(0) vec4<f32>{
    let uv = sample_spherical_map(normalize(in.local_position));
    let color = textureSample(tex, samp, uv);
    return vec4<f32>(color.xyz, 1.0);
    // return vec4f(1.0);
}
