use std::sync::Arc;

use crate::{
    SCREEN_FORMAT,
    bindings::{
        camera_binding::CameraBindGroupLayout, light_binding::DynamicLightBindGroupLayout,
        material_binding::PbrMaterialBindGroupLayout,
    },
    deferred_rendering::write_g_buffer_pipeline::{
        DeferredWriteGBufferPipeline, GBufferTextureBindGroupLayout, WriteGBufferPlugin,
    },
    prelude::*,
};
use bevy_app::Plugin;
use bevy_ecs::prelude::*;

pub mod write_g_buffer_pipeline;

pub(crate) struct DeferredRenderingPlugin;

impl Plugin for DeferredRenderingPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_plugins(WriteGBufferPlugin)
            .init_render_resource_with_config::<DeferredWriteGBufferPipeline>([
                after::<GBufferTextureBindGroupLayout>(),
                after::<CameraBindGroupLayout>(),
            ])
            .init_render_resource_with_config::<DeferredComputePipeline>([
                after::<GBufferTextureBindGroupLayout>(),
                after::<CameraBindGroupLayout>(),
                after::<PbrMaterialBindGroupLayout>(),
                after::<DynamicLightBindGroup>(),
            ]);
    }
}

#[allow(unused)]
#[derive(Resource)]
pub struct DeferredComputePipeline {
    pub pipeline: Arc<RenderPipeline>,
    pub pipeline_layout: Arc<PipelineLayout>,
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
            Some(&world.resource::<CameraBindGroupLayout>().0),
            Some(&world.resource::<GBufferTextureBindGroupLayout>().0),
            Some(&world.resource::<PbrMaterialBindGroupLayout>().0),
            Some(&world.resource::<DynamicLightBindGroupLayout>().0),
        ];

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("PBR Main Pipeline"),
                bind_group_layouts: &bind_group_layouts,
                immediate_size: 0,
            });

        let render_pipeline =
            device.create_render_pipeline(&lentille_wgpu_utils::full_screen_pipeline_desc(
                Some("PBR Main Pipeline"),
                &render_pipeline_layout,
                &full_screen_shader.module,
                &shader,
                &[Some(lentille_wgpu_utils::color_target_replace_write_all(
                    SCREEN_FORMAT,
                ))],
            ));

        DeferredComputePipeline {
            pipeline: Arc::new(render_pipeline),
            pipeline_layout: Arc::new(render_pipeline_layout),
        }
    }
}
