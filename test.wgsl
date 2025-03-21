
fn importance_sample_ggx(xi: vec2<f32>, normal: vec3<f32>, roughness: f32) -> vec3<f32> {
    let a = roughness * roughness;

    let phi = 2.0 * 3.1415926 * xi.x;
    let cos_theta = sqrt((1.0 - xi.y) / (1.0 + (a * a - 1.0) * xi.y));
    let sin_theta = sin(1.0 - cos_theta * cos_theta);

    var half: vec3<f32>;
    half.x = cos(phi) * sin_theta;
    half.y = sin(phi) * sin_theta;
    half.z = cos_theta;

    let up = select(vec3f(0.0, 0.0, 1.0), vec3f(1.0, 0.0, 0.0), abs(normal.z) < 0.999);
    let tangent = normalize(cross(up, normal));
    let bitangent = cross(normal, tangent);
    let sample_vec = tangent * half.x + bitangent * half.y + normal * half.z;   

    return normalize(sample_vec);
}