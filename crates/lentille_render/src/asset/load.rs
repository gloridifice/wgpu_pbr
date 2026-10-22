use crate::{
    asset::{AssetRef, AssetRoot, SubAsset, TextureAsset},
    material::pbr::TextureOverride,
    prelude::*,
};
use anyhow::{Context, Result, ensure};
use bevy_ecs::world::World;
use lentille_wgpu_utils::typed_texture::{TypedTexture, TypedTextureViewDescriptor};
use std::{fs, path::Path};

pub trait Loadable: Sized {
    fn load(path: AssetPath, world: &mut World) -> Result<Self>;
}

impl UploadedImage<Dim2D, SampleFloatFilterable> {
    pub fn load_from_data(
        data: &gltf::image::Data,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Result<Self> {
        let pixels = rgba8_pixels(data)?;
        ensure!(
            data.width > 0
                && data.height > 0
                && data.width <= device.limits().max_texture_dimension_2d
                && data.height <= device.limits().max_texture_dimension_2d,
            "Texture dimensions exceed device limits"
        );
        ensure!(
            pixels.len() == data.width as usize * data.height as usize * 4,
            "Invalid texture data size"
        );
        let size = wgpu::Extent3d {
            width: data.width,
            height: data.height,
            depth_or_array_layers: 1,
        };
        let texture = TypedTexture::from_descriptor(
            device,
            &wgpu::TextureDescriptor {
                label: Some("Asset texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
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
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(data.width * 4),
                rows_per_image: Some(data.height),
            },
            size,
        );
        let view = texture.create_view(&TypedTextureViewDescriptor::new(None));
        Ok(Self { texture, view })
    }
    pub fn load_from_path(
        path: AssetPath,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Result<Self> {
        Self::load_from_data(
            &load_image(&AssetRoot::default().resolve(&path)?)?,
            device,
            queue,
            format,
        )
    }
    pub fn load_hdri_to_f16(
        path: &Path,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<Self> {
        let image = image::open(path)?.to_rgba32f();
        let (width, height) = image.dimensions();
        ensure!(
            width > 0
                && height > 0
                && width <= device.limits().max_texture_dimension_2d
                && height <= device.limits().max_texture_dimension_2d,
            "Environment dimensions exceed device limits"
        );
        let pixels: Vec<u8> = image
            .pixels()
            .flat_map(|p| {
                p.0.into_iter()
                    .flat_map(|v| half::f16::from_f32(v).to_le_bytes())
            })
            .collect();
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = TypedTexture::from_descriptor(
            device,
            &wgpu::TextureDescriptor {
                label: Some("Environment source"),
                size,
                mip_level_count: 1,
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
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 8),
                rows_per_image: Some(height),
            },
            size,
        );
        let view = texture.create_view(&TypedTextureViewDescriptor::new(None));
        Ok(Self { texture, view })
    }
}
pub fn rgba8_pixels(data: &gltf::image::Data) -> Result<Vec<u8>> {
    Ok(match data.format {
        gltf::image::Format::R8G8B8A8 => data.pixels.clone(),
        gltf::image::Format::R8G8B8 => data
            .pixels
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        gltf::image::Format::R8 => data.pixels.iter().flat_map(|v| [*v, *v, *v, 255]).collect(),
        gltf::image::Format::R8G8 => data
            .pixels
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        _ => anyhow::bail!(
            "Unsupported image format {:?}; use 8-bit textures",
            data.format
        ),
    })
}

pub fn load_image(path: &Path) -> Result<gltf::image::Data> {
    let image = image::open(path)
        .with_context(|| format!("Image {}", path.display()))?
        .to_rgba8();
    Ok(gltf::image::Data {
        width: image.width(),
        height: image.height(),
        format: gltf::image::Format::R8G8B8A8,
        pixels: image.into_raw(),
    })
}

/// Parse only CPU data. The caller owns the asset root and all GPU preparation.
pub fn import_model(root: &AssetRoot, source: &AssetPath) -> Result<Model> {
    let path = root.resolve(source)?;
    let document = gltf::Gltf::open(&path).with_context(|| format!("Model {}", path.display()))?;
    // Validate external dependencies before gltf::import follows their URIs.
    let parent = Path::new(match source {
        AssetPath::Assets(s) => s,
    })
    .parent()
    .unwrap_or(Path::new(""));
    for uri in document
        .buffers()
        .filter_map(|b| match b.source() {
            gltf::buffer::Source::Uri(s) => Some(s),
            _ => None,
        })
        .chain(document.images().filter_map(|i| match i.source() {
            gltf::image::Source::Uri { uri, .. } => Some(uri),
            _ => None,
        }))
    {
        if !uri.starts_with("data:") {
            ensure!(
                !uri.contains('%') && !uri.contains("://"),
                "Encoded or remote asset URI is not supported: {uri}"
            );
            root.resolve_path(&parent.join(uri))?;
        }
    }
    let (doc, buffers, images) =
        gltf::import(&path).with_context(|| format!("Import {}", path.display()))?;
    let tex = |texture: gltf::Texture<'_>| {
        TextureOverride::Set(AssetRef::<TextureAsset>::new(
            match source {
                AssetPath::Assets(s) => s,
            },
            SubAsset::Image(texture.source().index()),
        ))
    };
    let material = |mat: gltf::Material<'_>| {
        let pbr = mat.pbr_metallic_roughness();
        PbrMaterial {
            base_color_texture: pbr
                .base_color_texture()
                .map(|t| tex(t.texture()))
                .unwrap_or_default(),
            normal_texture: mat
                .normal_texture()
                .map(|t| tex(t.texture()))
                .unwrap_or_default(),
            metallic_roughness_ao_texture: pbr
                .metallic_roughness_texture()
                .map(|t| tex(t.texture()))
                .unwrap_or_default(),
            emission_texture: mat
                .emissive_texture()
                .map(|t| tex(t.texture()))
                .unwrap_or_default(),
            color: Some(Color::from_linear_array(pbr.base_color_factor())),
            roughness: Some(pbr.roughness_factor()),
            metallic: Some(pbr.metallic_factor()),
            reflectance: Some(0.),
            alpha_mode: Some(AlphaMode::from(mat.alpha_mode())),
        }
    };
    let materials = doc.materials().map(&material).collect();
    let mut meshes = Vec::new();
    for mesh in doc.meshes() {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut primitives = Vec::new();
        for primitive in mesh.primitives() {
            ensure!(
                primitive.mode() == gltf::mesh::Mode::Triangles,
                "Only triangle meshes are supported"
            );
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
            let positions: Vec<_> = reader
                .read_positions()
                .context("Mesh has no positions")?
                .collect();
            ensure!(!positions.is_empty(), "Mesh is empty");
            let normals: Vec<_> = reader.read_normals().into_iter().flatten().collect();
            let tangents: Vec<_> = reader.read_tangents().into_iter().flatten().collect();
            let uvs: Vec<_> = reader
                .read_tex_coords(0)
                .map(|v| v.into_f32().collect())
                .unwrap_or_default();
            let colors: Vec<_> = reader
                .read_colors(0)
                .map(|v| v.into_rgba_f32().collect())
                .unwrap_or_default();
            let base = vertices.len() as u32;
            let local: Vec<u32> = reader
                .read_indices()
                .map(|v| v.into_u32().collect())
                .unwrap_or_else(|| (0..positions.len() as u32).collect());
            ensure!(
                local.len() % 3 == 0 && local.iter().all(|i| (*i as usize) < positions.len()),
                "Invalid triangle indices"
            );
            for (i, position) in positions.into_iter().enumerate() {
                ensure!(
                    position.iter().all(|v| v.is_finite()),
                    "Non-finite vertex position"
                );
                vertices.push(Vertex {
                    position,
                    normal: *normals.get(i).unwrap_or(&[0., 1., 0.]),
                    tangent: tangents
                        .get(i)
                        .map(|t| [t[0], t[1], t[2]])
                        .unwrap_or([1., 0., 0.]),
                    color: *colors.get(i).unwrap_or(&[1.; 4]),
                    tex_coord: *uvs.get(i).unwrap_or(&[0.; 2]),
                });
            }
            primitives.push(Primitive {
                indices_start: indices.len() as _,
                indices_num: local.len() as _,
                material: Some(material(primitive.material())),
            });
            indices.extend(local.into_iter().map(|i| i + base));
        }
        ensure!(!primitives.is_empty(), "Mesh has no primitives");
        meshes.push(Mesh {
            vertices,
            indices,
            primitives,
        });
    }
    Ok(Model {
        meshes,
        images,
        materials,
    })
}
impl Loadable for Model {
    fn load(path: AssetPath, world: &mut World) -> Result<Self> {
        let root = world
            .get_resource::<AssetRoot>()
            .cloned()
            .unwrap_or_default();
        import_model(&root, &path)
    }
}
impl Loadable for ShaderModule {
    fn load(path: AssetPath, world: &mut World) -> Result<Self> {
        let source = fs::read_to_string(path.final_path())?;
        Ok(world.resource::<RenderState>().device.create_shader_module(
            wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(source.into()),
            },
        ))
    }
}
