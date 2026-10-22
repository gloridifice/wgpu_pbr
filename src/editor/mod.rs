use bevy_app::Plugin;
use bevy_ecs::{
    observer::On,
    system::{Query, Res},
};
use lentille_core::window::{WinitWindow, WinitWindowResizeEvent};
use lentille_render::{RenderState, SurfaceState};

use crate::editor::gui::EditorGuiPlugin;

pub(crate) fn clear_scene_ui(world: &mut bevy_ecs::world::World) {
    gui::clear_scene_ui(world);
}

mod gui;

#[derive(bevy_ecs::schedule::SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct EditorUiSet;

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_plugins(EditorGuiPlugin)
            .add_observer(sys_on_winit_window_resized);
    }
}

fn sys_on_winit_window_resized(
    event: On<WinitWindowResizeEvent>,
    mut q_windows: Query<(&WinitWindow, &mut SurfaceState)>,
    rs: Res<RenderState>,
) {
    let WinitWindowResizeEvent {
        window_id,
        physical_size,
    } = &*event;
    if let Some((_, mut surface_state)) = q_windows
        .iter_mut()
        .find(|(window, _)| window.0.id() == *window_id)
    {
        surface_state.config.width = physical_size.width;
        surface_state.config.height = physical_size.height;
        surface_state.configure(&rs.device);
    }
}
