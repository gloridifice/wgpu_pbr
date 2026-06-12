use lentille_wgpu_utils::typed_texture::{
    TextureSampleTypeState, TextureViewDimensionState, TypedTextureView,
};

use crate::prelude::*;

pub mod cubemap;

pub struct UploadedImageWithSampler<D: TextureViewDimensionState, S: TextureSampleTypeState> {
    #[allow(unused)]
    pub texture: Tex2D<S>,
    pub view: TypedTextureView<D, S>,
    pub sampler: Sampler,
}

pub struct UploadedImage<D: TextureViewDimensionState, S: TextureSampleTypeState> {
    #[allow(unused)]
    pub texture: Tex2D<S>,
    pub view: TypedTextureView<D, S>,
}

impl<D: TextureViewDimensionState, S: TextureSampleTypeState> UploadedImageWithSampler<D, S> {
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

    pub fn size(&self) -> Extent3d {
        self.texture.size()
    }

    pub fn default_sampler_desc() -> wgpu::SamplerDescriptor<'static> {
        wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        }
    }
}
