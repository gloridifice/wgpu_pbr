use std::sync::Arc;

use cgmath::{Deg, Point3, Vector3};
use wgpu::{
    include_wgsl, util::DeviceExt, BindGroup, BindGroupLayout, BufferUsages, RenderPipeline,
    Sampler, ShaderModule, ShaderStages, TextureDescriptor, TextureFormat, TextureUsages,
    VertexBufferLayout,
};

use crate::{bg_descriptor, bg_layout_descriptor, macro_utils::BGLEntry};

use super::UploadedImage;

pub struct CubeMapConverter {
    pub pipeline: RenderPipeline,
    pub sampler: Sampler,
    pub shader: ShaderModule,
    pub directions_matrices: [Arc<BindGroup>; 6],
    pub matrix_bgl: BindGroupLayout,
    pub texture_bgl: BindGroupLayout,
    pub format: TextureFormat,
    vertices_buffer: wgpu::Buffer,
}

impl CubeMapConverter {
    pub fn new(device: &wgpu::Device, format: TextureFormat) -> Self {
        let shader =
            device.create_shader_module(include_wgsl!("../../assets/shaders/env_to_cubemap.wgsl"));
        let matrix_bgl = device.create_bind_group_layout(&bg_layout_descriptor! {
            ["Render Cubemap: Matrix"]
            0: ShaderStages::VERTEX => BGLEntry::UniformBuffer();
        });
        let texture_bgl = device.create_bind_group_layout(&bg_layout_descriptor! {
                ["Render Cubemap: Texture"]
                0: ShaderStages::FRAGMENT => BGLEntry::Sampler(wgpu::SamplerBindingType::Filtering);
                1: ShaderStages::FRAGMENT => BGLEntry::Tex2D(false, wgpu::TextureSampleType::Float { filterable: true });
            });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Cubemap"),
            bind_group_layouts: &[&matrix_bgl, &texture_bgl],
            push_constant_ranges: &[],
        });
        let attris = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("equirectangular to cube map"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[VertexBufferLayout {
                    array_stride: std::mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attris,
                }],
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

        let center = Point3::<f32>::new(0., 0., 0.);
        let proj = cgmath::perspective(Deg(90.), 1., 0.1, 10.);
        let directions_matrices = [
            // +x
            cgmath::Matrix4::look_at_rh(Point3::new(1., 0., 0.), center, Vector3::new(0., -1., 0.)),
            // -x
            cgmath::Matrix4::look_at_rh(
                Point3::new(-1., 0., 0.),
                center,
                Vector3::new(0., -1., 0.),
            ),
            // +y
            cgmath::Matrix4::look_at_rh(Point3::new(0., 1., 0.), center, Vector3::new(0., 0., 1.)),
            // -y
            cgmath::Matrix4::look_at_rh(
                Point3::new(0., -1., 0.),
                center,
                Vector3::new(0., 0., -1.),
            ),
            // +z
            cgmath::Matrix4::look_at_rh(Point3::new(0., 0., 1.), center, Vector3::new(0., -1., 0.)),
            // -z
            cgmath::Matrix4::look_at_rh(
                Point3::new(0., 0., -1.),
                center,
                Vector3::new(0., -1., 0.),
            ),
        ]
        .map(|view| {
            let mat = proj * view;
            let mat: [[f32; 4]; 4] = mat.into();

            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Render Cubemap Matrix"),
                contents: bytemuck::cast_slice(&[mat]),
                usage: BufferUsages::UNIFORM,
            });
            let bind_group = device.create_bind_group(&bg_descriptor!(
                ["Render Cube Map Matrix"][&matrix_bgl]
                0: buffer.as_entire_binding();
            ));
            Arc::new(bind_group)
        });

        let vertices_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&CUBE_VERTICES),
            usage: BufferUsages::VERTEX,
        });

        Self {
            pipeline,
            sampler,
            shader,
            directions_matrices,
            matrix_bgl,
            texture_bgl,
            format,
            vertices_buffer,
        }
    }

    pub fn render_hdir_to_cube_map(
        &self,
        device: &wgpu::Device,
        source: &wgpu::TextureView,
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

        let direction_contexts = self.directions_matrices.clone().map(|it| {
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
        });

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
            render_pass.set_vertex_buffer(0, self.vertices_buffer.slice(..));
            render_pass.set_bind_group(0, matrix_bind_group.as_ref(), &[]);
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

#[rustfmt::skip]
const CUBE_VERTICES: [f32; 288] = [
    // back face
    // Position, Normal, Texcoord
    -1.0, -1.0, -1.0,  0.0,  0.0, -1.0, 0.0, 0.0, // bottom-left
    1.0,  1.0, -1.0,  0.0,  0.0, -1.0, 1.0, 1.0, // top-right
    1.0, -1.0, -1.0,  0.0,  0.0, -1.0, 1.0, 0.0, // bottom-right
    1.0,  1.0, -1.0,  0.0,  0.0, -1.0, 1.0, 1.0, // top-right
    -1.0, -1.0, -1.0,  0.0,  0.0, -1.0, 0.0, 0.0, // bottom-left
    -1.0,  1.0, -1.0,  0.0,  0.0, -1.0, 0.0, 1.0, // top-left
    // front face
    -1.0, -1.0,  1.0,  0.0,  0.0,  1.0, 0.0, 0.0, // bottom-left
    1.0, -1.0,  1.0,  0.0,  0.0,  1.0, 1.0, 0.0, // bottom-right
    1.0,  1.0,  1.0,  0.0,  0.0,  1.0, 1.0, 1.0, // top-right
    1.0,  1.0,  1.0,  0.0,  0.0,  1.0, 1.0, 1.0, // top-right
    -1.0,  1.0,  1.0,  0.0,  0.0,  1.0, 0.0, 1.0, // top-left
    -1.0, -1.0,  1.0,  0.0,  0.0,  1.0, 0.0, 0.0, // bottom-left
    // left face
    -1.0,  1.0,  1.0, -1.0,  0.0,  0.0, 1.0, 0.0, // top-right
    -1.0,  1.0, -1.0, -1.0,  0.0,  0.0, 1.0, 1.0, // top-left
    -1.0, -1.0, -1.0, -1.0,  0.0,  0.0, 0.0, 1.0, // bottom-left
    -1.0, -1.0, -1.0, -1.0,  0.0,  0.0, 0.0, 1.0, // bottom-left
    -1.0, -1.0,  1.0, -1.0,  0.0,  0.0, 0.0, 0.0, // bottom-right
    -1.0,  1.0,  1.0, -1.0,  0.0,  0.0, 1.0, 0.0, // top-right
    // right face
    1.0,  1.0,  1.0,  1.0,  0.0,  0.0, 1.0, 0.0, // top-left
    1.0, -1.0, -1.0,  1.0,  0.0,  0.0, 0.0, 1.0, // bottom-right
    1.0,  1.0, -1.0,  1.0,  0.0,  0.0, 1.0, 1.0, // top-right
    1.0, -1.0, -1.0,  1.0,  0.0,  0.0, 0.0, 1.0, // bottom-right
    1.0,  1.0,  1.0,  1.0,  0.0,  0.0, 1.0, 0.0, // top-left
    1.0, -1.0,  1.0,  1.0,  0.0,  0.0, 0.0, 0.0, // bottom-left
    // bottom face
    -1.0, -1.0, -1.0,  0.0, -1.0,  0.0, 0.0, 1.0, // top-right
    1.0, -1.0, -1.0,  0.0, -1.0,  0.0, 1.0, 1.0, // top-left
    1.0, -1.0,  1.0,  0.0, -1.0,  0.0, 1.0, 0.0, // bottom-left
    1.0, -1.0,  1.0,  0.0, -1.0,  0.0, 1.0, 0.0, // bottom-left
    -1.0, -1.0,  1.0,  0.0, -1.0,  0.0, 0.0, 0.0, // bottom-right
    -1.0, -1.0, -1.0,  0.0, -1.0,  0.0, 0.0, 1.0, // top-right
    // top face
    -1.0,  1.0, -1.0,  0.0,  1.0,  0.0, 0.0, 1.0, // top-left
    1.0,  1.0 , 1.0,  0.0,  1.0,  0.0, 1.0, 0.0, // bottom-right
    1.0,  1.0, -1.0,  0.0,  1.0,  0.0, 1.0, 1.0, // top-right
    1.0,  1.0,  1.0,  0.0,  1.0,  0.0, 1.0, 0.0, // bottom-right
    -1.0,  1.0, -1.0,  0.0,  1.0,  0.0, 0.0, 1.0, // top-left
    -1.0,  1.0,  1.0,  0.0,  1.0,  0.0, 0.0, 0.0  // bottom-left
];
