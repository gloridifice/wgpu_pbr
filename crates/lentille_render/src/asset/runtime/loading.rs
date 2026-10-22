use super::*;
use lentille_wgpu_utils::typed_texture::{TypedTexture, TypedTextureViewDescriptor};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

#[cfg(test)]
mod tests;

pub struct RgbaImageData {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}
impl RgbaImageData {
    pub fn from_gltf(data: &gltf::image::Data) -> Result<Self> {
        let pixels = super::super::load::rgba8_pixels(data)?;
        ensure!(
            data.width > 0
                && data.height > 0
                && pixels.len() as u64 == u64::from(data.width) * u64::from(data.height) * 4,
            "Invalid image data"
        );
        Ok(Self {
            width: data.width,
            height: data.height,
            pixels,
        })
    }
}

type TextureKey = (AssetRef<TextureAsset>, bool);

/// All file access and image conversion happens before this crosses to the render thread.
pub struct CpuAssets {
    root: AssetRoot,
    models: HashMap<AssetPath, Arc<Model>>,
    meshes: Vec<(String, Arc<Mesh>)>,
    textures: Vec<(TextureKey, Arc<RgbaImageData>)>,
    materials: Vec<PbrMaterial>,
    pub cpu_bytes: u64,
    pub gpu_bytes: u64,
}
impl CpuAssets {
    pub fn load(
        root: AssetRoot,
        requests: &[(MeshSource, Option<PbrMaterial>)],
        max_cpu_bytes: u64,
        mut checkpoint: impl FnMut(&str) -> Result<()>,
    ) -> Result<Self> {
        let mut result = Self {
            root,
            models: HashMap::new(),
            meshes: Vec::new(),
            textures: Vec::new(),
            materials: Vec::new(),
            cpu_bytes: 0,
            gpu_bytes: 0,
        };
        let mut images = HashMap::<AssetRef<TextureAsset>, Arc<RgbaImageData>>::new();
        for (source, overrides) in requests {
            checkpoint(&format!("Mesh {source:?}"))?;
            let key = match source {
                MeshSource::Asset(r) => format!("{:?}/{:?}", normalized(&r.path)?, r.label),
                MeshSource::Plane { size } => format!("plane/{}", size.to_bits()),
            };
            let data = if let Some((_, data)) = result.meshes.iter().find(|(k, _)| *k == key) {
                data.clone()
            } else {
                let data = match source {
                    MeshSource::Asset(r) => {
                        let SubAsset::Mesh(index) = r.label else {
                            anyhow::bail!("Mesh reference requires Mesh(index)");
                        };
                        let model = result.model(&r.path)?;
                        Arc::new(
                            model
                                .meshes
                                .get(index)
                                .context("Missing mesh subresource")?
                                .clone(),
                        )
                    }
                    MeshSource::Plane { size } => {
                        ensure!(size.is_finite() && *size > 0., "Invalid plane size");
                        Arc::new(Mesh::plane(*size))
                    }
                };
                result.cpu_bytes += mesh_bytes(&data);
                result.gpu_bytes += mesh_bytes(&data);
                result.meshes.push((key, data.clone()));
                data
            };
            for primitive in &data.primitives {
                checkpoint("Material dependencies")?;
                let base = primitive.material.clone().unwrap_or_default();
                let description = overrides.as_ref().map(|v| v.merged(&base)).unwrap_or(base);
                if result.materials.contains(&description) {
                    continue;
                }
                for (i, texture) in [
                    &description.base_color_texture,
                    &description.normal_texture,
                    &description.metallic_roughness_ao_texture,
                    &description.emission_texture,
                ]
                .into_iter()
                .enumerate()
                {
                    let TextureOverride::Set(reference) = texture else {
                        continue;
                    };
                    let mut reference = reference.clone();
                    reference.path = normalized(&reference.path)?;
                    let key = (reference.clone(), i == 0 || i == 3);
                    if result.textures.iter().any(|(k, _)| *k == key) {
                        continue;
                    }
                    checkpoint(&format!("Texture {reference:?}"))?;
                    let image = if let Some(image) = images.get(&reference) {
                        image.clone()
                    } else {
                        let data = match reference.label {
                            SubAsset::File => load_image(&result.root.resolve(&reference.path)?)?,
                            SubAsset::Image(index) => result
                                .model(&reference.path)?
                                .images
                                .get(index)
                                .context("Missing image subresource")?
                                .clone(),
                            _ => anyhow::bail!("Texture reference must select File or Image"),
                        };
                        let image = Arc::new(RgbaImageData::from_gltf(&data)?);
                        result.cpu_bytes += image.pixels.len() as u64;
                        images.insert(reference, image.clone());
                        image
                    };
                    result.gpu_bytes += image.pixels.len() as u64;
                    result.textures.push((key, image));
                    ensure!(
                        result.cpu_bytes <= max_cpu_bytes,
                        "Scene CPU asset budget exceeded"
                    );
                }
                result.materials.push(description);
            }
            ensure!(
                result.cpu_bytes <= max_cpu_bytes,
                "Scene CPU asset budget exceeded"
            );
        }
        checkpoint("CPU assets ready")?;
        Ok(result)
    }
    fn model(&mut self, path: &AssetPath) -> Result<Arc<Model>> {
        let path = normalized(path)?;
        if let Some(model) = self.models.get(&path) {
            return Ok(model.clone());
        }
        let model = Arc::new(import_model(&self.root, &path)?);
        self.cpu_bytes += model_bytes(&model);
        self.models.insert(path, model.clone());
        Ok(model)
    }
    pub fn into_upload(self) -> AssetUpload {
        let mut cache = SceneAssetCache::new(self.root);
        cache.models = self.models;
        cache.preloaded = true;
        AssetUpload {
            cache,
            meshes: self.meshes.into(),
            textures: self.textures.into(),
            materials: self.materials.into(),
            current: None,
            uploaded_bytes: 0,
            total_bytes: self.gpu_bytes,
        }
    }
}
pub(super) fn mesh_bytes(mesh: &Mesh) -> u64 {
    (mesh.vertices.len() * std::mem::size_of::<Vertex>() + mesh.indices.len() * 4) as u64
}
pub(super) fn model_bytes(model: &Model) -> u64 {
    model.meshes.iter().map(mesh_bytes).sum::<u64>()
        + model
            .images
            .iter()
            .map(|i| i.pixels.len() as u64)
            .sum::<u64>()
}

