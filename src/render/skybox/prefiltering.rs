use std::sync::Arc;

use bevy_ecs::prelude::*;
use wgpu::{
    util::DeviceExt, BindGroupLayout, BindingResource, BufferUsages, CommandEncoderDescriptor,
    PipelineLayout, RenderPipeline, SamplerBindingType, ShaderStages, TextureFormat,
    TextureUsages,
};

use crate::{
    asset::AssetPath,
    bg_descriptor, bg_layout_descriptor, impl_pod_zeroable,
    macro_utils::BGLEntry,
    render::{
        self,
        cubemap::{CubemapMatrixBindGroups, CubemapVertexShader},
        shader_loader::ShaderLoader,
        utils::cube::CubeVerticesBuffer,
        UploadedImage,
    },
    wgpu_init,
};

const LABEL: Option<&'static str> = Some("Prefiltering Env Map");

#[derive(Resource)]
pub struct PrefilteringPipeline {
    pub pipeline: Arc<RenderPipeline>,
    pub layout: Arc<PipelineLayout>,
    pub uniform_bind_group_layout: Arc<BindGroupLayout>,
}

#[derive(Debug, Clone, Copy)]
pub struct PrefilteringEnvironmentUniform {
    pub roughness: f32,
    pub sample_count: u32,
}

impl_pod_zeroable!(PrefilteringEnvironmentUniform);

impl FromWorld for PrefilteringPipeline {
    fn from_world(world: &mut World) -> Self {
        let shader = ShaderLoader::load_module_by_world(
            world,
            AssetPath::new_shader_wgsl("prefiltering_env_map"),
        )
        .unwrap();

        let rs = world.resource::<crate::RenderState>();
        let device = &rs.device;

        let bg_layout = device.create_bind_group_layout(&bg_layout_descriptor! {
            ["Prefiltering Env Map"]
            0: ShaderStages::FRAGMENT => BGLEntry::UniformBuffer();
            1: ShaderStages::FRAGMENT => BGLEntry::TexCube(false, wgpu::TextureSampleType::Float { filterable: true });
            2: ShaderStages::FRAGMENT => BGLEntry::Sampler(SamplerBindingType::Filtering);
        });

        let matrix_bind_group_layout = world.resource::<CubemapMatrixBindGroups>();

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: LABEL,
            bind_group_layouts: &[&matrix_bind_group_layout.layout, &bg_layout],
            push_constant_ranges: &[],
        });

        let vert_shader = world.resource::<CubemapVertexShader>();

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: LABEL,
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &vert_shader.module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[render::utils::cube::cube_vertex_layout()],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Front),
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
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(TextureFormat::Rgba8UnormSrgb.into())],
            }),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline: Arc::new(pipeline),
            layout: Arc::new(layout),
            uniform_bind_group_layout: Arc::new(bg_layout),
        }
    }
}

// pub struct PrefilteringContext<'a> {
//     pub label: Option<&'static str>,
//     pub texture: &'a Texture,
//     pub view: &'a TextureView,
//     pub level_count: u32,
//     pub sample_count: u32,
// }

// pub fn sys_prefilter_environment_map(
//     input: In<&PrefilteringContext>,
//     rs: Res<RenderState>,
//     pipeline: Res<PrefilteringPipeline>,
//     matrix_bind_groups: Res<CubemapMatrixBindGroups>,
// ) -> anyhow::Result<UploadedImage> {
//     let In(input) = input;
//     prefilter(
//         input.label,
//         &rs.device,
//         &rs.queue,
//         input.texture,
//         input.view,
//         input.level_count,
//         input.sample_count,
//         &pipeline,
//         &matrix_bind_groups,
//     )
// }

/// 5 level is enough
pub fn prefilter(
    label: Option<&'static str>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    source_texture: &wgpu::Texture,
    source_view: &wgpu::TextureView,
    level_count: u32,
    sample_count: u32,

    pipeline: &PrefilteringPipeline,
    matrix_bind_groups: &CubemapMatrixBindGroups,
    cube_vertex_buffer: &CubeVerticesBuffer,
) -> anyhow::Result<UploadedImage> {
    let size = source_texture.size();
    if size.depth_or_array_layers != 6 {
        return Err(anyhow::anyhow!("Not a cubemap!"));
    }
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label,
        size,
        mip_level_count: level_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: source_texture.format(),
        usage: source_texture.usage() | TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_DST,
        view_formats: &[],
    });

    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::Cube),
        ..Default::default()
    });

    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: None });

    encoder.copy_texture_to_texture(
        wgpu::TexelCopyTextureInfoBase {
            texture: source_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyTextureInfoBase {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: 6,
        },
    );

    for level in 1..level_count {
        let roughness = 1.0 / (level_count as f32) * (level as f32);
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[PrefilteringEnvironmentUniform {
                roughness,
                sample_count,
            }]),
            usage: BufferUsages::UNIFORM,
        });
        let sampler = device.create_sampler(&wgpu_init::sampler_desc(
            None,
            wgpu::AddressMode::Repeat,
            wgpu::FilterMode::Linear,
        ));
        let uniform_bind_group = device.create_bind_group(&bg_descriptor!(
            ["Prefiltering Uniform"][&pipeline.uniform_bind_group_layout]
            0: buffer.as_entire_binding();
            1: BindingResource::TextureView(source_view);
            2: BindingResource::Sampler(&sampler);
        ));
        for j in 0..6 {
            let target = texture.create_view(&wgpu::TextureViewDescriptor {
                label: None,
                dimension: Some(wgpu::TextureViewDimension::D2),
                usage: Some(wgpu::TextureUsages::RENDER_ATTACHMENT),
                base_mip_level: level,
                mip_level_count: Some(1),
                base_array_layer: j,
                array_layer_count: Some(1),
                ..Default::default()
            });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Prefiltering"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            pass.set_pipeline(&pipeline.pipeline);
            pass.set_vertex_buffer(0, cube_vertex_buffer.vertices_buffer.slice(..));
            pass.set_bind_group(
                0,
                matrix_bind_groups.bind_groups.get(j as usize).unwrap(),
                &[],
            );
            pass.set_bind_group(1, &uniform_bind_group, &[]);
            pass.draw(0..36, 0..1);
        }
    }

    queue.submit(std::iter::once(encoder.finish()));
    // TODO write texture

    Ok(UploadedImage { texture, view })
}
