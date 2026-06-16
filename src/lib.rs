use std::sync::Arc;

use bevy_app::prelude::*;
use bevy_ecs::{prelude::*, system::RunSystemOnce, world::CommandQueue};
use bevy_log::LogPlugin;
use lentille_core::{
    input::InputPlugin,
    time::{Time, TimePlugin},
    window::WindowPlugin,
};
use lentille_render::{
    RenderPlugin, RenderPreparedStartup, SCREEN_FORMAT,
    camera::RenderTargetConfig,
    cubemap::{CubemapConverterRgba16Float, CubemapMatrixBindGroups},
    prelude::*,
    shadow_mapping::csm::CsmConfig,
    skybox::{Skybox, prefiltering::PrefilteringPipeline, sys_update_skybox_sh_from_path},
    utils::cube::CubeVerticesBuffer,
};

use crate::{
    control::{
        ControlPlugin,
        camera::{CameraController, MainCamera},
    },
    editor::EditorPlugin,
};

mod control;
mod editor;
mod egui_renderer;

pub struct WgpuPbrPlugin;

impl Plugin for WgpuPbrPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            LogPlugin::default(),
            RenderPlugin,
            InputPlugin,
            TimePlugin,
            WindowPlugin,
        ))
        .add_plugins((ControlPlugin, EditorPlugin));

        app.add_systems(RenderPreparedStartup, sys_spawn_camera)
            .add_systems(
                Startup,
                (sys_generate_plane_scene, sys_startup_light_and_environment),
            )
            .add_systems(Update, (sys_update_rotation, sys_gizmo));
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
    Color::new(r, g, b, 1.0)
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
            Transform::from_position(Vec3::new(x, y, z)),
            Name::new("点光源"),
        ))
    }
    vec.into_iter().for_each(|it| {
        world.spawn(it);
    });
}

// Commented out — requires models/DragonAttenuation/scene.gltf (not in repo)
// pub fn sys_generate_dragons_scene(world: &mut World) { ... }

fn create_procedural_plane_model(size: f32) -> Model {
    let half = size / 2.0;
    let vertices = vec![
        Vertex {
            position: [-half, 0.0, -half],
            normal: [0.0, 1.0, 0.0],
            tangent: [1.0, 0.0, 0.0],
            color: [1.0; 4],
            tex_coord: [0.0, 0.0],
        },
        Vertex {
            position: [half, 0.0, -half],
            normal: [0.0, 1.0, 0.0],
            tangent: [1.0, 0.0, 0.0],
            color: [1.0; 4],
            tex_coord: [1.0, 0.0],
        },
        Vertex {
            position: [half, 0.0, half],
            normal: [0.0, 1.0, 0.0],
            tangent: [1.0, 0.0, 0.0],
            color: [1.0; 4],
            tex_coord: [1.0, 1.0],
        },
        Vertex {
            position: [-half, 0.0, half],
            normal: [0.0, 1.0, 0.0],
            tangent: [1.0, 0.0, 0.0],
            color: [1.0; 4],
            tex_coord: [0.0, 1.0],
        },
    ];
    let indices: Vec<u32> = vec![2, 1, 0, 3, 2, 0];
    let mesh = Mesh {
        vertices,
        indices: indices.clone(),
        primitives: vec![Primitive {
            indices_start: 0,
            indices_num: indices.len() as u32,
            material: None,
        }],
    };
    Model { meshes: vec![mesh] }
}

pub fn sys_generate_plane_scene(world: &mut World) {
    let plane_model = Arc::new(create_procedural_plane_model(24.0));

    generate_point_lights(world, 10., 5., 10., 10, 0.5, 1.5);

    let mut queue = CommandQueue::from_world(world);
    let mut commands = Commands::new(&mut queue, world);

    commands.queue(SpawnModelCmd {
        model: plane_model,
        parent_bundle: (
            Transform::from_position(Vec3::new(0.0, -0.1, 0.0)),
            Name::new("地面"),
        ),
        child_bundle: (
            CastShadow,
            MainPassObject,
            PbrMaterial {
                color: Some(Color::new(0.7, 0.7, 0.7, 1.0)),
                roughness: Some(0.8),
                metallic: Some(0.05),
                ..Default::default()
            },
        ),
    });

    world
        .run_system_once_with(
            sys_generate_single_model,
            (
                AssetPath::new("models/stanford_dragon_pbr.glb"),
                (Name::new("Dragon"), Transform::new().scale(0.05)),
            ),
        )
        .unwrap();

    world
        .run_system_once_with(
            sys_generate_single_model,
            (
                AssetPath::new("models/rough_mountaintop_landscape.glb"),
                (
                    Name::new("Landscape"),
                    Transform::new()
                        .position(Vec3::new(0., -375., 0.))
                        .scale(0.5),
                ),
            ),
        )
        .unwrap();

    world
        .run_system_once_with(
            sys_generate_single_model,
            (
                AssetPath::new("models/vending_machine2k_retro-futuristic/scene.gltf"),
                (
                    Name::new("Vending Machine"),
                    Transform::new()
                        .rotation(Quat::from_angle_x(Deg(-90.0)))
                        .scale(2.0),
                ),
            ),
        )
        .unwrap();

    generate_point_lights(world, 2., 3., 3., 10, 1.0, 1.0);

    queue.apply(world);
}

