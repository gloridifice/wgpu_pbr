use std::sync::Arc;

use crate::base_assets::NoFilterSampler;
use crate::bindings::global_binding::GlobalBindGroupLayout;
use crate::{SurfaceState, prelude::*};
use bevy_app::{Plugin, PostUpdate, Update};
use bevy_ecs::component::Component;
use bevy_ecs::prelude::*;
use bevy_ecs::query::{Changed, Or};
use bevy_ecs::system::{RunSystemOnce, Single};
use bevy_ecs::world::FromWorld;
use cgmath::{Matrix4, SquareMatrix, perspective};
use lentille_core::window::PrimaryWinodw;
use lentille_wgpu_utils::impl_pod_zeroable;
use wgpu::{BindGroup, BufferDescriptor, TextureDimension};

use crate::RenderState;
use crate::base_assets::DFGTexture;
use crate::light::LightUnifromBuffer;
use crate::shadow_mapping::ShadowMap;
use crate::skybox::{DefaultSkybox, SkyboxSHBuffer};

use super::transform::{Transform, WorldTransform};

#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.5,
    0.0, 0.0, 0.0, 1.0,
);

pub(crate) struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_systems(
            PostUpdate,
            (
                sys_create_render_target,
                sys_crate_or_update_camera_buffer,
                sys_create_camera_global_bind_group,
            )
                .chain(),
        )
        .add_systems(Update, sys_resize_render_target);
    }
}

/// Camera component
/// todo add comments
#[derive(Component, Clone)]
#[require(Transform, RenderTargetConfig)]
pub struct Camera {
    /// Height / Width
    pub aspect: f32,
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
    pub view_proj: Matrix4<f32>,
}

/// Store wgpu buffer for gpu of camera's info
#[derive(Component)]
pub struct CameraBuffer {
    pub buffer: Arc<wgpu::Buffer>,
}

/// Store global bind group for gpu
#[derive(Component)]
pub struct CameraGlobalBindGroup {
    /// 该值应该与 RenderTarget 的 is_current_color_a 值保持一致
    is_current_b: bool,
    /// 对应 RenderTarget 的 color_a
    a: Arc<BindGroup>,
    /// 对应 RenderTarget 的 color_b
    b: Arc<BindGroup>,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub position: [f32; 4],
    pub direction: [f32; 4],
    pub resolution: [f32; 2],
}

impl_pod_zeroable!(CameraUniform);

#[derive(Debug, Clone)]
pub struct RefreshCameraGlobalBindGroupCmd {
    pub ids: Vec<Entity>,
}

#[derive(Default, Debug, Clone)]
pub struct RefreshAllCameraGlobalBindGroupCmd;

// -------- RenderTarget --------

/// 这是一个用于延迟产生 RenderTarget 的结构体。
/// 因为生成相机的时候主窗口可能不存在。
#[derive(Default, Clone, Debug, Component)]
pub enum RenderTargetConfig {
    #[default]
    PrimaryWindow,
    Window(Entity),
    Texture {
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    },
}

/// RenderTarget 采用 PingPong 的方式，
/// 在所有渲染阶段完成后会拷贝到 view 中
#[derive(Component)]
#[require(RenderTargetSize)]
pub struct RenderTarget {
    pub target_type: TargetType,
    /// true = a, false = b
    is_current_color_a: bool,
    color_a: Arc<UploadedImage>,
    color_b: Arc<UploadedImage>,
    pub depth: Option<Arc<UploadedImage>>,
}

/// RenderTargetSize 是外部控制 RenderTarget 的大小的组件
#[derive(Component)]
pub struct RenderTargetSize {
    pub width: u32,
    pub height: u32,
}

pub enum TargetType {
    WindowAndSurface(Entity),
    Texture(Arc<UploadedImage>),
}

