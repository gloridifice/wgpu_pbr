use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_log::LogPlugin;
use lentille_core::{
    input::InputPlugin,
    time::TimePlugin,
    window::{PrimaryWindow, WindowPlugin, WinitWindow, WinitWindowResizeEvent},
};
use lentille_render::{
    RenderPlugin, RenderState, SurfaceState,
    camera::{Camera, RenderTarget, RenderTargetSize},
    prelude::{AppExt, copy_texture2d_to_texture2d_no_mip},
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
        .add_render_system_in_frame_set(
            lentille_render::FrameSets::Present,
            sys_end_frame_and_present,
        )
        .add_systems(Startup, wgpu_pbr::sys_spawn_camera)
        .add_observer(sys_on_window_resize)
        .run();
}

fn sys_on_window_resize(
    event: On<WinitWindowResizeEvent>,
    q_camera: Single<(&mut RenderTargetSize), With<Camera>>,
) {
    let (mut target_size) = q_camera.into_inner();
    let size = event.physical_size;
    target_size.width = size.width;
    target_size.height = size.height;
}

fn sys_end_frame_and_present(
    rs: Res<RenderState>,
    window: Single<(&WinitWindow, &SurfaceState), With<PrimaryWindow>>,
    q_camera: Single<&RenderTarget, With<Camera>>,
) {
    let (_window, surface_state) = window.into_inner();
    let render_target = q_camera.into_inner();

    let current_texture = match surface_state.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(surface_texture)
        | wgpu::CurrentSurfaceTexture::Suboptimal(surface_texture) => surface_texture,
        _ => {
            return;
        }
    };

    if render_target.get_current_color().texture.size() != current_texture.texture.size() {
        return;
    }

    let mut encoder = rs
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Present"),
        });

    copy_texture2d_to_texture2d_no_mip(
        &mut encoder,
        &render_target.get_current_color().texture,
        &current_texture.texture,
        current_texture.texture.size(),
    );

    rs.queue.submit(std::iter::once(encoder.finish()));

    current_texture.present();
}
