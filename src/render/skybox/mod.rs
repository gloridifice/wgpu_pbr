use std::sync::Arc;

use bevy_ecs::prelude::*;
use bevy_ecs::system::RunSystemOnce;
use bevy_ecs::world::FromWorld;
use wgpu::{PipelineLayout, RenderPipeline};

use crate::asset::cubemap::load_cubemap_sliced;
use crate::{asset::AssetPath, RenderState};

use super::cubemap::CubemapMatrixBindGroups;
use super::defered_rendering::global_binding::GlobalBindGroup;
use super::utils::cube::CubeVerticesBuffer;
use super::{shader_loader::ShaderLoader, UploadedImage};

pub mod prefiltering;

#[derive(Resource)]
pub struct SkyboxPipeline {
    #[allow(unused)]
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
            // .map(|it| AssetPath::Assets(format!("textures/cubemap/test_{}.png", it)));
            .map(|it| AssetPath::Assets(format!("textures/cubemap/{}.jpg", it)));
        let source_texture = load_cubemap_sliced(&paths, &rs.device, &rs.queue).unwrap();

        let rs = world.resource::<RenderState>();
        let pipeline = world.resource::<prefiltering::PrefilteringPipeline>();
        let matrix_bind_groups = world.resource::<CubemapMatrixBindGroups>();
        let cube_vertex = world.resource::<CubeVerticesBuffer>();
        let texture = prefiltering::prefilter(
            Some("Default Skybox"),
            &rs.device,
            &rs.queue,
            &source_texture.texture,
            &source_texture.view,
            5,
            1145,
            pipeline,
            matrix_bind_groups,
            cube_vertex,
        )
        .unwrap();
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
        let cube_vertex_layout = super::utils::cube::cube_vertex_layout();
        let pipeline = Arc::new(
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Skybox"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &skybox_shader,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[cube_vertex_layout],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Front),
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
                    module: &skybox_shader,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(rs.config.format.into())],
                }),
                multiview: None,
                cache: None,
            }),
        );
        Self {
            pipeline_layout,
            pipeline,
        }
    }
}
