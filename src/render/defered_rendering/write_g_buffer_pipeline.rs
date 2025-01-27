use std::sync::Arc;

use wgpu::{BindingResource, RenderPassColorAttachment, Sampler, ShaderStages};

use crate::{
    bg_descriptor, bg_layout_descriptor,
    macro_utils::BGLEntry,
    render::{
        material::pbr::PBRMaterialBindGroupLayout,
        prelude::*,
        UploadedImage,
    },
};

#[derive(Resource, Clone)]
pub struct GBufferTexturesBindGroup {
    pub sampler: Arc<Sampler>,
    pub layout: Arc<BindGroupLayout>,
    pub textures: Vec<GBufferTexture>,
    pub bind_group: Arc<BindGroup>,
}

#[allow(unused)]
#[derive(Clone)]
pub struct GBufferTexture {
    pub label: String,
    pub size: Extent3d,
    pub image: Arc<UploadedImage>,
    pub format: TextureFormat,
}

#[allow(unused)]
#[derive(Resource)]
pub struct WriteGBufferPipeline {
    pub pipeline: RenderPipeline,
    pub pipeline_layout: PipelineLayout,
    pub bind_group_layouts: Vec<Arc<BindGroupLayout>>,
}

impl FromWorld for GBufferTexturesBindGroup {
    fn from_world(world: &mut World) -> Self {
        let rs = world.resource::<crate::RenderState>();
        let device = &rs.device;
        let size = world.resource::<RenderTargetSize>();
        Self::new(device, size.into())
    }
}

impl GBufferTexturesBindGroup {
    pub fn create_textures_and_bind_groups(
        device: &wgpu::Device,
        size: Extent3d,
        sampler: &Sampler,
        layout: &BindGroupLayout,
    ) -> (Vec<GBufferTexture>, Arc<BindGroup>) {
        let textures: Vec<GBufferTexture> = vec![
            ("World Pos", TextureFormat::Rgba8Unorm),
            ("Normal", TextureFormat::Rgba8Unorm),
            // ("TexCoord", TextureFormat::Rg8Unorm),
            ("Base Color", TextureFormat::Rgba8Unorm),
            ("PBR Parameters", TextureFormat::Rgba8Unorm),
        ]
        .into_iter()
        .map(|(label, format)| create_g_buffer_image(label, device, size, format))
        .collect();

        let bind_group = Arc::new(device.create_bind_group(&bg_descriptor! {
            ["GBuffer Textures"][&layout]
            0: BindingResource::Sampler(&sampler);
            1: BindingResource::TextureView(&textures[0].image.view);
            2: BindingResource::TextureView(&textures[1].image.view);
            // 3: BindingResource::TextureView(&textures[2].image.view);
            3: BindingResource::TextureView(&textures[2].image.view);
            4: BindingResource::TextureView(&textures[3].image.view);
        }));

        (textures, bind_group)
    }

    pub fn color_attachments(&self) -> Vec<Option<RenderPassColorAttachment>> {
        let color_attachements = self
            .textures
            .iter()
            .map(|it| {
                Some(wgpu_init::render_pass_color_attachment(
                    &it.image.view,
                    Some(wgpu::Color::TRANSPARENT),
                    true,
                ))
            })
            .collect::<Vec<_>>();

        color_attachements
    }

    pub fn new(device: &wgpu::Device, size: Extent3d) -> Self {
        let sampler = Arc::new(device.create_sampler(&wgpu_init::sampler_desc_no_filter()));
        let layout = Arc::new(device.create_bind_group_layout(&bg_layout_descriptor! {
            ["GBuffert Textures"]
            0: ShaderStages::FRAGMENT => BGLEntry::Sampler(wgpu::SamplerBindingType::NonFiltering); // Universal Sampler
            1: ShaderStages::FRAGMENT => BGLEntry::Tex2D(false, wgpu::TextureSampleType::Float { filterable: false }); // World Pos
            2: ShaderStages::FRAGMENT => BGLEntry::Tex2D(false, wgpu::TextureSampleType::Float { filterable: false }); // Normal
            // 3: ShaderStages::FRAGMENT => BGLEntry::Tex2D(false, wgpu::TextureSampleType::Float { filterable: false }); // TextureCoord
            3: ShaderStages::FRAGMENT => BGLEntry::Tex2D(false, wgpu::TextureSampleType::Float { filterable: false }); // Base Color
            4: ShaderStages::FRAGMENT => BGLEntry::Tex2D(false, wgpu::TextureSampleType::Float { filterable: false }); // PBR Parameters
        }));
        let (textures, bind_group) =
            Self::create_textures_and_bind_groups(device, size, &sampler, &layout);

        Self {
            textures,
            sampler,
            layout,
            bind_group,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32, device: &wgpu::Device) {
        let size = Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        (self.textures, self.bind_group) =
            Self::create_textures_and_bind_groups(device, size, &self.sampler, &self.layout);
    }
}
pub fn create_g_buffer_image(
    label: &str,
    device: &wgpu::Device,
    size: Extent3d,
    format: TextureFormat,
) -> GBufferTexture {
    let desc = wgpu_init::texture_desc_2d_one_mip_sample_level(
        Some("GBuffer Rgba8Unorm Texture"),
        size,
        format,
        TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT,
    );
    let texture = device.create_texture(&desc);
    let view = texture.create_view(&Default::default());
    GBufferTexture {
        label: label.to_string(),
        size,
        image: Arc::new(UploadedImage { texture, view }),
        format,
    }
}

impl FromWorld for WriteGBufferPipeline {
    fn from_world(world: &mut bevy_ecs::world::World) -> Self {
        let rs = world.resource::<RenderState>();

        let device = &rs.device;
        let shader = device.create_shader_module(wgpu::include_wgsl!(
            "../../../assets/shaders/write_g_buffer.wgsl"
        ));

        let global_bind_group_layout =
            Arc::clone(&world.resource::<GBufferGlobalBindGroup>().layout);
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
            Some(wgpu_init::color_target_replace_write_all(
                wgpu::TextureFormat::Rgba8Unorm,
            )),
            // Normal
            Some(wgpu_init::color_target_replace_write_all(
                wgpu::TextureFormat::Rgba8Unorm,
            )),
            // Base Color
            Some(wgpu_init::color_target_replace_write_all(
                wgpu::TextureFormat::Rgba8Unorm,
            )),
            // PBR Parameters
            Some(wgpu_init::color_target_replace_write_all(
                wgpu::TextureFormat::Rgba8Unorm,
            )),
        ];

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Write G-Buffer"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
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
