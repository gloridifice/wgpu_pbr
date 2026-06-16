use std::sync::Arc;

use crate::{
    bindings::camera_binding::CameraBindGroupLayout,
    camera::{RenderTarget, RenderTargetResizedEvent, RenderTargetSize},
    prelude::*,
};
use bevy_app::{Plugin, PreUpdate};
use bevy_ecs::prelude::*;
use lentille_wgpu_macros::DeviceNewFromWorld;
use lentille_wgpu_utils::typed_texture::{TypedTexture, TypedTextureViewDescriptor};
use wgpu::RenderPassColorAttachment;

pub struct WriteGBufferPlugin;

impl Plugin for WriteGBufferPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_render_resource::<GBufferTextureBindGroupLayout>()
            .add_systems(PreUpdate, sys_create_deferred_g_buffer)
            .add_observer(sys_resize_g_buffer_texture);
    }
}

binding_define! {
    [GBufferTexture]
    layout_macro: #[derive(Resource, DeviceNewFromWorld)]
    0: frag => depth_buffer: TexView2D<SampleDepth>,
    1: frag => g_buffer: TexView2D<SampleUint>
}

/// 挂在相机上
#[derive(Component, Clone)]
pub struct GBufferTextureBindGroup {
    pub texture: GBufferTexture,
    pub bind_group: Arc<BindGroup>,
}

/// 挂在相机上
#[allow(unused)]
#[derive(Clone)]
pub struct GBufferTexture {
    pub image: Arc<UploadedImage<Dim2D, SampleUint>>,
}

#[allow(unused)]
#[derive(Resource)]
pub struct DeferredWriteGBufferPipeline {
    pub pipeline: RenderPipeline,
    pub pipeline_layout: PipelineLayout,
}

impl GBufferTextureBindGroup {
    fn create_texture_and_bind_group(
        device: &wgpu::Device,
        size: Extent3d,
        layout: &GBufferTextureBindGroupLayout,
        depth_view: &TexView2D<SampleDepth>,
    ) -> (GBufferTexture, Arc<BindGroup>) {
        // G-buffer 现在只输出一张 packed Uint 纹理；世界坐标在 PBR 主通道
        let texture = create_g_buffer_image(device, size, TextureFormat::Rgba32Uint);

        let bind_group = Arc::new(
            GBufferTextureBindGroupBuilder {
                depth_buffer: depth_view,
                g_buffer: &texture.image.view,
            }
            .build(device, layout),
        );

        (texture, bind_group)
    }

    pub fn color_attachment(&self) -> Option<RenderPassColorAttachment> {
        Some(lentille_wgpu_utils::render_pass_color_attachment(
            &self.texture.image.view,
            Some(wgpu::Color::TRANSPARENT),
            true,
        ))
    }

    pub fn new(
        device: &wgpu::Device,
        size: Extent3d,
        layout: &GBufferTextureBindGroupLayout,
        depth_view: &TexView2D<SampleDepth>,
    ) -> Self {
        let (texture, bind_group) =
            Self::create_texture_and_bind_group(device, size, layout, depth_view);

        Self {
            texture,
            bind_group,
        }
    }
}

pub fn create_g_buffer_image(
    device: &wgpu::Device,
    size: Extent3d,
    format: TextureFormat,
) -> GBufferTexture {
    let desc = lentille_wgpu_utils::texture_desc_2d_one_mip_sample_level(
        Some("GBuffer Rgba8Unorm Texture"),
        size,
        format,
        TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT,
    );
    let texture: Tex2D<SampleUint> = TypedTexture::from_descriptor(device, &desc);
    let view = texture.create_view(&TypedTextureViewDescriptor::new(None));
    GBufferTexture {
        image: Arc::new(UploadedImage { texture, view }),
    }
}

impl FromWorld for DeferredWriteGBufferPipeline {
    fn from_world(world: &mut bevy_ecs::world::World) -> Self {
        let shader_source = world
            .resource_mut::<ShaderLoader>()
            .load_source(AssetPath::new_shader_wgsl("write_g_buffer"))
            .unwrap();
        let rs = world.resource::<RenderState>();

        let device = &rs.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Write G-Buffer"),
            source: shader_source,
        });

        let global_bind_group_layout = &world.resource::<CameraBindGroupLayout>().0;
        let material_bind_group_layout = &world.resource::<PbrMaterialBindGroupLayout>().0;
        let object_bind_group_layout = &world.resource::<ObjectBindGroupLayout>().0;

        let bind_group_layouts = vec![
            Some(global_bind_group_layout),
            Some(material_bind_group_layout),
            Some(object_bind_group_layout),
        ];

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Write G-Buffer Layout"),
                bind_group_layouts: &bind_group_layouts,
                immediate_size: 0,
            });

        let targets = [
            // G-Buffer
            Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba32Uint,
                blend: None,
                write_mask: ColorWrites::ALL,
            }),
        ];

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Write G-Buffer"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &targets,
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            //The `primitive` field describes how to interpret our vertices when converting them into triangles.
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: RenderState::DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            // relate with array layers
            multiview_mask: None,
            // cache allows wgpu to cache shader compilation data. Only really useful for Android build targets.
            cache: None,
        });

        Self {
            pipeline: render_pipeline,
            pipeline_layout: render_pipeline_layout,
        }
    }
}

fn sys_create_deferred_g_buffer(
    mut commands: Commands,
    q_camera: Query<(Entity, &RenderTargetSize, &RenderTarget), Without<GBufferTextureBindGroup>>,
    rs: Res<RenderState>,
    bgl: Res<GBufferTextureBindGroupLayout>,
) {
    for (id, size, target) in q_camera {
        let Some(depth) = target.depth.as_ref() else {
            continue;
        };
        commands.entity(id).insert(GBufferTextureBindGroup::new(
            &rs.device,
            Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            &bgl,
            &depth.view,
        ));
    }
}

fn sys_resize_g_buffer_texture(
    event: On<RenderTargetResizedEvent>,
    mut q_camera: Query<(&mut GBufferTextureBindGroup, &RenderTarget), With<RenderTargetSize>>,
    rs: Res<RenderState>,
    bgl: Res<GBufferTextureBindGroupLayout>,
) {
    let RenderTargetResizedEvent {
        render_target_entity,
        new_width,
        new_height,
    } = *event;

    if let Ok((mut bg, target)) = q_camera.get_mut(render_target_entity) {
        let Some(depth) = target.depth.as_ref() else {
            return;
        };
        *bg.as_mut() = GBufferTextureBindGroup::new(
            &rs.device,
            Extent3d {
                width: new_width,
                height: new_height,
                depth_or_array_layers: 1,
            },
            &bgl,
            &depth.view,
        );
    }
}
