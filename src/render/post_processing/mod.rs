use std::{collections::HashMap, sync::Arc};

use bevy_ecs::prelude::*;
use lazy_static::lazy_static;
use wgpu::{
    BindGroup, BindGroupLayout, BindingResource, Device, PipelineLayout, PipelineLayoutDescriptor,
    RenderPipeline, ShaderModule, ShaderStages, SurfaceConfiguration, VertexState,
};

use crate::{bg_descriptor, bg_layout_descriptor, render::BGLEntry, RenderState};

use super::{create_color_render_target_image, RenderTargetSize, UploadedImage};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum RenderStage {
    BeforeOpaque,
    AfterOpaque,
    BeforeTransparent,
    AfterTransparent,
}

#[derive(Resource)]
pub struct PostProcessingManager {
    pub pipelines: HashMap<RenderStage, Vec<PostProcessingPipeline>>,
    pub pipeline_layout: Arc<PipelineLayout>,
    #[allow(unused)]
    pub bind_group_layout: Arc<BindGroupLayout>,
    bind_group_0: Arc<BindGroup>,
    bind_group_1: Arc<BindGroup>,
    temp_texture_0: Arc<UploadedImage>,
    temp_texture_1: Arc<UploadedImage>,
    temp_texture_index: usize,
    pub vs_shader: Arc<ShaderModule>,
}

impl PostProcessingManager {
    pub fn get_current_source_texture(&self) -> Arc<UploadedImage> {
        match self.temp_texture_index {
            0 => Arc::clone(&self.temp_texture_0),
            _ => Arc::clone(&self.temp_texture_1),
        }
    }
    pub fn get_current_target_texture(&self) -> Arc<UploadedImage> {
        match self.temp_texture_index {
            0 => Arc::clone(&self.temp_texture_1),
            _ => Arc::clone(&self.temp_texture_0),
        }
    }
    pub fn next_source_and_target(&mut self) -> (Arc<BindGroup>, Arc<UploadedImage>) {
        let ret = match self.temp_texture_index {
            0 => (
                Arc::clone(&self.bind_group_0),
                Arc::clone(&self.temp_texture_1),
            ),
            _ => (
                Arc::clone(&self.bind_group_1),
                Arc::clone(&self.temp_texture_0),
            ),
        };
        self.temp_texture_index = (self.temp_texture_index + 1) % 2;
        ret
    }
    pub fn add_pipeline_from_shader(
        &mut self,
        label: Option<&str>,
        stage: RenderStage,
        fs_shader: ShaderModule,
        device: &Device,
        config: &SurfaceConfiguration,
    ) {
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label,
            layout: Some(&self.pipeline_layout),
            vertex: VertexState {
                module: &self.vs_shader,
                entry_point: "vs_main",
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: 0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &fs_shader,
                entry_point: "fs_main",
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        });

        self.pipelines
            .entry(stage)
            .or_insert(Vec::new())
            .push(PostProcessingPipeline {
                pipeline: Arc::new(pipeline),
            });
    }

    pub fn resize(&mut self, width: u32, height: u32, device: &Device, config: &SurfaceConfiguration) {
        let bind_group_layout = &self.bind_group_layout;
        self.temp_texture_0 = Arc::new(create_color_render_target_image(
            width,
            height,
            device,
            config,
        ));
        self.temp_texture_1 = Arc::new(create_color_render_target_image(
            width,
            height,
            device,
            config,
        ));

        self.bind_group_0 = Arc::new(device.create_bind_group(&bg_descriptor! {
            ["Post Processing"] [&bind_group_layout]
            0: BindingResource::TextureView(&self.temp_texture_0.view);
            1: BindingResource::Sampler(&self.temp_texture_0.sampler);
        }));
        self.bind_group_1 = Arc::new(device.create_bind_group(&bg_descriptor! {
            ["Post Processing"] [&bind_group_layout]
            0: BindingResource::TextureView(&self.temp_texture_1.view);
            1: BindingResource::Sampler(&self.temp_texture_1.sampler);
        }));
    }
}

impl FromWorld for PostProcessingManager {
    fn from_world(world: &mut World) -> Self {
        let rs = world.resource::<crate::RenderState>();

        let vs_shader = rs.device.create_shader_module(wgpu::include_wgsl!(
            "../../../assets/shaders/post_processing_vert.wgsl"
        ));
        let descriptor = bg_layout_descriptor! {
            ["Post Processing"]
            0: ShaderStages::FRAGMENT => BGLEntry::Tex2D(false, wgpu::TextureSampleType::Float { filterable: true });
            1: ShaderStages::FRAGMENT => BGLEntry::Sampler(wgpu::SamplerBindingType::Filtering);
        };
        let bind_group_layout = rs.device.create_bind_group_layout(&descriptor);

        let size = world.resource::<RenderTargetSize>();
        let temp_texture_0 = Arc::new(create_color_render_target_image(
            size.width,
            size.height,
            &rs.device,
            &rs.config,
        ));
        let temp_texture_1 = Arc::new(create_color_render_target_image(
            size.width,
            size.height,
            &rs.device,
            &rs.config,
        ));

        let bind_group_0 = Arc::new(rs.device.create_bind_group(&bg_descriptor! {
            ["Post Processing"] [&bind_group_layout]
            0: BindingResource::TextureView(&temp_texture_0.view);
            1: BindingResource::Sampler(&temp_texture_0.sampler);
        }));
        let bind_group_1 = Arc::new(rs.device.create_bind_group(&bg_descriptor! {
            ["Post Processing"] [&bind_group_layout]
            0: BindingResource::TextureView(&temp_texture_1.view);
            1: BindingResource::Sampler(&temp_texture_1.sampler);
        }));

        let pipeline_layout = rs.device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Post Processing"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        Self {
            pipelines: HashMap::new(),
            bind_group_layout: Arc::new(bind_group_layout),
            bind_group_0,
            bind_group_1,
            pipeline_layout: Arc::new(pipeline_layout),
            temp_texture_0,
            temp_texture_1,
            vs_shader: Arc::new(vs_shader),
            temp_texture_index: 0,
        }
    }
}

#[derive(Clone)]
pub struct PostProcessingPipeline {
    pub pipeline: Arc<RenderPipeline>,
}

// #[repr(C)]
// #[derive(Clone, Copy, Debug, Default)]
// pub struct ScreenVertex {
//     pub position: [f32; 3],
// }
// unsafe impl bytemuck::Zeroable for ScreenVertex {}
// unsafe impl bytemuck::Pod for ScreenVertex {}
// impl ScreenVertex {
//     const ATTRIBS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x3];
//     pub fn desc() -> wgpu::VertexBufferLayout<'static> {
//         wgpu::VertexBufferLayout {
//             array_stride: std::mem::size_of::<ScreenVertex>() as wgpu::BufferAddress,
//             step_mode: wgpu::VertexStepMode::Vertex,
//             attributes: &Self::ATTRIBS,
//         }
//     }
//     pub fn new(x: f32, y: f32, z: f32) -> Self {
//         Self {
//             position: [x, y, z],
//         }
//     }
// }

// lazy_static! {
//     pub static ref SCREEN_VERTEX: [ScreenVertex; 4] = [
//         ScreenVertex::new(0., 0., 0.),
//         ScreenVertex::new(0., 0., 0.),
//         ScreenVertex::new(0., 0., 0.),
//         ScreenVertex::new(0., 0., 0.),
//     ];
// }
