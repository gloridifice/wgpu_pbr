use bevy_ecs::prelude::*;

pub mod components;

#[derive(Resource)]
pub struct EguiConfig {
    pub egui_scale_factor: f32,
}
impl Default for EguiConfig {
    fn default() -> Self {
        Self {
            egui_scale_factor: 0.8,
        }
    }
}
