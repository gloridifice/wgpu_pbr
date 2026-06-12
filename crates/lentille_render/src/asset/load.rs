use std::io::Cursor;

use std::path::Path;
use std::thread::{Scope, scope};
use std::time::Instant;
use std::{fs, io, thread};
use std::{fs::File, io::Read, sync::Arc};

use anyhow::Result;
use anyhow::anyhow;
use bevy_ecs::world::World;
use bevy_log::{error, info};
use gltf::image::{Data, Format};
use image::{ColorType, DynamicImage};
use png::Decoder;
use wgpu::{AddressMode, Extent3d, FilterMode, ShaderModule, TextureDescriptor, TextureUsages};

use crate::image::UploadedImageWithSampler;
use crate::mesh::{Mesh, Model, Primitive, Vertex};
use crate::prelude::GltfMaterial;
use crate::prelude::{Dim2D, SampleFloatFilterable, TexView2D, Tex2D};
use crate::{AlphaMode, RenderState};
use lentille_wgpu_utils::typed_texture::{TypedTexture, TypedTextureViewDescriptor};

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

fn is_png_srgb_from_memory(data: &[u8]) -> bool {
    let cursor = Cursor::new(data);
    let decoder = Decoder::new(cursor);
    let Ok(reader) = decoder.read_info() else {
        return false;
    };

    return reader.info().srgb.is_some();
}

macro_rules! match_image_format {
    (
        $img:expr,
        $($color:pat => {
            $convert:expr,
            $format:expr
        }),+ $(,)?
    ) => {{
        match $img.color() {
            $(
                $color => {
                    let image = $convert;
                    let data = image
                        .pixels()
                        .flat_map(|it| it.0.map(|it| it.to_be_bytes()))
                        .flat_map(|c| c)
                        .collect::<Vec<u8>>();
                    let (width, height) = image.dimensions();
                    Data {
                        pixels: data,
                        format: $format,
                        width,
                        height,
                    }
                }
            )+
            other => panic!("{:?} is an unsupported texture format!", other),
        }
    }};
}

/// Return (texture, is_transparent)
fn load_gltf_image_data_from_memory(data: &[u8]) -> Result<gltf::image::Data> {
    let dynamic_image = image::load_from_memory(data)?;
    Ok(match_image_format!(&dynamic_image,
        ColorType::Rgb8 => { dynamic_image.into_rgb8(), Format::R8G8B8 },
        ColorType::Rgba8 => { dynamic_image.into_rgba8(), Format::R8G8B8A8 },
        ColorType::Rgb16 => { dynamic_image.into_rgb16(), Format::R16G16B16 },
        ColorType::Rgba16 => { dynamic_image.into_rgba16(), Format::R16G16B16A16 },
    ))
}

