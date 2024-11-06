use std::sync::Arc;

use cgmath::{InnerSpace, Vector3};
use egui_wgpu::ScreenDescriptor;
use winit::{event::WindowEvent, keyboard::KeyCode};

use crate::{
    asset::{load::Loadable, AssetPath},
    input::INPUT,
    render::{self, camera::CameraConfig, DrawAble},
    time::TIME,
    State,
};

impl State {
    pub fn init(&mut self) {
        self.load_default_material();

        let model = render::Model::load(
            AssetPath::Assets("Patagiosites laevis.glb".to_string()),
            self,
        )
        .unwrap();

        let mut render = model
            .meshes
            .iter()
            .map(|it| Arc::new(it.upload(self)) as Arc<dyn DrawAble>)
            .collect::<Vec<_>>();
        self.renderables.append(&mut render);
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

            self.draw_objects(&mut render_pass);
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

        self.render_state
            .queue
            .submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}
