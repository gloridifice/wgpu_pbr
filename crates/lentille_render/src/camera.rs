use std::sync::Arc;

use crate::bindings::global_binding::{GlobalBindGroupLayout, RawGlobalUniform};
use crate::{SurfaceState, prelude::*};
use bevy_app::{Plugin, PostUpdate};
use bevy_ecs::component::Component;
use bevy_ecs::prelude::*;
use bevy_ecs::query::{Changed, Or};
use bevy_ecs::system::{RunSystemOnce, Single};
use bevy_ecs::world::FromWorld;
use cgmath::{Matrix4, SquareMatrix, perspective};
use gltf::animation::Target;
use lentille_wgpu_utils::impl_pod_zeroable;
use wgpu::{BindGroup, BufferDescriptor, TextureDimension};
use winit::window::WindowId;

use crate::RenderState;
use crate::dfg::DFGTexture;
use crate::light::LightUnifromBuffer;
use crate::prelude::GlobalUniformBuffer;
use crate::shadow_mapping::ShadowMap;
use crate::skybox::{DefaultSkybox, SkyboxSHBuffer};

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
pub struct CameraTarget(pub Option<wgpu::TextureView>);

#[derive(Component)]
pub struct CameraGlobalBindGroup {
    /// 该值应该与 RenderTarget 的 is_current_color_a 值保持一致
    is_current_b: bool,
    /// 对应 RenderTarget 的 color_a
    a: Arc<BindGroup>,
    /// 对应 RenderTarget 的 color_b
    b: Arc<BindGroup>,
}

