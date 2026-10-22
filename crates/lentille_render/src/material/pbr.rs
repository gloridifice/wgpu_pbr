use super::UploadedMaterial;
use crate::{
    asset::{AssetRef, TextureAsset},
    bindings::material_binding::PbrMaterialBindGroupBuilder,
    prelude::*,
};
use bevy_ecs::prelude::*;
use bevy_reflect::Reflect;
use lentille_wgpu_utils::typed_sampler::FilteringSampler;
use std::sync::Arc;

type Tex2D = UploadedImage<Dim2D, SampleFloatFilterable>;

#[derive(Clone, Debug, Default, PartialEq, Reflect)]
pub enum TextureOverride {
    #[default]
    Inherit,
    Set(AssetRef<TextureAsset>),
    Clear,
}

#[derive(Component, Clone, Debug, Default, PartialEq, Reflect)]
#[reflect(Component)]
pub struct PbrMaterial {
    pub base_color_texture: TextureOverride,
    pub normal_texture: TextureOverride,
    pub metallic_roughness_ao_texture: TextureOverride,
    pub emission_texture: TextureOverride,
    pub color: Option<Color>,
    pub roughness: Option<f32>,
    pub metallic: Option<f32>,
    pub reflectance: Option<f32>,
    pub alpha_mode: Option<AlphaMode>,
}

impl PbrMaterial {
    pub fn merged(&self, base: &Self) -> Self {
        fn texture(value: &TextureOverride, base: &TextureOverride) -> TextureOverride {
            match value {
                TextureOverride::Inherit => base.clone(),
                _ => value.clone(),
            }
        }
        Self {
            base_color_texture: texture(&self.base_color_texture, &base.base_color_texture),
            normal_texture: texture(&self.normal_texture, &base.normal_texture),
            metallic_roughness_ao_texture: texture(
                &self.metallic_roughness_ao_texture,
                &base.metallic_roughness_ao_texture,
            ),
            emission_texture: texture(&self.emission_texture, &base.emission_texture),
            color: self.color.or(base.color),
            roughness: self.roughness.or(base.roughness),
            metallic: self.metallic.or(base.metallic),
            reflectance: self.reflectance.or(base.reflectance),
            alpha_mode: self.alpha_mode.or(base.alpha_mode),
        }
    }
    pub fn alpha_mode_or_default(&self) -> AlphaMode {
        self.alpha_mode.unwrap_or(AlphaMode::Opaque)
    }
}

#[derive(Default)]
pub struct ResolvedMaterial {
    pub description: PbrMaterial,
    pub textures: [Option<Arc<Tex2D>>; 4],
}

pub struct UploadedPbrMaterial {
    pub bind_group: Arc<BindGroup>,
    pub pipeline: Arc<RenderPipeline>,
    pub alpha_mode: AlphaMode,
}
impl UploadedPbrMaterial {
    pub fn new(
        device: &wgpu::Device,
        layout: &PbrMaterialBindGroupLayout,
        white: &Tex2D,
        normal: &Tex2D,
        sampler: &FilteringSampler,
        pipeline: Arc<RenderPipeline>,
        material: &ResolvedMaterial,
    ) -> Self {
        let [base, normal_tex, metallic, emission] = &material.textures;
        let raw = PbrMaterialUniform::from(&material.description);
        let buffer = TypedBuffer::new_init(
            device,
            Some("PBR"),
            raw,
            BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        );
        let bind_group = Arc::new(
            PbrMaterialBindGroupBuilder {
                pbr_uniform: &buffer,
                sampler,
                base_color_texture: &base.as_deref().unwrap_or(white).view,
                normal_texture: &normal_tex.as_deref().unwrap_or(normal).view,
                metallic_roughness_ao_texture: &metallic.as_deref().unwrap_or(white).view,
                emission_texture: &emission.as_deref().unwrap_or(white).view,
            }
            .build(device, layout),
        );
        Self {
            bind_group,
            pipeline,
            alpha_mode: material.description.alpha_mode_or_default(),
        }
    }
}
impl UploadedMaterial for UploadedPbrMaterial {
    fn get_bind_group(&self) -> &BindGroup {
        &self.bind_group
    }
    fn get_render_pipeline(&self) -> &RenderPipeline {
        &self.pipeline
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PbrMaterialUniform {
    pub color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub reflectance: f32,
    pub padding: f32,
}
impl_pod_zeroable!(PbrMaterialUniform);
impl From<&PbrMaterial> for PbrMaterialUniform {
    fn from(value: &PbrMaterial) -> Self {
        Self {
            color: value.color.unwrap_or(Color::WHITE).into_array(),
            metallic: value.metallic.unwrap_or(1.0),
            roughness: value.roughness.unwrap_or(1.0),
            reflectance: value.reflectance.unwrap_or(0.0),
            padding: 0.0,
        }
    }
}
