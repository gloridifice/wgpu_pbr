//! GPU pipeline and resources for entity-ID rendering during the picking pass.

use std::sync::Arc;

use bevy_ecs::prelude::*;
use wgpu::{
    ColorTargetState, ColorWrites, CompareFunction, DepthStencilState, PrimitiveState,
    TextureFormat,
};

use crate::{
    RenderState,
    bindings::{camera_binding::CameraBindGroupLayout, object_binding::ObjectBindGroupLayout},
    prelude::*,
    shader_loader::ShaderLoader,
};
use lentille_wgpu_utils::typed_texture::TypedTextureViewDescriptor;

// ===== Per-entity pick ID uniform =====

/// Uniform buffer storing a per-entity picking ID.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PickIDUniform {
    pub id: u32,
    _padding: [u32; 3],
}

impl PickIDUniform {
    pub fn new(id: u32) -> Self {
        Self {
            id,
            _padding: [0; 3],
        }
    }
}

binding_define! {
    [PickEntityID]
    layout_macro: #[derive(Resource)]
    0: frag => picking_id: TypedBuffer<PickIDUniform>,
}

impl FromWorld for PickEntityIDBindGroupLayout {
    fn from_world(world: &mut World) -> Self {
        Self::new(&world.resource::<RenderState>().device)
    }
}

/// Per-entity bind group containing the picking ID buffer.
#[derive(Component, Clone)]
pub struct PickEntityBindGroup {
    pub bind_group: Arc<BindGroup>,
    #[allow(unused)]
    pub buffer: Arc<TypedBuffer<PickIDUniform>>,
}

impl PickEntityBindGroup {
    pub fn new(device: &wgpu::Device, layout: &PickEntityIDBindGroupLayout, id: u32) -> Self {
        let buffer = Arc::new(TypedBuffer::new_init(
            device,
            Some("Pick ID Buffer"),
            PickIDUniform::new(id),
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        ));
        let bind_group = Arc::new(
            PickEntityIDBindGroupBuilder {
                picking_id: &buffer,
            }
            .build(device, layout),
        );
        Self {
            bind_group,
            buffer,
        }
    }
}

// ===== Pick target =====

/// Bytes of a single `Rgba32Uint` pixel.
const PICK_PIXEL_BYTES: u64 = 16;

/// Render target for the picking pass plus its readback staging buffer.
///
/// The color target has to match the size of the depth buffer it is attached
/// to (wgpu requires every attachment of a render pass to have the same
/// extent), so the texture is created lazily on first use and re-created
/// whenever the render target is resized.
#[derive(Resource, Default)]
pub struct PickTarget {
    size: Option<(u32, u32)>,
    image: Option<Arc<UploadedImage<Dim2D, SampleUint>>>,
    staging: Option<Arc<wgpu::Buffer>>,
}

impl PickTarget {
    /// Make sure the target matches `width`×`height`, re-creating it if needed.
    pub fn ensure_size(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.size == Some((width, height)) && self.image.is_some() && self.staging.is_some() {
            return;
        }

        if self.size != Some((width, height)) || self.image.is_none() {
            self.image = Some(Self::create_image(device, width, height));
            self.size = Some((width, height));
        }
        if self.staging.is_none() {
            self.staging = Some(Arc::new(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Pick Staging Buffer"),
                size: PICK_PIXEL_BYTES,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })));
        }
    }

    fn create_image(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> Arc<UploadedImage<Dim2D, SampleUint>> {
        let desc = wgpu::TextureDescriptor {
            label: Some("Pick Target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TextureFormat::Rgba32Uint,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[TextureFormat::Rgba32Uint],
        };
        let texture: Tex2D<SampleUint> = TypedTexture::from_descriptor(device, &desc);
        let view = texture.create_view(&TypedTextureViewDescriptor::new(Some("Pick Target View")));
        Arc::new(UploadedImage { texture, view })
    }

    pub fn image(&self) -> Option<&Arc<UploadedImage<Dim2D, SampleUint>>> {
        self.image.as_ref()
    }

    pub fn staging(&self) -> Option<&Arc<wgpu::Buffer>> {
        self.staging.as_ref()
    }
}

// ===== Render pipeline =====

#[derive(Resource)]
pub struct PickEntityPipeline {
    pub pipeline: Arc<wgpu::RenderPipeline>,
}

impl FromWorld for PickEntityPipeline {
    fn from_world(world: &mut World) -> Self {
        let shader_source = world
            .resource_mut::<ShaderLoader>()
            .load_source(AssetPath::new_shader_wgsl("pick_entity"))
            .expect("Failed to load pick_entity shader");

        let rs = world.resource::<RenderState>();
        let device = &rs.device;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Pick Entity Shader"),
            source: shader_source,
        });

        let camera_layout = &world.resource::<CameraBindGroupLayout>().0;
        let pick_id_layout = &world.resource::<PickEntityIDBindGroupLayout>().0;
        let object_layout = &world.resource::<ObjectBindGroupLayout>().0;

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Pick Entity Pipeline Layout"),
            bind_group_layouts: &[
                Some(camera_layout),    // group 0: CameraUniform
                Some(pick_id_layout),   // group 1: PickIDUniform (per entity)
                Some(object_layout),    // group 2: TransformUniform
            ],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Pick Entity Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(ColorTargetState {
                    format: TextureFormat::Rgba32Uint,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(DepthStencilState {
                format: RenderState::DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            pipeline: Arc::new(pipeline),
        }
    }
}
