use std::{collections::BTreeMap, sync::Arc};

use bevy_ecs::prelude::*;
use parallel_light::ParallelLight;
use point_light::{PointLight, RawPointLight};
use wgpu::{BindGroup, BindGroupLayout, BufferDescriptor, BufferUsages, ShaderStages};

use crate::{
    bg_descriptor, bg_layout_descriptor, impl_pod_zeroable, macro_utils::BGLEntry, RenderState,
};

use super::transform::{Transform, WorldTransform};

pub mod parallel_light;
pub mod point_light;

#[derive(Resource)]
pub struct LightUnifromBuffer {
    // pub main_light: MainLight,
    pub buffer: Arc<wgpu::Buffer>,
}

#[repr(C, align(16))]
#[derive(Debug, Clone, Copy)]
pub struct LightUniform {
    pub direction: [f32; 3],
    pub padding1: f32,
    pub color: [f32; 4],
    pub space_matrix: [[f32; 4]; 4],
    pub intensity: f32,
    pub padding2: [f32; 3],
    /// x: point_lights, y, z, w
    pub lights_count: [u32; 4],
}

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

impl LightUnifromBuffer {
    pub fn new(device: &wgpu::Device) -> Self {
        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Light Uniform Buffer"),
            size: size_of::<LightUniform>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            buffer: Arc::new(buffer),
        }
    }

    pub fn write_buffer(&self, queue: &wgpu::Queue, light_uniform: LightUniform) {
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[light_uniform]));
    }
}

impl LightUniform {
    pub fn from_lights(
        parallel: &ParallelLight,
        dynamic: &DynamicLights,
        transform: &WorldTransform,
    ) -> Self {
        Self {
            direction: transform.forward().into(),
            color: parallel.color.into(),
            intensity: parallel.intensity,
            padding2: [0f32; 3],
            padding1: 0.,
            space_matrix: parallel.light_space_matrix(&transform).into(),
            lights_count: [dynamic.point_lights.len() as u32, 0, 0, 0],
        }
    }
}

impl_pod_zeroable!(LightUniform);
impl_pod_zeroable!(RawPointLight);

#[derive(Resource, Default)]
pub struct DynamicLights {
    pub point_lights: BTreeMap<Entity, RawPointLight>,
}

pub fn sys_update_dynamic_lights(
    mut dynamic_lights: ResMut<DynamicLights>,
    q_lights: Query<
        (Entity, &PointLight, &WorldTransform),
        Or<(Changed<PointLight>, Changed<WorldTransform>)>,
    >,
) {
    for (id, light, transfrom) in q_lights.iter() {
        dynamic_lights.point_lights.insert(id, light.raw(transfrom));
    }
}

pub fn event_on_remove_point_light(
    trigger: Trigger<OnRemove, PointLight>,
    mut dynamic_lights: ResMut<DynamicLights>,
) {
    let entity = trigger.entity();
    dynamic_lights.point_lights.remove(&entity);
}

pub fn sys_update_dynamic_lights_bind_group(
    dynamic_lights: Res<DynamicLights>,
    light_buffer: Res<LightUnifromBuffer>,
    parallel_light: Single<(&ParallelLight, &WorldTransform)>,
    bg: Res<DynamicLightBindGroup>,
    rs: Res<RenderState>,
) {
    if dynamic_lights.is_changed() {
        rs.queue.write_buffer(
            &bg.point_lights_storage_buffer,
            0,
            bytemuck::cast_slice(
                &dynamic_lights
                    .point_lights
                    .values()
                    .cloned()
                    .collect::<Vec<_>>(),
            ),
        );
        let uniform =
            LightUniform::from_lights(parallel_light.0, &dynamic_lights, parallel_light.1);
        rs.queue
            .write_buffer(&light_buffer.buffer, 0, bytemuck::cast_slice(&[uniform]));
    }
}

pub fn sys_update_light_uniform(
    single: Option<
        Single<(&WorldTransform, &ParallelLight), Or<(Changed<Transform>, Changed<ParallelLight>)>>,
    >,
    dynamic_lights: Res<DynamicLights>,
    render_light: Res<LightUnifromBuffer>,
    rs: Res<RenderState>,
) {
    let Some(single) = single else {
        return;
    };
    let (transform, main_light) = single.into_inner();
    let uniform = LightUniform::from_lights(main_light, &dynamic_lights, transform);
    render_light.write_buffer(&rs.queue, uniform);
}
