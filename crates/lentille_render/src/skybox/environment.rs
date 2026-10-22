use super::{ComputedSHUniform, DefaultSkybox, Skybox};
use crate::{
    cubemap::{CubemapConverterRgba16Float, CubemapMatrixBindGroups},
    prelude::*,
    utils::cube::CubeVerticesBuffer,
};
use bevy_ecs::prelude::*;
use bevy_reflect::Reflect;

mod loading;
pub use loading::{CpuEnvironment, EnvironmentUpload};

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
    let root = world
        .get_resource::<crate::asset::AssetRoot>()
        .cloned()
        .unwrap_or_default();
    let mut upload = CpuEnvironment::load(&root, source, u64::MAX)?.into_upload(world)?;
    while !upload.advance(
        world,
        &mut crate::asset::FrameBudget::new(std::time::Duration::from_secs(1), usize::MAX),
    )? {}
    Ok(upload.finish())
}
