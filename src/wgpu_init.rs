use wgpu::{
    util::{DeviceExt, TextureDataOrder},
    BindGroupLayoutEntry, BindingType, ColorTargetState, Extent3d, PipelineCompilationOptions,
    PipelineLayout, RenderPassColorAttachment, RenderPipelineDescriptor, SamplerDescriptor,
    ShaderModule, ShaderStages, TextureDescriptor, TextureFormat, TextureUsages, TextureView,
    VertexBufferLayout, VertexState,
};

use crate::{math_type::Vec4, render::UploadedImageWithSampler};

// pub struct DynamicBuffer<'a> {
//     pub size: u64,
//     pub buffer: Arc<wgpu::Buffer>,
//     pub desc: BufferDescriptor<'a>,
// }

// impl<'a> DynamicBuffer<'a> {
//     pub fn new(device: &wgpu::Device, desc: BufferDescriptor<'a>) -> Self {
//         let size = desc.size;
//         let buffer = Arc::new(device.create_buffer(&desc));
//         Self { size, buffer, desc }
//     }

//     pub fn write_buffer(
//         &self,
//         queue: wgpu::Queue,
//         device: wgpu::Device,
//         offset: u64,
//         data: &[u8],
//     ) -> Option<Arc<Buffer>> {
//         let required_size = size_of_val(data) as u64 + offset;
//         let is_oversize = required_size > self.size;
//         if is_oversize {
//             let buffer = device.create_buffer(&self.desc);
//             let desc = self.desc.clone();
//             desc.size = queue.write_buffer(buffer, offset, data)
//         } else {
//             queue.write_buffer(&self.buffer, offset, data);
//         };
//         None
//     }

//     pub fn calculate_new_size(&self, target_size: u64) {
//         let mut size = self.size;
//         while size < target_size {}
//     }
// }

pub const fn bind_group_layout_entry_shader(binding: u32, ty: BindingType) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::VERTEX,
        ty,
        count: None,
    }
}

pub fn texture_desc_2d_one_mip_sample_level<'a>(
    label: Option<&'a str>,
    size: Extent3d,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> TextureDescriptor<'a> {
    TextureDescriptor {
        label,
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    }
}

pub fn render_pass_color_attachment(
    view: &TextureView,
    load_color: Option<wgpu::Color>,
    is_store_op_store: bool,
) -> RenderPassColorAttachment {
    RenderPassColorAttachment {
        view,
        resolve_target: None,
        ops: wgpu::Operations {
            load: match load_color {
                Some(color) => wgpu::LoadOp::Clear(color),
                None => wgpu::LoadOp::Load,
            },
            store: match is_store_op_store {
                true => wgpu::StoreOp::Store,
                false => wgpu::StoreOp::Discard,
            },
        },
    }
}

pub fn sampler_desc(
    label: Option<&'static str>,
    address_mode: wgpu::AddressMode,
    mag_min_filter: wgpu::FilterMode,
) -> SamplerDescriptor<'static> {
    SamplerDescriptor {
        label,
        address_mode_u: address_mode,
        address_mode_v: address_mode,
        address_mode_w: address_mode,
        mag_filter: mag_min_filter,
        min_filter: mag_min_filter,
        ..Default::default()
    }
}

pub fn sampler_desc_no_filter() -> SamplerDescriptor<'static> {
    wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::FilterMode::Nearest,
        compare: None,
        lod_min_clamp: 0.0,
        lod_max_clamp: 100.0,
        ..Default::default()
    }
}

pub fn full_screen_pipeline_desc<'a>(
    label: Option<&'a str>,
    layout: &'a PipelineLayout,
    vert: &'a ShaderModule,
    frag: &'a ShaderModule,
    targets: &'a [Option<ColorTargetState>],
) -> RenderPipelineDescriptor<'a> {
    wgpu::RenderPipelineDescriptor {
        label,
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: vert,
            entry_point: "vs_main",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: 0,
            alpha_to_coverage_enabled: false,
        },
        fragment: Some(wgpu::FragmentState {
            module: &frag,
            entry_point: "fs_main",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets,
        }),
        multiview: None,
        cache: None,
    }
}

pub fn color_target_replace_write_all(format: TextureFormat) -> ColorTargetState {
    wgpu::ColorTargetState {
        format,
        blend: Some(wgpu::BlendState::REPLACE),
        write_mask: wgpu::ColorWrites::ALL,
    }
}

pub fn vertex_state<'a>(
    module: &'a ShaderModule,
    buffers: &'a [VertexBufferLayout<'a>],
) -> VertexState<'a> {
    VertexState {
        module,
        entry_point: "vs_main",
        compilation_options: PipelineCompilationOptions::default(),
        buffers,
    }
}

pub fn primitive_triangle_list_default() -> wgpu::PrimitiveState {
    wgpu::PrimitiveState {
        topology: wgpu::PrimitiveTopology::TriangleList,
        strip_index_format: None,
        front_face: wgpu::FrontFace::Ccw,
        cull_mode: Some(wgpu::Face::Back),
        polygon_mode: wgpu::PolygonMode::Fill,
        unclipped_depth: false,
        conservative: false,
    }
}

pub fn create_pure_color_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    color: Vec4,
) -> UploadedImageWithSampler {
    let size = Extent3d {
        width: 1,
        height: 1,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture_with_data(
        queue,
        &TextureDescriptor {
            label: None,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        TextureDataOrder::LayerMajor,
        &[
            (color.x * 255.) as u8,
            (color.y * 255.) as u8,
            (color.z * 255.) as u8,
            (color.w * 255.) as u8,
        ],
    );
    let sampler = device.create_sampler(&sampler_desc(
        None,
        wgpu::AddressMode::Repeat,
        wgpu::FilterMode::Linear,
    ));
    let view = texture.create_view(&Default::default());

    UploadedImageWithSampler {
        texture,
        view,
        size,
        sampler,
    }
}
