use std::{fs, sync::Arc};

use bevy_app::prelude::*;
use bevy_ecs::{prelude::*, system::RunSystemOnce, world::CommandQueue};
use egui::{epaint::text::InsertFontFamily, Visuals};
use lentille_core::{
    input::InputPlugin,
    time::{Time, TimePlugin},
};
use lentille_render::{
    cubemap::{CubemapConverterRgba16Float, CubemapMatrixBindGroups},
    prelude::*,
    skybox::{prefiltering::PrefilteringPipeline, sys_update_skybox_sh_from_path, Skybox},
    utils::cube::CubeVerticesBuffer,
    RenderPlugin,
};

use crate::{
    control::{camera::CameraController, ControlPlugin},
    egui_renderer::EguiRenderer,
};

mod control;
mod editor;
mod egui_renderer;

pub struct WgpuPbrPlugin;

impl Plugin for WgpuPbrPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((RenderPlugin, InputPlugin, TimePlugin))
            .add_plugins(ControlPlugin);

        app.add_systems(
            Startup,
            (
                sys_setup_egui_visual,
                sys_startup_light_and_environment,
                sys_generate_dragons_scene,
            ),
        )
        .add_systems(
            PreUpdate,
            (
                // self.world.resource_mut::<Time>().update();
                editor::sys_on_resize_render_target,
                editor::sys_egui_tiles,
            ),
        )
        .add_systems(Update, sys_update_rotation);
    }
}

#[derive(Debug, Component)]
pub struct RotationObject {
    pub speed: f32,
}

pub struct SpawnModelCmd<PB: Bundle, CB: Bundle + Clone> {
    model: Arc<Model>,
    parent_bundle: PB,
    child_bundle: CB,
}

impl<PB: Bundle, CB: Bundle + Clone> Command for SpawnModelCmd<PB, CB> {
    fn apply(self, world: &mut World) {
        let parent = world.spawn(self.parent_bundle).id();
        for mesh in self.model.meshes.iter() {
            let uploaded = Arc::new(mesh.upload(world));
            world.spawn((
                MeshRenderer::new(uploaded, world),
                TransformBuilder::default()
                    .parent(Some(parent))
                    .build()
                    .unwrap(),
                self.child_bundle.clone(),
            ));
        }
    }
}

fn sys_setup_egui_visual(mut egui: ResMut<EguiRenderer>) {
    let mut visual = Visuals::dark();
    let ctx = egui.context();

    visual.widgets.noninteractive.bg_stroke.width = 0.0;
    ctx.set_visuals(visual);

    let font_data =
        fs::read(AssetPath::Assets("fonts/MiSans-Normal.ttf".to_string()).final_path()).unwrap();
    ctx.add_font(egui::epaint::text::FontInsert::new(
        "MiSans",
        egui::FontData::from_owned(font_data),
        vec![InsertFontFamily {
            family: egui::FontFamily::Proportional,
            priority: egui::epaint::text::FontPriority::Highest,
        }],
    ));
}

pub fn sys_update_rotation(mut q: Query<(&mut Transform, &RotationObject)>, time: Res<Time>) {
    for (mut trans, rot) in q.iter_mut() {
        trans.rotation =
            Quat::from_angle_y(Rad(rot.speed) * time.delta_time.as_secs_f32()) * trans.rotation;
    }
}

fn random_color() -> Color {
    let r = rand::random::<f32>();
    let a = rand::random::<f32>();
    let g = (1. - r) * a;
    let b = (1. - r) - g;
    return Color::new(r, g, b, 1.0);
}

fn generate_point_lights(
    world: &mut World,
    x_size: f32,
    y_size: f32,
    z_size: f32,
    count: u32,
    light_intensity_offset: f32,
    light_intensity_scale: f32,
) {
    let mut vec = Vec::with_capacity(20usize);
    for _ in 0..count {
        let x = rand::random::<f32>() * x_size;
        let y = rand::random::<f32>() * y_size;
        let z = rand::random::<f32>() * z_size;
        vec.push((
            PointLight {
                color: random_color(),
                intensity: rand::random::<f32>() * light_intensity_scale + light_intensity_offset,
                ..Default::default()
            },
            Transform::with_position(Vec3::new(x, y, z)),
            Name::new("点光源"),
        ))
    }
    vec.into_iter().for_each(|it| {
        world.spawn(it);
    });
}