pub struct FrameBudget {
    start: Instant,
    duration: Duration,
    limit: usize,
    pub used_bytes: usize,
}
impl FrameBudget {
    pub fn new(duration: Duration, bytes: usize) -> Self {
        Self {
            start: Instant::now(),
            duration: duration.max(Duration::from_micros(1)),
            limit: bytes.max(4),
            used_bytes: 0,
        }
    }
    pub fn available(&self) -> bool {
        self.start.elapsed() < self.duration && self.used_bytes < self.limit
    }
    pub fn remaining(&self) -> usize {
        self.limit.saturating_sub(self.used_bytes)
    }
    pub fn rows(&self, stride: usize, remaining: u32) -> u32 {
        let rows = self.remaining() / stride;
        let rows = if self.used_bytes == 0 {
            rows.max(1)
        } else {
            rows
        };
        rows.min(remaining as usize) as u32
    }
    pub fn consume(&mut self, bytes: usize) {
        self.used_bytes += bytes;
    }
}

enum PendingUpload {
    Mesh {
        key: String,
        data: Arc<Mesh>,
        uploaded: UploadedMesh,
        offset: usize,
    },
    Texture {
        key: TextureKey,
        data: Arc<RgbaImageData>,
        uploaded: UploadedImage<Dim2D, SampleFloatFilterable>,
        row: u32,
    },
}
pub struct AssetUpload {
    pub cache: SceneAssetCache,
    meshes: VecDeque<(String, Arc<Mesh>)>,
    textures: VecDeque<(TextureKey, Arc<RgbaImageData>)>,
    materials: VecDeque<PbrMaterial>,
    current: Option<PendingUpload>,
    pub uploaded_bytes: u64,
    pub total_bytes: u64,
}
impl AssetUpload {
    pub fn finish(mut self) -> Result<SceneAssetCache> {
        ensure!(
            self.current.is_none()
                && self.meshes.is_empty()
                && self.textures.is_empty()
                && self.materials.is_empty(),
            "Scene uploads are incomplete"
        );
        self.cache.preloaded = false;
        Ok(self.cache)
    }
    pub fn advance(&mut self, world: &World, budget: &mut FrameBudget) -> Result<bool> {
        let rs = world.resource::<RenderState>();
        while budget.available() {
            if self.current.is_none() {
                if let Some((key, data)) = self.meshes.pop_front() {
                    let vb = data.vertices.len() * std::mem::size_of::<Vertex>();
                    let ib = data.indices.len() * 4;
                    ensure!(
                        vb as u64 <= rs.device.limits().max_buffer_size
                            && ib as u64 <= rs.device.limits().max_buffer_size,
                        "Mesh exceeds GPU buffer limits"
                    );
                    let buffer = |size, usage| {
                        rs.device.create_buffer(&wgpu::BufferDescriptor {
                            label: Some("Scene mesh upload"),
                            size,
                            usage: usage | BufferUsages::COPY_DST,
                            mapped_at_creation: false,
                        })
                    };
                    let uploaded = UploadedMesh {
                        vertex_buffer: buffer(vb as u64, BufferUsages::VERTEX),
                        index_buffer: buffer(ib as u64, BufferUsages::INDEX),
                        primitives: data
                            .primitives
                            .iter()
                            .map(|p| crate::mesh::UploadedPrimitive {
                                indices_start: p.indices_start,
                                indices_num: p.indices_num,
                            })
                            .collect(),
                    };
                    self.current = Some(PendingUpload::Mesh {
                        key,
                        data,
                        uploaded,
                        offset: 0,
                    });
                } else if let Some((key, data)) = self.textures.pop_front() {
                    let max = rs.device.limits().max_texture_dimension_2d;
                    ensure!(
                        data.width <= max && data.height <= max,
                        "Texture dimensions exceed device limits"
                    );
                    let texture = TypedTexture::from_descriptor(
                        &rs.device,
                        &wgpu::TextureDescriptor {
                            label: Some("Scene texture upload"),
                            size: wgpu::Extent3d {
                                width: data.width,
                                height: data.height,
                                depth_or_array_layers: 1,
                            },
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D2,
                            format: if key.1 {
                                TextureFormat::Rgba8UnormSrgb
                            } else {
                                TextureFormat::Rgba8Unorm
                            },
                            usage: TextureUsages::TEXTURE_BINDING
                                | TextureUsages::COPY_DST
                                | TextureUsages::COPY_SRC,
                            view_formats: &[],
                        },
                    );
                    let view = texture.create_view(&TypedTextureViewDescriptor::new(None));
                    self.current = Some(PendingUpload::Texture {
                        key,
                        data,
                        uploaded: UploadedImage { texture, view },
                        row: 0,
                    });
                } else if let Some(description) = self.materials.pop_front() {
                    self.cache.material(description, world)?;
                    continue;
                } else {
                    return Ok(true);
                }
            }
            let complete = match self.current.as_mut().unwrap() {
                PendingUpload::Mesh {
                    data,
                    uploaded,
                    offset,
                    ..
                } => {
                    let vertices = bytemuck::cast_slice::<_, u8>(&data.vertices);
                    let indices = bytemuck::cast_slice::<_, u8>(&data.indices);
                    let (buffer, bytes, local) = if *offset < vertices.len() {
                        (&uploaded.vertex_buffer, vertices, *offset)
                    } else {
                        (&uploaded.index_buffer, indices, *offset - vertices.len())
                    };
                    let count = (bytes.len() - local).min(budget.remaining()) / 4 * 4;
                    if count == 0 {
                        return Ok(false);
                    }
                    rs.queue
                        .write_buffer(buffer, local as u64, &bytes[local..local + count]);
                    *offset += count;
                    budget.consume(count);
                    self.uploaded_bytes += count as u64;
                    *offset == vertices.len() + indices.len()
                }
                PendingUpload::Texture {
                    data,
                    uploaded,
                    row,
                    ..
                } => {
                    let stride = data.width as usize * 4;
                    let rows = budget.rows(stride, data.height - *row);
                    if rows == 0 {
                        return Ok(false);
                    }
                    let count = rows as usize * stride;
                    let start = *row as usize * stride;
                    rs.queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &uploaded.texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d {
                                x: 0,
                                y: *row,
                                z: 0,
                            },
                            aspect: wgpu::TextureAspect::All,
                        },
                        &data.pixels[start..start + count],
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(data.width * 4),
                            rows_per_image: Some(rows),
                        },
                        wgpu::Extent3d {
                            width: data.width,
                            height: rows,
                            depth_or_array_layers: 1,
                        },
                    );
                    *row += rows;
                    budget.consume(count);
                    self.uploaded_bytes += count as u64;
                    *row == data.height
                }
            };
            if complete {
                match self.current.take().unwrap() {
                    PendingUpload::Mesh {
                        key,
                        data,
                        uploaded,
                        ..
                    } => {
                        self.cache.mesh_keys.insert(key, self.cache.meshes.len());
                        self.cache.meshes.push(Arc::new(uploaded));
                        self.cache.mesh_data.push(data);
                    }
                    PendingUpload::Texture { key, uploaded, .. } => {
                        self.cache.textures.insert(key, Arc::new(uploaded));
                    }
                }
            }
        }
        Ok(false)
    }
}
