use bevy_app::{Plugin, Update};
use bevy_ecs::prelude::*;
use lentille_core::{input::Input, window::MainWindow};
use winit::keyboard::KeyCode;

use crate::control::camera::CameraContorlPlugin;

pub(crate) mod camera;

pub struct ControlPlugin;

impl Plugin for ControlPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_plugins(CameraContorlPlugin)
            .init_resource::<ControlState>()
            .add_systems(Update, (sys_control, sys_control_state));
    }
}

#[derive(Resource)]
pub struct ControlState {
    pub is_focused: bool,
}
impl Default for ControlState {
    fn default() -> Self {
        ControlState { is_focused: true }
    }
}

pub fn sys_control(
    mut commands: Commands,
    input: Res<Input>,
    mut control_state: ResMut<ControlState>,
) {
    if input.is_key_down(KeyCode::Escape) {
        control_state.is_focused = !control_state.is_focused;
        commands.queue(|world: &mut World| {
            world.run_system_cached(sys_control_state).unwrap();
        });
    }
}

pub fn sys_control_state(control_state: ResMut<ControlState>, main_window: Res<MainWindow>) {
    main_window.0.set_cursor_visible(!control_state.is_focused);
    let _ = main_window.0.set_cursor_grab(if control_state.is_focused {
        winit::window::CursorGrabMode::Locked
    } else {
        winit::window::CursorGrabMode::None
    });
}
