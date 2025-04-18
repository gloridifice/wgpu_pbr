use std::path::Path;
use std::thread::{scope, Scope};
use std::time::Instant;
use std::{fs, io, thread};
use std::{fs::File, io::Read, sync::Arc};

use crate::render::{self, UploadedImageWithSampler};
use crate::render::{prelude::*, AlphaMode};
use crate::RenderState;
use anyhow::*;
use bevy_ecs::world::World;
use image::{ColorType, DynamicImage};
use log::{error, info};
use wgpu::util::DeviceExt;
use wgpu::ShaderModule;

use super::AssetPath;

pub trait Loadable: Sized {
    fn load(path: AssetPath, world: &mut World) -> Result<Self>;
}

fn has_alpha(img: &DynamicImage) -> bool {
    matches!(
        img.color(),
        ColorType::La8
            | ColorType::La16
            | ColorType::Rgba8
            | ColorType::Rgba16
            | ColorType::Rgba32F
    )
}

fn is_any_pixel_transparent(img: &DynamicImage) -> bool {
    let rgba = img.to_rgba8();
    rgba.pixels().any(|p| p[3] < 255)
}

/// Return (texture, is_transparent)
fn load_texture_from_memory(
    data: &[u8],
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<(UploadedImageWithSampler, bool)> {
    let dynamic_image = image::load_from_memory(data)?;
    let is_transparent = has_alpha(&dynamic_image) && is_any_pixel_transparent(&dynamic_image);
    let image = dynamic_image.to_rgba8();

    let (width, height) = image.dimensions();
    let size = Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };

    let texture = device.create_texture_with_data(
        queue,
        &wgpu::TextureDescriptor {
            size,
            mip_level_count: 1,
            label: None,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        },
        Default::default(),
        &image,
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&UploadedImageWithSampler::default_sampler_desc());

    Ok((
        UploadedImageWithSampler {
            size,
            texture,
            view,
            sampler,
        },
        is_transparent,
    ))
}

fn load_texture_from_path(
    path: impl AsRef<Path>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<(UploadedImageWithSampler, bool)> {
    let mut file = File::open(path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    load_texture_from_memory(&buffer, device, queue)
}

impl Loadable for UploadedImageWithSampler {
    fn load(path: AssetPath, world: &mut World) -> Result<Self> {
        let rs = world.resource::<RenderState>();
        load_texture_from_path(path.final_path(), &rs.device, &rs.queue).map(|it| it.0)
    }
}

type Loaded = (
    gltf::Document,
    Vec<gltf::buffer::Data>,
    // (texture, is_tranparent)
    Vec<(Arc<UploadedImageWithSampler>, bool)>,
);

fn load_by_glb<'a>(
    path: impl AsRef<Path>,
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    scope: &'a Scope<'a, '_>,
) -> Result<Loaded> {
    let (document, buffers, images) = gltf::import(&path)?;

    let handles = images
        .into_iter()
        .map(|data| {
            scope.spawn(move || {
                Arc::new(UploadedImageWithSampler::from_glb_data(
                    &data, device, queue,
                ))
            })
        })
        .collect::<Vec<_>>();

    let images = handles
        .into_iter()
        .map(|handle| (handle.join().unwrap(), false))
        .collect();

    Ok((document, buffers, images))
}

fn load_by_gltf<'a>(
    path: impl AsRef<Path>,
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    scope: &'a Scope<'a, '_>,
) -> Result<Loaded> {
    let file = fs::File::open(&path)?;
    let buf = io::BufReader::new(file);
    let gltf = gltf::Gltf::from_reader(buf)?;

    let buffer_defs: Vec<_> = gltf.buffers().collect();
    let parent_dir = std::path::Path::new(path.as_ref())
        .parent()
        .unwrap()
        .to_path_buf();

    let handles: Vec<_> = buffer_defs
        .into_iter()
        .filter_map(|buffer| match buffer.source() {
            gltf::buffer::Source::Bin => {
                error!("A Bin source here!");
                None
            }
            gltf::buffer::Source::Uri(uri) => {
                let path = parent_dir.join(uri);
                Some(thread::spawn(move || -> Vec<u8> {
                    let mut file = File::open(path).unwrap();
                    let mut data = Vec::new();
                    file.read_to_end(&mut data).unwrap();
                    data
                }))
            }
        })
        .collect();
    let buffers: Vec<_> = handles
        .into_iter()
        .map(|h| gltf::buffer::Data(h.join().unwrap()))
        .collect();

    let image_defs: Vec<_> = gltf.images().collect();

    let handles: Vec<_> = image_defs
        .into_iter()
        .filter_map(|img| match img.source() {
            gltf::image::Source::View { .. } => {
                error!("A Bin texture here!");
                None
            }
            gltf::image::Source::Uri { uri, .. } => {
                let path = parent_dir.join(&uri);
                Some(scope.spawn(move || {
                    let loaded = load_texture_from_path(path, device, queue).unwrap();
                    (Arc::new(loaded.0), loaded.1)
                }))
            }
        })
        .collect();

    let images: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    Ok((gltf.document, buffers, images))
}

impl Loadable for Model {
    fn load(path: AssetPath, world: &mut World) -> Result<Self> {
        let rs = world.resource::<RenderState>();
        let device = &rs.device;
        let queue = &rs.queue;
        let path = path.final_path();
        info!("= Start Loading <{}>", &path);
        let start_instant = Instant::now();

        let (document, buffers, images) = if path.ends_with(".gltf") {
            scope(|scope| load_by_gltf(&path, device, queue, scope))?
        } else if path.ends_with(".glb") {
            scope(|scope| load_by_glb(&path, device, queue, scope))?
        } else {
            return Err(anyhow!("<{}> is not a model file (.gltf or .glb)!", &path));
        };

        info!(
            "  - imported from path, using {}s",
            start_instant.elapsed().as_secs_f64()
        );

        let meshes = document
            .meshes()
            .map(|mesh| {
                let mut vertices = Vec::<Vertex>::new();
                let mut indices = Vec::<u32>::new();
                let mut primitives = Vec::<Primitive>::new();
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
                            it.map(|it| Arc::clone(&images.get(it.index()).unwrap().0))
                        };

                        // Check is transparent and get decide alpha mode
                        let alpha_mode = pbr
                            .base_color_texture()
                            .map(|it| {
                                if images.get(it.texture().index()).unwrap().1 {
                                    AlphaMode::Blend
                                } else {
                                    AlphaMode::Opaque
                                }
                            })
                            .unwrap_or(AlphaMode::Opaque);

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
                            alpha_mode,
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
                render::mesh::Mesh {
                    vertices,
                    indices,
                    primitives,
                }
            })
            .collect::<Vec<render::mesh::Mesh>>();

        let duration = start_instant.elapsed();
        info!(
            "   ✅ End Loading <{}>, using {}s",
            &path,
            &duration.as_secs_f64()
        );
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
