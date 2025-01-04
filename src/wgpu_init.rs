use wgpu::{
    BindGroupLayoutEntry, BindingType, ColorTargetState, Extent3d, PipelineLayout,
    RenderPassColorAttachment, RenderPipelineDescriptor, SamplerDescriptor, ShaderModule,
    ShaderStages, TextureDescriptor, TextureFormat, TextureView,
};

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
