use std::{
    mem::swap,
    sync::{Arc, LazyLock},
};

use bevy_app::prelude::*;
use bevy_ecs::{prelude::*, schedule::ScheduleLabel};
use bevy_log::info;
use lentille_core::window::{PrimaryWindowCreatedEvent, WinitWindow};
use pollster::block_on;
use prelude::*;
use shader_loader::ShaderLoader;
use wgpu::{Features, Instance};

use systems::*;
use winit::{dpi::PhysicalSize, window::Window};

use crate::{
    app_ext::AppExt,
    base_assets::BaseAssetsPlugin,
    bindings::BindingsPlugin,
    blit::BlitPlugin,
    camera::CameraPlugin,
    cubemap::CubemapPlugin,
    deferred_rendering::DeferredRenderingPlugin,
    gizmo::GizmoPlugin,
    graph::after,
    light::LightPlugin,
    resource::{RENDER_RESOURCES_TO_ADD, ResourceGraph},
    shadow_mapping::ShadowMappingPlugin,
    skybox::SkyBoxPlugin,
    stage::StagePlugin,
    transform::TransformPlugin,
    transparent::TransparentPlugin,
};

pub mod asset;
pub mod base_assets;
pub mod bindings;
pub mod blit;
pub mod camera;
pub mod cubemap;
pub mod deferred_rendering;
pub mod gizmo;
pub mod graph;
pub mod image;
pub mod light;
pub mod material;
pub mod mesh;
pub mod mipmap;
pub mod prelude;
pub mod resource;
pub mod shader_loader;
pub mod shadow_mapping;
pub mod skybox;
pub mod stage;
pub mod systems;
pub mod transform;

pub static DEVICE_FEATURES: LazyLock<Arc<Vec<Features>>> =
    LazyLock::new(|| Arc::new(vec![Features::TIMESTAMP_QUERY]));

pub mod app_ext;
/// 想要一个物体以 Transparent 的管线渲染，需要至少有以下 Component:
/// - `TransparentPassObject`
/// - `WorldTransform`
/// - `MeshRenderer`
///
/// 同时物体不能持有 `MainPassObject`
pub mod transparent;
pub mod utils;

// TODO 检查这个 format 的正确性
// #[cfg(target_os = "windows")]
pub static SCREEN_FORMAT: TextureFormat = TextureFormat::Bgra8UnormSrgb;
// #[cfg(not(target_os = "windows"))]
// pub static SCREEN_FORMAT: TextureFormat = TextureFormat::Rgba8UnormSrgb;

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_plugins((
            StagePlugin,
            TransformPlugin,
            LightPlugin,
            CameraPlugin,
            CubemapPlugin,
            SkyBoxPlugin,
            BindingsPlugin,
            ShadowMappingPlugin,
            TransparentPlugin,
            BaseAssetsPlugin,
            DeferredRenderingPlugin,
            BlitPlugin,
            GizmoPlugin,
        ))
        .init_resource::<ShaderLoader>()
        .init_resource::<RenderState>();

        // Configure RenderSets
        app.configure_sets(
            Last,
            (
                FrameSets::Prepare,
                FrameSets::PreDraw,
                FrameSets::Draw,
                FrameSets::PostDraw,
                FrameSets::Present,
            )
                .chain()
                .run_if(resource_exists::<RenderState>),
        );

        // 初始化 RenderState 和初始化资源
        app.add_observer(sys_init_window);

        app.configure_render_stage::<OpaqueStage>([after::<PreStage>()])
            .configure_render_stage::<TransparentStage>([after::<OpaqueStage>()])
            .add_systems(Update, sys_create_surface)
            .add_systems(
                PostUpdate,
                material::pbr::sys_update_override_pbr_material_bind_group,
            )
            .add_frame_system::<PreStage, _, _>(sys_render_cascade_shadow_mapping_pass, [])
            .add_frame_system::<PreStage, _, _>(sys_render_shadow_mapping_pass, [])
            .add_frame_system::<OpaqueStage, _, _>(sys_render_write_g_buffer_pass, [])
            .add_frame_system::<OpaqueStage, _, _>(sys_render_main_pass, [])
            .add_frame_system::<TransparentStage, _, _>(sys_render_transparent, []);
    }
}

pub struct PreStage;
pub struct OpaqueStage;
pub struct TransparentStage;

#[derive(Component, Clone)]
pub struct MainPassObject;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlphaMode {
    Opaque,
    Blend,
}

/// 在 Startup 之后
#[derive(Debug, ScheduleLabel, PartialEq, Eq, Hash, Clone, Copy)]
pub struct RenderPreparedStartup;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameSets {
    /// Create RenderContext
    Prepare,
    PreDraw,
    Draw,
    PostDraw,
    Present,
}

#[derive(Resource)]
pub struct RenderState {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

#[derive(Component)]
pub struct SurfaceState {
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
    pub size: PhysicalSize<u32>,
}

impl FromWorld for RenderState {
    fn from_world(_world: &mut World) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            #[cfg(not(target_arch = "wasm32"))]
            backends: wgpu::Backends::PRIMARY,
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::GL,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });

        block_on(Self::new(instance))
    }
}

impl RenderState {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub async fn new(instance: Instance) -> RenderState {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let required_features = {
            let mut ret = Features::empty();
            for feat in DEVICE_FEATURES.iter() {
                ret |= *feat;
            }
            ret
        };

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features,
                required_limits: if cfg!(target_arch = "wasm32") {
                    wgpu::Limits::downlevel_webgl2_defaults()
                } else {
                    wgpu::Limits::default()
                },
                label: None,
                memory_hints: Default::default(),
                experimental_features: wgpu::ExperimentalFeatures::default(),
                trace: wgpu::Trace::default(),
            })
            .await
            .unwrap();

        Self {
            instance,
            adapter,
            device,
            queue,
        }
    }

    pub async fn create_surface(&self, window: Arc<Window>) -> SurfaceState {
        let size = window.inner_size();
        let surface = self
            .instance
            .create_surface(window)
            .expect("Failed to create surface!");

        let surface_caps = surface.get_capabilities(&self.adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        info!("Surface format is: '{:?}'.", surface_format);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            // disable VSync
            present_mode: wgpu::PresentMode::Immediate,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&self.device, &config);

        SurfaceState {
            surface,
            config,
            size,
        }
    }
}

impl SurfaceState {
    pub fn configure(&self, device: &wgpu::Device) {
        self.surface.configure(device, &self.config);
    }
}

fn sys_init_window(_event: On<PrimaryWindowCreatedEvent>, mut commands: Commands) {
    // 初始化 Resource
    let mut graph = ResourceGraph::new();
    swap(&mut graph, &mut RENDER_RESOURCES_TO_ADD.lock().unwrap());
    commands.queue(|world: &mut World| {
        for res in graph {
            res(world);
        }
        world.run_schedule(RenderPreparedStartup);
    });
}

fn sys_create_surface(
    mut commands: Commands,
    q_window: Query<(Entity, &WinitWindow), Without<SurfaceState>>,
    rs: Res<RenderState>,
) {
    for (id, window) in q_window {
        commands
            .entity(id)
            .insert(block_on(rs.create_surface(Arc::clone(&window.0))));
    }
}
