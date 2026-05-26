use crate::{
    deferred_rendering::DeferredComputePipeline, material::pbr::UploadedPBRMaterial, prelude::*,
};
use bevy_app::Plugin;
use bevy_ecs::prelude::*;

pub(super) struct BaseAssetsPlugin;

impl Plugin for BaseAssetsPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_render_resource::<WhiteTexture>()
            .init_render_resource::<NormalDefaultTexture>()
            .init_render_resource::<DFGTexture>()
            .init_render_resource::<super::mipmap::DefaultMipmapGenShader>() //TODO move
            .init_render_resource::<MissingTexture>()
            .init_render_resource::<FullScreenVertexShader>()
            .init_render_resource::<NoFilterClampSampler>()
            .init_render_resource_with_config::<DefaultPBRMaterial>([
                after::<MissingTexture>(),
                after::<WhiteTexture>(),
                after::<NormalDefaultTexture>(),
                after::<DeferredComputePipeline>(),
                after::<PBRMaterialBindGroupLayout>(),
            ]);
    }
}

#[derive(Resource, Clone)]
pub struct FullScreenVertexShader {
    pub module: Arc<ShaderModule>,
}

#[derive(Resource)]
pub struct DFGTexture {
    pub texture: Arc<UploadedImageWithSampler>,
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

#[derive(Resource, Clone)]
pub struct WhiteTexture(pub Arc<UploadedImageWithSampler>);

#[derive(Resource, Clone)]
pub struct NormalDefaultTexture(pub Arc<UploadedImageWithSampler>);

#[derive(Resource, Clone)]
pub struct MissingTexture(pub Arc<UploadedImageWithSampler>);

#[derive(Resource, Clone)]
pub struct DefaultPBRMaterial(pub Arc<UploadedPBRMaterial>);

#[derive(Resource, Clone)]
pub struct NoFilterClampSampler(pub Arc<Sampler>);

#[derive(Resource, Clone)]
pub struct LinearFilterClampSampler(pub Arc<Sampler>);

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
        let compute_pipeline = world.resource::<DeferredComputePipeline>();
        let layout = world.resource::<PBRMaterialBindGroupLayout>();

        let mat = UploadedPBRMaterial::from_gltf(
            device,
            layout,
            white_tex,
            normal_default_tex,
            Arc::clone(&compute_pipeline.pipeline),
            &GltfMaterial {
                base_color_texture: Some(Arc::clone(missing_tex)),
                ..Default::default()
            },
        );
        Self(Arc::new(mat))
    }
}

impl FromWorld for NoFilterClampSampler {
    fn from_world(world: &mut World) -> Self {
        let rs = world.resource::<RenderState>();
        let sampler = rs
            .device
            .create_sampler(&lentille_wgpu_utils::sampler_desc_no_filter());
        Self(Arc::new(sampler))
    }
}

impl FromWorld for LinearFilterClampSampler {
    fn from_world(world: &mut World) -> Self {
        let rs = world.resource::<RenderState>();
        let sampler = rs.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            compare: None,
            lod_min_clamp: 0.0,
            lod_max_clamp: 100.0,
            ..Default::default()
        });
        Self(Arc::new(sampler))
    }
}

impl FromWorld for DFGTexture {
    fn from_world(world: &mut World) -> Self {
        let rs = world.resource::<RenderState>();
        let texture = Arc::new(
            UploadedImageWithSampler::load_from_path(
                crate::asset::AssetPath::Assets("textures/ibl_brdf_lut.png".to_string()),
                &rs.device,
                &rs.queue,
                wgpu::TextureFormat::Rgba8Unorm,
            )
            .unwrap(),
        );
        Self { texture }
    }
}
