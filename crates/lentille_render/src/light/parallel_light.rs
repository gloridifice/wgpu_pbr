use crate::{camera::OPENGL_TO_WGPU_MATRIX, prelude::*};
use bevy_ecs::prelude::*;
use cgmath::Matrix4;

#[derive(Component)]
pub struct ParallelLight {
    pub intensity: f32,
    pub color: Color,
    pub size: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for ParallelLight {
    fn default() -> Self {
        Self {
            intensity: 1.0,
            color: Color::new(0.6, 0.6, 0.5, 1.0),
            size: 64.,
            near: 1.,
            far: 20.,
        }
    }
}

impl ParallelLight {
    pub fn light_space_matrix(&self, transform: &WorldTransform) -> Matrix4<f32> {
        let half_size = self.size / 2.;
        let proj = cgmath::ortho::<f32>(
            -half_size, half_size, -half_size, half_size, self.near, self.far,
        );
        let view = transform.view_matrix();
        OPENGL_TO_WGPU_MATRIX * proj * view
    }
}
