use std::sync::Arc;

use bevy_ecs::prelude::*;

use crate::RenderState;

use super::UploadedImageWithSampler;

#[derive(Resource)]
pub struct DFGTexture {
    pub texture: Arc<UploadedImageWithSampler>,
}
impl FromWorld for DFGTexture {
    fn from_world(world: &mut World) -> Self {
        let rs = world.resource::<RenderState>();
        let texture = Arc::new(
            UploadedImageWithSampler::load_from_path(
                crate::asset::AssetPath::Assets("textures/ibl_brdf_lut.png".to_string()),
                &rs.device,
                &rs.queue,
                wgpu::TextureFormat::Rgba8Unorm,
            )
            .unwrap(),
        );
        Self { texture }
    }
}
