use crate::render::{light::point_light::RawPointLight, prelude::*};
use bevy_ecs::prelude::*;

/// It manages lights' bind group and buffers that will change.
/// Dynamically increase or decrease.
#[derive(Resource)]
pub struct DynamicLightBindGroup {
    pub point_lights_storage_buffer: Arc<wgpu::Buffer>,
    pub layout: Arc<BindGroupLayout>,
    pub bind_group: Arc<BindGroup>,
}

impl FromWorld for DynamicLightBindGroup {
    fn from_world(world: &mut bevy_ecs::world::World) -> Self {
        let device = &world.resource::<RenderState>().device;

        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Point Light Storage Buffer"),
            size: 128 * size_of::<RawPointLight>() as u64,
            usage: BufferUsages::COPY_DST | BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        let layout_desc = bg_layout_descriptor! {
            ["Dynamic Light"]
            0: ShaderStages::FRAGMENT => BGLEntry::StorageBuffer(true);
            // // DFG Sampler
            // 1: ShaderStages::FRAGMENT => BGLEntry::Sampler(wgpu::SamplerBindingType::Filtering);
            // // IBL DFG LUT
            // 2: ShaderStages::FRAGMENT => BGLEntry::Tex2D(false, wgpu::TextureSampleType::Float { filterable: true });
            // // Env Cubemap Sampler
            // 3: ShaderStages::FRAGMENT => BGLEntry::Sampler(wgpu::SamplerBindingType::Filtering);
            // // Environment Cubemap
            // 4: ShaderStages::FRAGMENT => BGLEntry::Tex2D(false, wgpu::TextureSampleType::Float { filterable: true });
            // // Sepharical Harmonics Buffer
            // 5: ShaderStages::FRAGMENT => BGLEntry::StorageBuffer(true);
        };
        let layout = Arc::new(device.create_bind_group_layout(&layout_desc));

        let bg_desc = bg_descriptor!(
                ["Dynamic Light"][&layout]
                0: buffer.as_entire_binding();
        );
        let bind_group = Arc::new(device.create_bind_group(&bg_desc));
        Self {
            point_lights_storage_buffer: Arc::new(buffer),
            layout,
            bind_group,
        }
    }
}
