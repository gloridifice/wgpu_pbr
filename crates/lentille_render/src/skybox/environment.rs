use super::{
    ComputedSHUniform, DefaultSkybox, Skybox,
    prefiltering::{PrefilteringPipeline, prefilter},
};
use crate::{
    cubemap::{CubemapConverterRgba16Float, CubemapMatrixBindGroups},
    prelude::*,
    utils::cube::CubeVerticesBuffer,
};
use bevy_ecs::prelude::*;
use bevy_reflect::Reflect;

#[derive(Component, Clone, Debug, PartialEq, Reflect)]
#[reflect(Component)]
pub enum Environment {
    Default,
    Image {
        path: AssetPath,
        size: u32,
        levels: u32,
        samples: u32,
    },
}
impl Default for Environment {
    fn default() -> Self {
        Self::Default
    }
}

pub fn prepare_environment(
    world: &mut World,
    source: &Environment,
) -> anyhow::Result<(Skybox, ComputedSHUniform)> {
    let Environment::Image {
        path,
        size,
        levels,
        samples,
    } = source
    else {
        return Ok((Skybox::default(), world.resource::<DefaultSkybox>().sh));
    };
    let root = world
        .get_resource::<crate::asset::AssetRoot>()
        .cloned()
        .unwrap_or_default();
    let image = image::open(root.resolve(path)?)?;
    let sh = ComputedSHUniform {
        array: super::sh_coefficients::compute_sh_coefficients(&image),
    };
    let pipeline = PrefilteringPipeline::new(world, TextureFormat::Rgba16Float);
    let rs = world.resource::<RenderState>();
    anyhow::ensure!(
        *size > 0
            && *size <= rs.device.limits().max_texture_dimension_2d
            && *levels > 0
            && *levels <= size.ilog2() + 1
            && *samples > 0
            && *samples <= 4096,
        "Invalid environment preparation settings"
    );
    let hdri = UploadedImage::load_hdri_to_f16(&root.resolve(path)?, &rs.device, &rs.queue)?;
    let cube = world.resource::<CubeVerticesBuffer>();
    let texture = world
        .resource::<CubemapConverterRgba16Float>()
        .0
        .render_hdir_to_cube_map(
            &rs.device,
            &rs.queue,
            &hdri.view,
            &cube.vertices_buffer,
            *size,
        );
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::Cube),
        ..Default::default()
    });
    let texture = prefilter(
        Some("Scene environment"),
        &rs.device,
        &rs.queue,
        &texture,
        &view,
        *levels,
        *samples,
        &pipeline,
        world.resource::<CubemapMatrixBindGroups>(),
        cube,
    )?;
    Ok((
        Skybox {
            texture: Some(texture),
        },
        sh,
    ))
}
