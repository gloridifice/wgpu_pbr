use std::fs;
use std::path::Path;

use lentille_wgpu_utils::texture_readback::read_texture_to_cpu;
use lentille_wgpu_utils::typed_texture::{TypedTexture, TypedTextureViewDescriptor};
use wgpu::{Extent3d, Origin3d, TexelCopyBufferLayout, TexelCopyTextureInfoBase, TextureAspect};

use crate::prelude::*;

const MAGIC: &[u8; 4] = b"LPFC";
const VERSION: u32 = 1;

fn format_tag(format: TextureFormat) -> u32 {
    match format {
        TextureFormat::Rgba8UnormSrgb => 1,
        TextureFormat::Rgba8Unorm => 2,
        TextureFormat::Rgba16Float => 3,
        _ => 0,
    }
}

fn format_from_tag(tag: u32) -> Option<TextureFormat> {
    match tag {
        1 => Some(TextureFormat::Rgba8UnormSrgb),
        2 => Some(TextureFormat::Rgba8Unorm),
        3 => Some(TextureFormat::Rgba16Float),
        _ => None,
    }
}

/// 哈希 6 张源图原始字节 + 预过滤参数 + 输出格式，用于缓存失效判定。
pub fn fingerprint(
    source_bytes: &[Vec<u8>],
    level_count: u32,
    sample_count: u32,
    format: TextureFormat,
) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for bytes in source_bytes {
        bytes.hash(&mut hasher);
    }
    level_count.hash(&mut hasher);
    sample_count.hash(&mut hasher);
    format_tag(format).hash(&mut hasher);
    hasher.finish()
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn u32(&mut self) -> Option<u32> {
        let bytes = self.bytes(4)?;
        Some(u32::from_le_bytes(bytes.try_into().ok()?))
    }

    fn u64(&mut self) -> Option<u64> {
        let bytes = self.bytes(8)?;
        Some(u64::from_le_bytes(bytes.try_into().ok()?))
    }

    fn bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.pos + n;
        let slice = self.buf.get(self.pos..end)?;
        self.pos = end;
        Some(slice)
    }
}

/// 尝试从磁盘缓存加载预过滤 cubemap。指纹/版本/格式不匹配或文件损坏时返回 `None`。
pub fn try_load(
    path: &str,
    expected_fingerprint: u64,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Option<UploadedImage<DimCube, SampleFloatFilterable>> {
    let bytes = fs::read(path).ok()?;
    let mut reader = Reader::new(&bytes);

    if reader.bytes(4)? != MAGIC {
        return None;
    }
    if reader.u32()? != VERSION {
        return None;
    }
    if reader.u64()? != expected_fingerprint {
        return None;
    }
    let format = format_from_tag(reader.u32()?)?;
    let width = reader.u32()?;
    let height = reader.u32()?;
    let mip_level_count = reader.u32()?;
    let array_layers = reader.u32()?;
    if array_layers != 6 {
        return None;
    }

    let texture: Tex2D<SampleFloatFilterable> =
        TypedTexture::from_descriptor(device, &wgpu::TextureDescriptor {
            label: Some("Cached Prefiltered Skybox"),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: array_layers,
            },
            mip_level_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::COPY_SRC,
            view_formats: &[],
        });

    let block_size = format.block_copy_size(None).unwrap_or(4);
    for mip in 0..mip_level_count {
        let mw = (width >> mip).max(1);
        let mh = (height >> mip).max(1);
        let bpr = block_size * mw;
        let len = (bpr * mh) as usize;
        for layer in 0..array_layers {
            let slice = reader.bytes(len)?;
            queue.write_texture(
                TexelCopyTextureInfoBase {
                    texture: texture.texture(),
                    mip_level: mip,
                    origin: Origin3d {
                        x: 0,
                        y: 0,
                        z: layer,
                    },
                    aspect: TextureAspect::All,
                },
                slice,
                TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bpr),
                    rows_per_image: Some(mh),
                },
                Extent3d {
                    width: mw,
                    height: mh,
                    depth_or_array_layers: 1,
                },
            );
        }
    }

    let view = texture.create_view(&TypedTextureViewDescriptor::new(Some(
        "Cached Prefiltered Skybox",
    )));

    Some(UploadedImage { texture, view })
}

/// 将预过滤后的 cubemap 纹理回读并写入磁盘缓存（先写临时文件再 rename，防写入中断损坏）。
pub fn save(
    path: &str,
    fingerprint: u64,
    texture: &Tex2D<SampleFloatFilterable>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<()> {
    let readback = read_texture_to_cpu(device, queue, texture.texture());
    let format = texture.format();

    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent)?;
    }

    let mut buf = Vec::with_capacity(32 + readback.data.len());
    buf.extend_from_slice(MAGIC);
    buf.extend_from_slice(&VERSION.to_le_bytes());
    buf.extend_from_slice(&fingerprint.to_le_bytes());
    buf.extend_from_slice(&format_tag(format).to_le_bytes());
    buf.extend_from_slice(&readback.width.to_le_bytes());
    buf.extend_from_slice(&readback.height.to_le_bytes());
    buf.extend_from_slice(&readback.mip_level_count.to_le_bytes());
    buf.extend_from_slice(&readback.array_layers.to_le_bytes());
    buf.extend_from_slice(&readback.data);

    let tmp = format!("{path}.tmp");
    fs::write(&tmp, &buf)?;
    fs::rename(&tmp, path)?;
    Ok(())
}
