//! Rebuild the linear split-sum LUT: R=A, G=B; x=N.V, y=perceptual roughness.
//! GGX NDF importance sampling, height-correlated Smith visibility, Schlick F90=1.
use std::f64::consts::TAU;

fn integrate(nv: f64, perceptual_roughness: f64, samples: u32) -> [f64; 2] {
    let alpha = perceptual_roughness.clamp(0.089, 1.0).powi(2);
    let a2 = alpha * alpha;
    let vx = (1.0 - nv * nv).sqrt();
    let mut ab = [0.0; 2];
    for i in 0..samples {
        let u = i as f64 / samples as f64;
        let v = i.reverse_bits() as f64 / 4294967296.0;
        let nh = ((1.0 - v) / (1.0 + (a2 - 1.0) * v)).sqrt();
        let hx = (TAU * u).cos() * (1.0 - nh * nh).max(0.0).sqrt();
        let vh = (vx * hx + nv * nh).max(0.0);
        let nl = 2.0 * vh * nh - nv;
        if nl > 0.0 {
            let visibility = 0.5
                / (nl * (nv * nv * (1.0 - a2) + a2).sqrt()
                    + nv * (nl * nl * (1.0 - a2) + a2).sqrt());
            // BRDF*cos / pdf, pdf = D*NH/(4*VH).
            let weight = 4.0 * visibility * nl * vh / nh;
            let fc = (1.0 - vh).powi(5);
            ab[0] += (1.0 - fc) * weight;
            ab[1] += fc * weight;
        }
    }
    ab.map(|v| v / samples as f64)
}

fn main() -> anyhow::Result<()> {
    let size = 128;
    let image = image::RgbaImage::from_fn(size, size, |x, y| {
        let [a, b] = integrate(
            (x as f64 + 0.5) / size as f64,
            (y as f64 + 0.5) / size as f64,
            4096,
        );
        image::Rgba([(a * 255.0).round() as u8, (b * 255.0).round() as u8, 0, 255])
    });
    image.save("assets/textures/ibl_brdf_lut.png")?;
    println!("Wrote linear GGX A/B LUT (128x128, 4096 samples/texel)");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ggx_furnace_matches_independent_limits() {
        // At normal incidence and alpha=1 the analytic F=1 albedo is 1-ln(2).
        let ab = integrate(1.0, 1.0, 65536);
        assert!((ab[0] + ab[1] - (1.0 - 2.0_f64.ln())).abs() < 0.0001);
        // A nearly smooth perfect reflector returns essentially all incident energy.
        let ab = integrate(1.0, 0.0, 65536);
        assert!((ab[0] + ab[1] - 1.0).abs() < 0.001);
        for nv in [0.05, 0.25, 0.5, 1.0] {
            for r in [0.0, 0.25, 0.5, 1.0] {
                let ab = integrate(nv, r, 65536);
                assert!(ab.iter().all(|v| v.is_finite() && *v >= 0.0));
                assert!(ab[0] + ab[1] <= 1.002, "nv={nv}, r={r}, ab={ab:?}");
            }
        }
    }
}
