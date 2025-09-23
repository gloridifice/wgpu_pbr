use std::sync::Arc;

use crate::{
    base_assets::NoFilterClampSampler,
    bindings::global_binding::GlobalBindGroupLayout,
    camera::{RenderTargetResizedEvent, RenderTargetSize},
    prelude::*,
};
use bevy_app::{Plugin, PreUpdate};
use bevy_ecs::prelude::*;
use wgpu::RenderPassColorAttachment;

pub struct WriteGBufferPlugin;

impl Plugin for WriteGBufferPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_render_resource::<GBufferTextureBindGroupLayout>()
            .add_systems(PreUpdate, sys_create_deferred_g_buffer)
            .add_observer(sys_resize_g_buffer_texture);
    }
}

#[derive(Resource, Clone)]
pub struct GBufferTextureBindGroupLayout {
    pub layout: Arc<BindGroupLayout>,
}

/// 挂在相机上
#[derive(Component, Clone)]
pub struct GBufferTexturesBindGroup {
    pub textures: Vec<GBufferTexture>,
    pub bind_group: Arc<BindGroup>,
}

/// 挂在相机上
#[allow(unused)]
#[derive(Clone)]
pub struct GBufferTexture {
    pub image: Arc<UploadedImage>,
}

#[allow(unused)]
#[derive(Resource)]
pub struct DeferredWriteGBufferPipeline {
    pub pipeline: RenderPipeline,
    pub pipeline_layout: PipelineLayout,
    pub bind_group_layouts: Vec<Arc<BindGroupLayout>>,
}

impl GBufferTexturesBindGroup {
    pub fn create_textures_and_bind_groups(
        device: &wgpu::Device,
        size: Extent3d,
        layout: &BindGroupLayout,
        sampler: &Sampler,
    ) -> (Vec<GBufferTexture>, Arc<BindGroup>) {
        let textures: Vec<GBufferTexture> = vec![
            ("World Pos", TextureFormat::Rgba16Float),
            ("G-Buffer", TextureFormat::Rgba32Uint),
        ]
        .into_iter()
        .map(|(_, format)| create_g_buffer_image(device, size, format))
        .collect();

        let bind_group = Arc::new(device.create_bind_group(&bg_descriptor! {
            ["GBuffer Textures"][&layout]
            0: BindingResource::Sampler(sampler);
            1: BindingResource::TextureView(&textures[0].image.view);
            2: BindingResource::TextureView(&textures[1].image.view);
        }));

        (textures, bind_group)
    }

    pub fn color_attachments(&self) -> Vec<Option<RenderPassColorAttachment>> {
        let color_attachements = self
            .textures
            .iter()
            .map(|it| {
                Some(lentille_wgpu_utils::render_pass_color_attachment(
                    &it.image.view,
                    Some(wgpu::Color::TRANSPARENT),
                    true,
                ))
            })
            .collect::<Vec<_>>();

        color_attachements
    }

    pub fn new(
        device: &wgpu::Device,
        size: Extent3d,
        layout: &BindGroupLayout,
        sampler: &Sampler,
    ) -> Self {
        let (textures, bind_group) =
            Self::create_textures_and_bind_groups(device, size, layout, sampler);

        Self {
            textures,
            bind_group,
        }
    }

    pub fn resize(
        &mut self,
        width: u32,
        height: u32,
        device: &wgpu::Device,
        layout: &BindGroupLayout,
        sampler: &Sampler,
    ) {
        let size = Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        (self.textures, self.bind_group) =
            Self::create_textures_and_bind_groups(device, size, layout, sampler);
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
    let texture = device.create_texture(&desc);
    let view = texture.create_view(&Default::default());
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

        let global_bind_group_layout = Arc::clone(&world.resource::<GlobalBindGroupLayout>().0);
        let material_bind_group_layout =
            Arc::clone(&world.resource::<PBRMaterialBindGroupLayout>().0);
        let object_bind_group_layout = Arc::clone(&world.resource::<ObjectBindGroupLayout>().0);

        let bind_group_layouts = vec![
            global_bind_group_layout,
            Arc::clone(&material_bind_group_layout),
            object_bind_group_layout,
        ];

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Write G-Buffer Layout"),
                bind_group_layouts: &bind_group_layouts
                    .iter()
                    .map(|it| it.as_ref())
                    .collect::<Vec<_>>(),
                push_constant_ranges: &[],
            });

        let targets = [
            // World Position
            Some(lentille_wgpu_utils::color_target_replace_write_all(
                wgpu::TextureFormat::Rgba16Float,
            )),
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
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            // relate with array layers
            multiview: None,
            // cache allows wgpu to cache shader compilation data. Only really useful for Android build targets.
            cache: None,
        });

        Self {
            pipeline: render_pipeline,
            pipeline_layout: render_pipeline_layout,
            bind_group_layouts,
        }
    }
}

impl FromWorld for GBufferTextureBindGroupLayout {
    fn from_world(world: &mut World) -> Self {
        let device = &world.resource::<RenderState>().device;
        let layout = Arc::new(device.create_bind_group_layout(&bg_layout_descriptor! {
            ["GBuffert Textures"]
            0: ShaderStages::FRAGMENT => BGLEntry::Sampler(wgpu::SamplerBindingType::NonFiltering); // Universal Sampler
            1: ShaderStages::FRAGMENT => BGLEntry::Tex2D(false, wgpu::TextureSampleType::Float { filterable: false }); // World Pos
            2: ShaderStages::FRAGMENT => BGLEntry::Tex2D(false, wgpu::TextureSampleType::Uint); // G-Buffer
        }));

        Self { layout }
    }
}

fn sys_create_deferred_g_buffer(
    mut commands: Commands,
    q_camera: Query<(Entity, &RenderTargetSize), Without<GBufferTexturesBindGroup>>,
    rs: Res<RenderState>,
    bgl: Res<GBufferTextureBindGroupLayout>,
    no_filter_sampler: Res<NoFilterClampSampler>,
) {
    for (id, size) in q_camera {
        commands.entity(id).insert(GBufferTexturesBindGroup::new(
            &rs.device,
            Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            &bgl.layout,
            &no_filter_sampler.0,
        ));
    }
}

fn sys_resize_g_buffer_texture(
    event: Trigger<RenderTargetResizedEvent>,
    q_camera: Query<&mut GBufferTexturesBindGroup, With<RenderTargetSize>>,
    rs: Res<RenderState>,
    bgl: Res<GBufferTextureBindGroupLayout>,
    no_filter_sampler: Res<NoFilterClampSampler>,
) {
    for mut bg in q_camera {
        *bg.as_mut() = GBufferTexturesBindGroup::new(
            &rs.device,
            Extent3d {
                width: event.new_width,
                height: event.new_height,
                depth_or_array_layers: 1,
            },
            &bgl.layout,
            &no_filter_sampler.0,
        );
    }
}
