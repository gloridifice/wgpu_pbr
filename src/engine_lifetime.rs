use std::fs;
use std::sync::Arc;

use crate::cgmath_ext::{Vec3, Vec4, VectorExt};
use crate::editor::{self, sys_egui_tiles, RenderTargetEguiTexId};
use crate::egui_tools::{EguiConfig, EguiRenderer};
use crate::render::camera::{
    sys_update_camera_control, sys_update_camera_uniform, Camera, CameraController,
};
use crate::render::cubemap::{CubemapConverterRgba8unorm, CubemapMatrixBindGroups};
use crate::render::defered_rendering::global_binding::GlobalUniformBuffer;
use crate::render::defered_rendering::write_g_buffer_pipeline::{
    GBufferTexturesBindGroup, WriteGBufferPipeline,
};
use crate::render::defered_rendering::{global_binding::GlobalBindGroup, MainPipeline};
use crate::render::dfg::DFGTexture;
use crate::render::gizmos::{Gizmos, GizmosGlobalBindGroup, GizmosMaterial, GizmosPipeline};
use crate::render::light::parallel_light::ParallelLight;
use crate::render::light::point_light::PointLight;
use crate::render::light::{
    event_on_remove_point_light, sys_update_dynamic_lights, sys_update_dynamic_lights_bind_group,
    DynamicLightBindGroup, DynamicLights,
};
use crate::render::material::buffer_material::BufferMaterialManager;
use crate::render::material::pbr::{
    sys_update_override_pbr_material_bind_group, PBRMaterial, PBRMaterialBindGroupLayout,
};
use crate::render::mipmap::DefaultMipmapGenShader;
use crate::render::post_processing::{PostProcessingManager, RenderStage};
use crate::render::shader_loader::ShaderLoader;
use crate::render::shadow_mapping::{CastShadow, ShadowMapGlobalBindGroup, ShadowMappingPipeline};
use crate::render::skybox::prefiltering::PrefilteringPipeline;
use crate::render::skybox::{DefaultSkybox, SkyboxPipeline, SkyboxSHBuffer};
use crate::render::systems::{sys_refersh_global_bind_group, PassRenderContext};
use crate::render::transform::WorldTransform;
use crate::render::transparent::TransparentPipeline;
use crate::render::{
    prelude::*, AlphaMode, ColorRenderTarget, DefaultPBRMaterial, DepthRenderTarget,
    FullScreenVertexShader, MainPassObject, MissingTexture, NormalDefaultTexture,
    ObjectBindGroupLayout, RenderTargetSize, UploadedImageWithSampler, WhiteTexture,
};
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
use cgmath::{Deg, Quaternion, Rad, Rotation3};
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
    pub fn insert_resource<R>(&mut self)
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
        self.insert_resource::<ShaderLoader>();
        self.insert_resource::<WhiteTexture>();
        self.insert_resource::<NormalDefaultTexture>();
        self.insert_resource::<DFGTexture>();
        self.insert_resource::<DefaultMipmapGenShader>();
        self.insert_resource::<MissingTexture>();
        self.insert_resource::<BufferMaterialManager>();
        self.insert_resource::<RenderTargetSize>();
        self.insert_resource::<ColorRenderTarget>();
        self.insert_resource::<DepthRenderTarget>();
        self.insert_resource::<RenderTargetEguiTexId>();
        self.insert_resource::<render::utils::cube::CubeVerticesBuffer>();
        self.insert_resource::<render::cubemap::CubemapVertexShader>();
        self.insert_resource::<CubemapMatrixBindGroups>();
        self.insert_resource::<CubemapConverterRgba8unorm>();
        self.insert_resource::<PrefilteringPipeline>();
        self.insert_resource::<DefaultSkybox>();
        self.insert_resource::<GlobalUniformBuffer>();

        // --- Render resource ---
        self.insert_resource::<CameraBuffer>();
        self.insert_resource::<SkyboxSHBuffer>();
        self.world
            .insert_resource(LightUnifromBuffer::new(&self.render_state().device));
        self.insert_resource::<ShadowMap>();
        // self.insert_resource::<ShadowMapEguiTextureId>();

        self.insert_resource::<FullScreenVertexShader>();

        // 0. Layouts
        self.insert_resource::<ObjectBindGroupLayout>();
        self.insert_resource::<GizmosGlobalBindGroup>();
        self.insert_resource::<PBRMaterialBindGroupLayout>();

        // 1. Globals
        self.insert_resource::<ShadowMapGlobalBindGroup>();
        self.insert_resource::<DynamicLightBindGroup>();

        // 1.5
        self.insert_resource::<GBufferTexturesBindGroup>();
        self.insert_resource::<GlobalBindGroup>();

        // 2. Pipelines
        self.insert_resource::<WriteGBufferPipeline>();
        self.insert_resource::<SkyboxPipeline>();
        self.insert_resource::<MainPipeline>();
        self.insert_resource::<TransparentPipeline>();
        self.insert_resource::<ShadowMappingPipeline>();
        self.insert_resource::<GizmosPipeline>();

        // Post Processing
        self.insert_resource::<PostProcessingManager>();

        // --- Other resources ---
        self.insert_resource::<Input>();
        self.insert_resource::<ControlState>();
        self.insert_resource::<DynamicLights>();
        self.world.insert_resource(Time::default());
        self.world.insert_resource(EguiConfig::default());
        self.world.insert_resource(CameraConfig::default());
        self.insert_resource::<DefaultPBRMaterial>();

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
            .run_system_once(sys_generate_bistro_scene)
            .unwrap();
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

