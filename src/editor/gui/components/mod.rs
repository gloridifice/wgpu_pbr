use bevy_app::Plugin;
use bevy_ecs::prelude::*;
use egui::*;
use lentille_render::prelude::*;
use std::any::type_name;

use crate::{
    control::camera::CameraController,
    editor::gui::components::property_window::PropertyWindowPlugin,
};

pub mod basics;
pub mod depth_to_rgba;
pub mod dock_style_editor;
pub mod property_window;
pub mod scene_browser;
pub mod texture_preview;
pub mod world_hierarchy;

pub struct EditorComponentPlugin;
impl Plugin for EditorComponentPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_plugins(PropertyWindowPlugin);
    }
}

fn value(ui: &mut Ui, v: &mut f32) {
    ui.add_sized([60.0, 22.0], DragValue::new(v).max_decimals(1).speed(0.05));
}

fn color_rgba(ui: &mut Ui, color: &mut Color) -> egui::Response {
    let mut c: Color32 = Color32::from_rgba_unmultiplied(
        (color.r() * 255.0) as u8,
        (color.g() * 255.0) as u8,
        (color.b() * 255.0) as u8,
        (color.a() * 255.0) as u8,
    );
    let ret = ui.color_edit_button_srgba(&mut c);
    if ret.changed() {
        *color = Color::new(
            c.r() as f32 / 255.0,
            c.g() as f32 / 255.0,
            c.b() as f32 / 255.0,
            c.a() as f32 / 255.0,
        );
    }
    ret
}

fn texture_source_ui(
    ui: &mut Ui,
    label: &str,
    value: &mut lentille_render::material::pbr::TextureOverride,
) {
    use lentille_render::{
        asset::{AssetRef, SubAsset},
        material::pbr::TextureOverride,
    };
    ui.push_id(label, |ui| {
        let mut mode = match value {
            TextureOverride::Inherit => 0,
            TextureOverride::Set(_) => 1,
            TextureOverride::Clear => 2,
        };
        ui.horizontal(|ui| {
            ui.label(label);
            egui::ComboBox::from_id_salt("mode")
                .selected_text(["Inherit", "Set", "Clear"][mode])
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut mode, 0, "Inherit");
                    ui.selectable_value(&mut mode, 1, "Set");
                    ui.selectable_value(&mut mode, 2, "Clear");
                });
        });
        match mode {
            0 => *value = TextureOverride::Inherit,
            2 => *value = TextureOverride::Clear,
            _ => {
                if !matches!(value, TextureOverride::Set(_)) {
                    *value =
                        TextureOverride::Set(AssetRef::new("textures/white.png", SubAsset::File));
                }
                if let TextureOverride::Set(reference) = value {
                    let AssetPath::Assets(path) = &mut reference.path;
                    ui.text_edit_singleline(path);
                    let mut embedded = matches!(reference.label, SubAsset::Image(_));
                    if ui
                        .checkbox(&mut embedded, "Image embedded in model")
                        .changed()
                    {
                        reference.label = if embedded {
                            SubAsset::Image(0)
                        } else {
                            SubAsset::File
                        };
                    }
                    if let SubAsset::Image(index) = &mut reference.label {
                        ui.add(DragValue::new(index).prefix("Image "));
                    }
                }
            }
        }
    });
}

fn property_grid(ui: &mut Ui, id_source: impl std::hash::Hash, contents: impl FnOnce(&mut Ui)) {
    egui::Grid::new(id_source)
        .num_columns(2)
        .spacing([12.0, 8.0])
        .min_col_width(80.0)
        .striped(true)
        .show(ui, contents);
}

