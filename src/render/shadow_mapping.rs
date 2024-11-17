use std::sync::Arc;

use bevy_ecs::system::Resource;
use cgmath::{Matrix4, SquareMatrix};
use wgpu::{
    util::DeviceExt, BindGroup, BindGroupEntry, BindGroupLayout, Buffer, BufferUsages,
    PipelineLayout, Queue, RenderPipeline,
};

use crate::RenderState;

use super::{transform::Transform, UploadedImage, Vertex};

#[derive(Resource)]
pub struct ShadowMappingContext {
    pub light_depth_map: UploadedImage,
    pub pipeline: RenderPipeline,
    pub layout: Arc<PipelineLayout>,
    pub light_space_buffer: Arc<Buffer>,
    pub light_space_bind_group: Arc<BindGroup>,
    pub light_space_bind_group_layout: Arc<BindGroupLayout>,
}

impl ShadowMappingContext {
    pub fn new(
        device: &wgpu::Device,
        transform_bg_layout: &BindGroupLayout,
        width: u32,
        height: u32,
    ) -> Self {
        let texture = RenderState::create_depth_texture(device, width, height);

        let matrix: [[f32; 4]; 4] = Matrix4::<f32>::identity().into();

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light space buffer"),
            contents: bytemuck::cast_slice(&[matrix]),
            usage: BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Light space bind group"),
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
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Light space bind group"),
            layout: &layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Shadow mapping pipeline"),
            bind_group_layouts: &[transform_bg_layout, &layout],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(wgpu::include_wgsl!("../../assets/shaders/light_depth_map.wgsl"));

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Shadow mapping"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Vertex::desc()],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: RenderState::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: None,
            multiview: None,
            cache: None,
        });

        Self {
            light_depth_map: texture,
            light_space_buffer: Arc::new(buffer),
            pipeline,
            layout: Arc::new(pipeline_layout),
            light_space_bind_group: Arc::new(bind_group),
            light_space_bind_group_layout: Arc::new(layout),
        }
    }

    pub fn light_space_matirx(tranform: Transform) -> Matrix4<f32> {
        let proj = cgmath::ortho::<f32>(-10., 10., -10., 10., 0.1, 1000.);
        let view = tranform.local_matrix().0;
        view * proj
    }

    pub fn write_buffer(&self, queue: &Queue, matrix: [[f32; 4]; 4]) {
        queue.write_buffer(&self.light_space_buffer, 0, bytemuck::cast_slice(&[matrix]));
    }
}
