use std::f32::consts::PI;

use image::{DynamicImage, GenericImageView, Pixel};

fn texel_to_dir(x: u32, y: u32, width: u32, height: u32) -> [f32; 3] {
    let u = (x as f32 + 0.5) / width as f32;
    let v = (y as f32 + 0.5) / height as f32;
    let theta = v * PI; // 纬度
    let phi = u * 2.0 * PI; // 经度
    let sin_theta = theta.sin();
    [
        sin_theta * phi.cos(), // x
        theta.cos(),           // y
        sin_theta * phi.sin(), // z
    ]
}

/// SH 二阶的 9 个基函数（Y_0 到 Y_8）在方向 dir 上的值
fn sh_basis_2nd(dir: [f32; 3]) -> [f32; 9] {
    let (x, y, z) = (dir[0], dir[1], dir[2]);
    [
        0.282095,                       // Y₀₀
        0.488603 * y,                   // Y₁₋₁
        0.488603 * z,                   // Y₁₀
        0.488603 * x,                   // Y₁₁
        1.092548 * x * y,               // Y₂₋₂
        1.092548 * y * z,               // Y₂₋₁
        0.315392 * (3.0 * z * z - 1.0), // Y₂₀
        1.092548 * x * z,               // Y₂₁
        0.546274 * (x * x - y * y),     // Y₂₂
    ]
}

pub fn compute_sh_coefficients(image: &DynamicImage) -> [[f32; 4]; 9] {
    let (width, height) = image.dimensions();
    let mut coeffs = [[0.0f32; 4]; 9]; // 9 个基函数，每个 RGBA

    for y in 0..height {
        for x in 0..width {
            let dir = texel_to_dir(x, y, width, height);
            let basis = sh_basis_2nd(dir);

            let pixel = image.get_pixel(x, y).to_rgb();
            let r = pixel[0] as f32 / 255.0;
            let g = pixel[1] as f32 / 255.0;
            let b = pixel[2] as f32 / 255.0;

            // 球面采样的权重（面积近似）
            let weight = (PI / height as f32) * (2.0 * PI / width as f32) * dir[1].max(0.0); // y=cos(theta)

            for i in 0..9 {
                coeffs[i][0] += r * basis[i] * weight;
                coeffs[i][1] += g * basis[i] * weight;
                coeffs[i][2] += b * basis[i] * weight;
            }
        }
    }

    coeffs
}
