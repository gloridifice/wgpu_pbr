use std::sync::Arc;

use crate::{
    bg_descriptor, impl_pod_zeroable,
    render::{
        bindings::material_binding::PBRMaterialBindGroupLayout, defered_rendering::MainPipeline,
        prelude::*, AlphaMode, NormalDefaultTexture, WhiteTexture,
    },
};
use bevy_ecs::prelude::*;
use wgpu::{util::DeviceExt, BindingResource, BufferUsages};

use super::UploadedMaterial;

#[derive(Clone)]
pub struct GltfMaterial {
    pub base_color_texture: Option<Arc<UploadedImageWithSampler>>,
    pub normal_texture: Option<Arc<UploadedImageWithSampler>>,
    pub color: [f32; 4],
    pub roughness: f32,
    pub metallic: f32,
    pub reflectance: f32,
    pub alpha_mode: AlphaMode,
}

impl Default for GltfMaterial {
    fn default() -> Self {
        Self {
            base_color_texture: None,
            normal_texture: None,
            color: [1.0; 4],
            roughness: 0.5,
            metallic: 0.0,
            reflectance: 0.0,
            alpha_mode: AlphaMode::Opaque,
        }
    }
}

pub struct UploadedPBRMaterial {
    pub bind_group: Arc<BindGroup>,
    pub pipeline: Arc<RenderPipeline>,
}

impl UploadedPBRMaterial {
    pub fn from_gltf(
        device: &wgpu::Device,
        layout: &PBRMaterialBindGroupLayout,
        white_texture: &UploadedImageWithSampler,
        normal_texture: &UploadedImageWithSampler,
        main_pipeline: Arc<RenderPipeline>,
        gltf_material: &GltfMaterial,
    ) -> Self {
        let base_color = gltf_material
            .base_color_texture
            .as_ref()
            .map(|it| it.as_ref())
            .unwrap_or(white_texture);
        let normal = gltf_material
            .normal_texture
            .as_ref()
            .map(|it| it.as_ref())
            .unwrap_or(normal_texture);
        let material_bind_group_layout = &layout.0;

        let raw = RawPBRMaterial::from(gltf_material);

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("PBR"),
            contents: bytemuck::cast_slice(&[raw]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let bind_group = Arc::new(device.create_bind_group(&bg_descriptor!(
            ["PBR Material Bind Group"] [material_bind_group_layout]
            0: buffer.as_entire_binding();
            1: BindingResource::TextureView(&base_color.view);
            2: BindingResource::Sampler(&base_color.sampler);
            3: BindingResource::TextureView(&normal.view);
            4: BindingResource::Sampler(&normal.sampler);
        )));

        Self {
            bind_group,
            pipeline: main_pipeline,
        }
    }
}

impl UploadedMaterial for UploadedPBRMaterial {
    fn get_bind_group(&self) -> &BindGroup {
        &self.bind_group
    }
    fn get_render_pipeline(&self) -> &RenderPipeline {
        &self.pipeline
    }
}

#[derive(Component, Clone, Default)]
pub struct PBRMaterialOverride {
    pub material: PBRMaterial,
    pub uploaded_material: Option<Arc<UploadedPBRMaterial>>,
}

#[derive(Component, Clone, Default)]
#[require(PBRMaterialOverride)]
pub struct PBRMaterial {
    pub base_color_texture: Option<Arc<UploadedImageWithSampler>>,
    pub normal_texture: Option<Arc<UploadedImageWithSampler>>,
    pub color: Option<Vec4>,
    pub roughness: Option<f32>,
    pub metallic: Option<f32>,
    pub reflectance: Option<f32>,
    pub alpha_mode: Option<AlphaMode>,
}

#[derive(Clone, Copy, Debug)]
#[allow(unused)]
pub struct RawPBRMaterial {
    pub color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub reflectance: f32,
    pub padding: f32,
}
impl_pod_zeroable!(RawPBRMaterial);

impl From<&GltfMaterial> for RawPBRMaterial {
    fn from(value: &GltfMaterial) -> Self {
        Self {
            metallic: value.metallic,
            roughness: value.roughness,
            reflectance: value.reflectance,
            color: value.color,
            padding: 0.0,
        }
    }
}

pub fn sys_update_override_pbr_material_bind_group(
    rs: Res<RenderState>,
    main_pipeline: Res<MainPipeline>,
    white: Res<WhiteTexture>,
    normal_default: Res<NormalDefaultTexture>,
    layout: Res<PBRMaterialBindGroupLayout>,
    mut pbr_mats: Query<
        (&MeshRenderer, &PBRMaterial, &mut PBRMaterialOverride),
        Changed<PBRMaterial>,
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

        let mat = GltfMaterial {
            base_color_texture: pbr_mat.base_color_texture.clone().or(raw_mat
                .as_ref()
                .and_then(|it| it.base_color_texture.clone())),
            normal_texture: pbr_mat
                .normal_texture
                .clone()
                .or(raw_mat.as_ref().and_then(|it| it.normal_texture.clone())),
            roughness: pbr_mat
                .roughness
                .unwrap_or(raw_mat.map(|it| it.roughness).unwrap_or(Default::default())),
            metallic: pbr_mat
                .metallic
                .unwrap_or(raw_mat.map(|it| it.metallic).unwrap_or(Default::default())),
            reflectance: pbr_mat.reflectance.unwrap_or(
                raw_mat
                    .map(|it| it.reflectance)
                    .unwrap_or(Default::default()),
            ),
            color: pbr_mat.color.unwrap_or(Vec4::one()).into(),
            alpha_mode: pbr_mat.alpha_mode.unwrap_or(AlphaMode::Opaque),
        };
        override_pbr_mat.material = pbr_mat.clone();
        override_pbr_mat.uploaded_material = Some(Arc::new(UploadedPBRMaterial::from_gltf(
            &rs.device,
            &layout,
            &white.0,
            &normal_default.0,
            Arc::clone(&main_pipeline.pipeline),
            &mat,
        )))
    }
}
