use std::sync::Arc;

use bevy_ecs::prelude::*;
use bevy_ecs::world::FromWorld;
use wgpu::{PipelineLayout, RenderPipeline};

use crate::asset::cubemap::load_cubemap_sliced;
use crate::{asset::AssetPath, wgpu_init, RenderState};

use super::defered_rendering::global_binding::GlobalBindGroup;
use super::{cubemap, shader_loader::ShaderLoader, UploadedImage};

#[derive(Resource)]
pub struct SkyboxPipeline {
    pub pipeline_layout: Arc<PipelineLayout>,
    pub pipeline: Arc<RenderPipeline>,
}

#[derive(Resource, Default)]
pub struct Skybox {
    pub texture: Option<UploadedImage>,
}

#[derive(Resource)]
pub struct DefaultSkybox {
    pub texture: UploadedImage,
}

impl FromWorld for DefaultSkybox {
    fn from_world(world: &mut World) -> Self {
        let rs = world.resource::<RenderState>();
        let paths = ["posx", "negx", "posy", "negy", "posz", "negz"]
            .map(|it| AssetPath::Assets(format!("textures/cubemap/{}.jpg", it)));
        let texture = load_cubemap_sliced(&paths, &rs.device, &rs.queue).unwrap();
        Self { texture }
    }
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
