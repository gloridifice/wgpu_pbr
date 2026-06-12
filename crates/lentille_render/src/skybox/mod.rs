use std::fs;
use std::io::Read;
use std::sync::Arc;

use crate::cubemap::CubemapMatrixBindGroups;
use crate::prelude::*;
use crate::skybox::prefiltering::PrefilteringPipeline;
use crate::utils::cube::CubeVerticesBuffer;
use bevy_app::Plugin;
use bevy_ecs::prelude::*;

pub mod cache;
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
    pub texture: Option<UploadedImage<DimCube, SampleFloatFilterable>>,
}

#[derive(Resource)]
pub struct DefaultSkybox {
    pub texture: UploadedImage<DimCube, SampleFloatFilterable>,
}

impl FromWorld for DefaultSkybox {
    fn from_world(world: &mut World) -> Self {
        let level_count = 5u32;
        let sample_count = 1145u32;
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let cache_path = "assets/.cache/skybox_default_prefiltered.bin";

        let paths = ["posx", "negx", "posy", "negy", "posz", "negz"]
            .map(|it| AssetPath::Assets(format!("textures/cubemap/{}.jpg", it)));

        let source_bytes = crate::image::cubemap::read_cubemap_bytes(&paths).unwrap();
        let fingerprint = cache::fingerprint(&source_bytes, level_count, sample_count, format);

        {
            let rs = world.resource::<RenderState>();
            if let Some(texture) = cache::try_load(cache_path, fingerprint, &rs.device, &rs.queue) {
                return Self { texture };
            }
        }

        let rs = world.resource::<RenderState>();
        let source_texture =
            crate::image::cubemap::load_cubemap_from_bytes(&source_bytes, &rs.device, &rs.queue)
                .unwrap();

        let pipeline = PrefilteringPipeline::new(world, format);

        let rs = world.resource::<RenderState>();
        let matrix_bind_groups = world.resource::<CubemapMatrixBindGroups>();
        let cube_vertex = world.resource::<CubeVerticesBuffer>();
        let texture = prefiltering::prefilter(
            Some("Default Skybox"),
            &rs.device,
            &rs.queue,
            &source_texture.texture,
            &source_texture.view,
            level_count,
            sample_count,
            &pipeline,
            matrix_bind_groups,
            cube_vertex,
        )
        .unwrap();

        {
            let rs = world.resource::<RenderState>();
            if let Err(err) = cache::save(
                cache_path,
                fingerprint,
                &texture.texture,
                &rs.device,
                &rs.queue,
            ) {
                bevy_log::warn!("Failed to write skybox prefilter cache: {err}");
            }
        }

        Self { texture }
    }
}

#[derive(Resource)]
pub struct SkyboxSHBuffer {
    pub buffer: Arc<TypedBuffer<ComputedSHUniform>>,
}

#[derive(Clone, Copy)]
pub struct ComputedSHUniform {
    #[allow(unused)]
    pub array: [[f32; 4]; 9],
}

impl_pod_zeroable!(ComputedSHUniform);

impl FromWorld for SkyboxSHBuffer {
    fn from_world(world: &mut World) -> Self {
        let rs = world.resource::<RenderState>();
        let buffer = TypedBuffer::new_init(
            &rs.device,
            Some("Computed SH"),
            ComputedSHUniform {
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
            },
            BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        );

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
    match ComputedSHUniform::compute_from_path(input.0) {
        Ok(raw) => {
            skybox_sh_buffer.buffer.write(raw, &rs.queue);
        }
        Err(err) => {
            bevy_log::error!("Failed to update skybox SH from path. Err: \n {}", err);
        }
    }
}

impl ComputedSHUniform {
    pub fn compute_from_path(path: AssetPath) -> anyhow::Result<ComputedSHUniform> {
        let path = path.final_path();
        let mut file = fs::File::open(&path)?;
        let mut buffer = vec![];
        file.read_to_end(&mut buffer)?;
        let image = image::load_from_memory(&buffer)?;
        let result = sh_coefficients::compute_sh_coefficients(&image);

        Ok(ComputedSHUniform { array: result })
    }
}
