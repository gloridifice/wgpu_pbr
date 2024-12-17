use std::sync::Arc;

use crate::egui_tools::{self, EguiRenderer};
use crate::math_type::Vector3Ext;
use bevy_ecs::component::Component;
use bevy_ecs::prelude::Res;
use bevy_ecs::system::{In, InMut, ResMut, Single};
use bevy_ecs::{system::Resource, world::FromWorld};
use cgmath::{perspective, Matrix4};
use wgpu::BufferDescriptor;

use super::transform::{Transform, WorldTransform};

#[derive(Resource)]
pub struct RenderCamera {
    pub buffer: Arc<wgpu::Buffer>,
}

#[derive(Component)]
#[require(Transform)]
pub struct Camera {
    pub aspect: f32,
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
}

#[derive(Component, Default)]
pub struct CameraController {
    pub row: f32,
    pub yaw: f32,
}

impl FromWorld for RenderCamera {
    fn from_world(world: &mut bevy_ecs::world::World) -> Self {
        let rs = world.resource::<crate::RenderState>();
        RenderCamera::new(&rs.device)
    }
}

impl Camera {
    pub fn build_view_projection_matrix(&self, transform: &WorldTransform) -> Matrix4<f32> {
        let view = Matrix4::look_at_rh(
            transform.position.into_point(),
            (transform.position + transform.forward()).into_point(),
            transform.up(),
        );
        let proj = perspective(cgmath::Deg(self.fovy), self.aspect, self.znear, self.zfar);
        return OPENGL_TO_WGPU_MATRIX * proj * view;
    }

    pub fn new(aspect: f32) -> Camera {
        Self {
            aspect,
            fovy: 45.0,
            znear: 0.01,
            zfar: 100.0,
        }
    }

    pub fn get_uniform(&self, transform: &WorldTransform) -> CameraUniform {
        CameraUniform {
            view_proj: self.build_view_projection_matrix(transform).into(),
        }
    }
}

#[derive(Resource)]
pub struct CameraConfig {
    pub speed: f32,
}

impl Default for CameraConfig {
    fn default() -> Self {
        Self { speed: 5.0 }
    }
}

impl RenderCamera {
    pub fn new(device: &wgpu::Device) -> RenderCamera {
        let camera_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Camera Buffer"),
            size: size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        RenderCamera {
            buffer: Arc::new(camera_buffer),
        }
    }

    pub fn update_uniform2gpu(
        &self,
        camera: &Camera,
        transform: &WorldTransform,
        queue: &wgpu::Queue,
    ) {
        queue.write_buffer(
            &self.buffer,
            0,
            bytemuck::cast_slice(&[camera.get_uniform(transform)]),
        );
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
}
unsafe impl bytemuck::Pod for CameraUniform {}
unsafe impl bytemuck::Zeroable for CameraUniform {}

#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.5,
    0.0, 0.0, 0.0, 1.0,
);
