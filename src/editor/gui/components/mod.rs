use bevy_app::Plugin;
use bevy_ecs::prelude::*;
use egui::*;
use lentille_render::prelude::*;
use std::any::type_name;
use yummy_egui::{
    IconButton, PanelButton, ParameterList, ParameterRows, ParameterSlider, h3, segmented_control,
};

use crate::{
    control::camera::CameraController,
    editor::gui::components::property_window::PropertyWindowPlugin,
};

pub mod basics;
pub mod depth_to_rgba;
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

fn texture_source_rows(
    rows: &mut ParameterRows<'_>,
    id: &str,
    label: &str,
    value: &mut lentille_render::material::pbr::TextureOverride,
) {
    use lentille_render::{
        asset::{AssetRef, SubAsset},
        material::pbr::TextureOverride,
    };
    let mut mode = match value {
        TextureOverride::Inherit => 0,
        TextureOverride::Set(_) => 1,
        TextureOverride::Clear => 2,
    };
    rows.row((id, "mode"), label, "Texture override mode", |ui| {
        segmented_control(
            ui,
            &mut mode,
            &[(0, "Inherit"), (1, "Set"), (2, "Clear")],
        );
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
                rows.row((id, "path"), "Texture path", "Path relative to assets/", |ui| {
                    ui.text_edit_singleline(path);
                });
                let mut embedded = matches!(reference.label, SubAsset::Image(_));
                if rows
                    .row(
                        (id, "source"),
                        "Source",
                        "External file or embedded model image",
                        |ui| {
                            segmented_control(
                                ui,
                                &mut embedded,
                                &[(false, "File"), (true, "Embedded")],
                            )
                        },
                    )
                    .changed()
                {
                    reference.label = if embedded {
                        SubAsset::Image(0)
                    } else {
                        SubAsset::File
                    };
                }
                if let SubAsset::Image(index) = &mut reference.label {
                    rows.row((id, "index"), "Image index", "Embedded image index", |ui| {
                        ui.add(DragValue::new(index).prefix("Image "));
                    });
                }
            }
        }
    }
}

