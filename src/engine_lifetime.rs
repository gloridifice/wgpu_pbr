use std::sync::Arc;

use bevy_ecs::{
    change_detection::DetectChanges,
    component::Component,
    entity::Entity,
    system::{Query, Res, ResMut, RunSystemOnce},
};
use cgmath::{InnerSpace, Point3, Quaternion, Rad, Rotation3, Vector3};
use egui_wgpu::ScreenDescriptor;
use winit::{event::WindowEvent, keyboard::KeyCode};

use crate::{
    asset::{load::Loadable, AssetPath},
    input::Input,
    render::{
        self,
        camera::{CameraConfig, RenderCamera},
        light::RenderLight,
        transform::{Transform, TransformBindGroupLayout, TransformBuilder},
        DrawAble, DrawContext, MeshRenderer,
    },
    time::Time,
    State,
};

#[derive(Debug, Component)]
pub struct Rotation {
    pub speed: f32,
}

impl State {
    pub fn init(&mut self) {
        // init resource
        self.world.insert_resource(Time::default());
        self.world.insert_resource(Input::default());
        self.world.insert_resource(CameraConfig::default());
        self.world
            .insert_resource(TransformBindGroupLayout::new(&self.render_state.device));
        {
            let config = &self.render_state.config;
            let aspect = config.width as f32 / config.height as f32;
            self.world
                .insert_resource(RenderCamera::new(&self.render_state.device, aspect));
        }
        self.world
            .insert_resource(RenderLight::new(&self.render_state.device));

        self.load_default_material();

        let model = render::Model::load(AssetPath::Assets("ship.glb".to_string()), self).unwrap();

        // let trans = Transform::with_position(Point3::new(0.2, 0.2, 0.0));
        let parent = self
            .world
            .spawn((Transform::default(), Rotation { speed: 1.0 }))
            .id();

        for mesh in model.meshes {
            let uploaded = Arc::new(mesh.upload(self));
            self.world.spawn((
                MeshRenderer::new(uploaded),
                TransformBuilder::default()
                    .parent(Some(parent))
                    .position(Point3::new(0.0, 0.0, 0.0))
                    .build()
                    .unwrap(),
            ));
        }
    }

    pub fn input(&mut self, event: &WindowEvent) -> bool {
        self.world.resource_mut::<Input>().update(event);
        false
    }

    pub fn handle_redraw(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.egui_renderer.begin_frame(&self.window);
        self.pre_update();
        self.update();
        self.post_update();

        match self.render() {
            Ok(_) => {}
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.resize(self.render_state.size)
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
    }

    pub fn update(&mut self) {
        CameraConfig::panel(&mut self.world, &self.egui_renderer);
        self.world.run_system_once(sys_update_camera);
        self.world.run_system_once(sys_update_rotation);
    }

    pub fn post_update(&mut self) {
        // Update transform unifrom
        {
            let transform_bind_group_layout =
                self.world.resource::<TransformBindGroupLayout>().0.clone();
            let matrix_cache = self
                .world
                .query::<(Entity, &MeshRenderer, &Transform)>()
                .iter(&self.world)
                .map(|(entity, _, trans)| (entity, trans.calculate_world_matrix4x4(&self.world)))
                .collect::<Vec<_>>();

            for (entity, matrix) in matrix_cache.iter() {
                let (mut mesh_renderer, transform) = self
                    .world
                    .query::<(&mut MeshRenderer, &mut Transform)>()
                    .get_mut(&mut self.world, *entity)
                    .unwrap();

                if mesh_renderer.transform_bind_group.is_none() {
                    mesh_renderer.init_transform_buffer(
                        &self.render_state.device,
                        &transform_bind_group_layout,
                        *matrix,
                    );
                } else if transform.is_changed() {
                    mesh_renderer.update_transform_buffer(&self.render_state.queue, *matrix);
                }
            }
        }

        // Update camera uniform
        {
            if self.world.is_resource_changed::<RenderCamera>() {
                self.world
                    .resource::<RenderCamera>()
                    .update_uniform2gpu(&self.render_state.queue);
            }
        }

        // Update light uniform
        {
            if self.world.is_resource_changed::<RenderLight>() {
                self.world
                    .resource::<RenderLight>()
                    .update_uniform2gpu(&self.render_state.queue);
            }
        }
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.render_state.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder =
            self.render_state
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });

        {
            // 1. Render Pass
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
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

            // Draw Objects
            let default_material = self.material_instances.get_by_name("default").unwrap();

            for mesh_renderer in self.world.query::<&MeshRenderer>().iter(&self.world) {
                if let Some(mesh) = mesh_renderer.mesh.as_ref() {
                    if let Some(transform_bind_group) = mesh_renderer.transform_bind_group.as_ref()
                    {
                        let mut ctx = DrawContext {
                            render_pass: &mut render_pass,
                            default_material: Arc::clone(&default_material),
                            transform_bind_group,
                            world: &self.world,
                        };
                        mesh.draw(&mut ctx);
                    }
                }
            }
        }
        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [
                self.render_state.config.width,
                self.render_state.config.height,
            ],
            pixels_per_point: self.window.scale_factor() as f32 * self.egui_scale_factor,
        };
        {
            self.egui_renderer.end_frame_and_draw(
                &self.render_state.device,
                &self.render_state.queue,
                &mut encoder,
                &self.window,
                &view,
                screen_descriptor,
            )
        }
        // End Draw Objects

        self.render_state
            .queue
            .submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

pub fn sys_update_rotation(mut q: Query<(&mut Transform, &Rotation)>, time: Res<Time>) {
    for (mut trans, rot) in q.iter_mut() {
        //todo delta time
        trans.rotation = Quaternion::from_angle_y(Rad(rot.speed) * time.delta_time.as_secs_f32())
            * trans.rotation;
    }
}

pub fn sys_update_camera(
    config: Res<CameraConfig>,
    input: Res<Input>,
    time: Res<Time>,
    mut render_camera: ResMut<RenderCamera>,
) {
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
    if move_vec != Vector3::new(0., 0., 0.) {
        move_vec = move_vec.normalize() * speed * time.delta_time.as_secs_f32();
        render_camera.camera.eye += move_vec;
        render_camera.camera.target += move_vec;
    }
}
