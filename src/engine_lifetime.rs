use std::{borrow::BorrowMut, sync::Arc};

use bevy_ecs::{change_detection::DetectChanges, entity::Entity};
use cgmath::{InnerSpace, Point3, Vector3};
use egui_wgpu::ScreenDescriptor;
use winit::{event::WindowEvent, keyboard::KeyCode};

use crate::{
    asset::{load::Loadable, AssetPath},
    input::INPUT,
    render::{
        self,
        camera::CameraConfig,
        transform::{Transform, TransformBuilder},
        DrawAble, DrawContext, MeshRenderer,
    },
    time::TIME,
    PushConstants, State,
};

impl State {
    pub fn init(&mut self) {
        self.load_default_material();

        let model = render::Model::load(
            AssetPath::Assets("Patagiosites laevis.glb".to_string()),
            self,
        )
        .unwrap();

        // let trans = Transform::with_position(Point3::new(0.2, 0.2, 0.0));
        let parent = self.world.spawn(Transform::default()).id();

        for mesh in model.meshes {
            let uploaded = Arc::new(mesh.upload(self));
            let y: f32 = rand::random();
            println!("y: {}", y);
            self.world.spawn((
                MeshRenderer::new(uploaded),
                TransformBuilder::default()
                    .parent(Some(parent))
                    .position(Point3::new(0.0, y, 0.0))
                    .build()
                    .unwrap(),
            ));
        }
    }

    pub fn input(&mut self, event: &WindowEvent) -> bool {
        INPUT.lock().unwrap().update(event);
        false
    }

    pub fn handle_redraw(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.egui_renderer.begin_frame(&self.window);
        self.pre_update();
        self.update();

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
        TIME.lock().unwrap().update();
    }

    pub fn update(&mut self) {
        let time = TIME.lock().unwrap();
        let input = INPUT.lock().unwrap();

        self.render_camera
            .camera_update(&mut self.world, &self.render_state.queue, &input, &time);
        CameraConfig::panel(&mut self.world, &self.egui_renderer);

        {
            let a = self
                .world
                .query::<(Entity, &MeshRenderer, &Transform)>()
                .iter(&self.world)
                .map(|(entity, _, trans)| (entity, trans.calculate_world_matrix4x4(&self.world)))
                .collect::<Vec<_>>();

            for (entity, matrix) in a.iter() {
                let (mut mesh_renderer, transform) = self
                    .world
                    .query::<(&mut MeshRenderer, &mut Transform)>()
                    .get_mut(&mut self.world, *entity)
                    .unwrap();

                if mesh_renderer.transform_bind_group.is_none() {
                    mesh_renderer.init_transform_buffer(
                        &self.render_state.device,
                        &self.transform_bind_group_layout,
                        *matrix,
                    );
                } else if transform.is_changed() {
                    mesh_renderer.update_transform_buffer(&self.render_state.queue, *matrix);
                }
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
                            camera_bind_group: &self.render_camera.camera_bind_group,
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
