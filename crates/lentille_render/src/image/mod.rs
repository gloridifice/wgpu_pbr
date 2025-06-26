use wgpu::{Texture, TextureView};

use crate::prelude::*;

pub mod cubemap;

pub struct UploadedImageWithSampler {
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
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        }
    }
}
