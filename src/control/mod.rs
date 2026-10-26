use bevy_app::{Plugin, Update};
use bevy_ecs::prelude::*;
use lentille_core::{
    input::{CursorButton, Input},
    window::WinitWindow,
};
use lentille_math::Vec2;
use winit::{keyboard::KeyCode, window::CursorGrabMode};

use crate::control::camera::CameraControlPlugin;

pub(crate) mod camera;

pub struct ControlPlugin;

impl Plugin for ControlPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_plugins(CameraControlPlugin)
            .init_resource::<ControlState>()
            .add_systems(
                Update,
                sys_input_implement.after(crate::editor::EditorUiSet),
            );
    }
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum NavigationMode {
    #[default]
    Idle,
    Fly,
    Orbit,
}

#[derive(Resource, Default)]
pub struct ControlState {
    pub viewport_active: bool,
    pub mode: NavigationMode,
    pub just_started: bool,
    restore_cursor: Option<Vec2>,
}

pub fn sys_input_implement(
    input: Res<Input>,
    scene: Res<crate::scene::SceneController>,
    mut state: ResMut<ControlState>,
    windows: Query<&WinitWindow>,
) {
    let previous = state.mode;
    state.just_started = false;
    let alt = input.is_key_hold(KeyCode::AltLeft) || input.is_key_hold(KeyCode::AltRight);
    if !input.window_focused || scene.read_only() || input.is_key_down(KeyCode::Escape) {
        state.mode = NavigationMode::Idle;
    } else {
        match state.mode {
            NavigationMode::Fly if !input.hold_cursor_buttons.contains(&CursorButton::Right) => {
                state.mode = NavigationMode::Idle;
            }
            NavigationMode::Orbit
                if !alt || !input.hold_cursor_buttons.contains(&CursorButton::Left) =>
            {
                state.mode = NavigationMode::Idle;
            }
            NavigationMode::Idle if state.viewport_active => {
                if input.is_cursor_button_down(CursorButton::Right)
                    && input.hold_cursor_buttons.contains(&CursorButton::Right)
                {
                    state.mode = NavigationMode::Fly;
                } else if alt
                    && input.is_cursor_button_down(CursorButton::Left)
                    && input.hold_cursor_buttons.contains(&CursorButton::Left)
                {
                    state.mode = NavigationMode::Orbit;
                }
            }
            _ => {}
        }
    }
    if state.mode == previous {
        return;
    }
    state.just_started = state.mode != NavigationMode::Idle;
    if state.just_started {
        state.restore_cursor = Some(input.window_cursor_position);
    }
    for WinitWindow(window) in &windows {
        if state.mode != NavigationMode::Idle {
            if window.set_cursor_grab(CursorGrabMode::Locked).is_err() {
                let _ = window.set_cursor_grab(CursorGrabMode::Confined);
            }
            window.set_cursor_visible(false);
        } else {
            let _ = window.set_cursor_grab(CursorGrabMode::None);
            window.set_cursor_visible(true);
            if let Some(position) = state.restore_cursor {
                let _ = window
                    .set_cursor_position(winit::dpi::PhysicalPosition::new(position.x, position.y));
            }
        }
    }
    if state.mode == NavigationMode::Idle {
        state.restore_cursor = None;
    }
}
