use std::sync::Arc;

use crate::egui_tools::{self, EguiConfig, EguiRenderer};
use crate::math_type::Vec3;
use crate::render::camera::{Camera, CameraController};
use crate::render::material_impl::MainPipeline;
use crate::render::shadow_mapping::{CastShadow, ShadowMapGlobalBindGroup, ShadowMappingPipeline};
use crate::render::transform::{self, WorldTransform};
use crate::render::{
    DefaultMainPipelineMaterial, GlobalBindGroup, MaterialBindGroupLayout, ObjectBindGroupLayout,
};
use crate::{
    asset::{load::Loadable, AssetPath},
    engine::input::Input,
    engine::time::Time,
    render::{
        self,
        camera::{CameraConfig, RenderCamera},
        light::{MainLight, RenderLight},
        shadow_mapping::ShadowMap,
        transform::{Transform, TransformBuilder},
        DrawAble, DrawContext, MeshRenderer,
    },
    RenderState, State,
};
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::query::{Changed, Or, With};
use bevy_ecs::system::{Commands, Resource, Single};
use bevy_ecs::world::{CommandQueue, FromWorld, Mut, World};
use bevy_ecs::{
    component::Component,
    system::{Query, Res, ResMut, RunSystemOnce},
};
use cgmath::{vec2, Deg, InnerSpace, Quaternion, Rad, Rotation3, Vector3};
use egui::epaint::QuadraticBezierShape;
use egui_wgpu::ScreenDescriptor;
use winit::{event::WindowEvent, keyboard::KeyCode};

#[derive(Debug, Component)]
pub struct Rotation {
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
        // --- Render resource ---
        self.insert_resource::<RenderCamera>();
        self.world
            .insert_resource(RenderLight::new(&self.render_state().device));
        self.insert_resource::<ShadowMap>();
        // self.insert_resource::<ShadowMapEguiTextureId>();

        // 0. Layouts
        self.insert_resource::<ObjectBindGroupLayout>();
        self.insert_resource::<MaterialBindGroupLayout>();

        // 1. Globals
        self.insert_resource::<GlobalBindGroup>();
        self.insert_resource::<ShadowMapGlobalBindGroup>();

        // 2. Pipelines
        self.insert_resource::<MainPipeline>();
        self.insert_resource::<ShadowMappingPipeline>();

        // --- Other resources ---
        self.insert_resource::<Input>();
        self.world.insert_resource(Time::default());
        self.world.insert_resource(EguiConfig::default());
        self.world.insert_resource(CameraConfig::default());
        self.insert_resource::<DefaultMainPipelineMaterial>();

