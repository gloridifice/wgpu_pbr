use std::sync::Arc;

use bevy_ecs::{
    system::Resource,
    world::{FromWorld, World},
};
use wgpu::{
    BindGroup, BindGroupLayout, BindingResource, PipelineLayout, RenderPipeline,
    SamplerBindingType, ShaderStages, TextureSampleType,
};

use crate::{bg_descriptor, bg_layout_descriptor, macro_utils::BGLEntry, RenderState};

use super::{
    GltfMaterial, MaterialBindGroupLayout, ObjectBindGroupLayout, Vertex
};

#[derive(Resource)]
pub struct MainPipeline {
    pub pipeline: RenderPipeline,
    pub pipeline_layout: PipelineLayout,
    pub bind_group_layouts: Vec<Arc<BindGroupLayout>>,
    pub material_bind_group_layout: Arc<BindGroupLayout>,
}

impl FromWorld for MainPipeline {
    fn from_world(world: &mut bevy_ecs::world::World) -> Self {
        let rs = world.resource::<RenderState>();

        let device = &rs.device;
        let shader =
            device.create_shader_module(wgpu::include_wgsl!("../../assets/shaders/shader.wgsl"));

        let vert = ShaderStages::VERTEX;
        let frag = ShaderStages::FRAGMENT;
        let both = ShaderStages::all();

        let global_bind_group_layout =
            Arc::new(device.create_bind_group_layout(&bg_layout_descriptor! (
                ["Global Bind Group Layout"]
                0: vert => BGLEntry::UniformBuffer(); // Camera Uniform
                1: both => BGLEntry::UniformBuffer(); // Global Light Uniform
                2: frag => BGLEntry::Tex2D(false, TextureSampleType::Depth); // Shadow Map
                3: frag => BGLEntry::Sampler(SamplerBindingType::Comparison); // Shadow Map
            )));

        let material_bind_group_layout = Arc::clone(&world.resource::<MaterialBindGroupLayout>().0);
        let object_bind_group_layout = Arc::clone(&world.resource::<ObjectBindGroupLayout>().0);

        let bind_group_layouts = vec![
            global_bind_group_layout,
            Arc::clone(&material_bind_group_layout),
            object_bind_group_layout,
        ];

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &bind_group_layouts
                    .iter()
                    .map(|it| it.as_ref())
                    .collect::<Vec<_>>(),
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: rs.config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            //The `primitive` field describes how to interpret our vertices when converting them into triangles.
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: RenderState::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            // relate with array layers
            multiview: None,
            // cache allows wgpu to cache shader compilation data. Only really useful for Android build targets.
            cache: None,
        });

        MainPipeline {
            pipeline: render_pipeline,
            pipeline_layout: render_pipeline_layout,
            bind_group_layouts,
            material_bind_group_layout,
        }
    }
}

pub trait Material {
    /// Return the material bind group
    fn get_bind_group(&self) -> &BindGroup;
}

pub struct PBRMaterial {
    pub bind_group: Arc<BindGroup>,
}

impl PBRMaterial {
    pub fn form_gltf(world: &World, gltf_material: &GltfMaterial) -> Self {
        let base_color = &gltf_material.base_color_texture;
        let device = &world.resource::<RenderState>().device;
        let material_bind_group_layout = &world.resource::<MaterialBindGroupLayout>().0;
        let bind_group = Arc::new(device.create_bind_group(&bg_descriptor!(
            ["PBR Material Bind Group"] [material_bind_group_layout]
            0: BindingResource::TextureView(&base_color.view);
            1: BindingResource::Sampler(&base_color.sampler);
        )));
        Self { bind_group }
    }
}

impl Material for PBRMaterial {
    fn get_bind_group(&self) -> &BindGroup {
        &self.bind_group
    }
}