impl CameraGlobalBindGroup {
    pub fn next(&mut self) -> Arc<BindGroup> {
        self.is_current_b = !self.is_current_b;
        Arc::clone(if self.is_current_b { &self.b } else { &self.a })
    }
}

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
    q_camera_buffers: Query<(Entity, &CameraBuffer, &RenderTarget)>,
    light: Res<LightUnifromBuffer>,
    shadow_map: Res<ShadowMap>,
    dfg: Res<DFGTexture>,
    global_uniform_buffer: Res<GlobalUniformBuffer>,
    layout: Res<GlobalBindGroupLayout>,
    rs: Res<RenderState>,

    default_skybox: Res<DefaultSkybox>,
    skybox_sh: Res<SkyboxSHBuffer>,
    skeybox: Query<&Skybox>,
) {
    let skybox_texture = skeybox
        .single()
        .ok()
        .and_then(|it| it.texture.as_ref())
        .unwrap_or(&default_skybox.texture);

    let device = &rs.device;
    for (id, camera, target) in q_camera_buffers
        .iter()
        .filter(|(id, _, _)| to_refresh.contains(&id))
    {
        let size = target.color_a.texture.size();
        let buffer = &global_uniform_buffer.buffer;
        rs.queue.write_buffer(
            buffer,
            0,
            bytemuck::cast_slice(&[RawGlobalUniform {
                screen_resolution: [size.width as f32, size.height as f32],
            }]),
        );
        let bind_groups = [0, 1]
            .into_iter()
            .map(|it| {
                let image = if it == 0 {
                    &target.color_a
                } else {
                    &target.color_b
                };
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
                    9: BindingResource::Sampler(&image.sampler); //TODO A universal sampler
                    10: global_uniform_buffer.buffer.as_entire_binding();
                };
                Arc::new(device.create_bind_group(&bind_group_desc))
            })
            .collect::<Vec<_>>();
        commands.entity(id).insert(CameraGlobalBindGroup {
            is_current_b: false,
            a: bind_groups[0],
            b: bind_groups[1],
        });
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

// ========= RenderTarget ============

/// RenderTarget 采用 PingPong 的方式，
/// 在所有渲染阶段完成后会拷贝到 view 中
#[derive(Component)]
#[require(RenderTargetSize)]
pub struct RenderTarget {
    target_type: TargetType,
    /// true = a, false = b
    is_current_color_a: bool,
    color_a: Arc<UploadedImage>,
    color_b: Arc<UploadedImage>,
    pub depth: Option<Arc<UploadedImage>>,
}

/// RenderTargetSize 是外部控制 RenderTarget 的大小的组件
#[derive(Component)]
pub struct RenderTargetSize {
    width: u32,
    height: u32,
}

impl Default for RenderTargetSize {
    fn default() -> Self {
        Self {
            width: 512,
            height: 512,
        }
    }
}

pub fn sys_resize_render_target(
    q_render_target: Query<(&mut RenderTarget, &RenderTargetSize), Changed<RenderTargetSize>>,
    q_window_surface: Query<&SurfaceState>,
    rs: Res<RenderState>,
) {
    for (mut target, size) in q_render_target {
        let format = match &target.target_type {
            TargetType::WindowAndSurface(entity) => {
                q_window_surface
                    .get(*entity)
                    .expect("SurfaceState not exists")
                    .config
                    .format
            }
            TargetType::Texture(uploaded_image) => uploaded_image.texture.format(),
        };

        (target.color_a, target.color_b, target.depth) = RenderTarget::create_images(
            size.width,
            size.height,
            format,
            target.depth.is_some(),
            &rs.device,
        );
    }
}

pub enum TargetType {
    WindowAndSurface(Entity),
    Texture(Arc<UploadedImage>),
}

impl RenderTarget {
    pub fn next(&mut self) -> Arc<UploadedImage> {
        self.is_current_color_a = !self.is_current_color_a;
        Arc::clone(if self.is_current_color_a {
            &self.color_a
        } else {
            &self.color_b
        })
    }

    /// Return `(color_a, color_b, depth)`
    fn create_images(
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        has_depth: bool,
        device: &wgpu::Device,
    ) -> (
        Arc<UploadedImage>,
        Arc<UploadedImage>,
        Option<Arc<UploadedImage>>,
    ) {
        let color_a = Arc::new(create_color_render_target_image(
            width, height, device, format,
        ));

        let color_b = Arc::new(create_color_render_target_image(
            width, height, device, format,
        ));

        let depth = if has_depth {
            Some(Arc::new(create_depth_texture(width, height, device)))
        } else {
            None
        };

        (color_a, color_b, depth)
    }

    pub fn from_window(
        window_entity: Entity,
        surface_state: &SurfaceState,
        device: &wgpu::Device,
    ) -> Self {
        let size = surface_state.size;
        let width = size.width;
        let height = size.height;
        let format = surface_state.config.format;

        let (color_a, color_b, depth) = Self::create_images(width, height, format, true, device);

        Self {
            target_type: TargetType::WindowAndSurface(window_entity),
            is_current_color_a: false,
            color_a,
            color_b,
            depth,
        }
    }

    pub fn new_texture_target(
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        device: &wgpu::Device,
    ) -> Self {
        let color = Arc::new(create_color_render_target_image(
            width, height, device, format,
        ));

        let (color_a, color_b, depth) = Self::create_images(width, height, format, true, device);

        Self {
            target_type: TargetType::Texture(color),
            is_current_color_a: false,
            color_a,
            color_b,
            depth,
        }
    }
}

pub fn create_color_render_target_image(
    width: u32,
    height: u32,
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
) -> UploadedImage {
    let size = Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let desc = TextureDescriptor {
        label: Some("Render Target"),
        size,
        format,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_SRC | TextureUsages::COPY_DST,
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        view_formats: &[],
    };
    let texture = device.create_texture(&desc);
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    UploadedImage { texture, view }
}

pub fn create_depth_texture(
    width: u32,
    height: u32,
    device: &wgpu::Device,
    // compare: Option<wgpu::CompareFunction>, TODO
) -> UploadedImage {
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let desc = wgpu::TextureDescriptor {
        label: Some("Depth Texture"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: RenderState::DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[RenderState::DEPTH_FORMAT],
    };
    let texture = device.create_texture(&desc);
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    // let sampler = device.create_sampler(&{
    //     let mut desc = lentille_wgpu_utils::sampler_desc_no_filter();
    //     desc.compare = compare;
    //     desc
    // });

    UploadedImage { texture, view }
}
