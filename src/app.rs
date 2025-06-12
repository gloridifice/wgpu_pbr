use std::fs;

use crate::{
    asset::AssetPath,
    editor,
    egui_tools::EguiRenderer,
    engine::{
        input::{Input, InputPlugin},
        time::TimePlugin,
    },
    render::{self, prelude::*, systems::sys_refersh_global_bind_group},
};
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use egui::{epaint::text::InsertFontFamily, Visuals};

use std::sync::Arc;

use crate::cgmath_ext::{Vec3, Vec4, VectorExt};
use crate::egui_tools::EguiConfig;
use crate::render::bindings::global_binding::{GlobalBindGroup, GlobalUniformBuffer};
use crate::render::camera::{sys_update_camera_uniform, Camera, CameraController};
use crate::render::cubemap::{CubemapConverterRgba16Float, CubemapMatrixBindGroups};
use crate::render::defered_rendering::write_g_buffer_pipeline::{
    GBufferTexturesBindGroup, WriteGBufferPipeline,
};
use crate::render::defered_rendering::MainPipeline;
use crate::render::gizmos::{GizmosGlobalBindGroup, GizmosPipeline};
use crate::render::light::parallel_light::ParallelLight;
use crate::render::light::point_light::PointLight;
use crate::render::light::{
    event_on_remove_point_light, sys_update_dynamic_lights, sys_update_dynamic_lights_bind_group,
    DynamicLights,
};
use crate::render::material::pbr::{sys_update_override_pbr_material_bind_group, PBRMaterial};
use crate::render::post_processing::PostProcessingManager;
use crate::render::shader_loader::ShaderLoader;
use crate::render::shadow_mapping::{CastShadow, ShadowMapGlobalBindGroup, ShadowMappingPipeline};
use crate::render::skybox::prefiltering::{self, PrefilteringPipeline};
use crate::render::skybox::{Skybox, SkyboxPipeline, SkyboxSHBuffer};
use crate::render::transform::WorldTransform;
use crate::render::transparent::TransparentPipeline;
use crate::render::utils::cube::CubeVerticesBuffer;
use crate::MainWindow;
use crate::{
    asset::load::Loadable,
    engine::time::Time,
    render::{
        camera::{CameraBuffer, CameraConfig},
        shadow_mapping::ShadowMap,
        transform::{Transform, TransformBuilder},
    },
    RenderState,
};
use bevy_ecs::system::RunSystemOnce;
use bevy_ecs::world::CommandQueue;
use cgmath::{Deg, Euler, Quaternion, Rad, Rotation3};
use winit::keyboard::KeyCode;

pub struct AppPlugin;

impl Plugin for AppPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((InputPlugin, TimePlugin));
        app.init_resource::<ShaderLoader>()
            .init_resource::<WhiteTexture>()
            .init_resource::<NormalDefaultTexture>()
            .init_resource::<crate::render::dfg::DFGTexture>()
            .init_resource::<crate::render::mipmap::DefaultMipmapGenShader>()
            .init_resource::<MissingTexture>()
            .init_resource::<crate::render::material::buffer_material::BufferMaterialManager>()
            .init_resource::<RenderTargetSize>()
            .init_resource::<ColorRenderTarget>()
            .init_resource::<DepthRenderTarget>()
            .init_resource::<crate::editor::RenderTargetEguiTexId>()
            .init_resource::<super::render::utils::cube::CubeVerticesBuffer>()
            .init_resource::<super::render::cubemap::CubemapVertexShader>()
            .init_resource::<crate::render::cubemap::CubemapConvertingShader>()
            .init_resource::<crate::render::cubemap::CubemapMatrixBindGroups>()
            .init_resource::<crate::render::cubemap::CubemapConverterRgba16Float>()
            .init_resource::<crate::render::skybox::DefaultSkybox>()
            .init_resource::<GlobalUniformBuffer>()
            // --- Render resource ---
            .init_resource::<CameraBuffer>()
            .init_resource::<SkyboxSHBuffer>()
            .init_resource::<LightUniformBuffer>()
            .init_resource::<ShadowMap>()
            // .insert_resource::<ShadowMapEguiTextureId>()
            .init_resource::<FullScreenVertexShader>()
            // 0. Layouts
            .init_resource::<ObjectBindGroupLayout>()
            .init_resource::<GizmosGlobalBindGroup>()
            .init_resource::<PBRMaterialBindGroupLayout>()
            // 1. Globals
            .init_resource::<ShadowMapGlobalBindGroup>()
            .init_resource::<DynamicLightBindGroup>()
            // 1.5
            .init_resource::<GBufferTexturesBindGroup>()
            .init_resource::<GlobalBindGroup>()
            // 2. Pipelines
            .init_resource::<WriteGBufferPipeline>()
            .init_resource::<SkyboxPipeline>()
            .init_resource::<MainPipeline>()
            .init_resource::<TransparentPipeline>()
            .init_resource::<ShadowMappingPipeline>()
            .init_resource::<GizmosPipeline>()
            // Post Processing
            .init_resource::<PostProcessingManager>()
            // --- Other resources ---
            .init_resource::<ControlState>()
            .init_resource::<DynamicLights>()
            .insert_resource(EguiConfig::default())
            .insert_resource(CameraConfig::default())
            .init_resource::<DefaultPBRMaterial>();

        app.add_observer(event_on_remove_point_light)
            .add_systems(
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
            .add_systems(
                Update,
                (
                    sys_input,
                    render::camera::sys_update_camera_control,
                    sys_update_rotation,
                    sys_refersh_global_bind_group,
                ),
            )
            .add_systems(
                PostUpdate,
                (
                    render::transform::sys_update_world_transform,
                    render::transform::sys_update_children,
                    sys_update_transform_buffers,
                    // Update camera uniform
                    sys_update_camera_uniform,
                    // Update light uniform
                    render::light::sys_update_light_uniform,
                    // Dynamic Lights
                    sys_update_dynamic_lights,
                    sys_update_dynamic_lights_bind_group,
                    // Override Material
                    sys_update_override_pbr_material_bind_group,
                ),
            );
    }
}