fn load_gltf_image_data_from_path(path: impl AsRef<Path>) -> Result<gltf::image::Data> {
    let mut file = File::open(path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    load_gltf_image_data_from_memory(&buffer)
}

impl UploadedImageWithSampler<Dim2D, SampleFloatFilterable> {
    pub fn load_from_data(
        data: &gltf::image::Data,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Result<Self> {
        let size = Extent3d {
            width: data.width,
            height: data.height,
            depth_or_array_layers: 1,
        };
        let mut pixels = &data.pixels;
        let mut rgba = Vec::with_capacity(data.pixels.len() / 3 * 4);
        match data.format {
            Format::R8G8B8 => {
                for chunk in data.pixels.chunks(3) {
                    rgba.extend_from_slice(chunk);
                    rgba.push(255);
                }
                pixels = &rgba;
            }
            Format::R16G16B16 => {
                for chunk in data.pixels.chunks(6) {
                    rgba.extend_from_slice(chunk);
                    rgba.push(255);
                    rgba.push(255);
                }
                pixels = &rgba;
            }
            _ => {
                drop(rgba);
            }
        }

        let texture: Tex2D<SampleFloatFilterable> = TypedTexture::from_descriptor(
            device,
            &TextureDescriptor {
                label: None,
                size,
                mip_level_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: TextureUsages::COPY_DST
                    | TextureUsages::COPY_SRC
                    | TextureUsages::RENDER_ATTACHMENT
                    | TextureUsages::TEXTURE_BINDING,
                sample_count: 1,
                view_formats: &[],
            },
        );
        queue.write_texture(
            wgpu::TexelCopyTextureInfoBase {
                texture: texture.texture(),
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(
                    format.block_copy_size(None).unwrap_or(4) * data.width,
                ),
                rows_per_image: Some(data.height),
            },
            size,
        );

        let view: TexView2D<SampleFloatFilterable> =
            texture.create_view(&TypedTextureViewDescriptor::new(None));
        let sampler = device.create_sampler(&lentille_wgpu_utils::sampler_desc(
            None,
            AddressMode::MirrorRepeat,
            FilterMode::Linear,
        ));

        Ok(UploadedImageWithSampler {
            texture,
            view,
            sampler,
        })
    }

    pub fn load_from_path(
        path: AssetPath,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Result<Self> {
        let data = load_gltf_image_data_from_path(path.final_path())?;
        Self::load_from_data(&data, device, queue, format)
    }

    pub fn load_hdri_to_f16(
        path: AssetPath,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<Self> {
        let path = path.final_path();
        let mut file = File::open(&path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        let dynamic_image = image::load_from_memory(&buffer)?;
        let image = dynamic_image.to_rgba32f();

        let (width, height) = image.dimensions();
        let size = Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let mut image_data = Vec::with_capacity((width * height) as usize * 4 * 2);
        for px in image.pixels() {
            for &channel in px.0.iter() {
                let h = half::f16::from_f32(channel);
                image_data.extend_from_slice(&h.to_le_bytes());
            }
        }

        let texture: Tex2D<SampleFloatFilterable> = TypedTexture::from_descriptor(
            device,
            &wgpu::TextureDescriptor {
                size,
                mip_level_count: 1,
                label: None,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            },
        );
        queue.write_texture(
            wgpu::TexelCopyTextureInfoBase {
                texture: texture.texture(),
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &image_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4 * 2),
                rows_per_image: Some(height),
            },
            size,
        );

        let view: TexView2D<SampleFloatFilterable> =
            texture.create_view(&TypedTextureViewDescriptor::new(None));
        let sampler = device.create_sampler(&UploadedImageWithSampler::<Dim2D, SampleFloatFilterable>::default_sampler_desc());

        Ok(UploadedImageWithSampler {
            texture,
            view,
            sampler,
        })
    }
}

type Loaded = (
    gltf::Document,
    Vec<gltf::buffer::Data>,
    Vec<gltf::image::Data>,
);

fn load_by_glb<'a>(path: impl AsRef<Path>) -> Result<Loaded> {
    Ok(gltf::import(&path)?)
}

fn load_by_gltf<'a>(path: impl AsRef<Path>, scope: &'a Scope<'a, '_>) -> Result<Loaded> {
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
                let path = parent_dir.join(uri);
                Some(scope.spawn(move || {
                    let loaded = load_gltf_image_data_from_path(path).unwrap();
                    loaded
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
            scope(|scope| load_by_gltf(&path, scope))?
        } else if path.ends_with(".glb") {
            load_by_glb(&path)?
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

                        // Check is transparent and get decide alpha mode
                        let mut alpha_mode = AlphaMode::Opaque;
                        let base_color_texture = pbr.base_color_texture().map(|info| {
                            let index = info.texture().index();
                            let data = &images[index];
                            Arc::new(
                                UploadedImageWithSampler::load_from_data(
                                    data,
                                    device,
                                    queue,
                                    wgpu::TextureFormat::Rgba8UnormSrgb,
                                )
                                .unwrap(),
                            )
                        });
                        let normal_texture = mat.normal_texture().map(|info| {
                            let index = info.texture().index();
                            Arc::new(
                                UploadedImageWithSampler::load_from_data(
                                    &images[index],
                                    device,
                                    queue,
                                    wgpu::TextureFormat::Rgba8Unorm,
                                )
                                .unwrap(),
                            )
                        });

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
                Mesh {
                    vertices,
                    indices,
                    primitives,
                }
            })
            .collect::<Vec<Mesh>>();

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
