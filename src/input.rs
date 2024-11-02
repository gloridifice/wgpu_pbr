use std::{
    collections::{BTreeMap, HashMap},
    sync::Mutex,
};

use lazy_static::lazy_static;
use winit::{
    event::{ElementState, KeyEvent, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

lazy_static! {
    pub static ref INPUT: Mutex<InputManager> = Mutex::new(InputManager::new());
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    Empty,
    Down,
    Hold,
    Up,
}

pub struct InputManager {
    pub keys: BTreeMap<KeyCode, KeyState>,
}

impl InputManager {
    pub fn new() -> Self {
        InputManager {
            keys: BTreeMap::new(),
        }
    }

    pub fn get_key_state(&self, key: KeyCode) -> KeyState {
        if let Some(key) = self.keys.get(&key) {
            return *key;
        };
        KeyState::Empty
    }

    pub fn is_key_down(&self, key: KeyCode) -> bool {
        return self.get_key_state(key) == KeyState::Down;
    }
    pub fn is_key_up(&self, key: KeyCode) -> bool {
        return self.get_key_state(key) == KeyState::Up;
    }
    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        return self.get_key_state(key) == KeyState::Hold;
    }

    pub fn update(&mut self, event: &WindowEvent) {
        let mut to_change = vec![];
        for (code, state) in self.keys.iter() {
            if *state == KeyState::Up {
                to_change.push(*code);
            }
        }
        for key in to_change {
            self.keys.insert(key, KeyState::Empty);
        }

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
                let last_key_state = self.get_key_state(*key);
                let key_state =
                    if last_key_state == KeyState::Down && *state == ElementState::Pressed {
                        KeyState::Hold
                    } else {
                        match *state {
                            ElementState::Pressed => KeyState::Down,
                            ElementState::Released => KeyState::Up,
                        }
                    };
                self.keys.insert(*key, key_state);
            }
            _ => {}
        };
    }
}
