//! Standalone renderer: HDR scene -> SDR display -> surface-format-aware presentation.
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_log::LogPlugin;
use lentille_core::{
    input::InputPlugin,
    time::TimePlugin,
    window::{WindowPlugin, WinitWindowResizeEvent},
};
use lentille_render::{
    RenderPlugin,
    camera::{Camera, RenderTargetSize},
};

fn main() {
    color_backtrace::install();
    App::new()
        .add_plugins((
            LogPlugin::default(),
            RenderPlugin,
            InputPlugin,
            TimePlugin,
            WindowPlugin,
        ))
        .add_systems(Startup, spawn_camera)
        .add_observer(sys_on_window_resize)
        .run();
}

fn spawn_camera(mut commands: Commands) {
    use lentille_render::{camera::RenderTargetConfig, prelude::*, shadow::csm::CsmConfig};
    commands.spawn((
        Camera::new(1.0),
        RenderTargetConfig::PrimaryWindow,
        Transform::from_position(Vec3::new(0.0, 3.0, 8.0)),
        CsmConfig {
            level_count: 4,
            texture_size: 1024,
            linear_log_factor: 0.5,
            shadow_near: 1.0,
            shadow_far: 80.0,
        },
    ));
    commands.spawn((ParallelLight::default(), Transform::default()));
}

fn sys_on_window_resize(
    event: On<WinitWindowResizeEvent>,
    q_camera: Single<&mut RenderTargetSize, With<Camera>>,
) {
    let mut size = q_camera.into_inner();
    size.width = event.physical_size.width.max(1);
    size.height = event.physical_size.height.max(1);
}
