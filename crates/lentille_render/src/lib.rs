use std::sync::{
    Arc, LazyLock,
    atomic::{AtomicUsize, Ordering},
};

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_log::info;
use defered_rendering::MainPipeline;
use lentille_core::window::{MainWindow, ResizeEvent};
use material::pbr::{GltfMaterial, UploadedPBRMaterial};
use pollster::block_on;
use prelude::*;
use shader_loader::ShaderLoader;
use wgpu::{
    CommandEncoder, Extent3d, Features, Instance, ShaderModule, Surface, TextureDescriptor,
    TextureDimension, TextureFormat, TextureUsages, TextureView, TextureViewDescriptor,
};

use systems::*;
use winit::dpi::PhysicalSize;

use crate::{
    asset::AssetPath, camera::CameraPlugin, light::LightPlugin, transform::TransformPlugin,
};

pub mod asset;
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
pub mod shader_loader;
pub mod shadow_mapping;
pub mod skybox;
pub mod systems;
pub mod transform;

pub static DEVICE_FEATURES: LazyLock<Arc<Vec<Features>>> =
    LazyLock::new(|| Arc::new(vec![Features::TIMESTAMP_QUERY]));

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
        app.init_resource::<RenderState>()
            .init_resource::<ShaderLoader>()
            .init_resource::<WhiteTexture>()
            .init_resource::<NormalDefaultTexture>()
            .init_resource::<dfg::DFGTexture>()
            .init_resource::<mipmap::DefaultMipmapGenShader>()
            .init_resource::<MissingTexture>()
            .init_resource::<material::buffer_material::BufferMaterialManager>()
            .init_resource::<RenderTargetSize>()
            .init_resource::<ColorRenderTarget>()
            .init_resource::<DepthRenderTarget>()
            .init_resource::<utils::cube::CubeVerticesBuffer>()
            .init_resource::<cubemap::CubemapVertexShader>()
            .init_resource::<cubemap::CubemapConvertingShader>()
            .init_resource::<cubemap::CubemapMatrixBindGroups>()
            .init_resource::<cubemap::CubemapConverterRgba16Float>()
            .init_resource::<skybox::DefaultSkybox>()
            .init_resource::<GlobalUniformBuffer>()
            // --- Render resource ---
            .init_resource::<skybox::SkyboxSHBuffer>()
            .init_resource::<shadow_mapping::ShadowMap>()
            // .insert_resource::<ShadowMapEguiTextureId>()
            .init_resource::<FullScreenVertexShader>()
            // 0. Layouts
            .init_resource::<ObjectBindGroupLayout>()
            .init_resource::<PBRMaterialBindGroupLayout>()
            // 1. Globals
            .init_resource::<shadow_mapping::ShadowMapGlobalBindGroup>()
            .init_resource::<DynamicLightBindGroup>()
            // 1.5
            .init_resource::<defered_rendering::write_g_buffer_pipeline::GBufferTexturesBindGroup>()
            .init_resource::<GlobalBindGroup>()
            // 2. Pipelines
            .init_resource::<defered_rendering::write_g_buffer_pipeline::WriteGBufferPipeline>()
            .init_resource::<skybox::SkyboxPipeline>()
            .init_resource::<MainPipeline>()
            .init_resource::<transparent::TransparentPipeline>()
            .init_resource::<shadow_mapping::ShadowMappingPipeline>()
            // --- Other resources ---
            .init_resource::<DefaultPBRMaterial>();

        app.add_systems(Last, sys_render)
            .add_systems(
                Last,
                (
                    sys_create_render_context.in_set(RenderSets::Prepare),
                    sys_present_output_view.in_set(RenderSets::Present),
                ),
            )
            .add_systems(Update, sys_refersh_global_bind_group)
            .add_systems(
                PostUpdate,
                material::pbr::sys_update_override_pbr_material_bind_group,
            )
            .add_observer(sys_on_resize);

        app.add_plugins((TransformPlugin, LightPlugin, CameraPlugin));

        app.configure_sets(
            Last,
            (
                RenderSets::Prepare,
                RenderSets::FirstDraw.run_if(resource_exists::<RenderContext>),
                RenderSets::PreDraw.run_if(resource_exists::<RenderContext>),
                RenderSets::Draw.run_if(resource_exists::<RenderContext>),
                RenderSets::PostDraw.run_if(resource_exists::<RenderContext>),
                RenderSets::LastDraw.run_if(resource_exists::<RenderContext>),
                RenderSets::Present.run_if(resource_exists::<RenderContext>),
            )
                .chain(),
        );
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderSets {
    /// Create RenderContext
    Prepare,
    // 5 Draw stages
    FirstDraw,
    PreDraw,
    Draw,
    PostDraw,
    LastDraw,
    /// Submit encoder and present output texture
    Present,
}

