use std::sync::Arc;

use crate::editor::{self, sys_egui_tiles, RenderTargetEguiTexId};
use crate::egui_tools::{self, EguiConfig, EguiRenderer};
use crate::math_type::{Mat4, Vec3};
use crate::render::camera::{Camera, CameraController};
use crate::render::material_impl::MainPipeline;
use crate::render::shadow_mapping::{
    CastShadow, LightMatrixBuffer, ShadowMapGlobalBindGroup, ShadowMappingPipeline,
};
use crate::render::transform::WorldTransform;
use crate::render::{
    ColorRenderTarget, DefaultMainPipelineMaterial, DepthRenderTarget, GlobalBindGroup,
    MaterialBindGroupLayout, ObjectBindGroupLayout, RenderTargetSize,
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
use bevy_ecs::event::signal_event_update_system;
use bevy_ecs::query::{Changed, Or, With};
use bevy_ecs::system::{Commands, ResMut, Resource, Single};
use bevy_ecs::world::{CommandQueue, FromWorld, Mut, World};
use bevy_ecs::{
    component::Component,
    system::{Query, Res, RunSystemOnce},
};
use cgmath::{vec2, Deg, InnerSpace, Quaternion, Rad, Rotation, Rotation3, SquareMatrix, Vector3};
use egui_wgpu::ScreenDescriptor;
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
            .insert_resource(RenderLight::new(&self.render_state().device));
        self.insert_resource::<LightMatrixBuffer>();
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
        self.insert_resource::<ControlState>();
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

        // self.world.run_system_once(CameraConfig::sys_panel).unwrap();
        // self.world.run_system_once(sys_light_panel).unwrap();
        // self.world.run_system_once(sys_input_panel).unwrap();
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
        self.world
            .run_system_cached(sys_update_light_uniform)
            .unwrap();

        // Clear Down an Up maps
        self.world
            .run_system_cached(Input::sys_post_update)
            .unwrap();
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let window = self.window.clone();
        self.world
            .resource_scope(|world, depth_target: Mut<DepthRenderTarget>| {
                let depth_view = &depth_target.0.as_ref().unwrap().view;
                world.resource_scope(|world, target: Mut<ColorRenderTarget>| {
                    let output = world
                        .resource::<RenderState>()
                        .surface
                        .get_current_texture()?;
                    let output_view = output.texture.create_view(&Default::default());
                    let view = &target.0.as_ref().unwrap().view;
                    let mut encoder = world
                        .resource::<RenderState>()
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Render Encoder"),
                        });

                    // Shadow Mapping light depth map
                    {
                        let shadow_map = world.resource::<ShadowMap>();
                        let shadow_mapping_pipeline = world.resource::<ShadowMappingPipeline>();
                        let sm_global_bg = world.resource::<ShadowMapGlobalBindGroup>();

                        // let render_light = world.resource::<RenderLight>();
                        let mut shadow_map_render_pass =
                            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("Shadow Mapping Light Depth Render Pass"),
                                color_attachments: &[],
                                depth_stencil_attachment: Some(
                                    wgpu::RenderPassDepthStencilAttachment {
                                        depth_ops: Some(wgpu::Operations {
                                            load: wgpu::LoadOp::Clear(1.0),
                                            store: wgpu::StoreOp::Store,
                                        }),
                                        view: &shadow_map.image.view,
                                        stencil_ops: None,
                                    },
                                ),
                                occlusion_query_set: None,
                                timestamp_writes: None,
                            });

                        shadow_map_render_pass.set_pipeline(&shadow_mapping_pipeline.pipeline);
                        shadow_map_render_pass.set_bind_group(0, &sm_global_bg.bind_group, &[]);
                        for mesh_renderer in world
                            .query_filtered::<&MeshRenderer, With<CastShadow>>()
                            .iter(&world)
                        {
                            mesh_renderer.draw_depth(&mut shadow_map_render_pass);
                        }
                    }
                    {
                        let mut render_pass =
                            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
                                depth_stencil_attachment: Some(
                                    wgpu::RenderPassDepthStencilAttachment {
                                        depth_ops: Some(wgpu::Operations {
                                            load: wgpu::LoadOp::Clear(1.0),
                                            store: wgpu::StoreOp::Store,
                                        }),
                                        view: depth_view,
                                        stencil_ops: None,
                                    },
                                ),
                                occlusion_query_set: None,
                                timestamp_writes: None,
                            });

                        let main_pipeline = world.resource::<MainPipeline>();
                        let global_bind_group = &world.resource::<GlobalBindGroup>().bind_group;
                        render_pass.set_pipeline(&main_pipeline.pipeline);
                        render_pass.set_bind_group(0, global_bind_group, &[]);

                        for mesh_renderer in world.query::<&MeshRenderer>().iter(&world) {
                            let mut ctx = DrawContext {
                                render_pass: &mut render_pass,
                                world: &world,
                            };
                            mesh_renderer.draw_main(&mut ctx);
                        }
                    }

                    world.resource_scope(|world, mut egui_renderer: Mut<EguiRenderer>| {
                        let render_state = world.resource::<RenderState>();
                        let egui_config = world.resource::<EguiConfig>();

                        let screen_descriptor = ScreenDescriptor {
                            size_in_pixels: [render_state.config.width, render_state.config.height],
                            pixels_per_point: window.scale_factor() as f32
                                * egui_config.egui_scale_factor,
                        };

                        egui_renderer.end_frame_and_draw(
                            &render_state.device,
                            &render_state.queue,
                            &mut encoder,
                            &window,
                            &output_view,
                            screen_descriptor,
                        );
                    });
                    // End Draw Objects

                    world
                        .resource::<RenderState>()
                        .queue
                        .submit(std::iter::once(encoder.finish()));

                    output.present();

                    Ok(())
                })
            })
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
    mut control_state: ResMut<ControlState>,
    camera_query: Single<(&Camera, &mut Transform, &mut CameraController)>,
) {
    if input.is_key_down(KeyCode::Escape)  {
        control_state.is_focused = !control_state.is_focused;
    }
    if !control_state.is_focused {
        return;
    }

    let (_, mut cam_transform, mut controller) = camera_query.into_inner();

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
        move_vec =
            cam_transform.rotation.rotate_vector(move_vec.normalize()) * speed * delta_time_sec;
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

fn sys_update_light_uniform(
    single: Single<(&WorldTransform, &MainLight)>,
    render_light: Res<RenderLight>,
    rs: Res<RenderState>,
    light_matrix: Res<LightMatrixBuffer>,
) {
    let (transform, main_light) = single.into_inner();
    let uniform = main_light.get_uniform(transform);

    render_light.write_buffer(&rs.queue, uniform);
    // let cast: [[f32; 4];4] = Mat4::from(uniform.space_matrix).invert().unwrap().into();
    rs.queue.write_buffer(
        &light_matrix.buffer,
        0,
        bytemuck::cast_slice(&[uniform.space_matrix]),
    );
}

fn sys_input_panel(input: Res<Input>, egui: Res<EguiRenderer>) {
    let ctx = egui.context();
    egui::Window::new("Input").show(ctx, |ui| {
        ui.label(format!("Offset: {:?}", input.cursor_offset));
        ui.label(format!("Position: {:?}", input.cursor_position));
    });
}
