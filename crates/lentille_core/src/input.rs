use lentille_math::*;
use std::collections::HashSet;

use bevy_app::{Plugin, PostUpdate, PreUpdate};
use bevy_ecs::prelude::*;
use winit::{
    event::{DeviceEvent, KeyEvent, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

use crate::window::{WinitDeviceEvent, WinitWindowEvent};

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_resource::<Input>()
            .add_systems(PreUpdate, Input::sys_pre_update)
            .add_systems(PostUpdate, Input::sys_post_update)
            .add_observer(Input::sys_on_device_input)
            .add_observer(Input::sys_on_window_input);
    }
}

#[derive(Resource)]
pub struct Input {
    pub down_keys: HashSet<KeyCode>,
    pub hold_keys: HashSet<KeyCode>,
    pub up_keys: HashSet<KeyCode>,
    pub last_cursor_position: Vec2,
    pub cursor_position: Vec2,
    /// Will always be zero when curosr is locked. Use `cursor_delta` instead.
    pub cursor_offset: Vec2,
    pub down_cursor_buttons: HashSet<CursorButton>,
    /// Ignore cursor locking. From device input.
    pub cursor_delta: Vec2,
}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub enum CursorButton {
    Left,
    Middle,
    Right,
}

impl FromWorld for Input {
    fn from_world(_world: &mut bevy_ecs::world::World) -> Self {
        Input::new()
    }
}

impl Input {
    pub fn new() -> Self {
        Input {
            down_keys: HashSet::with_capacity(100),
            hold_keys: HashSet::with_capacity(100),
            up_keys: HashSet::with_capacity(100),
            last_cursor_position: Vec2::zero(),
            cursor_position: Vec2::zero(),
            cursor_offset: Vec2::zero(),
            down_cursor_buttons: HashSet::with_capacity(8),
            cursor_delta: Vec2::zero(),
        }
    }

    #[allow(unused)]
    pub fn is_key_down(&self, key: KeyCode) -> bool {
        self.down_keys.contains(&key)
    }

    #[allow(unused)]
    pub fn is_key_up(&self, key: KeyCode) -> bool {
        self.up_keys.contains(&key)
    }

    pub fn is_key_hold(&self, key: KeyCode) -> bool {
        self.hold_keys.contains(&key)
    }

    #[allow(unused)]
    pub fn is_cursor_button_down(&self, button: CursorButton) -> bool {
        self.down_cursor_buttons.contains(&button)
    }

    fn sys_on_window_input(event: On<WinitWindowEvent>, mut input: ResMut<Input>) {
        let event = &event.window_event;
        if let WindowEvent::KeyboardInput {
            event:
                KeyEvent {
                    state,
                    physical_key: PhysicalKey::Code(key),
                    ..
                },
            ..
        } = event
        {
            if state.is_pressed() {
                if !input.is_key_hold(*key) {
                    input.down_keys.insert(*key);
                }
                input.hold_keys.insert(*key);
            } else {
                if input.is_key_hold(*key) {
                    input.up_keys.insert(*key);
                }
                input.hold_keys.remove(key);
            };
        };
    }

    fn sys_on_device_input(event: On<WinitDeviceEvent>, mut input: ResMut<Input>) {
        if let DeviceEvent::MouseMotion { ref delta } = event.device_event {
            input.cursor_delta = Vec2::new(delta.0 as f32, delta.1 as f32);
        }
    }

    fn sys_pre_update(mut input: ResMut<Input>) {
        input.cursor_offset = input.cursor_position - input.last_cursor_position;
        input.last_cursor_position = input.cursor_position;
    }

    fn sys_post_update(mut input: ResMut<Input>) {
        input.down_keys.clear();
        input.up_keys.clear();
        input.down_cursor_buttons.clear();
        input.cursor_delta = Vec2::zero();
    }
}
