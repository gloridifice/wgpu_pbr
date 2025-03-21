use std::{f32::consts::PI, sync::Arc};

use bevy_ecs::prelude::*;
use cgmath::{Deg, Point3, Rad, Vector3};
use wgpu::{
    util::DeviceExt, BindGroup, BindGroupLayout, BufferUsages, RenderPipeline, Sampler,
    ShaderModule, ShaderStages, TextureDescriptor, TextureFormat, TextureUsages,
};

use crate::{
    asset::AssetPath, bg_descriptor, bg_layout_descriptor, macro_utils::BGLEntry, RenderState,
};

use super::{camera::OPENGL_TO_WGPU_MATRIX, shader_loader::ShaderLoader, UploadedImage};

pub struct CubemapConverter {
    pub pipeline: RenderPipeline,
    pub sampler: Sampler,
    pub texture_bgl: BindGroupLayout,
    pub matrix_bind_groups: Arc<[BindGroup; 6]>,
    pub format: TextureFormat,
}

#[derive(Debug, Resource)]
pub struct CubemapMatrixBindGroups {
    pub layout: BindGroupLayout,
    pub bind_groups: Arc<[BindGroup; 6]>,
}

#[derive(Resource)]
pub struct CubemapVertexShader {
    pub module: Arc<ShaderModule>,
}

#[derive(Resource)]
pub struct CubemapConverterRgba8unorm(pub CubemapConverter);

impl FromWorld for CubemapMatrixBindGroups {
    fn from_world(world: &mut World) -> Self {
        let rs = world.resource::<RenderState>();
        let device = &rs.device;
        let layout = device.create_bind_group_layout(&bg_layout_descriptor! {
            ["Render Cubemap: Matrix"]
            0: ShaderStages::VERTEX => BGLEntry::UniformBuffer();
        });

        let center = Point3::<f32>::new(0., 0., 0.);
        let proj = cgmath::perspective(Deg(90.0), 1., 0.1, 10.);
        let directions_matrices = [
            ((1., 0., 0.), (0., -1., 0.)),  // +x
            ((-1., 0., 0.), (0., -1., 0.)), // -x
            ((0., 1., 0.), (0., 0., 1.)),   // +y
            ((0., -1., 0.), (0., 0., -1.)), // -y
            ((0., 0., 1.), (0., -1., 0.)),  // +z
            ((0., 0., -1.), (0., -1., 0.)), // -z
        ]
        .map(|((px, py, pz), (ux, uy, uz))| {
            let view = cgmath::Matrix4::look_to_lh(
                center,
                Vector3::new(px, py, pz),
                Vector3::new(ux, uy, uz),
            );
            let mat = proj * view;
            let mat: [[f32; 4]; 4] = mat.into();

            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Render Cubemap Matrix"),
                contents: bytemuck::cast_slice(&[mat]),
                usage: BufferUsages::UNIFORM,
            });
            let bind_group = device.create_bind_group(&bg_descriptor!(
                ["Render Cube Map Matrix"][&layout]
                0: buffer.as_entire_binding();
            ));
            bind_group
        });

        Self {
            layout,
            bind_groups: Arc::new(directions_matrices),
        }
    }
}

impl FromWorld for CubemapConverterRgba8unorm {
    fn from_world(world: &mut World) -> Self {
        let shader_source = world
            .resource_mut::<ShaderLoader>()
            .load_source(AssetPath::new_shader_wgsl("env_to_cubemap"))
            .unwrap();
        let device = &world.resource::<RenderState>().device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Env to Cubemap"),
            source: shader_source,
        });
        let matrix_bind_groups = world.resource::<CubemapMatrixBindGroups>();
        let vert_shader = world.resource::<CubemapVertexShader>();
        Self(CubemapConverter::new(
            device,
            TextureFormat::Rgba8Unorm,
            &shader,
            &matrix_bind_groups,
            &vert_shader,
        ))
    }
}

impl CubemapConverter {
    pub fn new(
        device: &wgpu::Device,
        format: TextureFormat,
        shader: &ShaderModule,
        cubemap_matrices_bind_groups: &CubemapMatrixBindGroups,
        vert_shader: &CubemapVertexShader,
    ) -> Self {
        let texture_bgl = device.create_bind_group_layout(&bg_layout_descriptor! {
                ["Render Cubemap: Texture"]
                0: ShaderStages::FRAGMENT => BGLEntry::Sampler(wgpu::SamplerBindingType::Filtering);
                1: ShaderStages::FRAGMENT => BGLEntry::Tex2D(false, wgpu::TextureSampleType::Float { filterable: true });
            });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Cubemap"),
            bind_group_layouts: &[&cubemap_matrices_bind_groups.layout, &texture_bgl],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("equirectangular to cube map"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &vert_shader.module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[super::utils::cube::cube_vertex_layout()],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
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
                mask: 0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some((format).into())],
            }),
            multiview: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            ..Default::default()
        });

        Self {
            pipeline,
            sampler,
            matrix_bind_groups: Arc::clone(&cubemap_matrices_bind_groups.bind_groups),
            texture_bgl,
            format,
        }
    }

    pub fn render_hdir_to_cube_map(
        &self,
        device: &wgpu::Device,
        source: &wgpu::TextureView,
        cube_vertex_buffer: &wgpu::Buffer,
        piece_size: u32,
    ) -> wgpu::Texture {
        let ret_texture = device.create_texture(&TextureDescriptor {
            label: Some("Cubemap"),
            size: wgpu::Extent3d {
                width: piece_size,
                height: piece_size,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let texture_bind_group = device.create_bind_group(&bg_descriptor!(
            ["Texture BG"][&self.texture_bgl]
            0: wgpu::BindingResource::Sampler(&self.sampler);
            1: wgpu::BindingResource::TextureView(&source);
        ));

        let direction_contexts = self
            .matrix_bind_groups
            .iter()
            .map(|it| {
                let texture = device.create_texture(&TextureDescriptor {
                    label: Some("Render cubemap"),
                    size: wgpu::Extent3d {
                        width: piece_size,
                        height: piece_size,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: self.format,
                    usage: TextureUsages::COPY_SRC | TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                });
                let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                let image = UploadedImage { texture, view };
                (it, image)
            })
            .collect::<Vec<_>>();

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Cubemap"),
        });

        for (matrix_bind_group, target_image) in direction_contexts.iter() {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render cubemap"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_image.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_vertex_buffer(0, cube_vertex_buffer.slice(..));
            render_pass.set_bind_group(0, *matrix_bind_group, &[]);
            render_pass.set_bind_group(1, &texture_bind_group, &[]);
            render_pass.draw(0..36, 0..1)
            //todo draw cube
        }

        for (index, (_, image)) in direction_contexts.iter().enumerate() {
            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfoBase {
                    texture: &image.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfoBase {
                    texture: &ret_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: index as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width: piece_size,
                    height: piece_size,
                    depth_or_array_layers: 1,
                },
            );
        }
        ret_texture
    }
}

impl FromWorld for CubemapVertexShader {
    fn from_world(world: &mut World) -> Self {
        Self {
            module: Arc::new(
                ShaderLoader::load_module_by_world(
                    world,
                    AssetPath::new_shader_wgsl("render_cubemap_vert"),
                )
                .unwrap(),
            ),
        }
    }
}
