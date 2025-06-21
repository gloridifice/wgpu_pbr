use std::sync::Arc;

use crate::bindings::global_binding::{GlobalBindGroupLayout, RawGlobalUniform};
use crate::prelude::*;
use bevy_app::{Plugin, PostUpdate};
use bevy_ecs::component::Component;
use bevy_ecs::prelude::*;
use bevy_ecs::query::{Changed, Or};
use bevy_ecs::system::{RunSystemOnce, Single};
use bevy_ecs::world::FromWorld;
use cgmath::{Matrix4, SquareMatrix, perspective};
use lentille_wgpu_utils::impl_pod_zeroable;
use wgpu::{BindGroup, BufferDescriptor};

use crate::dfg::DFGTexture;
use crate::light::LightUnifromBuffer;
use crate::prelude::GlobalUniformBuffer;
use crate::shadow_mapping::ShadowMap;
use crate::skybox::{DefaultSkybox, SkyboxSHBuffer};
use crate::{ColorRenderTarget, RenderState};

use super::transform::{Transform, WorldTransform};

pub(crate) struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_systems(
            PostUpdate,
            (
                sys_update_camera_uniform,
                sys_create_camera_global_bind_group,
            )
                .chain(),
        );
    }
}

#[derive(Component)]
pub struct CameraBuffer {
    pub buffer: Arc<wgpu::Buffer>,
}

#[derive(Component, Clone, Copy, Default)]
pub struct ActiveCamera;

#[derive(Component, Clone)]
#[require(Transform)]
pub struct Camera {
    // Height / Width
    pub aspect: f32,
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
    pub view_proj: Matrix4<f32>,
}

#[derive(Component)]
pub struct CameraGlobalBindGroup(pub Vec<Arc<BindGroup>>);

impl FromWorld for CameraBuffer {
    fn from_world(world: &mut bevy_ecs::world::World) -> Self {
        let rs = world.resource::<crate::RenderState>();
        CameraBuffer::new(&rs.device)
    }
}

impl Camera {
    pub fn build_view_projection_matrix(&self, transform: &WorldTransform) -> Matrix4<f32> {
        let view = transform.view_matrix();
        let proj = perspective(cgmath::Deg(self.fovy), self.aspect, self.znear, self.zfar);
        OPENGL_TO_WGPU_MATRIX * proj * view
    }

    pub fn new(aspect: f32) -> Camera {
        Self {
            aspect,
            fovy: 45.0,
            znear: 0.01,
            zfar: 100.0,
            view_proj: Matrix4::identity(),
        }
    }

    pub fn get_uniform(&self, transform: &WorldTransform) -> CameraUniform {
        let pos = transform.position;
        let dir = transform.forward();
        CameraUniform {
            view_proj: self.build_view_projection_matrix(transform).into(),
            position: [pos.x, pos.y, pos.z, 1.],
            direction: [dir.x, dir.y, dir.z, 1.],
        }
    }
}

