use std::sync::Arc;

use bevy_ecs::{component::Component, system::Resource};
use cgmath::{Matrix4, Vector4};
use wgpu::{
    BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry,
    BufferDescriptor, BufferUsages, ShaderStages,
};

use crate::{bgl_entries, macro_utils::BGLEntry};

use super::transform::WorldTransform;

#[derive(Resource)]
pub struct RenderLight {
    // pub main_light: MainLight,
    pub buffer: Arc<wgpu::Buffer>,
    pub bind_group_layout: Arc<BindGroupLayout>,
    pub bind_group: Arc<BindGroup>,
}

#[derive(Component)]
pub struct MainLight {
    pub intensity: f32,
    pub color: Vector4<f32>,
}

impl Default for MainLight {
    fn default() -> Self {
        Self {
            intensity: 1.0,
            color: Vector4::new(0.6, 0.6, 0.5, 1.0),
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
        let layout = device.create_bind_group_layout(&LightUniform::layout_desc());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Light Uniform Bind Group"),
            layout: &layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        Self {
            buffer: Arc::new(buffer),
            bind_group_layout: Arc::new(layout),
            bind_group: Arc::new(bind_group),
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
            space_matrix: Self::light_space_matrix(&transform).into(),
        }
    }

    pub fn light_space_matrix(transform: &WorldTransform) -> Matrix4<f32> {
        let proj = cgmath::ortho::<f32>(-10., 10., -10., 10., 0.1, 1000.);
        let view = transform.model_matrix();
        view * proj
    }
}

impl LightUniform {
    const ENTRIES: [BindGroupLayoutEntry; 1] = bgl_entries! {
        0: ShaderStages::all() => BGLEntry::UniformBuffer();
    };

    pub fn layout_desc() -> BindGroupLayoutDescriptor<'static> {
        BindGroupLayoutDescriptor {
            label: Some("Light Bind Group Layout"),
            entries: &LightUniform::ENTRIES,
        }
    }
}

unsafe impl bytemuck::Pod for LightUniform {}
unsafe impl bytemuck::Zeroable for LightUniform {}