fn sys_generate_single_model(input: In<(AssetPath, impl Bundle)>, world: &mut World) {
    let In((model_asset_path, bundle)) = input;

    let model = match Model::load(model_asset_path, world) {
        Ok(model) => Arc::new(model),
        Err(e) => {
            bevy_log::error!("Failed to load model: {e}");
            return;
        }
    };

    let mut queue = CommandQueue::from_world(world);
    let mut commands = Commands::new(&mut queue, world);

    commands.queue(SpawnModelCmd {
        model,
        parent_bundle: bundle,
        child_bundle: (
            CastShadow,
            MainPassObject,
            PbrMaterial {
                color: Some(Color::new(0.7, 0.7, 0.7, 1.0)),
                roughness: Some(0.8),
                metallic: Some(0.05),
                ..Default::default()
            },
        ),
    });

    queue.apply(world);
}

pub fn sys_spawn_camera(mut commands: Commands) {
    commands.spawn((
        Name::new("相机"),
        MainCamera,
        Camera {
            fovy: 50.0,
            ..Camera::new(1.0)
        },
        CsmConfig {
            level_count: 4,
            texture_size: 2048,
            linear_log_factor: 0.5,
            shadow_near: 1.0,
            shadow_far: 80.0,
        },
        RenderTargetConfig::Texture {
            width: 1600,
            height: 900,
            format: SCREEN_FORMAT,
        },
        CameraController {
            row: 0.0,
            yaw: -25.0,
        },
        TransformBuilder::default()
            .position(Vec3::new(0.0, 9.0, 17.0))
            .rotation(Euler::new(Deg(-25.0), Deg(0.0), Deg(0.0)).into())
            .build()
            .unwrap(),
    ));
}

fn sys_startup_light_and_environment(world: &mut World) {
    world.spawn((
        TransformBuilder::default()
            .position(Vec3::new(0., 4., 5.))
            .rotation(Quat::from_angle_x(Deg(-45.)))
            .build()
            .unwrap(),
        ParallelLight::default(),
        Name::new("平行光源"),
    ));

    // Optionally spawn arrow model to visualize light direction
    if let Ok(light_arrow_model) = Model::load(AssetPath::new("models/arrow.glb"), world) {
        SpawnModelCmd {
            model: Arc::new(light_arrow_model),
            parent_bundle: Transform::default(),
            child_bundle: (MainPassObject,),
        }
        .apply(world);
    }

    // Optionally load HDR skybox; fall back to default cubemap if missing
    let skybox_image_path = AssetPath::new("textures/skybox/sky_110_2k.png");
    if let Ok(skybox_image) =
        world.run_system_once_with(sys_load_hdir_and_prefiler, skybox_image_path.clone())
    {
        if world
            .run_system_once_with(sys_update_skybox_sh_from_path, skybox_image_path)
            .is_ok()
        {
            world.spawn(Skybox {
                texture: Some(skybox_image),
            });
        }
    }
}

pub fn sys_load_hdir_and_prefiler(
    input: In<AssetPath>,
    world: &mut World,
) -> lentille_render::image::UploadedImage<
    lentille_render::prelude::DimCube,
    lentille_render::prelude::SampleFloatFilterable,
> {
    let pipeline = PrefilteringPipeline::new(world, wgpu::TextureFormat::Rgba16Float);

    let rs = world.resource::<RenderState>();
    let converter = world.resource::<CubemapConverterRgba16Float>();
    let cube_vertices_buffer = world.resource::<CubeVerticesBuffer>();
    let matrix_bind_groups = world.resource::<CubemapMatrixBindGroups>();

    let device = &rs.device;
    let queue = &rs.queue;
    let In(path) = input;

    let hdri = UploadedImage::load_hdri_to_f16(path, device, queue).unwrap();

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

fn sys_gizmo(
    lights: Query<(&PointLight, &WorldTransform)>,
    camera: Query<&WorldTransform, (With<Camera>, Without<MainCamera>)>,
) {
    for (light, trans) in lights.iter() {
        Gizmo::dot(trans.position, 0.1, light.color.into());
    }

    for transform in camera.iter() {
        Gizmo::line(
            transform.position,
            transform.position + transform.forward() * 2.0,
            Color::GREEN,
        );

        Gizmo::dot(transform.position, 0.5, Color::GREEN);
    }
}
