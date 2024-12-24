use std::collections::HashSet;

use bevy_ecs::{
    system::{ResMut, Resource},
    world::FromWorld,
};
use winit::{
    event::{ElementState, KeyEvent, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

use crate::math_type::{Vec2, VectorExt};

#[derive(Resource)]
pub struct Input {
    pub down_keys: HashSet<KeyCode>,
    pub hold_keys: HashSet<KeyCode>,
    pub up_keys: HashSet<KeyCode>,
    pub last_cursor_position: Vec2,
    pub cursor_position: Vec2,
    pub cursor_offset: Vec2,
    pub down_cursor_buttons: HashSet<CursorButton>
}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub enum CursorButton {
    Left, Middle, Right
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
        }
    }

    #[allow(unused)]
    pub fn is_key_down(&self, key: KeyCode) -> bool {
        return self.down_keys.contains(&key);
    }

    #[allow(unused)]
    pub fn is_key_up(&self, key: KeyCode) -> bool {
        return self.up_keys.contains(&key);
    }

    pub fn is_key_hold(&self, key: KeyCode) -> bool {
        return self.hold_keys.contains(&key);
    }

    #[allow(unused)]
    pub fn is_cursor_button_down(&self, button: CursorButton) -> bool {
        return self.down_cursor_buttons.contains(&button);
    }

    pub fn input(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::CursorMoved {  .. } => {
                // self.cursor_position = Vec2::new(position.x as f32, position.y as f32);
            }
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

    pub fn sys_pre_update(mut input: ResMut<Input>) {
        input.cursor_offset = input.cursor_position - input.last_cursor_position;
        input.last_cursor_position = input.cursor_position;
    }
    pub fn sys_post_update(mut input: ResMut<Input>) {
        input.down_keys.clear();
        input.up_keys.clear();
        input.down_cursor_buttons.clear();
    }
}
