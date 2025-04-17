use bevy_ecs::prelude::*;
use std::sync::Arc;

use crate::{asset::AssetPath, render::prelude::*};

use super::{
    defered_rendering::global_binding::GlobalBindGroup, light::DynamicLightBindGroup,
    material::pbr::PBRMaterialBindGroupLayout, shader_loader::ShaderLoader,
};

/// Transparent 是一个不进行深度写入，但是使用 Opaque 阶段的深度图进行深度测试的 Pipeline
/// Transparent Pipeline 不在延迟渲染管线内，会进行单独地按顺序地渲染。
#[derive(Resource)]
pub struct TransparentPipeline {
    pub pipeline: Arc<RenderPipeline>,
    #[allow(unused)]
    pub layout: Arc<PipelineLayout>,
}

impl FromWorld for TransparentPipeline {
    fn from_world(world: &mut World) -> Self {
        let mut shader = world.resource_mut::<ShaderLoader>();
        let shader_source = shader
            .load_source(AssetPath::new_shader_wgsl("transparent_pbr_main.wgsl"))
            .unwrap();

        let rs = world.resource::<RenderState>();
        let device = &rs.device;
        let config = &rs.config;
        let global_bind_group = world.resource::<GlobalBindGroup>();
        let material_bind_group = world.resource::<PBRMaterialBindGroupLayout>();
        let object_bind_group = world.resource::<ObjectBindGroupLayout>();
        let dynamic_light = world.resource::<DynamicLightBindGroup>();

        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: shader_source,
        });

        let bind_group_layouts = vec![
            global_bind_group.layout.as_ref(),
            material_bind_group.0.as_ref(),
            object_bind_group.0.as_ref(),
            dynamic_light.layout.as_ref(),
        ];

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Transparent Pipeline"),
            bind_group_layouts: &bind_group_layouts,
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Transparent Pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: RenderState::DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        Self {
            pipeline: Arc::new(pipeline),
            layout: Arc::new(layout),
        }
    }
}