fn sys_generate_dragons_scene(world: &mut World) {
    let dragon_model = Arc::new(
        Model::load(AssetPath::new("models/DragonAttenuation/scene.gltf"), world).unwrap(),
    );
    let plane_model = Arc::new(Model::load(AssetPath::new("models/plane.glb"), world).unwrap());

    generate_point_lights(world, 12., 8., 2., 20, 1., 1.);

    let mut queue = CommandQueue::from_world(world);
    let mut commands = Commands::new(&mut queue, world);
    let count = 5;
    for i in 0..count {
        let value = (i as f32) / (count - 1) as f32;
        let mut transform = TransformBuilder::default()
            .position(Vec3::new(i as f32 * 2.5, 0., 0.))
            .rotation(Euler::new(Deg(90.), Deg(0.), Deg(-30.)).into())
            .scale(Vec3::new_unit(0.3))
            .build()
            .unwrap();

        commands.queue(SpawnModelCmd {
            model: dragon_model.clone(),
            parent_bundle: (
                transform.clone(),
                Name::new(format!("龙模型 No_{}", i)),
                RotationObject { speed: 0.5 },
            ),
            child_bundle: (
                CastShadow,
                MainPassObject,
                PBRMaterial {
                    metallic: Some(value),
                    ..Default::default()
                },
            ),
        });

        transform.position.y += 3.0;
        commands.queue(SpawnModelCmd {
            model: dragon_model.clone(),
            parent_bundle: (
                transform,
                Name::new(format!("龙模型 No_{}", i)),
                RotationObject { speed: 0.5 },
            ),
            child_bundle: (
                CastShadow,
                MainPassObject,
                PBRMaterial {
                    reflectance: Some(value),
                    ..Default::default()
                },
            ),
        });
    }

    for i in 0..count {
        let value = (i as f32) / (count - 1) as f32;
        let mut transform = TransformBuilder::default()
            .position(Vec3::new(i as f32 * 2.5, -4., 0.))
            .rotation(Euler::new(Deg(90.), Deg(0.), Deg(-30.)).into())
            .scale(Vec3::new_unit(0.3))
            .build()
            .unwrap();

        commands.queue(SpawnModelCmd {
            model: dragon_model.clone(),
            parent_bundle: (
                transform.clone(),
                RotationObject { speed: 0.5 },
                Name::new(format!("透明龙模型 No_{}", i)),
            ),
            child_bundle: (
                PBRMaterial {
                    color: Some(Color::WHITE.with_alpha(0.0)),
                    metallic: Some(value),
                    alpha_mode: Some(AlphaMode::Blend),
                    ..Default::default()
                },
                MainPassObject,
            ),
        });

        transform.position.y -= 3.0;
        commands.queue(SpawnModelCmd {
            model: dragon_model.clone(),
            parent_bundle: (transform, Name::new(format!("透明龙模型 No_{}", i))),
            child_bundle: (
                PBRMaterial {
                    color: Some(Color::WHITE.with_alpha(0.0)),
                    reflectance: Some(value),
                    alpha_mode: Some(AlphaMode::Blend),
                    ..Default::default()
                },
                MainPassObject,
            ),
        });
    }

    // Colored transparent
    for i in 0..count {
        let value = (i as f32) / (count - 1) as f32;
        let mut transform = TransformBuilder::default()
            .position(Vec3::new(i as f32 * 2.5, -10., 0.))
            .rotation(Euler::new(Deg(90.), Deg(0.), Deg(-30.)).into())
            .scale(Vec3::new_unit(0.3))
            .build()
            .unwrap();

        commands.queue(SpawnModelCmd {
            model: dragon_model.clone(),
            parent_bundle: (
                transform.clone(),
                RotationObject { speed: 0.5 },
                Name::new(format!("透明龙模型 No_{}", i)),
            ),
            child_bundle: (
                PBRMaterial {
                    color: Some(random_color().with_alpha(value)),
                    metallic: Some(value),
                    alpha_mode: Some(AlphaMode::Blend),
                    ..Default::default()
                },
                MainPassObject,
            ),
        });

        transform.position.y -= 3.0;
        commands.queue(SpawnModelCmd {
            model: dragon_model.clone(),
            parent_bundle: (
                transform,
                Name::new(format!("透明龙模型 No_{}", i)),
                RotationObject { speed: 0.5 },
            ),
            child_bundle: (
                PBRMaterial {
                    color: Some(random_color().with_alpha(value)),
                    reflectance: Some(value),
                    alpha_mode: Some(AlphaMode::Blend),
                    ..Default::default()
                },
                MainPassObject,
            ),
        });
    }

    // commands.queue(SpawnModelCmd {
    //     model: plane_model.clone(),
    //     parent_bundle: (
    //         TransformBuilder::default()
    //             .position(Vec3::new_y(-1.0))
    //             .build()
    //             .unwrap(),
    //         Name::new("平面".to_string()),
    //     ),
    //     child_bundle: (
    //         CastShadow,
    //         MainPassObject,
    //         PBRMaterial {
    //             ..Default::default()
    //         },
    //     ),
    // });

    queue.apply(world);
}

