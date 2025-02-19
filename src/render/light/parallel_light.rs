use crate::render::{camera::OPENGL_TO_WGPU_MATRIX, prelude::*};
use bevy_ecs::prelude::*;
use cgmath::{Matrix, Matrix4};

#[derive(Component)]
pub struct ParallelLight {
    pub intensity: f32,
    pub color: Vec4,
    pub size: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for ParallelLight {
    fn default() -> Self {
        Self {
            intensity: 1.0,
            color: Vec4::new(0.6, 0.6, 0.5, 1.0),
            size: 10.,
            near: 1.,
            far: 20.,
        }
    }
}

impl ParallelLight {
    pub fn light_space_matrix(&self, transform: &WorldTransform) -> Matrix4<f32> {
        let size = self.size / 2.;
        let proj = cgmath::ortho::<f32>(-size, size, -size, size, self.near, self.far).transpose();
        let view = transform.view_matrix();
        OPENGL_TO_WGPU_MATRIX * proj * view
    }
}
