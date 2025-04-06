use std::collections::HashSet;

use bevy_ecs::{
    system::{ResMut, Resource},
    world::FromWorld,
};
use winit::{
    event::{DeviceEvent, ElementState, KeyEvent, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

use crate::cgmath_ext::{Vec2, VectorExt};

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

    pub fn window_input(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state,
                        physical_key: PhysicalKey::Code(key),
                        ..
                    },
                ..
            } => {
                match *state {
                    ElementState::Pressed => {
                        if !self.is_key_hold(*key) {
                            self.down_keys.insert(*key);
                        }
                        self.hold_keys.insert(*key);
                    }
                    ElementState::Released => {
                        if self.is_key_hold(*key) {
                            self.up_keys.insert(*key);
                        }
                        self.hold_keys.remove(key);
                    }
                };
            }
            _ => {}
        };
    }

    pub fn device_input(&mut self, event: &DeviceEvent) {
        match event {
            DeviceEvent::MouseMotion { delta } => {
                self.cursor_delta = Vec2::new(delta.0 as f32, delta.1 as f32);
            }
            _ => {}
        }
    }

    pub fn sys_pre_update(mut input: ResMut<Input>) {
        input.cursor_offset = input.cursor_position - input.last_cursor_position;
        input.last_cursor_position = input.cursor_position;
    }

    pub fn sys_post_update(mut input: ResMut<Input>) {
        input.down_keys.clear();
        input.up_keys.clear();
        input.down_cursor_buttons.clear();
        input.cursor_delta = Vec2::zero();
    }
}