#[derive(Resource)]
pub struct RenderContext {
    pub encoder: CommandEncoder,
    pub output_view: TextureView,
    pub output_texture: wgpu::SurfaceTexture,
}

#[derive(Resource)]
pub struct RenderState {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
    pub size: PhysicalSize<u32>,
}

impl RenderState {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub async fn new(
        instance: &Instance,
        surface: Surface<'static>,
        width: u32,
        height: u32,
    ) -> RenderState {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
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

        let surface_caps = surface.get_capabilities(&adapter);
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
            width,
            height,
            // determine how to sync
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        Self {
            device,
            queue,
            surface,
            config,
            size: PhysicalSize::new(width, height),
        }
    }

    #[allow(unused)]
    fn get_window_extend3d(&self) -> wgpu::Extent3d {
        wgpu::Extent3d {
            width: self.config.width.max(1),
            height: self.config.height.max(1),
            depth_or_array_layers: 1,
        }
    }
}

impl FromWorld for RenderState {
    fn from_world(world: &mut World) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            #[cfg(not(target_arch = "wasm32"))]
            backends: wgpu::Backends::PRIMARY,
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::GL,
            ..Default::default()
        });

        let i_width = 1600;
        let i_height = 900;

        let window = Arc::clone(&world.resource::<MainWindow>().0);
        let _ = window.request_inner_size(PhysicalSize::new(i_width, i_height));

        let surface = instance
            .create_surface(Arc::clone(&window))
            .expect("Failed to create surface!");

        block_on(RenderState::new(&instance, surface, i_width, i_height))
    }
}

fn sys_create_render_context(world: &mut World) {
    world.resource_scope(|world: &mut World, rs: Mut<RenderState>| {
        let output = rs.surface.get_current_texture().unwrap();
        let output_view = output.texture.create_view(&Default::default());
        let encoder = rs
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Main Render Encoder"),
            });

        world.insert_resource(RenderContext {
            encoder,
            output_view,
            output_texture: output,
        });
    });
}

fn sys_present_output_view(world: &mut World) {
    if let Some(ctx) = world.remove_resource::<RenderContext>() {
        world
            .resource::<RenderState>()
            .queue
            .submit(std::iter::once(ctx.encoder.finish()));
        ctx.output_texture.present();
    }
}

fn sys_render(world: &mut World) {
    let window = Arc::clone(&world.resource::<MainWindow>().0);

    let mut ctx = world.resource_scope(|_world, render_state: Mut<RenderState>| {
        let output = render_state.surface.get_current_texture().unwrap();
        let output_view = output.texture.create_view(&Default::default());
        let encoder = render_state
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        PassRenderContext {
            encoder,
            output_view,
            output_texture: output,
        }
    });

    // PASS: Shadow Mapping -----
    world
        .run_system_cached_with(sys_render_shadow_mapping_pass, &mut ctx)
        .unwrap();
    // --------------------------

    // PASS: Main ---------------
    world
        .run_system_cached_with(sys_render_write_g_buffer_pass, &mut ctx)
        .unwrap();
    world
        .run_system_cached_with(sys_render_main_pass, &mut ctx)
        .unwrap();
    // -------------------------

    world
        .run_system_cached_with(sys_render_transparent, &mut ctx)
        .unwrap();

    // PASS: Render Egui ----------
    //TODO

    // End Draw Objects ------------
    world
        .resource::<RenderState>()
        .queue
        .submit(std::iter::once(ctx.encoder.finish()));
    ctx.output_texture.present();
}

pub static COLOR_TARGET_INDEX: LazyLock<AtomicUsize> = LazyLock::new(|| AtomicUsize::new(0));

