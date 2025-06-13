use bevy_ecs::prelude::*;

pub mod input;
pub mod time;

#[derive(Debug, Component, Clone)]
pub struct Name(pub String);
