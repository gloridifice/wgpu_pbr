use bevy_app::{Plugin, Update};
use bevy_ecs::{prelude::*, system::RunSystemOnce};
use lentille_core::{input::Input, window::WinitWindow};
use winit::keyboard::KeyCode;

use crate::control::camera::CameraControlPlugin;

pub(crate) mod camera;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct EditorInputSet;

pub struct ControlPlugin;

impl Plugin for ControlPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_plugins(CameraControlPlugin)
            .init_resource::<ControlState>()
            .add_systems(Update, sys_input_implement.after(EditorInputSet));
    }
}

#[derive(Resource)]
pub struct ControlState {
    pub is_focused: bool,
    pub is_over_scene: bool,
    /// Set by the editor when Escape is consumed (e.g. to cancel a rename)
    /// so the control system can skip its own Escape action this frame.
    pub escape_consumed_by_editor: bool,
}
impl Default for ControlState {
    fn default() -> Self {
        ControlState {
            is_focused: true,
            is_over_scene: true,
            escape_consumed_by_editor: false,
        }
    }
}

/// 软件输入监测，目前行为：
///
/// - ECS 键触发隐藏光标和出现光标
pub fn sys_input_implement(
    mut commands: Commands,
    input: Res<Input>,
    mut control_state: ResMut<ControlState>,
) {
    if input.is_key_down(KeyCode::Escape) && !control_state.escape_consumed_by_editor {
        control_state.is_focused = !control_state.is_focused;
        commands.queue(|world: &mut World| {
            world.run_system_once(sys_toggle_cursor).unwrap();
        });
    }
}

/// 出现光标和隐藏光标
pub fn sys_toggle_cursor(control_state: ResMut<ControlState>, q_window: Query<&WinitWindow>) {
    for WinitWindow(window) in q_window {
        window.set_cursor_visible(!control_state.is_focused);
        let _ = window.set_cursor_grab(if control_state.is_focused {
            winit::window::CursorGrabMode::Locked
        } else {
            winit::window::CursorGrabMode::None
        });
    }
}
