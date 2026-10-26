use bevy_reflect::Reflect;
use std::sync::Arc;

use crate::transform::TransformPostUpdateSet;
use crate::{SurfaceState, prelude::*};
use bevy_app::{Plugin, PostUpdate, PreUpdate};
use bevy_ecs::component::Component;
use bevy_ecs::prelude::*;
use bevy_ecs::query::{Changed, Or};
use bevy_ecs::system::Single;
use bevy_ecs::world::FromWorld;
use cgmath::{Matrix4, SquareMatrix, perspective};
use lentille_core::window::PrimaryWindow;
use lentille_wgpu_utils::{
    impl_pod_zeroable,
    typed_texture::{TypedTexture, TypedTextureViewDescriptor},
};
use wgpu::TextureDimension;

use crate::RenderState;

pub type ColorImage = UploadedImage<Dim2D, SampleFloatFilterable>;
pub type DepthImage = UploadedImage<Dim2D, SampleDepth>;

use super::transform::{Transform, WorldTransform};

#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.0,
    0.0, 0.0, 0.5, 1.0,
);

pub(crate) struct CameraPlugin;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, SystemSet)]
pub struct CameraPostUpdateSet;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_systems(
            PostUpdate,
            (sys_create_render_target, sys_create_or_update_camera_buffer)
                .in_set(CameraPostUpdateSet)
                .after(TransformPostUpdateSet)
                .chain(),
        )
        .add_systems(PreUpdate, sys_resize_render_target);
    }
}

/// Camera component
/// todo add comments
#[derive(Component, Clone, Debug, PartialEq, Reflect)]
#[reflect(Component)]
#[require(Transform, RenderTargetConfig, CameraState)]
pub struct Camera {
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
}

#[derive(Component, Clone)]
pub struct CameraState {
    pub aspect: f32,
    pub view_proj: Matrix4<f32>,
}
impl Default for CameraState {
    fn default() -> Self {
        Self {
            aspect: 1.0,
            view_proj: Matrix4::identity(),
        }
    }
}

/// Store wgpu buffer for gpu of camera's info
#[derive(Component)]
pub struct CameraBuffer {
    pub buffer: Arc<TypedBuffer<CameraUniform>>,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub inv_view_proj: [[f32; 4]; 4],
    pub position: [f32; 4],
    pub direction: [f32; 4],
    pub resolution: [f32; 4],
}

impl_pod_zeroable!(CameraUniform);

// -------- RenderTarget --------

/// 这是一个用于延迟产生 RenderTarget 的结构体。
/// 因为生成相机的时候主窗口可能不存在。
#[derive(Default, Clone, Debug, PartialEq, Component, Reflect)]
#[reflect(Component)]
pub enum RenderTargetConfig {
    #[default]
    Viewport,
    PrimaryWindow,
    Window(#[entities] Entity),
    Texture {
        width: u32,
        height: u32,
    },
}

/// Linear scene colors. Clone for a camera frame; carry the index across stages.
/// A pass writes `alternate()` then calls `swap()` when that result is ready.
#[derive(Clone)]
pub struct HdrColorTargets {
    pub(crate) images: [Arc<ColorImage>; 2],
    current: usize,
}

impl HdrColorTargets {
    pub fn current(&self) -> &Arc<ColorImage> {
        &self.images[self.current]
    }

    pub fn alternate(&self) -> &Arc<ColorImage> {
        &self.images[self.alternate_index()]
    }

    pub(crate) fn alternate_index(&self) -> usize {
        1 - self.current
    }

    pub fn swap(&mut self) {
        self.current = self.alternate_index();
    }

