use std::{fs::File, io::Read, sync::Arc};

use crate::{
    render::{self, GltfMaterial, Model, Primitive, UploadedImage, Vertex},
    State,
};
use anyhow::*;
use cgmath::{Matrix3, Vector3};

use super::AssetPath;

pub trait Loadable: Sized {
    fn load(path: AssetPath, state: &mut State) -> Result<Self>;
}

impl Loadable for UploadedImage {
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

        let texture = state
            .render_state()
            .device
            .create_texture(&wgpu::TextureDescriptor {
                size,
                mip_level_count: 1,
                label: None,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });

        state.render_state().queue.write_texture(
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
        let sampler = state
            .render_state()
            .device
            .create_sampler(&UploadedImage::default_sampler_desc());

        Ok(UploadedImage {
            size,
            texture,
            view,
            sampler,
        })
    }
}

impl Loadable for Model {
    fn load(path: AssetPath, state: &mut State) -> Result<Self> {
        let path = path.final_path();
        let (document, buffers, images) = gltf::import(path)?;

        let meshes = document
            .meshes()
            .into_iter()
            .map(|mesh| {
                let mut vertices = Vec::<Vertex>::new();
                let mut indices = Vec::<u32>::new();
                let mut primitives = Vec::<render::Primitive>::new();
                for primitive in mesh.primitives() {
                    let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

                    let rotate_90 = Matrix3::from_angle_x(cgmath::Deg(90f32));
                    let positions = reader
                        .read_positions()
                        .map(|v| {
                            v.map(|raw_pos| (rotate_90 * Vector3::from(raw_pos)).into())
                                .collect::<Vec<_>>()
                        })
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
                    let mut primitive_indices = reader
                        .read_indices()
                        .map(|v| v.into_u32().collect::<Vec<_>>())
                        .unwrap_or_default();

                    for i in 0..positions.len() {
                        let v = Vertex {
                            position: *positions.get(i).unwrap_or(&[0.0; 3]),
                            normal: *normals.get(i).unwrap_or(&[0.0; 3]),
                            color: *colors.get(i).unwrap_or(&[0.0; 4]),
                            tex_coord: *tex_coords.get(i).unwrap_or(&[0.0; 2]),
                        };
                        vertices.push(v);
                    }

                    let material_instance: Option<GltfMaterial> = {
                        let base_color = primitive
                            .material()
                            .pbr_metallic_roughness()
                            .base_color_texture();
                        base_color.map(|tex_info| {
                            let uploaded_image = Arc::new(UploadedImage::from_glb_data(
                                images.get(tex_info.texture().index()).unwrap(),
                                &tex_info.texture().sampler(),
                                state,
                            ));
                            GltfMaterial {
                                base_color_texture: uploaded_image,
                            }
                        })
                    };

                    let indices_start = indices.len() as u32;
                    let indices_num = primitive_indices.len() as u32;

                    indices.append(&mut primitive_indices);
                    primitives.push(Primitive {
                        indices_start,
                        indices_num,
                        material: material_instance,
                    });
                }
                render::Mesh {
                    vertices,
                    indices,
                    primitives,
                }
            })
            .collect::<Vec<render::Mesh>>();

        Ok(Model { meshes })
    }
}