#[derive(Debug, Component, Clone)]
pub struct Name(pub String);

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

#[derive(Resource)]
pub struct ControlState {
    pub is_focused: bool,
}
impl Default for ControlState {
    fn default() -> Self {
        ControlState { is_focused: true }
    }
}

pub fn sys_update_rotation(mut q: Query<(&mut Transform, &RotationObject)>, time: Res<Time>) {
    for (mut trans, rot) in q.iter_mut() {
        trans.rotation = Quaternion::from_angle_y(Rad(rot.speed) * time.delta_time.as_secs_f32())
            * trans.rotation;
    }
}

pub fn sys_input(
    mut commands: Commands,
    input: Res<Input>,
    mut control_state: ResMut<ControlState>,
) {
    if input.is_key_down(KeyCode::Escape) {
        control_state.is_focused = !control_state.is_focused;
        commands.queue(|world: &mut World| {
            world.run_system_cached(sys_control_state).unwrap();
        });
    }
}

pub fn sys_control_state(control_state: ResMut<ControlState>, main_window: Res<MainWindow>) {
    main_window.0.set_cursor_visible(!control_state.is_focused);
    let _ = main_window.0.set_cursor_grab(if control_state.is_focused {
        winit::window::CursorGrabMode::Locked
    } else {
        winit::window::CursorGrabMode::None
    });
}

fn sys_update_transform_buffers(world: &mut World) {
    world.resource_scope(|world, render_state: Mut<RenderState>| {
        let mut query =
            world.query_filtered::<(&WorldTransform, &MeshRenderer), Changed<WorldTransform>>();
        for (world_trans, mesh_renderer) in query.iter(world) {
            mesh_renderer.update_transform_buffer(&render_state.queue, world_trans.get_uniform());
        }
    });
}

fn random_color_vec3() -> Vec3 {
    let r = rand::random::<f32>();
    let a = rand::random::<f32>();
    let g = (1. - r) * a;
    let b = (1. - r) - g;
    return Vec3::new(r, g, b);
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
                color: random_color_vec3().extend(1.0),
                intensity: rand::random::<f32>() * light_intensity_scale + light_intensity_offset,
                ..Default::default()
            },
            Transform::with_position(Vec3::new(x, y, z)),
            Name("点光源".to_string()),
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
                Name(format!("龙模型 No_{}", i)),
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
                Name(format!("龙模型 No_{}", i)),
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
                Name(format!("透明龙模型 No_{}", i)),
            ),
            child_bundle: (
                PBRMaterial {
                    color: Some(Vec4::new(1.0, 1.0, 1.0, 0.0)),
                    metallic: Some(value),
                    alpha_mode: Some(render::AlphaMode::Blend),
                    ..Default::default()
                },
                MainPassObject,
            ),
        });

        transform.position.y -= 3.0;
        commands.queue(SpawnModelCmd {
            model: dragon_model.clone(),
            parent_bundle: (transform, Name(format!("透明龙模型 No_{}", i))),
            child_bundle: (
                PBRMaterial {
                    color: Some(Vec4::new(1.0, 1.0, 1.0, 0.0)),
                    reflectance: Some(value),
                    alpha_mode: Some(render::AlphaMode::Blend),
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
                Name(format!("透明龙模型 No_{}", i)),
            ),
            child_bundle: (
                PBRMaterial {
                    color: Some(random_color_vec3().extend(value)),
                    metallic: Some(value),
                    alpha_mode: Some(render::AlphaMode::Blend),
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
                Name(format!("透明龙模型 No_{}", i)),
                RotationObject { speed: 0.5 },
            ),
            child_bundle: (
                PBRMaterial {
                    color: Some(random_color_vec3().extend(value)),
                    reflectance: Some(value),
                    alpha_mode: Some(render::AlphaMode::Blend),
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
    //         Name("平面".to_string()),
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
                .rotation(Quaternion::from_angle_x(Deg(90.0)))
                .build()
                .unwrap(),
            Name(name),
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
            Name("Room".to_string()),
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
        Name("相机".to_string()),
    ));

    SpawnModelCmd {
        model: light_arrow_model.clone(),
        parent_bundle: (
            TransformBuilder::default()
                .position(Vec3::new(0., 4., 5.))
                .rotation(Quaternion::from_angle_x(Deg(-45.)))
                .build()
                .unwrap(),
            ParallelLight::default(),
            Name("平行光源".to_string()),
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
        .run_system_once_with(
            render::skybox::sys_update_skybox_sh_from_path,
            skybox_image_path,
        )
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

    prefiltering::prefilter(
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
