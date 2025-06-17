use std::sync::Arc;

use bevy_app::{Plugin, PostUpdate};
use bevy_ecs::component::Component;
use bevy_ecs::prelude::*;
use bevy_ecs::query::{Changed, Or};
use bevy_ecs::system::Single;
use bevy_ecs::{prelude::Resource, world::FromWorld};
use cgmath::{Matrix4, SquareMatrix, perspective};
use lentille_wgpu_utils::impl_pod_zeroable;
use wgpu::BufferDescriptor;

use crate::RenderState;

use super::transform::{Transform, WorldTransform};

pub(crate) struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_resource::<CameraBuffer>()
            .add_systems(PostUpdate, sys_update_camera_uniform);
    }
}

#[derive(Resource)]
pub struct CameraBuffer {
    pub buffer: Arc<wgpu::Buffer>,
}

#[derive(Component, Clone)]
#[require(Transform)]
pub struct Camera {
    // Height / Width
    pub aspect: f32,
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
    pub view_proj: Matrix4<f32>,
}

impl FromWorld for CameraBuffer {
    fn from_world(world: &mut bevy_ecs::world::World) -> Self {
        let rs = world.resource::<crate::RenderState>();
        CameraBuffer::new(&rs.device)
    }
}

impl Camera {
    pub fn build_view_projection_matrix(&self, transform: &WorldTransform) -> Matrix4<f32> {
        let view = transform.view_matrix();
        let proj = perspective(cgmath::Deg(self.fovy), self.aspect, self.znear, self.zfar);
        OPENGL_TO_WGPU_MATRIX * proj * view
    }

    pub fn new(aspect: f32) -> Camera {
        Self {
            aspect,
            fovy: 45.0,
            znear: 0.01,
            zfar: 100.0,
            view_proj: Matrix4::identity(),
        }
    }

    pub fn get_uniform(&self, transform: &WorldTransform) -> CameraUniform {
        let pos = transform.position;
        let dir = transform.forward();
        CameraUniform {
            view_proj: self.build_view_projection_matrix(transform).into(),
            position: [pos.x, pos.y, pos.z, 1.],
            direction: [dir.x, dir.y, dir.z, 1.],
        }
    }
}

impl CameraBuffer {
    pub fn new(device: &wgpu::Device) -> CameraBuffer {
        let camera_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Camera Buffer"),
            size: size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        CameraBuffer {
            buffer: Arc::new(camera_buffer),
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub position: [f32; 4],
    pub direction: [f32; 4],
}

impl_pod_zeroable!(CameraUniform);

#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.5,
    0.0, 0.0, 0.0, 1.0,
);

pub fn sys_update_camera_uniform(
    camera_buffer: Res<CameraBuffer>,
    single: Single<(&mut Camera, &WorldTransform), Or<(Changed<Camera>, Changed<WorldTransform>)>>,
    rs: Res<RenderState>,
) {
    let (mut camera, transform) = single.into_inner();

    camera.view_proj = camera.build_view_projection_matrix(transform);

    rs.queue.write_buffer(
        &camera_buffer.buffer,
        0,
        bytemuck::cast_slice(&[camera.get_uniform(transform)]),
    );
}
