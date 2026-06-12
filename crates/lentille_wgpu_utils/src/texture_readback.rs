use wgpu::{
    BufferDescriptor, BufferUsages, CommandEncoderDescriptor, Device, Extent3d, MapMode, Origin3d,
    Queue, TexelCopyBufferInfoBase, TexelCopyBufferLayout, TexelCopyTextureInfoBase, Texture,
    TextureAspect,
};

const ALIGN: u32 = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;

fn align_up(value: u32, align: u32) -> u32 {
    value.div_ceil(align) * align
}

/// CPU 端的纹理完整拷贝（所有 mip、所有 array layer），行紧凑无 padding。
/// 数据排布顺序：mip 优先，其次 layer。
pub struct TextureReadback {
    pub width: u32,
    pub height: u32,
    pub mip_level_count: u32,
    pub array_layers: u32,
    pub block_size: u32,
    pub data: Vec<u8>,
}

impl TextureReadback {
    pub fn mip_extent(&self, mip: u32) -> (u32, u32) {
        ((self.width >> mip).max(1), (self.height >> mip).max(1))
    }

    pub fn mip_layer_byte_len(&self, mip: u32) -> usize {
        let (w, h) = self.mip_extent(mip);
        (self.block_size * w * h) as usize
    }

    fn mip_layer_offset(&self, mip: u32, layer: u32) -> usize {
        let mut offset = 0usize;
        for m in 0..mip {
            offset += self.mip_layer_byte_len(m) * self.array_layers as usize;
        }
        offset + self.mip_layer_byte_len(mip) * layer as usize
    }

    pub fn mip_layer_slice(&self, mip: u32, layer: u32) -> &[u8] {
        let start = self.mip_layer_offset(mip, layer);
        &self.data[start..start + self.mip_layer_byte_len(mip)]
    }
}

/// 回读整张纹理到 CPU。要求纹理带 `TextureUsages::COPY_SRC`，且为非块压缩格式。
pub fn read_texture_to_cpu(device: &Device, queue: &Queue, texture: &Texture) -> TextureReadback {
    let size = texture.size();
    let format = texture.format();
    let block_size = format
        .block_copy_size(None)
        .expect("block-compressed format is not supported for readback");
    let mip_level_count = texture.mip_level_count();
    let array_layers = size.depth_or_array_layers;

    let mut data = Vec::new();

    for mip in 0..mip_level_count {
        let mw = (size.width >> mip).max(1);
        let mh = (size.height >> mip).max(1);
        let unpadded_bpr = block_size * mw;
        let padded_bpr = align_up(unpadded_bpr, ALIGN);
        let buffer_size = (padded_bpr * mh * array_layers) as u64;

        let staging = device.create_buffer(&BufferDescriptor {
            label: Some("texture readback staging"),
            size: buffer_size,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: None });
        encoder.copy_texture_to_buffer(
            TexelCopyTextureInfoBase {
                texture,
                mip_level: mip,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            TexelCopyBufferInfoBase {
                buffer: &staging,
                layout: TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bpr),
                    rows_per_image: Some(mh),
                },
            },
            Extent3d {
                width: mw,
                height: mh,
                depth_or_array_layers: array_layers,
            },
        );
        queue.submit(std::iter::once(encoder.finish()));

        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .unwrap();
        rx.recv().unwrap().unwrap();

        let mapped = slice.get_mapped_range();
        for layer in 0..array_layers {
            let layer_base = (padded_bpr * mh * layer) as usize;
            for row in 0..mh {
                let row_start = layer_base + (padded_bpr * row) as usize;
                data.extend_from_slice(&mapped[row_start..row_start + unpadded_bpr as usize]);
            }
        }
        drop(mapped);
        staging.unmap();
    }

    TextureReadback {
        width: size.width,
        height: size.height,
        mip_level_count,
        array_layers,
        block_size,
        data,
    }
}
