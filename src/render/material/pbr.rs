use std::sync::Arc;

use crate::{
    bg_descriptor, impl_pod_zeroable,
    macro_utils::BGLEntry,
    render::{
        defered_rendering::MainPipeline, prelude::*, MeshRenderer, NormalDefaultTexture,
        WhiteTexture,
    },
};
use bevy_ecs::prelude::*;
use wgpu::{util::DeviceExt, BindingResource, BufferUsages};

use super::UploadedMaterial;

#[derive(Resource, Clone)]
pub struct PBRMaterialBindGroupLayout(pub Arc<BindGroupLayout>);

impl FromWorld for PBRMaterialBindGroupLayout {
    fn from_world(world: &mut World) -> Self {
        let rs = world.resource::<RenderState>();
        let device = &rs.device;
        let material_bind_group_layout =
            Arc::new(device.create_bind_group_layout(&bg_layout_descriptor!(
                ["Material Bind Group Layout"]
                0: ShaderStages::FRAGMENT => BGLEntry::UniformBuffer();
                1: ShaderStages::FRAGMENT => BGLEntry::Tex2D(false, TextureSampleType::Float { filterable: true }); // BaseColor Tex
                2: ShaderStages::FRAGMENT => BGLEntry::Sampler(SamplerBindingType::Filtering);
                3: ShaderStages::FRAGMENT => BGLEntry::Tex2D(false, TextureSampleType::Float { filterable: true }); // Normal Tex
                4: ShaderStages::FRAGMENT => BGLEntry::Sampler(SamplerBindingType::Filtering);
            )));
        Self(material_bind_group_layout)
    }
}

#[derive(Clone)]
pub struct GltfMaterial {
    pub base_color_texture: Option<Arc<UploadedImageWithSampler>>,
    pub normal_texture: Option<Arc<UploadedImageWithSampler>>,
    pub roughness: f32,
    pub metallic: f32,
    pub reflectance: f32,
}

impl Default for GltfMaterial {
    fn default() -> Self {
        Self {
            base_color_texture: None,
            normal_texture: None,
            roughness: 1.0,
            metallic: 0.0,
            reflectance: 0.5,
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
    pub material: Option<Arc<UploadedPBRMaterial>>,
}

#[derive(Component, Clone, Default)]
#[require(PBRMaterialOverride)]
pub struct PBRMaterial {
    pub base_color_texture: Option<Arc<UploadedImageWithSampler>>,
    pub normal_texture: Option<Arc<UploadedImageWithSampler>>,
    pub roughness: Option<f32>,
    pub metallic: Option<f32>,
    pub reflectance: Option<f32>,
}

#[derive(Clone, Copy, Debug)]
pub struct RawPBRMaterial {
    pub metallic: f32,
    pub roughness: f32,
    pub reflectance: f32,
}
impl_pod_zeroable!(RawPBRMaterial);

impl From<&GltfMaterial> for RawPBRMaterial {
    fn from(value: &GltfMaterial) -> Self {
        Self {
            metallic: value.metallic,
            roughness: value.roughness,
            reflectance: value.reflectance,
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
    for (mesh, ove_mat, mut ove) in pbr_mats.iter_mut() {
        let raw_mat = mesh
            .mesh
            .as_ref()
            .map(|it| {
                it.primitives
                    .first()
                    .as_ref()
                    .map(|primitive| primitive.material.as_ref())
            })
            .flatten()
            .flatten();
        let mat = GltfMaterial {
            base_color_texture: ove_mat.base_color_texture.clone().or(raw_mat
                .as_ref()
                .map(|it| it.base_color_texture.clone())
                .flatten()),
            normal_texture: ove_mat.normal_texture.clone().or(raw_mat
                .as_ref()
                .map(|it| it.normal_texture.clone())
                .flatten()),
            roughness: ove_mat
                .roughness
                .unwrap_or(raw_mat.map(|it| it.roughness).unwrap_or(Default::default())),
            metallic: ove_mat
                .metallic
                .unwrap_or(raw_mat.map(|it| it.metallic).unwrap_or(Default::default())),
            reflectance: ove_mat.reflectance.unwrap_or(
                raw_mat
                    .map(|it| it.reflectance)
                    .unwrap_or(Default::default()),
            ),
        };
        ove.material = Some(Arc::new(UploadedPBRMaterial::from_gltf(
            &rs.device,
            &layout,
            &white.0,
            &normal_default.0,
            Arc::clone(&main_pipeline.pipeline),
            &mat,
        )))
    }
}