fn sys_generate_dragons_scene(world: &mut World) {
    let mut vec = Vec::with_capacity(20usize);
    for _ in 0..10 {
        let x = rand::random::<f32>() * 12.;
        let y = rand::random::<f32>() * 2.;
        let z = rand::random::<f32>() * 2.;
        let r = rand::random::<f32>();
        let a = rand::random::<f32>();
        let g = (1. - r) * a;
        let b = (1. - r) - g;
        vec.push((
            PointLight {
                color: Vec4::new(r, g, b, 1.),
                ..Default::default()
            },
            Transform::with_position(Vec3::new(x, y, z)),
            Name("点光源".to_string()),
        ))
    }
    vec.into_iter().for_each(|it| {
        world.spawn(it);
    });

    let dragon_model =
        Arc::new(Model::load(AssetPath::new("models/DragonAttenuation.glb"), world).unwrap());
    let plane_model = Arc::new(Model::load(AssetPath::new("models/plane.glb"), world).unwrap());

    let mut queue = CommandQueue::from_world(world);
    let mut commands = Commands::new(&mut queue, world);
    let count = 5;
    for i in 0..count {
        commands.queue(SpawnModelCmd {
            model: dragon_model.clone(),
            parent_bundle: (
                TransformBuilder::default()
                    .position(Vec3::new(i as f32 * 2., 0., 0.))
                    .rotation(Quaternion::from_angle_x(Deg(90.0)))
                    .scale(Vec3::new_unit(0.3))
                    .build()
                    .unwrap(),
                RotationObject { speed: 0.5 },
                Name(format!("龙模型 No_{}", i)),
            ),
            child_bundle: (
                CastShadow,
                MainPassObject,
                PBRMaterial {
                    metallic: Some((i as f32) / (count - 1) as f32),
                    ..Default::default()
                },
            ),
        });
    }

    for i in 0..count {
        commands.queue(SpawnModelCmd {
            model: dragon_model.clone(),
            parent_bundle: (
                TransformBuilder::default()
                    .position(Vec3::new(i as f32 * 2., 0., 6.))
                    .rotation(Quaternion::from_angle_x(Deg(90.0)))
                    .scale(Vec3::new_unit(0.3))
                    .build()
                    .unwrap(),
                RotationObject { speed: 0.5 },
                Name(format!("透明龙模型 No_{}", i)),
            ),
            child_bundle: (
                PBRMaterial {
                    color: Some(Vec4::new(1.0, 1.0, 1.0, i as f32 / count as f32)),
                    alpha_mode: Some(AlphaMode::Blend),
                    ..Default::default()
                },
                MainPassObject,
            ),
        });
    }

    commands.queue(SpawnModelCmd {
        model: plane_model.clone(),
        parent_bundle: (
            TransformBuilder::default()
                .position(Vec3::new_y(-1.0))
                .build()
                .unwrap(),
            Name("平面".to_string()),
        ),
        child_bundle: (
            CastShadow,
            MainPassObject,
            PBRMaterial {
                ..Default::default()
            },
        ),
    });

    queue.apply(world);
}

fn sys_generate_bistro_scene(world: &mut World) {
    let bistro_model =
        Arc::new(Model::load(AssetPath::new("models/Bistro/Bistro.gltf"), world).unwrap());

    let mut queue = CommandQueue::from_world(world);
    let mut commands = Commands::new(&mut queue, world);

    commands.queue(SpawnModelCmd {
        model: bistro_model,
        parent_bundle: (
            TransformBuilder::default()
                .scale(Vec3::one() * 0.03)
                .build()
                .unwrap(),
            Name(format!("Bistro")),
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
        Camera::new(aspect),
        CameraController::default(),
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

    let hdri = UploadedImageWithSampler::load(
        AssetPath::Assets("textures/hdr/qwantani_afternoon_2k.png".to_string()),
        world,
    )
    .unwrap();

    let &RenderState {
        ref device,
        ref queue,
        ..
    } = &world.resource::<RenderState>();

    let cubemap_texture = {
        let converter = world.resource::<CubemapConverterRgba8unorm>();
        converter.0.render_hdir_to_cube_map(
            &device,
            &queue,
            &hdri.view,
            &world
                .resource::<crate::render::utils::cube::CubeVerticesBuffer>()
                .vertices_buffer,
            512,
        )
    };
    let _cubemap_view = cubemap_texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::Cube),
        ..Default::default()
    });

    // self.world.spawn(Skybox {
    //     texture: Some(render::UploadedImage {
    //         texture: cubemap_texture,
    //         view: cubemap_view,
    //     }),
    // });
}
