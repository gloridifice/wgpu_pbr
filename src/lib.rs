use crate::{
    control::ControlPlugin,
    editor::EditorPlugin,
    material::outline::{Outline, OutlineConfig, OutlinePlugin},
};
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_log::LogPlugin;
use lentille_core::{
    input::InputPlugin,
    time::{Time, TimePlugin},
    window::WindowPlugin,
};
use lentille_render::{
    RenderPlugin,
    picking::{
        PickingPlugin,
        resources::{PickEvent, Selected, SelectedObjects},
    },
    prelude::*,
};
mod control;
mod editor;
mod egui_renderer;
mod material;
pub mod scene;

struct SelectionOutlineGroup;
impl material::outline::OutlineGroupType for SelectionOutlineGroup {}
fn sys_on_pick_event(
    trigger: On<PickEvent>,
    mut commands: Commands,
    mut selected: ResMut<SelectedObjects>,
    old: Query<Entity, With<Selected>>,
) {
    for e in old {
        commands.entity(e).remove::<(Selected, Outline)>();
    }
    selected.set_single(trigger.entity);
    if let Some(e) = trigger.entity {
        commands.entity(e).insert((
            Selected,
            Outline::with_config::<SelectionOutlineGroup>(OutlineConfig {
                thickness: 4.,
                color: Color::new(1., 0.85, 0.2, 1.),
            }),
        ));
    }
}
pub struct WgpuPbrPlugin;
impl Plugin for WgpuPbrPlugin {
    fn build(&self, app: &mut App) {
        lentille_core::profiling::init(app);
        lentille_core::profiling::name_thread("main");
        // Legacy -t (and embedding applications) may already own the subscriber.
        if !tracing::dispatcher::has_been_set() {
            let log_plugin = LogPlugin::default();
            #[cfg(feature = "profile-tracy")]
            let log_plugin = LogPlugin {
                // Bevy's default `wgpu=error` also matches our `wgpu_pbr` target.
                filter: format!("{},wgpu_pbr=info", log_plugin.filter),
                // tracing-tracy reuses formatted fields; ANSI pollutes zone names.
                fmt_layer: |_| Some(Box::new(tracing_subscriber::fmt::layer().with_ansi(false))),
                ..log_plugin
            };
            app.add_plugins(log_plugin);
        }
        app.add_plugins((RenderPlugin, InputPlugin, TimePlugin, WindowPlugin))
            .add_plugins((
                ControlPlugin,
                EditorPlugin,
                OutlinePlugin,
                PickingPlugin,
                scene::ScenePlugin,
            ))
            .add_systems(
                Update,
                (sys_update_rotation.run_if(scene::scene_editable), sys_gizmo),
            )
            .add_observer(sys_on_pick_event);
    }
}
#[derive(Debug, Clone, PartialEq, Component, bevy_reflect::Reflect)]
#[reflect(Component)]
pub struct RotationObject {
    pub speed: f32,
}
fn sys_gizmo(
    lights: Query<(&PointLight, &WorldTransform)>,
    cameras: Query<&WorldTransform, (With<Camera>, Without<control::camera::MainCamera>)>,
) {
    for (light, transform) in &lights {
        Gizmo::dot(transform.position, 0.1, light.color);
    }
    for transform in &cameras {
        Gizmo::line(
            transform.position,
            transform.position + transform.forward() * 2.0,
            Color::GREEN,
        );
        Gizmo::dot(transform.position, 0.5, Color::GREEN);
    }
}

pub fn sys_update_rotation(mut query: Query<(&mut Transform, &RotationObject)>, time: Res<Time>) {
    for (mut trans, rot) in &mut query {
        if rot.speed != 0. {
            trans.rotation =
                Quat::from_angle_y(Rad(rot.speed) * time.delta_time.as_secs_f32()) * trans.rotation;
        }
    }
}