pub fn get_color_target_index() -> usize {
    COLOR_TARGET_INDEX.load(Ordering::Relaxed)
}
pub fn get_sampleable_target_index() -> usize {
    (COLOR_TARGET_INDEX.load(Ordering::Relaxed) + 1) % 2
}
pub fn switch_ping_pong() {
    COLOR_TARGET_INDEX.store(
        (COLOR_TARGET_INDEX.load(Ordering::Relaxed) + 1) % 2,
        Ordering::Relaxed,
    );
}

#[derive(Resource)]
pub struct ColorRenderTarget {
    pub ping_pong: Vec<Option<UploadedImageWithSampler>>,
}

#[derive(Resource)]
pub struct DepthRenderTarget(pub Option<UploadedImageWithSampler>);

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct RTId(uuid::Uuid);

#[derive(Resource, Clone)]
pub struct RenderTargetSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Resource, Clone)]
pub struct FullScreenVertexShader {
    module: Arc<ShaderModule>,
}

impl Default for RenderTargetSize {
    fn default() -> Self {
        Self {
            width: 512,
            height: 512,
        }
    }
}

impl From<&RenderTargetSize> for Extent3d {
    fn from(value: &RenderTargetSize) -> Self {
        Self {
            width: value.width,
            height: value.height,
            depth_or_array_layers: 1,
        }
    }
}

/// 包含两个部分，target 是用于被写入的。
/// sampleable 是用于作为读取的可被采样的。
pub struct PingPongImages<'a> {
    pub target: Option<&'a UploadedImageWithSampler>,
    #[allow(unused)]
    pub sampleable: Option<&'a UploadedImageWithSampler>,
}

impl ColorRenderTarget {
    pub fn new(
        width: u32,
        height: u32,
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
    ) -> Self {
        let a = create_color_render_target_image(width, height, device, config);
        let b = create_color_render_target_image(width, height, device, config);
        Self {
            ping_pong: vec![Some(a), Some(b)],
        }
    }

    pub fn get_size(&self) -> Option<Extent3d> {
        self.get_target().map(|it| it.size)
    }

    pub fn update_images(
        &mut self,
        width: u32,
        height: u32,
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
    ) {
        let a = create_color_render_target_image(width, height, device, config);
        let b = create_color_render_target_image(width, height, device, config);
        self.ping_pong[0] = Some(a);
        self.ping_pong[1] = Some(b);
    }

    /// 更新序号，然后获取当前的 Target 和采样贴图。
    pub fn switch_and_get_images(&mut self) -> PingPongImages {
        switch_ping_pong();

        PingPongImages {
            target: self
                .ping_pong
                .get(get_color_target_index())
                .and_then(|it| it.as_ref()),
            sampleable: self
                .ping_pong
                .get(get_sampleable_target_index())
                .and_then(|it| it.as_ref()),
        }
    }

    pub fn get_target(&self) -> Option<&UploadedImageWithSampler> {
        self.ping_pong
            .get(get_color_target_index())
            .and_then(|it| it.as_ref())
    }

    #[allow(unused)]
    pub fn get_sampleable(&self) -> Option<&UploadedImageWithSampler> {
        self.ping_pong
            .get(get_sampleable_target_index())
            .and_then(|it| it.as_ref())
    }
}

pub fn create_color_render_target_image(
    width: u32,
    height: u32,
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
) -> UploadedImageWithSampler {
    let size = Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let desc = TextureDescriptor {
        label: Some("Render Target"),
        size,
        format: config.format,
        usage: config.usage
            | TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_SRC
            | TextureUsages::COPY_DST,
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        view_formats: &[],
    };
    let texture = device.create_texture(&desc);
    let view = texture.create_view(&TextureViewDescriptor::default());

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        // 4.
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        compare: None, // 5.
        lod_min_clamp: 0.0,
        lod_max_clamp: 100.0,
        ..Default::default()
    });

    UploadedImageWithSampler {
        size,
        texture,
        view,
        sampler,
    }
}

pub fn create_depth_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    compare: Option<wgpu::CompareFunction>,
) -> UploadedImageWithSampler {
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
    let sampler = device.create_sampler(&{
        let mut desc = lentille_wgpu_utils::sampler_desc_no_filter();
        desc.compare = compare;
        desc
    });

    UploadedImageWithSampler {
        size,
        texture,
        view,
        sampler,
    }
}

