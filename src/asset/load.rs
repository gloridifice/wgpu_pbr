use std::fs;
use std::{fs::File, io::Read, sync::Arc};

use crate::render::material::pbr::GltfMaterial;
use crate::render::{self, Model, Primitive, UploadedImageWithSampler, Vertex};
use crate::RenderState;
use anyhow::*;
use bevy_ecs::world::World;
use wgpu::ShaderModule;

use super::AssetPath;

pub trait Loadable: Sized {
    fn load(path: AssetPath, world: &mut World) -> Result<Self>;
}

impl Loadable for UploadedImageWithSampler {
    fn load(path: AssetPath, world: &mut World) -> Result<Self> {
        let mut file = File::open(path.final_path())?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        let image = image::load_from_memory(&buffer)?.to_rgba8();
        let render_state = world.resource::<RenderState>();

        let dimensions = image.dimensions();
        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };

        let texture = render_state
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

        render_state.queue.write_texture(
            wgpu::TexelCopyTextureInfoBase {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &image,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * dimensions.0),
                rows_per_image: Some(dimensions.1),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = render_state
            .device
            .create_sampler(&UploadedImageWithSampler::default_sampler_desc());

        Ok(UploadedImageWithSampler {
            size,
            texture,
            view,
            sampler,
        })
    }
}

impl Loadable for Model {
    fn load(path: AssetPath, world: &mut World) -> Result<Self> {
        let path = path.final_path();
        let (document, buffers, images) = gltf::import(path)?;
        let render_state = world.resource::<RenderState>();

        let meshes = document
            .meshes()
            .map(|mesh| {
                let mut vertices = Vec::<Vertex>::new();
                let mut indices = Vec::<u32>::new();
                let mut primitives = Vec::<render::Primitive>::new();
                for primitive in mesh.primitives() {
                    let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

                    let positions = reader
                        .read_positions()
                        .map(|v| {
                            // v.map(|raw_pos| (rotate_90 * Vector3::from(raw_pos)).into())
                            v.collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    let normals = reader
                        .read_normals()
                        .map(|v| v.collect::<Vec<_>>())
                        .unwrap_or_default();
                    let tangents = reader
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
                            tangent: *tangents.get(i).unwrap_or(&[0.0; 3]),
                            color: *colors.get(i).unwrap_or(&[0.0; 4]),
                            tex_coord: *tex_coords.get(i).unwrap_or(&[0.0; 2]),
                        };
                        vertices.push(v);
                    }

                    let material_instance: GltfMaterial = {
                        let mat = primitive.material();

                        let pbr = mat.pbr_metallic_roughness();
                        let map_texture = |it: Option<gltf::Texture>| {
                            it.map(|it| {
                                Arc::new(UploadedImageWithSampler::from_glb_data(
                                    images.get(it.index()).unwrap(),
                                    &it.sampler(),
                                    &render_state.device,
                                    &render_state.queue,
                                ))
                            })
                        };

                        let base_color_texture =
                            map_texture(pbr.base_color_texture().map(|it| it.texture()));
                        let normal_texture =
                            map_texture(mat.normal_texture().map(|it| it.texture()));

                        GltfMaterial {
                            base_color_texture,
                            normal_texture,
                            color: pbr.base_color_factor(),
                            roughness: pbr.roughness_factor(),
                            metallic: pbr.metallic_factor(),
                            reflectance: 0.0,
                            alpha_mode: mat.alpha_mode().into(),
                        }
                    };

                    let indices_start = indices.len() as u32;
                    let indices_num = primitive_indices.len() as u32;

                    indices.append(&mut primitive_indices);
                    primitives.push(Primitive {
                        indices_start,
                        indices_num,
                        material: Some(material_instance),
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

impl Loadable for ShaderModule {
    fn load(path: AssetPath, world: &mut World) -> Result<Self> {
        let path = path.final_path();
        let rs = &world.resource::<RenderState>();
        let device = &rs.device;
        let wgsl_string = fs::read_to_string(path)?;
        Ok(device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(wgsl_string.into()),
        }))
    }
}
