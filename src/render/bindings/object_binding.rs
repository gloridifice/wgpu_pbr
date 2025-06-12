use std::sync::Arc;

use bevy_ecs::{
    prelude::Resource,
    world::{FromWorld, World},
};
use wgpu::{BindGroupLayout, ShaderStages};

use crate::{bg_layout_descriptor, macro_utils::BGLEntry, render::AlphaMode, RenderState};

#[derive(Resource, Clone)]
pub struct ObjectBindGroupLayout(pub Arc<BindGroupLayout>);

impl From<gltf::material::AlphaMode> for AlphaMode {
    fn from(value: gltf::material::AlphaMode) -> Self {
        match value {
            gltf::material::AlphaMode::Opaque => AlphaMode::Opaque,
            gltf::material::AlphaMode::Mask => AlphaMode::Blend,
            gltf::material::AlphaMode::Blend => AlphaMode::Blend,
        }
    }
}

impl FromWorld for ObjectBindGroupLayout {
    fn from_world(world: &mut World) -> Self {
        let rs = world.resource::<RenderState>();
        let device = &rs.device;
        let object_bind_group_layout =
            Arc::new(device.create_bind_group_layout(&bg_layout_descriptor!(
                ["Object Bind Group Layout"]
                0: ShaderStages::VERTEX => BGLEntry::UniformBuffer(); // Transform
            )));
        Self(object_bind_group_layout)
    }
}
