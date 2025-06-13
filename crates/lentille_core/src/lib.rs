use bevy_ecs::prelude::*;

pub mod input;
pub mod time;
pub mod window;

#[derive(Debug, Component, Clone)]
pub struct Name(pub String);
