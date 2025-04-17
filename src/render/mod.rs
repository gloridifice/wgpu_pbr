use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, LazyLock,
};

use bevy_ecs::{
    component::Component,
    system::Resource,
    world::{FromWorld, World},
};
use defered_rendering::MainPipeline;
use material::{
    pbr::{GltfMaterial, PBRMaterialBindGroupLayout, UploadedPBRMaterial},
    UploadedMaterial,
};
use shader_loader::ShaderLoader;
use wgpu::{
    BindGroup, BindGroupLayout, Buffer, BufferDescriptor, BufferUsages, Extent3d, RenderPass,
    Sampler, ShaderModule, ShaderStages, Texture, TextureDescriptor, TextureDimension,
    TextureUsages, TextureView, TextureViewDescriptor,
};

use crate::{
    asset::{load::Loadable, AssetPath},
    bg_descriptor, bg_layout_descriptor,
    macro_utils::BGLEntry,
    wgpu_init, RenderState,
};

pub mod camera;
pub mod cubemap;
pub mod defered_rendering;
pub mod dfg;
pub mod gizmos;
pub mod light;
pub mod material;
pub mod mesh;
pub mod mipmap;
pub mod post_processing;
pub mod prelude;
pub mod shader_loader;
pub mod shadow_mapping;
pub mod skybox;
pub mod systems;
pub mod transform;

/// 想要一个物体以 Transparent 的管线渲染，需要至少有以下 Component:
/// - `TransparentPassObject`
/// - `WorldTransform`
/// - `MeshRenderer`
///
/// 同时物体不能持有 `MainPassObject`
pub mod transparent;
pub mod utils;

pub static COLOR_TARGET_INDEX: LazyLock<AtomicUsize> = LazyLock::new(|| AtomicUsize::new(0));

pub fn get_color_target_index() -> usize {
    COLOR_TARGET_INDEX.load(Ordering::Relaxed)
}
pub fn get_sampleable_target_index() -> usize {
    (COLOR_TARGET_INDEX.load(Ordering::Relaxed) + 1) % 2
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
        COLOR_TARGET_INDEX.store(
            (COLOR_TARGET_INDEX.load(Ordering::Relaxed) + 1) % 2,
            Ordering::Relaxed,
        );
        PingPongImages {
            target: self
                .ping_pong
                .get(get_color_target_index())
                .map(|it| it.as_ref())
                .flatten(),
            sampleable: self
                .ping_pong
                .get(get_sampleable_target_index())
                .map(|it| it.as_ref())
                .flatten(),
        }
    }

    pub fn get_target(&self) -> Option<&UploadedImageWithSampler> {
        self.ping_pong
            .get(get_color_target_index())
            .map(|it| it.as_ref())
            .flatten()
    }

    #[allow(unused)]
    pub fn get_sampleable(&self) -> Option<&UploadedImageWithSampler> {
        self.ping_pong
            .get(get_sampleable_target_index())
            .map(|it| it.as_ref())
            .flatten()
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
        let mut desc = wgpu_init::sampler_desc_no_filter();
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
    Mask,
    Blend,
}

pub struct UploadedImageWithSampler {
    #[allow(unused)]
    pub size: wgpu::Extent3d,
    #[allow(unused)]
    pub texture: Texture,
    pub view: TextureView,
    pub sampler: Sampler,
}

pub struct UploadedImage {
    #[allow(unused)]
    pub texture: Texture,
    pub view: TextureView,
}

impl UploadedImageWithSampler {
    pub fn image_data_layout(
        width: u32,
        heigh: u32,
        pixel_size: u32,
        offset: u64,
    ) -> wgpu::TexelCopyBufferLayout {
        wgpu::TexelCopyBufferLayout {
            offset,
            bytes_per_row: Some(pixel_size * width),
            rows_per_image: Some(heigh),
        }
    }

    pub fn default_sampler_desc() -> wgpu::SamplerDescriptor<'static> {
        wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        }
    }

    pub fn from_glb_data(
        data: &gltf::image::Data,
        #[allow(unused)] gltf_sampler: &gltf::texture::Sampler,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Self {
        let size = wgpu::Extent3d {
            width: data.width,
            height: data.height,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let pixels = match data.format {
            gltf::image::Format::R8G8B8 => {
                let new_len = data.pixels.len() / 3 * 4;
                let mut ret = vec![0u8; new_len];
                for i in 0..new_len {
                    let divide = i / 4;
                    let modulo = i % 4;
                    ret[i] = if modulo != 3 {
                        *data.pixels.get(divide * 3 + modulo).unwrap()
                    } else {
                        0u8
                    };
                }
                ret
            }
            _ => data.pixels.clone(),
        };

        queue.write_texture(
            wgpu::TexelCopyTextureInfoBase {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            UploadedImageWithSampler::image_data_layout(data.width, data.height, 4, 0),
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // todo
        let sampler = device.create_sampler(&UploadedImageWithSampler::default_sampler_desc());

        Self {
            size,
            texture,
            view,
            sampler,
        }
    }
}

#[derive(Resource, Clone)]
pub struct ObjectBindGroupLayout(Arc<BindGroupLayout>);

impl From<gltf::material::AlphaMode> for AlphaMode {
    fn from(value: gltf::material::AlphaMode) -> Self {
        match value {
            gltf::material::AlphaMode::Opaque => AlphaMode::Opaque,
            gltf::material::AlphaMode::Mask => AlphaMode::Mask,
            gltf::material::AlphaMode::Blend => AlphaMode::Blend,
        }
    }
}

impl FromWorld for ObjectBindGroupLayout {
    fn from_world(world: &mut World) -> Self {
        let rs = world.resource::<RenderState>();
        let device = &rs.device;
        let object_bind_group_layout =
            Arc::new(device.create_bind_group_layout(&bg_layout_descriptor!(
                ["Object Bind Group Layout"]
                0: ShaderStages::VERTEX => BGLEntry::UniformBuffer(); // Transform
            )));
        Self(object_bind_group_layout)
    }
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
        Self(Arc::new(
            UploadedImageWithSampler::load(
                AssetPath::Assets("textures/white.png".to_string()),
                world,
            )
            .unwrap(),
        ))
    }
}

impl FromWorld for NormalDefaultTexture {
    fn from_world(world: &mut World) -> Self {
        Self(Arc::new(
            UploadedImageWithSampler::load(
                AssetPath::Assets("textures/normal_default.png".to_string()),
                world,
            )
            .unwrap(),
        ))
    }
}

impl FromWorld for MissingTexture {
    fn from_world(world: &mut World) -> Self {
        Self(Arc::new(
            UploadedImageWithSampler::load(
                AssetPath::Assets("textures/missing.png".to_string()),
                world,
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
