use std::sync::Arc;

use bevy_ecs::prelude::*;
use bevy_ecs::world::FromWorld;
use wgpu::{PipelineLayout, RenderPipeline};

use crate::{asset::AssetPath, wgpu_init, RenderState};

use super::{cubemap, defered_rendering::GlobalBindGroup, shader_loader::ShaderLoader};

#[derive(Resource)]
pub struct SkyboxPipeline {
    pub pipeline_layout: Arc<PipelineLayout>,
    pub pipeline: Arc<RenderPipeline>,
}

impl FromWorld for SkyboxPipeline {
    fn from_world(world: &mut World) -> Self {
        let mut shader_loader = world.resource_mut::<ShaderLoader>();
        let skybox_shader_source = shader_loader
            .load_source(AssetPath::new_shader_wgsl("skybox"))
            .unwrap();
        let rs = world.resource::<RenderState>();
        let device = &rs.device;
        let global_bind_group = world.resource::<GlobalBindGroup>();
        let skybox_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Skybox"),
            source: skybox_shader_source,
        });
        let pipeline_layout = Arc::new(device.create_pipeline_layout(
            &wgpu::PipelineLayoutDescriptor {
                label: Some("Skybox"),
                bind_group_layouts: &[&global_bind_group.layout],
                push_constant_ranges: &[],
            },
        ));
        let pipeline = Arc::new(device.create_render_pipeline(
            &wgpu_init::no_depth_stencil_pipeline_desc(
                Some("Skybox"),
                &pipeline_layout,
                &skybox_shader,
                &[cubemap::cube_vertex_layout()],
                &skybox_shader,
                &[Some(rs.config.format.into())],
            ),
        ));
        Self {
            pipeline_layout,
            pipeline,
        }
    }
}
