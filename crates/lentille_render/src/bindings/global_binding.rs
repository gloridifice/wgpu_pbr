use crate::prelude::*;
use bevy_ecs::{prelude::*, system::RunSystemOnce};

#[derive(Resource)]
pub struct GlobalBindGroupLayout(pub Arc<BindGroupLayout>);

#[derive(Clone, Copy, Default)]
#[allow(unused)]
pub struct RawGlobalUniform {
    pub screen_resolution: [f32; 2],
}

impl_pod_zeroable!(RawGlobalUniform);

#[derive(Resource)]
pub struct GlobalUniformBuffer {
    pub buffer: Arc<Buffer>,
}

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
            10: ShaderStages::all() => BGLEntry::UniformBuffer(); // Global Uniform
        };

        let rs = world.resource::<RenderState>();
        let device = &rs.device;

        Self(Arc::new(
            device.create_bind_group_layout(&bind_group_layout_desc),
        ))
    }
}

impl FromWorld for GlobalUniformBuffer {
    fn from_world(world: &mut World) -> Self {
        let resolution = world
            .resource::<ColorRenderTarget>()
            .get_target()
            .map(|it| [it.size.width as f32, it.size.height as f32])
            .unwrap_or([0., 0.]);
        let raw_global_uniform = RawGlobalUniform {
            screen_resolution: resolution,
        };
        let deivce = &world.resource::<RenderState>().device;
        let buffer = deivce.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Global Uniform Buffer"),
            contents: bytemuck::cast_slice(&[raw_global_uniform]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });
        Self {
            buffer: Arc::new(buffer),
        }
    }
}
