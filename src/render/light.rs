use std::sync::Arc;

use bevy_ecs::system::Resource;
use cgmath::{InnerSpace, Vector3, Vector4};
use wgpu::{
    util::DeviceExt, BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BufferUsages, ShaderStages,
};

#[derive(Resource)]
pub struct RenderLight {
    pub main_light: MainLight,
    pub buffer: Arc<wgpu::Buffer>,
    pub bind_group_layout: Arc<BindGroupLayout>,
    pub bind_group: Arc<BindGroup>,
}

pub struct MainLight {
    pub direction: Vector3<f32>,
    pub intensity: f32,
    pub color: Vector4<f32>,
}

impl Default for MainLight {
    fn default() -> Self {
        Self {
            direction: Vector3::new(-1.0, -1.0, 0.0).normalize(),
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
    pub intensity: f32,
    pub padding2: [f32; 3],
}

impl RenderLight {
    pub fn new(device: &wgpu::Device) -> Self {
        let main_light = MainLight::default();
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light Uniform Buffer"),
            contents: bytemuck::cast_slice(&[main_light.get_uniform()]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
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
            main_light,
            buffer: Arc::new(buffer),
            bind_group_layout: Arc::new(layout),
            bind_group: Arc::new(bind_group),
        }
    }

    pub fn update_uniform2gpu(&self, queue: &wgpu::Queue) {
        queue.write_buffer(
            &self.buffer,
            0,
            bytemuck::cast_slice(&[self.main_light.get_uniform()]),
        );
    }
}

impl MainLight {
    pub fn get_uniform(&self) -> LightUniform {
        LightUniform {
            direction: self.direction.normalize().into(),
            color: self.color.into(),
            intensity: self.intensity,
            padding2: [0f32; 3],
            padding1: 0.,
        }
    }
}

impl LightUniform {
    const ENTRIES: [BindGroupLayoutEntry; 1] = [BindGroupLayoutEntry {
        binding: 0,
        visibility: ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }];
    pub fn layout_desc() -> BindGroupLayoutDescriptor<'static> {
        BindGroupLayoutDescriptor {
            label: Some("Light Bind Group Layout"),
            entries: &LightUniform::ENTRIES,
        }
    }
}

unsafe impl bytemuck::Pod for LightUniform {}
unsafe impl bytemuck::Zeroable for LightUniform {}
