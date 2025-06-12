use std::fs;
use std::sync::Arc;

use crate::cgmath_ext::{Vec3, Vec4, VectorExt};
use crate::editor::{self, sys_egui_tiles, RenderTargetEguiTexId};
use crate::egui_tools::{EguiConfig, EguiRenderer};
use crate::render::bindings::global_binding::{GlobalBindGroup, GlobalUniformBuffer};
use crate::render::camera::{
    sys_update_camera_control, sys_update_camera_uniform, Camera, CameraController,
};
use crate::render::cubemap::{
    CubemapConverterRgba16Float, CubemapConvertingShader, CubemapMatrixBindGroups,
};
use crate::render::defered_rendering::write_g_buffer_pipeline::{
    GBufferTexturesBindGroup, WriteGBufferPipeline,
};
use crate::render::defered_rendering::MainPipeline;
use crate::render::dfg::DFGTexture;
use crate::render::gizmos::{GizmosGlobalBindGroup, GizmosPipeline};
use crate::render::light::parallel_light::ParallelLight;
use crate::render::light::point_light::PointLight;
use crate::render::light::{
    event_on_remove_point_light, sys_update_dynamic_lights, sys_update_dynamic_lights_bind_group,
    DynamicLights,
};
use crate::render::material::buffer_material::BufferMaterialManager;
use crate::render::material::pbr::{sys_update_override_pbr_material_bind_group, PBRMaterial};
use crate::render::mipmap::DefaultMipmapGenShader;
use crate::render::post_processing::{PostProcessingManager, RenderStage};
use crate::render::prelude::*;
use crate::render::shader_loader::ShaderLoader;
use crate::render::shadow_mapping::{CastShadow, ShadowMapGlobalBindGroup, ShadowMappingPipeline};
use crate::render::skybox::prefiltering::{self, PrefilteringPipeline};
use crate::render::skybox::{DefaultSkybox, Skybox, SkyboxPipeline, SkyboxSHBuffer};
use crate::render::systems::{sys_refersh_global_bind_group, PassRenderContext};
use crate::render::transform::WorldTransform;
use crate::render::transparent::TransparentPipeline;
use crate::render::utils::cube::CubeVerticesBuffer;
use crate::MainWindow;
use crate::{
    asset::{load::Loadable, AssetPath},
    engine::input::Input,
    engine::time::Time,
    render::{
        self,
        camera::{CameraBuffer, CameraConfig},
        light::LightUnifromBuffer,
        shadow_mapping::ShadowMap,
        transform::{Transform, TransformBuilder},
    },
    RenderState, State,
};
use bevy_ecs::prelude::*;
use bevy_ecs::system::{Commands, ResMut, Resource};
use bevy_ecs::world::{Command, CommandQueue, FromWorld, Mut, World};
use bevy_ecs::{
    component::Component,
    system::{Query, Res, RunSystemOnce},
};
use cgmath::{Deg, Euler, Quaternion, Rad, Rotation3};
use egui::epaint::text::InsertFontFamily;
use egui::Visuals;
use winit::event::DeviceEvent;
use winit::{event::WindowEvent, keyboard::KeyCode};

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

impl State {
    pub fn init_resource<R>(&mut self)
    where
        R: Resource + FromWorld,
    {
        let r = R::from_world(&mut self.world);
        self.world.insert_resource(r);
    }

    fn init_egui(&mut self) {
        let renderer = self.world.resource_mut::<EguiRenderer>();
        let ctx = renderer.context();
        let font_data =
            fs::read(AssetPath::Assets("fonts/MiSans-Normal.ttf".to_string()).final_path())
                .unwrap();
        ctx.add_font(egui::epaint::text::FontInsert::new(
            "MiSans",
            egui::FontData::from_owned(font_data),
            vec![InsertFontFamily {
                family: egui::FontFamily::Proportional,
                priority: egui::epaint::text::FontPriority::Highest,
            }],
        ));
    }

