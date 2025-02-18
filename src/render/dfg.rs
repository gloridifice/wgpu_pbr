use std::sync::Arc;

use bevy_ecs::prelude::*;

use crate::asset::load::Loadable;

use super::UploadedImageWithSampler;

#[derive(Resource)]
pub struct DFGTexture {
    texture: Arc<UploadedImageWithSampler>,
}
impl FromWorld for DFGTexture {
    fn from_world(world: &mut World) -> Self {
        let texture = Arc::new(
            UploadedImageWithSampler::load(
                crate::asset::AssetPath::Assets("textures/ibl_brdf_lut.png".to_string()),
                world,
            )
            .unwrap(),
        );
        Self { texture }
    }
}
