use std::sync::Arc;

use crate::egui_tools::EguiRenderer;
use bevy_ecs::system::Resource;
use bevy_ecs::prelude::Res;
use bevy_ecs::system::ResMut;
use cgmath::{perspective, Matrix4, Point3, Vector3};
use wgpu::util::DeviceExt;

#[derive(Resource)]
pub struct RenderCamera {
    pub camera: Camera,
    pub buffer: Arc<wgpu::Buffer>,
}

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

    pub fn get_uniform(&self) -> CameraUniform {
        CameraUniform {
            view_proj: self.build_view_projection_matrix().into(),
        }
    }
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
    pub fn sys_panel(mut camera_config: ResMut<CameraConfig>, egui_renderer: Res<EguiRenderer>){
        egui::Window::new("Camera").show(egui_renderer.context(), |ui| {
            ui.add(egui::widgets::Slider::new(&mut camera_config.speed, 0.5..=10.0).text("Speed"));
        });
    }
}

impl RenderCamera {
    pub fn new(device: &wgpu::Device, aspect: f32) -> RenderCamera {
        let camera = Camera::new(aspect);

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera.get_uniform()]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        RenderCamera {
            camera,
            buffer: Arc::new(camera_buffer),
        }
    }

    pub fn update_uniform2gpu(&self, queue: &wgpu::Queue) {
        queue.write_buffer(
            &self.buffer,
            0,
            bytemuck::cast_slice(&[self.camera.get_uniform()]),
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
