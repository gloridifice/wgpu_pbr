use std::sync::Arc;

use bevy_ecs::prelude::*;
use global_binding::GlobalBindGroup;
use wgpu::{BindGroupLayout, PipelineLayout, RenderPipeline};
use write_g_buffer_pipeline::GBufferTexturesBindGroup;

use crate::{asset::AssetPath, wgpu_init, RenderState};

use super::{light::DynamicLightBindGroup, shader_loader::ShaderLoader, FullScreenVertexShader};

pub mod global_binding;
pub mod write_g_buffer_pipeline;

#[allow(unused)]
#[derive(Resource)]
pub struct MainPipeline {
    pub pipeline: Arc<RenderPipeline>,
    pub pipeline_layout: Arc<PipelineLayout>,
    pub bind_group_layouts: Vec<Arc<BindGroupLayout>>,
}

impl FromWorld for MainPipeline {
    fn from_world(world: &mut bevy_ecs::world::World) -> Self {
        let shader_source = world
            .resource_mut::<ShaderLoader>()
            .load_source(AssetPath::new_shader_wgsl("pbr_main"))
            .unwrap();
        let rs = &world.resource::<RenderState>();
        let device = &rs.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("PBR Main"),
            source: shader_source,
        });
        let full_screen_shader = world.resource::<FullScreenVertexShader>();

        let bind_group_layouts = vec![
            Arc::clone(&world.resource::<GlobalBindGroup>().layout),
            Arc::clone(&world.resource::<GBufferTexturesBindGroup>().layout),
            Arc::clone(&world.resource::<DynamicLightBindGroup>().layout),
        ];

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("PBR Main Pipeline"),
                bind_group_layouts: &bind_group_layouts
                    .iter()
                    .map(|it| it.as_ref())
                    .collect::<Vec<_>>(),
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu_init::full_screen_pipeline_desc(
            Some("PBR Main Pipeline"),
            &render_pipeline_layout,
            &full_screen_shader.module,
            &shader,
            &[Some(wgpu_init::color_target_replace_write_all(
                rs.config.format,
            ))],
        ));

        MainPipeline {
            pipeline: Arc::new(render_pipeline),
            pipeline_layout: Arc::new(render_pipeline_layout),
            bind_group_layouts,
        }
    }
}
