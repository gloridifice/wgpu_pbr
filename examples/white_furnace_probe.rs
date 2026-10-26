//! Explicit HDR regression through the real scene loader and deferred renderer.
//! cargo run --example white_furnace_probe
use bevy_app::{App, AppExit, Last};
use bevy_ecs::prelude::*;
use lentille_render::{
    FrameSets, ToneMappingStage,
    camera::{CameraState, ColorImage},
    prelude::*,
    stage::RenderContext,
};
use lentille_wgpu_utils::texture_readback::read_texture_to_cpu;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use wgpu_pbr::{WgpuPbrPlugin, scene::SceneController};

#[derive(Resource)]
struct Probe {
    image: Option<Arc<ColorImage>>,
    started: Instant,
    ready_frames: u32,
}

fn capture(ctx: InMut<RenderContext>, mut probe: ResMut<Probe>) {
    probe.image = Some(ctx.hdr.current().clone());
}

fn check(world: &mut World) {
    let mut probe = world.remove_resource::<Probe>().unwrap();
    assert!(
        probe.started.elapsed() < Duration::from_secs(120),
        "White furnace load timed out"
    );
    let controller = world.resource::<SceneController>();
    assert!(controller.error.is_none(), "{:?}", controller.error);
    if controller.loading.is_some() || controller.path.is_none() || probe.image.is_none() {
        world.insert_resource(probe);
        return;
    }
    assert_eq!(
        controller.path.as_deref(),
        Some(std::path::Path::new("scenes/white_furnace.scn.ron"))
    );
    probe.ready_frames += 1;
    if probe.ready_frames < 3 {
        world.insert_resource(probe);
        return;
    }
    let image = probe.image.unwrap();
    let rs = world.resource::<RenderState>();
    let bytes = read_texture_to_cpu(&rs.device, &rs.queue, image.texture.texture());
    let rgb: Vec<[f32; 3]> = bytes
        .data
        .chunks_exact(8)
        .map(|p| {
            std::array::from_fn(|c| half::f16::from_le_bytes([p[c * 2], p[c * 2 + 1]]).to_f32())
        })
        .collect();
    // Contract: passive white materials under unit illumination cannot generate energy.
    let max = rgb.iter().flatten().copied().fold(0.0f32, f32::max);
    assert!(rgb.iter().flatten().all(|v| v.is_finite() && *v >= 0.0));
    assert!(max <= 1.02, "White furnace gained energy: max={max}");
    let size = image.texture.size();
    let matrix = world
        .query::<&CameraState>()
        .iter(world)
        .next()
        .unwrap()
        .view_proj;
    let sample = |point: Vec3| {
        let clip = matrix * point.extend(1.0);
        let x = ((clip.x / clip.w * 0.5 + 0.5) * size.width as f32) as usize;
        let y = ((0.5 - clip.y / clip.w * 0.5) * size.height as f32) as usize;
        assert!(
            x < size.width as usize && y < size.height as usize,
            "Sphere outside viewport"
        );
        rgb[y * size.width as usize + x][0]
    };
    for row in 1..=2 {
        for col in 0..6 {
            let value = sample(Vec3::new(
                (col as f32 - 2.5) * 2.6,
                (1.0 - row as f32) * 2.6,
                0.0,
            ));
            assert!(
                (value - 1.0).abs() < 0.02,
                "Dielectric/control row={row} col={col}: {value}"
            );
        }
    }
    let smooth = sample(Vec3::new(-6.5, 2.6, 0.0));
    let rough = sample(Vec3::new(6.5, 2.6, 0.0));
    assert!((smooth - 1.0).abs() < 0.03, "Smooth conductor: {smooth}");
    assert!(
        (0.25..0.5).contains(&rough),
        "Single-scatter rough conductor: {rough}"
    );
    println!(
        "HDR furnace passed: max={max:.5}, smooth metal={smooth:.5}, rough metal={rough:.5}; dielectric/control centers within 2% of 1"
    );
    world.write_message(AppExit::Success);
}

fn main() {
    let mut app = App::new();
    app.add_plugins(WgpuPbrPlugin)
        .insert_resource(Probe {
            image: None,
            started: Instant::now(),
            ready_frames: 0,
        })
        .add_render_system(capture, ToneMappingStage)
        .add_systems(Last, check.after(FrameSets::Present));
    app.world_mut().resource_mut::<SceneController>().request = Some(
        wgpu_pbr::scene::SceneRequest::Open("scenes/white_furnace.scn.ron".into()),
    );
    assert!(matches!(app.run(), AppExit::Success));
}
