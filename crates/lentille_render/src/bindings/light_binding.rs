use crate::{light::point_light::RawPointLight, prelude::*};
use bevy_ecs::prelude::*;
use lentille_wgpu_utils::typed_binding_resource::StorageBufferBinding;

binding_define! {
    [DynamicLight]
    layout_macro: #[derive(Resource)],
    0: frag => point_lights_storage_buffer: StorageBufferBinding<'a, true, false>,
}

impl FromWorld for DynamicLightBindGroupLayout {
    fn from_world(world: &mut World) -> Self {
        Self::new(&world.resource::<RenderState>().device)
    }
}

/// It manages lights' bind group and buffers that will change.
/// Dynamically increase or decrease.
#[derive(Resource)]
pub struct DynamicLightBindGroup {
    pub point_lights_storage_buffer: Arc<wgpu::Buffer>,
    pub bind_group: Arc<BindGroup>,
}

impl FromWorld for DynamicLightBindGroup {
    fn from_world(world: &mut bevy_ecs::world::World) -> Self {
        let device = &world.resource::<RenderState>().device;
        let layout = &world.resource::<DynamicLightBindGroupLayout>();

        let buffer = Arc::new(device.create_buffer(&BufferDescriptor {
            label: Some("Point Light Storage Buffer"),
            size: 128 * size_of::<RawPointLight>() as u64,
            usage: BufferUsages::COPY_DST | BufferUsages::STORAGE,
            mapped_at_creation: false,
        }));

        let bind_group = Arc::new(
            DynamicLightBindGroupBuilder {
                point_lights_storage_buffer: &StorageBufferBinding::from(buffer.as_ref()),
            }
            .build(device, layout),
        );

        Self {
            point_lights_storage_buffer: buffer,
            bind_group,
        }
    }
}