impl CameraBuffer {
    pub fn new(device: &wgpu::Device) -> CameraBuffer {
        let camera_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Camera Buffer"),
            size: size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        CameraBuffer {
            buffer: Arc::new(camera_buffer),
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub position: [f32; 4],
    pub direction: [f32; 4],
}

impl_pod_zeroable!(CameraUniform);

#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.5,
    0.0, 0.0, 0.0, 1.0,
);

pub fn sys_create_camera_global_bind_group(
    mut commands: Commands,
    q_camera: Query<Entity, (With<CameraBuffer>, Without<CameraGlobalBindGroup>)>,
) {
    if !q_camera.is_empty() {
        commands.queue(RefreshCameraGlobalBindGroupCmd {
            ids: q_camera.iter().collect(),
        });
    }
}

fn refresh_camera_global_bind_group_by_ids(
    In(to_refresh): In<Vec<Entity>>,

    mut commands: Commands,
    q_camera_buffers: Query<(Entity, &CameraBuffer)>,
    light: Res<LightUnifromBuffer>,
    shadow_map: Res<ShadowMap>,
    dfg: Res<DFGTexture>,
    target: Res<ColorRenderTarget>,
    global_uniform_buffer: Res<GlobalUniformBuffer>,
    layout: Res<GlobalBindGroupLayout>,
    rs: Res<RenderState>,

    default_skybox: Res<DefaultSkybox>,
    skybox_sh: Res<SkyboxSHBuffer>,
    skeybox: Query<&Skybox>,
) {
    let size = target.get_size().unwrap_or_default();
    let buffer = &global_uniform_buffer.buffer;
    rs.queue.write_buffer(
        buffer,
        0,
        bytemuck::cast_slice(&[RawGlobalUniform {
            screen_resolution: [size.width as f32, size.height as f32],
        }]),
    );

    let skybox_texture = skeybox
        .single()
        .ok()
        .and_then(|it| it.texture.as_ref())
        .unwrap_or(&default_skybox.texture);

    let device = &rs.device;
    for (id, camera) in q_camera_buffers
        .iter()
        .filter(|(id, _)| to_refresh.contains(&id))
    {
        let bind_groups = [0, 1]
            .into_iter()
            .map(|it| {
                let image = target.ping_pong[it].as_ref().unwrap();
                let bind_group_desc = bg_descriptor! {
                    ["Main PBR Global BindGroup"][&layout.0]
                    0: camera.buffer.as_entire_binding();
                    1: light.buffer.as_entire_binding();
                    2: BindingResource::TextureView(&shadow_map.image.view);
                    3: BindingResource::Sampler(&shadow_map.image.sampler);
                    4: BindingResource::TextureView(&dfg.texture.view);
                    5: BindingResource::TextureView(&skybox_texture.view);
                    6: BindingResource::Sampler(&dfg.texture.sampler); // todo cubemap sampler
                    7: skybox_sh.buffer.as_entire_binding();
                    8: BindingResource::TextureView(&image.view);
                    9: BindingResource::Sampler(&image.sampler);
                    10: global_uniform_buffer.buffer.as_entire_binding();
                };
                Arc::new(device.create_bind_group(&bind_group_desc))
            })
            .collect::<Vec<_>>();
        commands
            .entity(id)
            .insert(CameraGlobalBindGroup(bind_groups));
    }
}

#[derive(Debug, Clone)]
pub struct RefreshCameraGlobalBindGroupCmd {
    pub ids: Vec<Entity>,
}

#[derive(Default, Debug, Clone)]
pub struct RefreshAllCameraGlobalBindGroupCmd;

impl Command for RefreshCameraGlobalBindGroupCmd {
    fn apply(self, world: &mut World) -> () {
        world
            .run_system_once_with(refresh_camera_global_bind_group_by_ids, self.ids)
            .unwrap();
    }
}

impl Command for RefreshAllCameraGlobalBindGroupCmd {
    fn apply(self, world: &mut World) {
        let entities = world
            .query_filtered::<Entity, With<CameraBuffer>>()
            .iter(world)
            .collect::<Vec<_>>();
        world
            .run_system_once_with(refresh_camera_global_bind_group_by_ids, entities)
            .unwrap();
    }
}

pub fn sys_update_camera_uniform(
    mut commands: Commands,
    single: Single<
        (Entity, &mut Camera, &WorldTransform, Option<&CameraBuffer>),
        (Or<(Changed<Camera>, Changed<WorldTransform>)>,),
    >,
    rs: Res<RenderState>,
) {
    let (id, mut camera, transform, camera_buffer) = single.into_inner();

    match camera_buffer {
        Some(camera_buffer) => {
            camera.view_proj = camera.build_view_projection_matrix(transform);

            rs.queue.write_buffer(
                &camera_buffer.buffer,
                0,
                bytemuck::cast_slice(&[camera.get_uniform(transform)]),
            );
        }
        None => {
            commands.entity(id).insert(CameraBuffer::new(&rs.device));
        }
    };
}
