use std::sync::Arc;

use bevy_ecs::component::Component;
use bevy_ecs::prelude::*;
use bevy_ecs::query::{Changed, Or};
use bevy_ecs::system::Single;
use bevy_ecs::{system::Resource, world::FromWorld};
use cgmath::{
    perspective, vec2, Deg, InnerSpace, Matrix4, Quaternion, Rotation3, SquareMatrix, Vector3,
};
use wgpu::BufferDescriptor;
use winit::keyboard::KeyCode;

use crate::engine::input::Input;
use crate::engine::time::Time;
use crate::engine_lifetime::ControlState;
use crate::{impl_pod_zeroable, RenderState};

use super::transform::{Transform, WorldTransform};

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

#[derive(Component, Clone, Default)]
pub struct CameraController {
    pub row: f32,
    pub yaw: f32,
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

#[derive(Resource)]
pub struct CameraConfig {
    pub speed: f32,
}

impl Default for CameraConfig {
    fn default() -> Self {
        Self { speed: 5.0 }
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

pub fn sys_update_camera_control(
    config: Res<CameraConfig>,
    input: Res<Input>,
    time: Res<Time>,
    control_state: Res<ControlState>,
    camera_query: Single<(
        &Camera,
        &mut Transform,
        &WorldTransform,
        &mut CameraController,
    )>,
) {
    if !control_state.is_focused {
        return;
    }

    let (_, mut cam_transform, world_trans, mut controller) = camera_query.into_inner();

    let speed = config.speed;

    let mut move_vec = Vector3::new(0., 0., 0.);
    if input.is_key_hold(KeyCode::KeyW) {
        move_vec += world_trans.forward();
    }
    if input.is_key_hold(KeyCode::KeyA) {
        move_vec += world_trans.left();
    }
    if input.is_key_hold(KeyCode::KeyS) {
        move_vec -= world_trans.forward();
    }
    if input.is_key_hold(KeyCode::KeyD) {
        move_vec -= world_trans.left();
    }
    if input.is_key_hold(KeyCode::Space) {
        if input.is_key_hold(KeyCode::ShiftLeft) {
            move_vec += Vector3::new(0.0, -1.0, 0.0);
        } else {
            move_vec += Vector3::new(0.0, 1.0, 1.0);
        }
    }
    let delta_time_sec = time.delta_time.as_secs_f32();
    if move_vec != Vector3::new(0., 0., 0.) {
        move_vec = move_vec.normalize() * speed * delta_time_sec;
        cam_transform.position += move_vec;
    }

    let factor = vec2(0.6, 0.4);
    controller.row -= input.cursor_delta.x * factor.x;
    controller.yaw = (controller.yaw - input.cursor_delta.y * factor.y).clamp(-40.0, 80.0);
    cam_transform.rotation = Quaternion::from_angle_y(Deg(controller.row))
        * Quaternion::from_angle_x(Deg(controller.yaw));
}

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
