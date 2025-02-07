use std::any::type_name;

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::Resource;
use bevy_ecs::world::World;
use cgmath::{Deg, Euler};
use egui::{Color32, Context, DragValue, Ui, Widget};
use egui_wgpu::wgpu::{CommandEncoder, Device, Queue, StoreOp, TextureFormat, TextureView};
use egui_wgpu::{wgpu, Renderer, ScreenDescriptor};
use egui_winit::State;
use winit::event::WindowEvent;
use winit::window::Window;

use crate::cgmath_ext::{Vec4, Vector4Ext};
use crate::engine_lifetime::Name;
use crate::render::camera::CameraController;
use crate::render::light::{ParallelLight, PointLight};
use crate::render::material::pbr::PBRMaterial;
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

fn value(ui: &mut Ui, v: &mut f32) {
    ui.add_sized([40.0, 20.0], DragValue::new(v).max_decimals(1).speed(0.05));
}

fn label_value(ui: &mut Ui, text: &str, v: &mut f32) {
    ui.horizontal(|ui| {
        ui.label(text);
        value(ui, v);
    });
}

fn color_vec4_srgba(ui: &mut Ui, color: &mut Vec4) -> egui::Response {
    let mut c = color.to_color32();
    let ret = ui.color_edit_button_srgba(&mut c);
    *color = Vec4::from_color32(&c);
    ret
}

pub fn transform_ui(ui: &mut Ui, transform: &mut Transform) {
    ui.horizontal(|ui| {
        ui.label("Pos");
        [
            &mut transform.position.x,
            &mut transform.position.y,
            &mut transform.position.z,
        ]
        .into_iter()
        .for_each(|it| value(ui, it));
    });
    ui.horizontal(|ui| {
        let euler = Euler::from(transform.rotation);
        ui.label("Rot");
        let mut x = Deg::from(euler.x);
        let mut y = Deg::from(euler.y);
        let mut z = Deg::from(euler.z);
        [&mut x.0, &mut y.0, &mut z.0]
            .into_iter()
            .for_each(|it| value(ui, it));
        transform.rotation = Euler::new(x, y, z).into();
    });
    ui.horizontal(|ui| {
        ui.label("Sca");
        ui.add(DragValue::new(&mut transform.scale.x));
        ui.add(DragValue::new(&mut transform.scale.y));
        ui.add(DragValue::new(&mut transform.scale.z));
    });
}

macro_rules! impl_component_ui {
    ($A: ty, $W: expr, $I: expr, $ui: expr, $nui: ident, $N: ident, $B: block) => {
        if let Some(mut $N) = $W.get_mut::<$A>($I) {
            let ty_name = type_name::<$A>();
            egui::Frame::dark_canvas($ui.style())
                .inner_margin(egui::Vec2::new(10., 8.))
                .show($ui, |$nui| {
                    $nui.colored_label(
                        Color32::LIGHT_GRAY,
                        ty_name.split("::").last().unwrap_or(ty_name),
                    );
                    $B
                });
        }
    };
}

pub fn option_value<T>(
    ui: &mut Ui,
    opt: &mut Option<T>,
    default_value: T,
    behaviour: fn(&mut Ui, &mut T),
) {
    let mut checked = opt.is_some();
    egui::Checkbox::without_text(&mut checked).ui(ui);

    if checked && opt.is_none() {
        *opt = Some(default_value);
    }
    if !checked && opt.is_some() {
        *opt = None;
    }
    if let Some(value) = opt.as_mut() {
        behaviour(ui, value);
    }
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

        impl_component_ui!(CameraController, world, id, ui, ui, camera, {
            ui.horizontal(|ui| {
                ui.label("yaw");
                ui.add(DragValue::new(&mut camera.yaw));
                ui.label("row");
                ui.add(DragValue::new(&mut camera.row));
            });
        });

        impl_component_ui!(PointLight, world, id, ui, ui, light, {
            ui.horizontal(|ui| {
                ui.label("Color");
                color_vec4_srgba(ui, &mut light.color);
            });
            label_value(ui, "Intensity", &mut light.intensity);
            label_value(ui, "Iecay", &mut light.decay);
        });

        impl_component_ui!(PBRMaterial, world, id, ui, ui, mat, {
            egui::Grid::new(format!("PBR {}", id.index()))
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Roughness");
                    option_value(ui, &mut mat.roughness, 0.0, |ui, roughness| {
                        ui.add(egui::Slider::new(roughness, 0.0f32..=1.0f32));
                    });
                    ui.end_row();

                    ui.label("Metallic");
                    option_value(ui, &mut mat.metallic, 0.0, |ui, it| {
                        ui.add(egui::Slider::new(it, 0.0f32..=1.0f32));
                    });
                    ui.end_row();

                    ui.label("Reflectance");
                    option_value(ui, &mut mat.reflectance, 0.0, |ui, it| {
                        ui.add(egui::Slider::new(it, 0.0f32..=1.0f32));
                    });
                    ui.end_row();
                });
        });

        impl_component_ui!(ParallelLight, world, id, ui, ui, light, {
            egui::Grid::new(format!("ParallelLight {}", id.index()))
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Intensity");
                    value(ui, &mut light.intensity);
                    ui.end_row();

                    ui.label("Size");
                    value(ui, &mut light.size);
                    ui.end_row();

                    ui.label("Color");
                    color_vec4_srgba(ui, &mut light.color);
                    ui.end_row();
                });
        });

        let mut children = vec![];
        impl_component_ui!(Transform, world, id, ui, ui, trans, {
            transform_ui(ui, &mut trans);
            children = trans.children.clone()
        });

        for id in children.into_iter() {
            world_tree(ui, id, world);
        }
    });
}
