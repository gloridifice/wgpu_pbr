use std::{fs::File, io::Read};

use crate::{
    render::{Image, Mesh, UploadedMesh, Vertex},
    State,
};
use anyhow::*;
use gltf::{Glb, Gltf};

use super::AssetPath;

pub trait Loadable: Sized {
    fn load(path: AssetPath, state: &mut State) -> Result<Self>;
}

impl Loadable for Image {
    fn load(path: AssetPath, state: &mut State) -> Result<Self> {
        let mut file = File::open(path.final_path())?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        let image = image::load_from_memory(&buffer)?.to_rgba8();

        let dimensions = image.dimensions();
        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };

        let texture = state.device.create_texture(&wgpu::TextureDescriptor {
            size,
            mip_level_count: 1,
            label: None,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        state.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &image,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * dimensions.0),
                rows_per_image: Some(dimensions.1),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = state.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Ok(Image {
            size,
            texture,
            view,
            sampler,
        })
    }
}

impl Loadable for Mesh {
    fn load(path: AssetPath, state: &mut State) -> Result<Self> {
        let path = path.final_path();
        let (document, buffers, images) = gltf::import(path)?;

        // let meshes = Vec>::new();

        for mesh in document.meshes() {
            for primitive in mesh.primitives() {
                let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
                let positions = reader
                    .read_positions()
                    .map(|v| v.collect::<Vec<_>>())
                    .unwrap_or_default();
                let normals = reader
                    .read_normals()
                    .map(|v| v.collect::<Vec<_>>())
                    .unwrap_or_default();
                let tex_coords = reader
                    .read_tex_coords(0)
                    .map(|v| v.into_f32().collect::<Vec<_>>())
                    .unwrap_or_default();
                let colors = reader
                    .read_colors(0)
                    .map(|v| v.into_rgba_f32().collect::<Vec<_>>())
                    .unwrap_or_default();
                let indices = reader
                    .read_indices()
                    .map(|v| v.into_u32().collect::<Vec<_>>())
                    .unwrap_or_default();

                let mut vertices = Vec::<Vertex>::with_capacity(positions.len());
                for i in 0..positions.len() {
                    vertices[i] = Vertex {
                        position: positions[i],
                        normal: normals[i],
                        color: colors[i],
                        tex_coord: tex_coords[i],
                    };
                }

                return Ok(Mesh { vertices, indices });
            }
        }

        Err(anyhow!("Failed to load Mesh!"))
    }
}
