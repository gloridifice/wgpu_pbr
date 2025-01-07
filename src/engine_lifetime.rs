use std::sync::Arc;

use crate::editor::{self, sys_egui_tiles, RenderTargetEguiTexId};
use crate::egui_tools::{EguiConfig, EguiRenderer};
use crate::math_type::{Quat, Vec3, Vec4};
use crate::render::camera::{Camera, CameraController};
use crate::render::defered_rendering::write_g_buffer_pipeline::{
    GBufferTexturesBindGroup, WriteGBufferPipeline,
};
use crate::render::defered_rendering::{MainGlobalBindGroup, MainPipeline};
use crate::render::light::{
    event_on_remove_point_light, sys_update_dynamic_lights, sys_update_dynamic_lights_bind_group,
    DynamicLightBindGroup, DynamicLights, PointLight,
};
use crate::render::post_processing::{PostProcessingManager, RenderStage};
use crate::render::shadow_mapping::{CastShadow, ShadowMapGlobalBindGroup, ShadowMappingPipeline};
use crate::render::systems::PassRenderContext;
use crate::render::transform::WorldTransform;
use crate::render::{
    ColorRenderTarget, DefaultMainPipelineMaterial, DepthRenderTarget, FullScreenVertexShader,
    GBufferGlobalBindGroup, MaterialBindGroupLayout, ObjectBindGroupLayout, RenderTargetSize,
};
use crate::MainWindow;
use crate::{
    asset::{load::Loadable, AssetPath},
    engine::input::Input,
    engine::time::Time,
    render::{
        self,
        camera::{CameraConfig, RenderCamera},
        light::{LightUnifromBuffer, ParallelLight},
        shadow_mapping::ShadowMap,
        transform::{Transform, TransformBuilder},
        MeshRenderer,
    },
    RenderState, State,
};
use bevy_ecs::query::{Changed, Or, With};
use bevy_ecs::system::{Commands, ResMut, Resource, Single};
use bevy_ecs::world::{CommandQueue, FromWorld, Mut, World};
use bevy_ecs::{
    component::Component,
    system::{Query, Res, RunSystemOnce},
};
use cgmath::{vec2, Deg, InnerSpace, Quaternion, Rad, Rotation, Rotation3, Vector3};
use egui::Visuals;
use winit::{event::WindowEvent, keyboard::KeyCode};

#[derive(Debug, Component)]
pub struct RotationObject {
    pub speed: f32,
}

impl State {
    pub fn insert_resource<R>(&mut self)
    where
        R: Resource + FromWorld,
    {
        let r = R::from_world(&mut self.world);
        self.world.insert_resource(r);
    }

    pub fn init(&mut self) {
        self.insert_resource::<RenderTargetSize>();
        self.insert_resource::<ColorRenderTarget>();
        self.insert_resource::<DepthRenderTarget>();
        self.insert_resource::<RenderTargetEguiTexId>();

        // --- Render resource ---
        self.insert_resource::<RenderCamera>();
        self.world
            .insert_resource(LightUnifromBuffer::new(&self.render_state().device));
        self.insert_resource::<ShadowMap>();
        // self.insert_resource::<ShadowMapEguiTextureId>();

        self.insert_resource::<FullScreenVertexShader>();

        // 0. Layouts
        self.insert_resource::<ObjectBindGroupLayout>();
        self.insert_resource::<MaterialBindGroupLayout>();

        // 1. Globals
        self.insert_resource::<GBufferGlobalBindGroup>();
        self.insert_resource::<ShadowMapGlobalBindGroup>();
        self.insert_resource::<DynamicLightBindGroup>();

        // 1.5
        self.insert_resource::<GBufferTexturesBindGroup>();
        self.insert_resource::<MainGlobalBindGroup>();

        // 2. Pipelines
        self.insert_resource::<WriteGBufferPipeline>();
        self.insert_resource::<MainPipeline>();
        self.insert_resource::<ShadowMappingPipeline>();

        // Post Processing
        self.insert_resource::<PostProcessingManager>();

        // --- Other resources ---
        self.insert_resource::<Input>();
        self.insert_resource::<ControlState>();
        self.insert_resource::<DynamicLights>();
        self.world.insert_resource(Time::default());
        self.world.insert_resource(EguiConfig::default());
        self.world.insert_resource(CameraConfig::default());
        self.insert_resource::<DefaultMainPipelineMaterial>();

        // Add Events'Observers
        self.world.add_observer(event_on_remove_point_light);

        self.world
            .run_system_once(sys_insert_post_processing_pipelines)
            .unwrap();

        {
            // Set egui visual / style / theme
            let egui = self.world.resource_mut::<EguiRenderer>();
            let mut visual = Visuals::dark();
            visual.widgets.noninteractive.bg_stroke.width = 0.0;
            egui.context().set_visuals(visual);
        }

        self.world.spawn((
            PointLight {
                color: Vec4::new(1., 0., 0., 1.),
                intensity: 10.0,
                distance: 2.0,
                decay: 1.0,
            },
            Transform::with_position(Vec3::new(0., 4., 0.)),
        ));

        self.world.spawn((
            PointLight {
                color: Vec4::new(0., 1., 0., 1.),
                intensity: 2.0,
                distance: 2.0,
                decay: 2.0,
            },
            Transform::with_position(Vec3::new(0., 0., 2.)),
        ));

        let ship_model = render::Model::load(
            AssetPath::Assets("models/ship.glb".to_string()),
            &mut self.world,
        )
        .unwrap();
        let light_arrow = render::Model::load(
            AssetPath::Assets("models/arrow.glb".to_string()),
            &mut self.world,
        )
        .unwrap();

        let mut queue = CommandQueue::from_world(&mut self.world);
        let mut cmd = Commands::new(&mut queue, &self.world);
        let rs = &self.world.resource::<RenderState>().config;
        let aspect = rs.width as f32 / rs.height as f32;

        cmd.spawn((Camera::new(aspect), CameraController::default()));

        let main_light_id = cmd
            .spawn((
                Transform::with_position(Vec3::new(0., 0., 3.)),
                ParallelLight::default(),
            ))
            .id();
        for mesh in light_arrow.meshes {
            let uploaded = Arc::new(mesh.upload(&self));
            cmd.spawn((
                TransformBuilder::default()
                    .parent(Some(main_light_id))
                    .build()
                    .unwrap(),
                MeshRenderer::new(uploaded, &self.world),
            ));
        }

        let parent = cmd
            .spawn((
                TransformBuilder::default()
                    // .rotation(Quaternion::from_angle_x(Deg(-90.0)))
                    .build()
                    .unwrap(),
                RotationObject { speed: 0.5 },
            ))
            .id();

        for mesh in ship_model.meshes {
            let uploaded = Arc::new(mesh.upload(self));

            cmd.spawn((
                MeshRenderer::new(uploaded, &self.world),
                TransformBuilder::default()
                    .parent(Some(parent))
                    .build()
                    .unwrap(),
                CastShadow,
            ));
        }

        queue.apply(&mut self.world);
    }

