use image::DynamicImage;
use std::f32::consts::PI;

fn basis([x, y, z]: [f32; 3]) -> [f32; 9] {
    [
        0.282095,
        0.488603 * y,
        0.488603 * z,
        0.488603 * x,
        1.092548 * x * y,
        1.092548 * y * z,
        0.315392 * (3. * z * z - 1.),
        1.092548 * x * z,
        0.546274 * (x * x - y * y),
    ]
}
fn add(coeffs: &mut [[f32; 4]; 9], dir: [f32; 3], rgb: [f32; 3], weight: f32) {
    for (coefficient, b) in coeffs.iter_mut().zip(basis(dir)) {
        for c in 0..3 {
            coefficient[c] += rgb[c] * b * weight;
        }
    }
}
pub fn compute_sh_coefficients(image: &DynamicImage) -> [[f32; 4]; 9] {
    let image = image.to_rgb32f();
    let (width, height) = image.dimensions();
    let mut coeffs = [[0.; 4]; 9];
    for (x, y, pixel) in image.enumerate_pixels() {
        let theta = (y as f32 + 0.5) / height as f32 * PI;
        let phi = (x as f32 + 0.5) / width as f32 * 2. * PI;
        add(
            &mut coeffs,
            [
                theta.sin() * phi.cos(),
                theta.cos(),
                theta.sin() * phi.sin(),
            ],
            pixel.0,
            theta.sin() * (PI / height as f32) * (2. * PI / width as f32),
        );
    }
    coeffs
}
pub fn compute_cubemap_sh(faces: &[Vec<u8>]) -> anyhow::Result<[[f32; 4]; 9]> {
    let mut coeffs = [[0.; 4]; 9];
    for (face, bytes) in faces.iter().enumerate() {
        let image = image::load_from_memory(bytes)?
            .thumbnail(128, 128)
            .to_rgb32f();
        let (w, h) = image.dimensions();
        for (x, y, pixel) in image.enumerate_pixels() {
            let u = 2. * (x as f32 + 0.5) / w as f32 - 1.;
            let v = 2. * (y as f32 + 0.5) / h as f32 - 1.;
            let dir = match face {
                0 => [1., -v, -u],
                1 => [-1., -v, u],
                2 => [u, 1., v],
                3 => [u, -1., -v],
                4 => [u, -v, 1.],
                _ => [-u, -v, -1.],
            };
            let length = (1. + u * u + v * v).sqrt();
            let rgb = pixel.0.map(|s| {
                if s <= 0.04045 {
                    s / 12.92
                } else {
                    ((s + 0.055) / 1.055).powf(2.4)
                }
            });
            add(
                &mut coeffs,
                dir.map(|v| v / length),
                rgb,
                4. / (w * h) as f32 / length.powi(3),
            );
        }
    }
    Ok(coeffs)
}