    /// Preserve uncovered pixels before a partial-screen pass such as transparency.
    pub fn copy_and_swap(&mut self, encoder: &mut wgpu::CommandEncoder) {
        lentille_wgpu_utils::copy_texture2d_to_texture2d_no_mip(
            encoder,
            self.current().texture.texture(),
            self.alternate().texture.texture(),
            self.current().texture.size(),
        );
        self.swap();
    }
}

/// HDR ping-pong is frame-local; the stable SDR display is the consumer boundary.
#[derive(Component)]
#[require(RenderTargetSize)]
pub struct RenderTarget {
    pub target_type: TargetType,
    pub(crate) hdr: HdrColorTargets,
    display: Arc<ColorImage>,
    pub depth: Option<Arc<DepthImage>>,
}

/// RenderTargetSize 是外部控制 RenderTarget 的大小的组件
#[derive(Component, Clone)]
pub struct RenderTargetSize {
    pub width: u32,
    pub height: u32,
}

pub enum TargetType {
    WindowAndSurface(Entity),
    Texture(Arc<ColorImage>),
}

#[derive(Event, Clone)]
pub struct RenderTargetResizedEvent {
    pub render_target_entity: Entity,
    pub new_width: u32,
    pub new_height: u32,
}

// ------- Impls -------

impl FromWorld for CameraBuffer {
    fn from_world(world: &mut bevy_ecs::world::World) -> Self {
        let rs = world.resource::<crate::RenderState>();
        CameraBuffer::new(&rs.device)
    }
}

impl Camera {
    pub fn build_view_projection_matrix(
        &self,
        transform: &WorldTransform,
        aspect: f32,
    ) -> Matrix4<f32> {
        let view = transform.view_matrix();
        let proj = perspective(cgmath::Deg(self.fovy), aspect, self.znear, self.zfar);
        OPENGL_TO_WGPU_MATRIX * proj * view
    }

    pub fn new(_aspect: f32) -> Camera {
        Self {
            fovy: 45.0,
            znear: 0.01,
            zfar: 1000.0,
        }
    }

