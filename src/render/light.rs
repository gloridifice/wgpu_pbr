use std::sync::Arc;

use bevy_ecs::{component::Component, system::Resource};
use cgmath::{Matrix, Matrix4, Vector4};
use wgpu::{BufferDescriptor, BufferUsages};

use super::{camera::OPENGL_TO_WGPU_MATRIX, transform::WorldTransform};

#[derive(Resource)]
pub struct RenderLight {
    // pub main_light: MainLight,
    pub buffer: Arc<wgpu::Buffer>,
}

#[derive(Component)]
pub struct ParallelLight {
    pub intensity: f32,
    pub color: Vector4<f32>,
    pub size: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for ParallelLight {
    fn default() -> Self {
        Self {
            intensity: 1.0,
            color: Vector4::new(0.6, 0.6, 0.5, 1.0),
            size: 3.,
            near: 1.,
            far: 20.,
        }
    }
}

#[repr(C, align(16))]
#[derive(Debug, Clone, Copy)]
pub struct LightUniform {
    pub direction: [f32; 3],
    pub padding1: f32,
    pub color: [f32; 4],
    pub space_matrix: [[f32; 4]; 4],
    pub intensity: f32,
    pub padding2: [f32; 3],
}

impl RenderLight {
    pub fn new(device: &wgpu::Device) -> Self {
        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Light Uniform Buffer"),
            size: size_of::<LightUniform>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            buffer: Arc::new(buffer),
        }
    }

    pub fn write_buffer(&self, queue: &wgpu::Queue, light_uniform: LightUniform) {
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[light_uniform]));
    }
}

impl ParallelLight {
    pub fn get_uniform(&self, transform: &WorldTransform) -> LightUniform {
        LightUniform {
            direction: transform.forward().into(),
            color: self.color.into(),
            intensity: self.intensity,
            padding2: [0f32; 3],
            padding1: 0.,
            space_matrix: self.light_space_matrix(&transform).into(),
        }
    }

    pub fn light_space_matrix(&self, transform: &WorldTransform) -> Matrix4<f32> {
        let size = self.size / 2.;
        let proj = cgmath::ortho::<f32>(-size, size, -size, size, self.near, self.far).transpose();
        let view = transform.view_transform();
        OPENGL_TO_WGPU_MATRIX * proj * view
    }
}

unsafe impl bytemuck::Pod for LightUniform {}
unsafe impl bytemuck::Zeroable for LightUniform {}
