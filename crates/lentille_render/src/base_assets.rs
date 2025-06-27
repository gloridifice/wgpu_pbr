use crate::{
    defered_rendering::DeferredComputePipeline, material::pbr::UploadedPBRMaterial, prelude::*,
};
use bevy_ecs::prelude::*;

#[derive(Resource, Clone)]
pub struct FullScreenVertexShader {
    pub module: Arc<ShaderModule>,
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
pub struct NoFilterSampler(pub Arc<Sampler>);

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
        let main_pipeline = world.resource::<DeferredComputePipeline>();
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

impl FromWorld for NoFilterSampler {
    fn from_world(world: &mut World) -> Self {
        let rs = world.resource::<RenderState>();
        let sampler = rs
            .device
            .create_sampler(&lentille_wgpu_utils::sampler_desc_no_filter());
        Self(Arc::new(sampler))
    }
}
