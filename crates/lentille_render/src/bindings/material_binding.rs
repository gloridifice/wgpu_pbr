use crate::prelude::*;
use bevy_ecs::prelude::*;

#[derive(Resource, Clone)]
pub struct PBRMaterialBindGroupLayout(pub Arc<BindGroupLayout>);

impl FromWorld for PBRMaterialBindGroupLayout {
    fn from_world(world: &mut World) -> Self {
        let rs = world.resource::<RenderState>();
        let device = &rs.device;
        let material_bind_group_layout =
            Arc::new(device.create_bind_group_layout(&bg_layout_descriptor!(
                ["Material Bind Group Layout"]
                0: ShaderStages::FRAGMENT => BGLEntry::UniformBuffer();
                1: ShaderStages::FRAGMENT => BGLEntry::Tex2D(false, TextureSampleType::Float { filterable: true }); // BaseColor Tex
                2: ShaderStages::FRAGMENT => BGLEntry::Sampler(SamplerBindingType::Filtering);
                3: ShaderStages::FRAGMENT => BGLEntry::Tex2D(false, TextureSampleType::Float { filterable: true }); // Normal Tex
                4: ShaderStages::FRAGMENT => BGLEntry::Sampler(SamplerBindingType::Filtering);
            )));
        Self(material_bind_group_layout)
    }
}