        let ship_model = render::Model::load(
            AssetPath::Assets("models/test_scene.glb".to_string()),
            &mut self.world,
        )
        .unwrap();
        let light_bulb = render::Model::load(
            AssetPath::Assets("models/monkey.glb".to_string()),
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
                MainLight::default(),
            ))
            .id();
        for mesh in light_bulb.meshes {
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
            .spawn(
                TransformBuilder::default()
                    .rotation(Quaternion::from_angle_x(Deg(-90.0)))
                    .build()
                    .unwrap(),
            )
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
        self.world.resource_mut::<Input>().update(event);
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
    }

    pub fn update(&mut self) {
        self.world.run_system_once(CameraConfig::sys_panel).unwrap();
        self.world.run_system_once(sys_update_camera).unwrap();
        self.world.run_system_once(sys_update_rotation).unwrap();

        self.world.run_system_once(sys_light_panel).unwrap();
        self.world.run_system_once(sys_input_panel).unwrap();
    }

    pub fn post_update(&mut self) {
        // Update transform unifrom
        self.world
            .run_system_once(render::transform::sys_update_world_transform)
            .unwrap();
        self.world
            .run_system_once(sys_update_transform_buffers)
            .unwrap();

        // Update camera uniform
        self.world
            .run_system_cached(sys_update_camera_uniform)
            .unwrap();

        // Update light uniform
        {
            let (transform, main_light) = self
                .world
                .query::<(&WorldTransform, &MainLight)>()
                .single(&self.world);
            let uniform = main_light.get_uniform(transform);

            if self.world.is_resource_changed::<RenderLight>() {
                self.world
                    .resource::<RenderLight>()
                    .write_buffer(&self.render_state().queue, uniform);
                // self.world.resource::<>()
            }
        }
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.render_state().surface.get_current_texture()?;
        let view = Arc::new(
            output
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default()),
        );
        let mut encoder =
            self.render_state()
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });

        // Shadow Mapping light depth map
        {
            let shadow_map = self.world.resource::<ShadowMap>();
            let shadow_mapping_pipeline = self.world.resource::<ShadowMappingPipeline>();
            let sm_global_bg = self.world.resource::<ShadowMapGlobalBindGroup>();

            // let render_light = self.world.resource::<RenderLight>();
            let mut shadow_map_render_pass =
                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Shadow Mapping Light Depth Render Pass"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        view: &shadow_map.image.view,
                        stencil_ops: None,
                    }),
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

            shadow_map_render_pass.set_pipeline(&shadow_mapping_pipeline.pipeline);
            shadow_map_render_pass.set_bind_group(0, &sm_global_bg.bind_group, &[]);
            for mesh_renderer in self
                .world
                .query_filtered::<&MeshRenderer, With<CastShadow>>()
                .iter(&self.world)
            {
                mesh_renderer.draw_depth(&mut shadow_map_render_pass);
            }
        }
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    view: &self.depth_texture.view,
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            let main_pipeline = self.world.resource::<MainPipeline>();
            let global_bind_group = &self.world.resource::<GlobalBindGroup>().bind_group;
            render_pass.set_pipeline(&main_pipeline.pipeline);
            render_pass.set_bind_group(0, global_bind_group, &[]);

            for mesh_renderer in self.world.query::<&MeshRenderer>().iter(&self.world) {
                let mut ctx = DrawContext {
                    render_pass: &mut render_pass,
                    world: &self.world,
                };
                mesh_renderer.draw_main(&mut ctx);
            }
        }

        let world = &mut self.world;
        let window = self.window.clone();

        world.resource_scope(|world, mut egui_renderer: Mut<EguiRenderer>| {
            let render_state = world.resource::<RenderState>();
            let egui_config = world.resource::<EguiConfig>();

            let screen_descriptor = ScreenDescriptor {
                size_in_pixels: [render_state.config.width, render_state.config.height],
                pixels_per_point: window.scale_factor() as f32 * egui_config.egui_scale_factor,
            };
            egui_renderer.end_frame_and_draw(
                &render_state.device,
                &render_state.queue,
                &mut encoder,
                &window,
                &view,
                screen_descriptor,
            );
        });
        // End Draw Objects

        self.render_state()
            .queue
            .submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

pub fn sys_update_rotation(mut q: Query<(&mut Transform, &Rotation)>, time: Res<Time>) {
    for (mut trans, rot) in q.iter_mut() {
        trans.rotation = Quaternion::from_angle_y(Rad(rot.speed) * time.delta_time.as_secs_f32())
            * trans.rotation;
    }
}

pub fn sys_update_camera(
    config: Res<CameraConfig>,
    input: Res<Input>,
    time: Res<Time>,
    camera_query: Single<(&mut Camera, &mut Transform, &mut CameraController)>,
) {
    let (mut camera, mut cam_transform, mut controller) = camera_query.into_inner();

    let speed = config.speed;

    let mut move_vec = Vector3::new(0., 0., 0.);
    if input.is_key_hold(KeyCode::KeyW) {
        move_vec += Vector3::new(0.0, 0.0, -1.0);
    }
    if input.is_key_hold(KeyCode::KeyA) {
        move_vec += Vector3::new(-1.0, 0.0, 0.0);
    }
    if input.is_key_hold(KeyCode::KeyS) {
        move_vec += Vector3::new(0.0, 0.0, 1.0);
    }
    if input.is_key_hold(KeyCode::KeyD) {
        move_vec += Vector3::new(1.0, 0.0, 0.0);
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

fn sys_light_panel(light: Single<(&MainLight, &mut Transform)>, egui: Res<EguiRenderer>) {
    let (_, mut trans) = light.into_inner();
    let ctx = egui.context();
    egui::Window::new("Light").show(ctx, |ui| {
        egui_tools::transform_ui(ui, &mut trans);
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

fn sys_input_panel(input: Res<Input>, egui: Res<EguiRenderer>) {
    let ctx = egui.context();
    egui::Window::new("Input").show(ctx, |ui| {
        ui.label(format!("Offset: {:?}", input.cursor_offset));
        ui.label(format!("Position: {:?}", input.cursor_position));
    });
}
