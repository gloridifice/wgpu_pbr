use std::{collections::BTreeMap, sync::Arc};

use crate::{app_ext::AppExt, prelude::*};
use bevy_app::{Plugin, PostUpdate};
use bevy_ecs::prelude::*;
use parallel_light::ParallelLight;
use point_light::{PointLight, RawPointLight};

pub mod parallel_light;
pub mod point_light;

pub(crate) struct LightPlugin;

impl Plugin for LightPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_observer(event_on_remove_point_light)
            .add_systems(PostUpdate, sys_update_light_uniform)
            .add_systems(
                PostUpdate,
                (
                    sys_update_dynamic_lights,
                    sys_update_dynamic_lights_bind_group,
                )
                    .chain(),
            )
            .init_render_resource::<LightUnifromBuffer>()
            .init_render_resource::<DynamicLightBindGroup>()
            .init_render_resource::<DynamicLights>();
    }
}

#[derive(Resource)]
pub struct LightUnifromBuffer {
    // pub main_light: MainLight,
    pub buffer: Arc<TypedBuffer<LightUniform>>,
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

impl LightUnifromBuffer {
    pub fn new(device: &wgpu::Device) -> Self {
        let buffer = TypedBuffer::new(
            device,
            Some("Light Uniform Buffer"),
            BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        );
        Self {
            buffer: Arc::new(buffer),
        }
    }

    pub fn write_buffer(&self, queue: &wgpu::Queue, light_uniform: LightUniform) {
        self.buffer.write(light_uniform, queue);
    }
}

impl FromWorld for LightUnifromBuffer {
    fn from_world(world: &mut World) -> Self {
        let rs = world.resource::<RenderState>();
        Self::new(&rs.device)
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
            color: parallel.color.into_array(),
            intensity: parallel.intensity,
            padding2: [0f32; 3],
            padding1: 0.,
            space_matrix: parallel.light_space_matrix(transform).into(),
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
    for (id, light, transform) in q_lights.iter() {
        dynamic_lights.point_lights.insert(id, light.raw(transform));
    }
}

pub fn event_on_remove_point_light(
    trigger: On<Remove, PointLight>,
    mut dynamic_lights: ResMut<DynamicLights>,
) {
    let entity = trigger.event_target();
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
        light_buffer.buffer.write(uniform, &rs.queue);
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
