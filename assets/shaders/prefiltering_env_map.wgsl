#import vertex::CubemapVertexOutput

struct Uniform {
    roughness: f32,
    sample_count: u32,
}

@group(1) @binding(0) var<uniform> infos: Uniform;
@group(1) @binding(1) var cubemap: texture_cube<f32>;
@group(1) @binding(2) var cubemap_sampler: sampler;

/// A method from:
/// Eric Heitz - Sampling the GGX Distribution of Visible Normals
/// alpha_x: x 向的原始粗糙度
/// alpha_y: y 向的原始粗糙度
/// random_uniform: 一个随机值，x 决定了平行与法线的平面的旋转角度，y 决定了垂直于法线方向的旋转角度
fn sample_ggx_vndf(view_e: vec3<f32>, alpha_x: f32, alpha_y: f32, random_uniform: vec2<f32>) -> vec3<f32> {
    let view_h = normalize(vec3f(alpha_x * view_e.x, alpha_y * view_e.y, view_e.z));
    let lensq = view_h.x * view_h.x + view_h.y * view_h.y;
    let t_1: vec3<f32> = select(vec3f(-view_h.y, view_h.x, 0) * inverseSqrt(lensq), vec3f(1.0, 0.0, 0.0), lensq > 0);
    let t_2 = cross(view_h, t_1);

    let r = sqrt(random_uniform.x);
    let phi = 2.0 * 3.1415926 * random_uniform.y;
    let t1 = r * cos(phi);
    var t2 = r * sin(phi);
    let s = 0.5 * (1.0 + view_h.z);
    t2 = (1.0 - s) * sqrt(1.0 - t1 * t1) + s * t2;

    let normal_h = t1 * t_1 + t2 * t_2 + sqrt(max(0.0, 1.0 - t1 * t1 - t2 * t2)) * view_h;

    let normal_e = normalize(vec3f(alpha_x * normal_h.x, alpha_y * normal_h.y, max(0.0, normal_h.z)));
    return normal_e;
}


@fragment
fn fs_main(v2f: CubemapVertexOutput) -> @location(0) vec4<f32> {
    let perceptual_roughness = infos.roughness;
    let normal = normalize(v2f.local_position);
    let sample_count = infos.sample_count;
    var total_weight: f32 = 0.0;
    var color = vec3f(0.0);
    for (var i = 0; i < sample_count; i++)
    {
        let random_uniform = vec2f(f32(i) / f32(sample_count), fract(sin(f32(i) * 127.1) * 43758.5453));
        let to_sample_dir = sample_ggx_vndf(normal, perceptual_roughness, perceptual_roughness, random_uniform);

        let nDotL = max(dot(normal, to_sample_dir), 0.0);
        if (nDotL > 0.0)
        {
            /// TODO
            color += textureSample(cubemap, cubemap_sampler, to_sample_dir).rgb * nDotL;
            total_weight += nDotL;
        }
    }
    return vec4f(color / total_weight, 1.0);
}