    pub fn get_uniform(&self, transform: &WorldTransform, resolution: [f32; 2]) -> CameraUniform {
        let pos = transform.position;
        let dir = transform.forward();
        let view_proj =
            self.build_view_projection_matrix(transform, resolution[0] / resolution[1].max(1.0));
        let inv_view_proj = view_proj.invert().unwrap_or(Mat4::identity());

        CameraUniform {
            view_proj: view_proj.into(),
            inv_view_proj: inv_view_proj.into(),
            position: [pos.x, pos.y, pos.z, 1.],
            direction: [dir.x, dir.y, dir.z, 1.],
            resolution: [resolution[0], resolution[1], 0.0, 0.0],
        }
    }
}

impl CameraBuffer {
    pub fn new(device: &wgpu::Device) -> CameraBuffer {
        let camera_buffer = TypedBuffer::new(
            device,
            Some("Camera Buffer"),
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );

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
    pub fn display_color(&self) -> Arc<ColorImage> {
        Arc::clone(&self.display)
    }

    /// Base attachments: two HDR colors, SDR output, depth, packed G-buffer.
    /// Excludes optional picking/outline allocations.
    pub fn estimated_attachment_bytes(width: u32, height: u32) -> u64 {
        u64::from(width.max(1))
            .saturating_mul(u64::from(height.max(1)))
            .saturating_mul(8 * 2 + 4 + 4 + 16)
    }

    /// Return `(hdr, display, depth)`.
    fn create_images(
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        has_depth: bool,
        device: &wgpu::Device,
    ) -> (HdrColorTargets, Arc<ColorImage>, Option<Arc<DepthImage>>) {
        let width = width.max(1);
        let height = height.max(1);

        let hdr = HdrColorTargets {
            images: std::array::from_fn(|_| {
                Arc::new(create_color_render_target_image(
                    width,
                    height,
                    device,
                    crate::SCENE_COLOR_FORMAT,
                ))
            }),
            current: 0,
        };
        let display = Arc::new(create_color_render_target_image(
            width, height, device, format,
        ));

        let depth = if has_depth {
            Some(Arc::new(create_depth_texture(width, height, device)))
        } else {
            None
        };

        (hdr, display, depth)
    }

    pub fn from_window(
        window_entity: Entity,
        surface_state: &SurfaceState,
        device: &wgpu::Device,
    ) -> Self {
        let width = surface_state.config.width;
        let height = surface_state.config.height;
        let (hdr, display, depth) =
            Self::create_images(width, height, crate::SCREEN_FORMAT, true, device);

        Self {
            target_type: TargetType::WindowAndSurface(window_entity),
            hdr,
            display,
            depth,
        }
    }

    pub fn new_texture_target(
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        device: &wgpu::Device,
    ) -> Self {
        assert_eq!(
            format,
            crate::SCREEN_FORMAT,
            "Scene display targets must use SCREEN_FORMAT"
        );
        let (hdr, display, depth) = Self::create_images(width, height, format, true, device);

        Self {
            target_type: TargetType::Texture(Arc::clone(&display)),
            hdr,
            display,
            depth,
        }
    }
}

// --------- Systems ---------

/// Watch `RenderTargetSize`'s change and update textures
pub fn sys_resize_render_target(
    mut commands: Commands,
    q_render_target: Query<
        (
            Entity,
            &mut CameraState,
            &mut RenderTarget,
            &RenderTargetSize,
        ),
        Changed<RenderTargetSize>,
    >,
    mut q_window_surface: Query<&mut SurfaceState>,
    rs: Res<RenderState>,
) {
    for (id, mut camera, mut target, size) in q_render_target {
        let height = size.height.max(1);
        let width = size.width.max(1);
        camera.aspect = width as f32 / height as f32;
        let format = match &target.target_type {
            TargetType::WindowAndSurface(entity) => {
                q_window_surface
                    .get_mut(*entity)
                    .ok()
                    .map(|mut surface_state| {
                        surface_state.config.width = width;
                        surface_state.config.height = height;
                        surface_state.configure(&rs.device);
                        crate::SCREEN_FORMAT
                    })
            }
            TargetType::Texture(uploaded_image) => Some(uploaded_image.texture.format()),
        };

        if let Some(format) = format {
            (target.hdr, target.display, target.depth) = RenderTarget::create_images(
                width,
                height,
                format,
                target.depth.is_some(),
                &rs.device,
            );
            let display = target.display_color();
            if let TargetType::Texture(ref mut uploaded_image) = target.target_type {
                *uploaded_image = display;
            }
        }

        commands.trigger(RenderTargetResizedEvent {
            render_target_entity: id,
            new_width: width,
            new_height: height,
        });
    }
}

fn sys_create_or_update_camera_buffer(
    mut commands: Commands,
    q_camera: Query<
        (
            Entity,
            &Camera,
            &mut CameraState,
            &WorldTransform,
            &RenderTargetSize,
            Option<&CameraBuffer>,
        ),
        (
            Or<(
                Changed<Camera>,
                Changed<WorldTransform>,
                Changed<RenderTargetSize>,
                Added<CameraState>,
                Without<CameraBuffer>,
            )>,
        ),
    >,
    rs: Res<RenderState>,
) {
    for (id, camera, mut state, transform, render_target_size, camera_buffer) in q_camera {
        state.aspect = render_target_size.width as f32 / render_target_size.height.max(1) as f32;
        state.view_proj = camera.build_view_projection_matrix(transform, state.aspect);
        match camera_buffer {
            Some(camera_buffer) => {
                camera_buffer.buffer.write(
                    camera.get_uniform(
                        transform,
                        [
                            render_target_size.width as f32,
                            render_target_size.height as f32,
                        ],
                    ),
                    &rs.queue,
                );
            }
            None => {
                let buffer = CameraBuffer::new(&rs.device);
                buffer.buffer.write(
                    camera.get_uniform(
                        transform,
                        [
                            render_target_size.width as f32,
                            render_target_size.height as f32,
                        ],
                    ),
                    &rs.queue,
                );
                commands.entity(id).insert(buffer);
            }
        };
    }
}

fn sys_create_render_target(
    mut commands: Commands,
    q_camera: Query<
        (Entity, &RenderTargetConfig),
        Or<(Without<RenderTarget>, Changed<RenderTargetConfig>)>,
    >,
    q_primary_window: Option<Single<(Entity, &SurfaceState), With<PrimaryWindow>>>,
    q_window: Query<&SurfaceState, Without<PrimaryWindow>>,
    rs: Res<RenderState>,
) {
    let device = &rs.device;
    for (id, config) in q_camera {
        let make_result: Option<(RenderTarget, u32, u32)> = match config {
            RenderTargetConfig::PrimaryWindow => q_primary_window.as_ref().map(|window| {
                let (window_id, surface_state) = **window;
                (
                    RenderTarget::from_window(window_id, surface_state, device),
                    surface_state.config.width,
                    surface_state.config.height,
                )
            }),
            RenderTargetConfig::Window(entity) => q_window.get(*entity).ok().map(|surface_state| {
                (
                    RenderTarget::from_window(*entity, surface_state, device),
                    surface_state.config.width,
                    surface_state.config.height,
                )
            }),
            RenderTargetConfig::Viewport => Some((
                RenderTarget::new_texture_target(1600, 900, crate::SCREEN_FORMAT, device),
                1600,
                900,
            )),
            RenderTargetConfig::Texture { width, height } => Some((
                RenderTarget::new_texture_target(*width, *height, crate::SCREEN_FORMAT, device),
                *width,
                *height,
            )),
        };

        if let Some((render_target, width, height)) = make_result {
            commands
                .entity(id)
                .insert((render_target, RenderTargetSize { width, height }));
            commands.trigger(RenderTargetResizedEvent {
                render_target_entity: id,
                new_width: width,
                new_height: height,
            });
        }
    }
}

// --------- Target Creation Functions ---------

pub fn create_color_render_target_image(
    width: u32,
    height: u32,
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
) -> ColorImage {
    let size = Extent3d {
        width: width,
        height: height,
        depth_or_array_layers: 1,
    };
    // 对 sRGB 颜色目标额外暴露其线性对应格式的视图，
    // 便于像 egui 这样自带 gamma 处理的消费者直接采样原始字节，
    // 避免硬件 sRGB 解码 + shader 内手动解码导致的双重 gamma。
    let linear_view = linear_view_format_of(format);
    let view_formats: &[wgpu::TextureFormat] = match &linear_view {
        Some(f) => std::slice::from_ref(f),
        None => &[],
    };
    let desc = TextureDescriptor {
        label: Some("Render Target"),
        size,
        format,
        usage: TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_SRC
            | TextureUsages::COPY_DST
            | TextureUsages::RENDER_ATTACHMENT,
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        view_formats,
    };
    let texture = TypedTexture::from_descriptor(device, &desc);
    let view = texture.create_view(&TypedTextureViewDescriptor::new(Some("Render Target View")));

    ColorImage { texture, view }
}

/// 返回给定 sRGB 颜色格式对应的线性 UNORM 格式；非 sRGB 输入返回 `None`。
pub fn linear_view_format_of(format: wgpu::TextureFormat) -> Option<wgpu::TextureFormat> {
    match format {
        wgpu::TextureFormat::Rgba8UnormSrgb => Some(wgpu::TextureFormat::Rgba8Unorm),
        wgpu::TextureFormat::Bgra8UnormSrgb => Some(wgpu::TextureFormat::Bgra8Unorm),
        _ => None,
    }
}

pub fn create_depth_texture(width: u32, height: u32, device: &wgpu::Device) -> DepthImage {
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
    let texture = TypedTexture::from_descriptor(device, &desc);
    let view = texture.create_view(&TypedTextureViewDescriptor::new(Some("Depth Texture View")));

    DepthImage { texture, view }
}