    pub fn init(&mut self) {
        self.init_egui();
        self.init_resource::<ShaderLoader>();
        self.init_resource::<WhiteTexture>();
        self.init_resource::<NormalDefaultTexture>();
        self.init_resource::<DFGTexture>();
        self.init_resource::<DefaultMipmapGenShader>();
        self.init_resource::<MissingTexture>();
        self.init_resource::<BufferMaterialManager>();
        self.init_resource::<RenderTargetSize>();
        self.init_resource::<ColorRenderTarget>();
        self.init_resource::<DepthRenderTarget>();
        self.init_resource::<RenderTargetEguiTexId>();
        self.init_resource::<render::utils::cube::CubeVerticesBuffer>();
        self.init_resource::<render::cubemap::CubemapVertexShader>();
        self.init_resource::<CubemapConvertingShader>();
        self.init_resource::<CubemapMatrixBindGroups>();
        self.init_resource::<CubemapConverterRgba16Float>();
        self.init_resource::<DefaultSkybox>();
        self.init_resource::<GlobalUniformBuffer>();

        // --- Render resource ---
        self.init_resource::<CameraBuffer>();
        self.init_resource::<SkyboxSHBuffer>();
        self.world
            .insert_resource(LightUnifromBuffer::new(&self.render_state().device));
        self.init_resource::<ShadowMap>();
        // self.insert_resource::<ShadowMapEguiTextureId>();

        self.init_resource::<FullScreenVertexShader>();

        // 0. Layouts
        self.init_resource::<ObjectBindGroupLayout>();
        self.init_resource::<GizmosGlobalBindGroup>();
        self.init_resource::<PBRMaterialBindGroupLayout>();

        // 1. Globals
        self.init_resource::<ShadowMapGlobalBindGroup>();
        self.init_resource::<DynamicLightBindGroup>();

        // 1.5
        self.init_resource::<GBufferTexturesBindGroup>();
        self.init_resource::<GlobalBindGroup>();

        // 2. Pipelines
        self.init_resource::<WriteGBufferPipeline>();
        self.init_resource::<SkyboxPipeline>();
        self.init_resource::<MainPipeline>();
        self.init_resource::<TransparentPipeline>();
        self.init_resource::<ShadowMappingPipeline>();
        self.init_resource::<GizmosPipeline>();

        // Post Processing
        self.init_resource::<PostProcessingManager>();

        // --- Other resources ---
        self.init_resource::<Input>();
        self.init_resource::<ControlState>();
        self.init_resource::<DynamicLights>();
        self.world.insert_resource(Time::default());
        self.world.insert_resource(EguiConfig::default());
        self.world.insert_resource(CameraConfig::default());
        self.init_resource::<DefaultPBRMaterial>();

        // Add Events'Observers
        self.world.add_observer(event_on_remove_point_light);

        {
            // Set egui visual / style / theme
            let egui = self.world.resource_mut::<EguiRenderer>();
            let mut visual = Visuals::dark();
            visual.widgets.noninteractive.bg_stroke.width = 0.0;
            egui.context().set_visuals(visual);
        }

        self.world
            .run_system_once(sys_startup_light_and_environment)
            .unwrap();
        self.world
            .run_system_once(sys_generate_dragons_scene)
            .unwrap();
        // self.world
        //     .run_system_once_with(
        //         (AssetPath::new("models/ship.glb"), "船模型".to_string(), 1.0),
        //         sys_generate_single_model,
        //     )
        //     .unwrap();

        self.world.run_system_cached(sys_control_state).unwrap();
    }

    pub fn window_input(&mut self, event: &WindowEvent) -> bool {
        self.world.resource_mut::<Input>().window_input(event);
        false
    }
    pub fn device_input(&mut self, event: &DeviceEvent) {
        self.world.resource_mut::<Input>().device_input(event);
    }