pub fn vec3_values_ui(ui: &mut Ui, vec3: &mut Vec3, default_value: Vec3) {
    ui.horizontal(|ui| {
        if ui
            .add(
                IconButton::new("↺")
                    .size(22.0)
                    .icon_size(13.0)
                    .label("Reset"),
            )
            .clicked()
        {
            *vec3 = default_value;
        }
        let spacing = ui.spacing().item_spacing.x;
        let field_width = ((ui.available_width() - 2.0 * spacing) / 3.0).clamp(40.0, 64.0);
        for component in [&mut vec3.x, &mut vec3.y, &mut vec3.z] {
            ui.add_sized(
                [field_width, 22.0],
                DragValue::new(component).max_decimals(1).speed(0.05),
            );
        }
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
    ParameterList::new(("trans", id.index())).show(ui, |rows| {
        rows.row("position", "Position", "Local translation", |ui| {
            vec3_values_ui(ui, &mut transform.position, Vec3::zero());
        });
        rows.row("rotation", "Rotation", "Local Euler rotation in degrees", |ui| {
            vec3_values_ui(ui, &mut euler_deg, Vec3::zero());
        });
        rows.row("scale", "Scale", "Local scale", |ui| {
            vec3_values_ui(ui, &mut transform.scale, Vec3::one());
        });
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
                    h3($nui, ty_name.split("::").last().unwrap_or(ty_name));
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
        ParameterList::new(("mesh_source", entity.index())).show(ui, |rows| {
            rows.row(
                "source",
                "Source",
                "Procedural plane or imported model mesh",
                |ui| {
                    segmented_control(
                        ui,
                        &mut file,
                        &[(false, "Plane"), (true, "Model mesh")],
                    );
                },
            );
            if file != matches!(source, MeshSource::Asset(_)) {
                source = if file {
                    MeshSource::Asset(AssetRef::new("models/model.glb", SubAsset::Mesh(0)))
                } else {
                    MeshSource::Plane { size: 2.0 }
                };
            }
            match &mut source {
                MeshSource::Plane { size } => {
                    rows.row("size", "Size", "Plane extent", |ui| {
                        ui.add(DragValue::new(size).range(0.001..=10000.0));
                    });
                }
                MeshSource::Asset(reference) => {
                    let AssetPath::Assets(path) = &mut reference.path;
                    rows.row("path", "Path", "Model path relative to assets/", |ui| {
                        ui.text_edit_singleline(path);
                    });
                    if let SubAsset::Mesh(index) = &mut reference.label {
                        rows.row("mesh", "Mesh", "Mesh index in the model", |ui| {
                            ui.add(DragValue::new(index));
                        });
                    }
                }
            }
        });
    });
    if let Some(state) = world.get::<PreparationState>(entity) {
        if let Some(error) = &state.error {
            ui.colored_label(Color32::LIGHT_RED, error);
        }
        if ui
            .add(PanelButton::new("Rebuild / Retry assets"))
            .clicked()
        {
            world
                .resource_mut::<crate::scene::SceneController>()
                .request(crate::scene::SceneRequest::Retry(entity));
        }
    }
    impl_component_ui!(crate::RotationObject, world, entity, ui, ui, rotation, {
        ParameterList::new(("rotation", entity.index())).show(ui, |rows| {
            rows.row("speed", "Speed", "Angular speed in radians/sec", |ui| {
                ui.add(
                    DragValue::new(&mut rotation.speed)
                        .speed(0.01)
                        .suffix(" rad/s"),
                );
            });
        });
    });
    impl_component_ui!(Camera, world, entity, ui, ui, camera, {
        ParameterList::new(("cam", entity.index())).show(ui, |rows| {
            rows.row("fov", "FOV", "Vertical field of view in degrees", |ui| {
                ui.add(
                    ParameterSlider::new(&mut camera.fovy, 1.0..=179.0)
                        .suffix("°")
                        .max_decimals(1),
                );
            });
        });
    });

    impl_component_ui!(CameraController, world, entity, ui, ui, camera, {
        ParameterList::new(("camctrl", entity.index())).show(ui, |rows| {
            rows.row("yaw", "Yaw", "Camera yaw angle", |ui| {
                ui.add(DragValue::new(&mut camera.yaw).speed(0.05));
            });
            rows.row("row", "Row", "Camera pitch angle", |ui| {
                ui.add(DragValue::new(&mut camera.row).speed(0.05));
            });
        });
    });

    impl_component_ui!(PointLight, world, entity, ui, ui, light, {
        ParameterList::new(("pl", entity.index())).show(ui, |rows| {
            rows.row("color", "Color", "Light color", |ui| {
                color_rgba(ui, &mut light.color);
            });
            rows.row("intensity", "Intensity", "Light intensity", |ui| {
                value(ui, &mut light.intensity);
            });
            rows.row("decay", "Decay", "Distance attenuation decay", |ui| {
                value(ui, &mut light.decay);
            });
        });
    });

    if world.get::<MeshSource>(entity).is_some()
        && world.get::<PbrMaterial>(entity).is_none()
        && ui.add(PanelButton::new("Add material override")).clicked()
    {
        world.entity_mut(entity).insert(PbrMaterial::default());
    }
    impl_component_ui!(PbrMaterial, world, entity, ui, ui, mat, {
        ParameterList::new(("PBR", entity.index())).show(ui, |rows| {
            rows.row("roughness", "Roughness", "Surface roughness override", |ui| {
                option_value(ui, &mut mat.roughness, 0.0, |ui, roughness| {
                    ui.add(ParameterSlider::new(roughness, 0.0f32..=1.0f32).max_decimals(2));
                });
            });

            rows.row("metallic", "Metallic", "Metallic factor override", |ui| {
                option_value(ui, &mut mat.metallic, 0.0, |ui, it| {
                    ui.add(ParameterSlider::new(it, 0.0f32..=1.0f32).max_decimals(2));
                });
            });

            rows.row(
                "reflectance",
                "Reflectance",
                "Specular reflectance override",
                |ui| {
                    option_value(ui, &mut mat.reflectance, 0.0, |ui, it| {
                        ui.add(ParameterSlider::new(it, 0.0f32..=1.0f32).max_decimals(2));
                    });
                },
            );

            rows.row("color", "Color", "Base color override", |ui| {
                option_value(ui, &mut mat.color, Color::WHITE, |ui, it| {
                    let mut array_color = (*it).into_array();
                    ui.color_edit_button_rgba_unmultiplied(&mut array_color);
                    *it = Color::from_linear_array(array_color);
                });
            });

            texture_source_rows(
                rows,
                "base_color",
                "Base color texture",
                &mut mat.base_color_texture,
            );
            texture_source_rows(rows, "normal", "Normal texture", &mut mat.normal_texture);
            texture_source_rows(
                rows,
                "metallic_roughness",
                "Metallic / roughness",
                &mut mat.metallic_roughness_ao_texture,
            );
            texture_source_rows(
                rows,
                "emission",
                "Emission texture",
                &mut mat.emission_texture,
            );
        });
    });

    if world.get::<PbrMaterial>(entity).is_some()
        && ui
            .add(PanelButton::new("Remove material override"))
            .clicked()
    {
        world.entity_mut(entity).remove::<PbrMaterial>();
    }
    impl_component_ui!(ParallelLight, world, entity, ui, ui, light, {
        ParameterList::new(("ParallelLight", entity.index())).show(ui, |rows| {
            rows.row("intensity", "Intensity", "Directional light intensity", |ui| {
                value(ui, &mut light.intensity);
            });

            rows.row("size", "Size", "Angular size", |ui| {
                value(ui, &mut light.size);
            });

            rows.row("color", "Color", "Light color", |ui| {
                color_rgba(ui, &mut light.color);
            });
        });
    });

    impl_component_ui!(Transform, world, entity, ui, ui, trans, {
        transform_ui(ui, entity, &mut trans);
    });

    ui.add_space(4.0);
    ui.colored_label(Color32::GRAY, "Click outside or press ✖ to close");
}
