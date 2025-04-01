@group(0) @binding(0) var<uniform> env_sh_coefficients: array<vec3<f32>, 9>;
fn irradiance_sh(normal: vec3<f32>) -> vec3<f32>{
    return env_sh_coefficients[0]
    + env_sh_coefficients[1] * (normal.y)
    + env_sh_coefficients[2] * (normal.z)
    + env_sh_coefficients[3] * (normal.x)
    + env_sh_coefficients[4] * (normal.y * normal.x)
    + env_sh_coefficients[5] * (normal.y * normal.z)
    + env_sh_coefficients[6] * (3.0 * normal.z * normal.z - 1.0)
    + env_sh_coefficients[7] * (normal.z * normal.x)
    + env_sh_coefficients[8] * (normal.x * normal.x - normal.y * normal.y);
}
