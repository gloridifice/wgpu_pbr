use wgpu::{
    BindGroupLayoutEntry, BindingType, ColorTargetState, Extent3d, Origin3d,
    PipelineCompilationOptions, PipelineLayout, RenderPassColorAttachment,
    RenderPipelineDescriptor, SamplerDescriptor, ShaderModule, ShaderStages, TextureDescriptor,
    TextureFormat, TextureView, VertexBufferLayout, VertexState,
};

// Allow the generated code from `binding_define!` to reference this crate by its
// canonical name even when expanded inside this crate itself.
extern crate self as lentille_wgpu_utils;

pub mod bind_group_macro;
pub mod texture_readback;
pub mod typed_binding_resource;
pub mod typed_buffer;
pub mod typed_sampler;
pub mod typed_texture;

pub use lentille_wgpu_macros::binding_define;
pub use typed_buffer::TypedBuffer;

#[macro_export]
macro_rules! impl_type_state {
    (
        $vis:vis trait $trait_name:ident for $enum_name:ident {
            $( $struct_name:ident => $variant:ident $( { $($fields:tt)* } )? ),+ $(,)?
        }
    ) => {
        // 生成本地约束 Trait
        $vis trait $trait_name {
            const VALUE: $enum_name;
        }

        $(
            // 生成状态结构体
            $vis struct $struct_name;

            // 绑定第三方枚举变体
            impl $trait_name for $struct_name {
                const VALUE: $enum_name = $enum_name::$variant $( { $($fields)* } )?;
            }
        )+
    };
}

pub const fn bind_group_layout_entry_shader(binding: u32, ty: BindingType) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::VERTEX,
        ty,
        count: None,
    }
}

pub fn texture_desc_2d_one_mip_sample_level(
    label: Option<&str>,
    size: Extent3d,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> TextureDescriptor<'_> {
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
        depth_slice: None,
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
            entry_point: Some("vs_main"),
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
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        fragment: Some(wgpu::FragmentState {
            module: frag,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets,
        }),
        multiview: None,
        cache: None,
    }
}

pub fn no_depth_stencil_pipeline_desc<'a>(
    label: Option<&'a str>,
    layout: &'a PipelineLayout,
    vert: &'a ShaderModule,
    vert_buffers: &'a [VertexBufferLayout<'a>],
    frag: &'a ShaderModule,
    targets: &'a [Option<ColorTargetState>],
) -> RenderPipelineDescriptor<'a> {
    wgpu::RenderPipelineDescriptor {
        label,
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: vert,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: vert_buffers,
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
            module: frag,
            entry_point: Some("fs_main"),
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
        entry_point: Some("vs_main"),
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

pub fn copy_texture2d_to_texture2d_no_mip(
    encoder: &mut wgpu::CommandEncoder,
    source: &wgpu::Texture,
    target: &wgpu::Texture,
    size: Extent3d,
) {
    encoder.copy_texture_to_texture(
        wgpu::TexelCopyTextureInfoBase {
            texture: source,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyTextureInfoBase {
            texture: target,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        size,
    );
}
