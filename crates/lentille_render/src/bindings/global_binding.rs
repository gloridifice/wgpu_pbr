use crate::prelude::*;
use bevy_ecs::prelude::*;

#[derive(Resource)]
pub struct GlobalBindGroupLayout(pub Arc<BindGroupLayout>);

impl FromWorld for GlobalBindGroupLayout {
    fn from_world(world: &mut World) -> Self {
        let bind_group_layout_desc = bg_layout_descriptor! {
            ["Main PBR Global Bind Group Layout"]
            0: ShaderStages::all() => BGLEntry::UniformBuffer(); // Camera
            1: ShaderStages::all() => BGLEntry::UniformBuffer(); // Light
            2: ShaderStages::FRAGMENT => BGLEntry::Tex2D(false, wgpu::TextureSampleType::Depth); // Depth
            3: ShaderStages::FRAGMENT => BGLEntry::Sampler(wgpu::SamplerBindingType::Comparison); // Depth
            4: ShaderStages::FRAGMENT => BGLEntry::Tex2D(false, wgpu::TextureSampleType::Float { filterable: true }); // DFG
            5: ShaderStages::FRAGMENT => BGLEntry::TexCube(false, wgpu::TextureSampleType::Float { filterable: true }); // Skybox
            6: ShaderStages::FRAGMENT => BGLEntry::Sampler(wgpu::SamplerBindingType::Filtering); // Skybox
            7: ShaderStages::FRAGMENT => BGLEntry::UniformBuffer(); // Skybox SH for diffuse

            8: ShaderStages::FRAGMENT => BGLEntry::Tex2D(false, wgpu::TextureSampleType::Float { filterable: true}); // Sampleable Color Target
            9: ShaderStages::FRAGMENT => BGLEntry::Sampler(wgpu::SamplerBindingType::Filtering); // Sampleable Color Target
        };

        let rs = world.resource::<RenderState>();
        let device = &rs.device;

        Self(Arc::new(
            device.create_bind_group_layout(&bind_group_layout_desc),
        ))
    }
}
