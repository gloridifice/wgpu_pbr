use super::*;
use crate::{
    OpaqueStage, PostProcessStage, RenderPlugin, RenderReady, TransparentStage,
    bindings::camera_binding::CameraBindGroupLayout,
    camera::{RenderTarget, RenderTargetConfig, RenderTargetSize},
    shadow::csm::CsmConfig,
    stage::RenderStage,
};

#[derive(Component)]
struct TestRadiance(f64);

#[derive(Resource)]
struct ProbePipelines {
    background: RenderPipeline,
    overlay: RenderPipeline,
}

impl FromWorld for ProbePipelines {
    fn from_world(world: &mut World) -> Self {
        let rs = world.resource::<RenderState>();
        let vertex = &world.resource::<FullScreenVertexShader>().module;
        let camera_layout = &world.resource::<CameraBindGroupLayout>().0;
        let make = |source: &'static str, layouts: &[Option<&BindGroupLayout>], format| {
            let shader = rs
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("HDR contract probe"),
                    source: wgpu::ShaderSource::Wgsl(source.into()),
                });
            let layout = rs
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: None,
                    bind_group_layouts: layouts,
                    immediate_size: 0,
                });
            rs.device
                .create_render_pipeline(&lentille_wgpu_utils::full_screen_pipeline_desc(
                    None,
                    &layout,
                    vertex,
                    &shader,
                    &[Some(lentille_wgpu_utils::color_target_replace_write_all(
                        format,
                    ))],
                ))
        };
        Self {
            // Exercise the real camera background binding, rather than constructing
            // a test-owned source binding that could hide a stale Stage context.
            background: make(
                r#"
                @group(0) @binding(8) var background: texture_2d<f32>;
                @fragment fn fs_main(@builtin(position) p: vec4f) -> @location(0) vec4f {
                    return vec4f(textureLoad(background, vec2i(p.xy), 0).rgb * 0.5, 1.0);
                }
            "#,
                &[Some(camera_layout)],
                crate::SCENE_COLOR_FORMAT,
            ),
            overlay: make(
                r#"
                @fragment fn fs_main() -> @location(0) vec4f {
                    return vec4f(0.25, 0.25, 0.25, 1.0);
                }
            "#,
                &[],
                SCREEN_FORMAT,
            ),
        }
    }
}

fn seed_hdr(ctx: InMut<RenderContext>, values: Query<&TestRadiance>) {
    let value = values.get(ctx.camera_id).unwrap().0;
    let InMut(RenderContext { encoder, hdr, .. }) = ctx;
    let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Known HDR radiance"),
        color_attachments: &[Some(lentille_wgpu_utils::render_pass_color_attachment(
            &hdr.current().view,
            Some(wgpu::Color {
                r: value,
                g: value,
                b: value,
                a: 1.0,
            }),
            true,
        ))],
        ..Default::default()
    });
}

fn probe_background(ctx: InMut<RenderContext>, pipelines: Res<ProbePipelines>) {
    let InMut(RenderContext {
        encoder,
        hdr,
        camera_global_bind_group,
        ..
    }) = ctx;
    let size = hdr.current().texture.size();
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Partial-screen HDR background probe"),
        color_attachments: &[Some(lentille_wgpu_utils::render_pass_color_attachment(
            &hdr.current().view,
            None,
            true,
        ))],
        ..Default::default()
    });
    pass.set_pipeline(&pipelines.background);
    pass.set_bind_group(0, camera_global_bind_group.as_ref(), &[]);
    pass.set_scissor_rect(0, 0, size.width / 2, size.height);
    pass.draw(0..3, 0..1);
}

fn probe_overlay(ctx: InMut<RenderContext>, pipelines: Res<ProbePipelines>) {
    let InMut(RenderContext {
        encoder,
        display_target,
        ..
    }) = ctx;
    let size = display_target.texture.size();
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Exposure-independent display overlay"),
        color_attachments: &[Some(lentille_wgpu_utils::render_pass_color_attachment(
            &display_target.view,
            None,
            true,
        ))],
        ..Default::default()
    });
    pass.set_pipeline(&pipelines.overlay);
    pass.set_scissor_rect(size.width - 1, size.height - 1, 1, 1);
    pass.draw(0..3, 0..1);
}

