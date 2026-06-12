use bevy_ecs::prelude::Resource;

use crate::{prelude::*, transform::TransformUniform};
use lentille_wgpu_macros::DeviceNewFromWorld;

binding_define! {
    [Object]
    layout_macro: #[derive(Resource, DeviceNewFromWorld)],
    0: vert => transform_uniform: TypedBuffer<TransformUniform>,
}

impl From<gltf::material::AlphaMode> for AlphaMode {
    fn from(value: gltf::material::AlphaMode) -> Self {
        match value {
            gltf::material::AlphaMode::Opaque => AlphaMode::Opaque,
            gltf::material::AlphaMode::Mask => AlphaMode::Blend,
            gltf::material::AlphaMode::Blend => AlphaMode::Blend,
        }
    }
}
