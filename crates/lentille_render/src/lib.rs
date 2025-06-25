use std::{
    mem::swap,
    sync::{Arc, LazyLock},
};

use bevy_app::prelude::*;
use bevy_ecs::{prelude::*, schedule::ScheduleLabel};
use bevy_log::info;
use defered_rendering::DeferredComputePipeline;
use lentille_core::window::{MainWindowCreatedEvent, ResizeEvent, WinitWindow};
use pollster::block_on;
use prelude::*;
use shader_loader::ShaderLoader;
use wgpu::{Features, Instance};

use systems::*;
use winit::{dpi::PhysicalSize, window::Window};

use crate::{
    app_ext::AppExt,
    bindings::BindingsPlugin,
    camera::CameraPlugin,
    cubemap::CubemapPlugin,
    defered_rendering::DeferredRenderingPlugin,
    light::LightPlugin,
    resource::{RENDER_RESOURCES_TO_ADD, ResourceGraph},
    shadow_mapping::ShadowMappingPlugin,
    transform::TransformPlugin,
    transparent::TransparentPlugin,
};

pub mod asset;
pub mod base_assets;
pub mod bindings;
pub mod camera;
pub mod cubemap;
pub mod defered_rendering;
pub mod dfg;
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

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_plugins((
            TransformPlugin,
            LightPlugin,
            CameraPlugin,
            CubemapPlugin,
            BindingsPlugin,
            ShadowMappingPlugin,
            TransparentPlugin,
            DeferredRenderingPlugin,
        ));
        app.init_resource::<ShaderLoader>();

        // Configure RenderSets
        app.configure_sets(
            Last,
            (
                FrameSets::Prepare,
                FrameSets::PreDraw,
                // Opaque
                FrameSets::BeforeDrawOpaque,
                FrameSets::DrawOpaque,
                FrameSets::AfterDrawOpaque,
                // Transparent
                FrameSets::BeforeDrawTransparent,
                FrameSets::DrawTransparent,
                FrameSets::AfterDrawTransparent,
                // Last and present
                FrameSets::LastDraw,
                FrameSets::Present,
                FrameSets::Cleanup,
            )
                .chain()
                .run_if(resource_exists::<RenderState>),
        );

        // Add basics render systems
        app.add_systems(
            Last,
            (sys_cleanup_frame_context.in_set(FrameSets::Cleanup),),
        )
        // 初始化 RenderState 和初始化资源
        .add_observer(sys_init_window);

        // Add frame render systems
        app.add_observer(sys_on_resize)
            // 一般系统
            .add_systems(Update, sys_refersh_global_bind_group)
            .add_systems(
                PostUpdate,
                material::pbr::sys_update_override_pbr_material_bind_group,
            )
            .add_systems(
                Last,
                (
                    sys_render_shadow_mapping_pass.in_set(FrameSets::PreDraw),
                    (sys_render_write_g_buffer_pass, sys_render_main_pass)
                        .chain()
                        .in_set(FrameSets::DrawOpaque),
                    sys_render_transparent.in_set(FrameSets::DrawTransparent),
                ),
            );
        app.init_render_resource::<WhiteTexture>()
            .init_render_resource::<NormalDefaultTexture>()
            .init_render_resource::<dfg::DFGTexture>()
            .init_render_resource::<mipmap::DefaultMipmapGenShader>()
            .init_render_resource::<MissingTexture>()
            .init_render_resource::<FullScreenVertexShader>()
            .init_render_resource_with_config::<DefaultPBRMaterial>([
                after::<MissingTexture>(),
                after::<WhiteTexture>(),
                after::<NormalDefaultTexture>(),
                after::<DeferredComputePipeline>(),
                after::<PBRMaterialBindGroupLayout>(),
            ]);
    }
}

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
    // 5 Draw stages
    PreDraw,

    // Opaque
    BeforeDrawOpaque,
    DrawOpaque,
    AfterDrawOpaque,

    // Transparent
    BeforeDrawTransparent,
    DrawTransparent,
    AfterDrawTransparent,

    // Post-processing
    DrawPostProcessing,

    LastDraw,
    /// Submit encoder and present output texture
    Present,
    Cleanup,
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
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            #[cfg(not(target_arch = "wasm32"))]
            backends: wgpu::Backends::PRIMARY,
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::GL,
            ..Default::default()
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
            .request_device(
                &wgpu::DeviceDescriptor {
                    required_features,
                    required_limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                    label: None,
                    memory_hints: Default::default(),
                },
                None,
            )
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
        let surface = self
            .instance
            .create_surface(window)
            .expect("Failed to create surface!");
        let size = window.inner_size();

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
            // determine how to sync
            present_mode: surface_caps.present_modes[0],
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

fn sys_init_window(event: Trigger<MainWindowCreatedEvent>, world: &mut World) {
    // 初始化 Resource
    let mut graph = ResourceGraph::new();
    swap(&mut graph, &mut RENDER_RESOURCES_TO_ADD.lock().unwrap());
    for res in graph {
        res(world);
    }

    world.run_schedule(RenderPreparedStartup);
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

fn sys_on_resize(
    event: Trigger<ResizeEvent>,
    mut rs: ResMut<RenderState>,
    q_window: Query<&WinitWindow>,
) {
    // TODO 将 SurfaceState 移动到 Window Entity 中
    if let Some(window) = q_window.iter().find(|it| it.0.id() == event.window_id) {
        let new_size = event.physical_size;
        if new_size.width > 0 && new_size.height > 0 {
            rs.size = new_size;
            rs.config.width = new_size.width;
            rs.config.height = new_size.height;
            rs.surface.configure(&rs.device, &rs.config);
        }
    }
}

fn sys_cleanup_frame_context(world: &mut World) {
    world.remove_resource::<FrameRenderContext>();
}
