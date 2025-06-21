use std::sync::Arc;

use crate::{
    bindings::material_binding::PBRMaterialBindGroupLayout,
    defered_rendering::write_g_buffer_pipeline::{
        DeferredWriteGBufferPipeline, GBufferTexturesBindGroup,
    },
    prelude::*,
};
use bevy_app::Plugin;
use bevy_ecs::prelude::*;

pub mod write_g_buffer_pipeline;

pub(crate) struct DeferredRenderingPlugin;

impl Plugin for DeferredRenderingPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_render_resource_with_config::<DeferredWriteGBufferPipeline>([after::<
            GlobalBindGroup,
        >()])
        .init_render_resource_with_config::<write_g_buffer_pipeline::GBufferTexturesBindGroup>([
            after::<RenderTargetSize>(),
        ])
        .init_render_resource_with_config::<DeferredComputePipeline>([
            after::<GlobalBindGroup>(),
            after::<GBufferTexturesBindGroup>(),
            after::<PBRMaterialBindGroupLayout>(),
            after::<DynamicLightBindGroup>(),
        ]);
    }
}

#[allow(unused)]
#[derive(Resource)]
pub struct DeferredComputePipeline {
    pub pipeline: Arc<RenderPipeline>,
    pub pipeline_layout: Arc<PipelineLayout>,
    pub bind_group_layouts: Vec<Arc<BindGroupLayout>>,
}

impl FromWorld for DeferredComputePipeline {
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
            Arc::clone(&world.resource::<PBRMaterialBindGroupLayout>().0),
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

        let render_pipeline =
            device.create_render_pipeline(&lentille_wgpu_utils::full_screen_pipeline_desc(
                Some("PBR Main Pipeline"),
                &render_pipeline_layout,
                &full_screen_shader.module,
                &shader,
                &[Some(lentille_wgpu_utils::color_target_replace_write_all(
                    rs.config.format,
                ))],
            ));

        DeferredComputePipeline {
            pipeline: Arc::new(render_pipeline),
            pipeline_layout: Arc::new(render_pipeline_layout),
            bind_group_layouts,
        }
    }
}
