use std::sync::Arc;

use bevy_ecs::{system::Resource, world::World};
use cgmath::{perspective, InnerSpace, Matrix4, Point3, SquareMatrix, Vector3};
use egui::{InputState, Ui};
use tiny_bail::or_return;
use wgpu::{BindGroup, BindGroupLayout, BindGroupLayoutDescriptor, Queue};
use winit::keyboard::KeyCode;

use crate::{
    bevy_ecs_ext::BevyEcsExt, egui_tools::EguiRenderer, input::GameInput, time::GameTime, State,
};

pub struct Camera {
    pub eye: Point3<f32>,
    pub target: Point3<f32>,
    pub up: Vector3<f32>,
    pub aspect: f32,
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
}

impl Camera {
    pub fn build_view_projection_matrix(&self) -> Matrix4<f32> {
        let view = Matrix4::look_at_rh(self.eye, self.target, self.up);
        let proj = perspective(cgmath::Deg(self.fovy), self.aspect, self.znear, self.zfar);
        return OPENGL_TO_WGPU_MATRIX * proj * view;
    }

    pub fn new(aspect: f32) -> Camera {
        Self {
            eye: Point3::new(0.0, 1.0, 2.0),
            target: Point3::new(0.0, 0.0, 0.0),
            up: Vector3::new(0.0, 1.0, 0.0),
            aspect,
            fovy: 45.0,
            znear: 0.01,
            zfar: 1000.0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}
impl CameraUniform {
    pub fn new() -> Self {
        Self {
            view_proj: Matrix4::identity().into(),
        }
    }

    pub fn update_view_proj(&mut self, camera: &Camera) {
        self.view_proj = camera.build_view_projection_matrix().into();
    }

    pub fn layout_desc() -> BindGroupLayoutDescriptor<'static> {
        BindGroupLayoutDescriptor {
            label: Some("Camera Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        }
    }
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

pub struct RenderCamera {
    pub camera: Camera,
    pub camera_uniform: CameraUniform,
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group_layout: Arc<BindGroupLayout>,
    pub camera_bind_group: Arc<BindGroup>,
}

#[derive(Resource)]
pub struct CameraConfig {
    pub speed: f32,
}
impl Default for CameraConfig {
    fn default() -> Self {
        Self { speed: 1.0 }
    }
}

impl CameraConfig {
    pub fn panel(world: &mut World, egui_renderer: &EguiRenderer) {
        let mut camera_config = world.resource_or_default::<CameraConfig>();

        egui::Window::new("Camera").show(egui_renderer.context(), |ui| {
            ui.add(egui::widgets::Slider::new(&mut camera_config.speed, 0.5..=10.0).text("Speed"));
        });
    }
}

impl RenderCamera {
    pub fn camera_update(
        &mut self,
        world: &mut World,
        queue: &Queue,
        input: &GameInput,
        time: &GameTime,
    ) {
        let speed = world.resource_or_default::<CameraConfig>().speed;

        let mut move_vec = Vector3::new(0., 0., 0.);
        if input.is_key_hold(KeyCode::KeyW) {
            move_vec += Vector3::new(0.0, 0.0, -1.0);
        }
        if input.is_key_hold(KeyCode::KeyA) {
            move_vec += Vector3::new(-1.0, 0.0, 0.0);
        }
        if input.is_key_hold(KeyCode::KeyS) {
            move_vec += Vector3::new(0.0, 0.0, 1.0);
        }
        if input.is_key_hold(KeyCode::KeyD) {
            move_vec += Vector3::new(1.0, 0.0, 0.0);
        }
        if input.is_key_hold(KeyCode::Space) {
            if input.is_key_hold(KeyCode::ShiftLeft) {
                move_vec += Vector3::new(0.0, -1.0, 0.0);
            } else {
                move_vec += Vector3::new(0.0, 1.0, 1.0);
            }
        }
        if move_vec != Vector3::new(0., 0., 0.) {
            move_vec = move_vec.normalize() * speed * time.delta_time.as_secs_f32();
            self.camera.eye += move_vec;
            self.camera.target += move_vec;

            self.camera_uniform.update_view_proj(&self.camera);
            queue.write_buffer(
                &self.camera_buffer,
                0,
                bytemuck::cast_slice(&[self.camera_uniform]),
            );
        }
    }
}
