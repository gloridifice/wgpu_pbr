use std::sync::Arc;

use crate::{bg_descriptor, impl_pod_zeroable, macro_utils::BGLEntry, render::prelude::*};
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
                1: ShaderStages::FRAGMENT => BGLEntry::Tex2D(false, TextureSampleType::Float { filterable: true });
                2: ShaderStages::FRAGMENT => BGLEntry::Sampler(SamplerBindingType::Filtering);
            )));
        Self(material_bind_group_layout)
    }
}

#[derive(Clone)]
pub struct GltfMaterial {
    pub base_color_texture: Option<Arc<UploadedImageWithSampler>>,
    pub roughness: f32,
    pub metallic: f32,
    pub reflectance: f32,
}

impl Default for GltfMaterial {
    fn default() -> Self {
        Self {
            base_color_texture: None,
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
        main_pipeline: Arc<RenderPipeline>,
        gltf_material: &GltfMaterial,
    ) -> Self {
        let base_color = gltf_material
            .base_color_texture
            .as_ref()
            .map(|it| it.as_ref())
            .unwrap_or(white_texture);
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

#[derive(Component, Clone)]
#[require(PBRMaterialOverride)]
pub struct PBRMaterial {
    pub mat: GltfMaterial,
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
