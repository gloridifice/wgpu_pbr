use std::sync::Arc;

use bevy_ecs::{
    system::Resource,
    world::{self, FromWorld, Mut},
};
use wgpu::{
    BindGroup, BindGroupLayout, PipelineLayout, RenderPipeline,
};

use crate::RenderState;

use super::{
    ObjectBindGroupLayout, UploadedImage, Vertex,
};

#[derive(Resource)]
pub struct ShadowMap {
    // For shadow map rendering pass
    pub image: UploadedImage,
}

#[derive(Resource)]
pub struct ShadowMapGlobalBindGroup {
    pub layout: Arc<BindGroupLayout>,
    pub bind_group: Arc<BindGroup>,
}

#[derive(Resource)]
pub struct ShadowMappingPipeline {
    pub pipeline: Arc<RenderPipeline>,
    pub layout: Arc<PipelineLayout>,
}

impl FromWorld for ShadowMapGlobalBindGroup {
    fn from_world(world: &mut world::World) -> Self {
        todo!()
    }
}

impl FromWorld for ShadowMappingPipeline {
    fn from_world(world: &mut world::World) -> Self {
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

        let shader = device.create_shader_module(wgpu::include_wgsl!(
            "../../assets/shaders/light_depth_map.wgsl"
        ));

        let pipeline = Arc::new(
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Shadow Mapping Pipeline"),
                layout: Some(&layout),
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
            }),
        );

        Self { pipeline, layout }
    }
}

impl FromWorld for ShadowMap {
    fn from_world(world: &mut world::World) -> Self {
        world.resource_scope(|_, render_state: Mut<RenderState>| {
            let image = RenderState::create_depth_texture(&render_state.device, 1024, 1024);
            Self { image }
        })
    }
}