fn sys_generate_single_model(input: In<(AssetPath, String, f32)>, world: &mut World) {
    let In((model_asset_path, name, scale)) = input;

    generate_point_lights(world, 2., 3., 3., 10, 1.0, 1.0);

    let model = Arc::new(Model::load(model_asset_path, world).unwrap());

    let mut queue = CommandQueue::from_world(world);
    let mut commands = Commands::new(&mut queue, world);

    commands.queue(SpawnModelCmd {
        model,
        parent_bundle: (
            TransformBuilder::default()
                .scale(Vec3::one() * scale)
                .rotation(Quat::from_angle_x(Deg(90.0)))
                .build()
                .unwrap(),
            Name::new(name),
        ),
        child_bundle: (CastShadow, MainPassObject),
    });

    queue.apply(world);
}

fn sys_generate_unreal_vr_room_scene(world: &mut World) {
    generate_point_lights(world, 2., 3., 3., 10, 1.0, 1.0);

    let bistro_model = Arc::new(
        Model::load(
            AssetPath::new("models/sony_tc-510-2_tape_recorder/scene.gltf"),
            world,
        )
        .unwrap(),
    );

    let mut queue = CommandQueue::from_world(world);
    let mut commands = Commands::new(&mut queue, world);

    commands.queue(SpawnModelCmd {
        model: bistro_model,
        parent_bundle: (
            TransformBuilder::default()
                .scale(Vec3::one() * 0.1)
                .build()
                .unwrap(),
            Name::new("Room".to_string()),
        ),
        child_bundle: (CastShadow, MainPassObject),
    });

    queue.apply(world);
}

fn sys_startup_light_and_environment(world: &mut World) {
    let light_arrow_model =
        Arc::new(Model::load(AssetPath::new("models/arrow.glb"), world).unwrap());

    let config = &world.resource::<RenderState>().config;
    let aspect = config.width as f32 / config.height as f32;

    world.spawn((
        Camera {
            aspect,
            fovy: 17.1,
            ..Camera::new(aspect)
        },
        CameraController {
            row: -4.8,
            yaw: 0.0,
        },
        TransformBuilder::default()
            .position(Vec3::new(2.5, 0.6, 31.1))
            .rotation(Euler::new(Deg(0.0), Deg(-4.0), Deg(0.0)).into())
            .build()
            .unwrap(),
        Name::new("相机"),
    ));

    SpawnModelCmd {
        model: light_arrow_model.clone(),
        parent_bundle: (
            TransformBuilder::default()
                .position(Vec3::new(0., 4., 5.))
                .rotation(Quat::from_angle_x(Deg(-45.)))
                .build()
                .unwrap(),
            ParallelLight::default(),
            Name::new("平行光源"),
        ),
        child_bundle: (MainPassObject,),
    }
    .apply(world);

    let skybox_image_path = AssetPath::new("textures/hdr/warm_restaurant_night_4k.hdr");
    // let skybox_image_path = AssetPath::new("textures/hdr/golden_gate_hills_4k.hdr");
    let skybox_image = world
        .run_system_once_with(sys_load_hdir_and_prefiler, skybox_image_path.clone())
        .unwrap();
    world
        .run_system_once_with(sys_update_skybox_sh_from_path, skybox_image_path)
        .unwrap();

    world.spawn(Skybox {
        texture: Some(skybox_image),
    });
}

pub fn sys_load_hdir_and_prefiler(input: In<AssetPath>, world: &mut World) -> UploadedImage {
    let pipeline = PrefilteringPipeline::new(world, wgpu::TextureFormat::Rgba16Float);

    let rs = world.resource::<RenderState>();
    let converter = world.resource::<CubemapConverterRgba16Float>();
    let cube_vertices_buffer = world.resource::<CubeVerticesBuffer>();
    let matrix_bind_groups = world.resource::<CubemapMatrixBindGroups>();

    let device = &rs.device;
    let queue = &rs.queue;
    let In(path) = input;

    let hdri = UploadedImageWithSampler::load_hdri_to_f16(path, device, queue).unwrap();

    let source_texture = {
        converter.0.render_hdir_to_cube_map(
            device,
            queue,
            &hdri.view,
            &cube_vertices_buffer.vertices_buffer,
            512,
        )
    };

    let source_cubemap_view = source_texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::Cube),
        ..Default::default()
    });

    lentille_render::skybox::prefiltering::prefilter(
        Some("Default Skybox"),
        &rs.device,
        &rs.queue,
        &source_texture,
        &source_cubemap_view,
        5,
        1145,
        &pipeline,
        matrix_bind_groups,
        cube_vertices_buffer,
    )
    .unwrap()
}
