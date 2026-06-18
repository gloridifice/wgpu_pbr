use lentille_render::SCREEN_FORMAT;

const BLIT_VERTEX: &str = include_str!("shaders/preview_blit_vertex.wgsl");
const BLIT_FRAGMENT: &str = include_str!("shaders/preview_blit_fragment.wgsl");

/// Bundles the wgpu resources needed for blitting CSM preview quads on top
/// of the iced UI.
pub(crate) struct PreviewBlitResources {
    pub pipeline: wgpu::RenderPipeline,
    pub sampler: wgpu::Sampler,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub vertex_buf: wgpu::Buffer,
}

pub(crate) fn create_preview_blit_resources(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> PreviewBlitResources {
    let bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("preview_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

    let pipeline = create_preview_blit_pipeline(device, SCREEN_FORMAT, &bind_group_layout);

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("preview_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct BlitVertex {
        pos: [f32; 2],
        uv: [f32; 2],
    }
    let quad_verts: &[BlitVertex] = &[
        BlitVertex { pos: [-1.0, -1.0], uv: [0.0, 1.0] },
        BlitVertex { pos: [1.0, -1.0], uv: [1.0, 1.0] },
        BlitVertex { pos: [1.0, 1.0], uv: [1.0, 0.0] },
        BlitVertex { pos: [-1.0, -1.0], uv: [0.0, 1.0] },
        BlitVertex { pos: [1.0, 1.0], uv: [1.0, 0.0] },
        BlitVertex { pos: [-1.0, 1.0], uv: [0.0, 0.0] },
    ];
    let vertex_buf = {
        let bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(
                quad_verts.as_ptr() as *const u8,
                std::mem::size_of_val(quad_verts),
            )
        };
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("preview_quad_vbuf"),
            size: bytes.len() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&buf, 0, bytes);
        buf
    };

    PreviewBlitResources {
        pipeline,
        sampler,
        bind_group_layout,
        vertex_buf,
    }
}

fn create_preview_blit_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    bgl: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let vert = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("preview_blit_vert"),
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(BLIT_VERTEX)),
    });
    let frag = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("preview_blit_frag"),
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(BLIT_FRAGMENT)),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("preview_blit_layout"),
        bind_group_layouts: &[bgl],
        push_constant_ranges: &[],
    });

    let vertex_buffers = &[wgpu::VertexBufferLayout {
        array_stride: 16,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 8,
                shader_location: 1,
            },
        ],
    }];

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("preview_blit"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &vert,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: vertex_buffers,
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
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
            module: &frag,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview: None,
        cache: None,
    })
}