#[test]
#[ignore = "requires a GPU and engine assets; run with --test-threads=1"]
fn gpu_hdr_colors_survive_stage_swaps_exposure_and_resize() {
    let old_dir = std::env::current_dir().unwrap();
    struct Restore(std::path::PathBuf);
    impl Drop for Restore {
        fn drop(&mut self) {
            std::env::set_current_dir(&self.0).unwrap();
        }
    }
    let _restore = Restore(old_dir);
    std::env::set_current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")).unwrap();

    let mut app = bevy_app::App::new();
    app.add_plugins(RenderPlugin)
        .add_render_system(
            seed_hdr,
            OpaqueStage.after(crate::systems::sys_render_main_pass),
        )
        .add_render_system(
            probe_background,
            TransparentStage.after(crate::systems::sys_render_transparent),
        )
        .add_render_system(probe_overlay, PostProcessStage);
    let graph = std::mem::replace(
        &mut *crate::resource::RENDER_RESOURCES_TO_ADD.lock().unwrap(),
        crate::resource::ResourceGraph::new(),
    );
    for initialize in graph {
        initialize(app.world_mut());
    }
    app.init_resource::<ProbePipelines>();
    app.world_mut().insert_resource(RenderReady);
    app.world_mut()
        .spawn((ParallelLight::default(), Transform::default()));
    let cameras: Vec<_> = [4.0, 16.0]
        .into_iter()
        .map(|value| {
            app.world_mut()
                .spawn((
                    Camera::new(1.0),
                    RenderTargetConfig::Texture {
                        width: 8,
                        height: 4,
                    },
                    TestRadiance(value),
                    CsmConfig {
                        level_count: 1,
                        texture_size: 16,
                        linear_log_factor: 0.5,
                        shadow_near: 1.0,
                        shadow_far: 10.0,
                    },
                ))
                .id()
        })
        .collect();

    // Initialize the real camera/G-buffer/CSM resources before evaluating pixels.
    for _ in 0..3 {
        app.update();
    }
    assert_display(app.world(), cameras[0], 8, 4, 4.0, 1.0);
    assert_display(app.world(), cameras[1], 8, 4, 16.0, 1.0);

    // A radically different next frame detects accidental history sampling.
    app.world_mut()
        .get_mut::<TestRadiance>(cameras[0])
        .unwrap()
        .0 = 1.0;
    app.world_mut()
        .resource_mut::<ToneMappingSettings>()
        .exposure_compensation_ev = 1.0;
    app.update();
    assert_display(app.world(), cameras[0], 8, 4, 1.0, 2.0);
    assert_display(app.world(), cameras[1], 8, 4, 16.0, 2.0);

    let old_display = app
        .world()
        .get::<RenderTarget>(cameras[0])
        .unwrap()
        .display_color();
    *app.world_mut()
        .get_mut::<RenderTargetSize>(cameras[0])
        .unwrap() = RenderTargetSize {
        width: 12,
        height: 6,
    };
    app.world_mut()
        .get_mut::<TestRadiance>(cameras[0])
        .unwrap()
        .0 = 0.18;
    app.world_mut()
        .resource_mut::<ToneMappingSettings>()
        .exposure_compensation_ev = 0.0;
    app.update();
    assert_display(app.world(), cameras[0], 12, 6, 0.18, 1.0);
    assert_display(app.world(), cameras[1], 8, 4, 16.0, 1.0);
    // Resize must not mutate an image still held by a presentation consumer.
    assert_eq!(
        (old_display.texture.width(), old_display.texture.height()),
        (8, 4)
    );
    assert_window_output(app.world(), cameras[0]);
}

fn assert_window_output(world: &World, camera: Entity) {
    use crate::{
        base_assets::SkyboxSampler,
        blit::{BlitPipeline, BlitShader, blit},
    };
    use lentille_wgpu_utils::texture_readback::read_texture_to_cpu;
    let rs = world.resource::<RenderState>();
    let source = world.get::<RenderTarget>(camera).unwrap().display_color();
    let expected = read_texture_to_cpu(&rs.device, &rs.queue, source.texture.texture());
    // The real presentation shader must preserve encoded SDR bytes on either
    // hardware-encoded sRGB or manually encoded UNORM swapchains.
    for format in [
        TextureFormat::Bgra8UnormSrgb,
        TextureFormat::Rgba8UnormSrgb,
        TextureFormat::Rgba8Unorm,
    ] {
        let destination = crate::camera::create_color_render_target_image(
            expected.width,
            expected.height,
            &rs.device,
            format,
        );
        let pipeline = BlitPipeline::new(&rs.device, &world.resource::<BlitShader>().0, format);
        blit(
            &rs.device,
            &rs.queue,
            &pipeline,
            &world.resource::<SkyboxSampler>().0,
            &source.view,
            &destination.view,
        );
        let actual = read_texture_to_cpu(&rs.device, &rs.queue, destination.texture.texture());
        for (actual, expected) in actual.data.iter().zip(&expected.data) {
            assert!(
                actual.abs_diff(*expected) <= 2,
                "presentation {format:?}: {actual} != {expected}"
            );
        }
    }
}

fn assert_display(
    world: &World,
    camera: Entity,
    width: u32,
    height: u32,
    radiance: f64,
    exposure: f64,
) {
    let target = world.get::<RenderTarget>(camera).unwrap();
    let rs = world.resource::<RenderState>();
    let display = target.display_color();
    let pixels = lentille_wgpu_utils::texture_readback::read_texture_to_cpu(
        &rs.device,
        &rs.queue,
        display.texture.texture(),
    );
    assert_eq!((pixels.width, pixels.height), (width, height));
    // Independent scalar reference: Reinhard followed by IEC sRGB encoding.
    let encode = |linear: f64| -> u8 {
        let encoded = if linear <= 0.0031308 {
            12.92 * linear
        } else {
            1.055 * linear.powf(1.0 / 2.4) - 0.055
        };
        (encoded * 255.0).round() as u8
    };
    for y in 0..height {
        for x in 0..width {
            let expected = if x == width - 1 && y == height - 1 {
                encode(0.25)
            } else {
                let value = radiance * exposure * if x < width / 2 { 0.5 } else { 1.0 };
                encode(value / (1.0 + value))
            };
            let offset = ((y * width + x) * 4) as usize;
            for actual in &pixels.data[offset..offset + 3] {
                assert!(
                    actual.abs_diff(expected) <= 2,
                    "camera {camera:?}, ({x},{y}), radiance={radiance}, exposure={exposure}: {actual} != {expected}"
                );
            }
            assert_eq!(pixels.data[offset + 3], 255);
        }
    }
}

#[test]
fn tone_mapping_shader_compiles_and_validates() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/shaders");
    let wesl = wesl::Wesl::new(root);
    let source = wesl
        .compile(&"package::postprocessing::tonemap".parse().unwrap())
        .expect("tone mapping WESL must compile")
        .to_string();
    let module = wgpu::naga::front::wgsl::parse_str(&source).expect("tone mapping WGSL must parse");
    wgpu::naga::valid::Validator::new(
        wgpu::naga::valid::ValidationFlags::all(),
        wgpu::naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .expect("tone mapping shader must validate");
}