impl FromWorld for FullScreenVertexShader {
    fn from_world(world: &mut World) -> Self {
        let source = world
            .resource_mut::<ShaderLoader>()
            .load_source(AssetPath::new_shader_wgsl("utils/fullscreen_vertex"))
            .unwrap();
        let shader = world.resource::<RenderState>().device.create_shader_module(
            wgpu::ShaderModuleDescriptor {
                label: Some("Fullscreen Vertex Shader"),
                source,
            },
        );
        Self {
            module: Arc::new(shader),
        }
    }
}

impl FromWorld for ColorRenderTarget {
    fn from_world(world: &mut World) -> Self {
        let render_state = world.resource::<RenderState>();
        let size = world.resource::<RenderTargetSize>();

        Self::new(
            size.width,
            size.height,
            &render_state.device,
            &render_state.config,
        )
    }
}

impl FromWorld for DepthRenderTarget {
    fn from_world(world: &mut World) -> Self {
        let render_state = world.resource::<RenderState>();
        let size = world.resource::<RenderTargetSize>();

        let target = create_depth_texture(&render_state.device, size.width, size.height, None);

        Self(Some(target))
    }
}

#[derive(Component, Clone)]
pub struct MainPassObject;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlphaMode {
    Opaque,
    Blend,
}

#[derive(Resource, Clone)]
pub struct WhiteTexture(pub Arc<UploadedImageWithSampler>);

#[derive(Resource, Clone)]
pub struct NormalDefaultTexture(pub Arc<UploadedImageWithSampler>);

#[derive(Resource, Clone)]
pub struct MissingTexture(pub Arc<UploadedImageWithSampler>);

#[derive(Resource, Clone)]
pub struct DefaultPBRMaterial(pub Arc<UploadedPBRMaterial>);

impl FromWorld for WhiteTexture {
    fn from_world(world: &mut World) -> Self {
        let rs = world.resource::<RenderState>();
        Self(Arc::new(
            UploadedImageWithSampler::load_from_path(
                AssetPath::Assets("textures/white.png".to_string()),
                &rs.device,
                &rs.queue,
                TextureFormat::Rgba8UnormSrgb,
            )
            .unwrap(),
        ))
    }
}

impl FromWorld for NormalDefaultTexture {
    fn from_world(world: &mut World) -> Self {
        let rs = world.resource::<RenderState>();
        Self(Arc::new(
            UploadedImageWithSampler::load_from_path(
                AssetPath::Assets("textures/normal_default.png".to_string()),
                &rs.device,
                &rs.queue,
                TextureFormat::Rgba8UnormSrgb,
            )
            .unwrap(),
        ))
    }
}

impl FromWorld for MissingTexture {
    fn from_world(world: &mut World) -> Self {
        let rs = world.resource::<RenderState>();
        Self(Arc::new(
            UploadedImageWithSampler::load_from_path(
                AssetPath::Assets("textures/missing.png".to_string()),
                &rs.device,
                &rs.queue,
                TextureFormat::Rgba8UnormSrgb,
            )
            .unwrap(),
        ))
    }
}

impl FromWorld for DefaultPBRMaterial {
    fn from_world(world: &mut World) -> Self {
        let missing_tex = &world.resource::<MissingTexture>().0;
        let white_tex = &world.resource::<WhiteTexture>().0;
        let normal_default_tex = &world.resource::<NormalDefaultTexture>().0;
        let device = &world.resource::<RenderState>().device;
        let main_pipeline = world.resource::<MainPipeline>();
        let layout = world.resource::<PBRMaterialBindGroupLayout>();

        let mat = UploadedPBRMaterial::from_gltf(
            device,
            layout,
            white_tex,
            normal_default_tex,
            Arc::clone(&main_pipeline.pipeline),
            &GltfMaterial {
                base_color_texture: Some(Arc::clone(missing_tex)),
                ..Default::default()
            },
        );
        Self(Arc::new(mat))
    }
}

fn sys_on_resize(event: Trigger<ResizeEvent>, mut rs: ResMut<RenderState>) {
    let new_size = event.physical_size;
    if new_size.width > 0 && new_size.height > 0 {
        rs.size = new_size;
        rs.config.width = new_size.width;
        rs.config.height = new_size.height;
        rs.surface.configure(&rs.device, &rs.config);
    }
}