    pub fn input(&mut self, event: &WindowEvent) -> bool {
        self.world.resource_mut::<Input>().input(event);
        false
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
        self.world.run_system_once(sys_update_camera).unwrap();
        self.world.run_system_once(sys_update_rotation).unwrap();
        self.world
            .run_system_cached(Self::sys_print_normal_trans)
            .unwrap();
    }

    fn sys_print_normal_trans(
        q_parent: Query<(&WorldTransform), With<RotationObject>>,
        input: Res<Input>,
    ) {
        if input.is_key_down(KeyCode::KeyO) {
            for (world_trans) in q_parent.iter() {
                println!("{:?}", world_trans.normal_matrix());
            }
        }
    }

    pub fn post_update(&mut self) {
        // Update transform unifrom
        self.run_system_once(render::transform::sys_update_world_transform);
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

        ctx.stage = RenderStage::AfterTransparent;
        world
            .run_system_cached_with(render::systems::sys_render_post_processing, &mut ctx)
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

pub fn sys_update_camera(
    config: Res<CameraConfig>,
    input: Res<Input>,
    time: Res<Time>,
    main_window: Res<MainWindow>,
    mut control_state: ResMut<ControlState>,
    camera_query: Single<(
        &Camera,
        &mut Transform,
        &WorldTransform,
        &mut CameraController,
    )>,
) {
    if input.is_key_down(KeyCode::Escape) {
        control_state.is_focused = !control_state.is_focused;
        main_window.0.set_cursor_visible(!control_state.is_focused);
    }
    if !control_state.is_focused {
        return;
    }

    let (_, mut cam_transform, world_trans, mut controller) = camera_query.into_inner();

    let speed = config.speed;

    let mut move_vec = Vector3::new(0., 0., 0.);
    if input.is_key_hold(KeyCode::KeyW) {
        move_vec += world_trans.forward();
    }
    if input.is_key_hold(KeyCode::KeyA) {
        move_vec += world_trans.left();
    }
    if input.is_key_hold(KeyCode::KeyS) {
        move_vec -= world_trans.forward();
    }
    if input.is_key_hold(KeyCode::KeyD) {
        move_vec -= world_trans.left();
    }
    if input.is_key_hold(KeyCode::Space) {
        if input.is_key_hold(KeyCode::ShiftLeft) {
            move_vec += Vector3::new(0.0, -1.0, 0.0);
        } else {
            move_vec += Vector3::new(0.0, 1.0, 1.0);
        }
    }
    let delta_time_sec = time.delta_time.as_secs_f32();
    if move_vec != Vector3::new(0., 0., 0.) {
        move_vec = move_vec.normalize() * speed * delta_time_sec;
        cam_transform.position += move_vec;
    }

    let factor = vec2(0.6, 0.4);
    controller.row -= input.cursor_offset.x * factor.x;
    controller.yaw = (controller.yaw - input.cursor_offset.y * factor.y).clamp(-40.0, 80.0);
    cam_transform.rotation = Quaternion::from_angle_y(Deg(controller.row))
        * Quaternion::from_angle_x(Deg(controller.yaw));
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

fn sys_update_camera_uniform(
    render_camera: Res<RenderCamera>,
    single: Single<(&Camera, &WorldTransform), Or<(Changed<Camera>, Changed<WorldTransform>)>>,
    rs: Res<RenderState>,
) {
    let (camera, transform) = single.into_inner();
    render_camera.update_uniform2gpu(camera, transform, &rs.queue);
}

fn sys_insert_post_processing_pipelines(
    rs: Res<RenderState>,
    mut manager: ResMut<PostProcessingManager>,
) {
    return;
    let fs_shader = rs
        .device
        .create_shader_module(wgpu::include_wgsl!("../assets/shaders/post_test.wgsl"));
    manager.add_pipeline_from_shader(
        None,
        render::post_processing::RenderStage::AfterTransparent,
        fs_shader,
        &rs.device,
        &rs.config,
    );
}