    pub fn handle_redraw(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let window = self.window.clone();
        self.egui_renderer_mut().begin_frame(&window);
        self.pre_update();
        self.update();
        self.post_update();

        match self.render() {
            Ok(_) => {}
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.resize(self.render_state().size)
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                log::error!("OutOfMemory");
                event_loop.exit();
            }
            // This happaens when a frame takes too long to present
            Err(wgpu::SurfaceError::Timeout) => {
                log::warn!("Surface timeout")
            }
            Err(wgpu::SurfaceError::Other) => {
                log::warn!("Other Error of wgpu surface occeur!")
            }
        }
    }

    pub fn pre_update(&mut self) {
        self.world.resource_mut::<Time>().update();
        self.world.run_system_cached(Input::sys_pre_update).unwrap();
        self.world
            .run_system_cached(editor::sys_on_resize_render_target)
            .unwrap();
        self.world.run_system_cached(sys_egui_tiles).unwrap();
    }

    pub fn update(&mut self) {
        self.world.run_system_once(sys_input).unwrap();
        self.world
            .run_system_once(sys_update_camera_control)
            .unwrap();
        self.world.run_system_once(sys_update_rotation).unwrap();
        self.world
            .run_system_once(sys_refersh_global_bind_group)
            .unwrap();
    }

    pub fn post_update(&mut self) {
        // Update transform unifrom
        self.run_system_once(render::transform::sys_update_world_transform);
        self.run_system_once(render::transform::sys_update_children);

        self.run_system_once(sys_update_transform_buffers);

        // Update camera uniform
        self.run_system_cached(sys_update_camera_uniform);

        // Update light uniform
        self.run_system_cached(render::light::sys_update_light_uniform);

        // Clear Down an Up maps
        self.run_system_cached(Input::sys_post_update);

        // Dynamic Lights
        self.run_system_cached(sys_update_dynamic_lights);
        self.run_system_cached(sys_update_dynamic_lights_bind_group);

        // Override Material
        self.run_system_cached(sys_update_override_pbr_material_bind_group);
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let window = self.window.clone();
        let world = &mut self.world;

        let mut ctx = world.resource_scope(|_world, render_state: Mut<RenderState>| {
            let output = render_state.surface.get_current_texture()?;
            let output_view = output.texture.create_view(&Default::default());
            let encoder =
                render_state
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("Render Encoder"),
                    });

            let ctx = PassRenderContext {
                encoder,
                output_view,
                output_texture: output,
                window: Arc::clone(&window),
                stage: RenderStage::BeforeOpaque,
            };
            Ok(ctx)
        })?;

        // PASS: Shadow Mapping -----
        world
            .run_system_cached_with(render::systems::sys_render_shadow_mapping_pass, &mut ctx)
            .unwrap();
        // --------------------------

        ctx.stage = RenderStage::BeforeOpaque;
        world
            .run_system_cached_with(render::systems::sys_render_post_processing, &mut ctx)
            .unwrap();

        // PASS: Main ---------------
        world
            .run_system_cached_with(render::systems::sys_render_write_g_buffer_pass, &mut ctx)
            .unwrap();
        world
            .run_system_cached_with(render::systems::sys_render_main_pass, &mut ctx)
            .unwrap();
        // -------------------------

        ctx.stage = RenderStage::AfterOpaque;
        world
            .run_system_cached_with(render::systems::sys_render_post_processing, &mut ctx)
            .unwrap();

        ctx.stage = RenderStage::BeforeTransparent;
        world
            .run_system_cached_with(render::systems::sys_render_post_processing, &mut ctx)
            .unwrap();

        world
            .run_system_cached_with(render::systems::sys_render_transparent, &mut ctx)
            .unwrap();

        ctx.stage = RenderStage::AfterTransparent;
        world
            .run_system_cached_with(render::systems::sys_render_post_processing, &mut ctx)
            .unwrap();

        // Gizmos ---------------------
        world
            .run_system_cached_with(render::systems::sys_render_gizmos, &mut ctx)
            .unwrap();

        // PASS: Render Egui ----------
        world
            .run_system_cached_with(render::systems::sys_render_egui, &mut ctx)
            .unwrap();

        // End Draw Objects ------------
        world
            .resource::<RenderState>()
            .queue
            .submit(std::iter::once(ctx.encoder.finish()));
        ctx.output_texture.present();

        Ok(())
    }
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
        .run_system_once_with(skybox_image_path.clone(), sys_load_hdir_and_prefiler)
        .unwrap();
    world
        .run_system_once_with(
            skybox_image_path,
            render::skybox::sys_update_skybox_sh_from_path,
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
