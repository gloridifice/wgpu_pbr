use std::{fs::File, io::Read};

use wgpu::TextureViewDescriptor;

use crate::render::UploadedImage;

use super::AssetPath;

/// Order of paths is +x, -x, +y, -y, +z, -z
pub fn load_cubemap_sliced(
    paths: &[AssetPath; 6],
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<UploadedImage> {
    let mut byte_images = Vec::with_capacity(6);
    for path in paths {
        let mut file = File::open(path.final_path())?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        byte_images.push(image::load_from_memory(&buffer)?.to_rgba8());
    }

    let dimensions = byte_images[0].dimensions();
    let width = dimensions.0;
    let height = dimensions.1;
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 6,
    };

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        size,
        mip_level_count: 1,
        label: None,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });

    for (index, image) in byte_images.iter().enumerate() {
        queue.write_texture(
            wgpu::TexelCopyTextureInfoBase {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: index as u32,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &image,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }

    let view = texture.create_view(&TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::Cube),
        ..Default::default()
    });

    Ok(UploadedImage { texture, view })
}
