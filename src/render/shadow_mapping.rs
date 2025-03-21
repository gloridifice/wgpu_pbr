use std::sync::Arc;

use bevy_ecs::{
    component::Component,
    system::Resource,
    world::{self, FromWorld, Mut},
};
use wgpu::{BindGroup, BindGroupLayout, PipelineLayout, RenderPipeline, ShaderStages};

use crate::{
    asset::AssetPath, bg_descriptor, bg_layout_descriptor, macro_utils::BGLEntry, RenderState,
};

use super::{
    light::LightUnifromBuffer, shader_loader::ShaderLoader, ObjectBindGroupLayout,
    UploadedImageWithSampler, Vertex,
};

#[derive(Resource)]
pub struct ShadowMap {
    // For shadow map rendering pass
    pub image: UploadedImageWithSampler,
}

#[derive(Resource)]
pub struct ShadowMapGlobalBindGroup {
    pub layout: Arc<BindGroupLayout>,
    pub bind_group: Arc<BindGroup>,
}

#[derive(Resource)]
pub struct ShadowMappingPipeline {
    pub pipeline: Arc<RenderPipeline>,
    #[allow(unused)]
    pub layout: Arc<PipelineLayout>,
}

#[derive(Component, Clone, Default)]
pub struct CastShadow;

impl FromWorld for ShadowMapGlobalBindGroup {
    fn from_world(world: &mut world::World) -> Self {
        world.resource_scope(|world, render_state: Mut<RenderState>| {
            let device = &render_state.device;

            let layout = Arc::new(device.create_bind_group_layout(&bg_layout_descriptor! (
                ["Shadow Mapping Global Bind Group Layout"]
                0: ShaderStages::all() => BGLEntry::UniformBuffer(); // Light
            )));

            let light_uniform_buffer = &world.resource::<LightUnifromBuffer>().buffer;

            let bind_group = Arc::new(device.create_bind_group(&bg_descriptor!(
                ["Shadow Mapping Global Bind Group"] [ &layout ]
                0: light_uniform_buffer.as_entire_binding();
            )));

            Self { layout, bind_group }
        })
    }
}

impl FromWorld for ShadowMappingPipeline {
    fn from_world(world: &mut world::World) -> Self {
        let shader_source = world
            .resource_mut::<ShaderLoader>()
            .load_source(AssetPath::new_shader_wgsl("light_depth_map"))
            .unwrap();
        let render_state = world.resource::<RenderState>();
        let device = &render_state.device;
        let global_bg_layout = world.resource::<ShadowMapGlobalBindGroup>();
        let object_bg_layout = world.resource::<ObjectBindGroupLayout>();

        let layout = Arc::new(
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Shadow mapping pipeline"),
                bind_group_layouts: &[&global_bg_layout.layout, &object_bg_layout.0],
                push_constant_ranges: &[],
            }),
        );

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shadow Mapping Light Shader"),
            source: shader_source,
        });

        let pipeline = Arc::new(
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Shadow Mapping Pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[Vertex::desc()],
                },
                fragment: None,
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
                    bias: wgpu::DepthBiasState {
                        constant: 2,
                        slope_scale: 2.0,
                        clamp: 0.0,
                    },
                }),
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
                cache: None,
            }),
        );

        Self { pipeline, layout }
    }
}

impl FromWorld for ShadowMap {
    fn from_world(world: &mut world::World) -> Self {
        world.resource_scope(|_, render_state: Mut<RenderState>| {
            let image = crate::render::create_depth_texture(
                &render_state.device,
                2048,
                2048,
                Some(wgpu::CompareFunction::LessEqual),
            );

            Self { image }
        })
    }
}
