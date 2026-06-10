use std::fs;
use std::io::Read;
use std::sync::Arc;

use crate::cubemap::CubemapMatrixBindGroups;
use crate::image::cubemap::load_cubemap_sliced;
use crate::prelude::*;
use crate::skybox::prefiltering::PrefilteringPipeline;
use crate::utils::cube::CubeVerticesBuffer;
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
            ]);
    }
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
