use bevy_ecs::prelude::*;
use egui::*;
use lentille_render::prelude::*;
use std::any::type_name;

use crate::control::camera::CameraController;

fn value(ui: &mut Ui, v: &mut f32) {
    ui.add_sized([40.0, 20.0], DragValue::new(v).max_decimals(1).speed(0.05));
}

fn color_rgba(ui: &mut Ui, color: &mut Color) -> egui::Response {
    let mut c: Color32 = Color32::from_rgba_unmultiplied(
        (color.r() * 255.0) as u8,
        (color.g() * 255.0) as u8,
        (color.b() * 255.0) as u8,
        (color.a() * 255.0) as u8,
    );
    let ret = ui.color_edit_button_srgba(&mut c);
    *color = Color::new(
        c.r() as f32 / 255.0,
        c.g() as f32 / 255.0,
        c.b() as f32 / 255.0,
        c.a() as f32 / 255.0,
    );
    ret
}

fn property_grid(ui: &mut Ui, id_source: impl std::hash::Hash, contents: impl FnOnce(&mut Ui)) {
    egui::Grid::new(id_source)
        .num_columns(2)
        .spacing([10.0, 6.0])
        .min_col_width(70.0)
        .striped(true)
        .show(ui, contents);
}

pub fn vec3_values_ui(ui: &mut Ui, vec3: &mut Vec3, default_value: Vec3) {
    if ui.button("↺").clicked() {
        *vec3 = default_value;
    }
    value(ui, &mut vec3.x);
    value(ui, &mut vec3.y);
    value(ui, &mut vec3.z);
}

pub fn vec3_ui(ui: &mut Ui, label: &str, vec3: &mut Vec3, default_value: Vec3) {
    ui.horizontal(|ui| {
        ui.label(label);
        vec3_values_ui(ui, vec3, default_value);
    });
}

pub fn transform_ui(ui: &mut Ui, id: Entity, transform: &mut Transform) {
    let mut euler_deg = {
        let euler = Euler::from(transform.rotation);
        Vec3::new(
            Deg::from(euler.x).0,
            Deg::from(euler.y).0,
            Deg::from(euler.z).0,
        )
    };

    property_grid(ui, format!("trans {}", id.index()), |ui| {
        ui.label("Position");
        vec3_values_ui(ui, &mut transform.position, Vec3::zero());
        ui.end_row();

        ui.label("Rotation");
        vec3_values_ui(ui, &mut euler_deg, Vec3::zero());
        ui.end_row();

        ui.label("Scale");
        vec3_values_ui(ui, &mut transform.scale, Vec3::one());
        ui.end_row();
    });

    transform.rotation = Euler::new(Deg(euler_deg.x), Deg(euler_deg.y), Deg(euler_deg.z)).into();
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
    ui.horizontal(|ui| {
        let mut checked = opt.is_some();
        if egui::Checkbox::without_text(&mut checked).ui(ui).changed() {
            if checked && opt.is_none() {
                *opt = Some(default_value);
            }
            if !checked && opt.is_some() {
                *opt = None;
            }
        }
        ui.add_space(4.0);
        if let Some(value) = opt.as_mut() {
            behaviour(ui, value);
        }
    });
}

pub fn world_tree(ui: &mut Ui, id: Entity, world: &mut World) {
    let display_name = {
        let mut ret = format!(" #{}", id.index());
        if let Some(name) = world.get::<Name>(id) {
            ret.insert_str(0, name.as_str());
        }
        ret
    };

    ui.collapsing(display_name, |ui: &mut Ui| {
        ui.separator();

        impl_component_ui!(Camera, world, id, ui, ui, camera, {
            property_grid(ui, format!("cam {}", id.index()), |ui| {
                ui.label("FOV");
                ui.add(DragValue::new(&mut camera.fovy).speed(0.05));
                ui.end_row();
            });
        });

        impl_component_ui!(CameraController, world, id, ui, ui, camera, {
            property_grid(ui, format!("camctrl {}", id.index()), |ui| {
                ui.label("Yaw");
                ui.add(DragValue::new(&mut camera.yaw).speed(0.05));
                ui.end_row();
                ui.label("Row");
                ui.add(DragValue::new(&mut camera.row).speed(0.05));
                ui.end_row();
            });
        });

        impl_component_ui!(PointLight, world, id, ui, ui, light, {
            property_grid(ui, format!("pl {}", id.index()), |ui| {
                ui.label("Color");
                color_rgba(ui, &mut light.color);
                ui.end_row();
                ui.label("Intensity");
                value(ui, &mut light.intensity);
                ui.end_row();
                ui.label("Decay");
                value(ui, &mut light.decay);
                ui.end_row();
            });
        });

        impl_component_ui!(PBRMaterial, world, id, ui, ui, mat, {
            property_grid(ui, format!("PBR {}", id.index()), |ui| {
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

                ui.label("Color");
                option_value(ui, &mut mat.color, Color::WHITE, |ui, it| {
                    let mut array_color = (*it).into_array();
                    ui.color_edit_button_rgba_unmultiplied(&mut array_color);
                    *it = Color::from_linear_array(array_color);
                });
                ui.end_row();
            });
        });

        impl_component_ui!(ParallelLight, world, id, ui, ui, light, {
            property_grid(ui, format!("ParallelLight {}", id.index()), |ui| {
                ui.label("Intensity");
                value(ui, &mut light.intensity);
                ui.end_row();

                ui.label("Size");
                value(ui, &mut light.size);
                ui.end_row();

                ui.label("Color");
                color_rgba(ui, &mut light.color);
                ui.end_row();
            });
        });

        let mut children = vec![];
        impl_component_ui!(Transform, world, id, ui, ui, trans, {
            transform_ui(ui, id, &mut trans);
            children = trans.children.clone()
        });

        for id in children.into_iter() {
            world_tree(ui, id, world);
        }
    });
}
