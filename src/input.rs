use std::{collections::HashSet, sync::Mutex};

use lazy_static::lazy_static;
use winit::{
    event::{ElementState, KeyEvent, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

lazy_static! {
    pub static ref INPUT: Mutex<GameInput> = Mutex::new(GameInput::new());
}

pub struct GameInput {
    pub down_keys: HashSet<KeyCode>,
    pub hold_keys: HashSet<KeyCode>,
    pub up_keys: HashSet<KeyCode>,
}

impl GameInput {
    pub fn new() -> Self {
        GameInput {
            down_keys: HashSet::with_capacity(100),
            hold_keys: HashSet::with_capacity(100),
            up_keys: HashSet::with_capacity(100),
        }
    }

    pub fn is_key_down(&self, key: KeyCode) -> bool {
        return self.down_keys.contains(&key);
    }
    pub fn is_key_up(&self, key: KeyCode) -> bool {
        return self.up_keys.contains(&key);
    }
    pub fn is_key_hold(&self, key: KeyCode) -> bool {
        return self.hold_keys.contains(&key);
    }

    pub fn update(&mut self, event: &WindowEvent) {
        self.down_keys.clear();
        self.up_keys.clear();

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
}