pub fn vec3_values_ui(ui: &mut Ui, vec3: &mut Vec3, default_value: Vec3) {
    ui.horizontal(|ui| {
        if ui.button("↺").clicked() {
            *vec3 = default_value;
        }
        value(ui, &mut vec3.x);
        value(ui, &mut vec3.y);
        value(ui, &mut vec3.z);
    });
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

    let old_euler = euler_deg;
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

    if euler_deg != old_euler {
        transform.rotation =
            Euler::new(Deg(euler_deg.x), Deg(euler_deg.y), Deg(euler_deg.z)).into();
    }
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

macro_rules! impl_component_ui {
    ($A: ty, $W: expr, $I: expr, $ui: expr, $nui: ident, $N: ident, $B: block) => {
        if let Some(mut $N) = $W.get::<$A>($I).cloned() {
            let original = $N.clone();
            let ty_name = type_name::<$A>();
            basics::Frame::new()
                .inner_margin(egui::Vec2::new(10., 8.))
                .show($ui, |$nui| {
                    $nui.colored_label(
                        Color32::LIGHT_GRAY,
                        ty_name.split("::").last().unwrap_or(ty_name),
                    );
                    $B
                });
            if $N != original && !$W.resource::<crate::scene::SceneController>().read_only() {
                $W.entity_mut($I).insert($N);
            }
        }
    };
}

/// Renders all editable components for `entity` in a flat property-editor layout.
/// Designed to be called inside an egui Window / Area (not inside the hierarchy tree).
pub fn property_window_ui(ui: &mut egui::Ui, entity: Entity, world: &mut World) {
    let editable = !world
        .resource::<crate::scene::SceneController>()
        .read_only();
    ui.add_enabled_ui(editable, |ui| property_window_contents(ui, entity, world));
}
fn property_window_contents(ui: &mut egui::Ui, entity: Entity, world: &mut World) {
    use lentille_render::asset::{AssetRef, MeshSource, PreparationState, SubAsset};
    impl_component_ui!(MeshSource, world, entity, ui, ui, source, {
        let mut file = matches!(source, MeshSource::Asset(_));
        ui.horizontal(|ui| {
            ui.selectable_value(&mut file, false, "Plane");
            ui.selectable_value(&mut file, true, "Model mesh");
        });
        if file != matches!(source, MeshSource::Asset(_)) {
            source = if file {
                MeshSource::Asset(AssetRef::new("models/model.glb", SubAsset::Mesh(0)))
            } else {
                MeshSource::Plane { size: 2.0 }
            };
        }
        match &mut source {
            MeshSource::Plane { size } => {
                ui.add(DragValue::new(size).range(0.001..=10000.0).prefix("Size "));
            }
            MeshSource::Asset(reference) => {
                let AssetPath::Assets(path) = &mut reference.path;
                ui.text_edit_singleline(path);
                if let SubAsset::Mesh(index) = &mut reference.label {
                    ui.add(DragValue::new(index).prefix("Mesh "));
                }
            }
        }
    });
    if let Some(state) = world.get::<PreparationState>(entity) {
        if let Some(error) = &state.error {
            ui.colored_label(Color32::LIGHT_RED, error);
        }
        if ui.button("Rebuild / Retry assets").clicked() {
            world
                .resource_mut::<crate::scene::SceneController>()
                .request(crate::scene::SceneRequest::Retry(entity));
        }
    }
    impl_component_ui!(crate::RotationObject, world, entity, ui, ui, rotation, {
        ui.add(
            DragValue::new(&mut rotation.speed)
                .speed(0.01)
                .prefix("Radians/sec "),
        );
    });
    impl_component_ui!(Camera, world, entity, ui, ui, camera, {
        property_grid(ui, format!("cam {}", entity.index()), |ui| {
            ui.label("FOV");
            ui.add(
                DragValue::new(&mut camera.fovy)
                    .speed(0.05)
                    .range(1.0..=179.0),
            );
            ui.end_row();
        });
    });

    impl_component_ui!(CameraController, world, entity, ui, ui, camera, {
        property_grid(ui, format!("camctrl {}", entity.index()), |ui| {
            ui.label("Yaw");
            ui.add(DragValue::new(&mut camera.yaw).speed(0.05));
            ui.end_row();
            ui.label("Row");
            ui.add(DragValue::new(&mut camera.row).speed(0.05));
            ui.end_row();
        });
    });

    impl_component_ui!(PointLight, world, entity, ui, ui, light, {
        property_grid(ui, format!("pl {}", entity.index()), |ui| {
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

    if world.get::<MeshSource>(entity).is_some()
        && world.get::<PbrMaterial>(entity).is_none()
        && ui.button("Add material override").clicked()
    {
        world.entity_mut(entity).insert(PbrMaterial::default());
    }
    impl_component_ui!(PbrMaterial, world, entity, ui, ui, mat, {
        property_grid(ui, format!("PBR {}", entity.index()), |ui| {
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
        texture_source_ui(ui, "Base color texture", &mut mat.base_color_texture);
        texture_source_ui(ui, "Normal texture", &mut mat.normal_texture);
        texture_source_ui(
            ui,
            "Metallic / roughness",
            &mut mat.metallic_roughness_ao_texture,
        );
        texture_source_ui(ui, "Emission texture", &mut mat.emission_texture);
    });

    if world.get::<PbrMaterial>(entity).is_some() && ui.button("Remove material override").clicked()
    {
        world.entity_mut(entity).remove::<PbrMaterial>();
    }
    impl_component_ui!(ParallelLight, world, entity, ui, ui, light, {
        property_grid(ui, format!("ParallelLight {}", entity.index()), |ui| {
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

    impl_component_ui!(Transform, world, entity, ui, ui, trans, {
        transform_ui(ui, entity, &mut trans);
    });

    ui.add_space(4.0);
    ui.colored_label(Color32::GRAY, "Click outside or press ✖ to close");
}
