use std::sync::Arc;

use crate::{
    bindings::material_binding::PbrMaterialBindGroupBuilder,
    deferred_rendering::DeferredComputePipeline, prelude::*,
};
use bevy_ecs::prelude::*;
use lentille_wgpu_utils::typed_sampler::FilteringSampler;

use super::UploadedMaterial;

type Tex2D = UploadedImage<Dim2D, SampleFloatFilterable>;

#[derive(Component, Clone, Default)]
#[require(PBRMaterialOverride)]
pub struct PbrMaterial {
    pub base_color_texture: Option<Arc<Tex2D>>,
    pub normal_texture: Option<Arc<Tex2D>>,
    pub metallic_roughness_ao_texture: Option<Arc<Tex2D>>,
    pub emission_texture: Option<Arc<Tex2D>>,
    pub color: Option<Color>,
    pub roughness: Option<f32>,
    pub metallic: Option<f32>,
    pub reflectance: Option<f32>,
    pub alpha_mode: Option<AlphaMode>,
}

impl PbrMaterial {
    fn color_or_default(&self) -> Color {
        self.color.unwrap_or(Color::WHITE)
    }

    fn roughness_or_default(&self) -> f32 {
        self.roughness.unwrap_or(1.0)
    }

    fn metallic_or_default(&self) -> f32 {
        self.metallic.unwrap_or(1.0)
    }

    fn reflectance_or_default(&self) -> f32 {
        self.reflectance.unwrap_or(0.0)
    }

    pub fn alpha_mode_or_default(&self) -> AlphaMode {
        self.alpha_mode.unwrap_or(AlphaMode::Opaque)
    }
}

pub struct UploadedPbrMaterial {
    pub bind_group: Arc<BindGroup>,
    pub pipeline: Arc<RenderPipeline>,
}

impl UploadedPbrMaterial {
    pub fn new(
        device: &wgpu::Device,
        layout: &PbrMaterialBindGroupLayout,
        white_texture: &Tex2D,
        fallback_normal_texture: &Tex2D,
        sampler: &FilteringSampler,
        main_pipeline: Arc<RenderPipeline>,
        material: &PbrMaterial,
    ) -> Self {
        let base_color = material
            .base_color_texture
            .as_ref()
            .map(|it| it.as_ref())
            .unwrap_or(white_texture);
        let metallic_roughness_ao_texture = material
            .metallic_roughness_ao_texture
            .as_ref()
            .map(|it| it.as_ref())
            .unwrap_or(white_texture);
        let emission_texture = material
            .emission_texture
            .as_ref()
            .map(|it| it.as_ref())
            .unwrap_or(white_texture);
        let normal = material
            .normal_texture
            .as_ref()
            .map(|it| it.as_ref())
            .unwrap_or(fallback_normal_texture);

        let raw = PbrMaterialUniform::from(material);

        let buffer = TypedBuffer::new_init(
            device,
            Some("PBR"),
            raw,
            BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        );

        let bind_group = Arc::new(
            PbrMaterialBindGroupBuilder {
                pbr_uniform: &buffer,
                sampler: &sampler,
                base_color_texture: &base_color.view,
                normal_texture: &normal.view,
                metallic_roughness_ao_texture: &metallic_roughness_ao_texture.view,
                emission_texture: &emission_texture.view,
            }
            .build(&device, &layout),
        );

        Self {
            bind_group,
            pipeline: main_pipeline,
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

#[derive(Component, Clone, Default)]
pub struct PBRMaterialOverride {
    pub material: PbrMaterial,
    pub uploaded_material: Option<Arc<UploadedPbrMaterial>>,
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
            metallic: value.metallic_or_default(),
            roughness: value.roughness_or_default(),
            reflectance: value.reflectance_or_default(),
            color: value.color_or_default().into_array(),
            padding: 0.0,
        }
    }
}

pub fn sys_update_override_pbr_material_bind_group(
    rs: Res<RenderState>,
    main_pipeline: Res<DeferredComputePipeline>,
    white: Res<WhiteTexture>,
    normal_default: Res<NormalDefaultTexture>,
    layout: Res<PbrMaterialBindGroupLayout>,
    material_sampler: Res<DefaultMaterialSampler>,
    mut pbr_mats: Query<
        (&MeshRenderer, &PbrMaterial, &mut PBRMaterialOverride),
        Changed<PbrMaterial>,
    >,
) {
    for (mesh, pbr_mat, mut override_pbr_mat) in pbr_mats.iter_mut() {
        let raw_mat = mesh
            .mesh
            .as_ref()
            .and_then(|it| {
                it.primitives
                    .first()
                    .as_ref()
                    .map(|primitive| primitive.material.as_ref())
            })
            .flatten();

        let mut mat = raw_mat
            .as_ref()
            .map(|it| it.as_ref().clone())
            .unwrap_or_default();

        if let Some(base_color_texture) = pbr_mat.base_color_texture.clone() {
            mat.base_color_texture = Some(base_color_texture);
        }
        if let Some(normal_texture) = pbr_mat.normal_texture.clone() {
            mat.normal_texture = Some(normal_texture);
        }
        if let Some(metallic_roughness_ao_texture) = pbr_mat.metallic_roughness_ao_texture.clone() {
            mat.metallic_roughness_ao_texture = Some(metallic_roughness_ao_texture);
        }
        if let Some(emission_texture) = pbr_mat.emission_texture.clone() {
            mat.emission_texture = Some(emission_texture);
        }
        if let Some(color) = pbr_mat.color {
            mat.color = Some(color);
        }
        if let Some(roughness) = pbr_mat.roughness {
            mat.roughness = Some(roughness);
        }
        if let Some(metallic) = pbr_mat.metallic {
            mat.metallic = Some(metallic);
        }
        if let Some(reflectance) = pbr_mat.reflectance {
            mat.reflectance = Some(reflectance);
        }
        if let Some(alpha_mode) = pbr_mat.alpha_mode {
            mat.alpha_mode = Some(alpha_mode);
        }

        override_pbr_mat.material = pbr_mat.clone();
        override_pbr_mat.uploaded_material = Some(Arc::new(UploadedPbrMaterial::new(
            &rs.device,
            &layout,
            &white.0,
            &normal_default.0,
            &material_sampler.0,
            Arc::clone(&main_pipeline.pipeline),
            &mat,
        )))
    }
}
