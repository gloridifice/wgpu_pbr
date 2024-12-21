use std::sync::Arc;

use bevy_ecs::{component::Component, system::Resource};
use cgmath::{Deg, Matrix, Matrix4, SquareMatrix, Vector4};
use wgpu::{BufferDescriptor, BufferUsages};

use crate::math_type::Vector3Ext;

use super::{camera::OPENGL_TO_WGPU_MATRIX, transform::WorldTransform};

#[derive(Resource)]
pub struct RenderLight {
    // pub main_light: MainLight,
    pub buffer: Arc<wgpu::Buffer>,
}

#[derive(Component)]
pub struct MainLight {
    pub intensity: f32,
    pub color: Vector4<f32>,
    pub fov: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for MainLight {
    fn default() -> Self {
        Self {
            intensity: 1.0,
            color: Vector4::new(0.6, 0.6, 0.5, 1.0),
            fov: 120.,
            near: 1.,
            far: 100.,
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

impl MainLight {
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
        // let proj = cgmath::perspective(Deg(self.fov), 1.0, self.near, self.far);
        // let proj = ortho(1., 10., 0.1, 100.);
        let size = 3.;
        let proj = cgmath::ortho::<f32>(-size, size, -size, size, 0.1, 20.).transpose();
        let view = transform.model_matrix().invert().unwrap();
        OPENGL_TO_WGPU_MATRIX * proj * view
    }
}

unsafe impl bytemuck::Pod for LightUniform {}
unsafe impl bytemuck::Zeroable for LightUniform {}
