use std::fs;
use std::io::Read;
use std::sync::Arc;

use crate::bindings::global_binding::GlobalBindGroupLayout;
use crate::cubemap::CubemapMatrixBindGroups;
use crate::image::cubemap::load_cubemap_sliced;
use crate::skybox::prefiltering::PrefilteringPipeline;
use crate::utils::cube::CubeVerticesBuffer;
use crate::{SCREEN_FORMAT, prelude::*};
use bevy_app::Plugin;
use bevy_ecs::prelude::*;

pub mod prefiltering;
pub mod sh_coefficients;

pub(super) struct SkyBoxPlugin;

impl Plugin for SkyBoxPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_render_resource::<SkyboxSHBuffer>()
            .init_render_resource_with_config::<DefaultSkybox>([
                after::<CubemapMatrixBindGroups>(),
                after::<CubeVerticesBuffer>(),
            ])
            .init_render_resource_with_config::<SkyboxPipeline>([after::<GlobalBindGroupLayout>()]);
    }
}

#[derive(Resource)]
pub struct SkyboxPipeline {
    #[allow(unused)]
    pub pipeline_layout: Arc<PipelineLayout>,
    pub pipeline: Arc<RenderPipeline>,
}

#[derive(Component, Default)]
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

        let pipeline = PrefilteringPipeline::new(world, wgpu::TextureFormat::Rgba8UnormSrgb);

        let rs = world.resource::<RenderState>();
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
            &pipeline,
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
        let global_bind_group = world.resource::<GlobalBindGroupLayout>();
        let skybox_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Skybox"),
            source: skybox_shader_source,
        });
        let pipeline_layout = Arc::new(device.create_pipeline_layout(
            &wgpu::PipelineLayoutDescriptor {
                label: Some("Skybox"),
                bind_group_layouts: &[Some(&global_bind_group.0)],
                immediate_size: 0,
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
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &skybox_shader,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(SCREEN_FORMAT.into())],
                }),
                multiview_mask: None,
                cache: None,
            }),
        );
        Self {
            pipeline_layout,
            pipeline,
        }
    }
}

#[derive(Resource)]
pub struct SkyboxSHBuffer {
    pub buffer: Arc<Buffer>,
}

#[derive(Clone, Copy)]
pub struct ComputedSH {
    #[allow(unused)]
    pub array: [[f32; 4]; 9],
}

impl_pod_zeroable!(ComputedSH);

impl FromWorld for SkyboxSHBuffer {
    fn from_world(world: &mut World) -> Self {
        let rs = world.resource::<RenderState>();
        let buffer = rs
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Computed SH"),
                contents: bytemuck::cast_slice(&[ComputedSH {
                    array: [
                        [0.79, 0.44, 0.54],
                        [0.39, 0.35, 0.60],
                        [-0.34, -0.18, -0.27],
                        [-0.29, -0.06, -0.1],
                        [-0.11, -0.05, -0.12],
                        [-0.26, -0.22, -0.47],
                        [-0.16, -0.09, -0.15],
                        [0.56, 0.21, 0.14],
                        [0.21, -0.05, -0.3],
                    ]
                    .map(|it| [it[0], it[1], it[2], 0.0]),
                }]),
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            });

        Self {
            buffer: Arc::new(buffer),
        }
    }
}

pub fn sys_update_skybox_sh_from_path(
    input: In<AssetPath>,
    skybox_sh_buffer: Res<SkyboxSHBuffer>,
    rs: Res<RenderState>,
) {
    match ComputedSH::compute_from_path(input.0) {
        Ok(raw) => {
            rs.queue
                .write_buffer(&skybox_sh_buffer.buffer, 0, bytemuck::cast_slice(&[raw]));
        }
        Err(err) => {
            bevy_log::error!("Failed to update skybox SH from path. Err: \n {}", err);
        }
    }
}

impl ComputedSH {
    pub fn compute_from_path(path: AssetPath) -> anyhow::Result<ComputedSH> {
        let path = path.final_path();
        let mut file = fs::File::open(&path)?;
        let mut buffer = vec![];
        file.read_to_end(&mut buffer)?;
        let image = image::load_from_memory(&buffer)?;
        let result = sh_coefficients::compute_sh_coefficients(&image);

        Ok(ComputedSH { array: result })
    }
}