// ------- Impls -------

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

    pub fn get_uniform(&self, transform: &WorldTransform, resolution: [f32; 2]) -> CameraUniform {
        let pos = transform.position;
        let dir = transform.forward();
        CameraUniform {
            view_proj: self.build_view_projection_matrix(transform).into(),
            position: [pos.x, pos.y, pos.z, 1.],
            direction: [dir.x, dir.y, dir.z, 1.],
            resolution,
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

impl Default for RenderTargetSize {
    fn default() -> Self {
        Self {
            width: 512,
            height: 512,
        }
    }
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

    pub fn get_current_color(&self) -> Arc<UploadedImage> {
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
        let width = width.max(1);
        let height = height.max(1);

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

// --------- Systems ---------

/// 负责监控 `RenderTargetSize` 的变化并更新纹理
pub fn sys_resize_render_target(
    q_render_target: Query<
        (&mut Camera, &mut RenderTarget, &RenderTargetSize),
        Changed<RenderTargetSize>,
    >,
    mut q_window_surface: Query<&mut SurfaceState>,
    rs: Res<RenderState>,
) {
    for (mut camera, mut target, size) in q_render_target {
        camera.aspect = size.height as f32 / size.height as f32;
        let format = match &target.target_type {
            TargetType::WindowAndSurface(entity) => {
                q_window_surface
                    .get_mut(*entity)
                    .ok()
                    .map(|mut surface_state| {
                        surface_state.config.width = size.width;
                        surface_state.config.height = size.height;
                        surface_state
                            .surface
                            .configure(&rs.device, &surface_state.config);
                        surface_state.config.format
                    })
            }
            TargetType::Texture(uploaded_image) => Some(uploaded_image.texture.format()),
        };

        if let Some(format) = format {
            (target.color_a, target.color_b, target.depth) = RenderTarget::create_images(
                size.width,
                size.height,
                format,
                target.depth.is_some(),
                &rs.device,
            );
        }
    }
}

fn sys_crate_or_update_camera_buffer(
    mut commands: Commands,
    q_camera: Query<
        (
            Entity,
            &mut Camera,
            &WorldTransform,
            &RenderTargetSize,
            Option<&CameraBuffer>,
        ),
        (Or<(Changed<Camera>, Changed<WorldTransform>)>,),
    >,
    rs: Res<RenderState>,
) {
    for (id, mut camera, transform, render_target_size, camera_buffer) in q_camera {
        match camera_buffer {
            Some(camera_buffer) => {
                camera.view_proj = camera.build_view_projection_matrix(transform);

                rs.queue.write_buffer(
                    &camera_buffer.buffer,
                    0,
                    bytemuck::cast_slice(&[camera.get_uniform(
                        transform,
                        [
                            render_target_size.width as f32,
                            render_target_size.height as f32,
                        ],
                    )]),
                );
            }
            None => {
                commands.entity(id).insert(CameraBuffer::new(&rs.device));
            }
        };
    }
}

fn sys_create_render_target(
    mut commands: Commands,
    q_camera: Query<(Entity, &RenderTargetConfig), Without<RenderTarget>>,
    q_primary_window: Option<Single<&SurfaceState, With<PrimaryWinodw>>>,
    q_window: Query<&SurfaceState, Without<PrimaryWinodw>>,
    rs: Res<RenderState>,
) {
    let device = &rs.device;
    for (id, config) in q_camera {
        let render_target = match config {
            RenderTargetConfig::PrimaryWindow => q_primary_window
                .as_ref()
                .map(|surface_state| RenderTarget::from_window(id, &surface_state, device)),
            RenderTargetConfig::Window(entity) => q_window
                .get(*entity)
                .ok()
                .map(|surface_state| RenderTarget::from_window(*entity, surface_state, device)),
            RenderTargetConfig::Texture {
                width,
                height,
                format,
            } => Some(RenderTarget::new_texture_target(
                *width, *height, *format, device,
            )),
        };

        if let Some(render_target) = render_target {
            commands
                .entity(id)
                .insert(render_target)
                .remove::<RenderTargetConfig>();
        }
    }
}

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
    layout: Res<GlobalBindGroupLayout>,
    rs: Res<RenderState>,
    no_filter_sampler: Res<NoFilterSampler>,

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
        let mut bind_groups = [0, 1]
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
                    9: BindingResource::Sampler(&no_filter_sampler.0);
                };
                Arc::new(device.create_bind_group(&bind_group_desc))
            })
            .collect::<Vec<_>>();

        commands.entity(id).insert(CameraGlobalBindGroup {
            is_current_b: false,
            a: bind_groups.remove(0),
            b: bind_groups.remove(0),
        });
    }
}

// --------- Target Creation Functions ---------

pub fn create_color_render_target_image(
    width: u32,
    height: u32,
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
) -> UploadedImage {
    let size = Extent3d {
        width: width,
        height: height,
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

pub fn create_depth_texture(width: u32, height: u32, device: &wgpu::Device) -> UploadedImage {
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

    UploadedImage { texture, view }
}

pub fn create_depth_sampler(
    compare: Option<wgpu::CompareFunction>,
    device: &wgpu::Device,
) -> Sampler {
    let sampler = device.create_sampler(&{
        let mut desc = lentille_wgpu_utils::sampler_desc_no_filter();
        desc.compare = compare;
        desc
    });
    sampler
}
