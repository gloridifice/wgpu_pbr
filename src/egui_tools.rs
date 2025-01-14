use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::Resource;
use bevy_ecs::world::World;
use cgmath::{Deg, Euler};
use egui::{Context, DragValue, Ui};
use egui_wgpu::wgpu::{CommandEncoder, Device, Queue, StoreOp, TextureFormat, TextureView};
use egui_wgpu::{wgpu, Renderer, ScreenDescriptor};
use egui_winit::State;
use winit::event::WindowEvent;
use winit::window::Window;

use crate::engine_lifetime::Name;
use crate::render::transform::Transform;

#[derive(Resource)]
pub struct EguiConfig {
    pub egui_scale_factor: f32,
}
impl Default for EguiConfig {
    fn default() -> Self {
        Self {
            egui_scale_factor: 0.8,
        }
    }
}

#[derive(Resource)]
pub struct EguiRenderer {
    pub state: State,
    pub renderer: Renderer,
    pub frame_started: bool,
}

impl EguiRenderer {
    pub fn context(&self) -> &Context {
        self.state.egui_ctx()
    }

    pub fn new(
        device: &Device,
        output_color_format: TextureFormat,
        output_depth_format: Option<TextureFormat>,
        msaa_samples: u32,
        window: &Window,
    ) -> EguiRenderer {
        let egui_context = Context::default();

        let egui_state = egui_winit::State::new(
            egui_context,
            egui::viewport::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            Some(2 * 1024), // default dimension is 2048
        );
        let egui_renderer = Renderer::new(
            device,
            output_color_format,
            output_depth_format,
            msaa_samples,
            true,
        );

        EguiRenderer {
            state: egui_state,
            renderer: egui_renderer,
            frame_started: false,
        }
    }

    pub fn handle_input(&mut self, window: &Window, event: &WindowEvent) {
        let _ = self.state.on_window_event(window, event);
    }

    pub fn ppp(&mut self, v: f32) {
        self.context().set_pixels_per_point(v);
    }

    pub fn begin_frame(&mut self, window: &Window) {
        let raw_input = self.state.take_egui_input(window);
        self.state.egui_ctx().begin_pass(raw_input);
        self.frame_started = true;
    }

    pub fn end_frame_and_draw(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        window: &Window,
        window_surface_view: &TextureView,
        screen_descriptor: ScreenDescriptor,
    ) {
        if !self.frame_started {
            panic!("begin_frame must be called before end_frame_and_draw can be called!");
        }

        self.ppp(screen_descriptor.pixels_per_point);

        let full_output = self.state.egui_ctx().end_pass();

        self.state
            .handle_platform_output(window, full_output.platform_output);

        let tris = self
            .state
            .egui_ctx()
            .tessellate(full_output.shapes, self.state.egui_ctx().pixels_per_point());
        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer
                .update_texture(device, queue, *id, image_delta);
        }
        self.renderer
            .update_buffers(device, queue, encoder, &tris, &screen_descriptor);
        let rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: window_surface_view,
                resolve_target: None,
                ops: egui_wgpu::wgpu::Operations {
                    load: egui_wgpu::wgpu::LoadOp::Load,
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            label: Some("egui main render pass"),
            occlusion_query_set: None,
        });

        self.renderer
            .render(&mut rpass.forget_lifetime(), &tris, &screen_descriptor);
        for x in &full_output.textures_delta.free {
            self.renderer.free_texture(x)
        }

        self.frame_started = false;
    }
}

pub fn transform_ui(ui: &mut Ui, transform: &mut Transform) {
    ui.horizontal(|ui| {
        ui.label("Pos");
        ui.add(DragValue::new(&mut transform.position.x));
        ui.add(DragValue::new(&mut transform.position.y));
        ui.add(DragValue::new(&mut transform.position.z));
    });
    ui.horizontal(|ui| {
        let euler = Euler::from(transform.rotation);
        ui.label("Rot");
        let mut x = Deg::from(euler.x);
        let mut y = Deg::from(euler.y);
        let mut z = Deg::from(euler.z);
        ui.add(DragValue::new(&mut x.0));
        ui.add(DragValue::new(&mut y.0));
        ui.add(DragValue::new(&mut z.0));
        transform.rotation = Euler::new(x, y, z).into();
    });
    ui.horizontal(|ui| {
        ui.label("Sca");
        ui.add(DragValue::new(&mut transform.scale.x));
        ui.add(DragValue::new(&mut transform.scale.y));
        ui.add(DragValue::new(&mut transform.scale.z));
    });
}

pub fn world_tree(ui: &mut Ui, id: Entity, world: &mut World) {
    let display_name = {
        let mut ret = format!(" #{}", id.index());
        if let Some(name) = world.get::<Name>(id) {
            ret.insert_str(0, &name.0);
        }
        ret
    };

    ui.collapsing(display_name, |ui: &mut Ui| {
        ui.separator();
        let children = if let Some(mut trans) = world.get_mut::<Transform>(id) {
            transform_ui(ui, &mut trans);
            trans.children.clone()
        } else {
            vec![]
        };
        for id in children.into_iter() {
            world_tree(ui, id, world);
        }
    });
}
